//! Polls and quizzes in the history (`HistoryView::Poll`): the question,
//! the options to vote for, and the results once voted or closed.

use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, Hsla, InteractiveElement as _, IntoElement as _, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, relative,
};
use i18n::key;
use model::MessageId;
use model::message::{Content, Message};
use model::message_actions::Abilities;
use model::poll::{Poll, PollKind, PollOption};
use style::basic::{NORMAL_FONT, SEMIBOLD_FONT};
use style::chat::{MSG_DATE_FONT, MSG_FILE_MIN_WIDTH, MSG_PADDING};
use style::polls::{
    HISTORY_POLL_ANSWER_PADDING_NO_MEDIA as ANSWER_PADDING, HISTORY_POLL_ANSWERS_SKIP,
    HISTORY_POLL_BOTTOM_BUTTON_SKIP, HISTORY_POLL_BOTTOM_BUTTON_TOP, HISTORY_POLL_CHECKBOX_RADIUS,
    HISTORY_POLL_CHOICE_RIGHT, HISTORY_POLL_CHOICE_WRONG, HISTORY_POLL_FILLING_BG_OPACITY,
    HISTORY_POLL_FILLING_HEIGHT, HISTORY_POLL_FILLING_MIN, HISTORY_POLL_FILLING_RADIUS,
    HISTORY_POLL_FILLING_RIGHT, HISTORY_POLL_FILLING_TOP, HISTORY_POLL_IN_CHOICE_RIGHT,
    HISTORY_POLL_IN_CHOICE_RIGHT_SELECTED, HISTORY_POLL_IN_CHOSEN, HISTORY_POLL_IN_CHOSEN_SELECTED,
    HISTORY_POLL_OUT_CHOICE_RIGHT, HISTORY_POLL_OUT_CHOICE_RIGHT_SELECTED, HISTORY_POLL_OUT_CHOSEN,
    HISTORY_POLL_OUT_CHOSEN_SELECTED, HISTORY_POLL_PERCENT_FONT, HISTORY_POLL_PERCENT_SKIP,
    HISTORY_POLL_PERCENT_TOP, HISTORY_POLL_QUESTION_FONT, HISTORY_POLL_QUESTION_TOP,
    HISTORY_POLL_RADIO, HISTORY_POLL_RADIO_OPACITY, HISTORY_POLL_SUBTITLE_SKIP,
    HISTORY_POLL_TOTAL_VOTES_SKIP,
};
use style::{ColorId, Font, Icon};
use ui::{color, font, font_height, icon, icon_size};

use super::HistoryView;
use super::bubble::{OnClick, palette};
use super::menu::Item;
use crate::boxes::{self, ConfirmArgs};
use crate::lang::{group_digits, tr, tr_plural};
use crate::peer_menu::send;
use crate::px_of;

/// The options picked in multiple-choice polls, until VOTE sends them.
#[derive(Default)]
pub(super) struct Picks(HashMap<MessageId, BTreeSet<i32>>);

pub(super) struct View {
    question: String,
    subtitle: SharedString,
    is_multiple: bool,
    answers: Vec<Answer>,
    /// A quiz's explanation, once answered.
    explanation: Option<(SharedString, String)>,
    footer: Footer,
}

struct Answer {
    text: String,
    state: State,
}

enum State {
    /// Before the results: a click votes, or picks for VOTE.
    Choice {
        is_picked: bool,
        is_sending: bool,
        on_click: OnClick,
    },
    Result {
        percent: i32,
        votes: i32,
        mark: Mark,
    },
}

/// How a result's bar and its mark look (`paintFilling`'s style).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    None,
    /// Chosen in a poll, or a quiz's right answer.
    Chosen,
    /// The user's right answer to a quiz.
    Correct,
    /// The user's wrong answer to a quiz.
    Wrong,
}

enum Footer {
    Label(String),
    /// VOTE, active once something is picked.
    Vote(SharedString, Option<OnClick>),
}

