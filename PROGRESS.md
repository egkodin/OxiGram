# Progress

## Disk budget (owner's standing rule)

- The git repository (.git plus tracked files) stays under 50 MB; never
  commit build output, snapshots, logs, caches, media, FFmpeg or TDLib
  builds, or .DS_Store (`.gitignore` covers them).
- target/ stays under 5 GB: `du -sh target` at the start of each session
  and after each milestone; over budget, `cargo clean` and say so. One
  target/ only; no extra profiles or feature combinations.
- Dev builds keep line tables only, dependencies no debug info, and no
  incremental cache for `td-api`, `tg` and `style` (`Cargo.toml`); a full
  set of clippy, tests and one snapshot run measured 4.8 GB from clean
  (2026-11-03). Turn `debug = true` on only to debug, then back off.
- Snapshots, soak frames and live-run logs stay under target/ and are
  overwritten; delete an old app bundle before building a new one.
- TDLib and FFmpeg builds live in ~/.cache/rgram, one version each.
- Every milestone report gives the sizes of target/ and .git.

## M5: Media (in progress, started while the live runs of M1-M4 go on)

### Done (code, checks green)

- Photos in the history (`history/photo.rs`): tdesktop's sizes, the
  minithumbnail until the download completes, the caption below, the time
  on its pill over the picture. Snapshots 151-153.
- Files in the history (`history/file_row.rs`, `documents.rs`): download,
  cancel and open from the circle, the size or progress beside the time,
  "Show in Finder" in the menu; `launch.rs` asks before opening risky
  types, as tdesktop's `LaunchWithWarning`. Uploads share the row.
  Snapshots 154-156.
- Videos and GIFs in the history (`history/video.rs`): TDLib's still sized
  as a photo, the middle button downloads, cancels with the arc, then
  plays in the system player; length and size on the corner pill.
  Snapshots 157-159.
- Albums (`history/album.rs`, `grouped_layout.rs`): tdesktop's mosaic,
  one message per album with the one caption; media alone has no bubble
  or tail. Snapshots 160-162.
- Voice messages (`history/voice_note.rs`, `waveform.rs`, `media::decode`,
  `media::Player`): the waveform, download, play, pause and seek on the
  default output, the listened dot. Snapshots 163-164.
- Contacts in messages (`history/contact.rs`): tdesktop's card with the
  userpic, name and phone; MESSAGE opens the account's chat, ADD adds
  it under the card's name. Snapshots 165-166.
- "Save As..." in the menu of a downloaded photo, video, voice or file
  (`history/save_as.rs`): the system save box in Downloads, then a copy.
  Snapshot 156.
- Locations and venues (`history/location.rs`, `model::location`):
  TDLib's map picture with the pin, a venue's title and address, a click
  opens Maps. Snapshots 167-169.
- Media spoilers (`history/spoiler.rs`): the preview under dots until a
  click reveals it. Snapshots 170-172.
- Stickers (`history/sticker.rs`, `model::sticker`): still WebP stickers
  alone at tdesktop's size with the time on the service pill; animated
  ones show their still. Snapshots 173-174.
- Round video messages (`history/video.rs`): the circle with its still,
  download and play buttons, the length and the unwatched dot. Snapshots
  175-176.
- Polls and quizzes (`history/poll.rs`, `model::poll`): the question and
  its kind, radios or checks to vote (several answers wait for Vote),
  results with percents, counts and bars, quiz marks and the explanation;
  Retract vote and Stop poll in the menu. Snapshots 177-179.
- Large emoji and dice (`history/large_emoji.rs`): one to three emoji
  alone show big without a bubble; animated emoji and dice show their
  sticker's still at 112 px. Snapshots 180-181.
- Checklists (`history/checklist.rs`, `model::checklist`): tasks with
  their checks and completers, marked by a click; the status line.
  Snapshots 182-183.
