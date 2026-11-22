//! Files picked with the attach button, pasted or dropped, readied for the
//! send box (`Storage::PrepareMediaList`): each with its name, size and
//! how it goes when compressed.

use std::path::{Path, PathBuf};

use gpui::{ClipboardEntry, ClipboardItem, Context, ImageFormat, PathPromptOptions, Window};
use i18n::key;
use model::send_files::AttachmentKind;

use super::HistoryView;
use super::send_files_box::Compression;
use crate::lang::tr;

/// The server takes photos up to this size (`PreparedFile::Type::Photo`).
const PHOTO_SIZE_LIMIT: u64 = 10 * 1024 * 1024;
/// The server's limit on a photo's width plus height.
const PHOTO_SIDES_LIMIT: u32 = 10_000;
/// The longer side may be at most this many times the shorter
/// (`ValidateThumbDimensions`).
const PHOTO_RATIO_LIMIT: u32 = 20;
const PHOTO_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "bmp"];
// shortcut: by extension; tdesktop asks the video decoder, which comes in M5.
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mov", "m4v"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Prepared {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    /// How it goes when compressed; files go as files either way.
    pub kind: AttachmentKind,
}

fn extension(path: &Path) -> String {
    path.extension()
        .map(|extension| extension.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// Whether `path` names an image that may go as a photo, by its name only,
/// as the drop zones need while dragging.
pub(super) fn is_photo_path(path: &Path) -> bool {
    PHOTO_EXTENSIONS.contains(&extension(path).as_str())
}

fn is_valid_photo(width: u32, height: u32) -> bool {
    width > 0
        && height > 0
        && width.saturating_add(height) <= PHOTO_SIDES_LIMIT
        && width <= height.saturating_mul(PHOTO_RATIO_LIMIT)
        && height <= width.saturating_mul(PHOTO_RATIO_LIMIT)
}

/// A photo when the server takes it as one, a video by its extension,
/// else a file.
fn kind_of(path: &Path, size: u64) -> AttachmentKind {
    let extension = extension(path);
    if is_photo_path(path) && size <= PHOTO_SIZE_LIMIT {
        if let Ok((width, height)) = image::image_dimensions(path)
            && is_valid_photo(width, height)
        {
            return AttachmentKind::Photo { width, height };
        }
    } else if VIDEO_EXTENSIONS.contains(&extension.as_str()) {
        return AttachmentKind::Video;
    }
    AttachmentKind::File
}

/// The regular files among `paths`, read from disk; call it off the main
/// thread.
// shortcut: folders are left out; tdesktop offers to send them zipped.
fn prepare(paths: Vec<PathBuf>) -> Vec<Prepared> {
    paths
        .into_iter()
        .filter_map(|path| {
            let metadata = std::fs::metadata(&path)
                .inspect_err(|error| tracing::warn!(%error, "an attached file is unreadable"))
                .ok()?;
            if !metadata.is_file() {
                return None;
            }
            let name = path.file_name()?.to_string_lossy().into_owned();
            Some(Prepared {
                kind: kind_of(&path, metadata.len()),
                name,
                size: metadata.len(),
                path,
            })
        })
        .collect()
}

/// What a paste brings besides text: local files, else an image
/// (`HistoryWidget::canSendFiles`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Pasted {
    Files(Vec<PathBuf>),
    Image {
        bytes: Vec<u8>,
        extension: &'static str,
    },
}

pub(super) fn pasted(item: &ClipboardItem) -> Option<Pasted> {
    let entries = item.entries();
    let paths = entries.iter().find_map(|entry| match entry {
        ClipboardEntry::ExternalPaths(paths) if !paths.0.is_empty() => Some(paths.0.to_vec()),
        _ => None,
    });
    if let Some(paths) = paths {
        return Some(Pasted::Files(paths));
    }
    entries.iter().find_map(|entry| match entry {
        ClipboardEntry::Image(image) => Some(Pasted::Image {
            bytes: image.bytes.clone(),
            extension: image_extension(image.format),
        }),
        _ => None,
    })
}

fn image_extension(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Webp => "webp",
        ImageFormat::Gif => "gif",
        ImageFormat::Svg => "svg",
        ImageFormat::Bmp => "bmp",
        ImageFormat::Tiff => "tiff",
        _ => "img",
    }
}