/// "Anonymous Poll", "Quiz", "Final results"...
fn subtitle(poll: &Poll) -> i18n::Key {
    let is_quiz = matches!(poll.kind, PollKind::Quiz { .. });
    match (poll.is_closed, is_quiz, poll.is_anonymous) {
        (true, ..) => key::lng_polls_closed,
        (false, true, true) => key::lng_polls_anonymous_quiz,
        (false, true, false) => key::lng_polls_public_quiz,
        (false, false, true) => key::lng_polls_anonymous,
        (false, false, false) => key::lng_polls_public,
    }
}

/// `Answer::correct` and the style it picks.
fn mark(poll: &Poll, option: &PollOption, out: bool) -> Mark {
    let correct = match &poll.kind {
        PollKind::Quiz { correct, .. } => correct.contains(&option.index),
        PollKind::Regular => option.is_chosen,
    };
    let is_quiz = matches!(poll.kind, PollKind::Quiz { .. });
    match (option.is_chosen, correct) {
        (true, false) => Mark::Wrong,
        (true, true) if is_quiz && !out => Mark::Correct,
        (_, true) => Mark::Chosen,
        (false, false) => Mark::None,
    }
}

/// "3 votes", "No answers"...
fn total(poll: &Poll, cx: &App) -> String {
    let is_quiz = matches!(poll.kind, PollKind::Quiz { .. });
    match (poll.total_voters, is_quiz) {
        (0, true) => tr(cx, key::lng_polls_answers_none).to_string(),
        (0, false) => tr(cx, key::lng_polls_votes_none).to_string(),
        (count, quiz) => {
            let key = if quiz {
                key::lng_polls_answers_count
            } else {
                key::lng_polls_votes_count
            };
            let count = i64::from(count);
            tr_plural(cx, key, count, &[("count", &group_digits(count))])
        }
    }
}

impl HistoryView {
    /// A poll message's question, options or results, and its footer.
    // shortcut: plain text without entities or custom emoji; formatted questions come with polls' media (M8).
    pub(super) fn poll(&self, message: &Message, cx: &Context<Self>) -> Option<View> {
        let Content::Poll(poll) = &message.content else {
            return None;
        };
        let shows_results = poll.shows_results();
        let picks = self.poll_picks.0.get(&message.id);
        let answers = poll
            .options
            .iter()
            .map(|option| Answer {
                text: option.text.text.clone(),
                state: if shows_results {
                    State::Result {
                        percent: option.percentage,
                        votes: option.voters,
                        mark: mark(poll, option, message.is_outgoing),
                    }
                } else {
                    State::Choice {
                        is_picked: picks.is_some_and(|picks| picks.contains(&option.index)),
                        is_sending: option.is_being_chosen,
                        on_click: self.on_answer(message.id, poll, option.index, cx),
                    }
                },
            })
            .collect();
        let footer = if poll.allows_multiple_answers && !shows_results {
            let on_vote = picks
                .filter(|picks| !picks.is_empty())
                .map(|picks| self.on_vote(message.id, picks.iter().copied().collect(), cx));
            Footer::Vote(tr(cx, key::lng_polls_submit_votes), on_vote)
        } else {
            Footer::Label(total(poll, cx))
        };
        let explanation = match &poll.kind {
            PollKind::Quiz { explanation, .. } if shows_results && !explanation.text.is_empty() => {
                Some((
                    tr(cx, key::lng_polls_solution_title),
                    explanation.text.clone(),
                ))
            }
            _ => None,
        };
        Some(View {
            question: poll.question.text.clone(),
            subtitle: tr(cx, subtitle(poll)),
            is_multiple: poll.allows_multiple_answers,
            answers,
            explanation,
            footer,
        })
    }

    /// A vote at once, or a pick for VOTE when several can be chosen.
    fn on_answer(&self, id: MessageId, poll: &Poll, index: i32, cx: &Context<Self>) -> OnClick {
        let weak = cx.weak_entity();
        let is_multiple = poll.allows_multiple_answers;
        Rc::new(move |_, _, cx| {
            let _ = weak.update(cx, |this, cx| {
                if !is_multiple {
                    this.send_votes(id, vec![index], cx);
                    return;
                }
                let picks = this.poll_picks.0.entry(id).or_default();
                if !picks.remove(&index) {
                    picks.insert(index);
                }
                cx.notify();
            });
        })
    }

