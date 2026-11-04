# rgram

rgram is an unofficial desktop client for Telegram, written in Rust. It uses the
Telegram API through [TDLib](https://github.com/tdlib/td) and is not affiliated
with Telegram. It aims to look and behave like Telegram Desktop while starting
faster and using less memory.

Status: early development, macOS only so far. See [PROGRESS.md](PROGRESS.md).

## To-do

Every user-visible feature of Telegram Desktop, from
[docs/parity.md](docs/parity.md). A feature is checked only after it ran on a
live account; *in progress* means the code exists but has not been verified
live yet. Regenerate the list with `scripts/readme-todo.sh`.

<!-- todo:begin -->
<details>
<summary><b>Sign-in</b>: 0 of 23 done, 21 in progress</summary>

- [ ] Welcome screen with "Start Messaging" (M1) *(in progress)*
- [ ] Offer to continue in the system language on the welcome screen (M1) *(in progress)*
- [ ] Phone number entry with country picker, search and per-country format mask (M1) *(in progress)*
- [ ] Country preselected from the network (`getCountryCode`) (M1) *(in progress)*
- [ ] QR code login: show code, refresh on expiry, finish when accepted elsewhere (M1) *(in progress)*
- [ ] Code sent to the Telegram app, with the "code sent to your app" hint (M1) *(in progress)*
- [ ] Code by SMS, by phone call, flash call and missed call (M1) *(in progress)*
- [ ] Code from Fragment, with the link to fragment.com (M1) *(in progress)*
- [ ] Resend code with countdown and next delivery method (M1) *(in progress)*
- [ ] Auto-submit when the code reaches its length; "wrong code" error (M1) *(in progress)*
- [ ] Login email setup when the server requires one, and email code entry (M1) *(in progress)*
- [ ] Reset the login email when the code does not arrive (M1) *(in progress)*
- [ ] Cloud password (2FA) check with hint (M1) *(in progress)*
- [ ] Password recovery through the recovery email code (M1) *(in progress)*
- [ ] Account reset when the password is forgotten (with waiting period) (M1) *(in progress)*
- [ ] Sign-up: first name, last name, photo (M1) *(in progress)*
- [ ] Terms of service on sign-up and later updates of the terms (M1) *(in progress)*
- [ ] FLOOD_WAIT error (tdesktop shows the static `lng_flood_error`, no countdown) (M1) *(in progress)*
- [ ] Clear errors: invalid phone, banned number box, too many attempts, network (M1) *(in progress)*
- [ ] Stay signed in across restarts (M1) *(in progress)*
- [ ] Log out with confirmation (M1) *(in progress)*
- [ ] The "alternatives to logging out" box (settings, passcode, add account) (M6)
- [ ] Sign in with a passkey (M7)

</details>

<details>
<summary><b>Accounts</b>: 0 of 6 done, 3 in progress</summary>

- [ ] Add another account (up to 3, 6 with Premium) (M8) *(in progress)*
- [ ] Switch accounts from the main menu, with unread badges per account (M8) *(in progress)*
- [ ] Online while the window is active and in use, offline when it is not, after 30 s without a key press, click or wheel, or when the account is switched away (`Updates::updateOnline`, `offlineIdleTimeout`) (M8) *(in progress)*
- [ ] Accounts in the tray menu (M8)
- [ ] Per-account notifications from inactive accounts (M8)
- [ ] Reorder accounts (M8)

</details>

<details>
<summary><b>Chat list</b>: 3 of 41 done, 24 in progress</summary>

- [x] Virtualized chat list in the server's order (M2)
- [ ] Row: userpic, name, time, last message with sender and entities (M2) *(in progress)*
- [ ] Row: media thumbnails and emoji in the last-message preview (M2) *(in progress)*
- [ ] Row: verified, Premium, emoji status, scam, fake and bot badges (M2) *(in progress)*
- [ ] Row: unread counter (muted in grey), mention and reaction badges (M2) *(in progress)*
- [ ] Row: outgoing message read checks (M2) *(in progress)*
- [ ] Row: draft preview ("Draft:") (M2) *(in progress)*
- [ ] Row: typing, uploading and other chat actions (M2) *(in progress)*
- [ ] Row: online indicator on user userpics (M2) *(in progress)*
- [ ] Row: animated (video) userpics (M5)
- [ ] Row: mute icon, pinned icon, unread mark (M2) *(in progress)*
- [ ] Pinned chats: pin, unpin, drag to reorder, pinned limit box (M2) *(in progress)*
- [ ] Archive: collapsed row, open, archive and unarchive chats (M2) *(in progress)*
- [ ] Archive row collapse/expand and hide into main menu (M2) *(in progress)*
- [x] Live updates: new messages, edits, deletes, read states reorder the list (M2)
- [ ] Connecting / Updating / Waiting for network status with proxy state (M2) *(in progress)*
- [ ] Adaptive one, two and three column layout with column resize (M2) *(in progress)*
- [ ] Narrow (userpics only) chat list mode (M2) *(in progress)*
- [ ] Chat context menu: pin, mute, mark read/unread, archive, add to folder (M2) *(in progress)*
- [ ] Chat context menu: clear history, delete and leave, block, report (M2) *(in progress)*
- [ ] Open chat in a separate window (M8)
- [ ] Chat preview without opening (peek) (M2)
- [ ] Touchpad swipe actions on rows (archive, read, pin, mute, delete) (M8)
- [x] Search: chats, contacts and global results (M2)
- [ ] Search: messages across all chats with date and type filters (M2) *(in progress)*
- [ ] Search: in one chat, by sender (M2)
- [ ] Search: hashtags and cashtags, in this chat or everywhere (M2) *(in progress)*
- [ ] Search: public posts in channels (with Premium limits) (M13)
- [ ] Search: recent searches and top peers strip (M2) *(in progress)*
- [ ] Search: channel and app suggestions in the empty search (M2) *(in progress)*
- [ ] Search: Saved Messages tags (M3) *(in progress)*
- [ ] Top bar suggestions: set a userpic, set a birthday, contacts' birthdays (M6)
- [ ] Top bar suggestions: unreviewed login, Premium offers and grace period (M13)
- [ ] Top bar suggestions: low Stars for subscriptions, gift auctions, custom promo (M13)
- [ ] Proxy sponsor (promoted) chat at the top (M6)
- [ ] Communities in the chat list, with joinable and requested chats (M6)
- [ ] Forum rows with the latest topics (M9)
- [ ] Stories strip above the list and story rings on userpics (M11)
- [ ] Offer to restore separate windows after restart (M8)
- [ ] Contacts list with online status, search and sorting (M2) *(in progress)*
- [ ] Empty-state screens (no chats, empty folder, no results) (M2) *(in progress)*

</details>

<details>
<summary><b>Chat folders</b>: 1 of 10 done, 3 in progress</summary>

- [x] Folder tabs on top or in a left sidebar, with unread counts (M2)
- [ ] "All chats" tab and its position (M2) *(in progress)*
- [ ] Folder tab context menu: edit, mark as read, add chats, delete (M2) *(in progress)*
- [ ] Reorder folders by dragging tabs (M2) *(in progress)*
- [ ] Folder editor: name with emoji, icon, included and excluded chat types and chats (M6)
- [ ] Folder color and folder tags on chat rows (Premium) (M13)
- [ ] Recommended folders (M6)
- [ ] Shareable folder links: create, edit, delete, join from a link (M6)
- [ ] Add a chat to a folder from the context menu or by drag and drop (M6)
- [ ] Folder limit box (M6)

</details>

<details>
<summary><b>Chat screen</b>: 0 of 55 done, 41 in progress</summary>

- [ ] Title bar: name, status (online, last seen, members, subscribers), typing (M3) *(in progress)*
- [ ] Title bar buttons: search, call, video call, info panel, menu (M3) *(in progress)*
- [ ] Chat menu: mute, clear history, delete, report, auto-delete, wallpaper (M3) *(in progress)*
- [ ] Bubbles with tails, grouping by sender and time, userpics in groups (M3) *(in progress)*
- [ ] Sender names in peer colors, admin and member titles (M3) *(in progress)*
- [ ] Date separators, sticky date on scroll (M3) *(in progress)*
- [ ] Unread messages separator and jump to first unread (M3) *(in progress)*
- [ ] Service messages for every action type (M3) *(in progress)*
- [ ] Outgoing state: clock, sent, read, failed with retry (M3) *(in progress)*
- [ ] Edited mark, channel views, forwards count, post signatures (M3) *(in progress)*
- [ ] Forwarded header (user, channel, hidden sender, saved from) (M3) *(in progress)*
- [ ] Reply header with quote, jump to replied message and back (M3) *(in progress)*
- [ ] Reply to a message in another chat, reply to a story (M3) *(in progress)*
- [ ] "via @bot" label (M3) *(in progress)*
- [ ] Entities: bold, italic, underline, strikethrough, code, pre (M3) *(in progress)*
- [ ] Entities: syntax highlighting in pre blocks with copy button (M3) *(in progress)*
- [ ] Entities: links, text links, mentions, hashtags, cashtags, bot commands (M3) *(in progress)*
- [ ] Entities: emails, phone numbers, bank cards with their menus (M3) *(in progress)*
- [ ] Entities: spoilers with the reveal animation (M3) *(in progress)*
- [ ] Entities: block quotes and expandable quotes (M3) *(in progress)*
- [ ] Entities: custom emoji in text (M5)
- [ ] Entities: formatted dates and timestamps (timecodes) (M3) *(in progress)*
- [ ] Large single emoji and animated emoji (M5) *(in progress)*
- [ ] Emoji interactions (click an animated emoji, see the partner's) (M5)
- [ ] Link previews: small or large media, above or below the text (M3) *(in progress)*
- [ ] Loading history upward and downward without scroll jumps (M3) *(in progress)*
- [ ] Scroll-to-bottom button with unread counter (M3) *(in progress)*
- [ ] Jump buttons to unread mentions and unread reactions (M3) *(in progress)*
- [ ] Jump to a date with the calendar (M3) *(in progress)*
- [ ] Highlight of the message jumped to (M3) *(in progress)*
- [ ] Text selection and copy within and across messages (M3) *(in progress)*
- [ ] Keyboard text selection (M8)
- [ ] Multi-message selection: forward, delete, copy, download (M3) *(in progress)*
- [ ] Message context menu with all actions for the message type (M3) *(in progress)*
- [ ] Read date of outgoing messages ("read at") and who-read in small groups (M3) *(in progress)*
- [ ] Pinned message bar with multiple pins, cycling and unpin (M3) *(in progress)*
- [ ] Pinned messages section (M3) *(in progress)*
- [ ] Contact status bar: add contact, block, report spam, share phone (M3) *(in progress)*
- [ ] Business bot bar and paid-message bar (M13)
- [ ] Join requests bar for admins (M6)
- [ ] Translate bar and translated messages (M13)
- [ ] Group call / video chat bar (M12)
- [ ] Channel post comments (discussion) and the replies section (M3) *(in progress)*
- [ ] Saved Messages: sublists per origin chat and tags (M6) *(in progress)*
- [ ] Sponsored messages in channels with "about" and hide (M3) *(in progress)*
- [ ] Protected content (no forwards, no copy, no screenshots) (M3) *(in progress)*
- [ ] "Pull to next channel" at the bottom of a read channel (M3) *(in progress)*
- [ ] Swipe back gesture (M8)
- [ ] Unsupported message notice (M3) *(in progress)*
- [ ] Bot "about" card in an empty bot chat and start button (M10)
- [ ] Sensitive (18+) media blurred, with the show setting (M5)
- [ ] Message effects on send (Premium) (M13)
- [ ] AI summary of long messages and threads (M13)
- [ ] Streamed (live-typed) bot replies (M10)
- [ ] Ephemeral (self-hiding) messages (M9)

</details>

<details>
<summary><b>Reactions</b>: 0 of 7 done, 2 in progress</summary>

- [ ] Reaction counters in bubbles with userpics in small chats (M3) *(in progress)*
- [ ] Hover button, reaction strip and expanded selector with custom emoji (M4)
- [ ] Quick reaction on double-click (M4)
- [ ] Reaction animations (M8)
- [ ] Who reacted list with tabs per reaction (M3) *(in progress)*
- [ ] Paid star reactions with the top senders list (M13)
- [ ] Allowed reactions per chat (M6)

</details>

<details>
<summary><b>Admin log</b>: 0 of 2 done</summary>

- [ ] Recent actions of a group or channel (M6)
- [ ] Filters by action type and admin (M6)

</details>

<details>
<summary><b>Compose and sending</b>: 0 of 47 done, 20 in progress</summary>

- [ ] Multi-line field with auto-grow and scroll (M4) *(in progress)*
- [ ] Send a text message, split past the length limit, the text kept when refused (M4) *(in progress)*
- [ ] Formatting by shortcuts and context menu (bold, italic, underline, strike, mono, spoiler, quote, link, clear) (M4)
- [ ] Markdown-style formatting while typing (M4)
- [ ] Send with Enter or Ctrl+Enter, by setting (M4) *(in progress)*
- [ ] Send menu: silent, scheduled, when online (M4) *(in progress)*
- [ ] Schedule box and scheduled messages section (edit, send now, reschedule) (M4) *(in progress)*
- [ ] Reply panel, reply with a quote, reply in another chat (M4) *(in progress)*
- [ ] Edit message panel, edit caption and media (M4) *(in progress)*
- [ ] Forward panel with options: hide sender, hide captions, change recipient (M4) *(in progress)*
- [ ] Share / forward box: several recipients, comment, search (M4) *(in progress)*
- [ ] Link preview panel: remove, above/below, larger/smaller media (M4)
- [ ] Cloud drafts per chat and per topic, synced (M4) *(in progress)*
- [ ] Draft stash (keep a draft aside and restore it) (M4)
- [ ] Mention autocomplete (M4)
- [ ] Hashtag autocomplete (M4)
- [ ] Bot command autocomplete (M10)
- [ ] Emoji suggestions by shortcode and keywords (M4)
- [ ] Sticker suggestions by emoji (M5)
- [ ] Character counter and limits for text and captions (M4)
- [ ] Emoji, sticker and GIF button with the tabbed panel (M5) *(in progress)*
- [ ] Attach: photos and videos (compressed or as files), files, albums (M4) *(in progress)*
- [ ] Send files box: grouping, spoiler, caption above or below, reorder (M4) *(in progress)*
- [ ] Paste images and files from the clipboard (M4) *(in progress)*
- [ ] Drag and drop files into the chat (compressed or as files zones) (M4) *(in progress)*
- [ ] Upload progress and cancel (M4) *(in progress)*
- [ ] Send a GIF with a caption (M5)
- [ ] Voice recording: hold, lock, cancel, preview, view once (M4) *(in progress)*
- [ ] Round video recording (M5)
- [ ] Send-as chooser (send as a channel) (M4)
- [ ] Slow mode countdown and send restrictions (M4)
- [ ] Join, mute/unmute, unblock and start-bot bars in place of the field (M4) *(in progress)*
- [ ] Search in chat bar with results navigation and sender filter (M3) *(in progress)*
- [ ] Delete messages: for me or for everyone, with confirmation (M4) *(in progress)*
- [ ] Report messages with reasons (M4)
- [ ] Pin message box (notify all members, pin for both) (M4)
- [ ] Create polls and quizzes (with media and links on options) (M4)
- [ ] Create and edit checklists, add and mark tasks (M4)
- [ ] Share a location and live location (location picker) (M5)
- [ ] Share a contact (M4)
- [ ] Send dice and other animated emoji games (M5)
- [ ] Auto-delete timer for new messages in a chat (M4)
- [ ] Spell checking in the field (M8)
- [ ] Compose with AI and AI tones (M13)
- [ ] Suggested posts to channels (with price and date) (M13)
- [ ] Paid messages to users and groups that charge Stars (M13)
- [ ] Rich messages: compose, edit, send (M13)

</details>

<details>
<summary><b>Media in messages</b>: 0 of 32 done, 19 in progress</summary>

- [ ] Photos with blurred placeholder and progressive load (M5) *(in progress)*
- [ ] Albums in mosaic layout (M5) *(in progress)*
- [ ] Videos: thumbnail, duration, inline autoplay without sound (M5) *(in progress)*
- [ ] Video covers and video timestamps (M5) *(in progress)*
- [ ] Streaming playback while downloading (M5)
- [ ] Files: icon, size, progress, download, open, show in folder (M5) *(in progress)*
- [ ] Music files with the player (M5) *(in progress)*
- [ ] Voice messages: waveform, play, seek, speed (M5) *(in progress)*
- [ ] Voice and video transcription (Premium) (M13)
- [ ] Round video messages (M5) *(in progress)*
- [ ] Stickers: WebP, animated TGS (Lottie), video WebM (M5) *(in progress)*
- [ ] GIFs (MPEG-4 animations) with autoplay (M5) *(in progress)*
- [ ] Dice and slot machine animations (M5) *(in progress)*
- [ ] Contacts in messages (M5) *(in progress)*
- [ ] Locations, venues and live locations with a map preview (M5) *(in progress)*
- [ ] Polls and quizzes: vote, retract, results, explanation (M5) *(in progress)*
- [ ] Checklists in messages (M5) *(in progress)*
- [ ] Games (M10)
- [ ] Invoices (M13)
- [ ] Story mentions and shared stories (M11)
- [ ] Gift, unique gift, Premium gift and giveaway service boxes (M13)
- [ ] Suggested profile photo and birthday suggestion boxes (M6)
- [ ] Media spoilers (M5) *(in progress)*
- [ ] Paid media (Stars) (M13)
- [ ] View-once photos, videos and voice messages (M5) *(in progress)*
- [ ] Similar channels block after joining (M6)
- [ ] Call messages (missed, outgoing, duration) (M12)
- [ ] TON transfer messages (M13)
- [ ] Theme documents in messages (M6)
- [ ] "Save as" for files and media (M5) *(in progress)*
- [ ] Auto-download by chat type, media type and size (M6)
- [ ] Download manager with the downloads list (M5) *(in progress)*

</details>

<details>
<summary><b>Media viewer and player</b>: 0 of 14 done, 5 in progress</summary>

- [ ] Full-window viewer for photos and videos with paging (M5) *(in progress)*
- [ ] Zoom, fit, rotate, fullscreen (M5) *(in progress)*
- [ ] Album thumbnails strip (M5)
- [ ] Caption, sender and date; forward, delete, save, copy (M5) *(in progress)*
- [ ] Video controls: play, seek, volume, speed, quality (M5) *(in progress)*
- [ ] Picture-in-picture (M5)
- [ ] Profile photo gallery in the viewer (M6)
- [ ] Text recognition in photos (select and copy) (M7)
- [ ] Sponsored messages in long videos (M5)
- [ ] Audio player bar with playlist panel (M5) *(in progress)*
- [ ] Repeat, shuffle, reverse order, speed, volume (M5)
- [ ] Floating round-video player (M5)
- [ ] Voice messages queue and listened marks (M5)
- [ ] OS media controls (macOS Now Playing, Windows SMTC, MPRIS) (M7)

</details>

<details>
<summary><b>Stickers, emoji and GIFs</b>: 0 of 10 done, 1 in progress</summary>

- [ ] Emoji panel: categories, recent, skin tones, search (M5) *(in progress)*
- [ ] Custom emoji packs in the emoji panel (Premium) (M5)
- [ ] Sticker panel: recent, favorites, installed sets, search, trending (M5)
- [ ] GIF panel: saved GIFs and search (M5)
- [ ] Sticker set box: preview, add, remove, share link (M5)
- [ ] Installed, archived and featured sets manager with reorder (M6)
- [ ] Emoji picker overlay on hold (variants) (M5)
- [ ] Emoji style sets (download and switch) (M6)
- [ ] Sticker preview on hold (M5)
- [ ] Create a sticker or sticker set (M13)

</details>

<details>
<summary><b>Info panel and profiles</b>: 0 of 42 done, 9 in progress</summary>

- [ ] Info panel in the right column, as a layer or in a separate window (M6) *(in progress)*
- [ ] User profile: photo, name, status, bio, usernames, phone, birthday (M6) *(in progress)*
- [ ] User profile: personal channel, notes, business hours and location (M6)
- [ ] Profile action buttons: message, mute, call, video call, gift, share (M6) *(in progress)*
- [ ] Shared media tabs: photos, videos, files, music, voice, links, GIFs (M6) *(in progress)*
- [ ] Shared media calendar and zoomable grid (M6)
- [ ] Common groups and similar channels (M6) *(in progress)*
- [ ] Group and channel profile: description, link, members, admins (M6) *(in progress)*
- [ ] Members list with search, roles, kick and restrict (M6)
- [ ] Bot profile: description, commands, privacy policy (M10)
- [ ] Profile photo gallery, set, change and delete own photos (M6) *(in progress)*
- [ ] Profile color, cover and emoji backgrounds (M13)
- [ ] Emoji status picker (M13)
- [ ] QR code for a profile or username (M6)
- [ ] Profile stories tab and story albums (M11)
- [ ] Gifts tab and gift collections (M13)
- [ ] Profile music (saved music) (M13)
- [ ] Polls list in a chat (M6)
- [ ] Short info card on userpic hover/click (M6)
- [ ] Birthday effect on profiles (M8)
- [ ] Create an avatar from emoji or sticker (M6)
- [ ] Edit contact: name, share phone, delete contact, add contact (M6)
- [ ] Block and unblock users; blocked list (M6)
- [ ] Report a chat or user with reasons (M6)
- [ ] Create a group (members, title, photo, auto-delete) (M6) *(in progress)*
- [ ] Create a channel (title, photo, description, type) (M6) *(in progress)*
- [ ] Add members, invite by link (M6)
- [ ] Edit group or channel: title, photo, description (M6)
- [ ] Group and channel type: public with usernames (reorder, collectible) or private (M6)
- [ ] History visibility for new members, discussion group link, signatures (M6)
- [ ] Permissions, slow mode, banned rights per member (M6)
- [ ] Admins with rights and custom titles (M6)
- [ ] Allowed reactions (M6)
- [ ] Invite links: create, edit (expiry, limit, approval, subscription), revoke (M6)
- [ ] Join requests: approve, dismiss (M6)
- [ ] Transfer ownership (M6)
- [ ] Channel and group colors and levels (M13)
- [ ] Member tags (M6)
- [ ] Communities: create, manage, add chats, pending requests (M6)
- [ ] Enable topics in a group (M9)
- [ ] Verify peers (verification bots) (M10)
- [ ] Replace boosts between chats (M13)

</details>

<details>
<summary><b>Settings</b>: 0 of 48 done, 13 in progress</summary>

- [ ] Main page with account card, sections and settings search (M6) *(in progress)*
- [ ] Settings search with keyboard navigation (M6)
- [ ] Own profile QR code: card, copy and share the link (M6) *(in progress)*
- [ ] My account: name, username, bio, birthday, phone number change (M6) *(in progress)*
- [ ] Personal channel (M6) *(in progress)*
- [ ] Notifications: private chats, groups, channels, stories, reactions (M6) *(in progress)*
- [ ] Notifications: sound, preview, sender, count badge, include muted (M6) *(in progress)*
- [ ] Notification exceptions per chat (M6)
- [ ] Custom notification sounds (upload, choose, per chat) (M6)
- [ ] Privacy: phone number, last seen (with read-time hiding), profile photo (M6) *(in progress)*
- [ ] Privacy: forwards, calls, voice messages, invites, birthday, gifts, bio (M6) *(in progress)*
- [ ] Privacy: messages from non-contacts (Premium or Stars price) (M13)
- [ ] Privacy rule exceptions (always/never share with) (M6) *(in progress)*
- [ ] Blocked users (M6) *(in progress)*
- [ ] Active sessions: list, terminate, terminate all, auto-terminate period (M6)
- [ ] Logged-in websites (Log in with Telegram) (M6)
- [ ] Unconfirmed new login review and denial (M6)
- [ ] Passkeys management (M7)
- [ ] Two-step verification: set, change, hint, recovery email, remove (M6)
- [ ] Login email change (M6)
- [ ] Global auto-delete timer (M6)
- [ ] Account self-destruction period (M6)
- [ ] Archive and mute new non-contacts (M6) *(in progress)*
- [ ] Suggest frequent contacts; delete cloud drafts; clear payment info (M6) *(in progress)*
- [ ] Sensitive content setting (M6)
- [ ] Chat settings: text and emoji options, large emoji, replace emoji (M6)
- [ ] Chat settings: stickers suggestions, loop animated stickers (M6)
- [ ] Chat settings: default reaction (quick reaction) (M6)
- [ ] Chat settings: send by Enter or Ctrl+Enter (M6) *(in progress)*
- [ ] Auto-download and autoplay settings (M6)
- [ ] Download folder and "ask where to save" (M6)
- [ ] Archive settings (keep unmuted chats archived, folders) (M6)
- [ ] Folders settings page (M6)
- [ ] Language: cloud language list, search, apply without restart (M6)
- [ ] Translation options and do-not-translate languages (M13)
- [ ] Interface scale (100–300 %, follows system) (M6)
- [ ] Connection type and proxies: SOCKS5, MTProto, HTTP, list, check, share links (M6)
- [ ] Data and storage: cache size limits, clear cache, storage usage (M6)
- [ ] Power saving and animation toggles (M8)
- [ ] System window frame option (M7)
- [ ] Tray, launch at startup, start minimized, close to tray (M7)
- [ ] Calls settings: devices, peer-to-peer, accept calls on this device (M12)
- [ ] Keyboard shortcuts page (M8)
- [ ] Experimental settings (M14)
- [ ] Local passcode and auto-lock timer (M7)
- [ ] Ask a question (support chat) and FAQ link (M6)
- [ ] Telegram features page link and About box (M6)
- [ ] Default interface for chats: one-column, two-column preferences (M2)

</details>

<details>
<summary><b>Themes and backgrounds</b>: 0 of 9 done, 1 in progress</summary>

- [ ] Day and night themes, night mode toggle in the main menu (M6) *(in progress)*
- [ ] Follow the system dark mode (M7)
- [ ] Accent colors for the built-in themes (M6)
- [ ] Cloud themes: list, apply, create from current, share links (M6)
- [ ] Import and apply `.tdesktop-theme` files (M6)
- [ ] Theme editor (palette editing, export) (M6)
- [ ] Chat backgrounds: gallery, solid, gradient, pattern, blur, own image (M6)
- [ ] Per-chat themes and per-chat wallpapers (M6)
- [ ] Theme preview before applying (M6)

</details>

<details>
<summary><b>Window and navigation</b>: 0 of 9 done, 3 in progress</summary>

- [ ] Main menu: account, My Profile, new group, new channel, contacts, calls, Saved Messages, settings (M2) *(in progress)*
- [ ] Main menu: accounts list, night mode switch, version (M2) *(in progress)*
- [ ] Section slide animations and back navigation (M8)
- [ ] Chat switcher (Ctrl+Tab) (M8)
- [ ] Media preview on hover (stickers, GIFs) (M5)
- [ ] Separate windows per chat, restored on restart (M8)
- [ ] Lock screen (passcode) (M7)
- [ ] Set up a login email when the server asks for it (M6)
- [ ] Toasts and layer boxes with the tdesktop look (M2) *(in progress)*

</details>

<details>
<summary><b>Notifications and OS integration</b>: 0 of 25 done, 1 in progress</summary>

- [ ] Native notifications on macOS, Windows and Linux (M7)
- [ ] Quick reply and mark as read from a notification (M7)
- [ ] Built-in notification popups (position, count) as an alternative (M7)
- [ ] Grouping, sound, do-not-disturb and focus handling (M7)
- [ ] Tray icon with menu and unread count (M7)
- [ ] Dock / taskbar unread badge (M7)
- [ ] macOS Dock menu and global menu bar (M7)
- [ ] macOS Touch Bar (M7)
- [ ] Windows taskbar thumbnail buttons and jump list (M7)
- [ ] Linux X11 and Wayland, including native title bar and portals (M7)
- [ ] Native file dialogs (M4)
- [ ] Launch at startup (M7)
- [ ] Screenshot protection for protected chats (M7)
- [ ] Sleep, wake and network change handling (M7)
- [ ] Current location for the location picker (M7)
- [ ] Outdated OS warning bar (M7)
- [ ] `tg://` links and `https://t.me` links open in rgram (M7)
- [ ] All internal link types: users, chats, messages, invites, stickers, proxies, themes, settings, bots, games, stories, boosts, gifts (M7)
- [ ] Phone number and bank card menus from text (M3) *(in progress)*
- [ ] Single instance: a second launch activates the running one and passes links (M7)
- [ ] Keyboard shortcuts as in tdesktop (quit, lock, search, folders, chat navigation) (M8)
- [ ] Accessibility: screen readers through AccessKit (M8)
- [ ] IME for CJK and other input methods (M8)
- [ ] HiDPI and fractional scaling (M8)
- [ ] Application update check and install for rgram releases (M8)

</details>

<details>
<summary><b>Secret chats and topics</b>: 0 of 10 done</summary>

- [ ] Start a secret chat with a user (M9)
- [ ] Accept an incoming secret chat; waiting and closed states (M9)
- [ ] Encryption key visualization (M9)
- [ ] Self-destruct timer and destroying messages (M9)
- [ ] Secret chat restrictions (no forwards, screenshot notices) (M9)
- [ ] Forum topic list with per-topic unread, mentions and reactions (M9)
- [ ] Topic tabs in the chat (subsection tabs) (M9)
- [ ] Create, edit (title, icon), close, reopen, hide, pin and delete topics (M9)
- [ ] Topic notifications and mute (M9)
- [ ] Bot forum topics in private chats (M10)

</details>

<details>
<summary><b>Bots</b>: 0 of 16 done</summary>

- [ ] Reply keyboards (resize, one-time, placeholder, remove) (M10)
- [ ] Inline keyboards: callback, URL, switch inline, copy, pay, login, web app, user/chat requests (M10)
- [ ] Callback answers as toast or alert (M10)
- [ ] Commands menu button and command list (M10)
- [ ] Inline mode: results panel with all layouts, switch to PM (M10)
- [ ] Recent inline bots (M10)
- [ ] Prepared inline messages from mini apps (M10)
- [ ] Mini apps (Web Apps) through the system webview (M10)
- [ ] Mini apps: fullscreen, permissions, device storage, downloads, location, sharing (M10)
- [ ] Attachment menu bots and main mini app buttons (M10)
- [ ] Login widget links (URL authorization box) (M10)
- [ ] Games (open game URL, scores) (M10)
- [ ] Add a bot to a group or channel with rights (M10)
- [ ] Managed bots: create and manage bots (M10)
- [ ] Bot verification badges (M10)
- [ ] Business chatbots connection (M13)

</details>

<details>
<summary><b>Stories</b>: 0 of 12 done</summary>

- [ ] Stories strip in the chat list, expanded and collapsed (M11)
- [ ] Hidden (archived) stories of contacts (M11)
- [ ] Viewer: progress, pause, tap navigation, sibling transitions (M11)
- [ ] Captions with entities, full caption view (M11)
- [ ] Story areas: locations, reactions, links, weather, channel posts (M11)
- [ ] Reactions and replies to stories (M11)
- [ ] Recent viewers of own stories with reactions (M11)
- [ ] Reposts and sharing stories (M11)
- [ ] Stealth mode (Premium) (M13)
- [ ] Own stories: archive, pin to profile, albums (M11)
- [ ] Post a story (photo or video, privacy, caption) (M11)
- [ ] Story statistics (M13)

</details>

<details>
<summary><b>Calls</b>: 0 of 16 done</summary>

- [ ] One-to-one voice call with an official client (M12)
- [ ] One-to-one video call, camera switch (M12)
- [ ] Incoming call window with accept and decline (M12)
- [ ] Encryption emoji fingerprint, signal bars (M12)
- [ ] Active call top bar, mute, devices menu (M12)
- [ ] Screen sharing with source picker (M12)
- [ ] Call rating after the call (M12)
- [ ] Recent calls list (M12)
- [ ] Video chats in groups and channels: join, leave, mute, raise hand (M12)
- [ ] Members list with speaking indicators and volume per member (M12)
- [ ] Video grid and screen sharing in group calls (M12)
- [ ] Join as a channel; scheduled video chats (M12)
- [ ] Recording, title, invite links, admin controls (M12)
- [ ] RTMP live streams and live streams in the viewer (M12)
- [ ] Conference calls with end-to-end encryption (M12)
- [ ] Messages and paid reactions inside group calls (M12)

</details>

<details>
<summary><b>Payments, Stars and gifts</b>: 0 of 13 done</summary>

- [ ] Checkout panel for invoices: items, shipping, tips, total (M13)
- [ ] Card entry with tokenization (Stripe, SmartGlocal) and saved credentials (M13)
- [ ] 3-D Secure and provider pages (M13)
- [ ] Receipts (M13)
- [ ] Stars balance, top-up, transactions history (M13)
- [ ] Stars invoices, subscriptions and their management (M13)
- [ ] Gift Stars to a user (M13)
- [ ] Send a gift (with message, anonymously) (M13)
- [ ] Received gifts: show on profile, convert, upgrade, transfer, wear (M13)
- [ ] Gift resale market with filters (M13)
- [ ] Gift auctions (M13)
- [ ] Gift crafting (M13)
- [ ] Collectible usernames and numbers info (M13)

</details>

<details>
<summary><b>Premium and Business</b>: 0 of 11 done</summary>

- [ ] Premium page with features and subscribe (M13)
- [ ] Limit-reached boxes with upgrade (M13)
- [ ] Feature previews (M13)
- [ ] Gift Premium, gift codes, giveaways (M13)
- [ ] Boosts: boost a chat, boost links, boost list (M13)
- [ ] Business hub page (M13)
- [ ] Away and greeting messages (M13)
- [ ] Quick replies with shortcuts and editor (M13)
- [ ] Working hours and location (M13)
- [ ] Chat intro and business chat links (M13)
- [ ] Chatbots for business (M13)

</details>

<details>
<summary><b>Instant View, rich messages and export</b>: 0 of 9 done</summary>

- [ ] Instant View pages in a separate window (M13)
- [ ] Instant View search and zoom (M13)
- [ ] Rich messages (articles with blocks, media, formulas) (M13)
- [ ] Rich message editor: blocks, lists, tables, media, formulas (M13)
- [ ] Export a rich message to HTML (M13)
- [ ] Export Telegram data: settings (types, size limits, period, format) (M13)
- [ ] Export writers: HTML, JSON, HTML and JSON (M13)
- [ ] Export progress with the top bar and cancel (M13)
- [ ] Export a single chat's history (M13)

</details>

<details>
<summary><b>Editor</b>: 0 of 5 done</summary>

- [ ] Photo editor: crop, rotate, flip (M13)
- [ ] Paint: brushes, colors, undo (M13)
- [ ] Text, stickers and shapes on photos (M13)
- [ ] Video editor: trim, quality, audio track (M13)
- [ ] Draw to reply (M13)

</details>

<details>
<summary><b>Statistics and earnings</b>: 0 of 8 done</summary>

- [ ] Channel statistics with charts (growth, followers, views, sources, languages) (M13)
- [ ] Group statistics (M13)
- [ ] Message and story statistics, public forwards (M13)
- [ ] Chart interactions: zoom, filters, point details (M13)
- [ ] Export statistics (XLSX) (M13)
- [ ] Channel revenue in TON and Stars with withdrawal (M13)
- [ ] Bot earnings and the Stars affiliate program (M13)
- [ ] Giveaways: create and view (M13)

</details>

<details>
<summary><b>Passport, AI and translation</b>: 0 of 7 done</summary>

- [ ] Telegram Passport authorization forms (M13)
- [ ] Identity documents, scans, translations, address (M13)
- [ ] Phone and email verification inside Passport (M13)
- [ ] Translate a message or selection (M13)
- [ ] Auto-translate in channels (M13)
- [ ] AI tones management (M13)
- [ ] Create with AI for rich messages (M13)

</details>

<details>
<summary><b>TON wallet</b>: 0 of 8 done</summary>

- [ ] Wallet panel: card, balance, history (M13)
- [ ] Create, import, back up (secret phrase) and delete a wallet (M13)
- [ ] Send and receive TON, gasless transfers (M13)
- [ ] Collectibles (NFTs) and moving gifts (M13)
- [ ] Card customization (stickers, backgrounds) (M13)
- [ ] Key protection with the OS keychain (M13)
- [ ] TON Connect: connect to dApps, approve requests (M13)
- [ ] TON transfers between users in chats (M13)

</details>

<details>
<summary><b>Other</b>: 0 of 3 done</summary>

- [ ] Emoji stake (dice bets in Stars) (M13)
- [ ] About Cocoon box (M13)
- [ ] Fact checks on channel posts (view and edit) (M13)

</details>

<!-- todo:end -->

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

On macOS the build needs Xcode's Metal Toolchain (`scripts/check-metal.sh`
checks it; install it with `xcodebuild -downloadComponent MetalToolchain`).
`cargo run` and `cargo test` find `libtdjson` on their own. `RGRAM_LOG` sets the
log filter (default `info`), for example `RGRAM_LOG=tg=debug`.

On macOS, `bash scripts/build-macos-app.sh` builds `target/rgram.app`. First
build TDLib and the static FFmpeg with `scripts/build-ffmpeg.sh`. The app reads
credentials from this checkout's `.env`, or from the `rgram.api_id` and
`rgram.api_hash` Keychain entries described in `scripts/run-live.sh`. It does
not contain credentials. Move an existing app out of `target` before rebuilding.
The app is locally ad-hoc signed, not notarized for distribution.

`tools/style-gen` regenerates `crates/style` from the Telegram Desktop checkout
next to this repository (or the path given as its argument):

```bash
cargo run -p style-gen
```

Its output is unformatted; run `cargo fmt -p style` before reading the diff.
A new style root also needs its `pub mod` line in `crates/style/src/lib.rs`.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE) and [NOTICE.md](NOTICE.md) for the
material derived from Telegram Desktop and other projects.
