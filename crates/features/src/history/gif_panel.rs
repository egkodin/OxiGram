//! The saved GIFs tab, with stills for visible tiles and one hovered preview.

use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, Image, InteractiveElement as _,
    IntoElement as _, ListAlignment, ListState, ObjectFit, ParentElement as _, RenderImage,
    StatefulInteractiveElement as _, Styled as _, StyledImage as _, Subscription, Window, div, img,
    list, px,
};
use i18n::key;
use model::rights::Writing;
use model::saved_gifs::SavedGif;
use model::{FileId, MessageId, RequestError, Session};
use style::{ColorId, ColorRef};

use super::HistoryView;
use super::field_bar::FieldBar;
use super::media_video::{Ending, VideoPlayback};
use crate::lang::tr;
use crate::streaming::Playable;
use ui::color;

const COLUMNS: usize = 3;

fn still_preview(jpeg: Vec<u8>) -> Option<Arc<Image>> {
    if jpeg.len() > 16_384 {
        return None;
    }
    let (width, height) =
        image::ImageReader::with_format(Cursor::new(&jpeg), image::ImageFormat::Jpeg)
            .into_dimensions()
            .ok()?;
    (width <= 40 && height <= 40).then(|| ui::jpeg_image(jpeg))
}

#[derive(Default)]
enum Saved {
    #[default]
    Idle,
    Loading,
    Ready(Vec<(SavedGif, Option<Arc<Image>>)>),
    Failed,
}

pub(super) struct GifPanel {
    owner: Option<Session>,
    saved: Saved,
    list: ListState,
    sending: bool,
    send_failed: bool,
    hovered: Option<FileId>,
    playback: Option<(Entity<VideoPlayback>, Subscription)>,
    seen: bool,
}

impl GifPanel {
    pub(super) fn new() -> Self {
        Self {
            owner: None,
            saved: Saved::Idle,
            list: ListState::new(0, ListAlignment::Top, px(0.0)),
            sending: false,
            send_failed: false,
            hovered: None,
            playback: None,
            seen: false,
        }
    }

    pub(super) fn stop_preview(&mut self) {
        self.hovered = None;
        self.playback = None;
    }

    pub(super) fn tick(&mut self, window: &mut Window, cx: &App) {
        if !std::mem::take(&mut self.seen) {
            self.stop_preview();
        } else if self
            .playback
            .as_ref()
            .is_some_and(|(video, _)| video.read(cx).needs_redraw())
        {
            window.request_animation_frame();
        }
    }

    fn belongs_to(&self, session: &Session) -> bool {
        self.owner
            .as_ref()
            .is_some_and(|owner| owner.client_id() == session.client_id())
    }
}

