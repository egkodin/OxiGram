//! "Power Usage" (`PowerSavingBox`): the animations to keep, by group; a
//! group's toggle turns all of it on or off, its row opens its items.
// shortcut: the count sits in the title without tdesktop's bold label and arrow, and no "Save Power on Low Battery" (macOS low power state not read); with M8 polish.

use std::collections::BTreeSet;

use gpui::{
    App, AppContext as _, Context, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    ParentElement, Render, StatefulInteractiveElement as _, Styled, Window, div, px,
};
use i18n::{Key, key};
use style::layers::{BOX_WIDE_WIDTH, DEFAULT_BOX};
use style::menu_icons::{
    MENU_ICON_CHAT_BUBBLE, MENU_ICON_EMOJI, MENU_ICON_PHONE, MENU_ICON_START_STREAM,
    MENU_ICON_STICKERS,
};
use style::settings::{POWER_SAVING_BUTTON, SETTINGS_CHECKBOX, SETTINGS_CHECKBOX_PADDING};
use style::widgets::DEFAULT_CHECK;
use ui::{BoxFrame, Checkbox, Checked, RoundButton, SettingsButton};

use super::{Env, rows};
use crate::lang::tr;
use crate::power_saving::{Flag, Saving};
use crate::px_of;
use crate::settings;

/// The toggle's part of a group row (`rightsButtonToggleWidth`).
const TOGGLE_WIDTH: f32 = 70.0;

struct Group {
    /// None for a group of one, titled by its item.
    title: Option<Key>,
    icon: style::Icon,
    items: &'static [(Flag, Key)],
}

/// `PowerSavingLabels`.
const GROUPS: [Group; 5] = [
    Group {
        title: Some(key::lng_settings_power_stickers),
        icon: MENU_ICON_STICKERS,
        items: &[
            (Flag::StickersPanel, key::lng_settings_power_stickers_panel),
            (Flag::StickersChat, key::lng_settings_power_stickers_chat),
        ],
    },
    Group {
        title: Some(key::lng_settings_power_emoji),
        icon: MENU_ICON_EMOJI,
        items: &[
            (Flag::EmojiPanel, key::lng_settings_power_emoji_panel),
            (
                Flag::EmojiReactions,
                key::lng_settings_power_emoji_reactions,
            ),
            (Flag::EmojiChat, key::lng_settings_power_emoji_chat),
            (Flag::EmojiStatus, key::lng_settings_power_emoji_status),
        ],
    },
    Group {
        title: Some(key::lng_settings_power_chat),
        icon: MENU_ICON_CHAT_BUBBLE,
        items: &[
            (
                Flag::ChatBackground,
                key::lng_settings_power_chat_background,
            ),
            (Flag::ChatSpoiler, key::lng_settings_power_chat_spoiler),
            (Flag::ChatEffects, key::lng_settings_power_chat_effects),
        ],
    },
    Group {
        title: None,
        icon: MENU_ICON_PHONE,
        items: &[(Flag::Calls, key::lng_settings_power_calls)],
    },
    Group {
        title: None,
        icon: MENU_ICON_START_STREAM,
        items: &[(Flag::Animations, key::lng_settings_power_ui)],
    },
];

pub(super) struct PowerBox {
    env: Env,
    saving: Saving,
    /// Groups showing their items.
    open: BTreeSet<usize>,
    focus: FocusHandle,
}

impl Focusable for PowerBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

pub(crate) fn show(env: &Env, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| PowerBox {
        env: env.clone(),
        saving: settings::power_saving(cx),
        open: BTreeSet::new(),
        focus: cx.focus_handle(),
    });
    env.targets
        .layers
        .update(cx, |layers, cx| layers.show_above(view, window, cx));
}

impl PowerBox {
    fn close(&self, window: &mut Window, cx: &mut App) {
        self.env
            .targets
            .layers
            .update(cx, |layers, cx| layers.close(window, cx));
    }

