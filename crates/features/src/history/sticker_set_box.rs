//! A sticker or custom emoji set (`StickerSetBox`): its title, the
//! stickers in rows of five (emoji in rows of eight), and the button that
//! adds or removes it.

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, Focusable, ImageSource,
    IntoElement, ParentElement, Render, Styled, WeakEntity, Window, div, img, prelude::*,
};
use i18n::key;
use model::StickerSetId;
use model::sticker_set::{SetChange, SetKind, StickerSet};
use style::chat_helpers::{
    EMOJI_SET_MAX_HEIGHT, EMOJI_SET_PADDING, EMOJI_SET_SIZE, STICKER_SET_BOX,
    STICKER_SET_BOX_REMOVE_BUTTON, STICKERS_MAX_HEIGHT, STICKERS_PADDING, STICKERS_SIZE,
};
use style::layers::{BOX_TITLE_CLOSE, BOX_WIDE_WIDTH};
use ui::{BoxFrame, IconButton, RoundButton};

use crate::Store;
use crate::lang::tr;
use crate::layers::{Dismiss, Layers};
use crate::px_of;
use crate::store::spawn;

/// `kStickersPerRow`, `kEmojiPerRow`.
const STICKERS_PER_ROW: usize = 5;
const EMOJI_PER_ROW: usize = 8;

pub(super) struct StickerSetBox {
    store: Entity<Store>,
    layers: WeakEntity<Layers>,
    focus: FocusHandle,
    set: Option<StickerSet>,
    is_missing: bool,
}

impl Focusable for StickerSetBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// Opens the box on `set` and asks TDLib for it.
pub(super) fn show(
    store: &Entity<Store>,
    layers: &Entity<Layers>,
    set: StickerSetId,
    window: &mut Window,
    cx: &mut App,
) {
    let weak = layers.downgrade();
    let view = cx.new(|cx| {
        // Pictures arrive as file updates.
        cx.observe(store, |_, _, cx| cx.notify()).detach();
        let mut view = StickerSetBox {
            store: store.clone(),
            layers: weak,
            focus: cx.focus_handle(),
            set: None,
            is_missing: false,
        };
        view.load(set, cx);
        view
    });
    layers.update(cx, |layers, cx| {
        layers.show(view, Dismiss::Allowed, None, window, cx);
    });
}

impl super::HistoryView {
    /// Opens the pack a custom emoji comes from (`ShowStickerSetBox` from
    /// its link).
    pub(super) fn open_emoji_set(
        &self,
        id: model::CustomEmojiId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let task = spawn(self.store.read(cx).session().custom_emoji(vec![id]), cx);
        let (store, layers) = (self.store.clone(), self.layers.clone());
        cx.spawn_in(window, async move |_, cx| {
            let set = match task.await {
                Ok(found) => found.into_iter().find_map(|(_, picture)| picture.set),
                Err(error) => {
                    tracing::warn!(%error, "loading a custom emoji failed");
                    None
                }
            };
            let Some(set) = set else {
                return;
            };
            let _ = cx.update(|window, cx| show(&store, &layers, set, window, cx));
        })
        .detach();
    }
}

/// The button's words, by what the set holds and whether it is added.
fn button_text(set: &StickerSet, cx: &App) -> String {
    let count = set.stickers.len();
    let text = match (set.is_added(), set.kind) {
        (false, SetKind::Emoji) => key::lng_stickers_add_emoji,
        (false, SetKind::Masks) => key::lng_stickers_add_masks,
        (false, SetKind::Stickers) => key::lng_stickers_add_pack,
        (true, _) if set.is_official => return tr(cx, key::lng_about_done).to_string(),
        (true, SetKind::Emoji) => key::lng_custom_emoji_remove_pack_button,
        (true, SetKind::Masks) => key::lng_masks_remove_pack_button,
        (true, SetKind::Stickers) => key::lng_stickers_remove_pack_button,
    };
    // shortcut: the count follows the words; tdesktop draws it on a badge (`AddTextWithBadge`).
    format!("{} {count}", tr(cx, text))
}

