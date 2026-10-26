//! `CountrySelectBox`: a searchable list of countries with their calling
//! codes.

use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, IntoElement, KeyBinding,
    MouseButton, ParentElement, Render, ScrollStrategy, SharedString, Styled, Subscription,
    UniformListScrollHandle, WeakEntity, Window, actions, div, prelude::*, px, uniform_list,
};
use i18n::key;
use model::phone::Country;
use style::basic::{NO_CONTACTS_COLOR, NO_CONTACTS_FONT, NO_CONTACTS_HEIGHT};
use style::country_select_box::{
    COUNTRIES_SKIP, COUNTRY_ROW_BG, COUNTRY_ROW_BG_OVER, COUNTRY_ROW_CODE_FG,
    COUNTRY_ROW_CODE_FG_OVER, COUNTRY_ROW_CODE_FONT, COUNTRY_ROW_HEIGHT, COUNTRY_ROW_NAME_FG,
    COUNTRY_ROW_NAME_FONT, COUNTRY_ROW_PADDING,
};
use style::layers::{BOX_MAX_LIST_HEIGHT, DEFAULT_BOX};
use style::widgets::DEFAULT_MULTI_SELECT;
use ui::{BoxFrame, InputEvent, InputField, RoundButton, color, font, font_height};

use crate::lang::tr;
use crate::layers::{Dismiss, Layers};
use crate::px_of;

actions!(country_box, [SelectNext, SelectPrevious]);

const CONTEXT: &str = "CountryBox";
/// Room the box leaves around itself in a short window.
const WINDOW_MARGIN: f32 = 160.0;

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("down", SelectNext, Some(CONTEXT)),
        KeyBinding::new("up", SelectPrevious, Some(CONTEXT)),
    ]);
}

/// One row: a country with one of its calling codes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entry {
    pub name: SharedString,
    pub iso2: String,
    pub code: String,
    /// Lowercase words of the name and the English name, for search.
    words: Vec<String>,
}

/// The rows for `countries`, the one with `first` (ISO code) on top, as
/// tdesktop lists the last valid country first.
pub(crate) fn entries(countries: &[Country], first: &str) -> Vec<Entry> {
    let mut list: Vec<Entry> = Vec::new();
    let ordered = countries
        .iter()
        .filter(|country| country.code == first)
        .chain(countries.iter().filter(|country| country.code != first));
    for country in ordered.filter(|country| !country.is_hidden) {
        let words = search_words(&format!("{} {}", country.name, country.english_name));
        for code in &country.calling_codes {
            list.push(Entry {
                name: country.name.clone().into(),
                iso2: country.code.clone(),
                code: code.clone(),
                words: words.clone(),
            });
        }
    }
    list
}

fn search_words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|ch: char| ch.is_whitespace() || ch == '-')
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

/// `updateFilter`: every query word starts some word of the name.
fn matches(entry: &Entry, query: &[String]) -> bool {
    query.iter().all(|word| {
        entry
            .words
            .iter()
            .any(|name| name.starts_with(word.as_str()))
    })
}

pub(crate) type OnChosen = Rc<dyn Fn(&Entry, &mut Window, &mut App)>;

pub(crate) struct CountryBox {
    layers: WeakEntity<Layers>,
    search: Entity<InputField>,
    list: Rc<Vec<Entry>>,
    filtered: Rc<Vec<usize>>,
    selected: Option<usize>,
    scroll: UniformListScrollHandle,
    on_chosen: OnChosen,
    _subscription: Subscription,
}

impl std::fmt::Debug for CountryBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CountryBox")
            .field("rows", &self.list.len())
            .finish_non_exhaustive()
    }
}

impl Focusable for CountryBox {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.focus_handle(cx)
    }
}

