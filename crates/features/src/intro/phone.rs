//! The phone number step (`intro_phone.cpp`): a country, its calling code
//! and the number.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    MouseButton, ParentElement, Render, SharedString, Styled, Subscription, Window, div,
    prelude::*,
};
use i18n::key;
use model::phone::{Country, number_info};
use model::{Secret, auth};
use style::intro::{
    INTRO_COUNTRY, INTRO_COUNTRY_CODE, INTRO_COUNTRY_ICON, INTRO_COUNTRY_ICON_POSITION,
    INTRO_ERROR_TOP, INTRO_PHONE, INTRO_PHONE_TOP, INTRO_QR_LOGIN_LINK_TOP, INTRO_STEP_FIELD_TOP,
};
use style::widgets::DEFAULT_LINK_BUTTON;
use ui::{InputEvent, InputField, LinkButton, Markup, color, font, icon_size};

use super::country_box::{self, Entry};
use super::errors;
use super::phone_fields::{CodeCorrector, PhoneCorrector, groups_of};
use super::step::{Geometry, Header, Kind, Shared, StepError, StepEvent, StepView};
use crate::boxes;
use crate::lang::tr;
use crate::px_of;

/// The country tdesktop picks when nothing better is known.
const DEFAULT_COUNTRY: &str = "US";
/// Formatting does not depend on the interface language.
const FORMAT_LANGUAGE: &str = "en";

fn valid_code(code: &str) -> String {
    number_info(FORMAT_LANGUAGE, code).map_or_else(|_| code.to_owned(), |info| info.calling_code)
}

fn groups(code: &str, digits: &str) -> Vec<usize> {
    match number_info(FORMAT_LANGUAGE, &format!("{code}{digits}")) {
        Ok(info) if !code.is_empty() && info.calling_code == code => groups_of(&info.formatted),
        _ => Vec::new(),
    }
}

