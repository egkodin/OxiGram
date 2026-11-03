//! The sign-in screens (`intro/intro_widget.cpp`): a history of steps with
//! the back button, the next button and the cover shared between them.
//!
//! TDLib's authorization state drives the flow: each new state moves to the
//! step that answers it. Steps only move on their own for links (QR code or
//! phone number) and for going back.

mod chrome;
mod code;
mod code_input;
pub(crate) mod country_box;
mod email;
mod errors;
mod new_password;
mod password;
mod phone;
mod phone_fields;
mod qr;
mod signup;
mod start;
mod step;
pub(crate) mod terms;

use gpui::{
    App, Context, FocusHandle, Focusable, IntoElement, KeyBinding, ParentElement, Pixels, Render,
    Styled, Subscription, Window, actions, div, prelude::*,
};
use model::auth;
use style::intro::{INTRO_COVER_DURATION, INTRO_SLIDE_DURATION};
use ui::anim::{Easing, Simple};

use crate::store::StoreEvent;
use step::{Geometry, Header, Kind, Step, StepEvent, Visibility};

pub(crate) use step::{Data, Shared};

actions!(intro, [Submit, Back]);

const CONTEXT: &str = "Intro";

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", Submit, Some(CONTEXT)),
        KeyBinding::new("escape", Back, Some(CONTEXT)),
    ]);
    code_input::bind_keys(cx);
    country_box::bind_keys(cx);
    terms::bind_keys(cx);
}

/// Which way a step change moves (`Animate`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Forward,
    Back,
}

/// How the history changes (`StackAction`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stack {
    Forward,
    Replace,
    Back,
}

/// The step that is leaving, drawn while the new one shows.
struct Departing {
    step: Step,
    header: Header,
    geometry_has_cover: bool,
    direction: Direction,
}

/// The sign-in screens.
pub struct Intro {
    shared: Shared,
    history: Vec<Step>,
    departing: Option<Departing>,
    /// The step change: slide, or cover shown or hidden.
    show: Simple,
    /// Where the controls were when the cover started to change.
    controls_top_from: Pixels,
    next_top_from: Pixels,
    is_next_shown: bool,
    next_shown: Simple,
    error: Option<step::StepError>,
    error_shown: Simple,
    reset_button: Visibility,
    terms: Visibility,
    language_link: Option<chrome::LanguageLink>,
    /// The suggestion the language link was made for.
    requested_language: String,
    focus: FocusHandle,
    step_subscriptions: Vec<Subscription>,
    _subscriptions: Vec<Subscription>,
}

