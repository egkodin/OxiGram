//! "Messages" privacy (`EditMessagesPrivacyBox`): everyone may write first,
//! or only contacts and Premium users.
// shortcut: no "Charge for messages" choice with its price slider and exceptions; a fee set elsewhere shows as a third choice that Save keeps. Add them with Stars (M13). With row "Messages privacy".

use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    ParentElement, Render, Styled, Task, Window, div, px,
};
use i18n::{Key, key};
use model::privacy::NewChatPrivacy;
use style::layers::{BOX_WIDE_WIDTH, DEFAULT_BOX};
use style::menu_icons::MENU_BLUE_ICON_PREMIUM;
use style::settings::{
    MESSAGE_PRIVACY_BOTTOM_SKIP, MESSAGE_PRIVACY_CHECK, MESSAGE_PRIVACY_RADIO_SKIP,
    MESSAGE_PRIVACY_SUBSCRIBE, MESSAGE_PRIVACY_TOP_SKIP, SETTINGS_SEND_TYPE_PADDING,
};
use style::widgets::{DEFAULT_RADIO, DEFAULT_TOAST};
use ui::{BoxFrame, Checkbox, Checked, RoundButton, SettingsButton};

use super::{Env, PageId, rows};
use crate::lang::{tr, tr_with};
use crate::px_of;

/// TDLib's flag: free accounts may restrict new chats too.
const CAN_SET: &str = "can_set_new_chat_privacy_settings";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    Everyone,
    /// Contacts and Premium users.
    Restricted,
    /// A fee, set in another app.
    Charge,
}

impl Choice {
    fn of(privacy: NewChatPrivacy) -> Self {
        if privacy.paid_stars > 0 {
            Self::Charge
        } else if privacy.allow_unknown {
            Self::Everyone
        } else {
            Self::Restricted
        }
    }

    fn text(self) -> Key {
        match self {
            Self::Everyone => key::lng_messages_privacy_everyone,
            Self::Restricted => key::lng_messages_privacy_restricted,
            Self::Charge => key::lng_messages_privacy_charge,
        }
    }

    /// The settings it saves; a fee keeps its price.
    fn privacy(self, current: NewChatPrivacy) -> NewChatPrivacy {
        match self {
            Self::Everyone => NewChatPrivacy {
                allow_unknown: true,
                paid_stars: 0,
            },
            Self::Restricted => NewChatPrivacy {
                allow_unknown: false,
                paid_stars: 0,
            },
            Self::Charge => current,
        }
    }
}

pub(super) type OnSaved = Rc<dyn Fn(NewChatPrivacy, &mut App)>;

pub(super) struct MessagesBox {
    env: Env,
    current: NewChatPrivacy,
    chosen: Choice,
    /// None while TDLib is asked; false locks the choices behind Premium.
    is_allowed: Option<bool>,
    on_saved: OnSaved,
    focus: FocusHandle,
    _load: Task<()>,
}

impl Focusable for MessagesBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

pub(super) fn show(
    env: &Env,
    current: NewChatPrivacy,
    on_saved: OnSaved,
    window: &mut Window,
    cx: &mut App,
) {
    let view = cx.new(|cx: &mut Context<MessagesBox>| {
        let task = crate::store::spawn(
            env.targets.store.read(cx).session().bool_option(CAN_SET),
            cx,
        );
        let load = cx.spawn(async move |this, cx| {
            let is_allowed = task.await.unwrap_or_else(|error| {
                tracing::warn!(%error, "reading whether new chats can be restricted failed");
                false
            });
            let _ = this.update(cx, |this, cx| {
                this.is_allowed = Some(is_allowed);
                if !is_allowed {
                    this.chosen = Choice::Everyone;
                }
                cx.notify();
            });
        });
        MessagesBox {
            env: env.clone(),
            current,
            chosen: Choice::of(current),
            is_allowed: None,
            on_saved,
            focus: cx.focus_handle(),
            _load: load,
        }
    });
    env.targets
        .layers
        .update(cx, |layers, cx| layers.show_above(view, window, cx));
}

impl MessagesBox {
    fn close(&self, window: &mut Window, cx: &mut App) {
        self.env
            .targets
            .layers
            .update(cx, |layers, cx| layers.close(window, cx));
    }

    /// A locked choice explains Premium instead (`showToast`).
    fn choose(&mut self, choice: Choice, cx: &mut Context<Self>) {
        if self.is_allowed == Some(false) && choice != Choice::Everyone {
            let link = tr(cx, key::lng_messages_privacy_premium_link);
            let text = tr_with(cx, key::lng_messages_privacy_premium, &[("link", &link)]).into();
            self.env.targets.layers.update(cx, |layers, cx| {
                layers.show_toast(text, &DEFAULT_TOAST, crate::toast::DEFAULT_DURATION, cx);
            });
            return;
        }
        self.chosen = choice;
        cx.notify();
    }

    fn save(&self, window: &mut Window, cx: &mut App) {
        let privacy = self.chosen.privacy(self.current);
        let task = crate::store::spawn(
            self.env
                .targets
                .store
                .read(cx)
                .session()
                .set_new_chat_privacy(privacy),
            cx,
        );
        let on_saved = self.on_saved.clone();
        cx.spawn(async move |cx| match task.await {
            Ok(()) => cx.update(|cx| on_saved(privacy, cx)),
            Err(error) => tracing::warn!(%error, "saving the messages privacy failed"),
        })
        .detach();
        self.close(window, cx);
    }

