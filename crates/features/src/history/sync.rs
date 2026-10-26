//! Keeps the list's items in line with the history without moving what
//! the user looks at: items are spliced in and out, never reset, so the
//! list's scroll anchor stays on the same message.

use std::ops::Range;
use std::sync::Arc;

use gpui::ListState;
use model::message::Message;

/// One change to the items: `old` replaced by `count` new ones.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Splice {
    old: Range<usize>,
    count: usize,
}

/// The splices that turn `old` into `new`, both sorted by id, and the new
/// indices of messages that changed in place.
fn diff(old: &[Arc<Message>], new: &[Arc<Message>]) -> (Vec<Splice>, Vec<usize>) {
    let (mut splices, mut changed) = (Vec::new(), Vec::new());
    let (mut i, mut j) = (0, 0);
    let mut pending: Option<Splice> = None;
    while i < old.len() || j < new.len() {
        match (old.get(i), new.get(j)) {
            (Some(a), Some(b)) if a.id == b.id => {
                splices.extend(pending.take());
                if !Arc::ptr_eq(a, b) {
                    changed.push(j);
                }
                i += 1;
                j += 1;
            }
            (Some(a), b) if b.is_none_or(|b| a.id < b.id) => {
                let splice = pending.get_or_insert(Splice {
                    old: i..i,
                    count: 0,
                });
                splice.old.end = i + 1;
                i += 1;
            }
            _ => {
                pending
                    .get_or_insert(Splice {
                        old: i..i,
                        count: 0,
                    })
                    .count += 1;
                j += 1;
            }
        }
    }
    splices.extend(pending);
    (splices, changed)
}

/// Applies the difference to `list`; whether anything changed.
pub(super) fn apply(list: &ListState, old: &[Arc<Message>], new: &[Arc<Message>]) -> bool {
    let (splices, changed) = diff(old, new);
    // From the end, so earlier ranges keep their indices.
    for splice in splices.iter().rev() {
        list.splice(splice.old.clone(), splice.count);
    }
    for index in &changed {
        list.remeasure_items(*index..*index + 1);
    }
    !splices.is_empty() || !changed.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::message::{Content, Sender};
    use model::{ChatId, MessageId, UserId};

    fn messages(ids: &[i64]) -> Vec<Arc<Message>> {
        ids.iter()
            .map(|&id| {
                Arc::new(Message::new(
                    MessageId(id),
                    ChatId(1),
                    Sender::User(UserId(1)),
                    Content::Unsupported,
                ))
            })
            .collect()
    }

    #[test]
    fn prepends_appends_and_removals_become_splices() {
        let old = messages(&[3, 4, 5, 7]);
        let mut new = messages(&[1, 2]);
        new.extend([old[0].clone(), old[2].clone(), old[3].clone()]);
        new.extend(messages(&[8]));
        let (splices, changed) = diff(&old, &new);
        assert_eq!(
            splices,
            [
                Splice {
                    old: 0..0,
                    count: 2
                },
                Splice {
                    old: 1..2,
                    count: 0
                },
                Splice {
                    old: 4..4,
                    count: 1
                },
            ]
        );
        assert!(changed.is_empty());
        let list = ListState::new(old.len(), gpui::ListAlignment::Bottom, gpui::px(0.0));
        assert!(apply(&list, &old, &new));
        assert_eq!(list.item_count(), new.len());
    }

    #[test]
    fn an_edited_message_is_remeasured_in_place() {
        let old = messages(&[1, 2]);
        let new = vec![old[0].clone(), messages(&[2])[0].clone()];
        assert_eq!(diff(&old, &new), (Vec::new(), vec![1]));
        assert_eq!(diff(&old, &old), (Vec::new(), Vec::new()));
    }
}
