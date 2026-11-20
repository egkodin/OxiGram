//! The menu's file actions on a downloaded file: "Show in Finder" and
//! "Save As..." (`AddSaveDocumentAction`, `SavePhotoToFile`): a save box
//! suggests a name in Downloads, then the file is copied there.

use std::io;
use std::path::{Path, PathBuf};

use gpui::Context;
use i18n::{Key, key};
use model::message::{Content, Message};

use super::HistoryView;
use super::documents::current;
use super::menu::Item;

/// What "Save As..." copies, under which name and label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Saving {
    pub path: PathBuf,
    pub name: String,
    pub label: Key,
}

/// The label tdesktop gives the kind of file.
fn label(content: &Content) -> Option<Key> {
    Some(match content {
        Content::Photo(_) => key::lng_context_save_image,
        Content::Video(_) => key::lng_context_save_video,
        Content::Voice { .. } => key::lng_context_save_audio,
        Content::Animation(_) | Content::Document { .. } => key::lng_context_save_file,
        _ => return None,
    })
}

impl HistoryView {
    /// "Show in Finder" for a document on disk and "Save As..." for any
    /// file on disk the chat lets the user save.
    // shortcut: only files already on disk; tdesktop downloads straight to the chosen path. With the download manager (row 286).
    pub(super) fn file_items(&self, message: &Message, cx: &Context<Self>) -> Vec<Item> {
        let file = match &message.content {
            Content::Photo(media)
            | Content::Video(media)
            | Content::Animation(media)
            | Content::Document { media, .. } => media.file.as_ref(),
            Content::Voice { note, .. } => Some(&note.file),
            _ => None,
        };
        let (Some(file), Some(label)) = (file, label(&message.content)) else {
            return Vec::new();
        };
        let Some(path) = current(self, file, cx).path.clone().map(PathBuf::from) else {
            return Vec::new();
        };
        let is_document = !matches!(message.content, Content::Photo(_));
        let name = match &message.content {
            Content::Document { name, .. } if !name.is_empty() => name.clone(),
            // `filedialogDefaultName`.
            Content::Photo(_) => chrono::Local::now()
                .format("photo_%Y-%m-%d_%H-%M-%S.jpg")
                .to_string(),
            _ => path
                .file_name()
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned()),
        };
        let show = is_document.then(|| Item::ShowInFolder(path.clone()));
        let save = message
            .can_be_saved
            .then_some(Item::SaveAs(Saving { path, name, label }));
        show.into_iter().chain(save).collect()
    }

    /// Asks where to save, in Downloads first, then copies the file there.
    pub(super) fn save_as(&self, saving: &Saving, cx: &mut Context<Self>) {
        let source = saving.path.clone();
        let downloads = std::env::home_dir()
            .map(|home| home.join("Downloads"))
            .filter(|path| path.is_dir())
            .unwrap_or_else(std::env::temp_dir);
        let chosen = cx.prompt_for_new_path(&downloads, Some(&saving.name));
        cx.spawn(async move |_, cx| {
            let target = match chosen.await {
                Ok(Ok(Some(target))) => target,
                // Cancelled.
                Ok(Ok(None)) | Err(_) => return,
                Ok(Err(error)) => {
                    tracing::warn!(%error, "the save box failed");
                    return;
                }
            };
            let copied = cx
                .background_executor()
                .spawn(async move { save_copy(&source, &target) })
                .await;
            if let Err(error) = copied {
                tracing::warn!(%error, "saving a file failed");
            }
        })
        .detach();
    }
}

fn save_copy(source: &Path, target: &Path) -> io::Result<()> {
    let destination = if target.is_symlink() {
        target.canonicalize()?
    } else {
        target.to_owned()
    };
    let dir = destination.parent().unwrap_or_else(|| Path::new("."));
    let mut staging = tempfile::Builder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        staging.permissions(std::fs::Permissions::from_mode(0o700));
    }
    let staging = staging.tempdir_in(dir)?;
    let copy = staging.path().join("download");
    // fs::copy also preserves platform metadata such as macOS quarantine.
    std::fs::copy(source, &copy)?;
    std::fs::rename(copy, destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::message::Media;

    #[test]
    fn saving_over_the_source_preserves_its_contents() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("download");
        std::fs::write(&source, b"document").unwrap();
        save_copy(&source, &source).unwrap();
        assert_eq!(std::fs::read(&source).unwrap(), b"document");
        assert!(save_copy(&dir.path().join("missing"), &source).is_err());
        assert_eq!(std::fs::read(&source).unwrap(), b"document");
    }

    #[test]
    fn each_kind_gets_its_label() {
        let media = Media::default();
        assert_eq!(
            label(&Content::Photo(media.clone())),
            Some(key::lng_context_save_image)
        );
        assert_eq!(
            label(&Content::Video(media)),
            Some(key::lng_context_save_video)
        );
        assert_eq!(label(&Content::Unsupported), None);
    }
}