impl StickerSetBox {
    fn load(&mut self, set: StickerSetId, cx: &mut Context<Self>) {
        let task = spawn(self.store.read(cx).session().sticker_set(set), cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(set) => this.set = Some(set),
                    Err(error) => {
                        tracing::warn!(%error, "loading a sticker set failed");
                        this.is_missing = true;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(layers) = self.layers.upgrade() {
            layers.update(cx, |layers, cx| layers.close(window, cx));
        }
    }

    /// Adds or removes the set, then closes, as tdesktop's box does.
    // shortcut: removing asks nothing; tdesktop confirms the removal of a set the user made (`MakeConfirmRemoveSetBox`).
    fn change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(set) = &self.set else {
            return;
        };
        if set.is_added() && set.is_official {
            self.close(window, cx);
            return;
        }
        let change = if set.is_added() {
            SetChange::Remove
        } else {
            SetChange::Add
        };
        let task = spawn(
            self.store
                .read(cx)
                .session()
                .change_sticker_set(set.id, change),
            cx,
        );
        cx.spawn(async move |_, _| {
            if let Err(error) = task.await {
                tracing::warn!(%error, "changing a sticker set failed");
            }
        })
        .detach();
        self.close(window, cx);
    }

    fn grid(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(set) = self.set.clone() else {
            return div().h(px_of(STICKERS_MAX_HEIGHT)).into_any_element();
        };
        let (per_row, cell, padding, max) = match set.kind {
            SetKind::Emoji => (
                EMOJI_PER_ROW,
                EMOJI_SET_SIZE,
                EMOJI_SET_PADDING,
                EMOJI_SET_MAX_HEIGHT,
            ),
            SetKind::Stickers | SetKind::Masks => (
                STICKERS_PER_ROW,
                STICKERS_SIZE,
                STICKERS_PADDING,
                STICKERS_MAX_HEIGHT,
            ),
        };
        let rows = set.stickers.len().div_ceil(per_row);
        #[expect(clippy::cast_precision_loss, reason = "a few rows")]
        let height =
            (px_of(cell.height) * rows as f32 + px_of(padding.top) + px_of(padding.bottom))
                .min(px_of(max));
        let cells: Vec<AnyElement> = set
            .stickers
            .iter()
            .map(|sticker| {
                let picture = super::emoji_packs::still(&self.store, &sticker.picture, cx);
                div()
                    .w(px_of(cell.width))
                    .h(px_of(cell.height))
                    .flex()
                    .items_center()
                    .justify_center()
                    .children(picture.map(|path| {
                        img(ImageSource::from(path))
                            .size_full()
                            .object_fit(gpui::ObjectFit::Contain)
                    }))
                    .into_any_element()
            })
            .collect();
        div()
            .id("sticker-set-grid")
            .h(height)
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .pl(px_of(padding.left))
                    .pr(px_of(padding.right))
                    .pt(px_of(padding.top))
                    .pb(px_of(padding.bottom))
                    .children(cells),
            )
            .into_any_element()
    }
}

impl Render for StickerSetBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = match (&self.set, self.is_missing) {
            (Some(set), _) => set.title.clone().into(),
            (None, true) => tr(cx, key::lng_stickers_not_found),
            (None, false) => tr(cx, key::lng_contacts_loading),
        };
        let grid = self.grid(cx);
        let entity = cx.entity();
        let close = entity.clone();
        let frame = BoxFrame::new("sticker-set-box", &STICKER_SET_BOX)
            .width(px_of(BOX_WIDE_WIDTH))
            .title(title)
            .top_button(
                IconButton::new("sticker-set-close", &BOX_TITLE_CLOSE).on_click(
                    move |_, window, cx| close.update(cx, |this, cx| this.close(window, cx)),
                ),
            )
            .row(grid);
        let Some(set) = &self.set else {
            return frame;
        };
        let style = if set.is_added() && !set.is_official {
            &STICKER_SET_BOX_REMOVE_BUTTON
        } else {
            &STICKER_SET_BOX.button
        };
        frame.button(
            RoundButton::new("sticker-set-change", style, button_text(set, cx)).on_click(
                move |_, window, cx| entity.update(cx, |this, cx| this.change(window, cx)),
            ),
        )
    }
}
