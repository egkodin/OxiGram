//! "Chat Settings" (`Settings::Chat`): the color theme, auto-night mode,
//! stickers and emoji, messages, sensitive content and the archive.
// shortcut: no accent colors, name color, font family, cloud themes, chat background, swipe action, emoji replacing and suggestions, sticker looping, my stickers and emoji sets, quick reaction or corner buttons, keyboard shortcuts and theme editing; with rows "Chat Settings" and the ones they control.

use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement, ParentElement, Render,
    StatefulInteractiveElement as _, Styled, Task, Window, div, px,
};
use i18n::key;
use style::Color;
use style::basic::ROUND_RADIUS_LARGE;
use style::menu_icons::{MENU_ICON_ARCHIVE, MENU_ICON_NIGHT_MODE};
use style::settings::{
    SETTINGS_BUTTON_NO_ICON, SETTINGS_CHECKBOX, SETTINGS_CHECKBOX_PADDING,
    SETTINGS_CHECKBOXES_SKIP, SETTINGS_PRIVACY_SKIP, SETTINGS_SEND_TYPE,
    SETTINGS_SEND_TYPE_PADDING, SETTINGS_SEND_TYPE_SKIP, SETTINGS_THEME,
    SETTINGS_THEME_BUBBLE_POSITION, SETTINGS_THEME_BUBBLE_RADIUS, SETTINGS_THEME_BUBBLE_SIZE,
    SETTINGS_THEME_BUBBLE_SKIP, SETTINGS_THEME_PREVIEW_SIZE, SETTINGS_THEME_RADIO_BOTTOM,
    SETTINGS_THEMES_TOP_SKIP,
};
use style::widgets::{DEFAULT_CHECK, DEFAULT_RADIO};
use ui::{Checkbox, Checked, Markup, color, font, font_height, hsla};

use super::{Env, archive_box, rows};
use crate::boxes::{Callback, ConfirmArgs};
use crate::lang::tr;
use crate::layers::Layers;
use crate::px_of;
use crate::settings::{self, EmbeddedTheme, SendBy};

/// TDLib's sensitive content options.
const CAN_SHOW_SENSITIVE: &str = "can_ignore_sensitive_content_restrictions";
const SHOW_SENSITIVE: &str = "ignore_sensitive_content_restrictions";

/// A tile's colors (`EmbeddedScheme`): background, sent, received, the
/// radio on and off.
struct Scheme {
    theme: EmbeddedTheme,
    name: i18n::Key,
    colors: [u32; 5],
}

/// `EmbeddedThemes`, in tdesktop's order.
const SCHEMES: [Scheme; 4] = [
    Scheme {
        theme: EmbeddedTheme::Classic,
        name: key::lng_settings_theme_classic,
        colors: [0x9b_d4_94, 0xea_ff_dc, 0xff_ff_ff, 0xea_ff_dc, 0xff_ff_ff],
    },
    Scheme {
        theme: EmbeddedTheme::Day,
        name: key::lng_settings_theme_day,
        colors: [0x7e_c4_ea, 0xd7_f0_ff, 0xff_ff_ff, 0xd7_f0_ff, 0xff_ff_ff],
    },
    Scheme {
        theme: EmbeddedTheme::Tinted,
        name: key::lng_settings_theme_tinted,
        colors: [0x48_57_61, 0x5c_a7_d4, 0x6b_80_8d, 0x6b_80_8d, 0x5c_a7_d4],
    },
    Scheme {
        theme: EmbeddedTheme::Night,
        name: key::lng_settings_theme_night,
        colors: [0x48_57_61, 0x6b_80_8d, 0x6b_80_8d, 0x6b_80_8d, 0x75_bf_b5],
    },
];

fn rgb(value: u32) -> gpui::Hsla {
    let [_, r, g, b] = value.to_be_bytes();
    hsla(Color::rgba(r, g, b, 0xff))
}

