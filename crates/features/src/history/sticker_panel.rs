//! Installed sticker packs in the compose picker.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, ImageSource, InteractiveElement as _, IntoElement as _,
    ListAlignment, ListOffset, ListState, ObjectFit, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, StyledImage as _, div, img, list,
    prelude::FluentBuilder as _, px,
};
use i18n::key;
use model::rights::Writing;
use model::sticker::StickerKind;
use model::sticker_set::{SetKind, SetSticker};
use model::{ClientId, FileId};
use style::{ColorId, ColorRef};

use super::HistoryView;
use super::emoji_packs::{self, PackNav, Packs};
use super::field_bar::FieldBar;
use crate::lang::tr;
use crate::power_saving::{Flag, plays};
use crate::store::spawn;

const COLUMNS: usize = 5;
const SIDE: f32 = 48.0;

#[derive(Default)]
enum PacksState {
    #[default]
    Idle,
    Loading,
    Ready(Packs),
    Failed,
}

pub(super) struct StickerPanel {
    packs: PacksState,
    rows: Vec<(usize, Option<usize>)>,
    list: ListState,
    owner: Option<ClientId>,
    pack_page: usize,
    hovered: Option<FileId>,
    sending: bool,
    send_failed: bool,
}

impl StickerPanel {
    pub(super) fn new() -> Self {
        Self {
            packs: PacksState::Idle,
            rows: Vec::new(),
            list: ListState::new(0, ListAlignment::Top, px(0.0)),
            owner: None,
            pack_page: 0,
            hovered: None,
            sending: false,
            send_failed: false,
        }
    }

    pub(super) fn clear_hover(&mut self) {
        self.hovered = None;
    }
}

