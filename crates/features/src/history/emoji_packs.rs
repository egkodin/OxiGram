//! The user's custom emoji packs in the emoji panel, after the standard
//! categories (`EmojiListWidget` with `_custom`): a section per pack.
//! With Premium a click types the emoji; without it a toast says
//! Premium unlocks it.

use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, Entity, ImageSource, InteractiveElement as _, IntoElement as _,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, Window, div, img,
};
use i18n::key;
use model::sticker::{StickerKind, StickerPicture};
use model::sticker_set::{SetKind, SetSticker, StickerSet};
use model::{RequestError, Session};
use style::chat_helpers::{DEFAULT_EMOJI_PAN, EMOJI_PAN_AREA, EMOJI_PAN_RADIUS};
use style::widgets::DEFAULT_TOAST;
use ui::color;

use super::HistoryView;
use crate::Store;
use crate::lang::tr;
use crate::px_of;
use crate::store::spawn;

/// Pictures wait behind what the user opened.
const PICTURE_PRIORITY: i32 = 8;
/// How long the Premium toast stays.
const TOAST_DURATION: std::time::Duration = std::time::Duration::from_secs(3);
/// The custom emoji's side in a cell (`st::emojiPanCustomSize`… at 1x).
const EMOJI_SIDE: f32 = 24.0;

/// A pack with the emoji it holds.
pub(super) type Packs = Rc<Vec<StickerSet>>;

pub(super) async fn installed_packs(
    session: &Session,
    kind: SetKind,
) -> Result<Vec<StickerSet>, RequestError> {
    let ids = session.installed_sticker_sets(kind).await?;
    let mut sets = Vec::with_capacity(ids.len());
    for id in ids {
        sets.push(session.sticker_set(id).await?);
    }
    Ok(sets)
}

/// A still sticker's own file, else its thumbnail, once on disk; asks for
/// it otherwise.
// shortcut: animated custom emoji show their still; tdesktop plays them, which comes with a Lottie player per emoji.
pub(super) fn still(
    store: &Entity<Store>,
    sticker: &StickerPicture,
    cx: &mut App,
) -> Option<PathBuf> {
    let own = (sticker.kind == StickerKind::Still).then_some(&sticker.file);
    let file = own.or(sticker.still.as_ref())?;
    let known = store.read(cx).state().chats.file(file.id).unwrap_or(file);
    if let Some(path) = &known.path {
        return Some(PathBuf::from(path));
    }
    let id = file.id;
    store.update(cx, |store, cx| store.load_userpic(id, PICTURE_PRIORITY, cx));
    None
}

impl HistoryView {
    /// Loads the user's emoji packs once per chat, then lays the panel out
    /// again with them.
    pub(super) fn load_emoji_packs(&mut self, cx: &mut Context<Self>) {
        if self.emoji.has_asked_packs {
            return;
        }
        self.emoji.has_asked_packs = true;
        let session = self.store.read(cx).session().clone();
        let task = spawn(
            async move { installed_packs(&session, SetKind::Emoji).await },
            cx,
        );
        cx.spawn(async move |this, cx| {
            let sets = match task.await {
                Ok(sets) => sets,
                Err(error) => {
                    tracing::warn!(%error, "loading the emoji packs failed");
                    return;
                }
            };
            let _ = this.update(cx, |this, cx| {
                this.emoji.packs = Rc::new(sets);
                this.relayout_emoji_panel(cx);
            });
        })
        .detach();
    }

    fn is_premium(&self, cx: &App) -> bool {
        let state = self.store.read(cx).state();
        state
            .me
            .and_then(|me| state.chats.user(me))
            .is_some_and(|me| me.is_premium)
    }

    /// Types the emoji's glyph and remembers which emoji it stands for;
    /// without Premium says that Premium unlocks it.
    fn choose_custom_emoji(
        &mut self,
        sticker: &SetSticker,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = sticker.custom else {
            return;
        };
        if !self.is_premium(cx) {
            // shortcut: the toast has no "Telegram Premium" link to the Premium page; with links in toasts.
            let text = tr(cx, key::lng_animated_emoji_text).replace("**", "");
            self.layers.update(cx, |layers, cx| {
                layers.show_toast(text.into(), &DEFAULT_TOAST, TOAST_DURATION, cx);
            });
            return;
        }
        self.compose.field.update(cx, |field, cx| {
            field.insert_tagged(&sticker.emoji, id.0, cx);
        });
        self.focus_field(window, cx);
    }

    /// A pack's emoji in its cell; hovered like the standard ones.
    pub(super) fn custom_emoji_cell(
        &mut self,
        (index, column): (usize, usize),
        sticker: SetSticker,
        (width, height): (gpui::Pixels, gpui::Pixels),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let area_width = px_of(EMOJI_PAN_AREA.width);
        let area_height = px_of(EMOJI_PAN_AREA.height);
        let is_hovered = self.emoji.hovered == Some((index, column));
        let animated = if sticker.picture.kind == StickerKind::Animated
            && crate::power_saving::plays(crate::power_saving::Flag::EmojiPanel, cx)
        {
            let size = (EMOJI_SIDE, EMOJI_SIDE);
            self.lottie_key(&sticker.picture.file, size, 0, cx)
                .and_then(|key| crate::lottie::frame(&key, crate::lottie::Play::Loop, cx))
                .map(ImageSource::from)
        } else {
            None
        };
        let picture =
            animated.or_else(|| still(&self.store, &sticker.picture, cx).map(ImageSource::from));
        let side = gpui::px(EMOJI_SIDE);
        let mut area = div()
            .absolute()
            .left((width - area_width) / 2.0)
            .top((height - area_height) / 2.0)
            .w(area_width)
            .h(area_height)
            .rounded(px_of(EMOJI_PAN_RADIUS))
            .flex()
            .items_center()
            .justify_center()
            .children(
                picture.map(|source| super::picture_cache::bounded(img(source), cx).size(side)),
            );
        if is_hovered {
            area = area.bg(color(cx, DEFAULT_EMOJI_PAN.over_bg));
        }
        div()
            .id(("custom-emoji", index * 64 + column))
            .relative()
            .flex_none()
            .w(width)
            .h(height)
            .on_hover(cx.listener(move |this, is_over: &bool, _, cx| {
                this.hover_emoji((index, column), *is_over, cx);
            }))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.choose_custom_emoji(&sticker, window, cx);
            }))
            .child(area)
            .into_any_element()
    }
}