/// Whether TDLib takes a phone number in `state`.
pub(crate) fn takes_phone_number(state: &auth::State) -> bool {
    matches!(
        state,
        auth::State::WaitPhoneNumber
            | auth::State::WaitPremiumPurchase { .. }
            | auth::State::WaitEmailAddress
            | auth::State::WaitEmailCode(_)
            | auth::State::WaitCode(_)
            | auth::State::WaitRegistration { .. }
            | auth::State::WaitPassword(_)
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Request {
    Idle,
    /// Waiting for TDLib to leave QR login before sending the number.
    Queued,
    Sent,
}

pub(crate) struct PhoneStep {
    shared: Shared,
    focus: FocusHandle,
    code: Entity<InputField>,
    phone: Entity<InputField>,
    /// The code the phone field formats for, without `+`.
    code_digits: Rc<RefCell<String>>,
    /// Digits typed past the code, moved to the number.
    overflow: Rc<RefCell<String>>,
    countries: Vec<Country>,
    /// The chosen country's ISO code, if the code names one.
    country: Option<String>,
    country_text: SharedString,
    /// The user edited the number, so a detected country no longer applies.
    changed: bool,
    /// The code text set by the step itself, whose change event is not an edit.
    expected_code: Option<String>,
    request: Request,
    error: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

impl std::fmt::Debug for PhoneStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PhoneStep")
            .field("country", &self.country)
            .field("request", &self.request)
            .finish_non_exhaustive()
    }
}

impl PhoneStep {
    pub fn new(shared: Shared, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let code_digits = Rc::new(RefCell::new(String::new()));
        let overflow = Rc::new(RefCell::new(String::new()));
        let code = cx.new(|cx| {
            InputField::new(&INTRO_COUNTRY_CODE, "", window, cx).with_corrector(CodeCorrector {
                valid_code,
                overflow: overflow.clone(),
            })
        });
        let phone = cx.new(|cx| {
            InputField::new(&INTRO_PHONE, tr(cx, key::lng_phone_number), window, cx)
                .with_corrector(PhoneCorrector::new(code_digits.clone(), groups))
        });
        let subscriptions = vec![
            cx.subscribe_in(&code, window, |this, _, event, window, cx| {
                this.code_event(*event, window, cx);
            }),
            cx.subscribe_in(&phone, window, |this, _, event, window, cx| {
                this.phone_event(*event, window, cx);
            }),
            cx.observe_global::<crate::Lang>(|this, cx| this.load_countries(cx)),
        ];
        let mut step = Self {
            shared,
            focus: cx.focus_handle(),
            code,
            phone,
            code_digits,
            overflow,
            countries: Vec::new(),
            country: None,
            country_text: SharedString::default(),
            changed: false,
            expected_code: None,
            request: Request::Idle,
            error: None,
            _subscriptions: subscriptions,
        };
        step.select_country(DEFAULT_COUNTRY, cx);
        step.load_countries(cx);
        step.detect_country(cx);
        step
    }

    fn load_countries(&mut self, cx: &mut Context<Self>) {
        let request = self.shared.store.read(cx).session().countries();
        let task = cx.background_executor().spawn(request);
        cx.spawn(async move |this, cx| {
            let countries = match task.await {
                Ok(countries) => countries,
                Err(error) => {
                    tracing::warn!(%error, "loading the country list failed");
                    return;
                }
            };
            this.update(cx, |this, cx| {
                this.countries = countries;
                this.update_country_text(cx);
            })
            .ok();
        })
        .detach();
    }

    /// `getNearestDc`'s country: where the user connects from.
    fn detect_country(&mut self, cx: &mut Context<Self>) {
        let request = self.shared.store.read(cx).session().country_code();
        let task = cx.background_executor().spawn(request);
        cx.spawn(async move |this, cx| {
            let Ok(iso) = task.await else {
                return;
            };
            this.update(cx, |this, cx| {
                if !this.changed && !iso.is_empty() {
                    this.select_country(&iso, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// `CountryInput::chooseCountry`: the country and its first code.
    fn select_country(&mut self, iso: &str, cx: &mut Context<Self>) {
        let code = self
            .countries
            .iter()
            .find(|country| country.code == iso)
            .and_then(|country| country.calling_codes.first().cloned())
            .or_else(|| fallback_code(iso));
        let Some(code) = code else {
            return;
        };
        self.country = Some(iso.to_owned());
        self.set_code(&code, cx);
    }

    fn set_code(&mut self, code: &str, cx: &mut Context<Self>) {
        let text = format!("+{code}");
        self.expected_code = Some(text.clone());
        self.code
            .update(cx, |field, cx| field.set_text(&text, None, cx));
        self.code_changed(cx);
    }

    fn code_event(&mut self, event: InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Changed => {
                let text = self.code.read(cx).text().to_owned();
                if self.expected_code.take() != Some(text) {
                    self.changed = true;
                }
                self.error = None;
                self.code_changed(cx);
                let added = std::mem::take(&mut *self.overflow.borrow_mut());
                if !added.is_empty() {
                    self.phone.update(cx, |phone, cx| {
                        let text = format!("{added}{}", phone.text());
                        phone.set_text(&text, Some(added.len()), cx);
                    });
                    crate::focus(&self.phone, window, cx);
                }
                cx.notify();
            }
            InputEvent::Submitted => self.submit(window, cx),
            _ => {}
        }
    }

    fn phone_event(&mut self, event: InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Changed => {
                if !self.phone.read(cx).text().is_empty() {
                    self.changed = true;
                }
                if self.error.take().is_some() {
                    cx.notify();
                }
            }
            InputEvent::Submitted => self.submit(window, cx),
            // `startErasing`: backspace at the start erases the code.
            InputEvent::BackspaceAtStart => {
                self.code.update(cx, |code, cx| {
                    let mut text = code.text().to_owned();
                    text.pop();
                    code.set_text(&text, None, cx);
                });
                crate::focus(&self.code, window, cx);
            }
            InputEvent::Focused | InputEvent::Blurred => {}
        }
    }

    /// `codeChanged`: the country label and the number's pattern follow.
    fn code_changed(&mut self, cx: &mut Context<Self>) {
        let digits = self.code.read(cx).text().trim_start_matches('+').to_owned();
        if *self.code_digits.borrow() != digits {
            self.code_digits.replace(digits.clone());
            self.phone.update(cx, |phone, cx| {
                let text = phone.text().to_owned();
                phone.set_text(&text, None, cx);
            });
        }
        let keeps_country = self.country.as_ref().is_some_and(|iso| {
            self.country_by_iso(iso)
                .is_none_or(|country| country.calling_codes.contains(&digits))
        });
        if !keeps_country {
            self.country = self
                .countries
                .iter()
                .find(|country| country.calling_codes.contains(&digits))
                .map(|country| country.code.clone())
                .or_else(|| {
                    number_info(FORMAT_LANGUAGE, &digits)
                        .ok()
                        .filter(|info| info.calling_code == digits)
                        .and_then(|info| info.country.map(|country| country.code))
                });
        }
        self.update_country_text(cx);
    }

    fn country_by_iso(&self, iso: &str) -> Option<&Country> {
        self.countries.iter().find(|country| country.code == iso)
    }

    /// `CountryInput::onChooseCode`.
    fn update_country_text(&mut self, cx: &mut Context<Self>) {
        let digits = self.code_digits.borrow().clone();
        self.country_text = if digits.is_empty() {
            tr(cx, key::lng_country_code)
        } else if let Some(iso) = &self.country {
            match self.country_by_iso(iso) {
                Some(country) => country.name.clone().into(),
                None => number_info(FORMAT_LANGUAGE, &digits)
                    .ok()
                    .and_then(|info| info.country)
                    .map_or_else(
                        || tr(cx, key::lng_bad_country_code),
                        |country| country.name.into(),
                    ),
            }
        } else {
            tr(cx, key::lng_bad_country_code)
        };
        cx.notify();
    }

    fn choose_country(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let first = self.country.clone().unwrap_or_default();
        let list = country_box::entries(&self.countries, &first);
        let this = cx.entity().downgrade();
        let on_chosen: country_box::OnChosen = Rc::new(move |entry: &Entry, window, cx| {
            let entry = entry.clone();
            this.update(cx, |this, cx| {
                this.changed = true;
                this.country = Some(entry.iso2.clone());
                this.set_code(&entry.code, cx);
                crate::focus(&this.phone, window, cx);
            })
            .ok();
        });
        country_box::show(&self.shared.layers, list, on_chosen, window, cx);
    }

    fn full_number(&self, cx: &App) -> String {
        format!(
            "{}{}",
            self.code.read(cx).text(),
            self.phone.read(cx).text()
        )
    }

    fn show_phone_error(
        &mut self,
        text: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.phone.update(cx, |phone, cx| phone.show_error(cx));
        crate::focus(&self.phone, window, cx);
        self.error = Some(text);
        cx.notify();
    }

    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let number = self.full_number(cx);
        let digits: String = number.chars().filter(char::is_ascii_digit).collect();
        self.shared
            .data
            .update(cx, |data, _| data.phone = Secret::new(number.clone()));
        self.request = Request::Sent;
        let action = auth::Action::SetPhoneNumber(Secret::new(digits));
        let task = self.shared.store.read(cx).auth(action, cx);
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                this.request = Request::Idle;
                if let Err(error) = result {
                    this.failed(&error, &number, window, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// `phoneSubmitFail`.
    fn failed(
        &mut self,
        error: &auth::Error,
        number: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match error {
            auth::Error::PhoneNumberFlood => {
                let text = tr(cx, key::lng_error_phone_flood).to_string();
                boxes::inform(&self.shared.layers, Markup::plain(text), None, window, cx);
            }
            auth::Error::PhoneNumberBanned => {
                errors::phone_banned(&self.shared.layers, number, window, cx);
            }
            other => {
                let text = errors::text(other, cx);
                self.show_phone_error(text, window, cx);
            }
        }
    }
}

/// A calling code from TDLib's offline data before the list loads.
fn fallback_code(iso: &str) -> Option<String> {
    (iso == DEFAULT_COUNTRY).then(|| "1".to_owned())
}

impl EventEmitter<StepEvent> for PhoneStep {}

impl Focusable for PhoneStep {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl StepView for PhoneStep {
    fn header(&self, cx: &App) -> Header {
        Header {
            title: tr(cx, key::lng_phone_title),
            description: Markup::plain(tr(cx, key::lng_phone_desc).to_string()),
            has_cover: false,
            has_back: true,
            next: Some(tr(cx, key::lng_intro_next)),
            next_style: &style::intro::INTRO_NEXT_BUTTON,
            error: self.error.clone().map(|text| StepError {
                text,
                centered: true,
            }),
            error_top: px_of(INTRO_ERROR_TOP),
        }
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.request != Request::Idle {
            return;
        }
        let code_focused = self.code.read(cx).focus_handle(cx).is_focused(window);
        if code_focused
            && self.code.read(cx).text().len() > 1
            && self.phone.read(cx).text().is_empty()
        {
            self.phone.update(cx, |phone, cx| phone.hide_error(cx));
            crate::focus(&self.phone, window, cx);
            return;
        }
        let digits = self
            .full_number(cx)
            .chars()
            .filter(char::is_ascii_digit)
            .count();
        if digits <= 1 {
            let text = tr(cx, key::lng_bad_phone);
            self.show_phone_error(text, window, cx);
            return;
        }
        self.error = None;
        if takes_phone_number(self.shared.auth(cx)) {
            self.send(window, cx);
        } else {
            self.request = Request::Queued;
        }
        cx.notify();
    }

    fn activate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        crate::focus(&self.phone, window, cx);
    }

    fn auth_changed(&mut self, state: &auth::State, window: &mut Window, cx: &mut Context<Self>) {
        if self.request == Request::Queued && takes_phone_number(state) {
            self.send(window, cx);
        }
    }
}

impl Render for PhoneStep {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let g = Geometry::of(window);
        let top = g.content_top(false);
        let left = g.content_left();
        let country_top = top + px_of(INTRO_STEP_FIELD_TOP);
        let country_width = px_of(INTRO_COUNTRY.width);
        let country_height = px_of(INTRO_COUNTRY.height_min);
        let phone_top = country_top + country_height + px_of(INTRO_PHONE_TOP);
        let st = &INTRO_COUNTRY;
        let icon = icon_size(INTRO_COUNTRY_ICON);
        let this = cx.entity();
        let country = div()
            .id("intro-country")
            .absolute()
            .left(left)
            .top(country_top)
            .w(country_width)
            .h(country_height)
            .cursor_pointer()
            .border_b(px_of(st.border))
            .border_color(color(cx, st.border_fg))
            .child(
                div()
                    .absolute()
                    .left(px_of(st.text_margins.left))
                    .right(px_of(st.text_margins.right))
                    .top(px_of(st.text_margins.top))
                    .font(font(st.style.font))
                    .text_size(px_of(st.style.font.size))
                    .text_color(color(cx, st.text_fg))
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(self.country_text.clone()),
            )
            .child(
                div()
                    .absolute()
                    .left(country_width - icon.width - px_of(INTRO_COUNTRY_ICON_POSITION.x))
                    .top(px_of(INTRO_COUNTRY_ICON_POSITION.y))
                    .child(ui::icon(INTRO_COUNTRY_ICON)),
            )
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click({
                let this = this.clone();
                move |_, window, cx| this.update(cx, |this, cx| this.choose_country(window, cx))
            });
        div()
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .child(country)
            .child(
                div()
                    .absolute()
                    .left(left)
                    .top(phone_top)
                    .w(px_of(INTRO_COUNTRY_CODE.width))
                    .child(self.code.clone()),
            )
            .child(
                div()
                    .absolute()
                    .left(left + country_width - px_of(INTRO_PHONE.width))
                    .top(phone_top)
                    .w(px_of(INTRO_PHONE.width))
                    .child(self.phone.clone()),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .w(g.width)
                    .top(top + px_of(INTRO_QR_LOGIN_LINK_TOP))
                    .flex()
                    .justify_center()
                    .child(
                        LinkButton::new(
                            "phone-to-qr",
                            &DEFAULT_LINK_BUTTON,
                            tr(cx, key::lng_phone_to_qr),
                        )
                        .on_click(move |_, _, cx| {
                            this.update(cx, |_, cx| cx.emit(StepEvent::NextOrBack(Kind::Qr)));
                        }),
                    ),
            )
    }
}