impl HistoryView {
    fn load_sticker_packs(&mut self, cx: &mut Context<Self>) {
        let session = self.store.read(cx).session().clone();
        let owner = session.client_id();
        let panel = &mut self.emoji.stickers;
        if panel.owner != Some(owner) {
            *panel = StickerPanel::new();
            panel.owner = Some(owner);
        }
        if !matches!(panel.packs, PacksState::Idle) {
            return;
        }
        panel.packs = PacksState::Loading;
        let task = spawn(
            async move { emoji_packs::installed_packs(&session, SetKind::Stickers).await },
            cx,
        );
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.store.read(cx).session().client_id() != owner {
                    return;
                }
                let panel = &mut this.emoji.stickers;
                panel.packs = match result {
                    Ok(packs) => {
                        panel.rows = packs
                            .iter()
                            .enumerate()
                            .filter(|(_, pack)| !pack.stickers.is_empty())
                            .flat_map(|(index, pack)| {
                                std::iter::once((index, None)).chain(
                                    (0..pack.stickers.len())
                                        .step_by(COLUMNS)
                                        .map(move |start| (index, Some(start))),
                                )
                            })
                            .collect();
                        panel.list.reset(panel.rows.len());
                        PacksState::Ready(Rc::new(packs))
                    }
                    Err(error) => {
                        tracing::warn!(%error, "loading installed stickers failed");
                        PacksState::Failed
                    }
                };
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn installed_stickers(&mut self, cx: &mut Context<Self>) -> AnyElement {
        self.load_sticker_packs(cx);
        let panel = &self.emoji.stickers;
        let center = |text| {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(text)
        };
        match panel.packs {
            PacksState::Idle | PacksState::Loading => {
                center(tr(cx, key::lng_preview_loading)).into_any_element()
            }
            PacksState::Failed => center(tr(cx, key::lng_bot_download_retry))
                .id("retry-installed-stickers")
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.emoji.stickers.packs = PacksState::Idle;
                    this.load_sticker_packs(cx);
                    cx.notify();
                }))
                .into_any_element(),
            PacksState::Ready(_) if panel.rows.is_empty() => {
                center(tr(cx, key::lng_stickers_not_found)).into_any_element()
            }
            PacksState::Ready(ref packs) => {
                let weak = cx.weak_entity();
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .children(
                        panel
                            .send_failed
                            .then(|| div().p(px(8.0)).child(tr(cx, key::lng_attach_failed))),
                    )
                    .child(
                        list(panel.list.clone(), move |row, _, cx: &mut App| {
                            weak.update(cx, |this, cx| this.sticker_row(row, cx))
                                .unwrap_or_else(|_| div().into_any_element())
                        })
                        .flex_1()
                        .min_h_0(),
                    )
                    .children(emoji_packs::pack_strip(
                        packs,
                        &self.store,
                        (
                            panel.pack_page,
                            panel
                                .rows
                                .get(panel.list.logical_scroll_top().item_ix)
                                .map(|row| row.0),
                        ),
                        cx,
                        |this, action, cx| {
                            let panel = &mut this.emoji.stickers;
                            match action {
                                PackNav::Select(pack) => {
                                    if let Some(item_ix) =
                                        panel.rows.iter().position(|row| *row == (pack, None))
                                    {
                                        panel.list.scroll_to(ListOffset {
                                            item_ix,
                                            offset_in_item: px(0.0),
                                        });
                                    }
                                }
                                PackNav::Page(page) => panel.pack_page = page,
                            }
                            cx.notify();
                        },
                    ))
                    .into_any_element()
            }
        }
    }

    fn sticker_row(&mut self, row: usize, cx: &mut Context<Self>) -> AnyElement {
        let panel = &self.emoji.stickers;
        let PacksState::Ready(packs) = &panel.packs else {
            return div().into_any_element();
        };
        let Some(&(pack, start)) = panel.rows.get(row) else {
            return div().into_any_element();
        };
        let packs = Rc::clone(packs);
        let pack = &packs[pack];
        let Some(start) = start else {
            return div()
                .px(px(8.0))
                .py(px(4.0))
                .truncate()
                .child(pack.title.clone())
                .into_any_element();
        };
        div()
            .flex()
            .gap(px(4.0))
            .p(px(4.0))
            .children(
                pack.stickers
                    .iter()
                    .skip(start)
                    .take(COLUMNS)
                    .map(|sticker| self.sticker_tile(sticker, cx)),
            )
            .into_any_element()
    }

    fn sticker_tile(&mut self, sticker: &SetSticker, cx: &mut Context<Self>) -> AnyElement {
        let id = sticker.picture.file.id;
        let hovered = self.emoji.stickers.hovered == Some(id);
        let animated = if hovered
            && sticker.picture.kind == StickerKind::Animated
            && plays(Flag::StickersPanel, cx)
        {
            self.lottie_key(&sticker.picture.file, (SIDE, SIDE), 0, cx)
                .and_then(|key| crate::lottie::frame(&key, crate::lottie::Play::Loop, cx))
                .map(ImageSource::from)
        } else {
            None
        };
        let picture = animated.or_else(|| {
            emoji_packs::still(&self.store, &sticker.picture, cx).map(ImageSource::from)
        });
        let disabled = self.emoji.stickers.sending
            || matches!(
                self.compose.bar,
                Some(FieldBar::Edit { .. } | FieldBar::Forward(_))
            );
        let sticker = sticker.clone();
        let bg = hovered.then(|| ui::color(cx, ColorRef::Palette(ColorId::WindowBgOver)));
        let owner = self.emoji.stickers.owner;
        div()
            .id(("installed-sticker", u64::from(id.0.unsigned_abs())))
            .size(px(SIDE))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.0))
            .map(|tile| match bg {
                Some(bg) => tile.bg(bg),
                None => tile,
            })
            .child(picture.map_or_else(
                || div().child(sticker.emoji.clone()).into_any_element(),
                |source| {
                    super::picture_cache::bounded(img(source), cx)
                        .size_full()
                        .object_fit(ObjectFit::Contain)
                        .into_any_element()
                },
            ))
            .on_hover(cx.listener(move |this, over: &bool, _, cx| {
                let panel = &mut this.emoji.stickers;
                let next = if *over {
                    Some(id)
                } else {
                    panel.hovered.filter(|was| *was != id)
                };
                if panel.hovered != next {
                    panel.hovered = next;
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _, _, cx| {
                if !disabled {
                    this.send_picker_sticker(&sticker, owner, cx);
                }
            }))
            .into_any_element()
    }

    fn send_picker_sticker(
        &mut self,
        sticker: &SetSticker,
        owner: Option<ClientId>,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        if self.emoji.stickers.sending
            || owner != Some(store.session().client_id())
            || store.state().chats.writing(self.chat) != Writing::Field
        {
            return;
        }
        let reply = match self.compose.bar {
            Some(FieldBar::Reply(message)) => Some(message),
            None => None,
            _ => return,
        };
        let task = spawn(store.session().send_sticker(self.key(), reply, sticker), cx);
        self.emoji.stickers.sending = true;
        self.emoji.stickers.send_failed = false;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if owner != Some(this.store.read(cx).session().client_id()) {
                    return;
                }
                this.emoji.stickers.sending = false;
                if let Err(error) = result {
                    tracing::warn!(%error, "sending an installed sticker failed");
                    this.emoji.stickers.send_failed = true;
                    cx.notify();
                    return;
                }
                if matches!(this.compose.bar, Some(FieldBar::Reply(id)) if Some(id) == reply) {
                    this.take_reply();
                }
                this.hide_emoji_panel(cx);
                this.scroll_down(cx);
            });
        })
        .detach();
    }
}
