//! "Language" (`LanguageBox`): a searchable list of Telegram's languages; a
//! click switches the interface at once, OK closes.
// shortcut: no translation rows on top (M13), no recent / official split, no ⋮ menu (share, delete) or keyboard selection; with M13 and M8 polish.

use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, ParentElement, Render, StatefulInteractiveElement as _, Styled, Subscription,
    Task, Window, div, px,
};
use i18n::key;
use model::language::LanguagePack;
use style::boxes::LANGS_RADIO;
use style::layers::{BOX_MAX_LIST_HEIGHT, DEFAULT_BOX, DEFAULT_BOX_CHECKBOX};
use style::widgets::DEFAULT_MULTI_SELECT;
use style::{ColorId, ColorRef, Font, FontFlags};
use ui::{
    BoxFrame, Checkbox, Checked, InputEvent, InputField, RoundButton, color, font, font_height,
};

use super::Env;
use crate::intro::country_box::search_words;
use crate::lang::tr;
use crate::px_of;

/// `passportRowPadding`, `passportRowSkip`.
const ROW_LEFT: f32 = 22.0;
const ROW_RIGHT: f32 = 25.0;
const ROW_VERTICAL: f32 = 8.0;
const ROW_SKIP: f32 = 2.0;
const TITLE_FONT: Font = Font::new(13, FontFlags::BOLD, "");
const DESCRIPTION_FONT: Font = Font::new(13, FontFlags::NONE, "");
const TITLE_FG: ColorRef = ColorRef::Palette(ColorId::WindowFg);
const DESCRIPTION_FG: ColorRef = ColorRef::Palette(ColorId::WindowSubTextFg);
const ROW_BG_OVER: ColorRef = ColorRef::Palette(ColorId::WindowBgOver);
/// Room the box leaves around itself in a short window.
const WINDOW_MARGIN: f32 = 160.0;

/// Every query word starts a word of the name or the native name
/// (`Rows::filter`).
fn matches(pack: &LanguagePack, query: &[String]) -> bool {
    let words = search_words(&format!("{} {}", pack.name, pack.native_name));
    query
        .iter()
        .all(|word| words.iter().any(|name| name.starts_with(word.as_str())))
}

pub(super) struct LanguageBox {
    env: Env,
    search: Entity<InputField>,
    /// None while TDLib is asked.
    packs: Option<Rc<Vec<LanguagePack>>>,
    chosen: String,
    _load: Task<()>,
    _search: Subscription,
}

impl Focusable for LanguageBox {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.focus_handle(cx)
    }
}

pub(crate) fn show(env: &Env, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx: &mut Context<LanguageBox>| {
        let search = cx.new(|cx| {
            InputField::new(
                &DEFAULT_MULTI_SELECT.field,
                tr(cx, key::lng_participant_filter),
                window,
                cx,
            )
        });
        let subscription = cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Changed) {
                cx.notify();
            }
        });
        let task = crate::store::spawn(env.targets.store.read(cx).session().language_packs(), cx);
        let load = cx.spawn(async move |this, cx| match task.await {
            Ok(packs) => {
                let _ = this.update(cx, |this, cx| {
                    this.packs = Some(Rc::new(packs));
                    cx.notify();
                });
            }
            Err(error) => tracing::warn!(%error, "reading the languages failed"),
        });
        LanguageBox {
            env: env.clone(),
            search,
            packs: None,
            chosen: crate::lang::strings(cx).language().to_owned(),
            _load: load,
            _search: subscription,
        }
    });
    let field = view.read(cx).search.clone();
    env.targets
        .layers
        .update(cx, |layers, cx| layers.show_above(view, window, cx));
    crate::focus(&field, window, cx);
}

impl LanguageBox {
    fn choose(&mut self, id: String, cx: &mut Context<Self>) {
        if id == self.chosen {
            return;
        }
        let session = self.env.targets.store.read(cx).session().clone();
        crate::lang::switch_to(&session, &id, cx);
        self.chosen = id;
        cx.notify();
    }

    fn row(
        &self,
        index: usize,
        pack: &LanguagePack,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let checked = if pack.id == self.chosen {
            Checked::Yes
        } else {
            Checked::No
        };
        let (row_click, radio_click) = (cx.weak_entity(), cx.weak_entity());
        let (id, radio_id) = (pack.id.clone(), pack.id.clone());
        let text = |value: &str, st: Font, fg: ColorRef| {
            div()
                .font(font(st))
                .text_size(px_of(st.size))
                .line_height(font_height(st))
                .text_color(color(cx, fg))
                .truncate()
                .child(value.to_owned())
        };
        div()
            .id(("language", index))
            .flex()
            .items_center()
            .gap(px(ROW_LEFT))
            .pl(px(ROW_LEFT))
            .pr(px(ROW_RIGHT))
            .py(px(ROW_VERTICAL))
            .cursor_pointer()
            .hover(|row| row.bg(color(cx, ROW_BG_OVER)))
            .on_click(move |_, _, cx| {
                let _ = row_click.update(cx, |this, cx| this.choose(id.clone(), cx));
            })
            .child(
                div().flex_none().w(px_of(LANGS_RADIO.diameter)).child(
                    Checkbox::radio(
                        ("language-radio", index),
                        &DEFAULT_BOX_CHECKBOX,
                        &LANGS_RADIO,
                        "",
                        checked,
                    )
                    .on_toggle(move |_, cx| {
                        let _ =
                            radio_click.update(cx, |this, cx| this.choose(radio_id.clone(), cx));
                    }),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .gap(px(ROW_SKIP))
                    .child(text(&pack.name, TITLE_FONT, TITLE_FG))
                    .child(text(&pack.native_name, DESCRIPTION_FONT, DESCRIPTION_FG)),
            )
    }
}

impl Render for LanguageBox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let st = &DEFAULT_MULTI_SELECT;
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
        let query = search_words(self.search.read(cx).text());
        let packs = self.packs.clone().unwrap_or_default();
        let rows: Vec<_> = packs
            .iter()
            .filter(|pack| matches(pack, &query))
            .enumerate()
            .map(|(index, pack)| self.row(index, pack, cx))
            .collect();
        let empty = (self.packs.is_some() && rows.is_empty()).then(|| {
            div()
                .py(px(ROW_VERTICAL * 2.0))
                .text_center()
                .text_color(color(cx, DESCRIPTION_FG))
                .child(tr(cx, key::lng_languages_none))
        });
        let height =
            (px_of(BOX_MAX_LIST_HEIGHT)).min(window.viewport_size().height - px(WINDOW_MARGIN));
        let list = div()
            .id("languages")
            .flex()
            .flex_col()
            .max_h(height)
            .overflow_y_scroll()
            .children(rows)
            .children(empty);
        let layers = self.env.targets.layers.clone();
        BoxFrame::new("language-box", &DEFAULT_BOX)
            .title(tr(cx, key::lng_languages))
            .row(search)
            .row(list)
            .button(
                RoundButton::new("language-ok", &DEFAULT_BOX.button, tr(cx, key::lng_box_ok))
                    .on_click(move |_, window, cx| {
                        layers.update(cx, |layers, cx| layers.close(window, cx));
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_words_start_words_of_either_name() {
        let pack = LanguagePack {
            id: "pt-br".to_owned(),
            name: "Portuguese (Brazil)".to_owned(),
            native_name: "Português (Brasil)".to_owned(),
            is_official: true,
        };
        assert!(matches(&pack, &search_words("portu")));
        assert!(matches(&pack, &search_words("Portug (brasil")));
        assert!(!matches(&pack, &search_words("razil")));
        assert!(matches(&pack, &[]));
    }
}
