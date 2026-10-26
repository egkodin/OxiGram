# Progress

## M2: Chat list (in progress; checkpoint committed)

### Done (code, checks green)

- `model`: chats, users, messages, files and folders folded from TDLib
  updates (`chats`, `chat`, `user`, `message`, `file`); search
  (`search`: local, server and public chats, messages across chats with
  paging); chat actions (`chat_actions`: pin, archive, mute, mark read and
  unread, clear history, delete or leave, block).
- `features::dialogs`: the virtualized list with tdesktop's row layout (badges,
  counters, read checks, drafts, typing, online dots, thumbnails), the archive
  row and archive list with a back bar, search (chats, global results, message
  results with paging, stale results dropped), and the chat context menu with
  attention confirmations for destructive items.
- `features`: main menu panel (cover, items, night mode toggle, version),
  night palette generated from tdesktop's `night.tdesktop-theme` and saved in
  `settings.json`, Escape clears search or leaves the archive.
- `ui`: PopupMenu, SettingsButton, CrossButton, SVG icon masks.
- Folder tabs with unread counters, contacts box from the main menu
  (online-first sort, name search, last-seen texts in `features::status`),
  loading / no chats / empty folder texts, drag to reorder pinned chats,
  archive collapse, move to main menu and back, mark archive read, column
  resize with the narrow (userpics only) list and the one-column layout.
- Message search filters: "From archive" and chat type (All, Private,
  Groups, Channels) from the found-messages bar, as tdesktop's
  `ChatTypeFilter` menu; switching drops stale pages by query and filter.
- Search suggestions while the search field is focused and empty: the
  frequent contacts strip and recent searches with their menus and the
  Clear confirmation (`dialogs::suggestions`).
- Suggestion tabs (Chats, Channels, Apps) on the shared tab strip; group
  member and channel subscriber counts folded from `updateBasicGroup` /
  `updateSupergroup` (`ChatKind` now carries the group ids).
- Folder tab context menu (`dialogs::folder_menu`): Mark as read and Remove
  with tdesktop's confirmations; All chats offers Mark all chats as read.
- Fixed two click-through bugs found in snapshots: a popup menu item click
  also clicked the row under it, and a right click on a layer (main menu,
  boxes) reached the list behind. `PopupMenu` and `Layers` now occlude.
- Fixed an M1 bug: the log-out box had no cancel button, so Escape or a click
  outside logged out. Destructive boxes now go through `confirm_attention`
  (regression test `destructive_boxes_cancel_on_escape`).

### Verified (macOS arm64, this machine)

- `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`,
  `cargo test` green.
- Snapshots 11-36 (`cargo run -p rgram --example snapshots`): chat list with
  folder tabs, main menu, night mode, contacts box, folder switch, archive,
  chat and archive context menus, collapsed archive, archive in the main
  menu, pinned drag, folder tab menus; compared by eye with tdesktop's style and layout code.

### Not verified

- Snapshot windows never become active, so gpui sends no focus events there:
  the search suggestions and focused-field looks have no snapshot; the
  suggestions were checked by eye once with seeded data.

- Everything live: `RGRAM_API_ID` / `RGRAM_API_HASH` are still not set.
  Context-menu actions modify the account; they will only run in Saved
  Messages or on the test DC, with the owner's say-so.
- Perf numbers for a real list (needs an account).

### Next steps

1. Tab reordering, chat preview (peek); search
   in one chat, by sender and hashtags wait for the chat view (M3) as their
   entry point.
2. The narrow list's folder sidebar and search button.

## M1: Sign-in (in progress; blocked on API credentials for the live run)

### Done (code, checks green)

- `model`: authorization states and actions (`auth`), connection state,
  terms updates, suggested language, `Session` requests; `Secret` keeps phone
  numbers, codes and passwords out of `Debug` and logs.
- `i18n`: English strings generated from tdesktop's `lang.strings` by
  `tools/lang-gen`, plurals, rgram's own strings; `features::lang` loads and
  switches TDLib language packs.
- `style`: tdesktop `.style` files exported by `tools/style-gen` (basic,
  widgets, intro, window, layers, boxes and more) and their icons.
- `ui`: FlatLabel, InputField with correctors, RoundButton, LinkButton,
  IconButton, Checkbox, QrCode, InfiniteRadial, BoxFrame, ripples, animations.
- `features`: the intro (start, QR, phone with country box, code, password
  with recovery and account reset, sign-up with photo, login email, terms box),
  error boxes (banned number, Premium required, flood), the box layer, the
  connecting pill, a signed-in placeholder with log out, terms updates after
  sign-in, and `Root` choosing the screen by authorization state.
- `app`: per-OS data directory (`RGRAM_DATA_DIR` overrides it, test DC gets
  its own), `RGRAM_TEST_DC=1`, TDLib parameters on request, language pack on
  start, a new client after `authorizationStateClosed` (log out, QR to phone),
  close TDLib before quitting, localized Quit menu.
- `cargo run -p rgram --example snapshots`: renders each sign-in screen with a
  preset state to `target/snapshots/*.png` through gpui's `render_to_image`,
  offline (TDLib never gets parameters).

### Verified (macOS arm64, this machine)

- `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, `cargo deny check`, `cargo audit` (the same six
  allowed unmaintained warnings as M0).
- The debug binary starts without credentials, shows the notice, idles at
  0.0 % CPU; the `secondary-*` key bindings crashed gpui at startup and were
  fixed.
- Snapshots of start, QR, phone, code, password, sign-up and email screens,
  the Premium-required box, the signed-in placeholder and the terms-update box
  with its age check,
  compared by eye with tdesktop's layout code (no tdesktop screenshots: running
  Telegram Desktop needs a download the owner has not approved). Fixed from
  them: labels ignored their font size and line height (all text was 16 px),
  the phone placeholder drew under the number pattern, the email field had
  the wrong style and overlapped its description, and a sign-in state that
  arrived before the intro existed was never handled.

### Not verified

- Every live sign-in flow, staying signed in, log out and terms updates:
  `RGRAM_API_ID` / `RGRAM_API_HASH` are not set. Rows in `docs/parity.md` say
  which ones only have snapshots.
- The country box, the sign-up terms box, the banned-number and reset boxes,
  and animations: the snapshot example does not open them yet.
- Quitting through Cmd-Q with TDLib closing first: this shell cannot send key
  presses to the app (no accessibility permission).
- Linux and Windows runs of the new code (CI not run for it).

### Next steps

1. With credentials: sign in on the production DC (the owner types the code
   and 2FA password), restart to check the session survives, log out; record
   cold start and idle footprint in `docs/perf.md`; mark the parity rows.
2. Add the remaining box scenarios to the snapshot example (country box,
   banned-number box).
3. Start M2 (chat list) on the same pattern: model events, store state, and
   snapshot scenarios with preset chats.

### Open questions for the owner

- API credentials (asked once; still missing).
- Permission to download Telegram Desktop for screenshot comparison.

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