/// Runs `toggle` when night mode may change; while it follows the system,
/// asks to stop following first (`ToggleNightModeWithConfirmation`).
pub(crate) fn toggle_night_mode(
    layers: &gpui::Entity<Layers>,
    toggle: Callback,
    window: &mut Window,
    cx: &mut App,
) {
    if !settings::is_system_night_mode(cx) {
        toggle(window, cx);
        return;
    }
    let disable: Callback = Rc::new(move |window, cx| {
        settings::set_system_night_mode(false, cx);
        toggle(window, cx);
    });
    let mut args = ConfirmArgs::new(Markup::plain(
        tr(cx, key::lng_settings_auto_night_warning).to_string(),
    ));
    args.confirm_text = Some(tr(cx, key::lng_settings_auto_night_disable));
    args.cancel_text = Some(tr(cx, key::lng_cancel));
    args.confirmed = Some(disable);
    crate::boxes::confirm(layers, args, window, cx);
}

pub(super) struct ChatPage {
    env: Env,
    /// TDLib lets this account show 18+ content; `None` while asked.
    sensitive: Option<(bool, bool)>,
    _load: Task<()>,
}

impl ChatPage {
    pub(super) fn new(env: Env, cx: &mut Context<Self>) -> Self {
        let session = env.targets.store.read(cx).session().clone();
        let (can, shown) = (
            session.bool_option(CAN_SHOW_SENSITIVE),
            session.bool_option(SHOW_SENSITIVE),
        );
        let task = crate::store::spawn(
            async move { Ok::<_, model::RequestError>((can.await?, shown.await?)) },
            cx,
        );
        let load = cx.spawn(async move |this, cx| match task.await {
            Ok(sensitive) => {
                let _ = this.update(cx, |this, cx| {
                    this.sensitive = Some(sensitive);
                    cx.notify();
                });
            }
            Err(error) => tracing::warn!(%error, "sensitive content options failed"),
        });
        Self {
            env,
            sensitive: None,
            _load: load,
        }
    }

    fn tile(&self, index: usize, scheme: &Scheme, cx: &mut Context<Self>) -> AnyElement {
        let [bg, sent, received, radio_on, radio_off] = scheme.colors;
        let is_chosen = settings::theme(cx) == scheme.theme;
        let preview = SETTINGS_THEME_PREVIEW_SIZE;
        let bubble = SETTINGS_THEME_BUBBLE_SIZE;
        let position = SETTINGS_THEME_BUBBLE_POSITION;
        let bubble_at =
            |left: Option<style::Px>, right: Option<style::Px>, top: style::Px, fill: u32| {
                let block = div()
                    .absolute()
                    .top(px_of(top))
                    .w(px_of(bubble.width))
                    .h(px_of(bubble.height))
                    .rounded(px_of(SETTINGS_THEME_BUBBLE_RADIUS))
                    .bg(rgb(fill));
                match (left, right) {
                    (Some(left), _) => block.left(px_of(left)),
                    (_, Some(right)) => block.right(px_of(right)),
                    _ => block,
                }
            };
        let radio = DEFAULT_RADIO;
        let ring = if is_chosen { radio_on } else { radio_off };
        let diameter = px_of(radio.diameter);
        let dot = diameter - px(f32::from(u16::try_from(radio.skip.0).unwrap_or(0)) / 5.0);
        let text = &SETTINGS_THEME;
        let text_fg = if is_chosen {
            text.text_fg_active
        } else {
            text.text_fg
        };
        let theme = scheme.theme;
        let layers = self.env.targets.layers.clone();
        div()
            .id(("chat-theme", index))
            .relative()
            .w(px_of(preview.width))
            .h(px_of(text.text_position.y) + font_height(text.style.font))
            .cursor_pointer()
            .on_click(move |_, window, cx| {
                if settings::theme(cx).is_night() == theme.is_night() {
                    settings::set_theme(theme, cx);
                    return;
                }
                let apply: Callback = Rc::new(move |_, cx| settings::set_theme(theme, cx));
                toggle_night_mode(&layers, apply, window, cx);
            })
            .child(
                div()
                    .relative()
                    .w(px_of(preview.width))
                    .h(px_of(preview.height))
                    .rounded(px_of(ROUND_RADIUS_LARGE))
                    .bg(rgb(bg))
                    .child(bubble_at(Some(position.x), None, position.y, received))
                    .child(bubble_at(
                        None,
                        Some(position.x),
                        style::Px(position.y.0 + bubble.height.0 + SETTINGS_THEME_BUBBLE_SKIP.0),
                        sent,
                    ))
                    .child(
                        div()
                            .absolute()
                            .bottom(px_of(SETTINGS_THEME_RADIO_BOTTOM))
                            .left((px_of(preview.width) - diameter) / 2.0)
                            .size(diameter)
                            .rounded_full()
                            .border(px_of(radio.thickness))
                            .border_color(rgb(ring))
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(is_chosen, |this| {
                                this.child(div().size(dot).rounded_full().bg(rgb(ring)))
                            }),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .top(px_of(text.text_position.y))
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .font(font(text.style.font))
                    .text_size(px_of(text.style.font.size))
                    .text_color(color(cx, text_fg))
                    .child(tr(cx, scheme.name)),
            )
            .into_any_element()
    }

    fn themes(&mut self, cx: &mut Context<Self>) -> gpui::Div {
        let padding = SETTINGS_BUTTON_NO_ICON.padding;
        let tiles: Vec<_> = SCHEMES
            .iter()
            .enumerate()
            .map(|(index, scheme)| self.tile(index, scheme, cx))
            .collect();
        div()
            .flex()
            .flex_col()
            .child(rows::skip_by(SETTINGS_PRIVACY_SKIP))
            .child(rows::subtitle(tr(cx, key::lng_settings_themes), cx))
            .child(rows::skip_by(SETTINGS_THEMES_TOP_SKIP))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .pl(px_of(padding.left))
                    .pr(px_of(padding.right))
                    .children(tiles),
            )
            .child(rows::skip())
    }

