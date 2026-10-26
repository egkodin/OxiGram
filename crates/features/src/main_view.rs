//! The signed-in window until the chat list lands in M2: the app name and a
//! log out button with tdesktop's confirmation (`showLogoutConfirmation`).

use std::rc::Rc;

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement, Render, Styled,
    Window, div, prelude::*,
};
use i18n::key;
use model::auth;
use style::intro::{INTRO_NEXT_BUTTON, INTRO_TITLE};
use style::layers::ATTENTION_BOX_BUTTON;
use style::{ColorId, ColorRef};
use ui::{FlatLabel, Markup, RoundButton, color};

use crate::boxes::{self, ConfirmArgs};
use crate::lang::tr;
use crate::{Layers, Store};

// shortcut: a placeholder until M2 builds the chat list; replace the whole view then.
pub struct MainView {
    store: Entity<Store>,
    layers: Entity<Layers>,
    focus: FocusHandle,
}

impl std::fmt::Debug for MainView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MainView").finish_non_exhaustive()
    }
}

impl MainView {
    pub fn new(store: Entity<Store>, layers: Entity<Layers>, cx: &mut Context<Self>) -> Self {
        Self {
            store,
            layers,
            focus: cx.focus_handle(),
        }
    }

    fn confirm_logout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.clone();
        let mut args = ConfirmArgs::new(Markup::plain(tr(cx, key::lng_sure_logout).to_string()));
        args.confirm_text = Some(tr(cx, key::lng_settings_logout));
        args.confirm_style = &ATTENTION_BOX_BUTTON;
        args.confirmed = Some(Rc::new(move |_, cx| {
            let task = store.read(cx).auth(auth::Action::LogOut, cx);
            cx.spawn(async move |_| {
                if let Err(error) = task.await {
                    tracing::warn!(%error, "log out failed");
                }
            })
            .detach();
        }));
        boxes::confirm(&self.layers, args, window, cx);
    }
}

impl Focusable for MainView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for MainView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        div()
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .bg(color(cx, ColorRef::Palette(ColorId::WindowBg)))
            .child(FlatLabel::new(
                "main-title",
                &INTRO_TITLE,
                Markup::plain(tr(cx, key::rgram_app_name).to_string()),
            ))
            .child(
                RoundButton::new(
                    "main-logout",
                    &INTRO_NEXT_BUTTON,
                    tr(cx, key::lng_settings_logout),
                )
                .on_click(move |_, window, cx| {
                    entity.update(cx, |this, cx| this.confirm_logout(window, cx))
                }),
            )
    }
}