    fn on_vote(&self, id: MessageId, options: Vec<i32>, cx: &Context<Self>) -> OnClick {
        let weak = cx.weak_entity();
        Rc::new(move |_, _, cx| {
            let options = options.clone();
            let _ = weak.update(cx, |this, cx| {
                this.poll_picks.0.remove(&id);
                this.send_votes(id, options, cx);
            });
        })
    }

    /// Votes for `options`; none takes the vote back.
    pub(super) fn send_votes(&self, id: MessageId, options: Vec<i32>, cx: &mut Context<Self>) {
        let session = self.store.read(cx).session().clone();
        send(
            session.set_poll_answer(self.chat, id, options),
            "poll vote",
            cx,
        );
    }

    /// "If you stop this poll now...", then the poll closes for everyone.
    pub(super) fn confirm_stop_poll(
        &self,
        id: MessageId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut args = ConfirmArgs::new(ui::Markup::plain(
            tr(cx, key::lng_polls_stop_warning).to_string(),
        ));
        args.confirm_text = Some(tr(cx, key::lng_polls_stop_sure));
        args.cancel_text = Some(tr(cx, key::lng_cancel));
        let weak = cx.weak_entity();
        args.confirmed = Some(Rc::new(move |_, cx| {
            let _ = weak.update(cx, |this, cx| {
                let session = this.store.read(cx).session().clone();
                send(session.stop_poll(this.chat, id), "stop poll", cx);
            });
        }));
        boxes::confirm(&self.layers, args, window, cx);
    }
}

/// Retract vote and Stop poll (`AddPollActions`).
pub(super) fn items(message: &Message, abilities: &[Abilities]) -> Vec<Item> {
    let Content::Poll(poll) = &message.content else {
        return Vec::new();
    };
    if poll.is_closed {
        return Vec::new();
    }
    let is_quiz = matches!(poll.kind, PollKind::Quiz { .. });
    let retract =
        (poll.is_voted() && !is_quiz && poll.allows_revoting).then_some(Item::RetractVote);
    // TDLib lets the poll stop where the message can be edited.
    let can_stop = message.is_outgoing && abilities.iter().all(|a| a.can_be_edited);
    let stop = (can_stop && !abilities.is_empty()).then_some(Item::StopPoll);
    retract.into_iter().chain(stop).collect()
}

/// The colors a poll paints with, in or out and selected or not.
pub(super) struct Colors {
    pub(super) text: Hsla,
    pub(super) date: Hsla,
    pub(super) link: Hsla,
    pub(super) file: Hsla,
    pub(super) wrong: Hsla,
    pub(super) good: Hsla,
    /// The check over a chosen option, and the mark over a result.
    pub(super) chosen: Icon,
    pub(super) right: Icon,
}

pub(super) fn colors(out: bool, is_selected: bool, cx: &App) -> Colors {
    use ColorId as C;
    let (text, date, link, file, chosen, right) = match (out, is_selected) {
        (false, false) => (
            C::HistoryTextInFg,
            C::MsgInDateFg,
            C::MsgFileThumbLinkInFg,
            C::MsgFileInBg,
            HISTORY_POLL_IN_CHOSEN,
            HISTORY_POLL_IN_CHOICE_RIGHT,
        ),
        (false, true) => (
            C::HistoryTextInFgSelected,
            C::MsgInDateFgSelected,
            C::MsgFileThumbLinkInFgSelected,
            C::MsgFileInBgSelected,
            HISTORY_POLL_IN_CHOSEN_SELECTED,
            HISTORY_POLL_IN_CHOICE_RIGHT_SELECTED,
        ),
        (true, false) => (
            C::HistoryTextOutFg,
            C::MsgOutDateFg,
            C::MsgFileThumbLinkOutFg,
            C::MsgFileOutBg,
            HISTORY_POLL_OUT_CHOSEN,
            HISTORY_POLL_OUT_CHOICE_RIGHT,
        ),
        (true, true) => (
            C::HistoryTextOutFgSelected,
            C::MsgOutDateFgSelected,
            C::MsgFileThumbLinkOutFgSelected,
            C::MsgFileOutBgSelected,
            HISTORY_POLL_OUT_CHOSEN_SELECTED,
            HISTORY_POLL_OUT_CHOICE_RIGHT_SELECTED,
        ),
    };
    let paint = |id| color(cx, palette(id));
    Colors {
        text: paint(text),
        date: paint(date),
        link: paint(link),
        file: paint(file),
        wrong: paint(C::BoxTextFgError),
        good: paint(C::BoxTextFgGood),
        chosen,
        right,
    }
}

