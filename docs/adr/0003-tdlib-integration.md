# ADR 0003: TDLib integration

- Status: accepted
- Date: 2026-10-25

## Context

rgram uses TDLib for MTProto, encryption, sync, the local database and files
(writing our own MTProto is slow and a security risk). TDLib's source is pinned in
`tdlib/COMMIT` (`ea3f69ce1`, version 1.8.69).

The obvious Rust binding is `tdlib-rs`. Its latest release, 1.4.0 (also the
version `tgt` locks), was checked from the crate source:

- `TDLIB_VERSION = "1.8.61"`, and the bundled `tl/api.tl` is the 1.8.61 schema.
  Against our 1.8.69 `td_api.tl` the diff is about 4,700 lines, so the generated
  types would not match the library we link.
- The runtime keeps a process-wide `static OBSERVER` and a global `@extra`
  counter, `unwrap()`s the JSON of every received object, parses each object
  twice (into `serde_json::Value`, then into the typed value), and the generated
  request functions `unwrap()` the response deserialization. A schema drift or a
  malformed message would abort the process (we build with `panic = "abort"`).

Both conflict with our rules (no global mutable singletons, no `unwrap()` outside
tests), and the version mismatch alone rules out using its generated types.

## Decision

### Code generation

- `crates/td-api/tl/td_api.tl` is a verbatim copy of the schema at the pinned
  commit. `scripts/check-td-schema.sh` (run in CI) verifies the copy against
  `tdlib/td` at `tdlib/COMMIT`.
