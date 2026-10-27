# Parity registry

One row per user-visible feature of Telegram Desktop, built from the areas in
`tdesktop/Telegram/SourceFiles/AGENTS.md` and the files in each area's
directories (tdesktop at the commit in `tdesktop/`, TDLib 1.8.69). This registry
is the definition of done: rgram is finished when every row is `done`, or
`not applicable` with a reason.

- Milestones: M1 sign-in, M2 chat list, M3 history, M4 sending, M5 media,
  M6 profile and settings, M7 OS integration, M8 polish and delivery, M9 secret
  chats and topics, M10 bots, M11 stories, M12 calls, M13 payments, Premium and
  the rest, M14 parity sweep.
- Status: `todo`, `in progress`, `done`, `not applicable`.
- Verified: how the row was checked on a live account (or why it could not be).
  A row is `done` only after it ran in the real app.
- Every row was checked against `td_api.tl`: unless a row says otherwise, TDLib
  exposes what it needs.

## Sign-in (`intro/`)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| intro | Welcome screen with "Start Messaging" | M1 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); not verified: no API credentials yet, so no live sign-in |
| intro | Offer to continue in the system language on the welcome screen | M1 | in progress | not verified: no API credentials yet, so no live sign-in |
| intro | Phone number entry with country picker, search and per-country format mask | M1 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); not verified: no API credentials yet, so no live sign-in; country list needs TDLib online |
| intro | Country preselected from the network (`getCountryCode`) | M1 | in progress | not verified: no API credentials yet, so no live sign-in |
| intro | QR code login: show code, refresh on expiry, finish when accepted elsewhere | M1 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); not verified: no API credentials yet, so no live sign-in |
| intro | Code sent to the Telegram app, with the "code sent to your app" hint | M1 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); not verified: no API credentials yet, so no live sign-in |
| intro | Code by SMS, by phone call, flash call and missed call | M1 | in progress | not verified: no API credentials yet, so no live sign-in |
| intro | Code from Fragment, with the link to fragment.com | M1 | in progress | not verified: no API credentials yet, so no live sign-in |
| intro | Resend code with countdown and next delivery method | M1 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); not verified: no API credentials yet, so no live sign-in |
| intro | Auto-submit when the code reaches its length; "wrong code" error | M1 | in progress | not verified: no API credentials yet, so no live sign-in |
| intro | Login email setup when the server requires one, and email code entry | M1 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); not verified: no API credentials yet, so no live sign-in |
| intro | Reset the login email when the code does not arrive | M1 | in progress | not verified: no API credentials yet, so no live sign-in |
| intro | Cloud password (2FA) check with hint | M1 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); not verified: no API credentials yet, so no live sign-in |
| intro | Password recovery through the recovery email code | M1 | in progress | not verified: no API credentials yet, so no live sign-in |
| intro | Account reset when the password is forgotten (with waiting period) | M1 | in progress | not verified: no API credentials yet, so no live sign-in |
| intro | Sign-up: first name, last name, photo | M1 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); not verified: no API credentials yet, so no live sign-in |
| intro | Terms of service on sign-up and later updates of the terms | M1 | in progress | not verified: no API credentials yet, so no live sign-in |
| intro | FLOOD_WAIT error (tdesktop shows the static `lng_flood_error`, no countdown) | M1 | in progress | not verified: no API credentials yet, so no live sign-in |
| intro | Clear errors: invalid phone, banned number box, too many attempts, network | M1 | in progress | not verified: no API credentials yet, so no live sign-in |
| intro | Stay signed in across restarts | M1 | in progress | not verified: no API credentials yet, so no live sign-in |
| intro | Log out with confirmation | M1 | in progress | not verified: no API credentials yet, so no live sign-in |
| intro | The "alternatives to logging out" box (settings, passcode, add account) | M6 | todo | needs the settings sections it links to |
| intro | Sign in with a passkey | M7 | todo | |
| intro | Firebase code delivery | — | not applicable | Android/iOS-only delivery (`authenticationCodeTypeFirebaseAndroid/Ios`) |

## Accounts (`main/`, tray accounts menu)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| accounts | Add another account (up to 3, 6 with Premium) | M8 | todo | |
| accounts | Switch accounts from the main menu, with unread badges per account | M8 | todo | |
| accounts | Accounts in the tray menu | M8 | todo | |
| accounts | Per-account notifications from inactive accounts | M8 | todo | |
| accounts | Reorder accounts | M8 | todo | |

