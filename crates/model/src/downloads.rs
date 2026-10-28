//! TDLib's download list (`DownloadManager`): the files the user chose to
//! download, and the progress of those still loading.

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::Update;
use td_api::functions::{AddFileToDownloads, SearchFileDownloads};

use crate::ids::{ChatId, FileId, MessageId};
use crate::message::{Content, Message};
use crate::session::{RequestError, Session};

/// The files loading or just done (`updateFileDownloads`); all zero once
/// TDLib forgets a finished batch, a minute after it completes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Totals {
    pub total_size: i64,
    pub total_count: i32,
    pub downloaded_size: i64,
}

impl Totals {
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.total_count > 0 && self.downloaded_size >= self.total_size
    }
}

/// A file added to the list, and the message it came in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    pub file: FileId,
    pub chat: ChatId,
    pub message: MessageId,
    /// The file's name; empty for media without one.
    pub name: String,
}

impl From<td_api::types::FileDownload> for Download {
    fn from(download: td_api::types::FileDownload) -> Self {
        let message = Message::from(*download.message);
        let name = match message.content {
            Content::Document { name, .. } | Content::Audio { name, .. } => name,
            _ => String::new(),
        };
        Self {
            file: FileId(download.file_id),
            chat: message.chat,
            message: message.id,
            name,
        }
    }
}

/// A row of the downloads list: the message with the file, and when the
/// user added it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub message: Message,
    pub file: FileId,
    /// Unix time the file joined the list.
    pub added: i32,
}

impl From<td_api::types::FileDownload> for Entry {
    fn from(download: td_api::types::FileDownload) -> Self {
        Self {
            file: FileId(download.file_id),
            added: download.add_date,
            message: Message::from(*download.message),
        }
    }
}

/// The most files one search returns (`searchFileDownloads`' limit).
const LIST_LIMIT: i32 = 100;

/// What the download bar follows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Downloads {
    pub totals: Totals,
    /// The last file added, which a bar for one file names.
    pub latest: Option<Download>,
    /// The downloads section's rows as last fetched; `None` before that.
    pub list: Option<Vec<Entry>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Totals(Totals),
    Added(Download),
    /// The section's rows, from `Session::file_downloads`.
    Listed(Vec<Entry>),
}

impl Change {
    pub(crate) fn from_update(update: Update) -> Result<Self, Update> {
        Ok(match update {
            Update::FileDownloads(update) => Self::Totals(Totals {
                total_size: update.total_size,
                total_count: update.total_count,
                downloaded_size: update.downloaded_size,
            }),
            Update::FileAddedToDownloads(update) => {
                Self::Added(Download::from(*update.file_download))
            }
            other => return Err(other),
        })
    }
}

impl Downloads {
    pub fn apply(&mut self, change: Change) {
        match change {
            Change::Totals(totals) => self.totals = totals,
            Change::Added(download) => self.latest = Some(download),
            Change::Listed(entries) => self.list = Some(entries),
        }
    }
}

impl Session {
    /// Downloads a message's file into TDLib's download list, where the bar
    /// and the downloads section follow it (`addFileToDownloads`).
    pub fn add_file_to_downloads(
        &self,
        file: FileId,
        chat: ChatId,
        message: MessageId,
        priority: i32,
    ) -> impl Future<Output = Result<(), RequestError>> + Send + use<> {
        self.client()
            .request(&AddFileToDownloads {
                file_id: file.0,
                chat_id: chat.0,
                message_id: message.0,
                priority,
            })
            .map(|result| result.map(drop).map_err(RequestError::from))
    }

    /// The downloads list, newest first (`searchFileDownloads`).
    // shortcut: the newest 100 only, no paging or search; page with `next_offset` once someone keeps more.
    pub fn file_downloads(
        &self,
    ) -> impl Future<Output = Result<Vec<Entry>, RequestError>> + Send + use<> {
        self.client()
            .request(&SearchFileDownloads {
                query: String::new(),
                only_active: false,
                only_completed: false,
                offset: String::new(),
                limit: LIST_LIMIT,
            })
            .map(|result| {
                result
                    .map(|found| found.files.into_iter().map(Entry::from).collect())
                    .map_err(RequestError::from)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(json: &str) -> Option<Change> {
        Change::from_update(serde_json::from_str(json).expect("fixture is a valid update")).ok()
    }

    #[test]
    fn the_totals_follow_tdlib_and_finish_when_all_bytes_are_in() {
        let mut downloads = Downloads::default();
        let totals = change(
            r#"{"@type":"updateFileDownloads","total_size":300,"total_count":2,"downloaded_size":100}"#,
        )
        .expect("a download update");
        downloads.apply(totals);
        assert_eq!(downloads.totals.total_count, 2);
        assert!(!downloads.totals.is_done());
        downloads.apply(Change::Totals(Totals {
            downloaded_size: 300,
            ..downloads.totals
        }));
        assert!(downloads.totals.is_done());
        assert!(!Totals::default().is_done());
    }
}
