//! TDLib's download list (`DownloadManager`): the files the user chose to
//! download, and the progress of those still loading.

use std::future::Future;

use futures::FutureExt as _;
use td_api::enums::Update;
use td_api::functions::AddFileToDownloads;

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

/// What the download bar follows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Downloads {
    pub totals: Totals,
    /// The last file added, which a bar for one file names.
    pub latest: Option<Download>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Totals(Totals),
    Added(Download),
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
