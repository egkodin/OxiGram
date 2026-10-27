# Progress

## M4: Compose and sending (in progress, offline: the owner is away from the Mac)

Built on the offline pattern (model, store, snapshots, tests) until the
owner is back to sign in; every M4 row stays `in progress` until it runs
live.

### Done (code, checks green)

- `ui::TextArea`: lib_ui's multi-line `InputField` on a shared
  `text_buffer` (selection, graphemes, words, paragraphs, UTF-16 for the
  IME). Wraps, grows from 28 px to `historyComposeFieldMaxHeight` and then
  scrolls with the cursor kept in view; Up/Down keep their column, Home/End
  go by visual row; the placeholder slides right and fades as typing
  starts. Enter or Ctrl/Cmd+Enter submits by `SubmitKey`, the other one and
  Shift+Enter break the line, Ctrl/Cmd+Shift+Enter always submits.
- The compose bar under the history (`history::compose`): attach, field,
  emoji, and send, which shows the microphone while the field is empty;
  "Broadcast a message..." in channels. Hidden in the pinned section and
  while searching.
- `model::send`: `Session::send_text`, trimmed and split at 4096 UTF-16
  units on line breaks, then spaces, sent in order. TDLib's pending, sent
  and failed updates were already applied by the history. If TDLib refuses
  the request, the text goes back into the field when it is still empty.
- `settings::SendBy` (Enter or Ctrl+Enter), saved with the other settings.
- The switch (`settings_box`): the main menu's Settings opens "Messages"
  with "Send with Enter" and "Send with Cmd+Enter" (Ctrl+Enter off
  macOS) as radios (`ui::Checkbox::radio`), and Log Out; open fields
  follow a change at once. Snapshots 143-150.
- `model::rights` and `HistoryView::write_bar`: in tdesktop's order, Unblock
  (blocked user), Restart bot (blocked bot, unblocks then sends /start),
  Join channel/group (not a member), Mute/Unmute (a channel the user cannot
  post in), Start (a bot with no messages), else the field, or why the
  admins closed it (restricted until a date, by admins, for all). Member
  status, chat permissions and the block list follow their updates.
  Snapshots 85-89.
- Drafts (`history::drafts`, `Session::set_draft`): the chat's draft fills
  the field on open; typing saves it a second after the last key (at most
  five after the first unsaved one) and on leaving the chat, only when the
  text changed; a draft changed on another device replaces the field unless
  something typed here waits to be saved. Sending drops it (`clear_draft`).
