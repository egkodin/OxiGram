//! Recently searched hashtags over the search results while a hashtag is
//! typed (`InnerWidget::HashtagResult`).

use gpui::{
    AnyElement, Context, EventEmitter, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder as _,
};
use style::basic::SMALL_CLOSE_ICON_OVER;
use style::chat_helpers::{
    MENTION_BG_OVER, MENTION_FG, MENTION_FG_ACTIVE, MENTION_FG_OVER, MENTION_FG_OVER_ACTIVE,
    MENTION_FONT, MENTION_HEIGHT, MENTION_TOP,
};
use style::dialogs::DEFAULT_DIALOG_ROW;
use style::{ColorId, ColorRef};
use ui::{color, font, font_height, icon, icon_size};

use super::{Dialogs, RowKey};
use crate::{hashtags, px_of, settings};

/// Puts `0` in the search field; the main view searches for it
/// (`_completeHashtagRequests`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompleteQuery(pub String);

impl EventEmitter<CompleteQuery> for Dialogs {}

/// The recent hashtags that continue `query`.
pub(super) fn hashtags_for(query: &str, cx: &gpui::App) -> Vec<String> {
    hashtags::matching(&settings::recent_search_hashtags(cx), query)
}

impl Dialogs {
    /// Remembers the query's hashtags once a found message is opened.
    pub(super) fn save_found_hashtags(&self, cx: &mut Context<Self>) {
        if let Some(search) = &self.search {
            settings::save_search_hashtags(&search.query, cx);
        }
    }

    fn choose_hashtag(&mut self, tag: &str, cx: &mut Context<Self>) {
        let text = format!("#{tag}");
        settings::save_search_hashtags(&text, cx);
        cx.emit(CompleteQuery(format!("{text} ")));
    }

    fn remove_hashtag(&mut self, tag: &str, cx: &mut Context<Self>) {
        settings::remove_search_hashtag(tag, cx);
        if let Some(search) = &mut self.search {
            let tags = hashtags_for(&search.query, cx);
            search.set_hashtags(tags);
        }
        cx.notify();
    }

    /// Recent hashtag `index`: the typed part in the active color, the
    /// rest muted, and a cross under the pointer that forgets it.
    pub(super) fn hashtag_row(&mut self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(search) = &self.search else {
            return div().into_any_element();
        };
        let Some(tag) = search.hashtags.get(index).cloned() else {
            return div().into_any_element();
        };
        let key = RowKey::Hashtag(index);
        let is_hovered = self.hovered == Some(key);
        // The query is `#` and the typed start of the tag.
        let typed = search.query.chars().count().saturating_sub(1);
        let split = tag
            .char_indices()
            .nth(typed)
            .map_or(tag.len(), |(at, _)| at);
        let (first, second) = if typed == 0 {
            (String::new(), format!("#{tag}"))
        } else {
            (format!("#{}", &tag[..split]), tag[split..].to_owned())
        };
        let (fg_active, fg) = if is_hovered {
            (MENTION_FG_OVER_ACTIVE, MENTION_FG_OVER)
        } else {
            (MENTION_FG_ACTIVE, MENTION_FG)
        };
        let close = icon_size(SMALL_CLOSE_ICON_OVER);
        let close_skip = (px_of(MENTION_HEIGHT) - close.height) / 2.0;
        let (choose_tag, remove_tag) = (tag.clone(), tag);
        div()
            .id(key.element_id())
            .relative()
            .h(px_of(MENTION_HEIGHT))
            .pt(px_of(MENTION_TOP))
            .pl(px_of(DEFAULT_DIALOG_ROW.padding.left))
            .pr(px_of(DEFAULT_DIALOG_ROW.padding.right))
            .bg(color(
                cx,
                if is_hovered {
                    MENTION_BG_OVER
                } else {
                    ColorRef::Palette(ColorId::DialogsBg)
                },
            ))
            .flex()
            .overflow_hidden()
            .whitespace_nowrap()
            .font(font(MENTION_FONT))
            .text_size(px_of(MENTION_FONT.size))
            .line_height(font_height(MENTION_FONT))
            .cursor_pointer()
            .child(
                div()
                    .flex_none()
                    .text_color(color(cx, fg_active))
                    .child(first),
            )
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_color(color(cx, fg))
                    .child(second),
            )
            .when(is_hovered, |row| {
                row.child(
                    div()
                        .id("forget-hashtag")
                        .absolute()
                        .top(close_skip)
                        .right(close_skip)
                        .child(icon(SMALL_CLOSE_ICON_OVER))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.remove_hashtag(&remove_tag, cx);
                        })),
                )
            })
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                this.track_hover(key, *hovered, cx);
            }))
            .on_click(cx.listener(move |this, _, _, cx| this.choose_hashtag(&choose_tag, cx)))
            .into_any_element()
    }
}
