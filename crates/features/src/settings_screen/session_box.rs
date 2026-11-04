//! A session's or website's details (`SessionInfoBox`, the websites'
//! `InfoBox`) and renaming this device (`RenameBox`).
// shortcut: the big userpic is static, tdesktop plays a Lottie per device; add it with Lottie support.

use std::rc::Rc;

use chrono::Local;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, ParentElement, Render, SharedString, Styled,
    Subscription, Window, div, linear_color_stop, linear_gradient,
};
use i18n::key;
use model::sessions::{ActiveSession, Device};
use style::ColorRef;
use style::layers::{
    ATTENTION_BOX_BUTTON, BOX_LABEL, BOX_ROW_PADDING, BOX_WIDE_WIDTH, DEFAULT_BOX,
    DEFAULT_SUBSECTION_TITLE, DEFAULT_SUBSECTION_TITLE_PADDING,
};
use style::menu_icons::{
    MENU_ICON_ADDRESS, MENU_ICON_DEVICES, MENU_ICON_INFO, MENU_ICON_IP_ADDRESS,
};
use style::settings::{
    SESSION_BIG_COVER_PADDING, SESSION_BIG_NAME, SESSION_DATE_LABEL, SESSION_DATE_SKIP,
    SESSION_SUBTITLE_SKIP, SESSION_VALUE_PADDING, SESSION_VALUE_SKIP, SETTINGS_DEVICE_NAME,
};
use style::settings_active_sessions::{
    SESSION_BIG_ICON_OTHER, SESSION_BIG_ICON_WEB, SESSION_BIG_USERPIC_SIZE,
    SESSION_VALUE_ICON_POSITION, SESSION_VALUE_LABEL,
};
use ui::{BoxFrame, FlatLabel, InputEvent, InputField, Markup, RoundButton, color};

use super::{Env, rows, sessions};
use crate::history::layout;
use crate::lang::{strings, tr};
use crate::px_of;

/// `kMaxDeviceModelLength`.
const MAX_DEVICE_NAME: usize = 32;

/// What the box's Terminate does.
pub(super) type OnTerminate = Rc<dyn Fn(&mut Window, &mut App)>;

fn close(env: &Env, window: &mut Window, cx: &mut App) {
    env.targets
        .layers
        .update(cx, |layers, cx| layers.close(window, cx));
}

/// The circle with the device's gradient (`GenerateUserpicBig`).
fn big_userpic(device: Device, cx: &App) -> gpui::Div {
    let (from, to) = sessions::device_colors(device);
    let icon = match device {
        Device::Web => SESSION_BIG_ICON_WEB,
        Device::Other => SESSION_BIG_ICON_OTHER,
        device => sessions::device_icon(device),
    };
    div()
        .size(px_of(SESSION_BIG_USERPIC_SIZE))
        .rounded_full()
        .bg(linear_gradient(
            180.0,
            linear_color_stop(color(cx, ColorRef::Palette(from)), 0.0),
            linear_color_stop(color(cx, ColorRef::Palette(to)), 1.0),
        ))
        .flex()
        .items_center()
        .justify_center()
        .child(ui::icon(icon))
}