impl std::fmt::Debug for Intro {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Intro")
            .field(
                "steps",
                &self.history.iter().map(Step::kind).collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

impl Focusable for Intro {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Intro {
    pub fn new(shared: Shared, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![
            cx.subscribe_in(
                &shared.store,
                window,
                |this, _, event, window, cx| match event {
                    StoreEvent::AuthChanged { .. } => this.auth_changed(window, cx),
                },
            ),
            cx.observe_global::<crate::Lang>(|this, cx| {
                this.language_link = None;
                this.request_language_link(cx);
                cx.notify();
            }),
            cx.observe(&shared.store, |this, store, cx| {
                if store.read(cx).state().suggested_language != this.requested_language {
                    this.request_language_link(cx);
                }
            }),
        ];
        let mut intro = Self {
            shared,
            history: Vec::new(),
            departing: None,
            show: Simple::default(),
            controls_top_from: Pixels::ZERO,
            next_top_from: Pixels::ZERO,
            is_next_shown: true,
            next_shown: Simple::default(),
            error: None,
            error_shown: Simple::default(),
            reset_button: Visibility::Hidden,
            terms: Visibility::Hidden,
            language_link: None,
            requested_language: String::new(),
            focus: cx.focus_handle(),
            step_subscriptions: Vec::new(),
            _subscriptions: subscriptions,
        };
        let is_added = intro.shared.accounts.read(cx).before_intro().is_some();
        for kind in initial_history(intro.shared.auth(cx), is_added) {
            let step = Step::new(kind, &intro.shared, window, cx);
            intro.history.push(step);
        }
        intro.setup_step(window, cx);
        intro.is_next_shown = intro.current().header(cx).next.is_some();
        if let Some(step) = intro.history.last().cloned() {
            step.activate(window, cx);
        }
        intro.request_language_link(cx);
        // The state that created the intro arrived before it subscribed;
        // replay it for what the history does not show (a Premium box).
        cx.defer_in(window, |this, window, cx| this.auth_changed(window, cx));
        intro
    }

    fn current(&self) -> &Step {
        self.history
            .last()
            .expect("the intro history always holds a step")
    }

    fn setup_step(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let step = self.current().clone();
        self.step_subscriptions = vec![
            step.observe(cx),
            step.subscribe(window, cx, |this, event, window, cx| {
                this.step_event(event, window, cx);
            }),
        ];
    }

    fn step_event(&mut self, event: StepEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            StepEvent::NextOrBack(kind) => self.next_or_back(kind, window, cx),
            StepEvent::Back => self.back(window, cx),
            StepEvent::ResetButton(visibility) => {
                self.reset_button = visibility;
                cx.notify();
            }
            StepEvent::Terms(visibility) => {
                self.terms = visibility;
                cx.notify();
            }
        }
    }

    fn is_animating(&self) -> bool {
        self.departing.is_some()
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_animating() || self.shared.layers.read(cx).is_shown() {
            return;
        }
        let step = self.current().clone();
        step.submit(window, cx);
    }

    /// `backRequested`: the previous step, the account an added one came
    /// from, or the start screen when the history began later (after a
    /// restart).
    fn back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.current().header(cx).has_back {
            return;
        }
        if self.history.len() > 1 {
            self.move_to(None, Stack::Back, Direction::Back, window, cx);
        } else if self.shared.accounts.read(cx).before_intro().is_some() {
            self.shared
                .accounts
                .update(cx, |accounts, cx| accounts.back_from_intro(cx));
        } else {
            self.move_to(
                Some(Kind::Start),
                Stack::Replace,
                Direction::Back,
                window,
                cx,
            );
        }
    }

    fn next_or_back(&mut self, kind: Kind, window: &mut Window, cx: &mut Context<Self>) {
        let len = self.history.len();
        if len > 1 && self.history[len - 2].kind() == kind {
            self.move_to(None, Stack::Back, Direction::Back, window, cx);
        } else {
            self.move_to(Some(kind), Stack::Forward, Direction::Forward, window, cx);
        }
    }

    /// `moveToStep` and `historyMove`.
    fn move_to(
        &mut self,
        kind: Option<Kind>,
        stack: Stack,
        direction: Direction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let geometry = Geometry::of(window);
        let was = self.current().clone();
        let was_header = was.header(cx);
        match stack {
            Stack::Back => {
                self.history.pop();
            }
            Stack::Replace => {
                self.history.pop();
            }
            Stack::Forward => {}
        }
        if let Some(kind) = kind {
            let step = Step::new(kind, &self.shared, window, cx);
            self.history.push(step);
        }
        if self.history.is_empty() {
            let step = Step::new(Kind::Start, &self.shared, window, cx);
            self.history.push(step);
        }
        self.reset_button = Visibility::Hidden;
        self.terms = Visibility::Hidden;
        self.setup_step(window, cx);

        let header = self.current().header(cx);
        let changes_cover = was_header.has_cover != header.has_cover;
        if changes_cover {
            self.next_top_from =
                geometry.content_top(was_header.has_cover) + chrome::next_top_offset();
            self.controls_top_from = chrome::controls_top(was_header.has_cover);
        }
        let (duration, easing) = if changes_cover {
            let easing = if header.has_cover {
                Easing::EaseOutCirc
            } else {
                Easing::Linear
            };
            (INTRO_COVER_DURATION, easing)
        } else {
            (INTRO_SLIDE_DURATION, Easing::Linear)
        };
        self.show.start(0.0, 1.0, duration, easing);
        self.departing = Some(Departing {
            step: was,
            geometry_has_cover: was_header.has_cover,
            header: was_header,
            direction,
        });
        self.error = None;
        self.update_next_shown(header.next.is_some());
        let step = self.current().clone();
        step.activate(window, cx);
        cx.notify();
    }

    fn update_next_shown(&mut self, shown: bool) {
        if shown != self.is_next_shown {
            self.is_next_shown = shown;
            let (from, to) = if shown { (0.0, 1.0) } else { (1.0, 0.0) };
            self.next_shown
                .start(from, to, style::basic::SLIDE_DURATION, Easing::Linear);
        }
    }

    /// Moves to the step that answers TDLib's new authorization state.
    fn auth_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.shared.auth(cx).clone();
        let current = self.current().kind();
        let target = match &state {
            auth::State::WaitPhoneNumber => {
                let is_past_phone = matches!(
                    current,
                    Kind::Code | Kind::Password | Kind::Signup | Kind::Email
                );
                is_past_phone.then_some(Target::BackTo(Kind::Phone))
            }
            auth::State::WaitQrScan { .. } => Some(Target::Show(Kind::Qr)),
            auth::State::WaitCode(_) | auth::State::WaitEmailCode(_) => {
                Some(Target::Show(Kind::Code))
            }
            auth::State::WaitEmailAddress => Some(Target::Show(Kind::Email)),
            auth::State::WaitPassword(_) => Some(Target::Replace(Kind::Password)),
            auth::State::WaitRegistration { .. } => Some(Target::Replace(Kind::Signup)),
            auth::State::WaitPremiumPurchase { support_email } => {
                errors::premium_required(&self.shared.layers, support_email, window, cx);
                None
            }
            auth::State::Unknown
            | auth::State::WaitParameters
            | auth::State::Ready
            | auth::State::LoggingOut
            | auth::State::Closing
            | auth::State::Closed => None,
        };
        match target {
            Some(Target::Show(kind)) if kind == current => {}
            Some(Target::Show(kind)) => self.next_or_back(kind, window, cx),
            Some(Target::Replace(kind)) if kind == current => {}
            Some(Target::Replace(kind)) => {
                let stack = if matches!(current, Kind::Code | Kind::Qr) {
                    Stack::Replace
                } else {
                    Stack::Forward
                };
                self.move_to(Some(kind), stack, Direction::Forward, window, cx);
            }
            Some(Target::BackTo(kind)) => {
                if let Some(index) = self.history.iter().rposition(|step| step.kind() == kind) {
                    let last = self.history.len() - 1;
                    self.history.drain(index + 1..last);
                    self.move_to(None, Stack::Back, Direction::Back, window, cx);
                } else {
                    self.move_to(Some(kind), Stack::Replace, Direction::Back, window, cx);
                }
            }
            None => {}
        }
        let step = self.current().clone();
        step.auth_changed(&state, window, cx);
    }

