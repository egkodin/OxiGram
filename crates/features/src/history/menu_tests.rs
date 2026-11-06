//! Unit tests of the message menu's items.

use super::*;

const ALL: Abilities = Abilities {
    can_be_copied: true,
    can_be_deleted_only_for_self: true,
    can_be_deleted_for_all_users: true,
    can_get_link: true,
    can_be_pinned: true,
    can_be_replied: true,
    can_be_edited: true,
    can_be_forwarded: true,
    can_get_read_date: false,
    can_get_viewers: false,
};

#[test]
fn a_plain_message_offers_copy_link_delete_and_select() {
    assert_eq!(
        items(Upon::Nothing, true, None, &[ALL]),
        [
            Item::Reply,
            Item::Edit,
            Item::CopyText,
            Item::CopyMessageLink,
            Item::Forward,
            Item::Delete,
            Item::Select
        ]
    );
}

#[test]
fn a_link_under_the_pointer_replaces_the_message_link() {
    let link = LinkCopy::Hashtag("#trail".to_owned());
    assert_eq!(
        items(Upon::Nothing, true, Some(link.clone()), &[ALL]),
        [
            Item::Reply,
            Item::Edit,
            Item::CopyText,
            Item::CopyLink(link),
            Item::Forward,
            Item::Delete,
            Item::Select
        ]
    );
}

#[test]
fn unknown_abilities_offer_only_what_needs_no_permission() {
    assert_eq!(
        items(Upon::Nothing, true, None, &[]),
        [Item::CopyText, Item::Select]
    );
}

#[test]
fn protected_content_is_not_copied() {
    let protected = Abilities {
        can_be_copied: false,
        ..ALL
    };
    assert_eq!(
        items(Upon::SelectedText, true, None, &[protected]),
        [
            Item::Reply,
            Item::Edit,
            Item::CopyMessageLink,
            Item::Forward,
            Item::Delete,
            Item::Select
        ]
    );
}

#[test]
fn selected_messages_delete_only_when_all_can() {
    let kept = Abilities {
        can_be_deleted_only_for_self: false,
        can_be_deleted_for_all_users: false,
        ..ALL
    };
    assert_eq!(
        items(Upon::SelectedMessage, true, None, &[ALL, kept]),
        [
            Item::CopySelectedMessages,
            Item::ForwardSelected,
            Item::ClearSelection
        ]
    );
    assert_eq!(
        items(Upon::OtherMessages, true, None, &[ALL]),
        [Item::Select, Item::SelectUpTo]
    );
}