pub(super) fn text(text: impl Into<SharedString>, line_font: Font, fg: Hsla) -> gpui::Div {
    div()
        .font(font(line_font))
        .text_size(px_of(line_font.size))
        .line_height(font_height(line_font))
        .text_color(fg)
        .child(text.into())
}

/// The poll in its bubble, its footer on the time's line.
pub(super) fn element(view: View, id: u64, out: bool, is_selected: bool, cx: &App) -> AnyElement {
    let colors = colors(out, is_selected, cx);
    let inner = MSG_FILE_MIN_WIDTH.0 - MSG_PADDING.left.0 - MSG_PADDING.right.0;
    let explanation = view.explanation.map(|(title, body)| {
        let frame = super::headers::quote_frame(("poll-explanation", id), colors.link, MSG_PADDING);
        frame
            .mt(px_of(HISTORY_POLL_TOTAL_VOTES_SKIP))
            .child(text(title, SEMIBOLD_FONT, colors.link))
            .child(text(body, NORMAL_FONT, colors.text))
    });
    div()
        .flex()
        .flex_col()
        .min_w(px_of(style::Px(inner)))
        .mt(px_of(HISTORY_POLL_QUESTION_TOP) - px_of(MSG_PADDING.top))
        .child(text(view.question, HISTORY_POLL_QUESTION_FONT, colors.text))
        .child(
            text(view.subtitle, MSG_DATE_FONT, colors.date).mt(px_of(HISTORY_POLL_SUBTITLE_SKIP)),
        )
        .child(
            div().mt(px_of(HISTORY_POLL_ANSWERS_SKIP)).children(
                view.answers
                    .into_iter()
                    .enumerate()
                    .map(|(index, answer)| row(answer, (id, index), view.is_multiple, &colors)),
            ),
        )
        .children(explanation)
        .child(footer(view.footer, id, &colors))
        .into_any_element()
}

/// An option: its radio or check before the results, its share after.
// shortcut: a long answer wraps before the vote count, not under it, and the bar keeps one line's room; tdesktop's countVotesExtraHeight lands with polls' media (M8).
fn row(
    answer: Answer,
    (id, index): (u64, usize),
    is_multiple: bool,
    colors: &Colors,
) -> AnyElement {
    let votes = match &answer.state {
        State::Result { votes, .. } if *votes > 0 => Some(group_digits(i64::from(*votes))),
        _ => None,
    };
    let line = div()
        .flex()
        .child(
            text(answer.text, NORMAL_FONT, colors.text)
                .flex_1()
                .min_w_0(),
        )
        .children(votes.map(|votes| {
            text(votes, NORMAL_FONT, colors.date)
                .flex_none()
                .ml(px_of(HISTORY_POLL_PERCENT_SKIP))
                .mr(px_of(HISTORY_POLL_FILLING_RIGHT))
        }));
    let base = div()
        .id((
            "poll-answer",
            id.wrapping_mul(16).wrapping_add(index as u64),
        ))
        .relative()
        .pl(px_of(ANSWER_PADDING.left))
        .pt(px_of(ANSWER_PADDING.top))
        .child(line);
    match answer.state {
        State::Choice {
            is_picked,
            is_sending,
            on_click,
        } => base
            .pb(px_of(ANSWER_PADDING.bottom))
            .cursor_pointer()
            .on_click(move |event, window, cx| on_click(event, window, cx))
            .child(radio(is_picked, is_sending, is_multiple, colors))
            .into_any_element(),
        State::Result { percent, mark, .. } => base
            .child(
                text(
                    format!("{percent}%"),
                    HISTORY_POLL_PERCENT_FONT,
                    colors.text,
                )
                .absolute()
                .left_0()
                .top(px_of(ANSWER_PADDING.top) + px_of(HISTORY_POLL_PERCENT_TOP))
                .w(px_of(ANSWER_PADDING.left) - px_of(HISTORY_POLL_PERCENT_SKIP))
                .flex()
                .justify_end(),
            )
            .child(filling(percent, mark, is_multiple, colors))
            .into_any_element(),
    }
}