    /// `createLanguageLink`: back to English from another language, or to
    /// the language TDLib suggests for the system.
    fn request_language_link(&mut self, cx: &mut Context<Self>) {
        let current = cx.global::<crate::Lang>().language().to_owned();
        let suggested = self
            .shared
            .store
            .read(cx)
            .state()
            .suggested_language
            .clone();
        self.requested_language.clone_from(&suggested);
        if current != crate::lang::ENGLISH {
            let text = i18n::Strings::english()
                .text(i18n::key::lng_switch_to_this)
                .to_owned();
            self.language_link = Some(chrome::LanguageLink {
                text: text.into(),
                pack: String::new(),
            });
            return;
        }
        if suggested.is_empty() || suggested == current {
            return;
        }
        let key = i18n::key::lng_switch_to_this.name();
        let request = self
            .shared
            .store
            .read(cx)
            .session()
            .language_pack_strings(&suggested, vec![key.to_owned()]);
        let task = cx.background_executor().spawn(request);
        cx.spawn(async move |this, cx| {
            let strings = match task.await {
                Ok(strings) => strings,
                Err(error) => {
                    tracing::warn!(%error, "fetching the language suggestion failed");
                    return;
                }
            };
            let text = strings.into_iter().find_map(|string| match string.value {
                Some(model::language::PackValue::Text(text)) if string.key == key => Some(text),
                _ => None,
            });
            if let Some(text) = text {
                let link = chrome::LanguageLink {
                    text: text.into(),
                    pack: suggested,
                };
                this.update(cx, |this, cx| this.set_language_link(link, cx))
                    .ok();
            }
        })
        .detach();
    }

    pub(crate) fn set_language_link(&mut self, link: chrome::LanguageLink, cx: &mut Context<Self>) {
        self.language_link = Some(link);
        cx.notify();
    }
}

