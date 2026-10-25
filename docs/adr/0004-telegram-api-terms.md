# ADR 0004: Telegram API terms of service

- Status: accepted
- Date: 2026-10-25
- Source: <https://core.telegram.org/api/terms>, read on 2026-10-25

## Context

Third-party clients must follow the Telegram API Terms of Service. Telegram can
revoke API access for a breach that is not fixed within 10 days (§4.2), so the
terms are hard requirements, not guidelines.

## Requirements and how rgram meets them

| Term | Requirement | rgram |
|---|---|---|
| §1.1 | Guard user privacy; follow the Security Guidelines | Secrets and message content never reach logs; TDLib owns crypto and storage (ADR 0003) |
| §1.3 | Basic features work as expected with users of other clients | Feature parity is measured against tdesktop |
| §1.4 | Never act for the user without consent; never keep self-destructing content; never break online status, read receipts or typing indicators ("ghost mode" is forbidden) | No such options exist. Read receipts are sent when messages are actually viewed (`viewMessages`), typing is sent while typing (`sendChatAction`), self-destructing media is never cached outside TDLib |
| §1.5 | Telegram data must not be used to train or run AI/ML models | No such feature; no telemetry |
| §2.1 | Each app has its own `api_id` | `api_id`/`api_hash` come from `RGRAM_API_ID`/`RGRAM_API_HASH` or the user's config; never committed; startup fails with a clear error when missing |
| §2.2 | Users are told the app uses the Telegram API, prominently, in store listings and the in-app intro | The intro screen states "rgram is an unofficial client that uses the Telegram API"; the same line opens the README and every package description |
| §2.3 | "Telegram" may appear in the title only as "Unofficial …" | The product name is **rgram** |
| §2.4 | The official Telegram logo must not be used | rgram ships its own icon; tdesktop's `Resources/art/` and app icons are excluded (ADR 0001) |
| §3.2 | Monetization is disclosed in store descriptions | rgram has no monetization |
| §3.3 | Clients that show channel content must support sponsored messages and not interfere with them | Channel histories request and render sponsored messages (`getChatSponsoredMessages`; views reported through `viewMessages` with the sponsored message id; `clickChatSponsoredMessage`; `reportChatSponsoredMessage`); tracked as an M3 acceptance item |
| closing | Terms may change; developers are notified in-app on the connected account | Re-read the terms before each release; the release checklist links this ADR |

## Consequences

- Sponsored messages are part of M3, not optional polish.
- Any feature that changes what other users see about the user (online, read,
  typing) needs a check against §1.4 before it lands.