/// Opens the box; `on_chosen` runs with the picked row after it closes.
pub(crate) fn show(
    layers: &Entity<Layers>,
    list: Vec<Entry>,
    on_chosen: OnChosen,
    window: &mut Window,
    cx: &mut App,
) {
    let weak = layers.downgrade();
    let view = cx.new(|cx| {
        let search = cx.new(|cx| {
            InputField::new(
                &DEFAULT_MULTI_SELECT.field,
                tr(cx, key::lng_country_ph),
                window,
                cx,
            )
        });
        let subscription = cx.subscribe_in(
            &search,
            window,
            |this: &mut CountryBox, search, event, window, cx| match event {
                InputEvent::Changed => {
                    let query = search_words(search.read(cx).text());
                    this.filter(&query, cx);
                }
                InputEvent::Submitted => this.choose(this.selected, window, cx),
                _ => {}
            },
        );
        let list = Rc::new(list);
        let filtered = Rc::new((0..list.len()).collect());
        CountryBox {
            layers: weak,
            search,
            selected: (!list.is_empty()).then_some(0),
            list,
            filtered,
            scroll: UniformListScrollHandle::new(),
            on_chosen,
            _subscription: subscription,
        }
    });
    layers.update(cx, |layers, cx| {
        layers.show(view, Dismiss::Allowed, None, window, cx)
    });
}

impl CountryBox {
    fn filter(&mut self, query: &[String], cx: &mut Context<Self>) {
        let filtered: Vec<usize> = if query.is_empty() {
            (0..self.list.len()).collect()
        } else {
            (0..self.list.len())
                .filter(|index| matches(&self.list[*index], query))
                .collect()
        };
        self.selected = (!filtered.is_empty()).then_some(0);
        self.filtered = Rc::new(filtered);
        self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
    }

    fn select_skip(&mut self, direction: isize, cx: &mut Context<Self>) {
        if self.filtered.is_empty() {
            return;
        }
        let last = self.filtered.len() - 1;
        let next = match self.selected {
            None => 0,
            Some(index) => index.saturating_add_signed(direction).min(last),
        };
        self.selected = Some(next);
        self.scroll.scroll_to_item(next, ScrollStrategy::Center);
        cx.notify();
    }

    fn choose(&mut self, index: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = index
            .and_then(|index| self.filtered.get(index))
            .and_then(|row| self.list.get(*row))
            .cloned()
        else {
            return;
        };
        if let Some(layers) = self.layers.upgrade() {
            layers.update(cx, |layers, cx| layers.close(window, cx));
        }
        (self.on_chosen)(&entry, window, cx);
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(layers) = self.layers.upgrade() {
            layers.update(cx, |layers, cx| layers.close(window, cx));
        }
    }
}

