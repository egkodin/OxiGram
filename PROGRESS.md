# Progress

## M0: Foundation (done locally; CI not run yet)

### Done

- tdesktop submodules fetched (`lib_ui`, `lib_base`, `lib_rpl`, `lib_lottie`,
  `codegen`) with `git submodule update --init --depth 1`.
- License: GPL-3.0-or-later ([ADR 0001](docs/adr/0001-license.md), `LICENSE`,
  `NOTICE.md`). API terms mapped to requirements
  ([ADR 0004](docs/adr/0004-telegram-api-terms.md)).
- GUI: gpui pinned to Zed `876c1b0`, chosen by the owner after the spike
  ([ADR 0002](docs/adr/0002-gui.md)). The first gpui@git measurement had no text
  (missing `font-kit`); all three candidates were re-measured and the ADR holds
  the corrected numbers. Spike code deleted.
- TDLib: [ADR 0003](docs/adr/0003-tdlib-integration.md). `scripts/build-tdlib.sh`
  builds `libtdjson` 1.8.69 (`ea3f69ce1`) into `~/.cache/rgram`;
  `scripts/check-td-schema.sh` checks the vendored `td_api.tl` against it.
- Workspace: edition 2024, Rust 1.99.1, workspace lints, release profile
  (`lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip`), `Cargo.lock`.
  Crates:
  - `td-sys`: FFI to `tdjson`, a single-receiver token, runtime library copied
    next to the build so `cargo run` and `cargo test` find it.
  - `td-api-gen` and `td-api`: types, enums and functions generated from the
    full `td_api.tl` with serde (int64 as strings).
  - `tg`: receive thread, request/response routing by `@extra`, update stream
    per client, close and shutdown semantics, content-free decode errors.
  - `style` and `tools/style-gen`: the palette exported from `colors.palette`
    (586 colors), with a snapshot test.
  - `app` (`rgram`): credentials from `RGRAM_API_ID`/`RGRAM_API_HASH` with a
    clear error in the window, `tracing` logging, the empty main window, TDLib
    version check at startup.
- CI workflow: fmt, schema check, `cargo deny`, `cargo audit`,
  `cargo tree --duplicates`; clippy, test, build and rustdoc on macOS 15,
  Ubuntu 24.04 and Windows 2025, building TDLib with a cache.
- `docs/perf.md`: budgets and the M0 empty-window numbers.

### Verified (macOS arm64, this machine)

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
  --all-features -- -D warnings`, `cargo test --workspace --all-features`
  (32 tests, including a lifecycle test against the real `libtdjson`), and
  `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps`.
- `cargo deny check`: advisories, bans, licenses and sources ok.
  `cargo audit`: no vulnerabilities; 6 unmaintained crates, all under gpui.
- `cargo run -p rgram`: one 800×600 window opens; the log shows
  `TDLib is ready version=1.8.69` and, without credentials, the credentials
  error. Release build: 9.7 MB binary, 1.2 % idle CPU, 46.6 MB idle footprint.

### Not verified

- CI on all three OSes, and with it TDLib and gpui builds on Linux and Windows:
  there is no GitHub remote yet (needs the owner's permission).
- What the window shows on screen: `screencapture` needs a screen-recording
  permission this shell does not have. Text rendering is inferred from the
  enabled `font-kit` feature and the missing no-text warning.
- That `close` reaches TDLib before the process exits: gpui gives quit hooks
  200 ms. M1 is to wait for `authorizationStateClosed`.
- Credentials from a config file and TDLib's log file: moved to M1 with sign-in.

### Risks

- Idle CPU is 1.2 % against a ~0 % budget; gpui's display link keeps running
  (`docs/perf.md`). To fix or justify in M1.
- `block 0.1.6` (through gpui) has a future-incompatibility warning, and the
  tree has 24 duplicated crates, mostly from gpui.

### Environment changes made on this machine

- Homebrew: installed `cmake`, `ninja`, `cargo-deny` and `cargo-audit`; upgraded
  `rust` 1.97.1 to 1.99.1 (current gpui needs 1.98.1+).
- Xcode: downloaded the Metal Toolchain component (gpui precompiles shaders).
- Started Docker Desktop for a Linux build check; it stopped at its own
  admin-privileges prompt, was not used, and has been quit again.