/// A value, its name under it and the icon at the left
/// (`AddSessionInfoRow`); nothing for an empty value.
fn info_row(
    id: (&'static str, usize),
    name: SharedString,
    value: &str,
    icon: style::Icon,
) -> Option<AnyElement> {
    if value.is_empty() {
        return None;
    }
    let left = px_of(BOX_ROW_PADDING.left) + px_of(SESSION_VALUE_PADDING.left);
    let top = px_of(SESSION_VALUE_PADDING.top);
    Some(
        div()
            .relative()
            .pl(left)
            .pr(px_of(BOX_ROW_PADDING.right))
            .pt(top)
            .pb(px_of(SESSION_VALUE_SKIP))
            .child(FlatLabel::new(
                id,
                &BOX_LABEL,
                Markup::plain(value.to_owned()),
            ))
            .child(FlatLabel::new(
                (id.0, id.1 + 100),
                &SESSION_VALUE_LABEL,
                Markup::plain(name.to_string()),
            ))
            .child(
                div()
                    .absolute()
                    .left(px_of(SESSION_VALUE_ICON_POSITION.x))
                    .top(top + px_of(SESSION_VALUE_ICON_POSITION.y))
                    .child(ui::icon(icon)),
            )
            .into_any_element(),
    )
}

/// What the box's top shows.
pub(super) enum Cover {
    Device(Device),
    /// A website's bot.
    Userpic(ui::UserpicSource),
}

/// One box's content: the cover, a name and the line under it, then the
/// info rows (`SessionInfoBox`, the websites' `InfoBox`).
pub(super) struct Details {
    pub(super) cover: Cover,
    pub(super) name: String,
    pub(super) subtitle: String,
    /// The row's name, its value and icon; empty values are left out.
    pub(super) rows: Vec<(i18n::Key, String, style::Icon)>,
    pub(super) has_location: bool,
    /// The red button's text and action; `None` for this device.
    pub(super) terminate: Option<(i18n::Key, OnTerminate)>,
}

pub(super) struct SessionBox {
    env: Env,
    details: Details,
    focus: FocusHandle,
}

impl Focusable for SessionBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// Opens the details of `session`; `on_terminate` is `None` for this
/// device.
pub(super) fn show(
    env: &Env,
    session: ActiveSession,
    on_terminate: Option<OnTerminate>,
    window: &mut Window,
    cx: &mut App,
) {
    let details = Details {
        cover: Cover::Device(session.device),
        subtitle: layout::date_time_full(strings(cx), session.last_active, &Local::now()),
        has_location: !session.location.is_empty(),
        rows: vec![
            (
                key::lng_sessions_application,
                session.application,
                MENU_ICON_DEVICES,
            ),
            (key::lng_sessions_system, session.system, MENU_ICON_INFO),
            (key::lng_sessions_ip, session.ip, MENU_ICON_IP_ADDRESS),
            (
                key::lng_sessions_location,
                session.location,
                MENU_ICON_ADDRESS,
            ),
        ],
        name: session.device_model,
        terminate: on_terminate.map(|action| (key::lng_sessions_terminate, action)),
    };
    show_details(env, details, window, cx);
}

pub(super) fn show_details(env: &Env, details: Details, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| SessionBox {
        env: env.clone(),
        details,
        focus: cx.focus_handle(),
    });
    env.targets
        .layers
        .update(cx, |layers, cx| layers.show_above(view, window, cx));
}

impl SessionBox {
    fn info_rows(&self, cx: &App) -> Vec<AnyElement> {
        self.details
            .rows
            .iter()
            .enumerate()
            .filter_map(|(index, (name, value, icon))| {
                info_row(("session-info", index), tr(cx, *name), value, *icon)
            })
            .collect()
    }

    fn header(&self, cx: &App) -> gpui::Div {
        let details = &self.details;
        let cover = match &details.cover {
            Cover::Device(device) => big_userpic(*device, cx).into_any_element(),
            Cover::Userpic(source) => {
                ui::Userpic::new(px_of(SESSION_BIG_USERPIC_SIZE), source.clone()).into_any_element()
            }
        };
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .justify_center()
                    .pt(px_of(SESSION_BIG_COVER_PADDING.top))
                    .pb(px_of(SESSION_BIG_COVER_PADDING.bottom))
                    .child(cover),
            )
            .child(FlatLabel::new(
                "session-name",
                &SESSION_BIG_NAME,
                Markup::plain(details.name.clone()),
            ))
            .child(div().pb(px_of(SESSION_DATE_SKIP)).child(FlatLabel::new(
                "session-date",
                &SESSION_DATE_LABEL,
                Markup::plain(details.subtitle.clone()),
            )))
    }
}