    fn theme_settings(&self, cx: &App) -> gpui::Div {
        let is_auto = settings::is_system_night_mode(cx);
        let label = if is_auto {
            key::lng_settings_auto_night_mode_on
        } else {
            key::lng_settings_auto_night_mode_off
        };
        div()
            .flex()
            .flex_col()
            .child(rows::divider(cx))
            .child(rows::skip_by(SETTINGS_PRIVACY_SKIP))
            .child(rows::subtitle(tr(cx, key::lng_settings_theme_settings), cx))
            .child(
                rows::button(
                    "chat/auto-night-mode",
                    tr(cx, key::lng_settings_auto_night_mode),
                    MENU_ICON_NIGHT_MODE,
                )
                .label(tr(cx, label))
                .on_click(move |_, _, cx| settings::set_system_night_mode(!is_auto, cx)),
            )
            .child(rows::skip_by(SETTINGS_CHECKBOXES_SKIP))
    }

    /// A settings checkbox (`settingsCheckbox` in `settingsCheckboxPadding`).
    fn checkbox(
        id: &'static str,
        text: gpui::SharedString,
        is_checked: bool,
        set: fn(bool, &mut App),
    ) -> gpui::Div {
        let padding = SETTINGS_CHECKBOX_PADDING;
        let checked = if is_checked {
            Checked::Yes
        } else {
            Checked::No
        };
        div()
            .pl(px_of(padding.left))
            .pr(px_of(padding.right))
            .pt(px_of(padding.top))
            .pb(px_of(padding.bottom))
            .child(
                Checkbox::new(id, &SETTINGS_CHECKBOX, &DEFAULT_CHECK, text, checked).on_toggle(
                    move |window, cx| {
                        set(!is_checked, cx);
                        window.refresh();
                    },
                ),
            )
    }

