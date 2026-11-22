//! The viewer’s arrows, footer, save button and caption.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Side {
    Left,
    Right,
}

/// The arrow strip along a side (`_leftNav`, `_rightNav`), its circle
/// showing under the mouse.
pub(super) fn nav(side: Side, cx: &mut Context<MediaView>) -> impl IntoElement {
    let over = px_of(MEDIAVIEW_ICON_OVER);
    let (id, arrow, forward) = match side {
        Side::Left => ("media-view-left", MEDIAVIEW_LEFT, false),
        Side::Right => ("media-view-right", MEDIAVIEW_RIGHT, true),
    };
    let control_bg = color(cx, ColorRef::Palette(ColorId::MediaviewControlBg));
    div()
        .id(id)
        .group(id)
        .absolute()
        .top_0()
        .bottom_0()
        .when(side == Side::Left, |this| this.left_0())
        .when(side == Side::Right, |this| this.right_0())
        .w(px_of(MEDIAVIEW_CONTROL_SIZE))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |this, _, _, cx| this.page(forward, cx)))
        .child(
            div()
                .size(over)
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .group_hover(id, move |this| this.bg(control_bg))
                .child(icon(arrow)),
        )
}

fn line(text: impl Into<SharedString>, line_font: Font, fg: gpui::Hsla) -> gpui::Div {
    div()
        .font(font(line_font))
        .text_size(px_of(line_font.size))
        .line_height(font_height(line_font))
        .text_color(fg)
        .whitespace_nowrap()
        .child(text.into())
}

/// "Photo 2 of 5" over "Maria Garcia • today at 10:00", bottom left.
// shortcut: the name does not open the sender's profile; with profiles in M6.
pub(super) fn footer(
    entry: &Entry,
    index: usize,
    count: usize,
    fg: gpui::Hsla,
    cx: &App,
) -> impl IntoElement {
    let header = (count > 1).then(|| {
        let n = (index + 1).to_string();
        let amount = count.to_string();
        let text = tr_with(
            cx,
            key::lng_mediaview_n_of_amount,
            &[("n", &n), ("amount", &amount)],
        );
        line(text, SEMIBOLD_FONT, fg)
            .absolute()
            .left(px_of(MEDIAVIEW_TEXT_LEFT))
            .bottom(px_of(MEDIAVIEW_HEADER_TOP) - font_height(SEMIBOLD_FONT))
    });
    let name = (!entry.sender.is_empty()).then(|| line(entry.sender.clone(), SEMIBOLD_FONT, fg));
    let separator = name.is_some().then(|| line("\u{2022}", NORMAL_FONT, fg));
    let texts = div()
        .absolute()
        .left(px_of(MEDIAVIEW_TEXT_LEFT))
        .bottom(px_of(MEDIAVIEW_TEXT_TOP) - font_height(NORMAL_FONT))
        .flex()
        .gap(px_of(MEDIAVIEW_TEXT_SKIP_HALF))
        .children(name)
        .children(separator)
        .child(line(entry.date.clone(), NORMAL_FONT, fg));
    div().absolute().inset_0().children(header).child(texts)
}

/// The save button, bottom right (`_saveNav`).
// shortcut: no rotate or more menu yet; with the viewer's actions (row 295).
pub(super) fn save_button(message: MessageId, cx: &mut Context<MediaView>) -> impl IntoElement {
    let control_bg = color(cx, ColorRef::Palette(ColorId::MediaviewControlBg));
    div()
        .id("media-view-save")
        .group("media-view-save")
        .absolute()
        .right_0()
        .bottom_0()
        .w(px_of(MEDIAVIEW_ICON_SIZE.width))
        .h(px_of(MEDIAVIEW_ICON_SIZE.height))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |_, _, _, cx| cx.emit(MediaViewEvent::Save(message))))
        .child(
            div()
                .size(px_of(MEDIAVIEW_ICON_OVER))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .group_hover("media-view-save", move |this| this.bg(control_bg))
                .child(icon(MEDIAVIEW_SAVE)),
        )
}

/// The caption's width: 60% of the window, less its padding.
fn caption_width(window: &Window) -> Pixels {
    let padding = MEDIAVIEW_CAPTION_PADDING;
    window.viewport_size().width * CAPTION_SHARE - px_of(padding.left) - px_of(padding.right)
}

/// The caption's wrapped lines, at most a quarter of the window's height
/// in whole lines (`refreshCaptionGeometry`).
pub(super) fn caption_text_height(text: &str, window: &Window) -> Pixels {
    let line = font_height(NORMAL_FONT);
    let run = gpui::TextRun {
        len: text.len(),
        font: font(NORMAL_FONT),
        color: gpui::Hsla::default(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let rows: usize = window
        .text_system()
        .shape_text(
            SharedString::from(text.to_owned()),
            px_of(NORMAL_FONT.size),
            &[run],
            Some(caption_width(window)),
            None,
        )
        .map(|lines| {
            lines
                .iter()
                .map(|line| line.wrap_boundaries().len() + 1)
                .sum()
        })
        .unwrap_or(1);
    let max_rows = (window.viewport_size().height / 4.0 / line)
        .floor()
        .max(1.0);
    #[expect(clippy::cast_precision_loss, reason = "a few lines")]
    let rows = (rows as f32).min(max_rows);
    line * rows
}

// shortcut: the viewer shows plain text; add entities when it shares the chat’s rich text renderer.
pub(super) fn caption(entry: &Entry, lift: Pixels, height: Pixels, cx: &App) -> impl IntoElement {
    let padding = MEDIAVIEW_CAPTION_PADDING;
    div()
        .absolute()
        .left_0()
        .right_0()
        .bottom(px_of(MEDIAVIEW_CAPTION_MARGIN.height) + px_of(MEDIAVIEW_TEXT_TOP) + lift)
        .flex()
        .justify_center()
        .child(
            div()
                .id(("media-view-caption", entry.message.0.cast_unsigned()))
                .max_w(gpui::relative(CAPTION_SHARE))
                .max_h(height + px_of(padding.top) + px_of(padding.bottom))
                .overflow_x_hidden()
                .overflow_y_scroll()
                .px(px_of(padding.left))
                .pt(px_of(padding.top))
                .pb(px_of(padding.bottom))
                .rounded(px_of(MEDIAVIEW_CAPTION_RADIUS))
                .bg(color(cx, ColorRef::Palette(ColorId::MediaviewCaptionBg)))
                .font(font(NORMAL_FONT))
                .text_size(px_of(NORMAL_FONT.size))
                .line_height(font_height(NORMAL_FONT))
                .text_color(color(cx, ColorRef::Palette(ColorId::MediaviewCaptionFg)))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(entry.caption.clone()),
        )
}