impl Render for SessionBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut frame = BoxFrame::new("session-box", &DEFAULT_BOX)
            .width(px_of(BOX_WIDE_WIDTH))
            .row(self.header(cx))
            .row(rows::divider(cx))
            .row(rows::skip_by(SESSION_SUBTITLE_SKIP))
            .row(rows::subtitle(tr(cx, key::lng_sessions_info), cx));
        for row in self.info_rows(cx) {
            frame = frame.row(row);
        }
        frame = frame.row(rows::skip_by(SESSION_VALUE_SKIP));
        if self.details.has_location {
            frame = frame.row(rows::divider_text(
                tr(cx, key::lng_sessions_location_about),
                cx,
            ));
        }
        let done_env = self.env.clone();
        frame = frame.button(
            RoundButton::new(
                "session-done",
                &DEFAULT_BOX.button,
                tr(cx, key::lng_about_done),
            )
            .on_click(move |_, window, cx| close(&done_env, window, cx)),
        );
        if let Some((text, on_terminate)) = self.details.terminate.clone() {
            let env = self.env.clone();
            frame = frame.left_button(
                RoundButton::new("session-terminate", &ATTENTION_BOX_BUTTON, tr(cx, text))
                    .on_click(move |_, window, cx| {
                        close(&env, window, cx);
                        on_terminate(window, cx);
                    }),
            );
        }
        div().track_focus(&self.focus).child(frame)
    }
}

/// `base::CleanAndSimplify`: trimmed, inner runs of spaces made one.
fn clean(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// "Rename" next to "This device" (`RenameBox`).
pub(super) struct RenameBox {
    env: Env,
    field: Entity<InputField>,
    _field: Subscription,
}

impl Focusable for RenameBox {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.field.focus_handle(cx)
    }
}

/// `system_name` is the field's placeholder, the name an empty field gives.
pub(super) fn show_rename(env: &Env, system_name: &str, window: &mut Window, cx: &mut App) {
    let custom = crate::settings::custom_device_model(cx).unwrap_or_default();
    let view = cx.new(|cx| {
        let field = cx.new(|cx| {
            let mut field =
                InputField::new(&SETTINGS_DEVICE_NAME, system_name.to_owned(), window, cx)
                    .with_max_length(MAX_DEVICE_NAME);
            field.set_text(&custom, None, cx);
            field
        });
        let subscription = cx.subscribe_in(
            &field,
            window,
            |this: &mut RenameBox, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Submitted) {
                    this.save(window, cx);
                }
            },
        );
        RenameBox {
            env: env.clone(),
            field,
            _field: subscription,
        }
    });
    let field = view.read(cx).field.clone();
    env.targets
        .layers
        .update(cx, |layers, cx| layers.show_above(view, window, cx));
    crate::focus(&field, window, cx);
}

impl RenameBox {
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = clean(self.field.read(cx).text());
        crate::settings::set_custom_device_model(name, cx);
        close(&self.env, window, cx);
    }
}

impl Render for RenameBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let field_margins = SETTINGS_DEVICE_NAME.text_margins;
        let (save, cancel) = (cx.weak_entity(), self.env.clone());
        BoxFrame::new("rename-box", &DEFAULT_BOX)
            .title(tr(cx, key::lng_settings_rename_device_title))
            .row(
                div()
                    .px(px_of(BOX_ROW_PADDING.left))
                    .pt(px_of(DEFAULT_SUBSECTION_TITLE_PADDING.top))
                    .child(FlatLabel::new(
                        "rename-label",
                        &DEFAULT_SUBSECTION_TITLE,
                        Markup::plain(tr(cx, key::lng_settings_device_name).to_string()),
                    )),
            )
            .row(
                div()
                    .pl(px_of(BOX_ROW_PADDING.left) - px_of(field_margins.left))
                    .pr(px_of(BOX_ROW_PADDING.right) - px_of(field_margins.right))
                    .child(self.field.clone()),
            )
            .button(
                RoundButton::new(
                    "rename-save",
                    &DEFAULT_BOX.button,
                    tr(cx, key::lng_settings_save),
                )
                .on_click(move |_, window, cx| {
                    let _ = save.update(cx, |this, cx| this.save(window, cx));
                }),
            )
            .button(
                RoundButton::new(
                    "rename-cancel",
                    &DEFAULT_BOX.button,
                    tr(cx, key::lng_cancel),
                )
                .on_click(move |_, window, cx| close(&cancel, window, cx)),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_names_are_trimmed_and_simplified() {
        assert_eq!(clean("  My   Mac \n"), "My Mac");
        assert_eq!(clean("   "), "");
    }
}
