//! The window's content (`Window::Controller`): the intro or the signed-in
//! screen by authorization state, the connecting pill, and the box layer.

use std::rc::Rc;

use gpui::{
    AppContext as _, Context, Entity, IntoElement, ParentElement, Render, Styled, Subscription,
    Window, div, prelude::*,
};
use i18n::key;
use model::auth;
use style::layers::ATTENTION_BOX_BUTTON;
use style::{ColorId, ColorRef};
use ui::{Markup, color};

use crate::accounts::{Accounts, AccountsEvent};
use crate::boxes::{self, Callback, ConfirmArgs};
use crate::connecting::Connecting;
use crate::intro::{Data, Intro, Shared, terms};
use crate::lang::tr;
use crate::main_view::MainView;
use crate::{Layers, Store, StoreEvent};

enum Screen {
    /// TDLib is starting or closing; nothing to show yet.
    Blank,
    Intro(Entity<Intro>),
    Main(Entity<MainView>),
}

pub struct Root {
    shared: Shared,
    screen: Screen,
    connecting: Entity<Connecting>,
    /// The terms update the box is open for (`checkLockByTerms`).
    shown_terms: Option<auth::TermsId>,
    _switched: Subscription,
    /// The shown account's.
    _subscriptions: Vec<Subscription>,
}

impl std::fmt::Debug for Root {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Root").finish_non_exhaustive()
    }
}

impl Root {
    pub fn new(accounts: Entity<Accounts>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        crate::accounts::set_domain(accounts.clone(), cx);
        let store = accounts.read(cx).active().clone();
        let shared = Shared {
            store: store.clone(),
            accounts: accounts.clone(),
            layers: cx.new(|_| Layers::new()),
            data: cx.new(|_| Data::default()),
        };
        let connecting = cx.new(|cx| Connecting::new(store.clone(), cx));
        let switched =
            cx.subscribe_in(
                &accounts,
                window,
                |this, accounts, event, window, cx| match event {
                    AccountsEvent::Activated => {
                        let store = accounts.read(cx).active().clone();
                        this.show_account(store, window, cx);
                    }
                },
            );
        let mut this = Self {
            shared,
            screen: Screen::Blank,
            connecting,
            shown_terms: None,
            _switched: switched,
            _subscriptions: Vec::new(),
        };
        this.show_account(store, window, cx);
        this
    }

    /// Shows `store`'s account from scratch, as tdesktop's window does when
    /// the active account changes.
    fn show_account(&mut self, store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) {
        self.shared.store = store.clone();
        self.shared.data = cx.new(|_| Data::default());
        self.connecting = cx.new(|cx| Connecting::new(store.clone(), cx));
        self.shown_terms = None;
        self.screen = Screen::Blank;
        self.shared
            .layers
            .update(cx, |layers, cx| layers.close(window, cx));
        self._subscriptions =
            vec![
                cx.subscribe_in(&store, window, |this, _, event: &StoreEvent, window, cx| {
                    match event {
                        StoreEvent::AuthChanged { .. } => this.auth_changed(window, cx),
                    }
                }),
                cx.observe_in(&store, window, |this, _, window, cx| {
                    this.check_terms(window, cx)
                }),
            ];
        self.auth_changed(window, cx);
        self.check_terms(window, cx);
        cx.notify();
    }

