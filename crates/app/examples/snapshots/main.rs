//! Renders the sign-in screens to PNG files for comparison with Telegram
//! Desktop, without signing in: each window gets preset model events, and
//! TDLib never receives parameters, so nothing goes online.
//!
//! cargo run -p rgram --example snapshots -- [output directory]

mod chats;
mod history;

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context as _;
use features::settings::FoldersPlacement;
use features::{Root, Store};
use gpui::{
    AnyWindowHandle, App, AppContext as _, AsyncApp, Bounds, Entity, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, PlatformInput, Point, ScrollDelta,
    ScrollEvent, Window, WindowBounds, WindowOptions, point, px, size,
};
use model::auth::{Code, Delivery, Password, Recovery, State, Terms, TermsId};
use model::text::FormattedText;
use model::{Environment, Event, Secret, Session};

/// Long enough for the intro's slide and cover animations to finish.
const SETTLE: Duration = Duration::from_millis(1500);
/// Past the floating date's fade-in, before it hides.
const SCROLL_SETTLE: Duration = Duration::from_millis(500);
const WIDTH: f32 = 800.0;
const HEIGHT: f32 = 600.0;

/// A point in the window, in logical pixels.
type WindowPoint = (f32, f32);

/// One window: the events it receives, each step saved under its name.
struct Scenario {
    /// Applied before the first step.
    setup: Vec<Event>,
    steps: Vec<(&'static str, Event)>,
    /// Clicks at window points after the steps, each saved.
    clicks: Vec<(&'static str, MouseButton, (f32, f32))>,
    /// Then drags from one point to another, saved before the button is let
    /// go; the release drops on the target.
    drags: Vec<(&'static str, WindowPoint, WindowPoint)>,
    /// Then wheel scrolls at a point by pixels, up when positive; saved
    /// before the floating date hides.
    scrolls: Vec<(&'static str, WindowPoint, f32)>,
    /// Set before the window opens; it stays for later windows.
    folders: FoldersPlacement,
    /// Loaded into open chats before the window opens, as TDLib's pages.
    histories: Vec<(model::ChatId, Vec<model::message::Message>)>,
}

fn auth(state: State) -> Event {
    Event::Auth(state)
}

fn code() -> Event {
    auth(State::WaitCode(Code {
        phone_number: Secret::new("+12125550100".to_owned()),
        delivery: Delivery::Telegram { length: 5 },
        next_delivery: Some(Delivery::Sms { length: 5 }),
        resend_after: Duration::from_secs(60),
    }))
}

fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            setup: Vec::new(),
            clicks: Vec::new(),
            drags: Vec::new(),
            scrolls: Vec::new(),
            folders: FoldersPlacement::Sidebar,
            histories: Vec::new(),
            steps: vec![("01-start", auth(State::WaitPhoneNumber))],
        },
        Scenario {
            setup: Vec::new(),
            clicks: Vec::new(),
            drags: Vec::new(),
            scrolls: Vec::new(),
            folders: FoldersPlacement::Sidebar,
            histories: Vec::new(),
            steps: vec![(
                "02-qr",
                auth(State::WaitQrScan {
                    link: Secret::new("tg://login?token=c25hcHNob3Qtb25seQ".to_owned()),
                }),
            )],
        },
        Scenario {
            // Back from the code to the number, as a wrong number does.
            setup: Vec::new(),
            clicks: Vec::new(),
            drags: Vec::new(),
            scrolls: Vec::new(),
            folders: FoldersPlacement::Sidebar,
            histories: Vec::new(),
            steps: vec![
                ("04-code", code()),
                ("03-phone", auth(State::WaitPhoneNumber)),
            ],
        },
        Scenario {
            setup: Vec::new(),
            clicks: Vec::new(),
            drags: Vec::new(),
            scrolls: Vec::new(),
            folders: FoldersPlacement::Sidebar,
            histories: Vec::new(),
            steps: vec![(
                "05-password",
                auth(State::WaitPassword(Password {
                    hint: "pet's name".to_owned(),
                    recovery: Recovery::Email {
                        pattern: "a***@example.org".to_owned(),
                    },
                    has_passport_data: false,
                })),
            )],
        },
        Scenario {
            setup: Vec::new(),
            clicks: Vec::new(),
            drags: Vec::new(),
            scrolls: Vec::new(),
            folders: FoldersPlacement::Sidebar,
            histories: Vec::new(),
            steps: vec![(
                "06-signup",
                auth(State::WaitRegistration {
                    terms: Terms {
                        text: FormattedText::plain(
                            "By signing up you accept the Terms of Service.",
                        ),
                        min_age: 0,
                        show_popup: true,
                    },
                }),
            )],
        },
        Scenario {
            setup: Vec::new(),
            clicks: Vec::new(),
            drags: Vec::new(),
            scrolls: Vec::new(),
            folders: FoldersPlacement::Sidebar,
            histories: Vec::new(),
            steps: vec![("07-email", auth(State::WaitEmailAddress))],
        },
        Scenario {
            setup: Vec::new(),
            clicks: Vec::new(),
            drags: Vec::new(),
            scrolls: Vec::new(),
            folders: FoldersPlacement::Sidebar,
            histories: Vec::new(),
            steps: vec![(
                "08-premium-required",
                auth(State::WaitPremiumPurchase {
                    support_email: "support@example.org".to_owned(),
                }),
            )],
        },
        Scenario {
            setup: Vec::new(),
            clicks: Vec::new(),
            drags: Vec::new(),
            scrolls: Vec::new(),
            folders: FoldersPlacement::Sidebar,
            histories: Vec::new(),
            steps: vec![
                ("09-signed-in", auth(State::Ready)),
                (
                    "10-terms-update",
                    Event::TermsUpdate {
                        id: TermsId("snapshot".to_owned()),
                        terms: Terms {
                            text: FormattedText::plain(
                                "We have updated our Terms of Service. Please read and accept them to keep using the app.",
                            ),
                            min_age: 16,
                            show_popup: true,
                        },
                    },
                ),
            ],
        },
        Scenario {
            setup: chats::events(),
            // Collapses the archive and expands it again for the later windows.
            clicks: vec![
                ("21-archive-menu", MouseButton::Right, ARCHIVE_ROW),
                ("22-archive-collapsed", MouseButton::Left, FIRST_MENU_ITEM),
                ("23-collapsed-menu", MouseButton::Right, ARCHIVE_BAR),
                (
                    "24-archive-expanded",
                    MouseButton::Left,
                    FIRST_BAR_MENU_ITEM,
                ),
                ("25-archive-menu", MouseButton::Right, ARCHIVE_ROW),
                ("26-archive-moved", MouseButton::Left, SECOND_MENU_ITEM),
                ("27-main-menu-archive", MouseButton::Left, MENU_BUTTON),
                (
                    "28-archive-returned",
                    MouseButton::Right,
                    FIRST_MAIN_MENU_ROW,
                ),
                ("32-folder-menu", MouseButton::Right, WORK_TAB),
                ("33-folder-remove", MouseButton::Left, SECOND_TAB_MENU_ITEM),
                ("34-folder-kept", MouseButton::Left, CHAT_AREA),
                ("35-all-chats-menu", MouseButton::Right, ALL_CHATS_TAB),
                ("36-menu-closed", MouseButton::Left, CHAT_AREA),
            ],
            drags: vec![
                ("29-drag-pinned", MARIA_ROW, SAVED_ROW),
                ("30-narrow-list", COLUMN_EDGE, (60.0, 300.0)),
                ("31-wide-list", NARROW_COLUMN_EDGE, COLUMN_EDGE),
                ("37-drag-tab", WORK_TAB, CHANNELS_TAB),
            ],
            scrolls: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: Vec::new(),
            steps: vec![("20-pinned-chats", auth(State::Ready))],
        },
        // Last: the night mode it switches on stays for later windows.
        Scenario {
            setup: chats::events(),
            clicks: vec![
                ("42-history-group", MouseButton::Left, HIKING_ROW),
                ("44-history-reply-jump", MouseButton::Left, GROUP_REPLY),
                ("45-history-reply-return", MouseButton::Left, DOWN_BUTTON),
                ("48-history-link-menu", MouseButton::Right, GROUP_LINK),
                ("49-history-menu-closed", MouseButton::Left, HISTORY_GAP),
                (
                    "50-history-chat-menu",
                    MouseButton::Left,
                    HISTORY_MENU_TOGGLE,
                ),
                (
                    "51-history-chat-menu-closed",
                    MouseButton::Left,
                    HISTORY_GAP,
                ),
                ("52-history-pinned-jump", MouseButton::Left, PINNED_BAR),
                ("53-history-pinned-previous", MouseButton::Left, PINNED_BAR),
                (
                    "60-history-pinned-section",
                    MouseButton::Left,
                    PINNED_SHOW_ALL,
                ),
                ("61-history-pinned-closed", MouseButton::Left, SECTION_BACK),
                ("62-history-search-open", MouseButton::Left, CHAT_SEARCH),
                ("63-history-search-closed", MouseButton::Left, SECTION_BACK),
                ("54-history-link-previews", MouseButton::Left, ANN_ROW),
                ("55-history-calendar", MouseButton::Left, DATE_BADGE),
                (
                    "56-history-calendar-closed",
                    MouseButton::Left,
                    CALENDAR_CLOSE,
                ),
                ("64-history-comments", MouseButton::Left, LANDSCAPES_ROW),
                ("65-history-date-menu", MouseButton::Left, WALK_DATE),
                (
                    "66-history-date-menu-closed",
                    MouseButton::Left,
                    HISTORY_GAP,
                ),
                ("67-history-phone-menu", MouseButton::Left, WALK_PHONE),
                (
                    "68-history-phone-menu-closed",
                    MouseButton::Left,
                    HISTORY_GAP,
                ),
                ("69-history-contact-status", MouseButton::Left, BEN_ROW),
                ("70-history-block-confirm", MouseButton::Left, BLOCK_BUTTON),
                ("71-history-block-cancelled", MouseButton::Left, OUTSIDE_BOX),
                ("72-history-saved", MouseButton::Left, SAVED_ROW),
                ("73-history-saved-search", MouseButton::Left, CHAT_SEARCH),
                ("43-history-private", MouseButton::Left, MARIA_ROW),
            ],
            scrolls: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: history::histories(),
            steps: vec![("41-chat-list", auth(State::Ready))],
            // In Maria Garcia's chat, which the last click opens.
            drags: vec![
                ("46-history-text-selection", (330.0, 299.0), (440.0, 299.0)),
                (
                    "47-history-message-selection",
                    (400.0, 299.0),
                    (650.0, 382.0),
                ),
            ],
        },
        // Pulls past the end of Daily Landscapes to the next unread channel.
        Scenario {
            setup: [chats::events(), vec![unread_channel()]].concat(),
            clicks: vec![("75-history-pull-open", MouseButton::Left, LANDSCAPES_ROW)],
            drags: Vec::new(),
            scrolls: vec![("76-history-next-channel", HISTORY_MIDDLE, -150.0)],
            folders: FoldersPlacement::Top,
            histories: history::histories(),
            steps: vec![("74-history-pull-list", auth(State::Ready))],
        },
        // Scrolls Hiking Club up to show the floating date.
        Scenario {
            setup: chats::events(),
            clicks: vec![("58-history-scroll-open", MouseButton::Left, HIKING_ROW)],
            drags: Vec::new(),
            scrolls: vec![("59-history-floating-date", HISTORY_MIDDLE, 120.0)],
            folders: FoldersPlacement::Top,
            histories: history::histories(),
            steps: vec![("57-history-scroll-list", auth(State::Ready))],
        },
        Scenario {
            setup: chats::events(),
            clicks: vec![
                ("12-main-menu", MouseButton::Left, MENU_BUTTON),
                ("13-night-menu", MouseButton::Left, NIGHT_MODE_ROW),
                ("14-night-contacts", MouseButton::Left, CONTACTS_ROW),
                ("15-night-chat-list", MouseButton::Left, OUTSIDE_BOX),
                ("16-night-folder", MouseButton::Left, FAMILY_BUTTON),
                ("17-night-all-chats", MouseButton::Left, ALL_CHATS_BUTTON),
                ("38-night-sidebar-menu", MouseButton::Right, WORK_BUTTON),
                ("39-night-menu-closed", MouseButton::Left, CHAT_AREA),
                ("18-night-archive", MouseButton::Left, SIDEBAR_ARCHIVE_ROW),
                ("19-night-chat-menu", MouseButton::Right, FIRST_ARCHIVED),
            ],
            drags: vec![(
                "40-night-narrow-sidebar",
                SIDEBAR_COLUMN_EDGE,
                (100.0, 300.0),
            )],
            scrolls: Vec::new(),
            folders: FoldersPlacement::Sidebar,
            histories: Vec::new(),
            steps: vec![("11-chat-list", auth(State::Ready))],
        },
    ]
}

/// The main menu button, the menu's night mode and contacts rows, and a
/// point beside the contacts box.
const MENU_BUTTON: (f32, f32) = (27.0, 27.0);
const NIGHT_MODE_ROW: (f32, f32) = (100.0, 447.0);
const CONTACTS_ROW: (f32, f32) = (100.0, 287.0);
const OUTSIDE_BOX: (f32, f32) = (30.0, 300.0);
/// The sidebar's folder buttons under its main menu button, and the archive
/// row beside it, at the top of the list.
const ALL_CHATS_BUTTON: (f32, f32) = (36.0, 85.0);
const FAMILY_BUTTON: (f32, f32) = (36.0, 209.0);
const WORK_BUTTON: (f32, f32) = (36.0, 147.0);
/// The list's right edge beside the sidebar: 72 + 5/14 of the 728 left.
const SIDEBAR_COLUMN_EDGE: (f32, f32) = (332.0, 300.0);
const SIDEBAR_ARCHIVE_ROW: (f32, f32) = (220.0, 85.0);
const ALL_CHATS_TAB: (f32, f32) = (25.0, 70.0);
const WORK_TAB: (f32, f32) = (105.0, 70.0);
const CHANNELS_TAB: (f32, f32) = (240.0, 70.0);
/// The second item of a menu opened on [`WORK_TAB`].
const SECOND_TAB_MENU_ITEM: (f32, f32) = (155.0, 130.0);
/// Empty space beside the list; a click there closes menus and boxes.
const CHAT_AREA: (f32, f32) = (600.0, 450.0);
/// The archive row sits under the folder tabs; in the archive, which has no
/// tabs, its first chat sits higher.
const ARCHIVE_ROW: (f32, f32) = (150.0, 118.0);
/// The first and second items of a menu opened at the archive row, the collapsed
/// archive bar, and the first item of its menu.
const FIRST_MENU_ITEM: (f32, f32) = (200.0, 144.0);
const SECOND_MENU_ITEM: (f32, f32) = (200.0, 178.0);
/// The main menu's first row, under its cover.
const FIRST_MAIN_MENU_ROW: (f32, f32) = (100.0, 154.0);
const ARCHIVE_BAR: (f32, f32) = (150.0, 105.0);
const FIRST_BAR_MENU_ITEM: (f32, f32) = (200.0, 131.0);
/// The chat list's right edge at the default width (5/14 of 800), and in
/// the narrow list.
const COLUMN_EDGE: (f32, f32) = (286.0, 300.0);
const NARROW_COLUMN_EDGE: (f32, f32) = (66.0, 300.0);
/// Two pinned chats below the archive row.
const SAVED_ROW: (f32, f32) = (150.0, 180.0);
const MARIA_ROW: (f32, f32) = (150.0, 242.0);
const LANDSCAPES_ROW: (f32, f32) = (150.0, 428.0);
const BEN_ROW: (f32, f32) = (150.0, 490.0);
const BLOCK_BUTTON: (f32, f32) = (672.0, 79.0);
/// The date badge over Ann Lee's messages, and the calendar's Close.
const DATE_BADGE: (f32, f32) = (543.0, 63.0);
const CALENDAR_CLOSE: (f32, f32) = (538.0, 448.0);
const ANN_ROW: (f32, f32) = (150.0, 304.0);
const HIKING_ROW: (f32, f32) = (150.0, 366.0);
/// The reply box of "Can't wait to see them" in Hiking Club.
const GROUP_REPLY: (f32, f32) = (670.0, 328.0);
const DOWN_BUTTON: (f32, f32) = (761.0, 565.0);
/// The forwarded link in Hiking Club once back from the reply, and empty
/// space beside its date badge.
const GROUP_LINK: (f32, f32) = (610.0, 513.0);
const HISTORY_GAP: (f32, f32) = (420.0, 181.0);
/// The time and the phone number in Daily Landscapes' last post.
const WALK_DATE: (f32, f32) = (460.0, 427.0);
const WALK_PHONE: (f32, f32) = (557.0, 427.0);
/// The three dots at the right of the chat's top bar.
const HISTORY_MENU_TOGGLE: (f32, f32) = (778.0, 27.0);
/// The middle of the chat's message list.
const HISTORY_MIDDLE: (f32, f32) = (543.0, 400.0);
/// The pinned message bar under the chat's top bar, the button at its
/// right that lists all pins, and a section's back arrow.
const PINNED_BAR: (f32, f32) = (560.0, 80.0);
const PINNED_SHOW_ALL: (f32, f32) = (775.0, 80.0);
const SECTION_BACK: (f32, f32) = (312.0, 27.0);
/// The search button left of the chat's menu toggle.
const CHAT_SEARCH: (f32, f32) = (736.0, 27.0);
const FIRST_ARCHIVED: (f32, f32) = (150.0, 85.0);

fn main() -> anyhow::Result<()> {
    let out = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("target/snapshots"), PathBuf::from);
    std::fs::create_dir_all(&out).with_context(|| format!("creating {}", out.display()))?;
    tg::execute(&td_api::functions::SetLogVerbosityLevel {
        new_verbosity_level: 1,
    })
    .context("setting TDLib's log verbosity")?;
    let runtime = tg::Runtime::start().context("starting TDLib")?;
    let (client, updates) = runtime.create_client();
    gpui_platform::application().run(move |cx: &mut App| {
        // Kept alive so the idle client's updates have somewhere to go.
        let _updates = updates;
        if let Err(error) = ui::init(cx) {
            eprintln!("{error:#}");
            cx.quit();
            return;
        }
        features::init(cx);
        let session = Session::new(client);
        cx.spawn(async move |cx| {
            if let Err(error) = run(session, &out, cx).await {
                eprintln!("{error:#}");
            }
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
    drop(runtime);
    Ok(())
}

async fn run(session: Session, out: &Path, cx: &mut AsyncApp) -> anyhow::Result<()> {
    for scenario in scenarios() {
        let mut steps = scenario.steps.into_iter();
        let setup = scenario.setup;
        let histories = scenario.histories;
        let Some((first_name, first)) = steps.next() else {
            continue;
        };
        let folders = scenario.folders;
        cx.update(|cx| features::settings::set_folders_placement(folders, cx));
        let store = cx.new(|cx| {
            let mut store = Store::new(session.clone(), Environment::Production);
            for event in setup {
                store.apply(event, cx);
            }
            store.apply(first, cx);
            for (chat, messages) in histories {
                store.open_chat(chat.into(), cx);
                store.add_history_page(chat.into(), model::history::Page::Newest, messages, cx);
            }
            for event in history::pins() {
                store.apply(event, cx);
            }
            store
        });
        let window = cx.update(|cx| open(store.clone(), cx))?;
        settle_and_save(window, first_name, out, SETTLE, cx).await?;
        for (name, event) in steps {
            store.update(cx, |store, cx| store.apply(event, cx));
            settle_and_save(window, name, out, SETTLE, cx).await?;
        }
        for (name, button, (x, y)) in scenario.clicks {
            window.update(cx, |_, window, cx| {
                click(button, point(px(x), px(y)), window, cx);
            })?;
            settle_and_save(window, name, out, SETTLE, cx).await?;
        }
        for (name, (from_x, from_y), (to_x, to_y)) in scenario.drags {
            let (from, to) = (point(px(from_x), px(from_y)), point(px(to_x), px(to_y)));
            window.update(cx, |_, window, cx| drag(from, to, window, cx))?;
            settle_and_save(window, name, out, SETTLE, cx).await?;
            window.update(cx, |_, window, cx| release(to, window, cx))?;
        }
        for (name, (x, y), by) in scenario.scrolls {
            window.update(cx, |_, window, cx| {
                window.dispatch_event(
                    PlatformInput::Scroll(ScrollEvent {
                        position: point(px(x), px(y)),
                        delta: ScrollDelta::Pixels(point(px(0.0), px(by))),
                        ..ScrollEvent::default()
                    }),
                    cx,
                );
            })?;
            settle_and_save(window, name, out, SCROLL_SETTLE, cx).await?;
        }
        window.update(cx, |_, window, _| window.remove_window())?;
    }
    Ok(())
}

fn click(button: MouseButton, position: Point<Pixels>, window: &mut Window, cx: &mut App) {
    let modifiers = Modifiers::default();
    window.dispatch_event(
        PlatformInput::MouseMove(MouseMoveEvent {
            position,
            pressed_button: None,
            modifiers,
        }),
        cx,
    );
    window.draw(cx).clear(cx);
    window.dispatch_event(
        PlatformInput::MouseDown(MouseDownEvent {
            button,
            position,
            modifiers,
            click_count: 1,
            first_mouse: false,
        }),
        cx,
    );
    // Text links listen for the release only from the frame after the press.
    window.draw(cx).clear(cx);
    window.dispatch_event(
        PlatformInput::MouseUp(MouseUpEvent {
            button,
            position,
            modifiers,
            click_count: 1,
        }),
        cx,
    );
}

/// Presses at `from` and moves to `to` in steps, past gpui's drag threshold.
fn drag(from: Point<Pixels>, to: Point<Pixels>, window: &mut Window, cx: &mut App) {
    const STEPS: u16 = 8;
    let modifiers = Modifiers::default();
    window.dispatch_event(
        PlatformInput::MouseMove(MouseMoveEvent {
            position: from,
            pressed_button: None,
            modifiers,
        }),
        cx,
    );
    window.draw(cx).clear(cx);
    window.dispatch_event(
        PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position: from,
            modifiers,
            click_count: 1,
            first_mouse: false,
        }),
        cx,
    );
    for step in 1..=STEPS {
        let part = f32::from(step) / f32::from(STEPS);
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: from + (to - from) * part,
                pressed_button: Some(MouseButton::Left),
                modifiers,
            }),
            cx,
        );
        window.draw(cx).clear(cx);
    }
}