/// Where a new authorization state sends the intro.
enum Target {
    /// Forward to the step, or back if it is the previous one.
    Show(Kind),
    /// In place of a code or QR step (`goReplace`), forward otherwise.
    Replace(Kind),
    /// Back to the step, as when a code expires.
    BackTo(Kind),
}

/// The steps a restarted app shows for a state TDLib restored.
/// An added account starts at the QR code (`EnterPoint::Qr`).
fn initial_history(state: &auth::State, is_added: bool) -> Vec<Kind> {
    match state {
        auth::State::WaitCode(_) => vec![Kind::Phone, Kind::Code],
        auth::State::WaitEmailAddress => vec![Kind::Phone, Kind::Email],
        auth::State::WaitEmailCode(_) => vec![Kind::Phone, Kind::Email, Kind::Code],
        auth::State::WaitPassword(_) => vec![Kind::Phone, Kind::Password],
        auth::State::WaitRegistration { .. } => vec![Kind::Phone, Kind::Signup],
        auth::State::WaitQrScan { .. } if is_added => vec![Kind::Qr],
        auth::State::WaitQrScan { .. } => vec![Kind::Start, Kind::Qr],
        _ if is_added => vec![Kind::Qr],
        _ => vec![Kind::Start],
    }
}

impl Render for Intro {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let geometry = Geometry::of(window);
        let header = self.current().header(cx);
        self.update_next_shown(header.next.is_some());
        if header.error != self.error {
            match (&self.error, &header.error) {
                (None, Some(_)) | (Some(_), Some(_)) => {
                    self.error_shown.start(
                        0.0,
                        1.0,
                        style::basic::FADE_WRAP_DURATION,
                        Easing::Linear,
                    );
                    self.error.clone_from(&header.error);
                }
                (Some(_), None) => {
                    if !self.error_shown.animating() {
                        self.error_shown.start(
                            1.0,
                            0.0,
                            style::basic::FADE_WRAP_DURATION,
                            Easing::Linear,
                        );
                    }
                }
                (None, None) => {}
            }
        }
        let error_alpha = self
            .error_shown
            .value_in(window, if header.error.is_some() { 1.0 } else { 0.0 });
        if header.error.is_none() && !self.error_shown.animating() {
            self.error = None;
        }
        let progress = self.show.value_in(window, 1.0);
        if !self.show.animating() && self.departing.take().is_some() {
            window.refresh();
        }
        let frame = chrome::Frame {
            geometry,
            header: &header,
            step: self.current(),
            error: self.error.as_ref().map(|error| (error, error_alpha)),
            departing: self
                .departing
                .as_ref()
                .map(|departing| chrome::DepartingFrame {
                    header: &departing.header,
                    step: &departing.step,
                    had_cover: departing.geometry_has_cover,
                    backwards: departing.direction == Direction::Back,
                }),
            progress,
            controls_top_from: self.controls_top_from,
            next_top_from: self.next_top_from,
            next_shown: self
                .next_shown
                .value_in(window, if self.is_next_shown { 1.0 } else { 0.0 }),
            reset_button: self.reset_button == Visibility::Shown,
            terms: self.terms == Visibility::Shown,
            language_link: self.language_link.as_ref(),
            environment: self.shared.store.read(cx).environment(),
        };
        let content = frame.render(cx.entity(), window, cx);
        div()
            .id("intro")
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .on_action(cx.listener(|this, _: &Submit, window, cx| this.submit(window, cx)))
            .on_action(cx.listener(|this, _: &Back, window, cx| {
                if !this.is_animating() {
                    this.back(window, cx);
                }
            }))
            .child(content)
    }
}

impl Intro {
    pub(crate) fn submit_clicked(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.submit(window, cx);
    }

    pub(crate) fn back_clicked(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_animating() {
            self.back(window, cx);
        }
    }

    pub(crate) fn shared(&self) -> &Shared {
        &self.shared
    }

    pub(crate) fn terms(&self, cx: &App) -> Option<auth::Terms> {
        match self.shared.auth(cx) {
            auth::State::WaitRegistration { terms } => Some(terms.clone()),
            _ => None,
        }
    }
}
