//! Checklists in the history (`HistoryView::TodoList`): the title, each
//! task with its check and who did it, and how many are done.

use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement as _, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, div, px,
};
use i18n::key;
use model::MessageId;
use model::chat::ChatKind;
use model::checklist::{Checklist, Mark};
use model::message::{Content, Message};
use style::basic::NORMAL_FONT;
use style::chat::{
    HISTORY_CHECKLIST_BOTTOM_TOP, HISTORY_CHECKLIST_CHECKED_TOP, HISTORY_CHECKLIST_TASK_PADDING,
    MEDIA_UNREAD_SIZE, MSG_DATE_FONT, MSG_FILE_MIN_WIDTH, MSG_PADDING,
};
use style::polls::{
    HISTORY_POLL_ANSWERS_SKIP, HISTORY_POLL_BOTTOM_BUTTON_SKIP, HISTORY_POLL_CHOICE_RIGHT,
    HISTORY_POLL_FILLING_BOTTOM, HISTORY_POLL_FILLING_HEIGHT, HISTORY_POLL_QUESTION_FONT,
    HISTORY_POLL_QUESTION_TOP, HISTORY_POLL_RADIO, HISTORY_POLL_SUBTITLE_SKIP,
    HISTORY_POLL_TOTAL_VOTES_SKIP,
};
use style::{ColorId, Px};
use ui::{font_height, icon, icon_size};

use super::HistoryView;
use super::bubble::{OnClick, palette};
use super::poll::{Colors, colors, radio, text};
use crate::lang::{tr, tr_plural, tr_with};
use crate::peer_menu::send;
use crate::px_of;

pub(super) struct View {
    title: String,
    subtitle: SharedString,
    tasks: Vec<Task>,
    /// "2 of 5 completed".
    status: String,
}

struct Task {
    text: String,
    /// Who did it, once done.
    done_by: Option<String>,
    /// A click marks it, when the user may.
    on_click: Option<OnClick>,
}

impl HistoryView {
    /// A checklist message's tasks and status.
    // shortcut: plain text, no completer's userpic beside the check and no fireworks; with M8 polish.
    pub(super) fn checklist(&self, message: &Message, cx: &Context<Self>) -> Option<View> {
        let Content::Checklist(list) = &message.content else {
            return None;
        };
        let chats = &self.store.read(cx).state().chats;
        let is_private = matches!(
            chats.chat(self.chat).map(|chat| &chat.kind),
            Some(ChatKind::Private { .. } | ChatKind::Secret { .. })
        );
        let subtitle = match (list.others_can_mark_tasks_as_done, is_private) {
            (false, _) => key::lng_todo_title,
            (true, true) => key::lng_todo_title_user,
            (true, false) => key::lng_todo_title_group,
        };
        let tasks = list
            .tasks
            .iter()
            .map(|task| Task {
                text: task.text.text.clone(),
                done_by: task.completed_by.map(|sender| {
                    super::sender_face(chats, sender).map_or_else(String::new, |face| face.0)
                }),
                on_click: list.can_mark_tasks_as_done.then(|| {
                    let mark = if task.completed_by.is_some() {
                        Mark::NotDone
                    } else {
                        Mark::Done
                    };
                    self.on_task(message.id, task.id, mark, cx)
                }),
            })
            .collect();
        Some(View {
            title: list.title.text.clone(),
            subtitle: tr(cx, subtitle),
            tasks,
            status: status(list, cx),
        })
    }

    fn on_task(&self, message: MessageId, id: i32, mark: Mark, cx: &Context<Self>) -> OnClick {
        let weak = cx.weak_entity();
        Rc::new(move |_, _, cx| {
            let _ = weak.update(cx, |this, cx| {
                let session = this.store.read(cx).session().clone();
                let task = session.mark_checklist_task(this.chat, message, id, mark);
                send(task, "checklist task", cx);
            });
        })
    }
}

/// "2 of 5 completed", or "None of 5 completed".
fn status(list: &Checklist, cx: &App) -> String {
    let total = list.tasks.len().to_string();
    match list.done_count() {
        0 => tr_with(cx, key::lng_todo_completed_none, &[("total", &total)]),
        done => {
            let count = i64::try_from(done).unwrap_or(i64::MAX);
            let done = done.to_string();
            tr_plural(
                cx,
                key::lng_todo_completed,
                count,
                &[("count", &done), ("total", &total)],
            )
        }
    }
}

