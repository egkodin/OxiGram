//! The delete box (`DeleteMessagesBox`) where TDLib lets every message go for
//! everyone: "Also delete for Maria" in her chat, "Delete for everyone" in
//! Hiking Club, no checkbox in the channel or Saved Messages; toggling it
//! adds nothing (no "Remember this choice").

use features::settings::FoldersPlacement;
use gpui::MouseButton;
use model::ChatId;
use model::auth::State;
use model::message_actions::Abilities;

use super::chats::{self, ME};
use super::history;
use super::scenarios::{
    GROUP_REPLY, HIKING_ROW, LANDSCAPES_ROW, MARIA_ROW, SAVED_ROW, SEATS_MESSAGE, auth,
};
use super::{Input, Scenario};

/// Every message deletable for everyone, as for one's own fresh messages.
fn revocable() -> Abilities {
    Abilities {
        can_be_deleted_only_for_self: true,
        can_be_deleted_for_all_users: true,
        can_be_replied: true,
        can_be_forwarded: true,
        ..Abilities::default()
    }
}

/// A box opened by a message's menu: the chat, the menu, Delete, the
/// checkbox toggled, then closed.
fn scenario(
    names: [&'static str; 5],
    chat: (f32, f32),
    message: (f32, f32),
    delete: (f32, f32),
    checkbox: (f32, f32),
) -> Scenario {
    let [list, opened, menu, boxed, toggled] = names;
    Scenario {
        abilities: Some(revocable()),
        others: Vec::new(),
        setup: chats::events(),
        clicks: vec![
            (opened, MouseButton::Left, chat),
            (menu, MouseButton::Right, message),
            (boxed, MouseButton::Left, delete),
            (toggled, MouseButton::Left, checkbox),
        ],
        drags: Vec::new(),
        drops: Vec::new(),
        scrolls: Vec::new(),
        keys: Vec::new(),
        folders: FoldersPlacement::Top,
        histories: histories(),
        steps: vec![(list, auth(State::Ready))],
    }
}

fn histories() -> Vec<(ChatId, Vec<model::message::Message>)> {
    let mut all = history::histories();
    all.push((
        ChatId(ME),
        vec![history::message(ME, 1, ME, 600, "Packing list")],
    ));
    all
}

pub(crate) fn scenarios() -> Vec<Scenario> {
    vec![
        scenario(
            [
                "221-delete-private",
                "222-delete-private-chat",
                "223-delete-private-menu",
                "224-delete-private-box",
                "225-delete-private-toggled",
            ],
            MARIA_ROW,
            SEATS_MESSAGE,
            (712.0, 385.0),
            (275.0, 300.0),
        ),
        scenario(
            [
                "226-delete-group",
                "227-delete-group-chat",
                "228-delete-group-menu",
                "229-delete-group-box",
                "230-delete-group-toggled",
            ],
            HIKING_ROW,
            GROUP_REPLY,
            (710.0, 372.0),
            (275.0, 300.0),
        ),
        scenario(
            [
                "231-delete-channel",
                "232-delete-channel-chat",
                "233-delete-channel-menu",
                "234-delete-channel-box",
                "235-delete-channel-toggled",
            ],
            LANDSCAPES_ROW,
            (600.0, 300.0),
            (674.0, 390.0),
            (275.0, 300.0),
        ),
        scenario(
            [
                "236-delete-saved",
                "237-delete-saved-chat",
                "238-delete-saved-menu",
                "239-delete-saved-box",
                "240-delete-saved-toggled",
            ],
            SAVED_ROW,
            (600.0, 540.0),
            (674.0, 536.0),
            (275.0, 300.0),
        ),
        // Two of Maria's messages selected, deleted from the selection bar.
        Scenario {
            abilities: Some(revocable()),
            others: Vec::new(),
            setup: chats::events(),
            clicks: vec![
                ("242-delete-selected-chat", MouseButton::Left, MARIA_ROW),
                (
                    "243-delete-selected-menu",
                    MouseButton::Right,
                    SEATS_MESSAGE,
                ),
                ("244-delete-selected-one", MouseButton::Left, (712.0, 419.0)),
                ("245-delete-selected-two", MouseButton::Left, (650.0, 331.0)),
                ("246-delete-selected-box", MouseButton::Left, (450.0, 27.0)),
            ],
            drags: Vec::new(),
            drops: Vec::new(),
            scrolls: Vec::new(),
            keys: vec![("247-delete-selected-closed", vec![Input::Key("escape")])],
            folders: FoldersPlacement::Top,
            histories: histories(),
            steps: vec![("241-delete-selected", auth(State::Ready))],
        },
    ]
}
