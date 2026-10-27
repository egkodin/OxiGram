//! The box files go through before sending (`SendFilesBox`): a row per
//! file, "Group items" and "Send as documents", the caption, and Send.

use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, Focusable, IntoElement,
    ObjectFit, Render, Styled, StyledImage as _, Subscription, WeakEntity, Window, div, img,
    prelude::*, px,
};
use i18n::key;
use model::send_files::{Attachment, AttachmentKind, Grouping};
use style::Font;
use style::basic::ROUND_RADIUS_SMALL;
use style::basic::{NORMAL_FONT, SEMIBOLD_FONT};
use style::boxes::{
    BOX_PHOTO_CAPTION_SKIP, BOX_PHOTO_COMPRESSED_SKIP, BOX_PHOTO_PADDING, SEND_MEDIA_PREVIEW_SIZE,
    SEND_MEDIA_ROW_SKIP,
};
use style::chat::{ATTACH_PREVIEW_LAYOUT, ATTACH_PREVIEW_THUMB_LAYOUT};
use style::chat_helpers::DEFAULT_COMPOSE_FILES;
use style::layers::{BOX_MAX_LIST_HEIGHT, BOX_WIDE_WIDTH, DEFAULT_BOX};
use ui::{BoxFrame, Checkbox, Checked, RoundButton, TextArea, TextAreaEvent, color, font};

use super::HistoryView;
use super::attach::Prepared;
use super::field_bar::FieldBar;
use crate::file_sizes::size_text;
use crate::lang::{tr, tr_plural};
use crate::layers::{Dismiss, Layers, OnDismiss};
use crate::px_of;
use crate::store::spawn;

/// Whether photos and videos go compressed or as files (`SendFilesWay`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Compression {
    Compressed,
    AsFiles,
}

type OnSend = Rc<dyn Fn(Vec<Attachment>, String, Grouping, &mut Window, &mut App)>;
/// Takes back the caption of a box closed unsent.
type GiveBack = Rc<dyn Fn(&str, &mut App)>;

pub(super) struct SendFilesBox {
    layers: WeakEntity<Layers>,
    files: Vec<Prepared>,
    compression: Compression,
    grouping: Grouping,
    caption: Entity<TextArea>,
    on_send: OnSend,
    give_back: GiveBack,
    _subscription: Subscription,
}

impl Focusable for SendFilesBox {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.caption.focus_handle(cx)
    }
}

/// "Send an image", "Send a video file", "Send as a file", or how many
/// images or files (`refreshTitleText`).
fn title(files: &[Prepared], cx: &App) -> String {
    let is_photo = |file: &Prepared| matches!(file.kind, AttachmentKind::Photo { .. });
    match files {
        [only] => tr(
            cx,
            match only.kind {
                AttachmentKind::Photo { .. } => key::lng_send_image,
                AttachmentKind::Video => key::lng_send_video,
                AttachmentKind::File => key::lng_send_file,
            },
        )
        .to_string(),
        _ => {
            let key = if files.iter().all(is_photo) {
                key::lng_send_images_selected
            } else {
                key::lng_send_files_selected
            };
            tr_plural(cx, key, i64::try_from(files.len()).unwrap_or(i64::MAX), &[])
        }
    }
}

