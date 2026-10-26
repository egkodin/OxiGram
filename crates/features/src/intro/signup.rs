//! Sign-up for a new number (`intro_signup.cpp`): a name, a photo, and the
//! terms of service.

use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    MouseButton, ObjectFit, ParentElement, PathPromptOptions, Render, SharedString, Styled,
    Subscription, Window, div, img, prelude::*,
};
use i18n::key;
use model::auth;
use style::intro::{
    INTRO_ERROR_TOP, INTRO_NAME, INTRO_NEXT_BUTTON, INTRO_PHONE_TOP, INTRO_PHOTO_TOP,
    INTRO_STEP_FIELD_TOP,
};
use style::userpic_button::DEFAULT_USERPIC_BUTTON;
use ui::{InputEvent, InputField, Markup, color, icon_size};

use super::errors;
use super::step::{Geometry, Header, Shared, StepError, StepEvent, StepView, Visibility};
use super::terms;
use crate::boxes;
use crate::lang::tr;
use crate::px_of;

/// The image types the photo picker offers (`FileDialog::ImagesFilter`).
const IMAGE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "bmp"];

pub(crate) struct SignupStep {
    shared: Shared,
    focus: FocusHandle,
    first: Entity<InputField>,
    last: Entity<InputField>,
    /// The language writes the last name first (`langFirstNameGoesSecond`).
    is_inverted: bool,
    photo: Option<PathBuf>,
    is_sent: bool,
    error: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

impl std::fmt::Debug for SignupStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignupStep").finish_non_exhaustive()
    }
}

fn last_name_first(cx: &App) -> bool {
    let template = tr(cx, key::lng_full_name);
    match (template.find("{first_name}"), template.find("{last_name}")) {
        (Some(first), Some(last)) => last < first,
        _ => false,
    }
}

impl SignupStep {
    pub fn new(shared: Shared, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let first = cx
            .new(|cx| InputField::new(&INTRO_NAME, tr(cx, key::lng_signup_firstname), window, cx));
        let last =
            cx.new(|cx| InputField::new(&INTRO_NAME, tr(cx, key::lng_signup_lastname), window, cx));
        let subscriptions = [&first, &last]
            .into_iter()
            .map(|field| {
                cx.subscribe_in(
                    field,
                    window,
                    |this: &mut SignupStep, _, event, window, cx| match event {
                        InputEvent::Changed => {
                            if this.error.take().is_some() {
                                cx.notify();
                            }
                        }
                        InputEvent::Submitted => this.submit(window, cx),
                        _ => {}
                    },
                )
            })
            .collect();
        Self {
            shared,
            focus: cx.focus_handle(),
            first,
            last,
            is_inverted: last_name_first(cx),
            photo: None,
            is_sent: false,
            error: None,
            _subscriptions: subscriptions,
        }
    }

    fn terms(&self, cx: &App) -> Option<auth::Terms> {
        match self.shared.auth(cx) {
            auth::State::WaitRegistration { terms } if !terms.text.text.is_empty() => {
                Some(terms.clone())
            }
            _ => None,
        }
    }

