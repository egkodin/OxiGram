//! The windows the snapshots open, and the window points they act on.

use std::path::PathBuf;
use std::time::Duration;

use features::settings::FoldersPlacement;
use gpui::MouseButton;
use model::auth::{Code, Delivery, Password, Recovery, State, Terms, TermsId};
use model::text::FormattedText;
use model::{Event, Secret};

use super::{Input, Scenario, chats, history};

pub(crate) fn auth(state: State) -> Event {
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

pub(super) fn scenarios() -> Vec<Scenario> {
    let (photo, notes) = fixture_files();
    vec![
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: Vec::new(),
            clicks: Vec::new(),
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Sidebar,
            histories: Vec::new(),
            steps: vec![("01-start", auth(State::WaitPhoneNumber))],
        },
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: Vec::new(),
            clicks: Vec::new(),
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
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
            abilities: None,
            others: Vec::new(),
            // Back from the code to the number, as a wrong number does.
            setup: Vec::new(),
            clicks: Vec::new(),
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Sidebar,
            histories: Vec::new(),
            steps: vec![
                ("04-code", code()),
                ("03-phone", auth(State::WaitPhoneNumber)),
            ],
        },
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: Vec::new(),
            clicks: Vec::new(),
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
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
            abilities: None,
            others: Vec::new(),
            setup: Vec::new(),
            clicks: Vec::new(),
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
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
            abilities: None,
            others: Vec::new(),
            setup: Vec::new(),
            clicks: Vec::new(),
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Sidebar,
            histories: Vec::new(),
            steps: vec![("07-email", auth(State::WaitEmailAddress))],
        },
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: Vec::new(),
            clicks: Vec::new(),
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
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
            abilities: None,
            others: Vec::new(),
            setup: Vec::new(),
            clicks: Vec::new(),
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
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
            abilities: None,
            others: Vec::new(),
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
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: Vec::new(),
            steps: vec![("20-pinned-chats", auth(State::Ready))],
        },
        // Settings: Cmd+Enter sends, so Enter in the open field breaks the
        // line; Cmd+Enter then sends (refused with no account).
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("143-settings-chat", MouseButton::Left, SAVED_ROW),
                ("144-settings-menu", MouseButton::Left, MENU_BUTTON),
                ("145-settings-box", MouseButton::Left, SETTINGS_ROW),
                ("145-settings-rgram", MouseButton::Left, RGRAM_SETTINGS_ROW),
                (
                    "146-settings-cmd-enter",
                    MouseButton::Left,
                    SEND_BY_CTRL_ENTER,
                ),
                ("147-settings-closed", MouseButton::Left, SETTINGS_CLOSE),
                ("148-settings-focused", MouseButton::Left, COMPOSE_FIELD),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: vec![
                (
                    "149-settings-enter-breaks",
                    vec![
                        Input::Type("Two"),
                        Input::Key("enter"),
                        Input::Type("lines"),
                    ],
                ),
                (
                    "150-settings-cmd-enter-sends",
                    vec![Input::Key("cmd-enter")],
                ),
            ],
            folders: FoldersPlacement::Top,
            histories: history::histories(),
            steps: vec![("143-settings-list", auth(State::Ready))],
        },
        // Settings search finds Privacy by a keyword; the QR box opens
        // from the cover.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("313-settings-menu", MouseButton::Left, MENU_BUTTON),
                ("313-settings-main", MouseButton::Left, SETTINGS_ROW),
                ("314-settings-search", MouseButton::Left, SETTINGS_SEARCH),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: vec![("315-settings-found", vec![Input::Type("pass")])],
            folders: FoldersPlacement::Top,
            histories: Vec::new(),
            steps: vec![("313-settings-list", auth(State::Ready))],
        },
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("316-settings-menu", MouseButton::Left, MENU_BUTTON),
                ("316-settings-main", MouseButton::Left, SETTINGS_ROW),
                ("317-settings-qr", MouseButton::Left, SETTINGS_QR),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: Vec::new(),
            steps: vec![("316-settings-list", auth(State::Ready))],
        },
        // Settings' menu: Edit info opens My Account.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("318-settings-menu", MouseButton::Left, MENU_BUTTON),
                ("318-settings-main", MouseButton::Left, SETTINGS_ROW),
                ("319-settings-top-menu", MouseButton::Left, SETTINGS_MENU),
                (
                    "320-settings-account",
                    MouseButton::Left,
                    SETTINGS_EDIT_INFO,
                ),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: Vec::new(),
            steps: vec![("318-settings-list", auth(State::Ready))],
        },
        // Settings > Notifications and Sounds.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("321-settings-menu", MouseButton::Left, MENU_BUTTON),
                ("321-settings-main", MouseButton::Left, SETTINGS_ROW),
                (
                    "321-settings-notifications",
                    MouseButton::Left,
                    SETTINGS_NOTIFICATIONS,
                ),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: Vec::new(),
            steps: vec![("321-settings-list", auth(State::Ready))],
        },
        // Settings > Privacy and Security, then its lower half.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("322-settings-menu", MouseButton::Left, MENU_BUTTON),
                ("322-settings-main", MouseButton::Left, SETTINGS_ROW),
                ("322-settings-privacy", MouseButton::Left, SETTINGS_PRIVACY),
                ("324-settings-blocked", MouseButton::Left, SETTINGS_BLOCKED),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: Vec::new(),
            steps: vec![("322-settings-list", auth(State::Ready))],
        },
        // Privacy and Security > Active sessions.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("325-settings-menu", MouseButton::Left, MENU_BUTTON),
                ("325-settings-main", MouseButton::Left, SETTINGS_ROW),
                ("325-settings-privacy", MouseButton::Left, SETTINGS_PRIVACY),
                (
                    "325-settings-sessions",
                    MouseButton::Left,
                    SETTINGS_SESSIONS,
                ),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: Vec::new(),
            steps: vec![("325-settings-list", auth(State::Ready))],
        },
        // Privacy and Security > Auto-delete messages.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("326-settings-menu", MouseButton::Left, MENU_BUTTON),
                ("326-settings-main", MouseButton::Left, SETTINGS_ROW),
                ("326-settings-privacy", MouseButton::Left, SETTINGS_PRIVACY),
                ("326-settings-ttl", MouseButton::Left, SETTINGS_TTL),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: Vec::new(),
            steps: vec![("326-settings-list", auth(State::Ready))],
        },
        // Settings > Chat Settings.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("327-settings-menu", MouseButton::Left, MENU_BUTTON),
                ("327-settings-main", MouseButton::Left, SETTINGS_ROW),
                ("327-settings-chat", MouseButton::Left, SETTINGS_CHAT),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: Vec::new(),
            steps: vec![("327-settings-list", auth(State::Ready))],
        },
        // Settings > Folders.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("328-settings-menu", MouseButton::Left, MENU_BUTTON),
                ("328-settings-main", MouseButton::Left, SETTINGS_ROW),
                ("328-settings-folders", MouseButton::Left, SETTINGS_FOLDERS),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: Vec::new(),
            steps: vec![("328-settings-list", auth(State::Ready))],
        },
        // Settings > Folders > Create new folder.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("329-settings-menu", MouseButton::Left, MENU_BUTTON),
                ("329-settings-main", MouseButton::Left, SETTINGS_ROW),
                ("329-settings-folders", MouseButton::Left, SETTINGS_FOLDERS),
                (
                    "329-settings-folder-new",
                    MouseButton::Left,
                    SETTINGS_FOLDER_CREATE,
                ),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: Vec::new(),
            steps: vec![("329-settings-list", auth(State::Ready))],
        },
        // Settings > Folders > Create new folder > Add Chats.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("330-settings-menu", MouseButton::Left, MENU_BUTTON),
                ("330-settings-main", MouseButton::Left, SETTINGS_ROW),
                ("330-settings-folders", MouseButton::Left, SETTINGS_FOLDERS),
                (
                    "330-settings-folder-new",
                    MouseButton::Left,
                    SETTINGS_FOLDER_CREATE,
                ),
                (
                    "330-settings-folder-chats",
                    MouseButton::Left,
                    SETTINGS_FOLDER_ADD_CHATS,
                ),
                (
                    "330-settings-folder-type",
                    MouseButton::Left,
                    SETTINGS_FOLDER_TYPE,
                ),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: Vec::new(),
            steps: vec![("330-settings-list", auth(State::Ready))],
        },
        // Settings > Advanced.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("331-settings-menu", MouseButton::Left, MENU_BUTTON),
                ("331-settings-main", MouseButton::Left, SETTINGS_ROW),
                (
                    "331-settings-advanced",
                    MouseButton::Left,
                    SETTINGS_ADVANCED,
                ),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: Vec::new(),
            steps: vec![("331-settings-list", auth(State::Ready))],
        },
        // Settings > Advanced > Connection type.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("332-settings-menu", MouseButton::Left, MENU_BUTTON),
                ("332-settings-main", MouseButton::Left, SETTINGS_ROW),
                (
                    "332-settings-advanced",
                    MouseButton::Left,
                    SETTINGS_ADVANCED,
                ),
                (
                    "332-settings-proxies",
                    MouseButton::Left,
                    SETTINGS_CONNECTION,
                ),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: Vec::new(),
            steps: vec![("332-settings-list", auth(State::Ready))],
        },
        // Settings > Advanced > Manage local storage.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("333-settings-menu", MouseButton::Left, MENU_BUTTON),
                ("333-settings-main", MouseButton::Left, SETTINGS_ROW),
                (
                    "333-settings-advanced",
                    MouseButton::Left,
                    SETTINGS_ADVANCED,
                ),
                ("333-settings-storage", MouseButton::Left, SETTINGS_STORAGE),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: Vec::new(),
            steps: vec![("333-settings-list", auth(State::Ready))],
        },
        // Settings > Advanced > Connection type > Add proxy.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("334-settings-menu", MouseButton::Left, MENU_BUTTON),
                ("334-settings-main", MouseButton::Left, SETTINGS_ROW),
                (
                    "334-settings-advanced",
                    MouseButton::Left,
                    SETTINGS_ADVANCED,
                ),
                (
                    "334-settings-proxies",
                    MouseButton::Left,
                    SETTINGS_CONNECTION,
                ),
                (
                    "334-settings-proxy-add",
                    MouseButton::Left,
                    SETTINGS_PROXY_ADD,
                ),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: Vec::new(),
            steps: vec![("334-settings-list", auth(State::Ready))],
        },
        // Settings > Speakers and Camera > Output device.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("335-settings-menu", MouseButton::Left, MENU_BUTTON),
                ("335-settings-main", MouseButton::Left, SETTINGS_ROW),
                ("335-settings-calls", MouseButton::Left, SETTINGS_CALLS),
                ("335-settings-output", MouseButton::Left, SETTINGS_OUTPUT),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: Vec::new(),
            steps: vec![("335-settings-list", auth(State::Ready))],
        },
        // Settings > Battery and Animations.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("336-settings-menu", MouseButton::Left, MENU_BUTTON),
                ("336-settings-main", MouseButton::Left, SETTINGS_ROW),
                ("336-settings-power", MouseButton::Left, SETTINGS_POWER),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: Vec::new(),
            steps: vec![("336-settings-list", auth(State::Ready))],
        },
        // Settings > Battery and Animations > Animated Stickers.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("337-settings-menu", MouseButton::Left, MENU_BUTTON),
                ("337-settings-main", MouseButton::Left, SETTINGS_ROW),
                ("337-settings-power", MouseButton::Left, SETTINGS_POWER),
                (
                    "337-settings-power-stickers",
                    MouseButton::Left,
                    SETTINGS_POWER_STICKERS,
                ),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: Vec::new(),
            steps: vec![("337-settings-list", auth(State::Ready))],
        },
        // Settings > Language.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("338-settings-menu", MouseButton::Left, MENU_BUTTON),
                ("338-settings-main", MouseButton::Left, SETTINGS_ROW),
                (
                    "338-settings-language",
                    MouseButton::Left,
                    SETTINGS_LANGUAGE,
                ),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: Vec::new(),
            steps: vec![("338-settings-list", auth(State::Ready))],
        },
        // Settings, scrolled to "Default interface scale".
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("339-settings-menu", MouseButton::Left, MENU_BUTTON),
                ("339-settings-main", MouseButton::Left, SETTINGS_ROW),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: vec![("339-settings-scale", SETTINGS_MIDDLE, -400.0)],
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: Vec::new(),
            steps: vec![("339-settings-list", auth(State::Ready))],
        },
        // Settings, Telegram Premium: the cover, the periods with the
        // annual discount, the features and the subscribe button.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("340-settings-menu", MouseButton::Left, MENU_BUTTON),
                ("340-settings-main", MouseButton::Left, SETTINGS_ROW),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: vec![("340-settings-scrolled", SETTINGS_MIDDLE, -400.0)],
            keys: vec![("340-settings-premium", vec![Input::Click(SETTINGS_PREMIUM)])],
            folders: FoldersPlacement::Top,
            histories: Vec::new(),
            steps: vec![("340-settings-list", auth(State::Ready))],
        },
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("341-settings-menu", MouseButton::Left, MENU_BUTTON),
                ("341-settings-main", MouseButton::Left, SETTINGS_ROW),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: vec![("341-settings-scrolled", SETTINGS_MIDDLE, -400.0)],
            keys: vec![(
                "341-settings-business",
                vec![Input::Click(SETTINGS_BUSINESS)],
            )],
            folders: FoldersPlacement::Top,
            histories: Vec::new(),
            steps: vec![("341-settings-list", auth(State::Ready))],
        },
        // Types in Saved Messages' field: it grows, then scrolls; sending
        // fails with no account, so the text comes back.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("78-compose-saved", MouseButton::Left, SAVED_ROW),
                ("79-compose-focused", MouseButton::Left, COMPOSE_FIELD),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: vec![
                (
                    "80-compose-typed",
                    vec![Input::Type("Shopping for the weekend")],
                ),
                (
                    "81-compose-lines",
                    vec![
                        Input::Key("shift-enter"),
                        Input::Type("eggs, bread and coffee"),
                        Input::Key("shift-enter"),
                        Input::Type("and a cake"),
                    ],
                ),
                (
                    "82-compose-scrolled",
                    (0..14)
                        .flat_map(|_| [Input::Key("shift-enter"), Input::Type("one more line")])
                        .collect(),
                ),
                ("83-compose-send-refused", vec![Input::Key("enter")]),
                (
                    "84-compose-cleared",
                    vec![Input::Key("cmd-a"), Input::Key("backspace")],
                ),
            ],
            folders: FoldersPlacement::Top,
            histories: history::histories(),
            steps: vec![("77-compose-list", auth(State::Ready))],
        },
        // The bars in place of the field: Ben blocked, the bot never
        // started, the channel left, the group closed to members.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: [chats::events(), write_restrictions()].concat(),
            clicks: vec![
                ("86-write-unblock", MouseButton::Left, BEN_ROW),
                ("87-write-start-bot", MouseButton::Left, BOT_ROW),
                ("88-write-join", MouseButton::Left, LANDSCAPES_ROW),
                ("89-write-restricted", MouseButton::Left, HIKING_ROW),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: history::histories(),
            steps: vec![("85-write-list", auth(State::Ready))],
        },
        // The bar over the field: Up edits the last own message, Escape
        // gives the field back, Cmd+Up walks the reply up the history, and
        // a refused reply keeps both the text and the reply.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("91-bar-chat", MouseButton::Left, MARIA_ROW),
                ("92-bar-focused", MouseButton::Left, COMPOSE_FIELD),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: vec![
                ("93-bar-edit-last", vec![Input::Key("up")]),
                ("94-bar-edit-typed", vec![Input::Type(", or 9")]),
                ("95-bar-edit-cancelled", vec![Input::Key("escape")]),
                ("96-bar-reply", vec![Input::Key(SECONDARY_UP)]),
                ("97-bar-reply-previous", vec![Input::Key(SECONDARY_UP)]),
                ("98-bar-reply-typed", vec![Input::Type("On my way")]),
                ("99-bar-reply-refused", vec![Input::Key("enter")]),
                ("100-bar-reply-cancelled", vec![Input::Key("escape")]),
            ],
            folders: FoldersPlacement::Top,
            histories: history::histories(),
            steps: vec![("90-bar-list", auth(State::Ready))],
        },
        // Forwarding: select a message, Forward in the selection bar, pick
        // Saved Messages in the box, which opens with the forward over its
        // field; sending fails with no account and keeps it.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("102-forward-chat", MouseButton::Left, MARIA_ROW),
                ("103-forward-menu", MouseButton::Right, SEATS_MESSAGE),
                ("104-forward-selected", MouseButton::Left, MENU_SELECT),
                ("105-forward-box", MouseButton::Left, SELECTION_FORWARD),
                ("106-forward-bar", MouseButton::Left, BOX_SAVED_ROW),
                ("107-forward-focused", MouseButton::Left, COMPOSE_FIELD),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: vec![
                ("108-forward-comment", vec![Input::Type("For the trip")]),
                ("109-forward-refused", vec![Input::Key("enter")]),
            ],
            folders: FoldersPlacement::Top,
            histories: history::histories(),
            steps: vec![("101-forward-list", auth(State::Ready))],
        },
        // The emoji panel: hovering the button shows it and a click there
        // hides it, as in tdesktop; another click brings it back. An emoji
        // goes into the field, a footer icon brings its category up, and
        // leaving the panel hides it.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("111-emoji-chat", MouseButton::Left, MARIA_ROW),
                ("112-emoji-toggled-off", MouseButton::Left, COMPOSE_EMOJI),
                ("113-emoji-panel", MouseButton::Left, COMPOSE_EMOJI),
                ("114-emoji-typed", MouseButton::Left, EMOJI_FIRST),
                ("115-emoji-food", MouseButton::Left, EMOJI_FOOD),
                ("116-emoji-left", MouseButton::Left, COMPOSE_FIELD),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: history::histories(),
            steps: vec![("110-emoji-list", auth(State::Ready))],
        },
        // Files: a pending upload with its progress in Saved Messages, then
        // a photo and a text file dropped on the one zone, the box with its
        // caption typed, and Escape giving the caption back to the field.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![("118-upload-saved", MouseButton::Left, SAVED_ROW)],
            drags: Vec::new(),
            drops: vec![("119-drop-files", CHAT_AREA, vec![photo.clone(), notes])],
            scrolls: Vec::new(),
            keys: vec![
                ("120-files-caption", vec![Input::Type("Trail notes")]),
                ("121-files-cancelled", vec![Input::Key("escape")]),
            ],
            folders: FoldersPlacement::Top,
            histories: history::uploads(),
            steps: vec![("117-upload-list", auth(State::Ready))],
        },
        // A photo alone: documents above, compressed below; it goes compressed.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![("123-photo-chat", MouseButton::Left, MARIA_ROW)],
            drags: Vec::new(),
            drops: vec![("124-drop-photo", CHAT_AREA, vec![photo])],
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: history::histories(),
            steps: vec![("122-photo-list", auth(State::Ready))],
        },
        // Voice, from preset samples instead of the microphone: holding the
        // microphone and leaving the field (release cancels), back in the
        // field (release sends), then up past the lock.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![("125-voice-chat", MouseButton::Left, SAVED_ROW)],
            drags: vec![
                ("126-voice-outside", COMPOSE_MIC, (776.0, 520.0)),
                ("127-voice-in-field", COMPOSE_MIC, (700.0, 577.0)),
                ("128-voice-locked", COMPOSE_MIC, ABOVE_LOCK),
            ],
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: history::uploads(),
            steps: vec![("125-voice-list", auth(State::Ready))],
        },
        // The send menu on a right click with a draft in the field, and the
        // schedule box it opens, to a user whose last seen time shows: the
        // bell off, the calendar over the box and back, then the dots.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("129-send-chat", MouseButton::Left, BEN_ROW),
                ("130-send-menu", MouseButton::Right, COMPOSE_MIC),
                ("131-schedule-box", MouseButton::Left, SEND_MENU_SCHEDULE),
                ("132-schedule-silent", MouseButton::Left, SCHEDULE_NOTIFY),
                ("133-schedule-calendar", MouseButton::Left, SCHEDULE_DAY),
                ("134-schedule-back", MouseButton::Left, CALENDAR_CLOSE),
                ("135-schedule-menu", MouseButton::Left, SCHEDULE_DOTS),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: history::histories(),
            steps: vec![("129-send-list", auth(State::Ready))],
        },
        // In Saved Messages the menu only sets a reminder.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: saved_draft(),
            clicks: vec![
                ("136-reminder-chat", MouseButton::Left, SAVED_ROW),
                ("137-reminder-menu", MouseButton::Right, COMPOSE_MIC),
                ("138-reminder-box", MouseButton::Left, REMINDER_ITEM),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: history::histories(),
            steps: vec![("136-reminder-list", auth(State::Ready))],
        },
        // Reminders: the button by the field, the section with its day
        // badges, and a reminder's menu.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("140-reminders-chat", MouseButton::Left, SAVED_ROW),
                ("141-reminders", MouseButton::Left, COMPOSE_SCHEDULED),
                ("142-reminder-actions", MouseButton::Right, FIRST_REMINDER),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: history::uploads(),
            steps: [("139-reminders-list", auth(State::Ready))]
                .into_iter()
                .chain(
                    history::reminders()
                        .into_iter()
                        .map(|event| ("139-reminders-set", event)),
                )
                .collect(),
        },
        // Last: the night mode it switches on stays for later windows.
        Scenario {
            abilities: None,
            others: Vec::new(),
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
                ("55-history-calendar", MouseButton::Left, DATE_BADGE),
                (
                    "56-history-calendar-closed",
                    MouseButton::Left,
                    CALENDAR_CLOSE,
                ),
                ("54-history-link-previews", MouseButton::Left, ANN_ROW),
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
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: history::histories(),
            steps: vec![("41-chat-list", auth(State::Ready))],
            // In Maria Garcia's chat, which the last click opens.
            drags: vec![
                ("46-history-text-selection", (330.0, 253.0), (440.0, 253.0)),
                (
                    "47-history-message-selection",
                    (400.0, 253.0),
                    (650.0, 336.0),
                ),
            ],
        },
        // Pulls past the end of Daily Landscapes to the next unread channel.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: [chats::events(), vec![unread_channel()]].concat(),
            clicks: vec![("75-history-pull-open", MouseButton::Left, LANDSCAPES_ROW)],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: vec![("76-history-next-channel", HISTORY_MIDDLE, -150.0)],
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: history::histories(),
            steps: vec![("74-history-pull-list", auth(State::Ready))],
        },
        // Scrolls Hiking Club up to show the floating date.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![("58-history-scroll-open", MouseButton::Left, HIKING_ROW)],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: vec![("59-history-floating-date", HISTORY_MIDDLE, 120.0)],
            keys: Vec::new(),
            folders: FoldersPlacement::Top,
            histories: history::histories(),
            steps: vec![("57-history-scroll-list", auth(State::Ready))],
        },
        Scenario {
            abilities: None,
            others: Vec::new(),
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
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: Vec::new(),
            folders: FoldersPlacement::Sidebar,
            histories: Vec::new(),
            steps: vec![("11-chat-list", auth(State::Ready))],
        },
    ]
}