- `crates/td-api-gen` is our generator, used only as a build-dependency. It parses
  the schema with `tdlib-rs-parser` (parser only, MIT/Apache) and emits:
  - one struct per constructor, one enum per abstract type, with serde impls that
    follow TDLib's JSON conventions (`int64` and `bytes` as strings, `@type`
    tags, optional fields as `Option`);
  - one request struct per function implementing
    `trait Function: Serialize { type Response: DeserializeOwned; }`, so
    `client.request(GetOption { name }).await` is typed end to end;
  - enums tagged by `@type` (serde's internally tagged representation). A
    deserializer that reads `@type` first (TDLib always writes it first) and
    skips serde's `Content` buffering may replace it, but only after it beats the
    plain representation in a benchmark on recorded updates (M2, when update
    throughput is measured).
- `crates/td-api` runs the generator in `build.rs`. Its only hand-written code is
  the `Function` trait and the `int64` serde adapters. `bytes` stays a base64
  `String`; callers decode it where they need the bytes.

### Linking

- `scripts/build-tdlib.sh` builds the shared `tdjson` library from the pinned
  commit (from a `git archive` copy, so the source checkout is never written to)
  into `$RGRAM_TDLIB_DIR` (default `~/.cache/rgram/tdlib-<short commit>`).
  macOS uses `@rpath` install names; Windows uses vcpkg for OpenSSL, zlib and gperf.
- `crates/td-sys` holds the four `extern "C"` declarations (`td_create_client_id`,
  `td_send`, `td_receive`, `td_execute`) and a safe wrapper: receive/execute copy
  the returned C string immediately (TDLib frees it on the next call on the same
  thread), send takes `&CStr`, and `td_receive` is reachable only through a
  `Receiver` token that exists at most once per process and is `!Sync`. All
  `unsafe` in the workspace lives in this one module, each block with a
  `// SAFETY:` comment. The workspace lints deny `unsafe_code`; `td-sys` alone
  allows it at its crate root. Revisited in M5: `media/src/hw.rs` is the one
  other place, an `#![expect(unsafe_code)]` module for FFmpeg's hardware
  device API (`av_hwdevice_ctx_create`, `av_hwframe_transfer_data`), which
  ffmpeg-next does not wrap; same `// SAFETY:` rule.
- `td-sys/build.rs` links `tdjson` from `$RGRAM_TDLIB_DIR` (or the script's
  default), checks that the install was built from `tdlib/COMMIT`, and copies the
  runtime library into `OUT_DIR`, which Cargo puts on the dynamic library path
  for `cargo run` and `cargo test`. Running the binary from `target/` directly
  needs the library on the loader path until packaging (M8) adds an rpath.
- Dynamic linking for now. The binary-size budget excludes TDLib (24 MB on macOS
  arm64). Packaging (M8) bundles `libtdjson` next to the executable and must
  either link OpenSSL statically or bundle it: today's macOS build links
  Homebrew's `libssl.3.dylib` dynamically.

### Runtime (`crates/tg`)

- One TDLib client per account. `td_receive` returns the objects of every client
  in the process and must not be called concurrently, so one `Runtime` per
  process owns the receive thread, which calls `td_receive(1.0)` in a loop, and
  creates the `Client`s.
- Requests carry a numeric `@extra` from the runtime's counter. TDLib appends
  `"@extra"` and `"@client_id"` as the last fields of every object it returns
  (`ClientJson.cpp`, `from_response`), so the receive thread routes each object by
  scanning its tail, without a full parse:
  - with `@extra`: the raw JSON goes to the waiting request's oneshot channel; the
    caller deserializes it into `F::Response` on its own task;
  - without: it is deserialized into `td_api::Update` on the receive thread (which
    preserves TDLib's ordering) and sent to that client's update channel.
- Public interface, and nothing else: `Client::request(&F) -> impl Future<Output =
  Result<F::Response, TdError>>`, which sends at call time so requests reach TDLib
  in call order, and the client's `Updates` stream. Responses report outcomes
  only; state changes arrive as updates. `TdError` (thiserror) distinguishes a
  TDLib `error` object (`code`, `message`), a closed client or runtime, a request
  that cannot be encoded, and a response that failed to decode. Decode errors
  carry only the position and kind, because `serde_json`'s messages quote the
  offending value. Unknown or undecodable updates are logged by type name only and
  dropped; nothing panics.
- Shutdown: after `close`, the update stream ends with
  `updateAuthorizationState(authorizationStateClosed)`, waiting requests of that
  client fail with `Closed`, and new ones fail immediately. Dropping the
  `Runtime` stops the receive thread within one receive timeout and fails
  everything still waiting. On macOS gpui quits through `exit`, so the
  `Runtime` is never dropped; quitting therefore closes the clients and then
  calls `Stopper::stop`, which joins the receive thread. A thread left inside
  `td_receive` while `exit` destroys TDLib's statics crashes with signal 11.

### TDLib configuration

- `setTdlibParameters`: `use_message_database`, `use_file_database`,
  `use_chat_info_database` and `use_secret_chats` are `true`, so history and files
  live on disk inside TDLib, not in rgram's memory. Database and files go to the
  platform data directory per account. `api_id`/`api_hash` come from the
  environment or user config (ADR 0004).
- Memory: `closeChat` when a chat leaves the screen; the
  `message_unload_delay` option lets TDLib unload messages of closed chats;
  `use_storage_optimizer` is on, and `optimizeStorage` runs on a timer with the
  limits from the storage settings (M6).
- Logging: `setLogVerbosityLevel(1)` through `td_execute` before the first client
  exists, and verbosity above 2 is never enabled by default, because higher levels
  log message content. TDLib's log moves from stderr to a file in the data
  directory once rgram has one (M1).

## Consequences

- We own a code generator (a few hundred lines) and must regenerate when the pin
  moves. Moving the pin is a single commit: `tdlib/COMMIT`, the schema copy, and
  the fixes the compiler then demands.
- No dependency on `tdlib-rs`'s runtime; `tgt` remains a reference for build
  tricks only.
- CI must build TDLib on three OSes (30-60 minutes cold); the install directory is
  cached by the hash of `tdlib/COMMIT` and the build script.
- Routing by the JSON tail depends on TDLib's output layout. `tg`'s integration
  test runs requests and updates through real TDLib, unit tests pin the layout
  and its edge cases, and the router falls back to a full parse of the envelope
  (with a warning) if the tail does not match.