fn release(position: Point<Pixels>, window: &mut Window, cx: &mut App) {
    window.dispatch_event(
        PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::default(),
            click_count: 1,
        }),
        cx,
    );
}

fn open(store: Entity<Store>, cx: &mut App) -> anyhow::Result<AnyWindowHandle> {
    let bounds = Bounds::centered(None, size(px(WIDTH), px(HEIGHT)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    };
    let window = cx.open_window(options, |window, cx| {
        cx.new(|cx| Root::new(store, window, cx))
    })?;
    Ok(window.into())
}

async fn settle_and_save(
    window: AnyWindowHandle,
    name: &str,
    out: &Path,
    settle: Duration,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    // A window that is not on screen gets no display-link frames, so draw by
    // hand: once to start the transitions, once after they settled.
    window.update(cx, |_, window, cx| window.draw(cx).clear(cx))?;
    cx.background_executor().timer(settle).await;
    window.update(cx, |_, window, cx| window.draw(cx).clear(cx))?;
    let image = window.update(cx, |_, window, _| window.render_to_image())??;
    let path = out.join(format!("{name}.png"));
    image
        .save(&path)
        .with_context(|| format!("saving {}", path.display()))?;
    println!("{}", path.display());
    Ok(())
}

/// Alumni News with posts to read, for pulling to the next channel.
fn unread_channel() -> Event {
    Event::Chats(model::chats::Change::ReadInbox {
        chat: model::ChatId(-902),
        last_read: model::MessageId(0),
        unread_count: 5,
    })
}