## Chat list (`dialogs/`)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| dialogs | Virtualized chat list in the server's order | M2 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); not verified: no API credentials yet, so no live account |
| dialogs | Row: userpic, name, time, last message with sender and entities | M2 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); not verified: no API credentials yet, so no live account |
| dialogs | Row: media thumbnails and emoji in the last-message preview | M2 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); not verified: no API credentials yet, so no live account; custom emoji draw as their fallback emoji until M5 |
| dialogs | Row: verified, Premium, emoji status, scam, fake and bot badges | M2 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); not verified: no API credentials yet, so no live account; emoji status shows the Premium star until custom emoji land in M5 |
| dialogs | Row: unread counter (muted in grey), mention and reaction badges | M2 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); not verified: no API credentials yet, so no live account |
| dialogs | Row: outgoing message read checks | M2 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); not verified: no API credentials yet, so no live account |
| dialogs | Row: draft preview ("Draft:") | M2 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); not verified: no API credentials yet, so no live account |
| dialogs | Row: typing, uploading and other chat actions | M2 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); not verified: no API credentials yet, so no live account; one sender, no animated dots yet |
| dialogs | Row: online indicator on user userpics | M2 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); not verified: no API credentials yet, so no live account |
| dialogs | Row: animated (video) userpics | M5 | todo | |
| dialogs | Row: mute icon, pinned icon, unread mark | M2 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); not verified: no API credentials yet, so no live account |
| dialogs | Pinned chats: pin, unpin, drag to reorder, pinned limit box | M2 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); pin and unpin from the context menu, drag onto another pinned row (target lights up, rows do not slide); limit box not built; not verified: no API credentials yet, so no live account |
| dialogs | Archive: collapsed row, open, archive and unarchive chats | M2 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); not verified: no API credentials yet, so no live account |
| dialogs | Archive row collapse/expand and hide into main menu | M2 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); collapse, expand, move to main menu and back, mark archive read; the collapsed bar does not scroll with the list; the "moved to the main menu" toast (snapshot 26); not verified: no API credentials yet, so no live account |
| dialogs | Live updates: new messages, edits, deletes, read states reorder the list | M2 | in progress | unit tests on the update fold; not verified: no API credentials yet, so no live account |
| dialogs | Connecting / Updating / Waiting for network status with proxy state | M2 | in progress | status pill from M1; proxy state comes with M6; not verified: no API credentials yet, so no live account |
| dialogs | Adaptive one, two and three column layout with column resize | M2 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); two columns with the edge drag (ratio saved in settings.json) and one column under 640 px (unit test, no snapshot); the third column comes with the info panel; not verified: no API credentials yet, so no live account |
| dialogs | Narrow (userpics only) chat list mode | M2 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); drag the edge under 130 px; no search button and no folder sidebar in narrow mode yet; not verified: no API credentials yet, so no live account |
| dialogs | Chat context menu: pin, mute, mark read/unread, archive, add to folder | M2 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); not verified: no API credentials yet, so no live account; mute has no duration submenu; add to folder not built |
| dialogs | Chat context menu: clear history, delete and leave, block, report | M2 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); not verified: no API credentials yet, so no live account; report not built; destructive items confirm first |
| dialogs | Open chat in a separate window | M8 | todo | |
| dialogs | Chat preview without opening (peek) | M2 | todo | |
| dialogs | Touchpad swipe actions on rows (archive, read, pin, mute, delete) | M8 | todo | |
| dialogs | Search: chats, contacts and global results | M2 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); not verified: no API credentials yet, so no live account |
| dialogs | Search: messages across all chats with date and type filters | M2 | in progress | message results with paging; the found-messages bar opens tdesktop's filter menu (From archive, All / Private / Groups / Channels) mapped to `searchMessages`; unit tests only, as the snapshot run has no TDLib replies; tdesktop's global search has no date filter (min/max date 0), the calendar jump comes with search in one chat; not verified: no API credentials yet, so no live account |
| dialogs | Search: in one chat, by sender | M2 | todo | |
| dialogs | Search: hashtags and cashtags, in this chat or everywhere | M2 | in progress | everywhere: `#tag` and `$TAG` queries go through the message search; recently searched hashtags (ranked as `incrementRecentHashtag`, saved in settings.json) show over the results while a hashtag is typed, checked by eye with seeded data (snapshot windows cannot type); picking one completes the query, the cross forgets it (not exercised: needs hover in an active window); in this chat waits for search in one chat (M3); public posts by hashtag are M13; not verified: no API credentials yet, so no live account |
| dialogs | Search: public posts in channels (with Premium limits) | M13 | todo | |
| dialogs | Search: recent searches and top peers strip | M2 | in progress | while the search field is focused: frequent contacts strip (`getTopChats`) with online dots and counters, Recent list (`searchRecentlyFoundChats`) with Clear, Remove from Recent and Clear all; chats picked from search results join Recent; checked by eye with seeded data in a throwaway run, since snapshot windows never become active and gpui sends no focus events there; no "Show more" grid, no Hide frequent contacts; not verified: no API credentials yet, so no live account |
| dialogs | Search: channel and app suggestions in the empty search | M2 | in progress | Chats / Channels / Apps tabs over the suggestions: channels you joined (folds to 5 with Show more), similar channels (`getRecommendedChats`), apps you use (`getTopChats` web-app bots); checked by eye with seeded data in a throwaway run; TDLib 1.8.69 has no popular apps list, no Join button yet, no posts / media / downloads tabs; not verified: no API credentials yet, so no live account |
| dialogs | Search: Saved Messages tags | M3 | todo | |
| dialogs | Top bar suggestions: set a userpic, set a birthday, contacts' birthdays | M6 | todo | |
| dialogs | Top bar suggestions: unreviewed login, Premium offers and grace period | M13 | todo | |
| dialogs | Top bar suggestions: low Stars for subscriptions, gift auctions, custom promo | M13 | todo | |
| dialogs | Proxy sponsor (promoted) chat at the top | M6 | todo | |
| dialogs | Communities in the chat list, with joinable and requested chats | M6 | todo | |
| dialogs | Forum rows with the latest topics | M9 | todo | |
| dialogs | Stories strip above the list and story rings on userpics | M11 | todo | |
| dialogs | Offer to restore separate windows after restart | M8 | todo | |
| dialogs | Contacts list with online status, search and sorting | M2 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); sorted by last online, name search; no alphabet toggle, section headers or Add contact button; not verified: no API credentials yet, so no live account |
| dialogs | Empty-state screens (no chats, empty folder, no results) | M2 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); loading, no chats, empty folder and no search results texts; no Add contact / Edit folder links yet; not verified: no API credentials yet, so no live account |

## Chat folders (`boxes/filters/`, folder tabs)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| folders | Folder tabs on top or in a left sidebar, with unread counts | M2 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); the left sidebar by default when the window is 452 px or wider (snapshots 11, 16-18, 38-40: icons from TDLib's icon names, names up to three lines, badges, the main menu button, the folder menu, drag to reorder), tabs on top otherwise or with the tabs-on-top setting (snapshots 20-37), counters grey when all unread chats are muted; the setting has no switch until the folders settings page (M6); no Edit button under the folders; not verified: no API credentials yet, so no live account |
| folders | "All chats" tab and its position | M2 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); "All" always first; TDLib's main_chat_list_position ignored; not verified: no API credentials yet, so no live account |
| folders | Folder tab context menu: edit, mark as read, add chats, delete | M2 | in progress | snapshots 32-36: Mark as read (asks past 1000 unread), Remove with tdesktop's confirmation (delete wording for folders with invite links), All chats' Mark all chats as read; Edit folder, Edit folders and Add chats wait for the folder editor (M6); the shared-folder leave-chats box is missing; not verified: no API credentials yet, so no live account |
| folders | Reorder folders by dragging tabs | M2 | in progress | snapshot 37 (the dragged tab follows the pointer); dropping on a folder tab sends `reorderChatFolders`; "All chats" stays first (moving it is Premium, M13); the other tabs do not slide aside; not verified: no API credentials yet, so no live account |
| folders | Folder editor: name with emoji, icon, included and excluded chat types and chats | M6 | todo | |
| folders | Folder color and folder tags on chat rows (Premium) | M13 | todo | |
| folders | Recommended folders | M6 | todo | |
| folders | Shareable folder links: create, edit, delete, join from a link | M6 | todo | |
| folders | Add a chat to a folder from the context menu or by drag and drop | M6 | todo | |
| folders | Folder limit box | M6 | todo | |

