# OxiGram

OxiGram is an unofficial Telegram desktop client for **Linux, Windows and
macOS**, built in Rust with GPUI and TDLib. It follows Telegram Desktop's
interaction patterns and aims to use less memory and start faster.

| Platform | Development target | Verification |
| --- | --- | --- |
| Linux | Ubuntu 24.04 | CI target configured. Build and runtime verification pending. |
| Windows | Windows x64 with MSVC | CI target configured. Build and runtime verification pending. |
| macOS | Apple silicon | Build and offline tests verified. Local app packaging available. |

[CI matrix](.github/workflows/ci.yml) · [Verification notes](PROGRESS.md)

## What you can try

- Browse chats, switch accounts, organize chats with folders and the Archive,
  and search conversations.
- Send and reply to messages, open sender profiles and forwarded sources, and
  use reactions, installed sticker packs, custom emoji and saved GIFs.
- View photos and videos with scrollable captions, play voice messages, and
  swipe to reply or go back.
- Open recent Calls from the main menu. Signed-in call-history pagination
  remains unverified.

## Build from source

Install Rust with [rustup](https://rustup.rs/) and run the commands below from
the repository root. The TDLib and FFmpeg scripts build the pinned dependencies
for your platform.

### Linux

Ubuntu 24.04 example:

```bash
sudo apt update
sudo apt install build-essential clang libclang-dev cmake ninja-build gperf \
  zlib1g-dev libssl-dev libasound2-dev libfontconfig-dev libglib2.0-dev \
  libva-dev libvulkan1 libwayland-dev libx11-xcb-dev libxkbcommon-x11-dev \
  libzstd-dev meson nasm pkg-config curl
bash scripts/build-tdlib.sh
ffmpeg_dir="$HOME/.cache/rgram-ffmpeg"
bash scripts/build-ffmpeg.sh "$ffmpeg_dir"
export PKG_CONFIG_PATH="$ffmpeg_dir/lib/pkgconfig"
```

### Windows

Use the x64 MSVC Rust toolchain and Visual Studio Build Tools with the C++
workload and Windows SDK. Install CMake, LLVM, vcpkg, and MSYS2. Run MSYS2 Bash
from the x64 Visual Studio developer environment with the MSVC tools on
`PATH`. The FFmpeg build also needs Meson, Ninja, NASM, Make, and pkg-config.

```bash
export VCPKG_ROOT="<path to vcpkg>"
"$VCPKG_ROOT/vcpkg.exe" install gperf:x64-windows openssl:x64-windows zlib:x64-windows
export RGRAM_TDLIB_DIR="$(cygpath -m "$HOME/.cache/oxigram-tdlib")"
bash scripts/build-tdlib.sh "$RGRAM_TDLIB_DIR"
ffmpeg_dir="$(cygpath -m "$HOME/.cache/rgram-ffmpeg")"
bash scripts/build-ffmpeg.sh "$ffmpeg_dir"
export PKG_CONFIG_PATH="$ffmpeg_dir/lib/pkgconfig"
```

### macOS

Install the Xcode command-line tools and use [Homebrew](https://brew.sh/) for
the native dependencies:

```bash
xcode-select --install
brew install cmake ninja gperf openssl@3 meson pkgconf
bash scripts/build-tdlib.sh
ffmpeg_dir="$HOME/.cache/rgram-ffmpeg"
bash scripts/build-ffmpeg.sh "$ffmpeg_dir"
export PKG_CONFIG_PATH="$ffmpeg_dir/lib/pkgconfig"
```

For a local `.app`, install the Metal Toolchain with
`xcodebuild -downloadComponent MetalToolchain` and run
`RGRAM_FFMPEG_DIR="$ffmpeg_dir" bash scripts/build-macos-app.sh`. Move any
existing `target/OxiGram.app` before packaging. Linux and Windows installers
are planned.

## Run OxiGram

Obtain an API ID and hash from [Telegram's developer portal](https://my.telegram.org/).
Put these assignments in a local `.env` at the repository root and replace
the placeholders. Git ignores this file.

```dotenv
RGRAM_API_ID=your_numeric_api_id
RGRAM_API_HASH=your_api_hash
```

In the same shell you used to set up the native dependencies, load the
credentials and start the client:

```bash
(
  set -a
  . ./.env
  set +a
  cargo run --locked -p oxigram --features runtime-shaders,media/static-ffmpeg
)
```

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
cargo test --locked --workspace --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
```

Update `docs/parity.md` when a feature's status changes, then run
`scripts/update-roadmap.sh` to regenerate the checklist.

## License

GPL-3.0-or-later. OxiGram is an independent project. See [LICENSE](LICENSE) and
[third-party notices](NOTICE.md).
