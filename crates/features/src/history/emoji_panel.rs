//! The emoji panel over the field (`TabbedPanel` holding an
//! `EmojiListWidget`): recently used emoji, then the categories, with an
//! icon per section in the category bar. Hovering the emoji button shows it,
//! leaving both hides it, and clicking an emoji types it into the field.

use std::rc::Rc;
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement as _, ListAlignment, ListOffset,
    ListState, ParentElement as _, Pixels, SharedString, StatefulInteractiveElement as _,
    Styled as _, Task, Window, div, list, point, px,
};
use i18n::key;
use style::chat_helpers::{
    DEFAULT_EMOJI_PAN, EMOJI_PAN_AREA, EMOJI_PAN_DURATION, EMOJI_PAN_HEADER_FONT,
    EMOJI_PAN_HEIGHT_RATIO, EMOJI_PAN_MARGINS, EMOJI_PAN_MAX_HEIGHT, EMOJI_PAN_MIN_HEIGHT,
    EMOJI_PAN_RADIUS, EMOJI_PAN_WIDTH, HISTORY_ATTACH, HISTORY_ATTACH_EMOJI,
};
use style::emoji::EMOJI_SECTIONS;
use style::{ColorId, ColorRef, Icon};
use ui::{IconButton, color, font, font_height};

use super::HistoryView;
use crate::lang::tr;
use crate::px_of;
use crate::ranked::Limits;

/// Recent emoji kept and shown (`kRecentEmojiLimit`); counts halve past
/// the ceiling (`incrementRecentEmoji`).
const RECENT_LIMITS: Limits = Limits {
    length: 54,
    ceiling: 0x8000,
};
/// What Recent shows before anything was sent (`GetDefaultRecent`).
const DEFAULT_RECENT: &[&str] = &[
    "😂",
    "😘",
    "❤\u{fe0f}",
    "😍",
    "😊",
    "😁",
    "👍",
    "☺\u{fe0f}",
    "😔",
    "😄",
    "😭",
    "💋",
    "😒",
    "😳",
    "😜",
    "🙈",
    "😉",
    "😃",
    "😢",
    "😝",
    "😱",
    "😡",
    "😏",
    "😞",
    "😅",
    "😚",
    "🙊",
    "😌",
    "😀",
    "😋",
    "😆",
    "👌",
    "😐",
    "😕",
];
/// Recent, then the categories (`Ui::Emoji::Section`).
const SECTION_COUNT: usize = EMOJI_SECTIONS.len() + 1;
const SECTION_TITLES: [i18n::Key; SECTION_COUNT - 1] = [
    key::lng_emoji_category1,
    key::lng_emoji_category2,
    key::lng_emoji_category3,
    key::lng_emoji_category4,
    key::lng_emoji_category5,
    key::lng_emoji_category6,
    key::lng_emoji_category7,
];
/// Draws the glyphs at `Ui::Emoji::GetSizeLarge()` at 1x, 24 px.
const GLYPH_FONT_SIZE: f32 = 20.0;
const GLYPH_SIZE: f32 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Row {
    /// A category's title; Recent has none.
    Header(usize),
    /// Up to a row's worth of a section's emoji, from `start`.
    Emoji { section: usize, start: usize },
    /// Room under the last section, so the footer can bring it to the top.
    Bottom,
}

/// What the pointer is over; the panel stays while either is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Over {
    Button,
    Panel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Emoji,
    Stickers,
    Gifs,
}

pub(super) struct EmojiPanel {
    list: ListState,
    sections: Rc<Vec<Vec<SharedString>>>,
    rows: Rc<Vec<Row>>,
    is_shown: bool,
    is_pinned: bool,
    tab: Tab,
    pub(super) stickers: super::sticker_panel::StickerPanel,
    pub(super) gifs: super::gif_panel::GifPanel,
    over_button: bool,
    over_panel: bool,
    /// The emoji under the pointer, as its row and column.
    pub(super) hovered: Option<(usize, usize)>,
    /// The section the list's top is in, lit in the footer.
    active: usize,
    bottom_height: Pixels,
    hiding: Option<Task<()>>,
    /// The user's custom emoji packs, after the categories.
    pub(super) packs: super::emoji_packs::Packs,
    pub(super) has_asked_packs: bool,
}