/// The checklist in its bubble, the status on the time's line.
pub(super) fn element(view: View, id: u64, out: bool, is_selected: bool, cx: &App) -> AnyElement {
    let colors = colors(out, is_selected, cx);
    let file = match (out, is_selected) {
        (false, false) => ColorId::MsgFileInBg,
        (false, true) => ColorId::MsgFileInBgSelected,
        (true, false) => ColorId::MsgFileOutBg,
        (true, true) => ColorId::MsgFileOutBgSelected,
    };
    let inner = MSG_FILE_MIN_WIDTH.0 - MSG_PADDING.left.0 - MSG_PADDING.right.0;
    // `bottomButtonHeight`: the status sits high in it; the bubble's own
    // bottom padding makes up the rest.
    let top = MSG_PADDING.bottom.0 + HISTORY_CHECKLIST_BOTTOM_TOP.0;
    let choice = icon_size(HISTORY_POLL_CHOICE_RIGHT).height;
    let filling = px_of(HISTORY_POLL_FILLING_HEIGHT);
    let skip = choice - px_of(HISTORY_POLL_FILLING_BOTTOM) - filling - (choice - filling) / 2.0;
    let bottom = px_of(Px(HISTORY_POLL_TOTAL_VOTES_SKIP.0
        + HISTORY_POLL_BOTTOM_BUTTON_SKIP.0
        - top))
        + skip;
    div()
        .flex()
        .flex_col()
        .min_w(px_of(Px(inner)))
        .mt(px_of(HISTORY_POLL_QUESTION_TOP) - px_of(MSG_PADDING.top))
        .child(text(view.title, HISTORY_POLL_QUESTION_FONT, colors.text))
        .child(
            text(view.subtitle, MSG_DATE_FONT, colors.date).mt(px_of(HISTORY_POLL_SUBTITLE_SKIP)),
        )
        .child(
            div().mt(px_of(HISTORY_POLL_ANSWERS_SKIP)).children(
                view.tasks
                    .into_iter()
                    .enumerate()
                    .map(|(index, task)| row(task, (id, index), file, &colors)),
            ),
        )
        .child(
            text(view.status, MSG_DATE_FONT, colors.date)
                .flex()
                .justify_center()
                .pt(px_of(Px(top)))
                .pb(bottom),
        )
        .into_any_element()
}

/// A task: its check, or its state when the user may not mark it.
fn row(task: Task, (id, index): (u64, usize), file: ColorId, colors: &Colors) -> AnyElement {
    let padding = HISTORY_CHECKLIST_TASK_PADDING;
    let is_done = task.done_by.is_some();
    let check = match task.on_click {
        // The radio sits at the answer's top; the task's is lower.
        Some(_) => div()
            .absolute()
            .left_0()
            .top(px_of(padding.top) - px_of(style::polls::HISTORY_POLL_ANSWER_PADDING_NO_MEDIA.top))
            .child(radio(is_done, false, false, colors)),
        None => status_mark(is_done, file, colors.file),
    };
    let name_height = font_height(NORMAL_FONT);
    div()
        .id((
            "checklist-task",
            id.wrapping_mul(64).wrapping_add(index as u64),
        ))
        .relative()
        .pl(px_of(padding.left))
        .map(|this| {
            if is_done {
                // The completer's name under the text (`paintTask`).
                let rest = px_of(padding.top) + px_of(padding.bottom)
                    - px_of(HISTORY_CHECKLIST_CHECKED_TOP)
                    - name_height;
                this.pt(px_of(HISTORY_CHECKLIST_CHECKED_TOP))
                    .pb(rest.max(px(0.0)))
            } else {
                this.pt(px_of(padding.top)).pb(px_of(padding.bottom))
            }
        })
        .child(text(task.text, NORMAL_FONT, colors.text))
        .when_some(task.done_by, |this, name| {
            this.child(text(name, NORMAL_FONT, colors.date).truncate())
        })
        .child(check)
        .when_some(task.on_click, |this, on_click| {
            this.cursor_pointer()
                .on_click(move |event, window, cx| on_click(event, window, cx))
        })
        .into_any_element()
}

/// A done task's check, or a dot, in the file color (`paintStatus`).
fn status_mark(is_done: bool, file: ColorId, dot: gpui::Hsla) -> gpui::Div {
    let diameter = px_of(HISTORY_POLL_RADIO.diameter);
    let mark = if is_done {
        icon(style::polls::HISTORY_POLL_IN_CHOSEN)
            .color(palette(file))
            .into_any_element()
    } else {
        div()
            .size(px_of(MEDIA_UNREAD_SIZE))
            .rounded_full()
            .bg(dot)
            .into_any_element()
    };
    div()
        .absolute()
        .left_0()
        .top(px_of(HISTORY_CHECKLIST_TASK_PADDING.top))
        .size(diameter)
        .flex()
        .items_center()
        .justify_center()
        .child(mark)
}