## Chat screen (`history/`, top bars)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| history | Title bar: name, status (online, last seen, members, subscribers), typing | M3 | in progress | snapshot only (`cargo run -p rgram --example snapshots`, 42-43); not verified: no API credentials yet, so no live history; buttons, one-column back arrow not snapshotted |
| history | Title bar buttons: search, call, video call, info panel, menu | M3 | in progress | snapshots 50/51 and 62-63 (`cargo run -p rgram --example snapshots`); search and the menu toggle; not verified: no API credentials, so no live chat; calls come with M12, the info panel with M6 |
| history | Chat menu: mute, clear history, delete, report, auto-delete, wallpaper | M3 | in progress | snapshot only (`cargo run -p rgram --example snapshots`, 50/51); not verified: no API credentials, so no live actions; mute (for good), clear history, delete or leave with the chat list's confirmations; no report, auto-delete or wallpaper yet |
| history | Bubbles with tails, grouping by sender and time, userpics in groups | M3 | in progress | snapshot only (`cargo run -p rgram --example snapshots`, 42-43); not verified: no API credentials yet, so no live history; no wide-window mode |
| history | Sender names in peer colors, admin and member titles | M3 | in progress | snapshot only (`cargo run -p rgram --example snapshots`, 42-43); not verified: no API credentials yet, so no live history; no admin and member titles |
| history | Date separators, sticky date on scroll | M3 | in progress | snapshots 42-43 and 59 (`cargo run -p rgram --example snapshots`); scrolling up floats the top message's day over the history for 1 s, scrolling down only on entering another day (3 s), 200 ms fades, hidden while the message's own badge is in place, a click opens the calendar; not verified: no API credentials yet, so no live history; the next day's badge slides under the floating one instead of pushing it up |
| history | Unread messages separator and jump to first unread | M3 | in progress | snapshot only (`cargo run -p rgram --example snapshots`, 42/44/45); not verified: no API credentials, so no live chat; opens around the last read message; does not mark messages read (viewMessages) yet |
| history | Service messages for every action type | M3 | in progress | snapshot only (`cargo run -p rgram --example snapshots`, 42/43); not verified: no API credentials, so no live chat; common actions only, others hidden |
| history | Outgoing state: clock, sent, read, failed with retry | M3 | in progress | snapshot only (`cargo run -p rgram --example snapshots`, 42-43); not verified: no API credentials yet, so no live history; failed shows the clock, no retry |
| history | Edited mark, channel views, forwards count, post signatures | M3 | in progress | snapshot only (`cargo run -p rgram --example snapshots`, 42/43); not verified: no API credentials, so no live chat; no forwards count yet |
| history | Forwarded header (user, channel, hidden sender, saved from) | M3 | in progress | snapshot only (`cargo run -p rgram --example snapshots`, 42/43); not verified: no API credentials, so no live chat; no "saved from" yet |
| history | Reply header with quote, jump to replied message and back | M3 | in progress | snapshot only (`cargo run -p rgram --example snapshots`, 42/44/45); not verified: no API credentials, so no live chat; replied messages fetched with getMessage; click jumps, the down button returns, the target is shaded |
| history | Reply to a message in another chat, reply to a story | M3 | in progress | unit test (origin name and caption-less media, story poster); snapshot only (54); not verified: no API credentials, so no live chat; the header names the origin in its color with the quote or media kind, a story reply names the poster and "Story"; no peer emoji or thumbnail, clicking does not open the other chat or the story yet |
| history | "via @bot" label | M3 | in progress | snapshot 42 and unit test (`cargo test -p features via_names`); "via @username" after the sender's name in the service color, alone when no name shows; not verified: no API credentials, so no live inline-bot message; dropped on forwarded messages (tdesktop appends it to the forward line), a click does not insert the bot's username into the message field yet (M4) |
| history | Entities: bold, italic, underline, strikethrough, code, pre | M3 | in progress | snapshot only (`cargo run -p rgram --example snapshots`, 42/43); not verified: no API credentials, so no live chat; underline, strikethrough and pre not in the fixtures |
| history | Entities: syntax highlighting in pre blocks with copy button | M3 | todo | |
| history | Entities: links, text links, mentions, hashtags, cashtags, bot commands | M3 | in progress | snapshot only (`cargo run -p rgram --example snapshots`, 42/43); not verified: no API credentials, so no live chat; links, text links (confirm box), mentions, hashtags open; cashtags and bot commands are inert |
| history | Entities: emails, phone numbers, bank cards with their menus | M3 | in progress | unit test (`cargo test -p features phone_numbers_and_bank_cards`); snapshots 67/68: a phone opens "Copy Phone Number" over who has the number ("This number is not on Telegram" in the runner); emails open mail and copy from the message menu; bank cards open "Copy Card Number" with the bank's links and name from `getBankCardInfo`; not verified: no API credentials, so no live lookups; no "Add to contacts" (M6); the found user opens their chat, not the profile (M6); menus open once TDLib answers instead of showing "Loading..." |
| history | Entities: spoilers with the reveal animation | M3 | in progress | snapshot only (`cargo run -p rgram --example snapshots`, 42/43); not verified: no API credentials, so no live chat; click reveals, no particle animation |
| history | Entities: block quotes and expandable quotes | M3 | in progress | snapshot only (`cargo run -p rgram --example snapshots`, 42/43); not verified: no API credentials, so no live chat; expandable quotes do not collapse |
| history | Entities: custom emoji in text | M5 | todo | |
| history | Entities: formatted dates and timestamps (timecodes) | M3 | in progress | unit tests (`cargo test -p features dates`: relative and absolute formats, entities moved around the new text); snapshots 64-66: "6:30 pm" sent with a time-only format reads "18:30" as a link and opens "Copy Date"; not verified: no API credentials; relative dates refresh with the history, not on a timer; captions keep dates as sent (M5); 24-hour times for every locale (M7); no "Add to Calendar" (M7) or "Set a Reminder" (M4); timecodes wait for media players (M5) |
| history | Large single emoji and animated emoji | M5 | todo | |
| history | Emoji interactions (click an animated emoji, see the partner's) | M5 | todo | |
| history | Link previews: small or large media, above or below the text | M3 | in progress | snapshot 54 and a unit test (`cargo test -p features web_page`); site name, title and description in a tinted box above or below the text, the time on its last line, a click opens the link (asking first unless TDLib says the text shows it); not verified: no API credentials, so no real linkPreview from TDLib; no media (small or large), no type-specific button, description without entities, line limits estimated |
| history | Loading history upward and downward without scroll jumps | M3 | in progress | unit tests (model history, list splices); `scripts/history-perf.sh`: adding 50 older messages near the top of a 10,000-message history changes no history pixel (3 runs); not verified live: no API credentials yet, and loading newer pages is not measured |
| history | Scroll-to-bottom button with unread counter | M3 | in progress | snapshot only (`cargo run -p rgram --example snapshots`, 44/45); not verified: no API credentials, so no live chat; no show/hide animation, badge 20 px instead of 22 |
| history | Jump buttons to unread mentions and unread reactions | M3 | in progress | snapshots 42 and 44 (stacked over the down button with their counters); not verified: no API credentials, so searchChatMessages with the unread filters and viewMessages never ran live; a click jumps to the oldest of the newest 100 and reads it at once instead of when it scrolls into view; no show/hide animation |
| history | Jump to a date with the calendar | M3 | in progress | snapshots 55/56 and unit tests (`cargo test -p features calendar`); a date badge opens the month with the badge's day highlighted, days before the chat's first loaded day or after today disabled, a click jumps to the day's first message (getChatMessageByDate, then the message after it); not verified: no API credentials, so the jump never ran live; one month at a time with arrows instead of tdesktop's scrolling months, weeks start on Monday, no month/year chooser, no selecting days to clear history |
| history | Highlight of the message jumped to | M3 | in progress | snapshot only (44, 52, 53); shaded with the selection overlay for 3 s after a reply or pin jump, no fade yet; jumped-to message lands at the top, not centered |
| history | Text selection and copy within and across messages | M3 | in progress | snapshot only (`cargo run -p rgram --example snapshots`, 46/47); not verified: no API credentials, so no live chat; copying to the clipboard not checked by eye; drag selects within one message, across messages selects messages as tdesktop does; no double-click word selection or autoscroll while dragging |
| history | Keyboard text selection | M8 | todo | |
| history | Multi-message selection: forward, delete, copy, download | M3 | in progress | snapshot only (`cargo run -p rgram --example snapshots`, 46/47); not verified: no API credentials, so no live chat; copying to the clipboard not checked by eye; drag or Ctrl/Cmd+click selects, Ctrl/Cmd+C copies, Escape and Cancel clear; forward, delete and download come with M4/M5; delete through the menu (not verified live) |
| history | Message context menu with all actions for the message type | M3 | in progress | unit tests (item choice per selection and permissions, revoke checkbox); snapshot only (`cargo run -p rgram --example snapshots`, 48/49); not verified: no API credentials, so no live permissions, message links or deletes; copy text and links, copy message link, delete with "delete for everyone", select and select up to here; no reply, edit, forward, pin or report yet; the menu snaps to the window instead of opening upward |
| history | Read date of outgoing messages ("read at") and who-read in small groups | M3 | in progress | unit tests (`cargo test -p features read_date`); in a private chat the menu of one outgoing message starts with "today at 14:05" (getMessageReadDate when getMessageProperties allows it), or "read" when either side hides read dates; not verified: no API credentials, so never seen live or in a snapshot; in a group the line names the one viewer, says "3 Seen" or "Nobody Viewed" (getMessageViewers when `can_get_viewers`; unit test `cargo test -p features seen_names`); the line is inert (no "show when" link to privacy settings), and has no viewer userpics or submenu listing the viewers |
| history | Pinned message bar with multiple pins, cycling and unpin | M3 | in progress | snapshot (42, 52, 53) and unit tests (`cargo test -p features pinned`, `-p model pins`); not verified: no API credentials, so searchChatMessages, updateMessageIsPinned and unpinning were never run live; first 100 pins only; hiding when the user can't unpin lasts until restart; with several pins the close button gives way to "show all" (snapshot 53) |
| history | Pinned messages section | M3 | in progress | snapshots 60-61 and unit test (`cargo test -p model pins`); the bar's "show all" opens the chat's pins under "2 pinned messages" with a back arrow (Escape also goes back), over "Unpin all N messages" with a confirmation (unpinAllChatMessages) or "Don't show pinned messages" for those who may not pin; not verified: no API credentials, so unpinning all never ran live and pin rights never came back true; the chat reopens at its newest message instead of where it was, no down button or search inside the section, a message click does not jump to it in the chat |
| history | Contact status bar: add contact, block, report spam, share phone | M3 | todo | |
| history | Business bot bar and paid-message bar | M13 | todo | |
| history | Join requests bar for admins | M6 | todo | |
| history | Translate bar and translated messages | M13 | todo | |
| history | Group call / video chat bar | M12 | todo | |
| history | Channel post comments (discussion) and the replies section | M3 | in progress | unit tests (thread history takes only its own new messages, post over replies, unread comments); snapshot only (64: the button under posts, "Leave a comment" and "12 comments" with the last three commenters and the unread dot); the replies section checked once with a temporary fixture thread (bar "2 comments", the post, "Discussion started", the replies; Back restores the channel pixel for pixel), not kept as a snapshot because the snapshot run has no TDLib answers; not verified: no API credentials, so no live discussion group; no compose in the thread (M4), no "N replies" in groups, the count in the bar is as of opening, opens at the newest reply instead of the first unread |
| history | Saved Messages: sublists per origin chat and tags | M6 | in progress | the model keeps a history per Saved Messages topic (`getSavedMessagesTopicHistory`); tdesktop lists the sublists in the Saved Messages profile (`Info::Saved::SublistsWidget`), so the list waits for the profile panel (M6) |
| history | Sponsored messages in channels with "about" and hide | M3 | todo | |
| history | Protected content (no forwards, no copy, no screenshots) | M3 | in progress | unit test (`cargo test -p features protected_messages`); messages TDLib marks as not saveable offer no Copy in the menu, and Ctrl+C over them copies nothing and shows "Copying is not allowed…" for the chat's kind; not verified: no API credentials, so no live protected chat; forwarding is not built yet (M4), screenshots are the M7 platform row |
| history | "Pull to next channel" at the bottom of a read channel | M3 | todo | |
| history | Swipe back gesture | M8 | todo | |
| history | Unsupported message notice | M3 | done | snapshot 54 and a unit test (`cargo test -p features unsupported`); tdesktop's italic notice with its link, in the bubble and the chat list preview; not verified live (no API credentials) |
| history | Bot "about" card in an empty bot chat and start button | M10 | todo | |
| history | Sensitive (18+) media blurred, with the show setting | M5 | todo | |
| history | Message effects on send (Premium) | M13 | todo | |
| history | AI summary of long messages and threads | M13 | todo | |
| history | Streamed (live-typed) bot replies | M10 | todo | |
| history | Ephemeral (self-hiding) messages | M9 | todo | |

## Reactions (`history/view/reactions/`)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| reactions | Reaction counters in bubbles with userpics in small chats | M3 | in progress | unit tests (order: paid, then most chosen by others; userpics only when all who reacted are named and at most 3; both sides in private chats; the emoji look for lone code points); snapshot only (64: counts with the chosen one filled, userpics, the time on the last row or wrapped below); not verified: no API credentials, so no live reactions; custom emoji and paid reactions show a star placeholder until custom emoji (M5) and Stars (M13); Saved Messages tags draw as plain pills without titles; clicking a reaction does nothing until sending (M4) |
| reactions | Hover button, reaction strip and expanded selector with custom emoji | M4 | todo | |
| reactions | Quick reaction on double-click | M4 | todo | |
| reactions | Reaction animations | M8 | todo | |
| reactions | Who reacted list with tabs per reaction | M3 | in progress | unit tests (the menu line's text rules from `refreshText`, tab order, the reaction type sent to TDLib); checked once by a temporary fixture: "63 Reactions" in a channel post's menu, the list with All 63 / 🔥48 / 👍12 / 😍3 tabs and rows with the reaction; not verified: no API credentials, so no live `getMessageAddedReactions`; the list is a layer, not the info column (M6); no tab of who only saw the message; no userpics or submenu in the menu line; rows and a single reactor do not open profiles (M6); custom emoji show a star placeholder (M5) |
| reactions | Paid star reactions with the top senders list | M13 | todo | |
| reactions | Allowed reactions per chat | M6 | todo | |

## Admin log (`history/admin_log/`)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| admin-log | Recent actions of a group or channel | M6 | todo | |
| admin-log | Filters by action type and admin | M6 | todo | |

## Compose and sending (`history/view/controls/`, `chat_helpers/`, sending boxes)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| compose | Multi-line field with auto-grow and scroll | M4 | todo | |
| compose | Formatting by shortcuts and context menu (bold, italic, underline, strike, mono, spoiler, quote, link, clear) | M4 | todo | |
| compose | Markdown-style formatting while typing | M4 | todo | |
| compose | Send with Enter or Ctrl+Enter, by setting | M4 | todo | |
| compose | Send menu: silent, scheduled, when online | M4 | todo | |
| compose | Schedule box and scheduled messages section (edit, send now, reschedule) | M4 | todo | |
| compose | Reply panel, reply with a quote, reply in another chat | M4 | todo | |
| compose | Edit message panel, edit caption and media | M4 | todo | |
| compose | Forward panel with options: hide sender, hide captions, change recipient | M4 | todo | |
| compose | Share / forward box: several recipients, comment, search | M4 | todo | |
| compose | Link preview panel: remove, above/below, larger/smaller media | M4 | todo | |
| compose | Cloud drafts per chat and per topic, synced | M4 | todo | |
| compose | Draft stash (keep a draft aside and restore it) | M4 | todo | |
| compose | Mention autocomplete | M4 | todo | |
| compose | Hashtag autocomplete | M4 | todo | |
| compose | Bot command autocomplete | M10 | todo | |
| compose | Emoji suggestions by shortcode and keywords | M4 | todo | |
| compose | Sticker suggestions by emoji | M5 | todo | |
| compose | Character counter and limits for text and captions | M4 | todo | |
| compose | Emoji, sticker and GIF button with the tabbed panel | M5 | todo | |
| compose | Attach: photos and videos (compressed or as files), files, albums | M4 | todo | |
| compose | Send files box: grouping, spoiler, caption above or below, reorder | M4 | todo | |
| compose | Paste images and files from the clipboard | M4 | todo | |
| compose | Drag and drop files into the chat (compressed or as files zones) | M4 | todo | |
| compose | Upload progress and cancel | M4 | todo | |
| compose | Send a GIF with a caption | M5 | todo | |
| compose | Voice recording: hold, lock, cancel, preview, view once | M4 | todo | |
| compose | Round video recording | M5 | todo | |
| compose | Send-as chooser (send as a channel) | M4 | todo | |
| compose | Slow mode countdown and send restrictions | M4 | todo | |
| compose | Join, mute/unmute, unblock and start-bot bars in place of the field | M4 | todo | |
| compose | Search in chat bar with results navigation and sender filter | M3 | in progress | snapshots 62-63 and unit tests (`cargo test -p features chat_search`); the top bar's search button swaps the bar for a field after a back arrow (Escape or the arrow closes it); typing searches after 0.9 s, Enter at once (searchChatMessages); the bottom bar shows "1 of 12" or "No results" with the calendar and up/down arrows that jump to and shade each result, dimmed at the ends; the results bar was checked by eye once with a fixture answer, since the snapshot run has no TDLib; not verified: no API credentials, so no live search; first 100 results only, no sender filter, no results list, no F3/Ctrl+G stepping, no highlighting of the query in the text |
| compose | Delete messages: for me or for everyone, with confirmation | M4 | todo | |
| compose | Report messages with reasons | M4 | todo | |
| compose | Pin message box (notify all members, pin for both) | M4 | todo | |
| compose | Create polls and quizzes (with media and links on options) | M4 | todo | |
| compose | Create and edit checklists, add and mark tasks | M4 | todo | |
| compose | Share a location and live location (location picker) | M5 | todo | |
| compose | Share a contact | M4 | todo | |
| compose | Send dice and other animated emoji games | M5 | todo | |
| compose | Auto-delete timer for new messages in a chat | M4 | todo | |
| compose | Spell checking in the field | M8 | todo | |
| compose | Compose with AI and AI tones | M13 | todo | |
| compose | Suggested posts to channels (with price and date) | M13 | todo | |
| compose | Paid messages to users and groups that charge Stars | M13 | todo | |
| compose | Rich messages: compose, edit, send | M13 | todo | |

## Media in messages (`history/view/media/`, `media/`)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| media | Photos with blurred placeholder and progressive load | M5 | todo | |
| media | Albums in mosaic layout | M5 | todo | |
| media | Videos: thumbnail, duration, inline autoplay without sound | M5 | todo | |
| media | Video covers and video timestamps | M5 | todo | |
| media | Streaming playback while downloading | M5 | todo | |
| media | Files: icon, size, progress, download, open, show in folder | M5 | todo | |
| media | Music files with the player | M5 | todo | |
| media | Voice messages: waveform, play, seek, speed | M5 | todo | |
| media | Voice and video transcription (Premium) | M13 | todo | |
| media | Round video messages | M5 | todo | |
| media | Stickers: WebP, animated TGS (Lottie), video WebM | M5 | todo | |
| media | GIFs (MPEG-4 animations) with autoplay | M5 | todo | |
| media | Dice and slot machine animations | M5 | todo | |
| media | Contacts in messages | M5 | todo | |
| media | Locations, venues and live locations with a map preview | M5 | todo | |
| media | Polls and quizzes: vote, retract, results, explanation | M5 | todo | |
| media | Checklists in messages | M5 | todo | |
| media | Games | M10 | todo | |
| media | Invoices | M13 | todo | |
| media | Story mentions and shared stories | M11 | todo | |
| media | Gift, unique gift, Premium gift and giveaway service boxes | M13 | todo | |
| media | Suggested profile photo and birthday suggestion boxes | M6 | todo | |
| media | Media spoilers | M5 | todo | |
| media | Paid media (Stars) | M13 | todo | |
| media | View-once photos, videos and voice messages | M5 | todo | |
| media | Similar channels block after joining | M6 | todo | |
| media | Call messages (missed, outgoing, duration) | M12 | todo | |
| media | TON transfer messages | M13 | todo | |
| media | Theme documents in messages | M6 | todo | |
| media | "Save as" for files and media | M5 | todo | |
| media | Auto-download by chat type, media type and size | M6 | todo | |
| media | Download manager with the downloads list | M5 | todo | |

## Media viewer and player (`media/view/`, `media/player/`)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| media-view | Full-window viewer for photos and videos with paging | M5 | todo | |
| media-view | Zoom, fit, rotate, fullscreen | M5 | todo | |
| media-view | Album thumbnails strip | M5 | todo | |
| media-view | Caption, sender and date; forward, delete, save, copy | M5 | todo | |
| media-view | Video controls: play, seek, volume, speed, quality | M5 | todo | |
| media-view | Picture-in-picture | M5 | todo | |
| media-view | Profile photo gallery in the viewer | M6 | todo | |
| media-view | Text recognition in photos (select and copy) | M7 | todo | |
| media-view | Sponsored messages in long videos | M5 | todo | |
| player | Audio player bar with playlist panel | M5 | todo | |
| player | Repeat, shuffle, reverse order, speed, volume | M5 | todo | |
| player | Floating round-video player | M5 | todo | |
| player | Voice messages queue and listened marks | M5 | todo | |
| player | OS media controls (macOS Now Playing, Windows SMTC, MPRIS) | M7 | todo | |

## Stickers, emoji and GIFs (`chat_helpers/`, sticker boxes)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| stickers | Emoji panel: categories, recent, skin tones, search | M5 | todo | |
| stickers | Custom emoji packs in the emoji panel (Premium) | M5 | todo | |
| stickers | Sticker panel: recent, favorites, installed sets, search, trending | M5 | todo | |
| stickers | GIF panel: saved GIFs and search | M5 | todo | |
| stickers | Sticker set box: preview, add, remove, share link | M5 | todo | |
| stickers | Installed, archived and featured sets manager with reorder | M6 | todo | |
| stickers | Emoji picker overlay on hold (variants) | M5 | todo | |
| stickers | Emoji style sets (download and switch) | M6 | todo | |
| stickers | Sticker preview on hold | M5 | todo | |
| stickers | Create a sticker or sticker set | M13 | todo | |

## Info panel and profiles (`info/`, `profile/`, `boxes/peers/`)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| info | Info panel in the right column, as a layer or in a separate window | M6 | todo | |
| info | User profile: photo, name, status, bio, usernames, phone, birthday | M6 | todo | |
| info | User profile: personal channel, notes, business hours and location | M6 | todo | |
| info | Profile action buttons: message, mute, call, video call, gift, share | M6 | todo | |
| info | Shared media tabs: photos, videos, files, music, voice, links, GIFs | M6 | todo | |
| info | Shared media calendar and zoomable grid | M6 | todo | |
| info | Common groups and similar channels | M6 | todo | |
| info | Group and channel profile: description, link, members, admins | M6 | todo | |
| info | Members list with search, roles, kick and restrict | M6 | todo | |
| info | Bot profile: description, commands, privacy policy | M10 | todo | |
| info | Profile photo gallery, set, change and delete own photos | M6 | todo | |
| info | Profile color, cover and emoji backgrounds | M13 | todo | |
| info | Emoji status picker | M13 | todo | |
| info | QR code for a profile or username | M6 | todo | |
| info | Profile stories tab and story albums | M11 | todo | |
| info | Gifts tab and gift collections | M13 | todo | |
| info | Profile music (saved music) | M13 | todo | |
| info | Polls list in a chat | M6 | todo | |
| info | Short info card on userpic hover/click | M6 | todo | |
| info | Birthday effect on profiles | M8 | todo | |
| info | Create an avatar from emoji or sticker | M6 | todo | |
| peers | Edit contact: name, share phone, delete contact, add contact | M6 | todo | |
| peers | Block and unblock users; blocked list | M6 | todo | |
| peers | Report a chat or user with reasons | M6 | todo | |
| peers | Create a group (members, title, photo, auto-delete) | M6 | todo | |
| peers | Create a channel (title, photo, description, type) | M6 | todo | |
| peers | Add members, invite by link | M6 | todo | |
| peers | Edit group or channel: title, photo, description | M6 | todo | |
| peers | Group and channel type: public with usernames (reorder, collectible) or private | M6 | todo | |
| peers | History visibility for new members, discussion group link, signatures | M6 | todo | |
| peers | Permissions, slow mode, banned rights per member | M6 | todo | |
| peers | Admins with rights and custom titles | M6 | todo | |
| peers | Allowed reactions | M6 | todo | |
| peers | Invite links: create, edit (expiry, limit, approval, subscription), revoke | M6 | todo | |
| peers | Join requests: approve, dismiss | M6 | todo | |
| peers | Transfer ownership | M6 | todo | |
| peers | Channel and group colors and levels | M13 | todo | |
| peers | Member tags | M6 | todo | |
| peers | Communities: create, manage, add chats, pending requests | M6 | todo | |
| peers | Enable topics in a group | M9 | todo | |
| peers | Verify peers (verification bots) | M10 | todo | |
| peers | Replace boosts between chats | M13 | todo | |

## Settings (`settings/`, settings boxes)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| settings | Main page with account card, sections and settings search | M6 | todo | |
| settings | Settings search with keyboard navigation | M6 | todo | |
| settings | My account: name, username, bio, birthday, phone number change | M6 | todo | |
| settings | Personal channel | M6 | todo | |
| settings | Notifications: private chats, groups, channels, stories, reactions | M6 | todo | |
| settings | Notifications: sound, preview, sender, count badge, include muted | M6 | todo | |
| settings | Notification exceptions per chat | M6 | todo | |
| settings | Custom notification sounds (upload, choose, per chat) | M6 | todo | |
| settings | Privacy: phone number, last seen (with read-time hiding), profile photo | M6 | todo | |
| settings | Privacy: forwards, calls, voice messages, invites, birthday, gifts, bio | M6 | todo | |
| settings | Privacy: messages from non-contacts (Premium or Stars price) | M13 | todo | |
| settings | Privacy rule exceptions (always/never share with) | M6 | todo | |
| settings | Blocked users | M6 | todo | |
| settings | Active sessions: list, terminate, terminate all, auto-terminate period | M6 | todo | |
| settings | Logged-in websites (Log in with Telegram) | M6 | todo | |
| settings | Unconfirmed new login review and denial | M6 | todo | |
| settings | Passkeys management | M7 | todo | |
| settings | Two-step verification: set, change, hint, recovery email, remove | M6 | todo | |
| settings | Login email change | M6 | todo | |
| settings | Global auto-delete timer | M6 | todo | |
| settings | Account self-destruction period | M6 | todo | |
| settings | Archive and mute new non-contacts | M6 | todo | |
| settings | Suggest frequent contacts; delete cloud drafts; clear payment info | M6 | todo | |
| settings | Sensitive content setting | M6 | todo | |
| settings | Chat settings: text and emoji options, large emoji, replace emoji | M6 | todo | |
| settings | Chat settings: stickers suggestions, loop animated stickers | M6 | todo | |
| settings | Chat settings: default reaction (quick reaction) | M6 | todo | |
| settings | Chat settings: send by Enter or Ctrl+Enter | M6 | todo | |
| settings | Auto-download and autoplay settings | M6 | todo | |
| settings | Download folder and "ask where to save" | M6 | todo | |
| settings | Archive settings (keep unmuted chats archived, folders) | M6 | todo | |
| settings | Folders settings page | M6 | todo | |
| settings | Language: cloud language list, search, apply without restart | M6 | todo | |
| settings | Translation options and do-not-translate languages | M13 | todo | |
| settings | Interface scale (100–300 %, follows system) | M6 | todo | |
| settings | Connection type and proxies: SOCKS5, MTProto, HTTP, list, check, share links | M6 | todo | |
| settings | Data and storage: cache size limits, clear cache, storage usage | M6 | todo | |
| settings | Power saving and animation toggles | M8 | todo | |
| settings | System window frame option | M7 | todo | |
| settings | Tray, launch at startup, start minimized, close to tray | M7 | todo | |
| settings | Calls settings: devices, peer-to-peer, accept calls on this device | M12 | todo | |
| settings | Keyboard shortcuts page | M8 | todo | |
| settings | Experimental settings | M14 | todo | |
| settings | Local passcode and auto-lock timer | M7 | todo | |
| settings | Ask a question (support chat) and FAQ link | M6 | todo | |
| settings | Telegram features page link and About box | M6 | todo | |
| settings | Default interface for chats: one-column, two-column preferences | M2 | todo | |

## Themes and backgrounds (`window/themes/`, background boxes)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| themes | Day and night themes, night mode toggle in the main menu | M6 | in progress | snapshot only (`cargo run -p rgram --example snapshots`): day and night palettes and the toggle, saved in settings.json; the night theme's background image waits for wallpapers in M3 |
| themes | Follow the system dark mode | M7 | todo | |
| themes | Accent colors for the built-in themes | M6 | todo | |
| themes | Cloud themes: list, apply, create from current, share links | M6 | todo | |
| themes | Import and apply `.tdesktop-theme` files | M6 | todo | |
| themes | Theme editor (palette editing, export) | M6 | todo | |
| themes | Chat backgrounds: gallery, solid, gradient, pattern, blur, own image | M6 | todo | |
| themes | Per-chat themes and per-chat wallpapers | M6 | todo | |
| themes | Theme preview before applying | M6 | todo | |

## Window and navigation (`window/`, main menu)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| window | Main menu: account, My Profile, new group, new channel, contacts, calls, Saved Messages, settings | M2 | in progress | snapshot 12: every item is there; Contacts and Saved Messages work; Settings opens only the log-out box until the settings screen (M6); My Profile and new group / channel close the menu until M6, Calls until M10; not verified: no API credentials yet, so no live account |
| window | Main menu: accounts list, night mode switch, version | M2 | in progress | snapshot only (`cargo run -p rgram --example snapshots`); no accounts list until multi-account in M8; not verified: no API credentials yet, so no live account |
| window | Section slide animations and back navigation | M8 | todo | |
| window | Chat switcher (Ctrl+Tab) | M8 | todo | |
| window | Media preview on hover (stickers, GIFs) | M5 | todo | |
| window | Separate windows per chat, restored on restart | M8 | todo | |
| window | Lock screen (passcode) | M7 | todo | |
| window | Set up a login email when the server asks for it | M6 | todo | |
| window | Toasts and layer boxes with the tdesktop look | M2 | in progress | snapshots: boxes (M1, M2) and the archive toast (26), centered, fading in 200 ms, out 1 s after its duration (`features::toast`); one toast at a time where tdesktop stacks them; no icon or link toasts yet; not verified: no API credentials yet, so no live account |

## Notifications and OS integration (`window/notifications_*`, `platform/`, `core/`)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| notifications | Native notifications on macOS, Windows and Linux | M7 | todo | |
| notifications | Quick reply and mark as read from a notification | M7 | todo | |
| notifications | Built-in notification popups (position, count) as an alternative | M7 | todo | |
| notifications | Grouping, sound, do-not-disturb and focus handling | M7 | todo | |
| platform | Tray icon with menu and unread count | M7 | todo | |
| platform | Dock / taskbar unread badge | M7 | todo | |
| platform | macOS Dock menu and global menu bar | M7 | todo | |
| platform | macOS Touch Bar | M7 | todo | |
| platform | Windows taskbar thumbnail buttons and jump list | M7 | todo | |
| platform | Linux X11 and Wayland, including native title bar and portals | M7 | todo | |
| platform | Native file dialogs | M4 | todo | |
| platform | Launch at startup | M7 | todo | |
| platform | Screenshot protection for protected chats | M7 | todo | |
| platform | Sleep, wake and network change handling | M7 | todo | |
| platform | Current location for the location picker | M7 | todo | |
| platform | Outdated OS warning bar | M7 | todo | |
| links | `tg://` links and `https://t.me` links open in rgram | M7 | todo | |
| links | All internal link types: users, chats, messages, invites, stickers, proxies, themes, settings, bots, games, stories, boosts, gifts | M7 | todo | |
| links | Phone number and bank card menus from text | M3 | in progress | see the history row on emails, phone numbers and bank cards |
| core | Single instance: a second launch activates the running one and passes links | M7 | todo | |
| core | Keyboard shortcuts as in tdesktop (quit, lock, search, folders, chat navigation) | M8 | todo | |
| core | Accessibility: screen readers through AccessKit | M8 | todo | |
| core | IME for CJK and other input methods | M8 | todo | |
| core | HiDPI and fractional scaling | M8 | todo | |
| core | Application update check and install for rgram releases | M8 | todo | |
| core | Crash report window | — | not applicable | Uploads Telegram Desktop crash dumps to Telegram's own crash server |
| core | "What's new" changelog messages after an update | — | not applicable | Describe Telegram Desktop's own releases |
| core | Linux protected install (updater privilege split) | — | not applicable | Part of Telegram Desktop's own updater |

## Secret chats and topics (M9)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| secret | Start a secret chat with a user | M9 | todo | |
| secret | Accept an incoming secret chat; waiting and closed states | M9 | todo | |
| secret | Encryption key visualization | M9 | todo | |
| secret | Self-destruct timer and destroying messages | M9 | todo | |
| secret | Secret chat restrictions (no forwards, screenshot notices) | M9 | todo | |
| topics | Forum topic list with per-topic unread, mentions and reactions | M9 | todo | |
| topics | Topic tabs in the chat (subsection tabs) | M9 | todo | |
| topics | Create, edit (title, icon), close, reopen, hide, pin and delete topics | M9 | todo | |
| topics | Topic notifications and mute | M9 | todo | |
| topics | Bot forum topics in private chats | M10 | todo | |

## Bots (`inline_bots/`, bot keyboards, mini apps)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| bots | Reply keyboards (resize, one-time, placeholder, remove) | M10 | todo | |
| bots | Inline keyboards: callback, URL, switch inline, copy, pay, login, web app, user/chat requests | M10 | todo | |
| bots | Callback answers as toast or alert | M10 | todo | |
| bots | Commands menu button and command list | M10 | todo | |
| bots | Inline mode: results panel with all layouts, switch to PM | M10 | todo | |
| bots | Recent inline bots | M10 | todo | |
| bots | Prepared inline messages from mini apps | M10 | todo | |
| bots | Mini apps (Web Apps) through the system webview | M10 | todo | |
| bots | Mini apps: fullscreen, permissions, device storage, downloads, location, sharing | M10 | todo | |
| bots | Attachment menu bots and main mini app buttons | M10 | todo | |
| bots | Login widget links (URL authorization box) | M10 | todo | |
| bots | Games (open game URL, scores) | M10 | todo | |
| bots | Add a bot to a group or channel with rights | M10 | todo | |
| bots | Managed bots: create and manage bots | M10 | todo | |
| bots | Bot verification badges | M10 | todo | |
| bots | Business chatbots connection | M13 | todo | |

## Stories (`media/stories/`, stories data)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| stories | Stories strip in the chat list, expanded and collapsed | M11 | todo | |
| stories | Hidden (archived) stories of contacts | M11 | todo | |
| stories | Viewer: progress, pause, tap navigation, sibling transitions | M11 | todo | |
| stories | Captions with entities, full caption view | M11 | todo | |
| stories | Story areas: locations, reactions, links, weather, channel posts | M11 | todo | |
| stories | Reactions and replies to stories | M11 | todo | |
| stories | Recent viewers of own stories with reactions | M11 | todo | |
| stories | Reposts and sharing stories | M11 | todo | |
| stories | Stealth mode (Premium) | M13 | todo | |
| stories | Own stories: archive, pin to profile, albums | M11 | todo | |
| stories | Post a story (photo or video, privacy, caption) | M11 | todo | |
| stories | Story statistics | M13 | todo | |

## Calls (`calls/`)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| calls | One-to-one voice call with an official client | M12 | todo | |
| calls | One-to-one video call, camera switch | M12 | todo | |
| calls | Incoming call window with accept and decline | M12 | todo | |
| calls | Encryption emoji fingerprint, signal bars | M12 | todo | |
| calls | Active call top bar, mute, devices menu | M12 | todo | |
| calls | Screen sharing with source picker | M12 | todo | |
| calls | Call rating after the call | M12 | todo | |
| calls | Recent calls list | M12 | todo | |
| group-calls | Video chats in groups and channels: join, leave, mute, raise hand | M12 | todo | |
| group-calls | Members list with speaking indicators and volume per member | M12 | todo | |
| group-calls | Video grid and screen sharing in group calls | M12 | todo | |
| group-calls | Join as a channel; scheduled video chats | M12 | todo | |
| group-calls | Recording, title, invite links, admin controls | M12 | todo | |
| group-calls | RTMP live streams and live streams in the viewer | M12 | todo | |
| group-calls | Conference calls with end-to-end encryption | M12 | todo | |
| group-calls | Messages and paid reactions inside group calls | M12 | todo | |

## Payments, Stars and gifts (`payments/`, gifts boxes)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| payments | Checkout panel for invoices: items, shipping, tips, total | M13 | todo | |
| payments | Card entry with tokenization (Stripe, SmartGlocal) and saved credentials | M13 | todo | |
| payments | 3-D Secure and provider pages | M13 | todo | |
| payments | Receipts | M13 | todo | |
| stars | Stars balance, top-up, transactions history | M13 | todo | |
| stars | Stars invoices, subscriptions and their management | M13 | todo | |
| stars | Gift Stars to a user | M13 | todo | |
| gifts | Send a gift (with message, anonymously) | M13 | todo | |
| gifts | Received gifts: show on profile, convert, upgrade, transfer, wear | M13 | todo | |
| gifts | Gift resale market with filters | M13 | todo | |
| gifts | Gift auctions | M13 | todo | |
| gifts | Gift crafting | M13 | todo | |
| gifts | Collectible usernames and numbers info | M13 | todo | |

## Premium and Business (`boxes/premium_*`, `settings/business/`)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| premium | Premium page with features and subscribe | M13 | todo | |
| premium | Limit-reached boxes with upgrade | M13 | todo | |
| premium | Feature previews | M13 | todo | |
| premium | Gift Premium, gift codes, giveaways | M13 | todo | |
| premium | Boosts: boost a chat, boost links, boost list | M13 | todo | |
| business | Business hub page | M13 | todo | |
| business | Away and greeting messages | M13 | todo | |
| business | Quick replies with shortcuts and editor | M13 | todo | |
| business | Working hours and location | M13 | todo | |
| business | Chat intro and business chat links | M13 | todo | |
| business | Chatbots for business | M13 | todo | |

## Instant View, rich messages and export (`iv/`, `export/`)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| iv | Instant View pages in a separate window | M13 | todo | |
| iv | Instant View search and zoom | M13 | todo | |
| iv | Rich messages (articles with blocks, media, formulas) | M13 | todo | |
| iv | Rich message editor: blocks, lists, tables, media, formulas | M13 | todo | |
| iv | Export a rich message to HTML | M13 | todo | |
| export | Export Telegram data: settings (types, size limits, period, format) | M13 | todo | |
| export | Export writers: HTML, JSON, HTML and JSON | M13 | todo | |
| export | Export progress with the top bar and cancel | M13 | todo | |
| export | Export a single chat's history | M13 | todo | |

## Editor (`editor/`)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| editor | Photo editor: crop, rotate, flip | M13 | todo | |
| editor | Paint: brushes, colors, undo | M13 | todo | |
| editor | Text, stickers and shapes on photos | M13 | todo | |
| editor | Video editor: trim, quality, audio track | M13 | todo | |
| editor | Draw to reply | M13 | todo | |

## Statistics and earnings (`info/statistics/`, `info/channel_statistics/`, `statistics/`)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| stats | Channel statistics with charts (growth, followers, views, sources, languages) | M13 | todo | |
| stats | Group statistics | M13 | todo | |
| stats | Message and story statistics, public forwards | M13 | todo | |
| stats | Chart interactions: zoom, filters, point details | M13 | todo | |
| stats | Export statistics (XLSX) | M13 | todo | |
| stats | Channel revenue in TON and Stars with withdrawal | M13 | todo | |
| stats | Bot earnings and the Stars affiliate program | M13 | todo | |
| stats | Giveaways: create and view | M13 | todo | |

## Passport, AI and translation (`passport/`, `boxes/translate_*`, AI boxes)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| passport | Telegram Passport authorization forms | M13 | todo | |
| passport | Identity documents, scans, translations, address | M13 | todo | |
| passport | Phone and email verification inside Passport | M13 | todo | |
| translate | Translate a message or selection | M13 | todo | |
| translate | Auto-translate in channels | M13 | todo | |
| ai | AI tones management | M13 | todo | |
| ai | Create with AI for rich messages | M13 | todo | |

## TON wallet (`wallet/`, `gram/`)

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| wallet | Wallet panel: card, balance, history | M13 | todo | |
| wallet | Create, import, back up (secret phrase) and delete a wallet | M13 | todo | |
| wallet | Send and receive TON, gasless transfers | M13 | todo | |
| wallet | Collectibles (NFTs) and moving gifts | M13 | todo | |
| wallet | Card customization (stickers, backgrounds) | M13 | todo | |
| wallet | Key protection with the OS keychain | M13 | todo | |
| wallet | TON Connect: connect to dApps, approve requests | M13 | todo | |
| wallet | TON transfers between users in chats | M13 | todo | |

## Other

| Area | Feature | Milestone | Status | Verified |
|---|---|---|---|---|
| other | Emoji stake (dice bets in Stars) | M13 | todo | |
| other | About Cocoon box | M13 | todo | |
| other | Fact checks on channel posts (view and edit) | M13 | todo | |
| other | Telegram support account tools (templates, autocomplete) | — | not applicable | Only for Telegram support staff accounts |