    fn choose_photo(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr(cx, key::lng_choose_image)),
        });
        cx.spawn(async move |this, cx| {
            let path = match paths.await {
                Ok(Ok(Some(mut paths))) => paths.pop(),
                Ok(Ok(None)) => None,
                Ok(Err(error)) => {
                    tracing::warn!(%error, "choosing a photo failed");
                    None
                }
                Err(_) => None,
            };
            let Some(path) = path else {
                return;
            };
            let is_image = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    IMAGE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
                });
            this.update(cx, |this, cx| {
                if is_image {
                    this.photo = Some(path);
                } else {
                    this.error = Some(tr(cx, key::lng_bad_photo));
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn register(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let registration = auth::Registration {
            first_name: self.first.read(cx).text().trim().to_owned(),
            last_name: self.last.read(cx).text().trim().to_owned(),
        };
        let photo = self.photo.clone();
        self.shared.data.update(cx, |data, _| data.photo = photo);
        self.is_sent = true;
        let task = self
            .shared
            .store
            .read(cx)
            .auth(auth::Action::Register(registration), cx);
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                this.is_sent = false;
                if let Err(error) = result {
                    this.failed(&error, window, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// `nameSubmitFail`.
    fn failed(&mut self, error: &auth::Error, window: &mut Window, cx: &mut Context<Self>) {
        match error {
            auth::Error::PhoneNumberFlood => {
                let text = tr(cx, key::lng_error_phone_flood).to_string();
                boxes::inform(&self.shared.layers, Markup::plain(text), None, window, cx);
            }
            auth::Error::PhoneNumberInvalid
            | auth::Error::PhoneNumberBanned
            | auth::Error::CodeExpired
            | auth::Error::CodeInvalid
            | auth::Error::PhoneNumberOccupied => cx.emit(StepEvent::Back),
            auth::Error::FirstNameInvalid => {
                self.error = Some(tr(cx, key::lng_bad_name));
                crate::focus(&self.first, window, cx);
            }
            auth::Error::LastNameInvalid => {
                self.error = Some(tr(cx, key::lng_bad_name));
                crate::focus(&self.last, window, cx);
            }
            other => {
                self.error = Some(errors::text(other, cx));
                let field = if self.is_inverted {
                    &self.last
                } else {
                    &self.first
                };
                crate::focus(&field.clone(), window, cx);
            }
        }
        cx.notify();
    }
}

impl EventEmitter<StepEvent> for SignupStep {}

impl Focusable for SignupStep {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl StepView for SignupStep {
    fn header(&self, cx: &App) -> Header {
        Header {
            title: tr(cx, key::lng_signup_title),
            description: Markup::plain(tr(cx, key::lng_signup_desc).to_string()),
            has_cover: false,
            has_back: true,
            next: Some(tr(cx, key::lng_intro_finish)),
            next_style: &INTRO_NEXT_BUTTON,
            error: self.error.clone().map(|text| StepError {
                text,
                centered: true,
            }),
            error_top: px_of(INTRO_ERROR_TOP),
        }
    }

    /// `SignupWidget::submit`: both names, then the terms if they must be
    /// accepted.
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_sent {
            return;
        }
        let (primary, secondary) = if self.is_inverted {
            (self.last.clone(), self.first.clone())
        } else {
            (self.first.clone(), self.last.clone())
        };
        let has_text =
            |field: &Entity<InputField>, cx: &App| !field.read(cx).text().trim().is_empty();
        let primary_focused = primary.read(cx).focus_handle(cx).is_focused(window);
        if (primary_focused || has_text(&primary, cx)) && !has_text(&secondary, cx) {
            crate::focus(&secondary, window, cx);
            return;
        }
        if !has_text(&primary, cx) {
            crate::focus(&primary, window, cx);
            return;
        }
        self.error = None;
        match self.terms(cx).filter(|terms| terms.show_popup) {
            Some(terms) => {
                let this = cx.entity().downgrade();
                let accepted: boxes::Callback = Rc::new(move |window, cx| {
                    this.update(cx, |this, cx| this.register(window, cx)).ok();
                });
                terms::show(&self.shared.layers, terms, Some(accepted), window, cx);
            }
            None => self.register(window, cx),
        }
    }

    fn activate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let field = if self.is_inverted {
            self.last.clone()
        } else {
            self.first.clone()
        };
        crate::focus(&field, window, cx);
        // `finishInit`: the terms link under the fields.
        if self.terms(cx).is_some() {
            cx.emit(StepEvent::Terms(Visibility::Shown));
        }
    }
}

impl Render for SignupStep {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let g = Geometry::of(window);
        let top = g.content_top(false);
        let left = g.content_left();
        let st = &DEFAULT_USERPIC_BUTTON;
        let size = px_of(st.photo_size);
        let photo_left = left + px_of(INTRO_NEXT_BUTTON.width) - size;
        let first_top = top + px_of(INTRO_STEP_FIELD_TOP);
        let second_top = first_top + px_of(INTRO_NAME.height_min) + px_of(INTRO_PHONE_TOP);
        let (first_top, last_top) = if self.is_inverted {
            (second_top, first_top)
        } else {
            (first_top, second_top)
        };
        let icon = icon_size(st.change_icon);
        let icon_left = if st.change_icon_position.x.0 < 0 {
            (size - icon.width) / 2.0
        } else {
            px_of(st.change_icon_position.x)
        };
        let icon_top = if st.change_icon_position.y.0 < 0 {
            (size - icon.height) / 2.0
        } else {
            px_of(st.change_icon_position.y)
        };
        let this = cx.entity();
        let photo = div()
            .id("signup-photo")
            .absolute()
            .left(photo_left)
            .top(top + px_of(INTRO_PHOTO_TOP))
            .size(size)
            .rounded_full()
            .overflow_hidden()
            .cursor_pointer()
            .bg(color(cx, st.change_button.text_bg))
            .hover(|style| style.bg(color(cx, st.change_button.text_bg_over)))
            .map(|photo| match self.photo.clone() {
                Some(path) => photo.child(img(path).size(size).object_fit(ObjectFit::Cover)),
                None => photo.child(
                    div()
                        .absolute()
                        .left(icon_left)
                        .top(icon_top)
                        .child(ui::icon(st.change_icon)),
                ),
            })
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |_, _, cx| this.update(cx, |this, cx| this.choose_photo(cx)));
        div()
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .child(photo)
            .child(
                div()
                    .absolute()
                    .left(left)
                    .top(first_top)
                    .w(px_of(INTRO_NAME.width))
                    .child(self.first.clone()),
            )
            .child(
                div()
                    .absolute()
                    .left(left)
                    .top(last_top)
                    .w(px_of(INTRO_NAME.width))
                    .child(self.last.clone()),
            )
    }
}