impl SendFilesBox {
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(layers) = self.layers.upgrade() {
            layers.update(cx, |layers, cx| layers.close(window, cx));
        }
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.caption.read(cx).text().to_owned();
        (self.give_back)(&text, cx);
        self.close(window, cx);
    }

    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let attachments = self
            .files
            .iter()
            .map(|file| Attachment {
                path: file.path.clone(),
                kind: match self.compression {
                    Compression::Compressed => file.kind,
                    Compression::AsFiles => AttachmentKind::File,
                },
            })
            .collect();
        let caption = self.caption.read(cx).text().to_owned();
        self.close(window, cx);
        (self.on_send)(attachments, caption, self.grouping, window, cx);
    }

    fn has_media(&self) -> bool {
        self.files
            .iter()
            .any(|file| !matches!(file.kind, AttachmentKind::File))
    }

    /// The file's picture: its thumbnail for a photo, else an icon on a
    /// circle (`AbstractSingleFilePreview::paintEvent`).
    fn thumb(file: &Prepared, cx: &App) -> AnyElement {
        let st = &DEFAULT_COMPOSE_FILES;
        if let AttachmentKind::Photo { .. } = file.kind {
            let size = px_of(ATTACH_PREVIEW_THUMB_LAYOUT.thumb_size);
            return img(file.path.clone())
                .flex_none()
                .size(size)
                .rounded(px_of(ROUND_RADIUS_SMALL))
                .object_fit(ObjectFit::Cover)
                .into_any_element();
        }
        let icon = match file.kind {
            AttachmentKind::Video => st.icon_play,
            _ => st.icon_document,
        };
        div()
            .flex_none()
            .size(px_of(ATTACH_PREVIEW_LAYOUT.thumb_size))
            .rounded_full()
            .bg(color(cx, st.icon_bg))
            .flex()
            .items_center()
            .justify_center()
            .child(ui::icon(icon))
            .into_any_element()
    }

    // shortcut: photos and videos show as file rows; tdesktop previews them large and as an album grid, with the media work in M5.
    fn file_row(file: &Prepared, cx: &App) -> AnyElement {
        let st = &DEFAULT_COMPOSE_FILES;
        let layout = if matches!(file.kind, AttachmentKind::Photo { .. }) {
            &ATTACH_PREVIEW_THUMB_LAYOUT
        } else {
            &ATTACH_PREVIEW_LAYOUT
        };
        let line = |text: String, top, text_font: Font, fg| {
            div()
                .absolute()
                .top(px_of(top))
                .left(px(0.0))
                .right(px(0.0))
                .font(font(text_font))
                .text_size(px_of(text_font.size))
                .text_color(color(cx, fg))
                .whitespace_nowrap()
                .overflow_hidden()
                .text_ellipsis()
                .child(text)
        };
        div()
            .h(px_of(layout.thumb_size))
            .flex()
            .gap(px_of(layout.thumb_skip))
            .child(Self::thumb(file, cx))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(line(
                        file.name.clone(),
                        layout.name_top,
                        SEMIBOLD_FONT,
                        st.name_fg,
                    ))
                    .child(line(
                        size_text(i64::try_from(file.size).unwrap_or(i64::MAX)),
                        layout.status_top,
                        NORMAL_FONT,
                        st.status_fg,
                    )),
            )
            .into_any_element()
    }

    fn checkboxes(&self, cx: &mut Context<Self>) -> AnyElement {
        let st = &DEFAULT_COMPOSE_FILES;
        let group = (self.files.len() > 1).then(|| {
            let checked = if self.grouping == Grouping::Albums {
                Checked::Yes
            } else {
                Checked::No
            };
            let weak = cx.weak_entity();
            Checkbox::new(
                "send-files-group",
                &st.checkbox,
                &st.check,
                tr(cx, key::lng_send_grouped),
                checked,
            )
            .on_toggle(move |_, cx| {
                let _ = weak.update(cx, |this, cx| {
                    this.grouping = match this.grouping {
                        Grouping::Albums => Grouping::Separate,
                        Grouping::Separate => Grouping::Albums,
                    };
                    cx.notify();
                });
            })
        });
        let as_files = self.has_media().then(|| {
            let checked = if self.compression == Compression::AsFiles {
                Checked::Yes
            } else {
                Checked::No
            };
            let text = if self.files.len() > 1 {
                key::lng_send_as_documents
            } else {
                key::lng_send_as_documents_one
            };
            let weak = cx.weak_entity();
            Checkbox::new(
                "send-files-as-files",
                &st.checkbox,
                &st.check,
                tr(cx, text),
                checked,
            )
            .on_toggle(move |_, cx| {
                let _ = weak.update(cx, |this, cx| {
                    this.compression = match this.compression {
                        Compression::Compressed => Compression::AsFiles,
                        Compression::AsFiles => Compression::Compressed,
                    };
                    cx.notify();
                });
            })
        });
        div()
            .flex()
            .flex_col()
            .gap(px_of(BOX_PHOTO_COMPRESSED_SKIP))
            .children(group)
            .children(as_files)
            .into_any_element()
    }
}

