//! "New Group" and "New Channel" (`GroupInfoBox`): the name typed, then
//! for the group the members box with a contact picked. The runner has no
//! TDLib, so nothing is created.

use features::settings::FoldersPlacement;
use gpui::MouseButton;
use model::auth::State;

use super::chats;
use super::scenarios::{MENU_BUTTON, auth};
use super::{Input, Scenario};

const NEW_GROUP: (f32, f32) = (100.0, 207.0);
const NEW_CHANNEL: (f32, f32) = (100.0, 247.0);

fn scenario(
    names: [&'static str; 3],
    item: (f32, f32),
    keys: Vec<(&'static str, Vec<Input>)>,
) -> Scenario {
    let [list, menu, opened] = names;
    Scenario {
        abilities: None,
        others: Vec::new(),
        setup: chats::events(),
        clicks: vec![
            (menu, MouseButton::Left, MENU_BUTTON),
            (opened, MouseButton::Left, item),
        ],
        drags: Vec::new(),
        drops: Vec::new(),
        scrolls: Vec::new(),
        keys,
        folders: FoldersPlacement::Top,
        histories: Vec::new(),
        steps: vec![(list, auth(State::Ready))],
    }
}

pub(crate) fn scenarios() -> Vec<Scenario> {
    vec![
        scenario(
            [
                "301-create-group",
                "302-create-group-menu",
                "303-create-group-box",
            ],
            NEW_GROUP,
            vec![
                ("304-create-group-named", vec![Input::Type("Book Club")]),
                ("305-create-group-members", vec![Input::Key("enter")]),
                ("306-create-group-filtered", vec![Input::Type("ann")]),
                ("307-create-group-closed", vec![Input::Key("escape")]),
            ],
        ),
        scenario(
            [
                "308-create-channel",
                "309-create-channel-menu",
                "310-create-channel-box",
            ],
            NEW_CHANNEL,
            vec![
                ("311-create-channel-named", vec![Input::Type("Daily Notes")]),
                ("312-create-channel-closed", vec![Input::Key("escape")]),
            ],
        ),
    ]
}