impl EmojiPanel {
    pub(super) fn new() -> Self {
        Self {
            list: ListState::new(0, ListAlignment::Top, px(0.0)),
            sections: Rc::default(),
            rows: Rc::default(),
            is_shown: false,
            is_pinned: false,
            tab: Tab::Emoji,
            stickers: super::sticker_panel::StickerPanel::new(),
            gifs: super::gif_panel::GifPanel::new(),
            over_button: false,
            over_panel: false,
            hovered: None,
            active: 0,
            bottom_height: px(0.0),
            hiding: None,
            packs: Rc::default(),
            has_asked_packs: false,
        }
    }
}

/// The grid inside the list (`EmojiListWidget::countDesiredHeight`):
/// columns, the cell size, and the first cell's left edge.
fn grid() -> (usize, Pixels, Pixels, Pixels) {
    let st = &DEFAULT_EMOJI_PAN;
    let list_width = px_of(EMOJI_PAN_WIDTH) - px_of(EMOJI_PAN_RADIUS) - px_of(st.scroll.width);
    let full_width = px_of(st.margin.left) + list_width + px_of(st.margin.right);
    let inner_width = full_width - px_of(st.padding.left) - px_of(st.padding.right);
    let columns = ((inner_width / px_of(st.desired_size)).floor() as usize).max(1);
    let cell_width = (inner_width / columns as f32).floor();
    let cell_height = cell_width - px_of(st.vertical_size_sub) * 2.0;
    let left = px_of(st.padding.left) + (inner_width - cell_width * columns as f32) / 2.0
        - px_of(st.margin.left);
    (columns, cell_width, cell_height, left)
}

/// Stored recents, then the defaults they leave out, up to the limit
/// (`Settings::resolveRecentEmoji`).
fn recent(stored: &crate::ranked::Ranked) -> Vec<SharedString> {
    let mut shown: Vec<SharedString> = stored
        .iter()
        .map(|(emoji, _)| SharedString::from(emoji.clone()))
        .collect();
    for emoji in DEFAULT_RECENT {
        if shown.len() >= RECENT_LIMITS.length {
            break;
        }
        if !shown.iter().any(|known| known.as_ref() == *emoji) {
            shown.push((*emoji).into());
        }
    }
    shown.truncate(RECENT_LIMITS.length);
    shown
}

/// The rows for sections of `lengths` emoji each.
fn rows(lengths: &[usize], columns: usize) -> Vec<Row> {
    let mut rows = Vec::new();
    for (section, length) in lengths.iter().copied().enumerate() {
        if section > 0 {
            rows.push(Row::Header(section));
        }
        rows.extend(
            (0..length)
                .step_by(columns)
                .map(|start| Row::Emoji { section, start }),
        );
    }
    rows.push(Row::Bottom);
    rows
}

fn section_of(row: Row, last: usize) -> usize {
    match row {
        Row::Header(section) | Row::Emoji { section, .. } => section,
        Row::Bottom => last,
    }
}

/// The panel's height inside its shadow margins for a bottom edge
/// `bottom` from the top (`TabbedPanel::updateContentHeight`).
fn content_height(bottom: Pixels) -> Pixels {
    let margins = px_of(EMOJI_PAN_RADIUS) + px_of(DEFAULT_EMOJI_PAN.footer);
    let padding = px_of(EMOJI_PAN_MARGINS.top) + px_of(EMOJI_PAN_MARGINS.bottom);
    let wanted = ((bottom - margins) * EMOJI_PAN_HEIGHT_RATIO as f32).round() - padding;
    margins + wanted.clamp(px_of(EMOJI_PAN_MIN_HEIGHT), px_of(EMOJI_PAN_MAX_HEIGHT))
}

fn section_icon(section: usize, is_active: bool) -> Icon {
    let icons = &DEFAULT_EMOJI_PAN.icons;
    let (icon, active) = match section {
        0 => (icons.recent, icons.recent_active),
        1 => (icons.people, icons.people_active),
        2 => (icons.nature, icons.nature_active),
        3 => (icons.food, icons.food_active),
        4 => (icons.activity, icons.activity_active),
        5 => (icons.travel, icons.travel_active),
        6 => (icons.objects, icons.objects_active),
        _ => (icons.symbols, icons.symbols_active),
    };
    if is_active { active } else { icon }
}