- The bar over the field (`history::field_bar`, `history::forwarding`):
  reply (menu, Ctrl/Cmd+Up and Down through the history), edit (menu, Up
  in an empty field; plain text only; Escape gives back what the field
  held), forward (menu, Forward selected, the selection bar's Forward N)
  through "Choose recipient..." (`recipients_box`: Saved Messages, then
  the main list, writable chats only, name search). Sending a forward sends
  the typed text first. Escape cancels the bar after the selection and
  search. The selection bar also has Delete N. Reply previews cover
  spoilers. `peer_list` holds the rows the contacts and recipients boxes
  share. Snapshots 90-109.
- Emoji panel (`emoji_panel`, `TabbedPanel` + `EmojiListWidget`): hovering
  the emoji button shows it over the field at the right, clicking the
  button toggles it, leaving both hides it after 200 ms. Recent (used
  emoji by rating, then tdesktop's defaults, 54 at most, kept in
  `settings.json`), then the 7 categories with headers, 9 columns of 37x35;
  footer icons light the section at the top and scroll to theirs. Clicking
  an emoji types it at the cursor. The categories are generated from
  `lib_ui/emoji.txt` by style-gen (`crates/style/src/emoji.rs`). The ranking
  is shared with recent hashtags (`ranked`). Snapshots 110-116.
- Attachments: the attach button opens the system picker; pasting files or
  an image (saved to a temp file) and dropping files on the chat
  (`drop_area`: one zone for files, or for images "without compression"
  over "in a quick way") open the send box (`send_files_box`): title by
  kind and count, file rows with photo thumbnails and sizes, Group items
  and Send as documents, the caption taken from the field and given back
  on Cancel/Escape. `model::send_files` groups like tdesktop (albums of
  photos and videos or of files, 10 at most, caption on the last group)
  and sends with `sendMessageAlbum`/`sendMessage`, replying to the bar's
  message. While a file goes up its bubble shows the file row with the
  progress arc, "1.1 / 3.3 MB" from `updateFile`, and the cross that
  cancels (deletes the pending message). Snapshots 117-124 (harness drops
  files through gpui's `FileDropEvent`); the harness's scenarios moved to
  `scenarios.rs`.
- Voice messages (`history::voice`, new `media` crate, ADR 0006): holding
  the microphone 120 ms starts recording; the bar replaces the field (the
  blinking dot, the time with tenths, "Release outside this field to
  cancel"), the round button over the send button grows with the level and
  turns red outside the field, the lock rises above it. Releasing in the
  field sends, outside cancels; dragging up to the lock keeps recording,
  then the button sends and Cancel discards. A quick click shows the hold
  tip, a microphone that does not open shows tdesktop's problem toast.
  `media` reads the default input with cpal, mixes to mono, resamples to
  48 kHz and encodes on a worker like tdesktop (400 ms skipped, 300 ms
  fades, under a second dropped, Opus 32 kbit/s in OGG, the 5-bit
  waveform); `Session::send_voice` sends it as `inputMessageVoiceNote`.
  Snapshots 125-128 record preset samples (`preset_voice_samples`), never
  the microphone.
- Scheduled and silent sending (`history::send_menu`, `schedule_box`): a
  right click on send (in the field and in the files box) opens
  "Send without sound", "Schedule message" and, to a user whose last seen
  time shows, "Send when online"; Saved Messages only "Set a reminder".
  The schedule box shows the day, which opens the calendar over the box,
  "at" and the time ("H:MM", the colon typed for you), from ten seconds
  to a year ahead, ten minutes on by default; the bell makes it silent,
  as Ctrl/Cmd held on Schedule does, and the dots offer "Send when
  online". `model::send::SendOptions` goes to every send request;
  scheduled messages (`Message::schedule`) stay out of the history.
  Layers now stack (`Layers::show_above`), so the calendar returns to the
  box and the box to the files box.
- `ui::InputField` (single line) edits a `text_buffer::TextBuffer` like
  the multi-line field, so cursor, word, selection and UTF-16 logic live
  once; `input` is split into `mod.rs`, `keys.rs` and `element.rs` as
  `text_area` is. Every snapshot that types into a single-line field is
  unchanged.
- Scheduled messages section (`history::scheduled_section`,
  `model::scheduled`): a clock button by the field while the chat has
  scheduled messages opens "Scheduled Messages" ("Reminders" in Saved
  Messages), each day under "Scheduled for {date}" or "Scheduled until
  online"; the menu adds Send Now (asks first) and Reschedule. Snapshots
  139-142.
- Snapshots 77-84: typing, line breaks, growth, scrolling at the maximum,
  a refused send giving the text back, clearing. The snapshot harness now
  types keys (`Input`). History click targets moved up by the bar's 46 px;
  the calendar steps open from Hiking Club's date badge.

### Half done (shortcuts in code)

- Scheduling: the section has no field and no editing (Edit is not in
  its menu); its list loads on open and after Reschedule, then follows
  updates; scheduled silent messages show no silent mark; no repeat
  period (Premium), no "Send without
  sound" menu on the box's Schedule button, no send menu on a locked voice
  recording; the bell is a still speaker icon with no tooltip; Tab into
  the day does not open the calendar; the time field's cursor jumps to the
  end after each edit.
- Voice: no preview after locking (stop, play, trim), pause, view once,
  round videos, "recording voice" chat action, Enter/Escape while locked,
  or the hold tip as a tooltip (it is a toast); the blobs are plain circles
  and the padlock does not open or tilt; recordings stay in the temp folder;
  the resampler is linear.
- Attachments: photos and videos show as file rows in the box and while
  uploading (large previews, album grids and media bubbles come with M5);
  the arc stands still; videos go without duration, size or thumbnail, and
  music and GIFs as plain files; kinds are told by extension; folders are
  skipped, not zipped; no spoiler, caption above, reorder or per-file
  remove in the box; pasted images stay in the temp folder.
- Emoji panel: no Stickers/GIFs tabs (M5), search, skin tones, custom
  emoji, show/hide animation, scrollbar, or third-column mode in wide
  windows; Recent refreshes when the panel opens, not while it is shown.
- Edit keeps no formatting, so formatted messages and captions are not
  offered for editing; no quotes, reply in another chat, forward options,
  several recipients or global search in the box yet.
- Topic and thread drafts are saved but not loaded back (forum topics are
  not in the model yet); drafts keep no formatting or reply yet.
- The bars read only basic-message rights; media rights come with
  attachments; slow mode and the "only premium may write" state are not
  shown yet.
- Settings opens a box (Messages: Enter or Ctrl/Cmd+Enter, and Log Out)
  until the settings screen (M7); its radios have no toggle animation and
  use the box checkbox's text offset, not `settingsCheckbox`'s.

### Live test list (M4, after M1-M3)

Sending only in Saved Messages:
1. Type, wrap a long line, Shift+Enter, grow to the maximum and scroll;
   Up/Down/Home/End, selection by mouse and keys, copy, cut, paste with line
   breaks; IME composition (e.g. Japanese input).
2. Enter sends; with `send_by` set to Ctrl+Enter in `settings.json`, Enter
   breaks the line and Ctrl/Cmd+Enter sends.
3. The message appears with the clock, then the check; the list scrolls to
   it. A text over 4096 characters arrives as several messages in order.
4. The send button turns into the microphone when the field empties.
5. In a channel the owner does not admin: the Mute/Unmute bar instead of
   the field; nothing is sent. Pressing Mute only in a channel the owner
   agrees to mute (it changes the account).
6. A public channel or group the owner is not in (open it from search): the
   Join bar; do not press it without the owner's say-so. A blocked user or
   bot shows Unblock / Restart bot; a bot never started shows Start (press
   only with the owner's say-so). A group where the owner is restricted
   shows the restriction text.
7. Drafts in Saved Messages: type, switch chats, come back: the text is
   there and the list shows "Draft:". Type on the phone in Saved Messages
   while rgram has it open: the field follows. Send: the draft goes.
8. In Saved Messages: reply to a message from the menu and with Cmd+Up,
   send; edit the last message with Up, change it, Enter; Escape while
   editing gives the field back. Forward a message from another chat to
   Saved Messages (menu and the selection bar), with and without a comment.
   Delete N in the selection bar, only in Saved Messages.
9. Emoji panel: hover the emoji button, the panel appears; move into it and
   out, it hides; click an emoji, it is typed at the cursor; send it in
   Saved Messages; reopen: it leads Recent. Footer icons jump to their
   category and light up while scrolling. Glyphs render in color.
10. Attachments, only in Saved Messages: the attach button picks a photo and
    a text file; the box shows both, Send arrives as two messages (or an
    album of files with Group items). Paste a copied image and a file
    copied in Finder. Drag a photo onto each zone: compressed arrives as a
    photo, the top zone as a file. Send a large file (tens of MB): the
    bubble's arc and "x / y MB" advance; press the cross: the message goes
    and the upload stops. Type a caption in the field first: it moves into
    the box and back on Cancel.
11. Voice in Saved Messages (macOS asks for microphone access first): click
    the microphone, the hold tip shows; hold and speak, the time runs and
    the button pulses; release on the field: it arrives as a voice message
    with a waveform and plays on the phone. Hold, drag left out of the
    field, release: nothing is sent. Hold, drag up past the lock, let go:
    still recording; Send sends, Cancel discards. Try a 44.1 kHz input
    (e.g. a USB headset) for the resampler.

12. Scheduling, in Saved Messages: with text in the field, right click
    send: only "Set a reminder"; pick a day in the calendar and a time a
    few minutes ahead, Schedule: the field clears, nothing shows in the
    history, and the reminder arrives at that time on the phone too. To
    test "Send without sound" and "Send when online", use the test DC
    (`use_test_dc`), not a real chat.

13. Scheduled section, in Saved Messages: set two reminders; the clock
    button appears by the field; click it: "Reminders" lists both under
    "Scheduled for" days. Right click one, Reschedule to another day: it
    moves. Right click, Send Now, confirm: it leaves the list and shows in
    the chat. Delete the last one: the section closes and the button goes.

14. Send by Cmd+Enter: with Saved Messages open, main menu, Settings, pick
    "Send with Cmd+Enter", Close. Enter breaks the line; Cmd+Enter sends.
    Quit and start again: the choice stays. Set it back to Enter.

## M3: Chat history (code complete; stopped here before M4 at the owner's request; waits on a live run)

### Done (code, checks green)

- `features::history`: the virtualized, bottom-aligned history with
  tdesktop's bubbles, tails, grouping, userpics and sender colors, date
  badges, service messages, outgoing states, edited marks, views and
  signatures; reply and forward headers; entities (bold, italic, underline,
  strikethrough, code, pre, block quotes, links, hidden links with a
  confirmation, mentions, hashtags, spoilers); loading pages up and down by
  splicing the list (`sync`), so the scroll anchor stays put.
- Opening at the first unread message under the unread bar, jumps to a
  replied message and back, the scroll-down button with the unread counter
  (`jump`, `unread`).
- Text selection inside a message, message selection by dragging or
  Ctrl/Cmd+click, Ctrl/Cmd+C copy, Escape and the selection bar's Cancel
  (`selection`).
- `history-perf` example and `scripts/history-perf.sh`: the M3 frame-budget
  and no-jump check on 10,000 generated messages (`docs/perf.md`). It found a
  crash on the first wheel scroll (the scroll handler read the list while the
  list held its own state); fixed by deferring the handler's work.
- The message context menu (copy text or selection, copy link or message
  link, delete with tdesktop's revoke rules, select, select up to here) and
  the chat menu behind the top bar's three dots (`menu`, `delete`,
  `peer_menu`).
- The pinned message bar (`pinned`): pins from `searchChatMessages`, kept
  by `updateMessageIsPinned`; the bar follows the view bottom, each click
  jumps and steps to the older pin, wraps after the oldest; close offers
  unpin when allowed, else hides. Jumps shade the target message.
- Link previews (`web_page`): site, title and description above or below
  the text, the time on the box's last line; the unsupported message notice
  as tdesktop words it (`dialogs::unsupported_text`).
- Corner buttons for unread mentions and reactions over the down button
  (`unread`); a click jumps to the oldest unread one and reads it.
- The calendar a date badge opens (`calendar`); a chosen day jumps to its
  first message. Snapshot fixtures now count from noon today, so day badges
  stay put whatever time the snapshots run.
- The read date of an outgoing message at the top of its menu in private
  chats (`read_date`).
- "via @bot" after the sender's name; Ctrl+C refuses protected messages
  with tdesktop's toast; the floating date while scrolling (`scroll_date`);
  who saw an outgoing group message in its menu (`read_date::seen`); the
  pinned messages section (`pinned_section`), a mode of the history view;
  search in a chat (`chat_search`).
  The snapshot runner gained wheel-scroll steps (57-59).
- Replies to messages in other chats and to stories (`headers::external_reply`):
  the origin's name and color, the quote or the media kind without its caption.
- Histories are keyed by chat and topic (`model::thread::Key`): a post's
  comments in its discussion group, a Saved Messages topic. The comments
  button under channel posts (`comments`) opens the thread over the channel
  (`thread_section`); the main view keeps the chats a thread came from, so
  Back returns to the same place.
- Reactions under messages (`reactions`): counts or, when few and all
  named, userpics; the chosen one filled; the time keeps its exact room on
  the last row. The comments and reactions share `userpics_row`.
- Who reacted (`who_reacted`, `reactions_box`): the group or channel
  message menu's first line follows tdesktop's `refreshText` ("Maria",
  "2/5 Reacted", "63 Reactions", "3 Seen") and opens the reactions list:
  "All" and a tab per reaction, the most used first, pages of 50 from
  `getMessageAddedReactions`. Checked once with a temporary fixture (the
  runner gets no TDLib answers), then restored.
- Phone numbers, bank cards and dates in text (`link_menus`, `dates`):
  phones and cards open tdesktop's menus with TDLib's lookups; dates read
  in the reader's words (relative or absolute) and open "Copy Date". The
  history formats dates once at `sync`, so layout, selection and copy see
  one text. The snapshot runner now draws between press and release, as
  gpui's text links need; before, link clicks never fired in it.
- Code block header (`body::pre_header`): tdesktop's band with the
  language or "copy" and the copy icon; a click copies the code with the
  "Code copied" toast. Syntax highlighting is not done (see open questions).
- Sponsored messages (`model::sponsored`, `history::sponsored`): a broadcast
  channel's first text ad shows after its last post once the newest posts
  are loaded, as tdesktop's "Ad" link box with "what's this?" and the
  button; the pill beside it hides (Premium) or explains that Premium hides
  ads, and opens the ad menu (advertiser info, about, report, hide). The
  report flow follows `ShowReportSponsoredBox` with TDLib's options.
  Checked once with a temporary fixture ad, then restored.
- Boxes no longer grow past the window: `BoxFrame` caps itself at the
  window height inside the style's margin and scrolls its rows
  (`updateMaxRealHeight`). The contacts box is 6 px shorter for it.
- The contact status bar (`model::action_bar`, `history::contact_status`):
  TDLib's chat action bar under the pinned bar, with tdesktop's buttons
  and confirmations (block, report and leave, share the phone number, the
  join-request note); close removes the bar. Users now carry their phone
  number as a `Secret`, for the share confirmation.
- Saved Messages tags in the chat search (`model::saved_tags`,
  `history::search_tags`): the tag pills under the field, search by the
  picked tag, and the lock pill leading to what tags are without Premium.
- Pull to next channel (`history::pull_next`): overscrolling the end of a
  channel shows the next unread channel and opens it on release.

### Verified (macOS arm64, this machine)

- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` green.
- Snapshots 41-76 compared by eye with tdesktop's style and layout code.
- `scripts/history-perf.sh`, three runs at 120 Hz: frame build p99 5.44 ms,
  no dropped refreshes over 60 s of scrolling, and adding 50 older messages
  near the top changes no history pixel. Rerun once after the pull handler
  (160 Hz display): frame build p99 2.93 ms, max 7.93 ms, no late frames.

### Not verified

- Everything live (API credentials still missing): real pages from TDLib,
  opening time from the TDLib cache, marking messages read.
- Clipboard contents after copy: the checks do not write the user's
  clipboard.

### Half done

- M3 rows: none todo; all in progress until the live run below. No row in
  `docs/parity.md` is `done`: the unsupported-message row went back to
  in progress, since it never ran on a live account.
- M2 rows still todo: chat preview (peek), search in one chat by sender
  (needs the member list, M6), default one- or two-column interface.
- Forward and Delete on the selection bar wait for M4 (sending) and the
  message context menu.

### Next steps

1. With API credentials: the live run below, updating each parity row's
   Verified column and marking `done` what passes.
2. Measure on the live account: cold start to the chat list, idle RSS,
   opening a chat from the TDLib cache (`docs/perf.md`).
3. M4 (compose and sending), only when the owner says to start it.

### Live run setup

- Credentials: git-ignored `.env` at the project root (mode 600), with a
  copy in the login Keychain (`rgram.api_id`, `rgram.api_hash`).
- Start with `scripts/run-live.sh` (release build, production DC, log in
  `target/run-live.log`). It sets `RGRAM_WITHHOLD_VIEWS=1`, so browsing
  real chats reads nothing; reading is checked in Saved Messages only, with
  `RGRAM_WITHHOLD_VIEWS=0`.
- The owner signs in at the Mac (phone, code, 2FA password, or QR); the
  codes are never asked for in chat. Run M1-M3 first, then M4.

### Live run order (once credentials are set)

M1, sign-in (the owner enters codes and the 2FA password):
1. Start without credentials: the startup error; then with them.
2. Welcome screen, language offer; phone entry with the country from
   `getCountryCode`, picker search and the format mask.
3. QR login: code shown, refresh on expiry, finish after scanning on a phone.
4. Log out, then phone login: code to the app, resend countdown and the next
   method, wrong-code error, auto-submit at full length.
5. 2FA password with its hint (recovery and account reset only if the owner
   wants them exercised).
6. Restart: stays signed in. Log out confirmation at the end of the session.
7. Error paths that can be produced safely: invalid number, network off.
   Sign-up, terms, Fragment, login email and FLOOD_WAIT stay unverified
   unless they happen naturally.

M2, chat list:
1. Cold start to the list: server order, rows (userpic, name, time, preview
   with sender and entities, media previews, badges, unread and mention
   counters, read checks, drafts, online dots, mute and pin icons).
2. Live updates from a second device: new message, edit, delete, read state,
   typing, reordering.
3. Connecting / Updating / Waiting for network by toggling the network.
4. Folders: tabs on top and in the sidebar, counts, tab menu, reorder.
5. Archive: open, collapse, hide into the main menu; archive and unarchive.
6. Chat menus: pin, unpin, drag order, mute, mark read and unread, add to
   folder (destructive items only on a test chat the owner names).
7. Search: chats, contacts, global, messages with filters, hashtags, recent
   searches, top peers, suggestions; contacts list; empty states.
8. Layouts: one, two, three columns, resize, narrow mode; main menu, night
   mode, toasts.

M3, history:
1. Open chats of each kind: private, group, channel, Saved Messages; title
   bar status and typing; opening at the first unread under the bar.
2. Scroll up and down through real pages: no jumps; date badges, the
   floating date, the scroll-down button with its counter, mention and
   reaction jumps, the calendar.
3. Message rendering on real content: bubbles, names and titles, service
   messages, outgoing states, edited, views, signatures, forwards, replies
   (also cross-chat and story replies), via-bot, every entity kind,
   spoilers, quotes, dates, link previews, reactions and who reacted.
4. Selection and copy (text and messages), the context menu, link, phone
   and card menus, read dates and who read.
5. Pinned bar, pinned section, unpin (on Saved Messages).
6. Channel comments and the replies section; sponsored messages in a large
   channel; protected content in a chat that forbids saving.
7. Chat search with results stepping; Saved Messages tags search.
8. Contact status bar, from a chat with someone not in contacts (actions
   that change the account only with the owner's say-so).
9. Pull to next channel with a trackpad, with and without unread channels.
10. Perf: opening a chat from the TDLib cache.

### Open questions for the owner

- API credentials: provided; in `.env` and the Keychain (see Live run setup).
- Syntax highlighting in code blocks: tdesktop uses libprisma through
  lib_spellcheck; both submodule folders are empty in the read-only
  checkout (lib_ui is there). Options: add the
  `tdesktop` submodules locally, or pick a Rust highlighter (adds a
  dependency) with tdesktop's token colors.
- Permission to download Telegram Desktop for screenshot comparison.
- The unsupported message notice links to desktop.telegram.org as tdesktop
  does; an rgram releases page could replace it once releases exist.

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
- Folder tabs reorder by dragging (`reorderChatFolders`); All chats stays
  first.
- Toasts (`features::toast`, shown through `Layers::show_toast`), first
  used for the archive's "moved to the main menu" note.
- The folders sidebar (`dialogs::sidebar`), tdesktop's default when the
  window has room, with the main menu button; the narrow list shows a
  search button beside it. `settings::FoldersPlacement` keeps tabs on top.
- Recently searched hashtags over the search results (`hashtags`,
  `dialogs::hashtag_results`).
- Fixed two click-through bugs found in snapshots: a popup menu item click
  also clicked the row under it, and a right click on a layer (main menu,
  boxes) reached the list behind. `PopupMenu` and `Layers` now occlude.
- Fixed an M1 bug: the log-out box had no cancel button, so Escape or a click
  outside logged out. Destructive boxes now go through `confirm_attention`
  (regression test `destructive_boxes_cancel_on_escape`).

### Verified (macOS arm64, this machine)

- `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`,
  `cargo test` green.
- Snapshots 11-40 (`cargo run -p rgram --example snapshots`): chat list with
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

1. Remaining M2 rows: chat preview (peek), search in one chat and by
   sender wait for the chat view (M3) as their entry point.
2. M3: the chat view.

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