/// The radio or check box an option shows before the results.
// shortcut: a vote on its way shows a still ring, not tdesktop's spinning arc; with M8 polish.
pub(super) fn radio(
    is_picked: bool,
    is_sending: bool,
    is_multiple: bool,
    colors: &Colors,
) -> AnyElement {
    let diameter = px_of(HISTORY_POLL_RADIO.diameter);
    let ring = if is_sending {
        colors.link
    } else {
        #[expect(clippy::cast_possible_truncation, reason = "an opacity")]
        let opacity = HISTORY_POLL_RADIO_OPACITY as f32;
        colors.date.opacity(opacity)
    };
    div()
        .absolute()
        .left_0()
        .top(px_of(ANSWER_PADDING.top))
        .size(diameter)
        .flex()
        .items_center()
        .justify_center()
        .map(|this| {
            if is_multiple {
                this.rounded(px_of(HISTORY_POLL_CHECKBOX_RADIUS))
            } else {
                this.rounded_full()
            }
        })
        .map(|this| {
            if is_picked {
                this.bg(colors.link).child(icon(colors.chosen))
            } else {
                this.border(px_of(HISTORY_POLL_RADIO.thickness))
                    .border_color(ring)
            }
        })
        .into_any_element()
}

/// The result's bar under its text, with the mark of a chosen or right
/// option before it.
fn filling(percent: i32, mark: Mark, is_multiple: bool, colors: &Colors) -> AnyElement {
    let fill = match mark {
        Mark::Wrong => colors.wrong,
        Mark::Correct => colors.good,
        Mark::None | Mark::Chosen => colors.file,
    };
    let thickness = px_of(HISTORY_POLL_FILLING_HEIGHT);
    let radius = px_of(HISTORY_POLL_FILLING_RADIUS);
    let mark_icon = match mark {
        Mark::None => None,
        Mark::Chosen => Some(colors.right),
        Mark::Correct => Some(HISTORY_POLL_CHOICE_RIGHT),
        Mark::Wrong => Some(HISTORY_POLL_CHOICE_WRONG),
    };
    let mark = mark_icon.map(|mark_icon| {
        let size = icon_size(mark_icon);
        div()
            .absolute()
            .left(-(px_of(HISTORY_POLL_PERCENT_SKIP) + size.width))
            .top((thickness - size.height) / 2.0)
            .w(size.width)
            .h(size.height)
            .map(|this| {
                if is_multiple {
                    this.rounded(px_of(HISTORY_POLL_CHECKBOX_RADIUS))
                } else {
                    this.rounded_full()
                }
            })
            .bg(fill)
            .child(icon(mark_icon))
    });
    #[expect(clippy::cast_precision_loss, reason = "a percentage")]
    let share = percent.clamp(0, 100) as f32 / 100.0;
    #[expect(clippy::cast_possible_truncation, reason = "an opacity")]
    let opacity = HISTORY_POLL_FILLING_BG_OPACITY as f32;
    div()
        .relative()
        .mt(px_of(HISTORY_POLL_FILLING_TOP))
        .mr(px_of(HISTORY_POLL_FILLING_RIGHT))
        .h(thickness)
        .rounded(radius)
        .bg(fill.opacity(opacity))
        .child(
            div()
                .h_full()
                .rounded(radius)
                .min_w(px_of(HISTORY_POLL_FILLING_MIN))
                .w(relative(share))
                .bg(fill),
        )
        .children(mark)
        .into_any_element()
}

