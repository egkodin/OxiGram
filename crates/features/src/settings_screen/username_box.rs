//! "Username" (`UsernamesBox` for oneself): the field, a line saying
//! whether the name is free as one types, what a username is for, Save.
// shortcut: no list of collectible usernames to reorder (`UsernamesList`), and the purchase line has no Fragment link; add them with Fragment usernames.

use std::time::Duration;

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement,
    Render, SharedString, Styled, Subscription, Task, WeakEntity, Window, div,
};
use i18n::key;
use model::account::UsernameCheck;
use model::{Session, UserId};
use style::boxes::{ABOUT_REVOKE_PUBLIC_LABEL, USERNAME_PADDING, USERNAME_SKIP};
use style::layers::{BOX_ROW_PADDING, DEFAULT_BOX};
use style::widgets::{BOX_DIVIDER_LABEL, DEFAULT_BOX_DIVIDER_LABEL_PADDING, DEFAULT_INPUT_FIELD};
use style::{ColorId, ColorRef};
use ui::{BoxFrame, FlatLabel, InputEvent, InputField, Markup, RoundButton};

use crate::info::Saved;
use crate::lang::{tr, tr_with};
use crate::layers::Layers;
use crate::{Store, px_of};

/// `Ui::EditPeer::kMinUsernameLength`, `kUsernameCheckTimeout` and
/// `Ui::kMaxUsernameLength`.
const MIN_LENGTH: usize = 5;
const CHECK_DELAY: Duration = Duration::from_millis(200);
const MAX_LENGTH: usize = 32;

/// The line under the field.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Status {
    Default,
    Good(SharedString),
    Error(SharedString),
}

/// What can be said of `name` without asking Telegram
/// (`UsernameEditor::changed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Local {
    Empty,
    BadSymbols,
    TooShort,
    /// Ask Telegram.
    Check,
}

fn local_check(name: &str) -> Local {
    if name.is_empty() {
        return Local::Empty;
    }
    let is_allowed = |(index, c): (usize, char)| {
        c.is_ascii_alphanumeric() || c == '_' || (c == '@' && index == 0)
    };
    if !name.chars().enumerate().all(is_allowed) {
        Local::BadSymbols
    } else if name.chars().count() < MIN_LENGTH {
        Local::TooShort
    } else {
        Local::Check
    }
}

/// The name without the `@` and spaces (`UsernameEditor::getName`).
fn clean(text: &str) -> String {
    text.replace('@', "").trim().to_owned()
}

pub(super) struct UsernameBox {
    me: UserId,
    current: String,
    session: Session,
    field: Entity<InputField>,
    status: Status,
    layers: WeakEntity<Layers>,
    on_saved: Saved,
    checking: Option<Task<()>>,
    _field: Subscription,
}

impl std::fmt::Debug for UsernameBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UsernameBox").finish_non_exhaustive()
    }
}

impl Focusable for UsernameBox {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.field.focus_handle(cx)
    }
}

/// Opens the box over the open layer.
pub(crate) fn show(
    store: &Entity<Store>,
    layers: &Entity<Layers>,
    on_saved: Saved,
    window: &mut Window,
    cx: &mut App,
) {
    let state = store.read(cx).state();
    let Some(me) = state.me else {
        return;
    };
    let current = state
        .chats
        .user(me)
        .map(|user| user.username.clone())
        .unwrap_or_default();
    let session = store.read(cx).session().clone();
    let weak_layers = layers.downgrade();
    let view = cx.new(|cx| {
        let field = cx.new(|cx| {
            let mut field = InputField::new(&DEFAULT_INPUT_FIELD, "@username", window, cx)
                .with_max_length(MAX_LENGTH);
            field.set_text(&current, None, cx);
            field
        });
        let subscription = cx.subscribe_in(
            &field,
            window,
            |this: &mut UsernameBox, _, event: &InputEvent, window, cx| match event {
                InputEvent::Changed => this.changed(cx),
                InputEvent::Submitted => this.save(window, cx),
                _ => {}
            },
        );
        let status = if current.is_empty() {
            Status::Default
        } else {
            Status::Good(tr(cx, key::lng_username_available))
        };
        UsernameBox {
            me,
            current,
            session,
            field,
            status,
            layers: weak_layers,
            on_saved,
            checking: None,
            _field: subscription,
        }
    });
    let field = view.read(cx).field.clone();
    layers.update(cx, |layers, cx| layers.show_above(view, window, cx));
    crate::focus(&field, window, cx);
}

impl UsernameBox {
    fn changed(&mut self, cx: &mut Context<Self>) {
        let name = clean(self.field.read(cx).text());
        self.checking = None;
        self.status = match local_check(&name) {
            Local::Empty => Status::Default,
            Local::BadSymbols => Status::Error(tr(cx, key::lng_username_bad_symbols)),
            Local::TooShort => Status::Error(tr(cx, key::lng_username_too_short)),
            Local::Check => {
                self.schedule_check(name, cx);
                Status::Default
            }
        };
        cx.notify();
    }