impl Render for CountryBox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let st = &DEFAULT_MULTI_SELECT;
        let search_height =
            px_of(st.field.height_min) + px_of(st.padding.top) + px_of(st.padding.bottom);
        let window_room = window.viewport_size().height - px(WINDOW_MARGIN);
        let list_height = (px_of(BOX_MAX_LIST_HEIGHT) - search_height)
            .min(window_room)
            .max(px(100.0));
        let search = div()
            .bg(color(cx, st.bg))
            .pl(px_of(st.padding.left))
            .pr(px_of(st.padding.right))
            .pt(px_of(st.padding.top))
            .pb(px_of(st.padding.bottom))
            .flex()
            .items_center()
            .gap(px_of(st.field_icon_skip))
            .child(ui::icon(st.field_icon))
            .child(div().flex_1().child(self.search.clone()));

        let list = if self.filtered.is_empty() {
            div()
                .h(px_of(NO_CONTACTS_HEIGHT))
                .flex()
                .items_center()
                .justify_center()
                .font(font(NO_CONTACTS_FONT))
                .text_size(px_of(NO_CONTACTS_FONT.size))
                .text_color(color(cx, NO_CONTACTS_COLOR))
                .child(tr(cx, key::lng_country_none))
                .into_any_element()
        } else {
            let entity = cx.entity();
            let rows = self.list.clone();
            let filtered = self.filtered.clone();
            let selected = self.selected;
            uniform_list("countries", filtered.len(), move |range, _, cx| {
                range
                    .map(|index| {
                        row(
                            &entity,
                            &rows[filtered[index]],
                            index,
                            selected == Some(index),
                            cx,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .track_scroll(&self.scroll)
            .pt(px_of(COUNTRIES_SKIP))
            .h(list_height)
            .into_any_element()
        };

        let entity = cx.entity();
        div()
            .key_context(CONTEXT)
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.select_skip(1, cx)))
            .on_action(cx.listener(|this, _: &SelectPrevious, _, cx| this.select_skip(-1, cx)))
            .child(
                BoxFrame::new("country-box", &DEFAULT_BOX)
                    .title(tr(cx, key::lng_country_select))
                    .row(search)
                    .row(list)
                    .button(
                        RoundButton::new(
                            "country-close",
                            &DEFAULT_BOX.button,
                            tr(cx, key::lng_close),
                        )
                        .on_click(move |_, window, cx| {
                            entity.update(cx, |this, cx| this.close(window, cx))
                        }),
                    ),
            )
    }
}

fn row(
    entity: &Entity<CountryBox>,
    entry: &Entry,
    index: usize,
    is_selected: bool,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    let padding = COUNTRY_ROW_PADDING;
    let code_fg = if is_selected {
        COUNTRY_ROW_CODE_FG_OVER
    } else {
        COUNTRY_ROW_CODE_FG
    };
    let bg = if is_selected {
        COUNTRY_ROW_BG_OVER
    } else {
        COUNTRY_ROW_BG
    };
    let hover_entity = entity.clone();
    let click_entity = entity.clone();
    div()
        .id(("country", index))
        .h(px_of(COUNTRY_ROW_HEIGHT))
        .w_full()
        .bg(color(cx, bg))
        .pl(px_of(padding.left))
        .pt(px_of(padding.top))
        .flex()
        .gap(px_of(padding.right))
        .cursor_pointer()
        .child(
            div()
                .font(font(COUNTRY_ROW_NAME_FONT))
                .text_size(px_of(COUNTRY_ROW_NAME_FONT.size))
                .line_height(font_height(COUNTRY_ROW_NAME_FONT))
                .text_color(color(cx, COUNTRY_ROW_NAME_FG))
                .text_ellipsis()
                .overflow_hidden()
                .whitespace_nowrap()
                .child(entry.name.clone()),
        )
        .child(
            div()
                .flex_none()
                .font(font(COUNTRY_ROW_CODE_FONT))
                .text_size(px_of(COUNTRY_ROW_CODE_FONT.size))
                .line_height(font_height(COUNTRY_ROW_CODE_FONT))
                .text_color(color(cx, code_fg))
                .child(format!("+{}", entry.code)),
        )
        .on_hover(move |hovered, _, cx| {
            if *hovered {
                hover_entity.update(cx, |this, cx| {
                    if this.selected != Some(index) {
                        this.selected = Some(index);
                        cx.notify();
                    }
                });
            }
        })
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, window, cx| {
            click_entity.update(cx, |this, cx| this.choose(Some(index), window, cx));
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn country(code: &str, name: &str, english: &str, codes: &[&str]) -> Country {
        Country {
            code: code.to_owned(),
            name: name.to_owned(),
            english_name: english.to_owned(),
            flag: String::new(),
            is_hidden: false,
            calling_codes: codes.iter().map(|code| (*code).to_owned()).collect(),
        }
    }

    #[test]
    fn lists_the_first_country_on_top_with_a_row_per_code() {
        let countries = [
            country("AG", "Antigua & Barbuda", "Antigua & Barbuda", &["1268"]),
            country("US", "USA", "United States", &["1"]),
        ];
        let list = entries(&countries, "US");
        assert_eq!(list[0].iso2, "US");
        assert_eq!(list[1].code, "1268");
    }

    #[test]
    fn search_matches_word_prefixes_in_both_names() {
        let list = entries(&[country("US", "США", "United States", &["1"])], "");
        assert!(matches(&list[0], &search_words("uni sta")));
        assert!(matches(&list[0], &search_words("сш")));
        assert!(!matches(&list[0], &search_words("nited")));
    }
}
