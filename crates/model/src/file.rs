//! Files TDLib downloads on request, and the tiny inline previews it sends
//! with photos.

use base64::Engine as _;

use crate::ids::FileId;

/// What the UI needs about a file: where it is once downloaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    pub id: FileId,
    /// The local path once the download completed.
    pub path: Option<String>,
    pub is_downloading: bool,
    /// In bytes, exact or TDLib's estimate; 0 if unknown.
    pub size: i64,
    /// The bytes on the server while it uploads.
    pub uploaded: i64,
    pub is_uploading: bool,
}

/// A blurred JPEG preview of at most 40x40 pixels, shown until the real
/// image loads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Minithumbnail {
    pub width: u32,
    pub height: u32,
    pub jpeg: Vec<u8>,
}

impl From<td_api::types::File> for File {
    fn from(file: td_api::types::File) -> Self {
        let local = file.local;
        Self {
            id: FileId(file.id),
            path: (local.is_downloading_completed && !local.path.is_empty()).then_some(local.path),
            is_downloading: local.is_downloading_active,
            size: if file.size > 0 {
                file.size
            } else {
                file.expected_size
            },
            uploaded: file.remote.uploaded_size,
            is_uploading: file.remote.is_uploading_active,
        }
    }
}

impl Minithumbnail {
    /// `None` when TDLib's base64 data does not decode.
    pub(crate) fn from_td(thumbnail: td_api::types::Minithumbnail) -> Option<Self> {
        let jpeg = base64::engine::general_purpose::STANDARD
            .decode(thumbnail.data)
            .ok()?;
        Some(Self {
            width: u32::try_from(thumbnail.width).ok()?,
            height: u32::try_from(thumbnail.height).ok()?,
            jpeg,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_path_only_after_the_download_completed() {
        let file: td_api::types::File = serde_json::from_str(
            r#"{"@type":"file","id":7,"size":10,"expected_size":10,
            "local":{"@type":"localFile","path":"/tmp/a.jpg","can_be_downloaded":true,"can_be_deleted":true,
            "is_downloading_active":false,"is_downloading_completed":true,"download_offset":0,
            "downloaded_prefix_size":10,"downloaded_size":10},
            "remote":{"@type":"remoteFile","id":"x","unique_id":"y","is_uploading_active":false,
            "is_uploading_completed":true,"uploaded_size":10}}"#,
        )
        .expect("fixture is a valid file");
        let file = File::from(file);
        assert_eq!(file.id, FileId(7));
        assert_eq!(file.path.as_deref(), Some("/tmp/a.jpg"));
        assert_eq!(
            (file.size, file.uploaded, file.is_uploading),
            (10, 10, false)
        );
    }

    #[test]
    fn decodes_minithumbnail_bytes() {
        let thumbnail = td_api::types::Minithumbnail {
            width: 2,
            height: 1,
            data: "/9j/".to_owned(),
        };
        let thumbnail = Minithumbnail::from_td(thumbnail).expect("valid base64");
        assert_eq!(thumbnail.jpeg, vec![0xff, 0xd8, 0xff]);
    }
}