impl HistoryView {
    fn show_emoji_panel(&mut self, cx: &mut Context<Self>) {
        let panel = &mut self.emoji;
        panel.hiding = None;
        if panel.is_shown {
            return;
        }
        let mut sections = vec![recent(&crate::settings::recent_emoji(cx))];
        sections.extend(EMOJI_SECTIONS.iter().map(|section| {
            section
                .iter()
                .map(|emoji| SharedString::from(*emoji))
                .collect()
        }));
        panel.sections = Rc::new(sections);
        panel.active = 0;
        panel.hovered = None;
        panel.is_shown = true;
        self.relayout_emoji_panel(cx);
        self.load_emoji_packs(cx);
    }

    /// Lays the rows out again for the sections and packs there are.
    pub(super) fn relayout_emoji_panel(&mut self, cx: &mut Context<Self>) {
        let panel = &mut self.emoji;
        let lengths: Vec<usize> = panel
            .sections
            .iter()
            .map(Vec::len)
            .chain(panel.packs.iter().map(|pack| pack.stickers.len()))
            .collect();
        let last = lengths.len().saturating_sub(1);
        let rows = rows(&lengths, grid().0);
        // shortcut: packs that land while the panel is open send its list to the top; keep the offset when that shows.
        panel.list.reset(rows.len());
        let weak = cx.weak_entity();
        let sections_of: Rc<Vec<usize>> =
            Rc::new(rows.iter().map(|row| section_of(*row, last)).collect());
        panel.list.set_scroll_handler(move |event, _, cx| {
            let active = sections_of
                .get(event.visible_range.start)
                .copied()
                .unwrap_or(0);
            // The view is gone once its chat closed; nothing to light then.
            let _ = weak.update(cx, |this, cx| {
                if this.emoji.active != active {
                    this.emoji.active = active;
                    cx.notify();
                }
            });
        });
        panel.rows = Rc::new(rows);
        cx.notify();
    }

    pub(super) fn hide_emoji_panel(&mut self, cx: &mut Context<Self>) {
        self.emoji.is_shown = false;
        self.emoji.is_pinned = false;
        self.emoji.gifs.stop_preview();
        self.emoji.stickers.clear_hover();
        self.emoji.hiding = None;
        cx.notify();
    }

