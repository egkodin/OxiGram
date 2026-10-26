//! What every intro step shares (`Intro::Step`): the header and error the
//! intro draws for it, the next button's text, and the layout formulas.

use gpui::{
    AnyView, App, AppContext as _, Context, Entity, EventEmitter, Pixels, SharedString, Window, px,
};
use model::{Secret, auth};
use style::intro::{
    INTRO_CONTENT_TOP_ADD, INTRO_HEIGHT, INTRO_NEXT_BUTTON, INTRO_NEXT_TOP, INTRO_STEP_HEIGHT_FULL,
    INTRO_STEP_TOP_MIN,
};
use style::window::WINDOW_MIN_HEIGHT;
use ui::Markup;

use super::code::CodeStep;
use super::email::EmailStep;
use super::password::PasswordStep;
use super::phone::PhoneStep;
use super::qr::QrStep;
use super::signup::SignupStep;
use super::start::StartStep;
use crate::layers::Layers;
use crate::px_of;
use crate::store::Store;

/// What a step needs to act: the model, the window's box layer and what
/// earlier steps learned (`Intro::Data`).
#[derive(Debug, Clone)]
pub(crate) struct Shared {
    pub store: Entity<Store>,
    pub layers: Entity<Layers>,
    pub data: Entity<Data>,
}

/// What the steps pass on to each other.
#[derive(Debug, Default)]
pub(crate) struct Data {
    /// The number the code went to, formatted for showing (`+1 212 555 0100`).
    pub phone: Secret,
    /// The photo chosen on sign-up, set once signed in.
    pub photo: Option<std::path::PathBuf>,
}

impl Shared {
    pub fn auth<'a>(&self, cx: &'a App) -> &'a auth::State {
        &self.store.read(cx).state().auth
    }
}

/// An error under a step's fields; centered ones span the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StepError {
    pub text: SharedString,
    pub centered: bool,
}

/// What the intro draws around a step.
#[derive(Debug, Clone)]
pub(crate) struct Header {
    pub title: SharedString,
    pub description: Markup,
    pub has_cover: bool,
    pub has_back: bool,
    /// `None` hides the next button.
    pub next: Option<SharedString>,
    pub next_style: &'static style::widgets::RoundButton,
    pub error: Option<StepError>,
    /// The error's top relative to the content top (`errorTop`).
    pub error_top: Pixels,
}

/// Navigation a step asks the intro for (`goNext`, `goBack`, ...).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StepEvent {
    /// Go back to `kind` if it is the previous step, otherwise forward to it
    /// (`goNextOrBack`).
    NextOrBack(Kind),
    Back,
    /// Show or hide the reset account button (`showResetButton`).
    ResetButton(Visibility),
    /// Show or hide the terms of service link under the fields.
    Terms(Visibility),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Visibility {
    Shown,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Start,
    Qr,
    Phone,
    Code,
    Password,
    Signup,
    Email,
}

/// One step of the sign-in flow.
#[derive(Debug, Clone)]
pub(crate) enum Step {
    Start(Entity<StartStep>),
    Qr(Entity<QrStep>),
    Phone(Entity<PhoneStep>),
    Code(Entity<CodeStep>),
    Password(Entity<PasswordStep>),
    Signup(Entity<SignupStep>),
    Email(Entity<EmailStep>),
}

/// A step's screen. The intro draws the header; the step draws the rest in
/// window coordinates.
pub(crate) trait StepView: gpui::Render + gpui::Focusable + EventEmitter<StepEvent> {
    fn header(&self, cx: &App) -> Header;
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>);
    /// The step became the current one; focus its first field.
    fn activate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle(cx).focus(window, cx);
    }
    /// TDLib reported an authorization state while the step is current.
    fn auth_changed(&mut self, _: &auth::State, _: &mut Window, _: &mut Context<Self>) {}
}

macro_rules! each_step {
    ($step:expr, $entity:ident => $body:expr) => {
        match $step {
            Step::Start($entity) => $body,
            Step::Qr($entity) => $body,
            Step::Phone($entity) => $body,
            Step::Code($entity) => $body,
            Step::Password($entity) => $body,
            Step::Signup($entity) => $body,
            Step::Email($entity) => $body,
        }
    };
}

