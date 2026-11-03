//! Profiles (`Info::Profile`) opened from a chat's name and from the main
//! menu's "My Profile". The runner has no TDLib, so the full info and the
//! shared media counts never arrive: the cover, what the chat list knows
//! and the actions show.

use features::settings::FoldersPlacement;
use gpui::MouseButton;
use model::auth::State;

use super::chats;
use super::history;
use super::scenarios::{HIKING_ROW, LANDSCAPES_ROW, MARIA_ROW, MENU_BUTTON, auth};
use super::{Input, Scenario};

/// The top bar's name beside the chat list.
const TOP_BAR_NAME: (f32, f32) = (480.0, 27.0);
/// "My Profile" under the main menu's cover.
const MY_PROFILE: (f32, f32) = (140.0, 160.0);
/// The Bio row in one's own profile.
const BIO_ROW: (f32, f32) = (240.0, 330.0);

fn scenario(names: [&'static str; 4], first: (f32, f32), second: (f32, f32)) -> Scenario {
    let [list, opened, profile, closed] = names;
    Scenario {
        abilities: None,
        others: Vec::new(),
        setup: chats::events(),
        clicks: vec![
            (opened, MouseButton::Left, first),
            (profile, MouseButton::Left, second),
        ],
        drags: Vec::new(),
        drops: Vec::new(),
        scrolls: Vec::new(),
        keys: vec![(closed, vec![Input::Key("escape")])],
        folders: FoldersPlacement::Top,
        histories: history::histories(),
        steps: vec![(list, auth(State::Ready))],
    }
}

pub(crate) fn scenarios() -> Vec<Scenario> {
    vec![
        scenario(
            [
                "281-profile-user",
                "282-profile-user-chat",
                "283-profile-user-info",
                "284-profile-user-closed",
            ],
            MARIA_ROW,
            TOP_BAR_NAME,
        ),
        scenario(
            [
                "285-profile-group",
                "286-profile-group-chat",
                "287-profile-group-info",
                "288-profile-group-closed",
            ],
            HIKING_ROW,
            TOP_BAR_NAME,
        ),
        scenario(
            [
                "289-profile-channel",
                "290-profile-channel-chat",
                "291-profile-channel-info",
                "292-profile-channel-closed",
            ],
            LANDSCAPES_ROW,
            TOP_BAR_NAME,
        ),
        scenario(
            [
                "293-profile-self",
                "294-profile-self-menu",
                "295-profile-self-info",
                "296-profile-self-closed",
            ],
            MENU_BUTTON,
            MY_PROFILE,
        ),
        // A bio longer than the field scrolls to the caret.
        Scenario {
            abilities: None,
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("297-profile-bio-menu", MouseButton::Left, MENU_BUTTON),
                ("298-profile-bio-info", MouseButton::Left, MY_PROFILE),
                ("299-profile-bio-box", MouseButton::Left, BIO_ROW),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: vec![(
                "300-profile-bio-long",
                vec![Input::Type(
                    "Web, Telegram, backend and AI: sites, parsers, bots",
                )],
            )],
            folders: FoldersPlacement::Top,
            histories: history::histories(),
            steps: vec![("297-profile-bio", auth(State::Ready))],
        },
    ]
}