    fn radio(&self, choice: Choice, cx: &mut Context<Self>) -> Checkbox {
        let checked = if self.chosen == choice {
            Checked::Yes
        } else {
            Checked::No
        };
        let weak = cx.weak_entity();
        Checkbox::radio(
            ("messages-privacy", choice as usize),
            &MESSAGE_PRIVACY_CHECK,
            &DEFAULT_RADIO,
            tr(cx, choice.text()),
            checked,
        )
        .on_toggle(move |_, cx| {
            let _ = weak.update(cx, |this, cx| this.choose(choice, cx));
        })
    }

    fn choices(&self, cx: &mut Context<Self>) -> gpui::Div {
        let padding = SETTINGS_SEND_TYPE_PADDING;
        let row = |radio: Checkbox, top: style::Px, bottom: style::Px| {
            div()
                .pl(px_of(padding.left) - px(1.0))
                .pr(px_of(padding.right))
                .pt(px_of(padding.top) + px_of(top))
                .pb(px_of(padding.bottom) + px_of(bottom))
                .child(radio)
        };
        let none = style::Px(0);
        let mut column = div()
            .flex()
            .flex_col()
            .child(rows::skip_by(MESSAGE_PRIVACY_TOP_SKIP))
            .child(rows::subtitle(
                tr(cx, key::lng_messages_privacy_subtitle),
                cx,
            ))
            .child(row(self.radio(Choice::Everyone, cx), none, none))
            .child(row(
                self.radio(Choice::Restricted, cx),
                MESSAGE_PRIVACY_RADIO_SKIP,
                MESSAGE_PRIVACY_BOTTOM_SKIP,
            ))
            .child(rows::divider_text(
                tr(cx, key::lng_messages_privacy_about),
                cx,
            ));
        if self.current.paid_stars > 0 {
            column = column
                .child(row(
                    self.radio(Choice::Charge, cx),
                    MESSAGE_PRIVACY_BOTTOM_SKIP,
                    MESSAGE_PRIVACY_BOTTOM_SKIP,
                ))
                .child(rows::divider_text(
                    tr(cx, key::lng_messages_privacy_charge_about),
                    cx,
                ));
        }
        column
    }

    /// "Subscribe to Premium" and why (`!allowed()`).
    fn premium(&self, cx: &App) -> gpui::Div {
        let env = self.env.clone();
        div()
            .flex()
            .flex_col()
            .child(rows::skip())
            .child(
                SettingsButton::new(
                    "messages-privacy-premium",
                    &MESSAGE_PRIVACY_SUBSCRIBE,
                    tr(cx, key::lng_messages_privacy_premium_button),
                )
                .icon(MENU_BLUE_ICON_PREMIUM)
                .on_click(move |_, window, cx| {
                    env.targets
                        .layers
                        .update(cx, |layers, cx| layers.close(window, cx));
                    env.open(PageId::Premium, window, cx);
                }),
            )
            .child(rows::skip())
            .child(rows::divider_text(
                tr(cx, key::lng_messages_privacy_premium_about),
                cx,
            ))
    }
}

impl Render for MessagesBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_locked = self.is_allowed == Some(false);
        let mut frame = BoxFrame::new("messages-privacy-box", &DEFAULT_BOX)
            .width(px_of(BOX_WIDE_WIDTH))
            .title(tr(cx, key::lng_messages_privacy_title))
            .row(rows::divider(cx).into_any_element())
            .row(self.choices(cx));
        let (save, cancel) = (cx.weak_entity(), cx.weak_entity());
        if is_locked {
            frame = frame.row(self.premium(cx)).button(
                RoundButton::new(
                    "messages-privacy-done",
                    &DEFAULT_BOX.button,
                    tr(cx, key::lng_about_done),
                )
                .on_click(move |_, window, cx| {
                    let _ = cancel.update(cx, |this, cx| this.close(window, cx));
                }),
            );
        } else {
            frame = frame
                .button(
                    RoundButton::new(
                        "messages-privacy-save",
                        &DEFAULT_BOX.button,
                        tr(cx, key::lng_settings_save),
                    )
                    .on_click(move |_, window, cx| {
                        let _ = save.update(cx, |this, cx| this.save(window, cx));
                    }),
                )
                .button(
                    RoundButton::new(
                        "messages-privacy-cancel",
                        &DEFAULT_BOX.button,
                        tr(cx, key::lng_cancel),
                    )
                    .on_click(move |_, window, cx| {
                        let _ = cancel.update(cx, |this, cx| this.close(window, cx));
                    }),
                );
        }
        div().track_focus(&self.focus).child(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saving_a_choice_keeps_only_a_set_fee() {
        let fee = NewChatPrivacy {
            allow_unknown: true,
            paid_stars: 50,
        };
        assert_eq!(Choice::of(fee), Choice::Charge);
        assert_eq!(Choice::Charge.privacy(fee), fee);
        assert_eq!(Choice::Restricted.privacy(fee).paid_stars, 0);
        assert!(!Choice::Restricted.privacy(fee).allow_unknown);
    }
}