    fn auth_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.shared.auth(cx).clone() {
            auth::State::Ready => {
                if !matches!(self.screen, Screen::Main(_)) {
                    let (store, layers) = (self.shared.store.clone(), self.shared.layers.clone());
                    self.screen =
                        Screen::Main(cx.new(|cx| MainView::new(store, layers, window, cx)));
                    self.upload_photo(cx);
                    cx.notify();
                }
            }
            // The intro stays through a client restart (QR login to phone);
            // the signed-in screen does not outlive its session.
            auth::State::Unknown
            | auth::State::WaitParameters
            | auth::State::LoggingOut
            | auth::State::Closing
            | auth::State::Closed => {
                if matches!(self.screen, Screen::Main(_)) {
                    self.screen = Screen::Blank;
                    self.shared
                        .layers
                        .update(cx, |layers, cx| layers.close(window, cx));
                    cx.notify();
                }
            }
            _ => {
                if !matches!(self.screen, Screen::Intro(_)) {
                    let shared = self.shared.clone();
                    self.screen = Screen::Intro(cx.new(|cx| Intro::new(shared, window, cx)));
                    cx.notify();
                }
            }
        }
    }

    /// The photo picked on sign-up, set once the account exists.
    fn upload_photo(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.shared.data.update(cx, |data, _| data.photo.take()) else {
            return;
        };
        let task = self
            .shared
            .store
            .read(cx)
            .session()
            .set_profile_photo(&path.to_string_lossy());
        let task = crate::store::spawn(task, cx);
        cx.spawn(async move |_, _| {
            if let Err(error) = task.await {
                tracing::warn!(%error, "setting the sign-up photo failed");
            }
        })
        .detach();
    }

    fn check_terms(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pending = self.shared.store.read(cx).state().terms_update.clone();
        let Some((id, terms)) = pending else {
            if self.shown_terms.take().is_some() {
                self.shared
                    .layers
                    .update(cx, |layers, cx| layers.close(window, cx));
            }
            return;
        };
        if self.shown_terms.as_ref() == Some(&id) {
            return;
        }
        self.shown_terms = Some(id.clone());
        self.show_terms_update(id, terms, window, cx);
    }

    fn show_terms_update(
        &mut self,
        id: auth::TermsId,
        terms: auth::Terms,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.shared.store.clone();
        let accepted: Callback = {
            let id = id.clone();
            Rc::new(move |_, cx| {
                let task = store
                    .read(cx)
                    .auth(auth::Action::AcceptTerms(id.clone()), cx);
                store.update(cx, |store, cx| store.accept_terms(&id, cx));
                cx.spawn(async move |_| {
                    if let Err(error) = task.await {
                        tracing::warn!(%error, "accepting the terms failed");
                    }
                })
                .detach();
            })
        };
        let this = cx.entity().downgrade();
        let declined: Callback = Rc::new(move |window, cx| {
            this.update(cx, |this, cx| this.terms_declined(window, cx))
                .ok();
        });
        terms::show_update(&self.shared.layers, terms, accepted, declined, window, cx);
    }

    /// `showTermsDecline`, then `showTermsDelete`; going back shows the
    /// terms again, as tdesktop's lock does.
    fn terms_declined(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let layers = self.shared.layers.clone();
        let this = cx.entity().downgrade();
        let back: Callback = Rc::new(move |window, cx| {
            this.update(cx, |this, cx| {
                this.shown_terms = None;
                this.check_terms(window, cx);
            })
            .ok();
        });
        let store = self.shared.store.clone();
        let delete_back = back.clone();
        let delete_layers = layers.clone();
        let mut args = ConfirmArgs::new(Markup::plain(
            tr(cx, key::lng_terms_update_sorry).to_string(),
        ));
        args.title = Some(tr(cx, key::lng_terms_header));
        args.confirm_text = Some(tr(cx, key::lng_terms_decline_and_delete));
        args.confirm_style = &ATTENTION_BOX_BUTTON;
        args.confirmed = Some(Rc::new(move |window, cx| {
            let store = store.clone();
            let mut delete = ConfirmArgs::new(Markup::plain(
                tr(cx, key::lng_terms_delete_warning).to_string(),
            ));
            delete.confirm_text = Some(tr(cx, key::lng_terms_delete_now));
            delete.confirm_style = &ATTENTION_BOX_BUTTON;
            delete.confirmed = Some(Rc::new(move |_, cx| {
                let task = store.read(cx).auth(auth::Action::DeclineTerms, cx);
                cx.spawn(async move |_| {
                    if let Err(error) = task.await {
                        tracing::warn!(%error, "deleting the account failed");
                    }
                })
                .detach();
            }));
            delete.cancelled = Some(delete_back.clone());
            boxes::confirm(&delete_layers, delete, window, cx);
        }));
        args.cancel_text = Some(tr(cx, key::lng_terms_back));
        args.cancelled = Some(back);
        boxes::confirm(&layers, args, window, cx);
    }
}

impl Render for Root {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = div().size_full();
        let root = match &self.screen {
            Screen::Main(main) => crate::presence::watch_input(root, main.read(cx).presence()),
            Screen::Blank | Screen::Intro(_) => root,
        };
        root.relative()
            .bg(color(cx, ColorRef::Palette(ColorId::WindowBg)))
            .map(|root| match &self.screen {
                Screen::Blank => root,
                Screen::Intro(intro) => root.child(intro.clone()),
                Screen::Main(main) => root.child(main.clone()),
            })
            .child(self.connecting.clone())
            .child(self.shared.layers.clone())
    }
}
