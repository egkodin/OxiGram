//! Lays out and paints the field: its height from the wrapped text, then
//! the background, placeholder, selection, text and cursor, scrolled.

use std::rc::Rc;

use gpui::{
    App, AvailableSpace, Bounds, ContentMask, Element, ElementId, ElementInputHandler, Entity,
    GlobalElementId, LayoutId, PaintQuad, Pixels, Point, ShapedLine, Style, TextAlign, Window,
    fill, point, prelude::*, px, relative, size,
};

use super::layout::{Layout, Metrics};
use super::{Painted, TextArea};
use crate::anim::{interpolate, mix};
use crate::font::{font, font_height};
use crate::input::{plain_run, px_of, text_runs};
use crate::theme::{Theme, hsla};

pub(super) struct AreaElement {
    pub(super) area: Entity<TextArea>,
}

impl IntoElement for AreaElement {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

pub(super) struct Prepaint {
    layout: Rc<Layout>,
    origin: Point<Pixels>,
    quads: Vec<PaintQuad>,
    placeholder: Option<(ShapedLine, Point<Pixels>, Pixels)>,
    cursor: Option<PaintQuad>,
}

impl Element for AreaElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let area = self.area.read(cx);
        let metrics = area.metrics;
        let text = area.buffer.text.clone();
        let run = plain_run(text.len(), &font(area.style.style.font), gpui::black());
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        let layout = window.request_measured_layout(style, move |known, available, window, _| {
            let width = known.width.unwrap_or(match available.width {
                AvailableSpace::Definite(width) => width,
                AvailableSpace::MinContent | AvailableSpace::MaxContent => px(0.0),
            });
            let layout = Layout::shape(&text, std::slice::from_ref(&run), metrics, width, window);
            size(width, metrics.height(layout.rows()))
        });
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Prepaint {
        let area = self.area.read(cx);
        let st = area.style;
        let palette = &cx.global::<Theme>().palette;
        let fg = hsla(st.text_fg.resolve(palette));
        let runs = text_runs(
            &area.buffer.text,
            &font(st.style.font),
            fg,
            hsla(palette.history_text_in_fg_selected),
            &area.buffer.selected,
            area.buffer.marked.as_ref(),
        );
        let layout = Rc::new(Layout::shape(
            &area.buffer.text,
            &runs,
            area.metrics,
            bounds.size.width,
            window,
        ));
        let cursor_at = layout.position(area.buffer.cursor());
        let scroll_top = scroll_top(area, &layout, bounds, cursor_at.y);
        let origin = point(
            bounds.left() + area.metrics.margin,
            bounds.top() + area.metrics.margin - scroll_top,
        );
        let mut quads = vec![fill(bounds, hsla(st.text_bg.resolve(palette)))];
        quads.extend(selection_quads(area, &layout, origin, palette));
        let cursor = (area.focus.is_focused(window)
            && area.cursor_visible
            && area.buffer.selected.is_empty())
        .then(|| {
            fill(
                Bounds::new(origin + cursor_at, size(px(1.0), layout.line_height)),
                fg,
            )
        });
        let placeholder = placeholder_line(area, bounds, palette, window);
        self.area.update(cx, |area, _| {
            area.scroll_top = scroll_top;
            area.reveal_cursor = false;
        });
        Prepaint {
            layout,
            origin,
            quads,
            placeholder,
            cursor,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        prepaint: &mut Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.area.read(cx).focus.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.area.clone()),
            cx,
        );
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for quad in prepaint.quads.drain(..) {
                window.paint_quad(quad);
            }
            if let Some((placeholder, origin, line_height)) = prepaint.placeholder.take()
                && let Err(error) =
                    placeholder.paint(origin, line_height, TextAlign::Left, None, window, cx)
            {
                tracing::warn!(%error, "cannot paint a placeholder");
            }
            let line_height = prepaint.layout.line_height;
            for (line, top) in prepaint.layout.lines() {
                let origin = point(prepaint.origin.x, prepaint.origin.y + top);
                if let Err(error) =
                    line.paint(origin, line_height, TextAlign::Left, None, window, cx)
                {
                    tracing::warn!(%error, "cannot paint a field's text");
                }
            }
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        });
        let layout = prepaint.layout.clone();
        self.area.update(cx, |area, _| {
            area.painted = Some(Painted { layout, bounds });
        });
    }
}

/// Keeps the scroll in range, and the cursor's row in view after an edit
/// or a move.
fn scroll_top(
    area: &TextArea,
    layout: &Layout,
    bounds: Bounds<Pixels>,
    cursor_y: Pixels,
) -> Pixels {
    let metrics: Metrics = area.metrics;
    let mut top = area.scroll_top;
    if area.reveal_cursor {
        let visible = bounds.size.height - metrics.margin * 2.0;
        if cursor_y < top {
            top = cursor_y;
        } else if cursor_y + layout.line_height > top + visible {
            top = cursor_y + layout.line_height - visible;
        }
    }
    top.clamp(
        px(0.0),
        metrics.max_scroll(layout.rows(), bounds.size.height),
    )
}

/// A box under the selected part of each row.
fn selection_quads(
    area: &TextArea,
    layout: &Layout,
    origin: Point<Pixels>,
    palette: &style::Palette,
) -> Vec<PaintQuad> {
    let selected = &area.buffer.selected;
    if selected.is_empty() {
        return Vec::new();
    }
    let color = hsla(palette.msg_in_bg_selected);
    (layout.row_of(selected.start)..=layout.row_of(selected.end))
        .filter_map(|row| {
            let range = layout.row_range(row);
            let (start, end) = (selected.start.max(range.start), selected.end.min(range.end));
            if start >= end {
                return None;
            }
            // A wrapped row's first offset reads as the end of the row above.
            let left = if start == range.start {
                px(0.0)
            } else {
                layout.position(start).x
            };
            #[expect(clippy::cast_precision_loss, reason = "row counts are small")]
            let top = layout.line_height * row as f32;
            Some(fill(
                Bounds::from_corners(
                    origin + point(left, top),
                    origin + point(layout.position(end).x, top + layout.line_height),
                ),
                color,
            ))
        })
        .collect()
}

/// The placeholder, sliding right and fading as text appears.
fn placeholder_line(
    area: &TextArea,
    bounds: Bounds<Pixels>,
    palette: &style::Palette,
    window: &Window,
) -> Option<(ShapedLine, Point<Pixels>, Pixels)> {
    let st = area.style;
    let hidden = area
        .placeholder_animation
        .value_in(window, if area.placeholder_hidden { 1.0 } else { 0.0 });
    if area.placeholder.is_empty() || hidden >= 1.0 {
        return None;
    }
    let focused = area
        .focused_animation
        .value_in(window, if area.focused { 1.0 } else { 0.0 });
    let mut color = hsla(mix(
        st.placeholder_fg.resolve(palette),
        st.placeholder_fg_active.resolve(palette),
        focused,
    ));
    color.a *= 1.0 - hidden;
    let run = plain_run(area.placeholder.len(), &font(st.placeholder_font), color);
    let line = window.text_system().shape_line(
        area.placeholder.clone(),
        px_of(st.placeholder_font.size),
        &[run],
        None,
    );
    let margin = area.metrics.margin;
    #[expect(clippy::cast_precision_loss, reason = "style sizes are small")]
    let shift = px(interpolate(0.0, -st.placeholder_shift.0 as f32, hidden));
    let origin = point(
        bounds.left() + margin + px_of(st.placeholder_margins.left) + shift,
        bounds.top() + margin + px_of(st.placeholder_margins.top),
    );
    Some((line, origin, font_height(st.placeholder_font)))
}