- Media viewer for photos (`history/media_view.rs`, `Layers::show_full`):
  a click on a photo opens it over the window, fitted when bigger, else at
  its own size; arrows and Left/Right page through the loaded photos;
  Cmd+=/Cmd+-/Cmd+0 and Cmd+wheel zoom by tdesktop's levels, a drag moves
  it; "Photo 2 of 3", the sender and the date bottom left, the caption
  box, the save button; Esc or a click beside it closes. Snapshots 184-188.
- View-once and timed media (`history/view_once.rs`, `model::SelfDestruct`,
  `Service::Expired`): photos and videos under dots with the fire and the
  "1" badge; a click opens a view-once photo alone in the viewer and
  reports it opened; expired media show as "Expired photo" and the like.
  Snapshots 189-194.
- Video covers and timestamps (`history/video.rs`, `Media::start_timestamp`):
  a cover the sender picked replaces TDLib's still; a start time shows as
  the line along the bottom. Snapshots 195-196.
- Minimal static FFmpeg (`scripts/build-ffmpeg.sh`, `ffmpeg/SOURCES`, ADR
  0006 "Revisited in M5"): FFmpeg 9.0.2 and dav1d 1.5.4 from checked
  tarballs, only rgram's decoders, demuxers and parsers, VideoToolbox /
  VAAPI / D3D11VA. Built and linked here: 34 MB of archives, 6.2 MB in a
  stripped probe that decodes H.264.
- Download bar (`dialogs/download_bar.rs`, `model::downloads`): downloads
  from a file's or video's button go through TDLib's download list
  (`addFileToDownloads`); the bar under the chat list shows the ring, the
  file's name or "3 files", "x / y MB", then "View in chat" / "View in
  downloads"; a click opens the one file's message. Snapshots 197-199.
- Downloads section (`downloads.rs`, `Session::file_downloads`): the bar's
  click on several files shows TDLib's list in place of the chat, rows as
  tdesktop's shared files (colored square with the extension, or the
  circle that resumes or cancels), the date opens the message; Escape or
  back closes it. The connecting pill moves above the bar. Snapshots
  200-201.
- FFmpeg video decoder (`media/src/video.rs`, `hw.rs`): a thread per
  video, VideoToolbox / VA-API / D3D11VA with the software fallback,
  frames scaled to the size shown and queued two deep, seek to a time;
  `media/static-ffmpeg` links the minimal build (checked here: no FFmpeg
  or Homebrew library in the binary); CI builds it cached.
- Videos play in the viewer (`history/media_video.rs`): Play on a
  downloaded video opens it in the viewer, paging with the photos; it
  plays from its start time on the wall clock under tdesktop's
  controller (play/pause, played and left times, the seek bar that seeks
  on click and drag); Space plays and pauses. Snapshots 202-205.
- Round videos and GIFs play in their bubbles (`history/inline_video.rs`):
  Play on a round plays it in the circle with the played part around the
  edge and the time left; a click pauses; at the end the still returns.
  GIFs on disk autoplay muted and looped while on screen. One decoder per
  message in view; what scrolls away stops, a GIF that cannot be decoded
  is not retried. Snapshots 206-208.
- Sound (`media/src/audio.rs`): any file's sound track through FFmpeg,
  resampled on a thread to the output's format half a second ahead of
  it; videos in the viewer and round videos play with it, GIFs and
  stickers stay muted. Automated runs call `media::silence_output()`: the
  players keep time on a clock thread and open no device (the snapshot
  harness does this). Unit tests on the silent clock.
- Music files (`history/music.rs`): TDLib's title, performer, duration and
  file in the model; the row names the song as tdesktop does and shows the
  length, the transfer, or the time played; Play streams it in the
  app's one player (`player/`: a voice or a song), and voices that are
  not Opus stream the same way instead of opening the system player.
  Snapshots 209-211.
- Player bar (`player/bar.rs`, `Media::Player::Widget`): while a song
  plays, a bar tops the chat column (or the one column) with play and
  pause, "Performer – Title", the time played, close, and the played line
  that a click seeks. The sound lives in a global player, so it keeps
  playing when another chat opens; the rows follow it. Snapshot 211.
  Previous and next (`player/playlist.rs`): the chat's songs in message
  order, the closed end dimmed; a song that ends starts the next one,
  downloaded first (`Session::fetch_file`). Unit test
  `the_next_song_is_newer_and_the_ends_stop`.