/// The main menu button, the menu's night mode and contacts rows, and a
/// point beside the contacts box.
pub(crate) const MENU_BUTTON: (f32, f32) = (27.0, 27.0);
const NIGHT_MODE_ROW: (f32, f32) = (100.0, 447.0);
const CONTACTS_ROW: (f32, f32) = (100.0, 287.0);
pub(crate) const OUTSIDE_BOX: (f32, f32) = (30.0, 300.0);
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
pub(crate) const CHAT_AREA: (f32, f32) = (600.0, 450.0);
/// The archive row sits under the folder tabs; in the archive, which has no
/// tabs, its first chat sits higher.
const ARCHIVE_ROW: (f32, f32) = (150.0, 118.0);
/// The first and second items of a menu opened at the archive row, the collapsed
/// archive bar, and the first item of its menu.
const FIRST_MENU_ITEM: (f32, f32) = (200.0, 144.0);
const SECOND_MENU_ITEM: (f32, f32) = (200.0, 178.0);
/// The main menu's first row, under its cover.
const FIRST_MAIN_MENU_ROW: (f32, f32) = (100.0, 154.0);
/// The main menu's Settings row.
const SETTINGS_ROW: (f32, f32) = (100.0, 407.0);
/// Settings' "rgram settings" row, then its Cmd+Enter choice and the
/// layer's close button.
const RGRAM_SETTINGS_ROW: (f32, f32) = (300.0, 566.0);
const SEND_BY_CTRL_ENTER: (f32, f32) = (330.0, 326.0);
const SETTINGS_CLOSE: (f32, f32) = (568.0, 48.0);
/// The settings layer's search button and the cover's QR button.
const SETTINGS_SEARCH: (f32, f32) = (482.0, 48.0);
const SETTINGS_QR: (f32, f32) = (564.0, 124.0);
/// The settings layer's menu button and the menu's Edit info.
const SETTINGS_MENU: (f32, f32) = (528.0, 48.0);
const SETTINGS_EDIT_INFO: (f32, f32) = (450.0, 72.0);
const SETTINGS_NOTIFICATIONS: (f32, f32) = (300.0, 246.0);
const SETTINGS_PRIVACY: (f32, f32) = (300.0, 291.0);
const SETTINGS_BLOCKED: (f32, f32) = (300.0, 270.0);
const SETTINGS_SESSIONS: (f32, f32) = (300.0, 310.0);
const SETTINGS_TTL: (f32, f32) = (300.0, 190.0);
const SETTINGS_CHAT: (f32, f32) = (300.0, 336.0);
const SETTINGS_FOLDERS: (f32, f32) = (300.0, 381.0);
const SETTINGS_FOLDER_CREATE: (f32, f32) = (300.0, 372.0);
const SETTINGS_FOLDER_ADD_CHATS: (f32, f32) = (300.0, 247.0);
const SETTINGS_FOLDER_TYPE: (f32, f32) = (400.0, 330.0);
const SETTINGS_ADVANCED: (f32, f32) = (300.0, 426.0);
const SETTINGS_CONNECTION: (f32, f32) = (300.0, 142.0);
const SETTINGS_STORAGE: (f32, f32) = (300.0, 182.0);
const SETTINGS_PROXY_ADD: (f32, f32) = (522.0, 410.0);
const SETTINGS_CALLS: (f32, f32) = (300.0, 451.0);
const SETTINGS_POWER: (f32, f32) = (300.0, 491.0);
const SETTINGS_LANGUAGE: (f32, f32) = (300.0, 531.0);
/// The Premium section's rows once the main page scrolled 400 px.
const SETTINGS_PREMIUM: (f32, f32) = (300.0, 320.0);
const SETTINGS_BUSINESS: (f32, f32) = (300.0, 400.0);
const SETTINGS_MIDDLE: (f32, f32) = (400.0, 400.0);
const SETTINGS_POWER_STICKERS: (f32, f32) = (300.0, 240.0);
const SETTINGS_OUTPUT: (f32, f32) = (300.0, 142.0);
const ARCHIVE_BAR: (f32, f32) = (150.0, 105.0);
const FIRST_BAR_MENU_ITEM: (f32, f32) = (200.0, 131.0);
/// The chat list's right edge at the default width (5/14 of 800), and in
/// the narrow list.
const COLUMN_EDGE: (f32, f32) = (286.0, 300.0);
const NARROW_COLUMN_EDGE: (f32, f32) = (66.0, 300.0);
/// Two pinned chats below the archive row.
pub(crate) const SAVED_ROW: (f32, f32) = (150.0, 180.0);
pub(crate) const MARIA_ROW: (f32, f32) = (150.0, 242.0);
pub(crate) const LANDSCAPES_ROW: (f32, f32) = (150.0, 428.0);
const BEN_ROW: (f32, f32) = (150.0, 490.0);
pub(crate) const BOT_ROW: (f32, f32) = (150.0, 552.0);
const BLOCK_BUTTON: (f32, f32) = (672.0, 79.0);
/// The date badge over Hiking Club's messages, and the calendar's Close.
const DATE_BADGE: (f32, f32) = (543.0, 135.0);
const CALENDAR_CLOSE: (f32, f32) = (538.0, 448.0);
const ANN_ROW: (f32, f32) = (150.0, 304.0);
pub(crate) const HIKING_ROW: (f32, f32) = (150.0, 366.0);
/// The reply box of "Can't wait to see them" in Hiking Club.
pub(crate) const GROUP_REPLY: (f32, f32) = (670.0, 282.0);
const DOWN_BUTTON: (f32, f32) = (761.0, 519.0);
/// The forwarded link in Hiking Club once back from the reply, and empty
/// space beside its date badge.
const GROUP_LINK: (f32, f32) = (610.0, 467.0);
const HISTORY_GAP: (f32, f32) = (420.0, 135.0);
/// The time and the phone number in Daily Landscapes' last post.
const WALK_DATE: (f32, f32) = (460.0, 381.0);
const WALK_PHONE: (f32, f32) = (557.0, 381.0);
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
/// Ctrl+Up, Cmd+Up on macOS: reply to the message above.
const SECONDARY_UP: &str = if cfg!(target_os = "macos") {
    "cmd-up"
} else {
    "ctrl-up"
};
/// "Yes, two seats in row 5" in Maria's chat.
pub(crate) const SEATS_MESSAGE: (f32, f32) = (640.0, 295.0);
/// "Select" in the menu that opens there.
pub(crate) const MENU_SELECT: (f32, f32) = (712.0, 351.0);
/// "Forward 1" in the selection bar.
const SELECTION_FORWARD: (f32, f32) = (350.0, 27.0);
/// Saved Messages, first in "Forward to...".
const BOX_SAVED_ROW: (f32, f32) = (360.0, 125.0);
/// The message field under an open chat.
const COMPOSE_FIELD: (f32, f32) = (543.0, 577.0);
/// The microphone in the send button's place, and above it past the lock.
const COMPOSE_MIC: (f32, f32) = (776.0, 577.0);
const ABOVE_LOCK: (f32, f32) = (776.0, 380.0);
/// The emoji button right of the field.
/// "Schedule message" in the send menu opened on the send button.
const SEND_MENU_SCHEDULE: (f32, f32) = (690.0, 521.0);
/// The schedule box's bell, dots and day field.
const SCHEDULE_NOTIFY: (f32, f32) = (510.0, 228.0);
const SCHEDULE_DOTS: (f32, f32) = (555.0, 228.0);
const SCHEDULE_DAY: (f32, f32) = (334.0, 297.0);
/// "Set a reminder", the Saved Messages send menu's one item.
const REMINDER_ITEM: (f32, f32) = (690.0, 555.0);
/// The scheduled button left of emoji, and the first reminder's bubble.
const COMPOSE_SCHEDULED: (f32, f32) = (686.0, 577.0);
const FIRST_REMINDER: (f32, f32) = (660.0, 495.0);
const COMPOSE_EMOJI: (f32, f32) = (731.0, 577.0);
/// The first recent emoji in the panel.
const EMOJI_FIRST: (f32, f32) = (471.0, 154.0);
/// Food & Drink in the panel's footer.
const EMOJI_FOOD: (f32, f32) = (595.0, 526.0);
const FIRST_ARCHIVED: (f32, f32) = (150.0, 85.0);

