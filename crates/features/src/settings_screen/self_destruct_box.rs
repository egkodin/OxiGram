//! "If away for" and "If inactive for" (`SelfDestructionBox`): how long the
//! account, or an unused session, lives without the owner.
// shortcut: no ⋮ menu with "Delete account" on the account's box; add it with a confirmation flow the owner agrees to test.

use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    ParentElement, Render, Styled, Window, div,
};
use i18n::key;
use style::layers::{
    BOX_LABEL, BOX_MEDIUM_SKIP, BOX_OPTION_LIST_SKIP, BOX_PADDING, DEFAULT_BOX,
    DEFAULT_BOX_CHECKBOX,
};
use style::widgets::DEFAULT_RADIO;
use ui::{BoxFrame, Checkbox, Checked, FlatLabel, Markup, RoundButton};

use super::{Env, rows};
use crate::lang::{tr, tr_plural};
use crate::px_of;

/// What the box sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Account,
    Sessions,
}

impl Kind {
    /// `SelfDestructionBox::Values`.
    fn days(self) -> &'static [i32] {
        match self {
            Self::Account => &[30, 90, 180, 365, 548, 720],
            Self::Sessions => &[7, 30, 90, 180, 365],
        }
    }
}

pub(super) type OnSaved = Rc<dyn Fn(i32, &mut App)>;

/// `SelfDestructionBox::DaysLabel`.
pub(super) fn days_label(days: i32, cx: &App) -> String {
    match days {
        0 => String::new(),
        days if days > 25 => tr_plural(cx, key::lng_months, i64::from((days / 30).max(1)), &[]),
        days => tr_plural(cx, key::lng_weeks, i64::from((days / 7).max(1)), &[]),
    }
}

/// The choice nearest to `days` (`gotCurrent`).
fn nearest(kind: Kind, days: i32) -> i32 {
    kind.days()
        .iter()
        .copied()
        .min_by_key(|value| (days - value).abs())
        .unwrap_or(days)
}

pub(super) struct SelfDestructBox {
    env: Env,
    kind: Kind,
    chosen: i32,
    on_saved: OnSaved,
    focus: FocusHandle,
}

impl std::fmt::Debug for SelfDestructBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SelfDestructBox")
            .field("kind", &self.kind)
            .field("chosen", &self.chosen)
            .finish_non_exhaustive()
    }
}

impl Focusable for SelfDestructBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

pub(super) fn show(
    env: &Env,
    kind: Kind,
    current: i32,
    on_saved: OnSaved,
    window: &mut Window,
    cx: &mut App,
) {
    let view = cx.new(|cx| SelfDestructBox {
        env: env.clone(),
        kind,
        chosen: nearest(kind, current),
        on_saved,
        focus: cx.focus_handle(),
    });
    env.targets
        .layers
        .update(cx, |layers, cx| layers.show_above(view, window, cx));
}

impl SelfDestructBox {
    fn close(&self, window: &mut Window, cx: &mut App) {
        self.env
            .targets
            .layers
            .update(cx, |layers, cx| layers.close(window, cx));
    }

    fn save(&self, window: &mut Window, cx: &mut App) {
        let days = self.chosen;
        let session = self.env.targets.store.read(cx).session().clone();
        let task = match self.kind {
            Kind::Account => crate::store::spawn(session.set_account_ttl_days(days), cx),
            Kind::Sessions => crate::store::spawn(session.set_inactive_session_ttl(days), cx),
        };
        let on_saved = self.on_saved.clone();
        cx.spawn(async move |cx| match task.await {
            Ok(()) => cx.update(|cx| on_saved(days, cx)),
            Err(error) => tracing::warn!(%error, "saving a self-destruct period failed"),
        })
        .detach();
        self.close(window, cx);
    }
}

impl Render for SelfDestructBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (title, about) = match self.kind {
            Kind::Account => (
                key::lng_self_destruct_title,
                key::lng_self_destruct_description,
            ),
            Kind::Sessions => (
                key::lng_self_destruct_sessions_title,
                key::lng_self_destruct_sessions_description,
            ),
        };
        let radios: Vec<_> = self
            .kind
            .days()
            .iter()
            .map(|days| {
                let days = *days;
                let checked = if days == self.chosen {
                    Checked::Yes
                } else {
                    Checked::No
                };
                let weak = cx.weak_entity();
                Checkbox::radio(
                    (
                        "self-destruct-days",
                        usize::try_from(days).unwrap_or_default(),
                    ),
                    &DEFAULT_BOX_CHECKBOX,
                    &DEFAULT_RADIO,
                    days_label(days, cx),
                    checked,
                )
                .on_toggle(move |_, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.chosen = days;
                        cx.notify();
                    });
                })
            })
            .collect();
        let content = div()
            .flex()
            .flex_col()
            .pl(px_of(BOX_PADDING.left))
            .pr(px_of(BOX_PADDING.right))
            .pb(px_of(BOX_PADDING.bottom))
            .gap(px_of(BOX_OPTION_LIST_SKIP))
            .child(
                div()
                    .pb(px_of(BOX_MEDIUM_SKIP) - px_of(BOX_OPTION_LIST_SKIP))
                    .child(FlatLabel::new(
                        "self-destruct-about",
                        &BOX_LABEL,
                        Markup::plain(tr(cx, about).to_string()),
                    )),
            )
            .children(radios);
        let (save, cancel) = (cx.weak_entity(), cx.weak_entity());
        div().track_focus(&self.focus).child(
            BoxFrame::new("self-destruct-box", &DEFAULT_BOX)
                .title(tr(cx, title))
                .row(content)
                .row(rows::skip_by(style::Px(0)))
                .button(
                    RoundButton::new(
                        "self-destruct-save",
                        &DEFAULT_BOX.button,
                        tr(cx, key::lng_settings_save),
                    )
                    .on_click(move |_, window, cx| {
                        let _ = save.update(cx, |this, cx| this.save(window, cx));
                    }),
                )
                .button(
                    RoundButton::new(
                        "self-destruct-cancel",
                        &DEFAULT_BOX.button,
                        tr(cx, key::lng_cancel),
                    )
                    .on_click(move |_, window, cx| {
                        let _ = cancel.update(cx, |this, cx| this.close(window, cx));
                    }),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_period_snaps_to_the_nearest_choice() {
        assert_eq!(nearest(Kind::Account, 183), 180);
        assert_eq!(nearest(Kind::Account, 1), 30);
        assert_eq!(nearest(Kind::Sessions, 1000), 365);
        assert_eq!(nearest(Kind::Sessions, 8), 7);
    }
}