- Blurry userpics (live 2026-11-03): rows kept the 40 px minithumbnail
  for photos TDLib had downloaded in an earlier run, because it embeds
  those files in chats and users and sends no `updateFile`. `Photo` now
  keeps the small and big files and the store learns them. Snapshot 209,
  unit test `a_userpic_downloaded_before_is_known_without_update_file`.
- Delete for everyone (live 2026-11-03, no checkbox seen): the box now
  follows `DeleteMessagesBox`: "Also delete for <name>" in private chats
  and "Delete for everyone" in basic groups when TDLib allows revoke for
  all; only one's own messages revoke when others cannot; no checkbox in
  supergroups, channels, Saved Messages or bots (bots always revoke); the
  hints after the question; no "Remember this choice" (the owner's call,
  tdesktop has it). The live cause is unconfirmed: Saved
  Messages, supergroups and channels show no checkbox in tdesktop either.
  Snapshots 221-247, unit tests `cargo test -p features delete`.
- Crash on quit (signal 11 after the full snapshot run): `exit` destroyed
  TDLib's statics while the `tdlib-receive` thread was still inside
  `td_receive` (crash report: that thread faults in libtdjson under
  `NSApplication terminate:`). Quitting now closes TDLib, then
  `tg::Stopper` stops and joins the receive thread, then quits; the app's
  Quit and the snapshot harness both do. Test in `tg/tests/runtime.rs`.
- WebM stickers loop in the history with their alpha: VP9 streams marked
  with alpha decode on libvpx (`libvpx-vp9`, as tdesktop forces), others
  keep the hardware decoder; libvpx 1.17.0 joins the minimal FFmpeg build
  (VP9 decoder only). Snapshots 206-208, unit test `a_webm_keeps_its_alpha`.

- Multiple accounts (`accounts.rs`, `main_menu/accounts.rs`, app
  `client.rs`), as tdesktop's `Main::Domain` and main menu: one TDLib
  client, store and data directory per account (the first keeps the old
  directory, the others `account-N/`), the list and the active one in
  settings.json. "Add Account" opens the QR intro (back returns to the
  previous account), past 3 (6 with Premium) the limit box; the accounts
  list under the cover's arrow with badges, the unread dot on the arrow
  and the menu button; switching without a restart; logging out of an
  account other accounts stay with removes it; signing in a user already
  there logs the new one out and shows the old one. Snapshots 251-271.