/// Alumni News with posts to read, for pulling to the next channel.
fn unread_channel() -> Event {
    Event::Chats(model::chats::Change::ReadInbox {
        chat: model::ChatId(-902),
        last_read: model::MessageId(0),
        unread_count: 5,
    })
}

/// Ben blocked, the bot's chat empty, Daily Landscapes left and Hiking Club
/// closed to its members.
fn write_restrictions() -> Vec<Event> {
    use model::chats::Change;
    vec![
        Event::Chats(Change::Blocked {
            chat: model::ChatId(4),
            is_blocked: true,
        }),
        Event::Chats(Change::LastMessage {
            chat: model::ChatId(5),
            message: None,
            positions: Vec::new(),
        }),
        Event::Chats(Change::Supergroup {
            supergroup: model::SupergroupId(200),
            count: 48_210,
            status: model::rights::MemberStatus::Left,
        }),
        Event::Chats(Change::Permissions {
            chat: model::ChatId(-100),
            permissions: model::rights::Permissions {
                can_send_basic_messages: false,
            },
        }),
    ]
}

/// A photo and a text file to drop, written where the system clears them.
pub(crate) fn fixture_files() -> (PathBuf, PathBuf) {
    let folder = std::env::temp_dir().join("rgram-snapshots");
    let photo = folder.join("summit.png");
    let notes = folder.join("trail-notes.txt");
    let written = std::fs::create_dir_all(&folder)
        .map_err(anyhow::Error::from)
        .and_then(|()| {
            let image = image::RgbImage::from_fn(640, 427, |x, y| {
                let [x, y] = [x, y].map(|value| u8::try_from(value % 256).unwrap_or(0));
                image::Rgb([x / 2 + 60, y / 2 + 90, 200])
            });
            image.save(&photo)?;
            std::fs::write(&notes, "Start at the lake, 12.4 km.\n")?;
            Ok(())
        });
    if let Err(error) = written {
        // The drop scenarios then save their boxes without these files.
        eprintln!("writing the drop fixtures: {error:#}");
    }
    (photo, notes)
}

/// The chat list with a draft in Saved Messages, so its send button shows.
fn saved_draft() -> Vec<Event> {
    let mut events = chats::events();
    events.push(Event::Chats(model::chats::Change::Draft {
        chat: model::ChatId(chats::ME),
        draft: Some(model::chat::Draft {
            date: chats::now(),
            text: model::text::FormattedText::plain("Call the dentist"),
        }),
        positions: Vec::new(),
    }));
    events
}