    /// Shows the panel while the pointer is over it or the button; once
    /// it leaves both, hides it after the time tdesktop fades it out in.
    pub(super) fn emoji_hover(&mut self, over: Over, is_over: bool, cx: &mut Context<Self>) {
        match over {
            Over::Button => self.emoji.over_button = is_over,
            Over::Panel => self.emoji.over_panel = is_over,
        }
        if is_over {
            if over == Over::Button {
                self.show_emoji_panel(cx);
            }
            self.emoji.hiding = None;
            return;
        }
        if !self.emoji.is_shown
            || self.emoji.is_pinned
            || self.emoji.over_button
            || self.emoji.over_panel
        {
            return;
        }
        let delay = Duration::from_millis(u64::try_from(EMOJI_PAN_DURATION).unwrap_or(0));
        self.emoji.hiding = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            // The view is gone once its chat closed.
            let _ = this.update(cx, |this, cx| this.hide_emoji_panel(cx));
        }));
    }

    fn toggle_emoji_panel(&mut self, cx: &mut Context<Self>) {
        if self.emoji.is_pinned {
            self.hide_emoji_panel(cx);
        } else {
            self.emoji.is_pinned = true;
            self.show_emoji_panel(cx);
        }
    }

    /// Types `emoji` where the field's cursor is and counts it as used.
    fn choose_emoji(&mut self, emoji: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        crate::settings::count_emoji_use(&emoji, RECENT_LIMITS, cx);
        self.compose
            .field
            .update(cx, |field, cx| field.insert(&emoji, cx));
        self.focus_field(window, cx);
    }

    fn scroll_emoji_to(&mut self, section: usize, cx: &mut Context<Self>) {
        let last = SECTION_COUNT + self.emoji.packs.len() - 1;
        let Some(row) = self
            .emoji
            .rows
            .iter()
            .position(|row| section_of(*row, last) == section)
        else {
            return;
        };
        self.emoji.list.scroll_to(ListOffset {
            item_ix: row,
            offset_in_item: px(0.0),
        });
        self.emoji.active = section;
        cx.notify();
    }

    /// The emoji button right of the field.
    pub(super) fn emoji_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let weak = cx.weak_entity();
        div()
            .id("compose-emoji-area")
            .on_hover(cx.listener(|this, is_over: &bool, _, cx| {
                this.emoji_hover(Over::Button, *is_over, cx)
            }))
            .child(
                IconButton::new("compose-emoji", &HISTORY_ATTACH_EMOJI.inner).on_click(
                    move |_, _, cx| {
                        let _ = weak.update(cx, |this, cx| this.toggle_emoji_panel(cx));
                    },
                ),
            )
            .into_any_element()
    }

    fn emoji_header(&self, section: usize, cx: &App) -> AnyElement {
        let st = &DEFAULT_EMOJI_PAN;
        let title = match section.checked_sub(SECTION_COUNT) {
            Some(pack) => self
                .emoji
                .packs
                .get(pack)
                .map(|pack| SharedString::from(pack.title.clone()))
                .unwrap_or_default(),
            None => SECTION_TITLES
                .get(section.wrapping_sub(1))
                .map(|title| tr(cx, *title))
                .unwrap_or_default(),
        };
        div()
            .h(px_of(st.header))
            .pt(px_of(st.header_top))
            .pl(px_of(st.header_left) - px_of(st.margin.left))
            .font(font(EMOJI_PAN_HEADER_FONT))
            .text_size(px_of(EMOJI_PAN_HEADER_FONT.size))
            .line_height(font_height(EMOJI_PAN_HEADER_FONT))
            .text_color(color(cx, st.header_fg))
            .whitespace_nowrap()
            .overflow_hidden()
            .child(title)
            .into_any_element()
    }

    fn emoji_cell(
        &self,
        index: usize,
        column: usize,
        emoji: SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (_, cell_width, cell_height, _) = grid();
        let area_width = px_of(EMOJI_PAN_AREA.width);
        let area_height = px_of(EMOJI_PAN_AREA.height);
        let is_hovered = self.emoji.hovered == Some((index, column));
        let chosen = emoji.clone();
        div()
            .id(("emoji", index * 64 + column))
            .cursor_pointer()
            .relative()
            .flex_none()
            .w(cell_width)
            .h(cell_height)
            .on_hover(cx.listener(move |this, is_over: &bool, _, cx| {
                this.hover_emoji((index, column), *is_over, cx);
            }))
            .on_click(
                cx.listener(move |this, _, window, cx| {
                    this.choose_emoji(chosen.clone(), window, cx)
                }),
            )
            .child(
                div()
                    .absolute()
                    .left((cell_width - area_width) / 2.0)
                    .top((cell_height - area_height) / 2.0)
                    .w(area_width)
                    .h(area_height)
                    .rounded(px_of(EMOJI_PAN_RADIUS))
                    .when(is_hovered, |area| {
                        area.bg(color(cx, DEFAULT_EMOJI_PAN.over_bg))
                    })
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(GLYPH_FONT_SIZE))
                    .line_height(px(GLYPH_SIZE))
                    .child(emoji),
            )
            .into_any_element()
    }

    /// Lights the cell under the pointer, and only it.
    pub(super) fn hover_emoji(
        &mut self,
        cell: (usize, usize),
        is_over: bool,
        cx: &mut Context<Self>,
    ) {
        let next = if is_over {
            Some(cell)
        } else {
            self.emoji.hovered.filter(|was| *was != cell)
        };
        if self.emoji.hovered != next {
            self.emoji.hovered = next;
            cx.notify();
        }
    }

    fn emoji_row(&mut self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let (columns, cell_width, cell_height, left) = grid();
        match self.emoji.rows.get(index).copied() {
            Some(Row::Header(section)) => self.emoji_header(section, cx),
            Some(Row::Emoji { section, start }) if section >= SECTION_COUNT => {
                let packs = self.emoji.packs.clone();
                let stickers = packs.get(section - SECTION_COUNT).map_or(&[][..], |pack| {
                    &pack.stickers[start..(start + columns).min(pack.stickers.len())]
                });
                div()
                    .h(cell_height)
                    .pl(left)
                    .flex()
                    .children(stickers.iter().enumerate().map(|(column, sticker)| {
                        self.custom_emoji_cell(
                            (index, column),
                            sticker.clone(),
                            (cell_width, cell_height),
                            cx,
                        )
                    }))
                    .into_any_element()
            }
            Some(Row::Emoji { section, start }) => {
                let sections = self.emoji.sections.clone();
                let emoji = sections.get(section).map_or(&[][..], |emoji| {
                    &emoji[start..(start + columns).min(emoji.len())]
                });
                div()
                    .h(cell_height)
                    .pl(left)
                    .flex()
                    .children(
                        emoji.iter().enumerate().map(|(column, emoji)| {
                            self.emoji_cell(index, column, emoji.clone(), cx)
                        }),
                    )
                    .into_any_element()
            }
            Some(Row::Bottom) => div().h(self.emoji.bottom_height).into_any_element(),
            None => div().into_any_element(),
        }
    }

    /// The bottom row's height: what the last section leaves of the list,
    /// at least the list's bottom padding (`countDesiredHeight`).
    fn sync_emoji_bottom(&mut self, list_height: Pixels) {
        let st = &DEFAULT_EMOJI_PAN;
        let (_, _, cell_height, _) = grid();
        let last = SECTION_COUNT + self.emoji.packs.len() - 1;
        let last_rows = self
            .emoji
            .rows
            .iter()
            .filter(|row| matches!(row, Row::Emoji { section, .. } if *section == last))
            .count();
        let last_height = px_of(st.header) + cell_height * last_rows as f32;
        let padding = px_of(st.padding.bottom);
        let height = (list_height - padding - last_height).max(px(0.0)) + padding;
        if height != self.emoji.bottom_height {
            self.emoji.bottom_height = height;
            let count = self.emoji.list.item_count();
            if count > 0 {
                self.emoji.list.splice(count - 1..count, 1);
            }
        }
    }

    fn emoji_categories(&self, cx: &mut Context<Self>) -> AnyElement {
        let st = &DEFAULT_EMOJI_PAN;
        let skip = px_of(st.icon_skip);
        let cell = ((px_of(EMOJI_PAN_WIDTH) - skip * 2.0) / SECTION_COUNT as f32).floor();
        let area = px_of(st.icon_area);
        div()
            .flex_none()
            .h(px_of(st.footer))
            .pl(skip)
            .flex()
            .bg(color(cx, st.categories_bg))
            .children((0..SECTION_COUNT).map(|section| {
                let is_active = self.emoji.active == section;
                div()
                    .id(("emoji-section", section))
                    .cursor_pointer()
                    .relative()
                    .w(cell)
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(is_active, |icon| {
                        icon.child(
                            div()
                                .absolute()
                                .left((cell - area) / 2.0)
                                .top((px_of(st.footer) - area) / 2.0)
                                .size(area)
                                .rounded(px_of(EMOJI_PAN_RADIUS))
                                .bg(color(cx, st.categories_bg_over)),
                        )
                    })
                    .child(
                        div()
                            .relative()
                            .child(ui::icon(section_icon(section, is_active))),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.scroll_emoji_to(section, cx)))
            }))
            .into_any_element()
    }

    /// The panel, its bottom on the buttons' top and its right on the
    /// chat's (`HistoryWidget::moveFieldControls`).
    pub(super) fn emoji_panel(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.emoji.is_shown {
            return None;
        }
        let st = &DEFAULT_EMOJI_PAN;
        let bottom = window.viewport_size().height - px_of(HISTORY_ATTACH.height);
        let height = content_height(bottom);
        let tabs_height = px(36.0);
        let tab = self.emoji.tab;
        let body = match tab {
            Tab::Gifs => self.saved_gifs(cx),
            Tab::Stickers => self.installed_stickers(cx),
            Tab::Emoji => {
                self.sync_emoji_bottom(height - px_of(st.footer) - tabs_height);
                let weak = cx.weak_entity();
                div()
                    .flex_1()
                    .min_h_0()
                    .pl(px_of(EMOJI_PAN_RADIUS))
                    .child(
                        list(self.emoji.list.clone(), move |index, _, cx: &mut App| {
                            weak.update(cx, |this, cx| this.emoji_row(index, cx))
                                .unwrap_or_else(|_| div().into_any_element())
                        })
                        .size_full(),
                    )
                    .into_any_element()
            }
        };
        let tabs = div()
            .h(tabs_height)
            .flex_none()
            .flex()
            .items_center()
            .gap(px(4.0))
            .px(px(8.0))
            .children(
                [
                    (Tab::Emoji, key::lng_switch_emoji),
                    (Tab::Stickers, key::lng_switch_stickers),
                    (Tab::Gifs, key::lng_switch_gifs),
                ]
                .into_iter()
                .enumerate()
                .map(|(index, (choice, title))| {
                    let fg = if choice == tab {
                        ColorId::WindowActiveTextFg
                    } else {
                        ColorId::WindowFg
                    };
                    let selected =
                        color(cx, ColorRef::Palette(ColorId::WindowActiveTextFg)).opacity(0.14);
                    let hover = color(cx, ColorRef::Palette(ColorId::WindowBgOver));
                    div()
                        .id(("picker-tab", index))
                        .flex_1()
                        .h(px(28.0))
                        .rounded(px(8.0))
                        .text_sm()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .when(choice == tab, |tab| tab.bg(selected))
                        .when(choice != tab, |tab| tab.hover(move |tab| tab.bg(hover)))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .text_color(color(cx, ColorRef::Palette(fg)))
                        .child(tr(cx, title))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.emoji.tab = choice;
                            if choice != Tab::Gifs {
                                this.emoji.gifs.stop_preview();
                            }
                            if choice != Tab::Stickers {
                                this.emoji.stickers.clear_hover();
                            }
                            cx.notify();
                        }))
                }),
            );
        let shadow = st.show_animation.shadow;
        Some(
            div()
                .id("emoji-panel")
                .absolute()
                .right(px(0.0))
                .bottom(px_of(HISTORY_ATTACH.height))
                .pl(px_of(EMOJI_PAN_MARGINS.left))
                .pr(px_of(EMOJI_PAN_MARGINS.right))
                .pt(px_of(EMOJI_PAN_MARGINS.top))
                .pb(px_of(EMOJI_PAN_MARGINS.bottom))
                .on_hover(cx.listener(|this, is_over: &bool, _, cx| {
                    this.emoji_hover(Over::Panel, *is_over, cx)
                }))
                // The panel keeps its clicks from the history under it.
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .w(px_of(EMOJI_PAN_WIDTH))
                        .h(height)
                        .flex()
                        .flex_col()
                        .rounded(px_of(EMOJI_PAN_RADIUS))
                        .overflow_hidden()
                        .bg(color(cx, st.bg))
                        .text_color(color(cx, ColorRef::Palette(ColorId::WindowFg)))
                        .shadow(vec![gpui::BoxShadow {
                            color: color(cx, ColorRef::Palette(ColorId::WindowShadowFg))
                                .opacity(shadow.opacity as f32),
                            offset: point(px_of(shadow.offset.x), px_of(shadow.offset.y)),
                            blur_radius: px_of(shadow.blur_radius),
                            spread_radius: px(0.0),
                            inset: false,
                        }])
                        .child(tabs)
                        .children((tab == Tab::Emoji).then(|| self.emoji_categories(cx)))
                        .child(body),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_has_tdesktops_nine_columns() {
        let (columns, width, height, left) = grid();
        assert_eq!(columns, 9);
        assert_eq!((width, height, left), (px(37.0), px(35.0), px(0.0)));
    }

    #[test]
    fn recent_puts_used_emoji_before_the_defaults() {
        let stored = vec![("😕".to_owned(), 3), ("🦊".to_owned(), 1)];
        let shown = recent(&stored);
        assert_eq!(shown[..3], ["😕", "🦊", "😂"]);
        assert_eq!(shown.len(), DEFAULT_RECENT.len() + 1);
    }

    #[test]
    fn rows_give_categories_headers_and_wrap_at_the_columns() {
        let rows = rows(&[3, 10], 9);
        assert_eq!(
            rows,
            [
                Row::Emoji {
                    section: 0,
                    start: 0
                },
                Row::Header(1),
                Row::Emoji {
                    section: 1,
                    start: 0
                },
                Row::Emoji {
                    section: 1,
                    start: 9
                },
                Row::Bottom,
            ]
        );
    }

    #[test]
    fn the_panel_takes_three_quarters_of_the_room_within_its_bounds() {
        // 0.75 of 554 less the radius and footer, less the shadow margins.
        assert_eq!(content_height(px(554.0)), px(407.0));
        assert_eq!(content_height(px(200.0)), px(44.0 + 278.0));
        assert_eq!(content_height(px(2000.0)), px(44.0 + 640.0));
    }
}