    fn set_group(&mut self, index: usize, is_on: bool, cx: &mut Context<Self>) {
        self.saving = GROUPS[index]
            .items
            .iter()
            .fold(self.saving, |saving, (flag, _)| saving.with(*flag, !is_on));
        cx.notify();
    }

    fn group(&self, index: usize, group: &Group, cx: &mut Context<Self>) -> gpui::Div {
        let on = group
            .items
            .iter()
            .filter(|(flag, _)| !self.saving.has(*flag))
            .count();
        let title = match group.title {
            Some(title) => format!("{} {on}/{}", tr(cx, title), group.items.len()).into(),
            None => tr(cx, group.items[0].1),
        };
        let is_nested = group.title.is_some();
        let (expand, toggle) = (cx.weak_entity(), cx.weak_entity());
        let row = SettingsButton::new(("power-group", index), &POWER_SAVING_BUTTON, title)
            .icon(group.icon)
            .toggle(on > 0)
            .on_click(move |_, _, cx| {
                let _ = expand.update(cx, |this, cx| {
                    if !is_nested {
                        this.set_group(index, on == 0, cx);
                    } else if !this.open.remove(&index) {
                        this.open.insert(index);
                    }
                    cx.notify();
                });
            });
        let toggle_area = div()
            .id(("power-group-toggle", index))
            .absolute()
            .top_0()
            .right_0()
            .h_full()
            .w(px(TOGGLE_WIDTH))
            .cursor_pointer()
            .on_click(move |_, _, cx| {
                let _ = toggle.update(cx, |this, cx| this.set_group(index, on == 0, cx));
            });
        let shown = if is_nested && self.open.contains(&index) {
            group.items
        } else {
            &[]
        };
        let items: Vec<_> = shown
            .iter()
            .map(|(flag, text)| self.item(*flag, *text, cx))
            .collect();
        div()
            .flex()
            .flex_col()
            .child(div().relative().child(row).child(toggle_area))
            .children(items)
    }

    fn item(&self, flag: Flag, text: Key, cx: &mut Context<Self>) -> gpui::Div {
        let padding = SETTINGS_CHECKBOX_PADDING;
        let is_on = !self.saving.has(flag);
        let weak = cx.weak_entity();
        div()
            .pl(px_of(padding.left))
            .pr(px_of(padding.right))
            .pt(px_of(padding.top))
            .pb(px_of(padding.bottom))
            .child(
                Checkbox::new(
                    ("power-item", flag.bit() as usize),
                    &SETTINGS_CHECKBOX,
                    &DEFAULT_CHECK,
                    tr(cx, text),
                    if is_on { Checked::Yes } else { Checked::No },
                )
                .on_toggle(move |_, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.saving = this.saving.with(flag, is_on);
                        cx.notify();
                    });
                }),
            )
    }
}

impl Render for PowerBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let groups: Vec<_> = GROUPS
            .iter()
            .enumerate()
            .map(|(index, group)| self.group(index, group, cx))
            .collect();
        let content = div()
            .flex()
            .flex_col()
            .child(rows::subtitle(tr(cx, key::lng_settings_power_subtitle), cx))
            .children(groups)
            .child(rows::skip());
        let (save, cancel) = (cx.weak_entity(), cx.weak_entity());
        div().track_focus(&self.focus).child(
            BoxFrame::new("power-box", &DEFAULT_BOX)
                .width(px_of(BOX_WIDE_WIDTH))
                .title(tr(cx, key::lng_settings_power_title))
                .row(content)
                .button(
                    RoundButton::new(
                        "power-save",
                        &DEFAULT_BOX.button,
                        tr(cx, key::lng_settings_save),
                    )
                    .on_click(move |_, window, cx| {
                        let _ = save.update(cx, |this, cx| {
                            settings::set_power_saving(this.saving, cx);
                            this.close(window, cx);
                        });
                    }),
                )
                .button(
                    RoundButton::new("power-cancel", &DEFAULT_BOX.button, tr(cx, key::lng_cancel))
                        .on_click(move |_, window, cx| {
                            let _ = cancel.update(cx, |this, cx| this.close(window, cx));
                        }),
                ),
        )
    }
}