impl Step {
    pub fn new(kind: Kind, shared: &Shared, window: &mut Window, cx: &mut App) -> Self {
        let shared = shared.clone();
        match kind {
            Kind::Start => Self::Start(cx.new(StartStep::new)),
            Kind::Qr => Self::Qr(cx.new(|cx| QrStep::new(shared, cx))),
            Kind::Phone => Self::Phone(cx.new(|cx| PhoneStep::new(shared, window, cx))),
            Kind::Code => Self::Code(cx.new(|cx| CodeStep::new(shared, window, cx))),
            Kind::Password => Self::Password(cx.new(|cx| PasswordStep::new(shared, window, cx))),
            Kind::Signup => Self::Signup(cx.new(|cx| SignupStep::new(shared, window, cx))),
            Kind::Email => Self::Email(cx.new(|cx| EmailStep::new(shared, window, cx))),
        }
    }

    pub fn kind(&self) -> Kind {
        match self {
            Self::Start(_) => Kind::Start,
            Self::Qr(_) => Kind::Qr,
            Self::Phone(_) => Kind::Phone,
            Self::Code(_) => Kind::Code,
            Self::Password(_) => Kind::Password,
            Self::Signup(_) => Kind::Signup,
            Self::Email(_) => Kind::Email,
        }
    }

    pub fn header(&self, cx: &App) -> Header {
        each_step!(self, step => step.read(cx).header(cx))
    }

    pub fn view(&self) -> AnyView {
        each_step!(self, step => step.clone().into())
    }

    pub fn submit(&self, window: &mut Window, cx: &mut App) {
        each_step!(self, step => step.update(cx, |step, cx| step.submit(window, cx)));
    }

    pub fn activate(&self, window: &mut Window, cx: &mut App) {
        each_step!(self, step => step.update(cx, |step, cx| step.activate(window, cx)));
    }

    pub fn auth_changed(&self, state: &auth::State, window: &mut Window, cx: &mut App) {
        each_step!(self, step => step.update(cx, |step, cx| step.auth_changed(state, window, cx)));
    }

    /// Calls `on_event` with the step's navigation requests.
    pub fn subscribe<T: 'static>(
        &self,
        window: &mut Window,
        cx: &mut Context<T>,
        on_event: impl Fn(&mut T, StepEvent, &mut Window, &mut Context<T>) + 'static,
    ) -> gpui::Subscription {
        each_step!(self, step => cx.subscribe_in(step, window, move |this, _, event, window, cx| {
            on_event(this, *event, window, cx);
        }))
    }

    /// Re-renders the intro when the step changes its header.
    pub fn observe<T: 'static>(&self, cx: &mut Context<T>) -> gpui::Subscription {
        each_step!(self, step => cx.observe(step, |_, _, cx| cx.notify()))
    }
}

/// The intro's layout for a window size (`Step::contentLeft/contentTop`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Geometry {
    pub width: Pixels,
    pub height: Pixels,
}

impl Geometry {
    pub fn of(window: &Window) -> Self {
        let size = window.viewport_size();
        Self {
            width: size.width,
            height: size.height,
        }
    }

    /// The left edge of the 300px column the fields share.
    pub fn content_left(self) -> Pixels {
        ((self.width - px_of(INTRO_NEXT_BUTTON.width)) / 2.0).floor()
    }

    pub fn content_top(self, has_cover: bool) -> Pixels {
        let mut top = ((self.height - px_of(INTRO_HEIGHT)) / 2.0)
            .floor()
            .max(px_of(INTRO_STEP_TOP_MIN));
        if has_cover {
            let full = top + px_of(INTRO_NEXT_TOP) + px_of(INTRO_CONTENT_TOP_ADD);
            let range = px_of(INTRO_STEP_HEIGHT_FULL) - px_of(WINDOW_MIN_HEIGHT);
            let added = 1.0 - ((full - px_of(WINDOW_MIN_HEIGHT)) / range).clamp(0.0, 1.0);
            top += px((added * f32::from(px_of(INTRO_CONTENT_TOP_ADD))).round());
        }
        top
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_is_centered_and_kept_below_the_minimum_top() {
        let small = Geometry {
            width: px(380.0),
            height: px(480.0),
        };
        assert_eq!(small.content_left(), px(40.0));
        assert_eq!(small.content_top(false), px(76.0));
        // A short window pushes the cover's content down by introContentTopAdd.
        assert_eq!(small.content_top(true), px(106.0));
        let tall = Geometry {
            width: px(800.0),
            height: px(1000.0),
        };
        assert_eq!(tall.content_top(false), px(297.0));
        assert_eq!(tall.content_top(true), px(297.0));
    }
}