impl Render for SendFilesBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let side = px_of(BOX_PHOTO_PADDING.left);
        let rows = div()
            .id("send-files-rows")
            .max_h(px_of(BOX_MAX_LIST_HEIGHT))
            .overflow_y_scroll()
            .px(side)
            .flex()
            .flex_col()
            .gap(px_of(SEND_MEDIA_ROW_SKIP))
            .children(self.files.iter().map(|file| Self::file_row(file, cx)));
        let controls = div()
            .px(side)
            .pt(px_of(BOX_PHOTO_COMPRESSED_SKIP))
            .flex()
            .flex_col()
            .gap(px_of(BOX_PHOTO_COMPRESSED_SKIP))
            .child(self.checkboxes(cx))
            .child(
                div()
                    .mt(px_of(BOX_PHOTO_CAPTION_SKIP) - px_of(BOX_PHOTO_COMPRESSED_SKIP))
                    .w(px_of(SEND_MEDIA_PREVIEW_SIZE))
                    .child(self.caption.clone()),
            );
        let send = cx.entity();
        let cancel = cx.entity();
        BoxFrame::new("send-files-box", &DEFAULT_BOX)
            .width(px_of(BOX_WIDE_WIDTH))
            .title(title(&self.files, cx))
            .row(rows)
            .row(controls)
            .button(
                RoundButton::new(
                    "send-files-send",
                    &DEFAULT_BOX.button,
                    tr(cx, key::lng_send_button),
                )
                .on_click(move |_, window, cx| send.update(cx, |this, cx| this.send(window, cx))),
            )
            .button(
                RoundButton::new(
                    "send-files-cancel",
                    &DEFAULT_BOX.button,
                    tr(cx, key::lng_cancel),
                )
                .on_click(move |_, window, cx| {
                    cancel.update(cx, |this, cx| this.cancel(window, cx))
                }),
            )
    }
}

impl HistoryView {
    /// Opens the box for `files` with the field's text as the caption,
    /// which goes back to the field if the box closes unsent
    /// (`HistoryWidget::confirmSendingFiles`).
    pub(super) fn show_send_files(
        &mut self,
        files: Vec<Prepared>,
        compression: Compression,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let is_editing = matches!(self.compose.bar, Some(FieldBar::Edit { .. }));
        let field = self.compose.field.clone();
        let text = if is_editing {
            String::new()
        } else {
            let text = field.read(cx).text().to_owned();
            field.update(cx, |field, cx| field.set_text("", cx));
            text
        };
        let weak = cx.weak_entity();
        let on_send: OnSend = Rc::new(move |attachments, caption, grouping, _, cx| {
            // The view is gone once its chat closed; nothing to send to then.
            let _ = weak.update(cx, |this, cx| {
                this.send_files(attachments, caption, grouping, cx)
            });
        });
        let give_back: GiveBack = Rc::new(move |text, cx| {
            if !text.is_empty() && field.read(cx).text().is_empty() {
                field.update(cx, |field, cx| field.set_text(text, cx));
            }
        });
        let layers = self.layers.downgrade();
        let dismissed = give_back.clone();
        let view = cx.new(|cx| {
            let caption = cx.new(|cx| {
                let mut caption = TextArea::new(
                    &DEFAULT_COMPOSE_FILES.caption,
                    tr(cx, key::lng_photo_caption),
                    window,
                    cx,
                );
                caption.set_submit_key(crate::settings::send_by(cx).submit_key());
                caption.set_text(&text, cx);
                caption
            });
            let subscription = cx.subscribe_in(
                &caption,
                window,
                |this: &mut SendFilesBox, _, event: &TextAreaEvent, window, cx| {
                    if matches!(event, TextAreaEvent::Submitted) {
                        this.send(window, cx);
                    }
                },
            );
            SendFilesBox {
                layers,
                files,
                compression,
                grouping: Grouping::Albums,
                caption,
                on_send,
                give_back,
                _subscription: subscription,
            }
        });
        let caption = view.read(cx).caption.clone();
        let on_dismiss: OnDismiss = Box::new(move |_, cx| {
            let text = caption.read(cx).text().to_owned();
            dismissed(&text, cx);
        });
        self.layers.update(cx, |layers, cx| {
            layers.show(view, Dismiss::Allowed, Some(on_dismiss), window, cx)
        });
    }

    /// Sends the files, as a reply if one is open, and drops the reply.
    fn send_files(
        &mut self,
        attachments: Vec<Attachment>,
        caption: String,
        grouping: Grouping,
        cx: &mut Context<Self>,
    ) {
        let reply = self.take_reply();
        let request = self.store.read(cx).session().send_files(
            self.chat,
            self.topic,
            reply,
            &attachments,
            &caption,
            grouping,
        );
        let task = spawn(request, cx);
        cx.spawn(async move |_, _| {
            if let Err(error) = task.await {
                tracing::warn!(%error, "sending files failed");
            }
        })
        .detach();
        self.scroll_down(cx);
        cx.notify();
    }
}