    fn schedule_check(&mut self, name: String, cx: &mut Context<Self>) {
        let request = self.session.check_username(self.me, &name);
        let is_current = name == self.current;
        self.checking = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(CHECK_DELAY).await;
            let result = cx.background_spawn(request).await;
            let _ = this.update(cx, |this, cx| {
                this.status = match result {
                    Ok(UsernameCheck::Available) => {
                        Status::Good(tr(cx, key::lng_username_available))
                    }
                    Ok(_) if is_current => Status::Good(tr(cx, key::lng_username_available)),
                    Ok(UsernameCheck::Invalid) => Status::Error(tr(cx, key::lng_username_invalid)),
                    Ok(UsernameCheck::Purchasable) => {
                        let link = tr(cx, key::lng_username_purchase_available_link);
                        let text =
                            tr_with(cx, key::lng_username_purchase_available, &[("link", &link)]);
                        Status::Error(text.into())
                    }
                    Ok(_) => Status::Error(tr(cx, key::lng_username_occupied)),
                    Err(error) => {
                        tracing::warn!(%error, "checking the username failed");
                        Status::Default
                    }
                };
                cx.notify();
            });
        }));
    }

    fn close(&self, window: &mut Window, cx: &mut App) {
        if let Some(layers) = self.layers.upgrade() {
            layers.update(cx, |layers, cx| layers.close(window, cx));
        }
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = clean(self.field.read(cx).text());
        if name == self.current {
            self.close(window, cx);
            return;
        }
        if matches!(local_check(&name), Local::BadSymbols | Local::TooShort) {
            self.field.update(cx, |field, cx| field.show_error(cx));
            return;
        }
        let task = crate::store::spawn(self.session.set_username(name), cx);
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| match result {
                Ok(()) => {
                    (this.on_saved)(cx);
                    this.close(window, cx);
                }
                Err(error) => {
                    tracing::warn!(%error, "saving the username failed");
                    this.status = Status::Error(tr(cx, key::lng_username_occupied));
                    this.field.update(cx, |field, cx| field.show_error(cx));
                    cx.notify();
                }
            });
        })
        .detach();
    }
}

impl Render for UsernameBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (text, fg) = match &self.status {
            Status::Default => (tr(cx, key::lng_username_choose), ColorId::WindowSubTextFg),
            Status::Good(text) => (text.clone(), ColorId::BoxTextFgGood),
            Status::Error(text) => (text.clone(), ColorId::BoxTextFgError),
        };
        let label = &ABOUT_REVOKE_PUBLIC_LABEL;
        let skip = (px_of(USERNAME_SKIP) - ui::font_height(label.style.font)) / 4.0;
        let description = format!(
            "{}\n\n{}",
            tr(cx, key::lng_username_description1),
            tr(cx, key::lng_username_description2)
        );
        let entity = cx.entity();
        let layers = self.layers.clone();
        let padding = USERNAME_PADDING;
        let divider = DEFAULT_BOX_DIVIDER_LABEL_PADDING;
        BoxFrame::new("username-box", &DEFAULT_BOX)
            .title(tr(cx, key::lng_username_title))
            .row(
                div()
                    .pl(px_of(padding.left))
                    .pr(px_of(padding.right))
                    .pt(px_of(padding.top))
                    .child(self.field.clone()),
            )
            .row(
                div().px(px_of(BOX_ROW_PADDING.left)).py(skip * 2.0).child(
                    FlatLabel::new("username-status", label, Markup::plain(text.to_string()))
                        .color(ColorRef::Palette(fg)),
                ),
            )
            .row(
                div()
                    .bg(ui::color(cx, ColorRef::Palette(ColorId::BoxDividerBg)))
                    .pl(px_of(divider.left))
                    .pr(px_of(divider.right))
                    .pt(px_of(divider.top))
                    .pb(px_of(divider.bottom))
                    .child(FlatLabel::new(
                        "username-about",
                        &BOX_DIVIDER_LABEL,
                        Markup::rich(&description),
                    )),
            )
            .button(
                RoundButton::new(
                    "username-save",
                    &DEFAULT_BOX.button,
                    tr(cx, key::lng_settings_save),
                )
                .on_click(move |_, window, cx| {
                    entity.update(cx, |this, cx| this.save(window, cx));
                }),
            )
            .button(
                RoundButton::new(
                    "username-cancel",
                    &DEFAULT_BOX.button,
                    tr(cx, key::lng_cancel),
                )
                .on_click(move |_, window, cx| {
                    if let Some(layers) = layers.upgrade() {
                        layers.update(cx, |layers, cx| layers.close(window, cx));
                    }
                }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_checked_locally_before_telegram() {
        assert_eq!(local_check(""), Local::Empty);
        assert_eq!(local_check("abc"), Local::TooShort);
        assert_eq!(local_check("ab-cd_e"), Local::BadSymbols);
        assert_eq!(local_check("a@bcdef"), Local::BadSymbols);
        assert_eq!(local_check("rgram_test"), Local::Check);
        assert_eq!(clean(" @rgram_test "), "rgram_test");
    }
}
