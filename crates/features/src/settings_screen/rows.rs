//! The pieces settings pages are made of (`Ui::AddSkip`, `Ui::AddDivider`,
//! `Ui::AddDividerText`, `Ui::AddSubsectionTitle`, `Settings::AddButton`).

use gpui::{AnyElement, App, IntoElement as _, ParentElement as _, SharedString, Styled as _, div};
use style::basic::DEFAULT_VERTICAL_LIST_SKIP;
use style::layers::{DEFAULT_SUBSECTION_TITLE, DEFAULT_SUBSECTION_TITLE_PADDING};
use style::settings::{SETTINGS_BUTTON, SETTINGS_BUTTON_NO_ICON};
use style::widgets::{
    BOX_DIVIDER_HEIGHT, DEFAULT_BOX_DIVIDER_LABEL_PADDING, DEFAULT_DIVIDER_LABEL,
};
use style::{ColorId, ColorRef};
use ui::{SettingsButton, color, font, font_height};

use crate::px_of;

/// `Ui::AddSkip` with the default skip.
pub(super) fn skip() -> AnyElement {
    skip_by(DEFAULT_VERTICAL_LIST_SKIP)
}

pub(super) fn skip_by(height: style::Px) -> AnyElement {
    div().flex_none().h(px_of(height)).into_any_element()
}

/// The gray band between groups of rows.
pub(super) fn divider(cx: &App) -> AnyElement {
    div()
        .flex_none()
        .h(px_of(BOX_DIVIDER_HEIGHT))
        .bg(color(cx, ColorRef::Palette(ColorId::BoxDividerBg)))
        .into_any_element()
}

/// Gray text on the divider's band, explaining the rows above.
pub(super) fn divider_text(text: impl Into<SharedString>, cx: &App) -> AnyElement {
    let label = &DEFAULT_DIVIDER_LABEL.label;
    let padding = DEFAULT_BOX_DIVIDER_LABEL_PADDING;
    let text_font = label.style.font;
    div()
        .flex_none()
        .bg(color(cx, DEFAULT_DIVIDER_LABEL.bar.bg))
        .pl(px_of(padding.left))
        .pr(px_of(padding.right))
        .pt(px_of(padding.top))
        .pb(px_of(padding.bottom))
        .font(font(text_font))
        .text_size(px_of(text_font.size))
        .line_height(font_height(text_font))
        .text_color(color(cx, label.text_fg))
        .child(text.into())
        .into_any_element()
}

/// The blue title over a group of rows.
pub(super) fn subtitle(text: impl Into<SharedString>, cx: &App) -> AnyElement {
    let label = &DEFAULT_SUBSECTION_TITLE;
    let padding = DEFAULT_SUBSECTION_TITLE_PADDING;
    let text_font = label.style.font;
    div()
        .flex_none()
        .pl(px_of(padding.left))
        .pr(px_of(padding.right))
        .pt(px_of(padding.top))
        .pb(px_of(padding.bottom))
        .font(font(text_font))
        .text_size(px_of(text_font.size))
        .line_height(font_height(text_font))
        .text_color(color(cx, label.text_fg))
        .child(text.into())
        .into_any_element()
}

/// One line of text in a label's font and colour.
pub(super) fn text(
    value: impl Into<SharedString>,
    st: &'static style::widgets::FlatLabel,
    cx: &App,
) -> gpui::Div {
    let text_font = st.style.font;
    div()
        .font(font(text_font))
        .text_size(px_of(text_font.size))
        .line_height(font_height(text_font))
        .text_color(color(cx, st.text_fg))
        .child(value.into())
}

/// A row with an icon (`settingsButton`).
pub(super) fn button(
    id: &'static str,
    text: impl Into<SharedString>,
    icon: style::Icon,
) -> SettingsButton {
    SettingsButton::new(id, &SETTINGS_BUTTON, text).icon(icon)
}

/// A row without an icon (`settingsButtonNoIcon`).
pub(super) fn plain(
    id: impl Into<gpui::ElementId>,
    text: impl Into<SharedString>,
) -> SettingsButton {
    SettingsButton::new(id, &SETTINGS_BUTTON_NO_ICON, text)
}