/// The total votes, or VOTE, centered on the time's line.
// shortcut: no extra line when the text would run into the time, as tdesktop's dateInfoSkip adds; a total that long is rare.
fn footer(footer: Footer, id: u64, colors: &Colors) -> AnyElement {
    let top =
        HISTORY_POLL_TOTAL_VOTES_SKIP.0 + MSG_PADDING.bottom.0 + HISTORY_POLL_BOTTOM_BUTTON_TOP.0;
    // The bubble's own bottom padding makes up the rest.
    let bottom =
        HISTORY_POLL_BOTTOM_BUTTON_SKIP.0 - HISTORY_POLL_BOTTOM_BUTTON_TOP.0 - MSG_PADDING.bottom.0;
    let line = div()
        .id(("poll-footer", id))
        .flex()
        .justify_center()
        .pt(px_of(style::Px(top)))
        .pb(px_of(style::Px(bottom.max(0))));
    match footer {
        Footer::Label(total) => line.child(text(total, MSG_DATE_FONT, colors.date)),
        Footer::Vote(label, on_click) => {
            let fg = if on_click.is_some() {
                colors.link
            } else {
                colors.date
            };
            line.child(text(label, SEMIBOLD_FONT, fg))
                .when_some(on_click, |this, on_click| {
                    this.cursor_pointer()
                        .on_click(move |event, window, cx| on_click(event, window, cx))
                })
        }
    }
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use model::message::Sender;
    use model::text::FormattedText;
    use model::{ChatId, UserId};

    use super::*;

    fn option(index: i32, is_chosen: bool) -> PollOption {
        PollOption {
            index,
            text: FormattedText::plain("a"),
            voters: 1,
            percentage: 50,
            is_chosen,
            is_being_chosen: false,
        }
    }

    fn poll(kind: PollKind, chosen: i32) -> Poll {
        Poll {
            id: 1,
            question: FormattedText::plain("Q"),
            options: vec![option(0, chosen == 0), option(1, chosen == 1)],
            total_voters: 2,
            is_anonymous: true,
            allows_multiple_answers: false,
            allows_revoting: true,
            can_see_results: true,
            is_closed: false,
            close_date: 0,
            kind,
        }
    }

    fn quiz() -> PollKind {
        PollKind::Quiz {
            correct: vec![1],
            explanation: FormattedText::plain(""),
        }
    }

    #[test]
    fn quiz_marks_the_wrong_pick_and_the_right_answer() {
        let wrong = poll(quiz(), 0);
        assert_eq!(mark(&wrong, &wrong.options[0], false), Mark::Wrong);
        assert_eq!(mark(&wrong, &wrong.options[1], false), Mark::Chosen);
        let right = poll(quiz(), 1);
        assert_eq!(mark(&right, &right.options[1], false), Mark::Correct);
        // Outgoing bubbles keep the plain mark (`!context.outbg`).
        assert_eq!(mark(&right, &right.options[1], true), Mark::Chosen);
        let regular = poll(PollKind::Regular, 1);
        assert_eq!(mark(&regular, &regular.options[0], false), Mark::None);
        assert_eq!(mark(&regular, &regular.options[1], false), Mark::Chosen);
    }

    #[test]
    fn menu_offers_retract_for_a_voted_poll_and_stop_to_its_author() {
        let message = |poll: Poll, is_outgoing: bool| Message {
            is_outgoing,
            ..Message::new(
                MessageId(1),
                ChatId(1),
                Sender::User(UserId(1)),
                Content::Poll(Box::new(poll)),
            )
        };
        let editable = [Abilities {
            can_be_edited: true,
            ..Abilities::default()
        }];
        let voted = message(poll(PollKind::Regular, 1), true);
        assert_eq!(
            items(&voted, &editable),
            [Item::RetractVote, Item::StopPoll]
        );
        assert_eq!(items(&voted, &[]), [Item::RetractVote]);
        // A quiz answer is final.
        assert!(items(&message(poll(quiz(), 1), false), &editable).is_empty());
    }
}
