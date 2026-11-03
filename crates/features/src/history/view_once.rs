//! View-once and timed photos and videos (`isTtlCoveredMedia`): the
//! preview under dots with a fire in the middle until a click opens it in
//! the viewer, which starts its end.

use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use gpui::{
    AnyElement, App, Context, Image, ImageFormat, IntoElement as _, ParentElement as _,
    Styled as _, div, img,
};
use model::message::{Content, Message, SelfDestruct};
use style::chat::MSG_FILE_LAYOUT;
use style::chat_style::HISTORY_VIDEO_MESSAGE_TTL_ICON;
use style::{ColorId, ColorRef};
use ui::{color, icon, icon_size};

use super::HistoryView;
use super::bubble::OnClick;
use super::photo::Photo;
use crate::px_of;

/// A flame in the place of tdesktop's `gui/ttl/video_message_icon.svg`,
/// drawn for rgram: ADR 0001 keeps tdesktop's `Resources/art/` out.
const FIRE: &str = r##"<svg viewBox="0 0 72 72" xmlns="http://www.w3.org/2000/svg"><path d="M36 5C44 19 58 29 58 46C58 58 48 67 36 67C24 67 14 58 14 46C14 36 19 29 24 25C25 33 28 37 32 39C30 29 32 17 36 5Z" fill="#fff"/></svg>"##;

fn fire_image() -> Arc<Image> {
    static FIRE_IMAGE: OnceLock<Arc<Image>> = OnceLock::new();
    FIRE_IMAGE
        .get_or_init(|| {
            Arc::new(Image::from_bytes(
                ImageFormat::Svg,
                FIRE.as_bytes().to_vec(),
            ))
        })
        .clone()
}

impl HistoryView {
    /// `photo` covered while its message destructs once opened; other
    /// media as it is.
    pub(super) fn once(
        &self,
        message: &Message,
        kind: SelfDestruct,
        photo: Photo,
        cx: &Context<Self>,
    ) -> Photo {
        let open: OnClick = match message.content {
            Content::Photo(_) => {
                let weak = cx.weak_entity();
                let id = message.id;
                Rc::new(move |_, window, cx| {
                    let _ = weak.update(cx, |this, cx| this.open_once(id, window, cx));
                })
            }
            // shortcut: a covered video does nothing on click, as the system player would keep a copy; it plays in the viewer with the video player (row 292).
            Content::Video(_) => Rc::new(|_, _, _| {}),
            _ => return photo,
        };
        Photo {
            source: photo.thumbnail.clone(),
            video: None,
            frame: None,
            spoiler: Some(open),
            open: None,
            once: Some(kind),
            ..photo
        }
    }
}

/// The fire in its circle, with the "1" badge on view-once media
/// (`paintTtlFire`, `PaintTtlSingleViewBadge`).
// shortcut: no countdown ring or seconds label on timed media, which open once anyway; with M8 polish.
pub(super) fn fire(kind: SelfDestruct, is_selected: bool, cx: &App) -> AnyElement {
    let bg = color(
        cx,
        ColorRef::Palette(if is_selected {
            ColorId::MsgDateImgBgSelected
        } else {
            ColorId::MsgDateImgBg
        }),
    );
    let inner = px_of(MSG_FILE_LAYOUT.thumb_size);
    let badge = (kind == SelfDestruct::Immediately).then(|| {
        let side = icon_size(HISTORY_VIDEO_MESSAGE_TTL_ICON);
        div()
            .absolute()
            .left(inner - side.width * 3.0 / 4.0)
            .top(inner - side.height * 3.0 / 4.0)
            .w(side.width)
            .h(side.height)
            .rounded_full()
            .bg(bg)
            .flex()
            .items_center()
            .justify_center()
            .child(icon(HISTORY_VIDEO_MESSAGE_TTL_ICON))
    });
    let circle = div()
        .relative()
        .size(inner)
        .rounded_full()
        .bg(bg)
        .flex()
        .items_center()
        .justify_center()
        .child(img(fire_image()).size(inner / 2.0))
        .children(badge);
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .child(circle)
        .into_any_element()
}
