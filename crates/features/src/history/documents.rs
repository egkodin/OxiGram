//! Received and sent files in the history (`HistoryDocument`): download on
//! a click, cancel while it loads, open once it is on disk, and "Show in
//! Finder" from the menu.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui::Context;
use model::FileId;
use model::file::File;
use model::message::{Content, Message};
use model::user::Badge;

use super::HistoryView;
use super::file_row::{FileRow, Thumb, progress};
use crate::file_sizes::{progress_text, size_text};
use crate::lang::strings;
use crate::store::spawn;

/// A file the user asked for loads before anything shown on its own.
pub(super) const CLICKED_PRIORITY: i32 = 32;

/// The file's state now, from `updateFile` when TDLib sent one.
pub(super) fn current<'a>(
    view: &'a HistoryView,
    sent: &'a File,
    cx: &'a Context<HistoryView>,
) -> &'a File {
    view.store
        .read(cx)
        .state()
        .chats
        .file(sent.id)
        .unwrap_or(sent)
}

impl HistoryView {
    /// A document message's row, unless it is still going up.
    pub(super) fn document(&self, message: &Message, cx: &Context<Self>) -> Option<FileRow> {
        let Content::Document { name, media } = &message.content else {
            return None;
        };
        let file = current(self, media.file.as_ref()?, cx);
        let (thumb, status) = match (&file.path, file.is_downloading) {
            (Some(_), _) => (Thumb::Document, size_text(file.size)),
            (None, true) => (
                Thumb::Cancel {
                    progress: progress(file.downloaded, file.size),
                },
                progress_text(strings(cx), file.downloaded, file.size),
            ),
            (None, false) => (Thumb::Download, size_text(file.size)),
        };
        let weak = cx.weak_entity();
        let id = file.id;
        let path = file.path.clone().map(PathBuf::from);
        Some(FileRow {
            name: name.clone(),
            status,
            thumb,
            on_click: Rc::new(move |_, window, cx| {
                let _ = weak.update(cx, |this, cx| match thumb {
                    Thumb::Download => this.start_download(id, CLICKED_PRIORITY, cx),
                    Thumb::Cancel { .. } => this.cancel_download(id, cx),
                    Thumb::Document => {
                        if let Some(path) = &path {
                            this.open_file(path, window, cx);
                        }
                    }
                });
            }),
        })
    }

    /// Downloads a userpic once per history view.
    pub(super) fn request_file(&mut self, file: FileId, cx: &mut Context<Self>) {
        self.fetch(file, super::USERPIC_PRIORITY, cx);
    }

    /// Downloads a file once per history view.
    pub(super) fn fetch(&mut self, file: FileId, priority: i32, cx: &mut Context<Self>) {
        if self.requested_files.insert(file) {
            self.start_download(file, priority, cx);
        }
    }

    pub(super) fn start_download(&mut self, file: FileId, priority: i32, cx: &mut Context<Self>) {
        let task = spawn(
            self.store.read(cx).session().download_file(file, priority),
            cx,
        );
        cx.spawn(async move |_, _| {
            if let Err(error) = task.await {
                tracing::warn!(%error, "downloading a file failed");
            }
        })
        .detach();
    }

    pub(super) fn cancel_download(&mut self, file: FileId, cx: &mut Context<Self>) {
        let task = spawn(self.store.read(cx).session().cancel_download_file(file), cx);
        cx.spawn(async move |_, _| {
            if let Err(error) = task.await {
                tracing::warn!(%error, "cancelling a download failed");
            }
        })
        .detach();
    }

    /// Where a document message's file is, once it is on disk.
    pub(super) fn document_path(&self, message: &Message, cx: &Context<Self>) -> Option<PathBuf> {
        let Content::Document { media, .. } = &message.content else {
            return None;
        };
        let file = current(self, media.file.as_ref()?, cx);
        file.path.clone().map(PathBuf::from)
    }

    pub(super) fn open_file(
        &mut self,
        path: &Path,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let state = self.store.read(cx).state();
        // Only a verified user's files skip the warning; channel badges come with profiles (M6).
        let is_verified = state
            .chats
            .chat(self.chat)
            .and_then(|chat| chat.user())
            .and_then(|user| state.chats.user(user))
            .is_some_and(|user| user.badge == Badge::Verified);
        crate::launch::open(path, is_verified, &self.layers, window, cx);
    }
}
