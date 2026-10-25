# rgram

rgram is an unofficial desktop client for Telegram, written in Rust. It uses the
Telegram API through [TDLib](https://github.com/tdlib/td) and is not affiliated
with Telegram. It aims to look and behave like Telegram Desktop while starting
faster and using less memory.

Status: early development (milestone M0). See [PROGRESS.md](PROGRESS.md).

## Telegram API credentials

Every build needs its own `api_id` and `api_hash` from
<https://my.telegram.org>. They are never stored in this repository. Set them in
the environment before starting rgram:

```bash
export RGRAM_API_ID=123456
export RGRAM_API_HASH=0123456789abcdef0123456789abcdef
```

## Building TDLib

rgram links the shared `tdjson` library built from the TDLib commit pinned in
`tdlib/COMMIT`:

```bash
scripts/build-tdlib.sh
```

The script installs into `~/.cache/rgram/tdlib-<commit>` (override with
`RGRAM_TDLIB_DIR`). Set `TD_SOURCE` to an existing `tdlib/td` checkout to avoid a
clone; the checkout is only read. macOS needs `cmake`, `ninja`, `gperf` and
`openssl@3` from Homebrew.

## Building and running

With TDLib built:

```bash
cargo run -p rgram
cargo test --workspace
```

`cargo run` and `cargo test` find `libtdjson` on their own. `RGRAM_LOG` sets the
log filter (default `info`), for example `RGRAM_LOG=tg=debug`.

`tools/style-gen` regenerates `crates/style` from the Telegram Desktop checkout
next to this repository (or the path given as its argument):

```bash
cargo run -p style-gen
```

## License

GPL-3.0-or-later. See [LICENSE](LICENSE) and [NOTICE.md](NOTICE.md) for the
material derived from Telegram Desktop and other projects.