    fn stickers_and_messages(&self, cx: &App) -> gpui::Div {
        let padding = SETTINGS_SEND_TYPE_PADDING;
        let send_by = settings::send_by(cx);
        let send_radio = |value: SendBy, text: i18n::Key, cx: &App| {
            let checked = if send_by == value {
                Checked::Yes
            } else {
                Checked::No
            };
            div()
                .pl(px_of(padding.left))
                .pr(px_of(padding.right))
                .pt(px_of(padding.top))
                .pb(px_of(padding.bottom))
                .child(
                    Checkbox::radio(
                        ("chat/send-by", value as usize),
                        &SETTINGS_SEND_TYPE,
                        &DEFAULT_RADIO,
                        tr(cx, text),
                        checked,
                    )
                    .on_toggle(move |window, cx| {
                        settings::set_send_by(value, cx);
                        window.refresh();
                    }),
                )
        };
        let ctrl_enter = if cfg!(target_os = "macos") {
            key::lng_settings_send_cmdenter
        } else {
            key::lng_settings_send_ctrlenter
        };
        div()
            .flex()
            .flex_col()
            .child(rows::divider(cx))
            .child(rows::skip())
            .child(rows::subtitle(tr(cx, key::lng_settings_stickers_emoji), cx))
            .child(Self::checkbox(
                "chat/large-emoji",
                tr(cx, key::lng_settings_large_emoji),
                settings::is_large_emoji(cx),
                settings::set_large_emoji,
            ))
            .child(rows::skip_by(SETTINGS_CHECKBOXES_SKIP))
            .child(rows::divider(cx))
            .child(rows::skip())
            .child(rows::subtitle(tr(cx, key::lng_settings_messages), cx))
            .child(rows::skip_by(SETTINGS_SEND_TYPE_SKIP))
            .child(send_radio(SendBy::Enter, key::lng_settings_send_enter, cx))
            .child(send_radio(SendBy::CtrlEnter, ctrl_enter, cx))
            .child(rows::skip_by(SETTINGS_SEND_TYPE_SKIP))
            .child(Self::checkbox(
                "chat/pull-to-next",
                tr(cx, key::lng_settings_pull_to_next_channel),
                settings::is_pull_to_next_channel(cx),
                settings::set_pull_to_next_channel,
            ))
            .child(rows::skip())
    }

    /// `SetupSensitiveContent`, when Telegram lets the account change it.
    fn sensitive(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let (can, shown) = self.sensitive?;
        if !can {
            return None;
        }
        let weak = cx.weak_entity();
        Some(
            div()
                .flex()
                .flex_col()
                .child(rows::divider(cx))
                .child(rows::skip())
                .child(rows::subtitle(
                    tr(cx, key::lng_settings_sensitive_title),
                    cx,
                ))
                .child(
                    rows::plain(
                        "chat/sensitive",
                        tr(cx, key::lng_settings_sensitive_disable_filtering),
                    )
                    .toggle(shown)
                    .on_click(move |_, _, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            let session = this.env.targets.store.read(cx).session().clone();
                            let task = session.set_bool_option(SHOW_SENSITIVE, !shown);
                            crate::peer_menu::send(task, "showing sensitive content", cx);
                            this.sensitive = Some((can, !shown));
                            cx.notify();
                        });
                    }),
                )
                .child(rows::skip())
                .child(rows::divider_text(
                    tr(cx, key::lng_settings_sensitive_about),
                    cx,
                )),
        )
    }
}

impl Render for ChatPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let env = self.env.clone();
        let archive = rows::button(
            "chat/archive-settings",
            tr(cx, key::lng_context_archive_settings),
            MENU_ICON_ARCHIVE,
        )
        .on_click(move |_, window, cx| archive_box::show(&env, window, cx));
        div()
            .flex()
            .flex_col()
            .child(self.themes(cx))
            .child(self.theme_settings(cx))
            .child(self.stickers_and_messages(cx))
            .children(self.sensitive(cx))
            .child(rows::skip())
            .child(archive)
            .child(rows::skip())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_go_classic_day_tinted_night() {
        let themes: Vec<_> = SCHEMES.iter().map(|scheme| scheme.theme).collect();
        assert_eq!(
            themes,
            [
                EmbeddedTheme::Classic,
                EmbeddedTheme::Day,
                EmbeddedTheme::Tinted,
                EmbeddedTheme::Night
            ]
        );
        assert!(
            SCHEMES
                .iter()
                .all(|s| s.theme.is_night() == (s.colors[0] == 0x48_57_61))
        );
    }
}
