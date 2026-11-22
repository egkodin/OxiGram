# OxiGram

OxiGram is an unofficial Telegram desktop client built in Rust with GPUI and
TDLib. It follows Telegram Desktop's interaction patterns and aims to use
less memory and start faster.

**In development.** The latest verified build is for Apple silicon macOS.
The CI matrix includes Linux and Windows. Build and runtime verification on
those platforms remains open. [Verification notes](PROGRESS.md)

## What you can try

- Browse chats, switch accounts, organize chats with folders and the Archive,
  and search conversations.
- Send and reply to messages, open sender profiles and forwarded sources, and
  use reactions, installed sticker packs, custom emoji and saved GIFs.
- View photos and videos with scrollable captions, play voice messages, and
  swipe to reply or go back.
- Open recent Calls from the main menu. Signed-in call-history pagination
  remains unverified.

## Build and run on macOS

Install Rust with [rustup](https://rustup.rs/) and install the native build
dependencies with [Homebrew](https://brew.sh/):

```bash
xcode-select --install
brew install cmake ninja gperf openssl@3 ffmpeg pkgconf
bash scripts/build-tdlib.sh
```

Get your own API ID and hash from [Telegram's developer portal](https://my.telegram.org/).
Put the two assignments below in a local `.env` at the repository root and
replace the placeholders. Git ignores this file.

```dotenv
RGRAM_API_ID=your_numeric_api_id
RGRAM_API_HASH=your_api_hash
```

```bash
chmod 600 .env
(
  set -a
  . ./.env
  set +a
  cargo run --locked -p oxigram --features runtime-shaders
)
```

The runtime shader feature lets source builds compile Metal shaders when they
start. To package a local `.app`, run `brew install meson`,
`bash scripts/build-ffmpeg.sh` and `xcodebuild -downloadComponent MetalToolchain`.
Then run `bash scripts/build-macos-app.sh` and open `target/OxiGram.app`. Keep any existing
bundle by moving it before packaging. The bundle uses an ad-hoc signature. The launcher can also read credentials
from the macOS Keychain. [Launcher and credential setup](scripts/run-live.sh)

The [CI workflow](.github/workflows/ci.yml) lists native dependencies and build
commands for all three targets. Windows uses MSVC and vcpkg for TDLib. Linux and
Windows installers are planned.

## To-do

The [full checklist](docs/roadmap.md) separates completed, in-progress and
planned work by feature area. A completed item has been exercised in the app
on a live account. Partial implementations and features still awaiting live
verification are in progress. [Evidence and remaining gaps](docs/parity.md)

Work still open includes verifying call-history pagination, extending sticker
and GIF search, validating Linux and Windows builds, and measuring signed-in
startup and idle memory. [Performance results and methods](docs/perf.md)

## Development

- `crates/app`, `crates/features`, `crates/ui`, `crates/style` and
  `crates/i18n` cover application startup, UI and localization.
- `crates/model`, `crates/tg`, `crates/td-api` and `crates/td-sys` handle
  account state and TDLib. `crates/media` handles decoding and playback.
- `scripts/` builds native dependencies and packages macOS. `tools/`
  contains style, language and code generation tools.

```bash
cargo fmt --all -- --check
cargo test --locked --workspace --features runtime-shaders
cargo clippy --locked --workspace --all-targets --features runtime-shaders -- -D warnings
```

Update `docs/parity.md` when a feature's status changes, then run
`scripts/update-roadmap.sh` to regenerate the checklist.

## License

GPL-3.0-or-later. OxiGram is an independent project. See [LICENSE](LICENSE) and
[third-party notices](NOTICE.md).