- Profiles (`info/`, model `profile.rs`), as tdesktop's info section in a
  layer: from a chat's name in the top bar and "My Profile". The cover
  with the big photo (its viewer pages a user's photos), the details (bio,
  username, phone, birthday; description and link for groups), the
  notifications toggle, the shared media lists with counts, groups in
  common, members, and the actions (message, share or delete contact,
  block, report with TDLib's reasons, delete or leave). One's own profile
  edits the name, bio, username and birthday and sets the photo.
  Snapshots 281-296.
- New Group and New Channel from the main menu (`create_chat.rs`, model
  `create.rs`), as tdesktop's `GroupInfoBox`, `SetupChannelBox` and the
  members box: name, photo and (channels) description; a group picks its
  members and opens; a channel then goes public with a t.me link or stays
  private with its invite link, picks subscribers and opens. The contacts
  box gained a multi-pick mode for it. Snapshots 301-312.
- Live tests 28 and 29 run 2026-11-04 (release bundle, views withheld):
  profiles of a channel, a group, a member and one's own; the big photo
  sharpens, photos page in the viewer, the media lists load, the bio edit
  saved and was put back. "rgram test group" (no members, no photo) and
  "rgram test channel" (private) were created, opened and deleted from
  their profiles. Fixed from the run: night colors that copy another
  color (`boxDividerBg: windowBgOver` and 12 more) now follow the night
  theme as tdesktop's `palette::compute` does (the dividers were white);
  a click inside the profile no longer closes it (centered layers stop
  the mouse-down, as boxes did); a chat deleted for everyone closes
  (`CloseChat`, not checked live); Report is hidden on one's own
  supergroups and channels and in basic groups; the subscribers box after
  a new channel has Skip, which opens the channel.

### Half done (shortcuts in code)

- Rounds and GIFs in bubbles: a round stops
  when it scrolls away (no picture-in-picture); no seek on the ring; a
  GIF's click pauses instead of opening the viewer; frames stretch to the
  box rather than crop.
- WebM stickers play only in the history (stills in the emoji panel and
  set boxes); straight alpha, so one drawn larger than decoded fringes.
- The minithumbnail is stretched, not blurred; no loading radial or
  cancel; no click to open; captions do not widen a photo and narrow
  windows do not shrink it.
- Albums of files or music show as separate messages; the mosaic keeps
  430 px in a narrow history; a late part does not re-measure the row.
- Voice speed has no button; speed shifts the pitch; no drag to seek.
- Quitting: the receive thread can take up to its 1 s receive timeout to
  stop; when the OS quits the app, gpui waits only 200 ms, so that path
  may still crash (wake `td_receive` with a request if live test 24 shows
  it); the `history-perf` example still quits without stopping it.
- Player bar: songs only (a voice shows none); no volume, repeat, order
  or speed; square play and pause glyphs without the morph. The next
  song plays only once it is fully downloaded, and the bar hides in
  between.
- Videos and GIFs play in the system player; inline playback and GIF
  autoplay wait for the media viewer (row 292).
- Contact cards: ADD adds without AddContactBox; a card with no
  account has no VIEW CONTACT (vCard) box; no ripple.
- "Save As..." only for files already on disk.
- Live locations show as still points: no sender pin, updated line or
  countdown; the map picture is always TDLib's 2x one.
- Spoilers: the preview is not blurred and the dots stand still (M8).
- Stickers: TGS and WebM do not play; replies and forwards put a sticker
  in a bubble; selection tints its square; no click to open the set.
- Round videos play in the system player, not inline in the circle.
- Polls: plain text without custom emoji; no option media, voters list
  ("View votes") or close timer; the explanation always shows instead of
  the bulb toggle; a vote on its way shows a still ring; long answers
  wrap before the vote count.
- Large emoji and dice do not animate (Lottie player); slot machines show
  their emoji; a reply or forward keeps emoji in a bubble.
- Checklists: no completer's userpic, fireworks or adding tasks.
- Media viewer: photos only, and only the loaded messages' (not the
  chat's shared media), so "N of M" counts those; no rotate, more menu,
  forward, delete or copy, no album strip, no fullscreen; the caption is
  plain text; the sender's name opens nothing; the zoom does not follow
  the mouse; no open/close animation beyond the fade.
- View-once: covered videos do nothing on click until the in-app video
  player; no countdown ring, seconds label, viewer badge or tooltip;
  view-once voice and round messages play inline, without tdesktop's
  layer that hides the chat.
- Video timestamps: the line's ends are cut square.
- Video player: sound and picture keep separate clocks (started together,
  paused and seeked together); nothing pauses a voice that plays at the
  same time; plays only once the whole
  file is on disk (streaming, row 259); no volume, speed, fullscreen,
  picture-in-picture or quality; the controller never hides; GIFs and
  round videos still open in the system player; the snapshot harness
  draws twice, so 204 shows the clip's end under the Pause button.
- Download bar: a still ring, no grow or fade when done, no thumbnail in
  the circle; voice notes still download outside the list; the section
  has only the newest 100, no search, delete, pause, menu or selection; the static FFmpeg
  is not in CI yet and rgram does not link it yet.

- Online presence (owner's go-ahead 2026-11-04): `Session::set_presence`
  sets TDLib's `online` option from the main view, online while the
  window is active, offline when it is not and when the account's view
  is dropped (switching accounts). `features/presence.rs` adds
  tdesktop's idle timeout: offline after 30 s without a key press, click
  or wheel in the window (boxes included), online again on the next one;
  checked live in the debug log (Offline after 31 s, Online on a wheel
  turn). Shortcuts: the 30 s default, not the server's
  `offline_idle_timeout_ms`; mouse moves and the OS idle time do not count.
- `ui::InputField` scrolls sideways to keep the caret in view, the text
  clipped at the field's margins (snapshots 297-300: a long bio). The bio
  field stays one line (tdesktop's is multi-line).
- Playlist panel (`player/panel.rs`, snapshot 212): resting on the
  player bar's name opens the chat's songs under it, newest first; a
  click plays or pauses one. The playlist is now the chat's 32 songs
  around the one playing (`Session::shared_media_around`), not only the
  history's loaded ones. Shortcuts: no month headers, download icon or
  progress in the panel, no fade.
- The empty New Group members box is right: TDLib's `getContacts`
  returned 0 for the owner's account (`contacts loaded` debug log).

### Decided by the owner (M5)

- Video decoder: FFmpeg via `ffmpeg-next`. Release builds link a minimal
  FFmpeg built from source (no system FFmpeg), hardware decoding with a
  software fallback, decoding off the main thread with only the frames on
  screen kept.

### Next steps (M5, offline), in the owner's order

- Repeat, order, speed and volume (row 302), then the bar for voices
  with their queue (row 304).
- Sticker and GIF panels in the emoji panel.

### Live test list (M5)

1. Open a channel with photos (read only): each photo's box has its size
   at once, the blurry preview shows, then the photo; the time sits on a
   dark pill at the bottom right, under a caption when there is one.
   Scroll a long way through photos and watch memory in Activity Monitor.
2. Files, read only: in a chat with a file, click the arrow: the arc and
   "x / y MB" move, the cross stops it, a second click resumes. Once done,
   click opens it in its app (a .pdf straight away; an .html or unknown
   type asks first); right click, "Show in Finder" selects it. Send a file
   in Saved Messages: the upload row is the same one.
3. Videos and GIFs, read only: a video shows its still with "1:15, 8.3 MB"
   top left and an arrow in the middle; click: the arc and "x / y MB"
   move, the cross stops it; once loaded the play button opens it in the
   system player. A GIF says "GIF, size", then "GIF".
4. Albums, read only: in a channel with albums, each album is one message
   in a mosaic like Telegram Desktop's, with thin gaps, round outer
   corners, one caption below; an album without a caption has no bubble.
   Videos in it have their own button.
5. Voice messages: in Saved Messages send a voice, then play it: the
   button turns to pause, the bars fill and "0:01 / 0:03" moves; pause and
   resume; click the middle of the waveform: it jumps there. Read only
   elsewhere: an unheard incoming voice shows its bars blue with a dot;
   playing it removes the dot (only when views are reported).
6. Contacts: in Saved Messages share a contact (from a phone): the card
   shows the userpic, name and phone; MESSAGE opens their chat; ADD
   shows only for someone not in contacts. Read only elsewhere.
7. Save As: right click a downloaded photo, video, voice and file:
   "Save As..." opens the save box in Downloads with a sensible name;
   save, and the copy opens. In a chat that forbids saving there is none.
8. Places, read only: a shared location shows the map with a red pin in
   the middle and the time on a pill; a venue has its name and address
   under the map; click either: Maps opens at that point.
9. Spoilers, read only: a photo sent with a spoiler shows dots over a
   blurry picture; one click shows the photo; a video under a spoiler has
   no play button until revealed.
10. Stickers, read only: a still sticker shows without a bubble, the
    time on a grey pill; an animated one shows its first frame.
11. Round videos, read only: a round video shows as a circle with "0:09"
    and a dot bottom left; play it: it opens in the player and the dot
    goes (only when views are reported).
12. Polls, only in Saved Messages (create one from a phone): vote in a
    single-answer poll: the results show with your option checked; in
    a several-answer poll pick two and press Vote; right-click: Retract
    vote brings the radios back; Stop poll asks first, then shows "Final
    results". A quiz answered from the phone shows red/green marks and
    the explanation.
13. Large emoji, only in Saved Messages: send one emoji, then three: they
    show big without a bubble; send "🎲": the die shows (still), not a
    text bubble.
14. Checklists, only in Saved Messages (create one from a phone, Premium
    only): click a task: it gets a check and your name; click again:
    the check goes; the status line counts them.
15. Media viewer, read only: in a chat with photos click one: it opens
    over the window, a big photo fitted, a small one at its own size;
    "Photo N of M", the sender and the date bottom left, the caption
    below the photo; Left/Right and the side arrows page; Cmd+= and
    Cmd+- zoom by steps, Cmd+0 toggles 1:1 and fit, drag moves it; the
    download icon opens the save box (cancel it); Esc and a click beside
    the photo close it.
16. View-once, only on the test DC or with the owner's OK per message
    (opening one burns it): a phone sends a view-once photo: it shows
    under dots with the fire and "1"; click: it opens alone, no arrows,
    no save; close: with views reported it turns into "Expired photo";
    with views withheld it stays. A timed photo shows the fire without
    the "1".
17. Video covers and timestamps, read only: a channel post with a video
    that has a custom cover shows that cover, not the first frame; a
    link to a video with a time (or a forward with a start time) shows
    the blue line along the bottom up to that point.
18. Download bar, only in Saved Messages (forward a file of a few MB
    there first): click the file's download button: the bar shows under
    the chat list with the ring, the name and "x / y MB"; when done it
    says "View in chat", and a click opens the message; download two
    files: it says "2 files", and a click opens the Downloads section with
    both, newest first; click a date: the chat opens at that message;
    Escape closes the section. A minute after all are done the bar goes
    away.
19. Video player, read only: in a chat with a video, download it, then
    click Play: it opens in the viewer and plays with its sound under
    the controller; Space pauses and resumes; click and drag on the seek
    bar; at the end the button shows Play and a click starts again; a
    720p or 1080p H.264 video decodes on VideoToolbox (Activity Monitor:
    low CPU for rgram while it plays); a link with a start time starts
    there.
20. Round videos and GIFs, read only: in a chat with round videos,
    download one and click Play: it plays in the circle with its sound
    with the ring filling clockwise and the time counting down; a click
    pauses and resumes; at the end the still and Play come back; an
    unwatched one loses its dot. A chat with GIFs: downloaded GIFs loop
    by themselves; scroll them away and back: they start again; CPU
    stays low with several GIFs on screen.
21. WebM stickers, read only: open a chat with video stickers (or a set
    sent to Saved Messages): each loops in place with no box behind it,
    its edges clean over the wallpaper; scroll away and back: it starts
    again.
22. Music, only in Saved Messages (forward a song there first): the row
    shows "Performer – Title" and "3:45, 8.2 MB"; download it, click Play:
    it plays aloud with "0:12 / 3:45" counting; click again to pause;
    start a voice: the song stops. A song without tags shows its file name.
23. Player bar, only in Saved Messages: play a song: a bar shows above the
    chat with "Performer – Title" and the time counting; its button
    pauses and resumes it, a click on the line under it seeks; open
    another chat: the bar stays and the song keeps playing; come back:
    the row shows the time played; the cross stops it and the bar goes.
    With two or more songs in the chat: previous and next move between
    them (the oldest has previous dimmed, the newest next); let a song
    end: the next newer one downloads and plays. Rest the pointer on the
    bar's name: the playlist opens under it with the chat's songs, newest
    on top, the playing one scrolled into view with "played / length";
    click another song: it plays; move the pointer away: it closes.
24. Quit (Cmd-Q, then once with the Dock's Quit): rgram closes within a
    couple of seconds with no "quit unexpectedly" dialog and no new
    `rgram-*.ips` in ~/Library/Logs/DiagnosticReports.
25. Userpics after a restart: quit rgram, start it again: the chat list,
    group senders, top bar and main menu show sharp photos at once (no
    blurry previews that stay); compare a few with Telegram on a phone.
26. Delete box, without deleting outside Saved Messages: right-click your
    own fresh message in a private chat: "Also delete for <name>" is ticked;
    untick it: no "Remember this choice" shows; Cancel. In a basic group:
    "Delete for everyone"; in a supergroup or channel: no checkbox, only the
    hint; Cancel each time. In Saved Messages delete one test message: no
    checkbox. Select two messages in a private chat, Delete 2: the same box.
27. Accounts (needs you to sign in): main menu, "Add Account": the QR
    intro shows with a back arrow; back returns to your account. Add
    Account again and sign the second account in yourself in the app window
    (QR from the phone, or phone number, code and 2FA). Then: the main menu
    lists both accounts under the arrow, the active one ringed; a message to
    the other account shows its badge and the dot on the menu button;
    clicking it switches without a restart; quit and start: the same
    account shows. Right-click the inactive one, Log out, confirm: it goes
    and the first account stays.
28. Profiles, read-only except in your own: click a contact's name over
    their chat: their photo sharpens, bio, username, phone and birthday
    show; click the photo: their photos page in the viewer (Escape). The
    shared media rows list what the chat has; open one and a message.
    A group: description, link, members; a member opens their profile.
    Do not press Block, Report, Delete or Leave. My Profile (main menu):
    edit the bio, then put it back; the same for the birthday if you like.
29. New Group and New Channel (they change your account: only after you
    say yes, and both get deleted afterwards). New Group: name "rgram test
    group", a photo, Next, pick one contact you agree to add, Create: the
    group opens with its photo. New Channel: name "rgram test channel",
    Create, Private shows the invite link; Save, Skip the subscribers: the
    channel opens. Then delete both from their profiles: Delete group,
    Delete channel (owners only, confirmed).
30. Online presence: bring rgram's window to the front; your own profile
    (and your account seen from another device) shows "online". Switch to
    another app: within a minute it reads "last seen just now". Leave the
    window in front untouched for 30 s: it reads "last seen just now";
    scroll or type: "online" again.
31. Settings main page, read-only (no switch flips): main menu, Settings:
    your photo, name, phone and @username show as on the phone; the
    sections list in tdesktop's order, Language says your language. Click
    the photo: your photos open in the viewer (Escape). The search button:
    type "pass": Privacy and Security shows; Escape twice closes the search,
    then Settings. The QR button: a code shows with your username; scan it
    with the phone's camera: it opens your profile; do not press copy or
    share. The menu: Add Account, Edit info and Log Out show (press none).
    "rgram settings": the send-by choice matches Chat Settings (do not
    change it). Escape goes back a page, then closes.
32. My Account, read-only unless you agree: Settings, menu, Edit info:
    your photo, name, "online", bio, name, phone, @username, personal
    channel and birthday match the phone. Click the phone: the box says to
    change it in the mobile app. Click the username: type "ab": "too
    short"; type a name you know is taken: "already taken"; Cancel. Click
    Personal channel: your public channels list (or "You don't have any");
    Done without choosing. The bio: only with your OK, add a "." at the
    end, wait two seconds, check it on the phone, then remove it again.

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

### Live run log

- 2026-10-27, first start on the live account: a white window with
  "Connecting..." forever. Two causes, both fixed (`1aa5270`): TDLib starts
  an instance only on its first request, and `tg::Runtime::create_client`
  sent none; then TDLib 1.8.69 refuses an empty device model, and the
  refusal hid behind options it answers only after the parameters. The
  runtime test now waits for the first authorization state with no request
  sent; `app::device` sends the model and system version.
- 2026-10-28, M2 on the live account (read only): list order and rows,
  live updates from the phone in Saved Messages, Wi-Fi off and on,
  folders, archive and search look right. Photo userpics were blurry and
  square: gpui clips children only to rectangles and scales images on the
  GPU without mipmaps. Fixed: `ui::round_image` cuts each downloaded
  userpic to an antialiased circle at its device pixel size (Lanczos3);
  to recheck live with M3.
- 2026-10-28, the userpic cache, at the owner's request before M6: an LRU
  of 512 that frees evicted textures (`drop_image`), failed decodes retried
  on a file change or after a back-off, decoding on the background
  executor. Soak (`SNAPSHOTS_SOAK=300`, docs/perf.md): RSS flat at
  223-239 MB over 5 minutes of scrolling 2000 userpics.

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
