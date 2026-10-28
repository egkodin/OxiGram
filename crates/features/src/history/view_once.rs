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

/// tdesktop's `gui/ttl/video_message_icon.svg`.
const FIRE: &str = r##"<svg viewBox="0 0 72 72" xmlns="http://www.w3.org/2000/svg"><path d="m27.57 30.79q0.77-0.44 1.14-1.28 4.38-9.86 4.67-24.25 0.03-1.64 1.63-1.54 1.14 0.07 1.9 0.65c14.45 10.9 28.35 31.97 18.06 50.37-9.55 17.08-32.38 15.75-41.59-0.69-5.25-9.37-0.83-23.06 4.26-32.03a2.13 2.12 43.5 0 1 3.64-0.09l5.53 8.68a0.57 0.56-31.3 0 0 0.76 0.18z" fill="#fff"/></svg>"##;

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
