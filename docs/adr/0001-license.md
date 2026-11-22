# ADR 0001: License GPL-3.0-or-later

- Status: accepted
- Date: 2026-10-25

## Context

oxigram copies Telegram Desktop's look as exactly as possible. The cheapest way to
get the numbers right is to reuse tdesktop's style constants, palette, themes and
icons. Telegram Desktop is GPLv3 (with an OpenSSL linking exception), so any work
derived from those files must be GPL-compatible.

Other inputs:

| Input | License | Compatible with GPLv3 |
|---|---|---|
| tdesktop sources, styles, palette, icons, themes | GPLv3 + OpenSSL exception | yes |
| TDLib (`td/`), including `td_api.tl` | Boost 1.0 | yes |
| OpenSSL 3.x (linked by TDLib) | Apache 2.0 | yes (GPLv3 only, not GPLv2) |
| Open Sans 1.10 fonts (`lib_ui/fonts`) | Apache 2.0 | yes (GPLv3 only) |
| `tgt` (technique reference only, no code copied) | MIT / Apache 2.0 | yes |
| `tdlib-rs-parser` (TL parser, build-time) | MIT / Apache 2.0 | yes |

## Decision

- oxigram is licensed **GPL-3.0-or-later**. `LICENSE` holds the canonical GPLv3 text
  from gnu.org. Every crate sets `license = "GPL-3.0-or-later"`.
- Derived material keeps attribution: `NOTICE.md` lists what comes from where, and
  every generated or copied file names its source path in a header comment.
- We do not carry tdesktop's OpenSSL exception. OpenSSL 3 is Apache 2.0, which
  GPLv3 already permits.
- Not reused, despite being in the tdesktop tree:
  - `Telegram/Resources/emoji/*.webp`: the provenance of the emoji artwork is not
    documented in the repository. oxigram uses Noto Color Emoji (OFL) or the
    platform emoji font until the provenance is confirmed.
  - `Telegram/Resources/art/` and any other Telegram logo or app icon: forbidden by
    the API terms (see [ADR 0004](0004-telegram-api-terms.md)).
- `cargo deny check licenses` runs in CI with an allow-list of GPL-compatible
  licenses.

## Consequences

- Anyone distributing oxigram binaries must offer the corresponding source.
- A future change of license needs every contributor's agreement and must drop
  all tdesktop-derived files first.
- Dependencies under GPL-incompatible licenses (for example, some proprietary or
  SSPL code) cannot be added.
