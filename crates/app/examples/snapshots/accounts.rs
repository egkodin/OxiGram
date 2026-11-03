//! Accounts in the main menu: the arrow in the cover with the others'
//! unread badge, the list with the shown account ringed, each other one's
//! badge and menu, and "Add Account" opening the new account's QR code,
//! whose back arrow returns.

use features::settings::FoldersPlacement;
use gpui::MouseButton;
use model::auth::State;
use model::chats::Change;
use model::{Event, UnreadMessages, UserId};

use super::chats;
use super::scenarios::{MENU_BUTTON, auth};
use super::{Input, Scenario};

/// The arrow's area in the main menu's cover.
const ACCOUNTS_TOGGLE: (f32, f32) = (250.0, 110.0);
/// Rows under the cover once the list is open: the shown account, Sam,
/// then Kim and "Add Account", or "Add Account" when there is no Kim.
const SAM_ROW: (f32, f32) = (140.0, 200.0);
const ADD_ROW: (f32, f32) = (140.0, 240.0);
const LIMITED_ADD_ROW: (f32, f32) = (140.0, 280.0);
/// The cover above the name, where a click only closes a menu.
const COVER: (f32, f32) = (200.0, 40.0);
/// The intro's back arrow, top left.
const INTRO_BACK: (f32, f32) = (30.0, 30.0);

/// A signed-in account named `first last` with `unread` messages, all in
/// muted chats when `unmuted` is 0.
fn account(id: i64, first: &str, last: &str, unread: i32, unmuted: i32) -> Vec<Event> {
    vec![
        Event::Auth(State::Ready),
        Event::Me(UserId(id)),
        Event::Chats(Change::User(Box::new(chats::user(id, first, last)))),
        Event::UnreadMessages(UnreadMessages {
            count: unread,
            unmuted,
        }),
    ]
}

pub(crate) fn scenarios() -> Vec<Scenario> {
    vec![
        // Three accounts: the badges, an account's menu, and the limit.
        Scenario {
            others: vec![
                account(501, "Sam", "Rivera", 5, 2),
                account(502, "Kim", "Lee", 12, 0),
            ],
            clicks: vec![
                ("252-accounts-menu", MouseButton::Left, MENU_BUTTON),
                ("253-accounts-open", MouseButton::Left, ACCOUNTS_TOGGLE),
                ("254-accounts-row-menu", MouseButton::Right, SAM_ROW),
                ("255-accounts-menu-closed", MouseButton::Left, COVER),
                ("256-accounts-limit", MouseButton::Left, LIMITED_ADD_ROW),
            ],
            keys: vec![("257-accounts-limit-closed", vec![Input::Key("escape")])],
            steps: vec![("251-accounts-list", auth(State::Ready))],
            ..scenario()
        },
        // Two: "Add Account" shows the new one's QR code, whose back arrow
        // returns; then a click on Sam shows Sam's account.
        Scenario {
            others: vec![account(501, "Sam", "Rivera", 5, 2)],
            clicks: vec![
                ("264-accounts-menu", MouseButton::Left, MENU_BUTTON),
                ("265-accounts-add", MouseButton::Left, ADD_ROW),
                ("266-accounts-back", MouseButton::Left, INTRO_BACK),
                ("267-accounts-menu-again", MouseButton::Left, MENU_BUTTON),
                ("268-accounts-switch", MouseButton::Left, SAM_ROW),
                ("269-accounts-switched-menu", MouseButton::Left, MENU_BUTTON),
                ("270-accounts-closed", MouseButton::Left, ACCOUNTS_TOGGLE),
            ],
            keys: vec![("271-accounts-done", vec![Input::Key("escape")])],
            steps: vec![("263-accounts-two", auth(State::Ready))],
            ..scenario()
        },
    ]
}

fn scenario() -> Scenario {
    Scenario {
        abilities: None,
        others: Vec::new(),
        setup: chats::events(),
        clicks: Vec::new(),
        drags: Vec::new(),
        drops: Vec::new(),
        scrolls: Vec::new(),
        keys: Vec::new(),
        folders: FoldersPlacement::Top,
        histories: Vec::new(),
        steps: Vec::new(),
    }
}