/// Writes a pasted image where TDLib can upload it from.
// shortcut: the files stay in the temp folder for the system to clear; tdesktop uploads from memory.
fn save_pasted(bytes: &[u8], extension: &str) -> std::io::Result<PathBuf> {
    let folder = std::env::temp_dir().join("oxigram-pasted");
    std::fs::create_dir_all(&folder)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    let path = folder.join(format!("image-{stamp}.{extension}"));
    std::fs::write(&path, bytes)?;
    Ok(path)
}

impl HistoryView {
    /// The attach button: the system's file picker (`chooseAttach`).
    pub(super) fn choose_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(tr(cx, key::lng_choose_files)),
        });
        cx.spawn_in(window, async move |this, cx| {
            let paths = match paths.await {
                Ok(Ok(Some(paths))) => paths,
                Ok(Ok(None)) | Err(_) => return,
                Ok(Err(error)) => {
                    tracing::warn!(%error, "choosing files failed");
                    return;
                }
            };
            // The view is gone once its chat closed.
            let _ = this.update_in(cx, |this, window, cx| {
                this.confirm_files(paths, Compression::Compressed, window, cx);
            });
        })
        .detach();
    }

    /// Takes a paste of files or an image; `false` leaves it to the field.
    pub(super) fn paste_files(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(pasted) = cx.read_from_clipboard().as_ref().and_then(pasted) else {
            return false;
        };
        let paths = match pasted {
            Pasted::Files(paths) => paths,
            Pasted::Image { bytes, extension } => match save_pasted(&bytes, extension) {
                Ok(path) => vec![path],
                Err(error) => {
                    tracing::warn!(%error, "cannot keep the pasted image");
                    return true;
                }
            },
        };
        self.confirm_files(paths, Compression::Compressed, window, cx);
        true
    }

    /// Reads `paths` off the main thread, then asks how to send them.
    pub(super) fn confirm_files(
        &mut self,
        paths: Vec<PathBuf>,
        compression: Compression,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let task = cx
            .background_executor()
            .spawn(async move { prepare(paths) });
        cx.spawn_in(window, async move |this, cx| {
            let prepared = task.await;
            if prepared.is_empty() {
                return;
            }
            // The view is gone once its chat closed.
            let _ = this.update_in(cx, |this, window, cx| {
                this.show_send_files(prepared, compression, window, cx);
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn photos_keep_within_the_servers_sides_and_ratio() {
        assert!(is_valid_photo(1280, 960));
        assert!(!is_valid_photo(0, 10));
        assert!(!is_valid_photo(6000, 5000));
        assert!(!is_valid_photo(2100, 100));
    }

    #[test]
    fn a_paste_prefers_files_then_an_image() {
        let image = gpui::Image::from_bytes(ImageFormat::Png, vec![1, 2]);
        let item = ClipboardItem::new_image(&image);
        assert_eq!(
            pasted(&item),
            Some(Pasted::Image {
                bytes: vec![1, 2],
                extension: "png"
            })
        );
        assert_eq!(pasted(&ClipboardItem::new_string("hi".to_owned())), None);
    }

    #[test]
    fn files_are_read_with_their_kind() {
        let dir = std::env::temp_dir().join(format!("oxigram-attach-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the temp dir is writable");
        let photo = dir.join("a.PNG");
        image::RgbImage::new(4, 3)
            .save(&photo)
            .expect("the temp dir is writable");
        let text = dir.join("notes.txt");
        std::fs::write(&text, "hello").expect("the temp dir is writable");
        let video = dir.join("clip.mp4");
        std::fs::write(&video, "x").expect("the temp dir is writable");
        let prepared = prepare(vec![photo, text, video, dir.clone(), dir.join("missing")]);
        let kinds: Vec<_> = prepared
            .iter()
            .map(|file| (file.name.as_str(), file.kind))
            .collect();
        assert_eq!(
            kinds,
            [
                (
                    "a.PNG",
                    AttachmentKind::Photo {
                        width: 4,
                        height: 3
                    }
                ),
                ("notes.txt", AttachmentKind::File),
                ("clip.mp4", AttachmentKind::Video),
            ]
        );
        assert_eq!(prepared[1].size, 5);
        std::fs::remove_dir_all(&dir).ok();
    }
}
