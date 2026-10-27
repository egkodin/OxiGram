//! A file on its way up (`HistoryDocument` while uploading): the circle
//! with the progress arc and a cross that cancels, and how much went up.

use gpui::Context;
use i18n::key;
use model::message::{Content, Message, Sending};
use model::message_actions::Deletion;

use super::HistoryView;
use super::file_row::{FileRow, Thumb, progress};
use crate::file_sizes::progress_text;
use crate::lang::{strings, tr};
use crate::store::spawn;

impl HistoryView {
    /// The upload of a message being sent with a file, if it has one.
    pub(super) fn upload(&self, message: &Message, cx: &Context<Self>) -> Option<FileRow> {
        if message.sending != Some(Sending::Pending) {
            return None;
        }
        let (media, name) = match &message.content {
            Content::Photo(media) => (media, tr(cx, key::lng_in_dlg_photo).to_string()),
            Content::Video(media) | Content::Animation(media) => {
                (media, tr(cx, key::lng_in_dlg_video).to_string())
            }
            Content::Document { name, media } => (media, name.clone()),
            _ => return None,
        };
        let sent = media.file.as_ref()?;
        let file = self
            .store
            .read(cx)
            .state()
            .chats
            .file(sent.id)
            .unwrap_or(sent);
        let weak = cx.weak_entity();
        let (chat, id) = (self.chat, message.id);
        Some(FileRow {
            name,
            status: progress_text(strings(cx), file.uploaded, file.size),
            thumb: Thumb::Cancel {
                progress: progress(file.uploaded, file.size),
            },
            on_click: std::rc::Rc::new(move |_, _, cx| {
                // Deleting a message being sent stops its upload (`cancelUpload`).
                let _ = weak.update(cx, |this, cx| {
                    let request =
                        this.store
                            .read(cx)
                            .session()
                            .delete_messages(chat, &[id], Deletion::ForMe);
                    let task = spawn(request, cx);
                    cx.spawn(async move |_, _| {
                        if let Err(error) = task.await {
                            tracing::warn!(%error, "cancelling an upload failed");
                        }
                    })
                    .detach();
                });
            }),
        })
    }
}