impl HistoryView {
    fn load_saved_gifs(&mut self, cx: &mut Context<Self>) {
        let session = self.store.read(cx).session().clone();
        let panel = &mut self.emoji.gifs;
        if !panel.belongs_to(&session) {
            *panel = GifPanel::new();
            panel.owner = Some(session.clone());
        }
        if !matches!(panel.saved, Saved::Idle) {
            return;
        }
        panel.saved = Saved::Loading;
        let request = session.saved_gifs();
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                let panel = &mut this.emoji.gifs;
                if !panel.belongs_to(&session)
                    || this.store.read(cx).session().client_id() != session.client_id()
                {
                    return;
                }
                panel.saved = match result {
                    Ok(gifs) => {
                        panel.list.reset(gifs.len().div_ceil(COLUMNS));
                        Saved::Ready(
                            gifs.into_iter()
                                .map(|mut gif| {
                                    let preview = gif
                                        .preview
                                        .take()
                                        .and_then(|preview| still_preview(preview.jpeg));
                                    (gif, preview)
                                })
                                .collect(),
                        )
                    }
                    Err(error) => {
                        tracing::warn!(%error, "loading saved GIFs failed");
                        Saved::Failed
                    }
                };
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn saved_gifs(&mut self, cx: &mut Context<Self>) -> AnyElement {
        self.load_saved_gifs(cx);
        let panel = &self.emoji.gifs;
        let empty = |label| {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(label)
        };
        match &panel.saved {
            Saved::Idle | Saved::Loading => {
                empty(tr(cx, key::lng_preview_loading)).into_any_element()
            }
            Saved::Ready(gifs) if gifs.is_empty() => {
                empty(tr(cx, key::lng_gifs_no_saved)).into_any_element()
            }
            Saved::Failed => empty(tr(cx, key::lng_bot_download_retry))
                .id("retry-saved-gifs")
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.emoji.gifs.saved = Saved::Idle;
                    this.load_saved_gifs(cx);
                    cx.notify();
                }))
                .into_any_element(),
            Saved::Ready(_) => {
                let weak = cx.weak_entity();
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .children(
                        panel
                            .send_failed
                            .then(|| div().px(px(8.0)).child(tr(cx, key::lng_attach_failed))),
                    )
                    .child(
                        list(panel.list.clone(), move |index, _, cx: &mut App| {
                            weak.update(cx, |this, cx| this.saved_gif_row(index, cx))
                                .unwrap_or_else(|_| div().into_any_element())
                        })
                        .flex_1()
                        .min_h_0(),
                    )
                    .into_any_element()
            }
        }
    }

    fn saved_gif_row(&mut self, row: usize, cx: &mut Context<Self>) -> AnyElement {
        let Saved::Ready(gifs) = &self.emoji.gifs.saved else {
            return div().into_any_element();
        };
        let Some(owner) = self.emoji.gifs.owner.clone() else {
            return div().into_any_element();
        };
        if owner.client_id() != self.store.read(cx).session().client_id() {
            return div().into_any_element();
        }
        let Some(gifs) = gifs.chunks(COLUMNS).nth(row) else {
            return div().into_any_element();
        };
        let hovered = self.emoji.gifs.hovered;
        let playing = gifs
            .iter()
            .find(|(gif, _)| hovered == Some(gif.file.id))
            .map(|(gif, _)| gif.clone());
        let animated = playing.as_ref().and_then(|gif| self.gif_frame(gif, cx));
        let Saved::Ready(gifs) = &self.emoji.gifs.saved else {
            return div().into_any_element();
        };
        let gifs = &gifs[row * COLUMNS..];
        let disabled = self.emoji.gifs.sending
            || matches!(
                self.compose.bar,
                Some(FieldBar::Edit { .. } | FieldBar::Forward(_))
            );
        div()
            .flex()
            .gap(px(4.0))
            .p(px(4.0))
            .children(
                gifs.iter()
                    .take(COLUMNS)
                    .enumerate()
                    .map(|(column, (gif, preview))| {
                        let index = row * COLUMNS + column;
                        let (owner, file) = (owner.clone(), gif.file.id);
                        let frame = animated.as_ref().filter(|_| hovered == Some(file)).cloned();
                        let path = frame
                            .is_none()
                            .then(|| self.gif_thumbnail(gif, cx))
                            .flatten();
                        let picture = frame
                            .map(img)
                            .or_else(|| path.map(img))
                            .or_else(|| preview.clone().map(img));
                        let needs_placeholder = picture.is_none();
                        div()
                            .id(("saved-gif", index))
                            .flex_1()
                            .min_w_0()
                            .h(px(92.0))
                            .rounded(px(4.0))
                            .overflow_hidden()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(color(cx, ColorRef::Palette(ColorId::WindowBgOver)))
                            .children(picture.map(|picture| {
                                let preview = preview.clone();
                                super::picture_cache::bounded(picture, cx)
                                    .object_fit(ObjectFit::Cover)
                                    .size_full()
                                    .with_loading(move || {
                                        preview.clone().map_or_else(
                                            || div().into_any_element(),
                                            |preview| {
                                                img(preview)
                                                    .object_fit(ObjectFit::Cover)
                                                    .size_full()
                                                    .into_any_element()
                                            },
                                        )
                                    })
                            }))
                            .on_hover(cx.listener(move |this, over: &bool, _, cx| {
                                let panel = &mut this.emoji.gifs;
                                if *over && panel.hovered != Some(file) {
                                    panel.stop_preview();
                                    panel.hovered = Some(file);
                                    panel.seen = true;
                                    cx.notify();
                                } else if !*over && panel.hovered == Some(file) {
                                    panel.stop_preview();
                                    cx.notify();
                                }
                            }))
                            .children(
                                needs_placeholder.then(|| {
                                    div().child(crate::history::duration_text(gif.duration))
                                }),
                            )
                            .when(!disabled, |cell| {
                                cell.cursor_pointer().on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.send_gif(file, &owner, cx);
                                    },
                                ))
                            })
                    }),
            )
            .into_any_element()
    }

    fn gif_frame(&mut self, gif: &SavedGif, cx: &mut Context<Self>) -> Option<Arc<RenderImage>> {
        self.emoji.gifs.seen = true;
        if !crate::power_saving::plays(crate::power_saving::Flag::Animations, cx) {
            self.emoji.gifs.playback = None;
            return None;
        }
        if self.emoji.gifs.playback.is_none() {
            let store = self.store.read(cx);
            let file = store.state().chats.file(gif.file.id).unwrap_or(&gif.file);
            let Some(playable) = Playable::of(store.session(), file) else {
                self.request_file(gif.file.id, cx);
                return None;
            };
            let video = cx.new(|cx| {
                VideoPlayback::new(
                    &playable,
                    Duration::ZERO,
                    (320, 240),
                    Duration::ZERO,
                    Ending::Loop,
                    cx,
                )
            });
            let observer = cx.observe(&video, |_, _, cx| cx.notify());
            self.emoji.gifs.playback = Some((video, observer));
        }
        let (video, _) = self.emoji.gifs.playback.as_ref()?;
        video.update(cx, |video, cx| {
            video.advance(cx);
            video.frame()
        })
    }

    fn gif_thumbnail(&self, gif: &SavedGif, cx: &mut App) -> Option<PathBuf> {
        let file = gif.thumbnail.as_ref()?;
        let known = self
            .store
            .read(cx)
            .state()
            .chats
            .file(file.id)
            .unwrap_or(file);
        if let Some(path) = &known.path {
            return Some(PathBuf::from(path));
        }
        self.store
            .update(cx, |store, cx| store.load_userpic(file.id, 1, cx));
        None
    }

    fn send_gif(&mut self, file: FileId, owner: &Session, cx: &mut Context<Self>) {
        let session = self.store.read(cx).session().clone();
        if self.emoji.gifs.sending
            || owner.client_id() != session.client_id()
            || !self.emoji.gifs.belongs_to(&session)
            || self.store.read(cx).state().chats.writing(self.chat) != Writing::Field
        {
            return;
        }
        let reply = match self.compose.bar {
            Some(FieldBar::Reply(id)) => Some(id),
            None => None,
            _ => return,
        };
        let Saved::Ready(gifs) = &self.emoji.gifs.saved else {
            return;
        };
        let Some((gif, _)) = gifs.iter().find(|(gif, _)| gif.file.id == file) else {
            return;
        };
        let request = session.send_saved_gif(self.key(), reply, gif);
        self.emoji.gifs.sending = true;
        self.emoji.gifs.send_failed = false;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.finish_gif_send(&session, reply, result, cx)
            });
        })
        .detach();
    }

    fn finish_gif_send(
        &mut self,
        session: &Session,
        reply: Option<MessageId>,
        result: Result<(), RequestError>,
        cx: &mut Context<Self>,
    ) {
        if !self.emoji.gifs.belongs_to(session)
            || self.store.read(cx).session().client_id() != session.client_id()
        {
            return;
        }
        self.emoji.gifs.sending = false;
        if let Err(error) = result {
            tracing::warn!(%error, "sending a saved GIF failed");
            self.emoji.gifs.send_failed = true;
            cx.notify();
            return;
        }
        if matches!(self.compose.bar, Some(FieldBar::Reply(id)) if Some(id) == reply) {
            self.take_reply();
        }
        self.hide_emoji_panel(cx);
        self.scroll_down(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_picker_rejects_invalid_or_oversized_preview_images() {
        let jpeg = |width| {
            let mut bytes = Cursor::new(Vec::new());
            image::RgbImage::new(width, 4)
                .write_to(&mut bytes, image::ImageFormat::Jpeg)
                .unwrap();
            bytes.into_inner()
        };
        assert!(still_preview(jpeg(40)).is_some());
        assert!(still_preview(jpeg(41)).is_none());
        assert!(still_preview(vec![0; 16_385]).is_none());
        assert!(still_preview(vec![0; 8]).is_none());
    }
}
