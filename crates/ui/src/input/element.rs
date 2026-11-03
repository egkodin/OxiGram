//! Paints the field: background, border, placeholder, text, selection and
//! cursor.

use std::ops::Range;

use gpui::{
    App, Bounds, Element, ElementId, ElementInputHandler, Entity, GlobalElementId, LayoutId,
    Pixels, Point, ShapedLine, Style, TextRun, UnderlineStyle, Window, fill, point, prelude::*, px,
    relative, size,
};

use crate::anim::{interpolate, mix};
use crate::font::{font, font_height};
use crate::theme::{Theme, hsla};

use super::{InputField, px_of};

/// Paints the field: background, border, placeholder, text, selection and
/// cursor, and registers the input handler.
pub(super) struct FieldElement {
    pub(super) input: Entity<InputField>,
}

impl IntoElement for FieldElement {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

pub(super) struct Prepaint {
    quads: Vec<gpui::PaintQuad>,
    placeholder: Option<(ShapedLine, Point<Pixels>, Pixels)>,
    line: ShapedLine,
    hint: Option<ShapedLine>,
    origin: Point<Pixels>,
    line_height: Pixels,
    cursor: Option<gpui::PaintQuad>,
    /// The text area between the side margins; text outside it is clipped.
    text_bounds: Bounds<Pixels>,
    scroll_x: Pixels,
}

impl Element for FieldElement {
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
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = relative(1.0).into();
        (window.request_layout(style, [], cx), ())
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
        let input = self.input.read(cx);
        let st = input.style;
        let palette = &cx.global::<Theme>().palette;
        let text_font = font(st.style.font);
        let font_size = px_of(st.style.font.size);
        let display = input.display_text();
        let selected = input.text_to_display(input.buffer.selected.start)
            ..input.text_to_display(input.buffer.selected.end);
        let marked = input
            .buffer
            .marked
            .as_ref()
            .map(|range| input.text_to_display(range.start)..input.text_to_display(range.end));
        let runs = text_runs(
            &display,
            &text_font,
            hsla(st.text_fg.resolve(palette)),
            hsla(palette.history_text_in_fg_selected),
            &selected,
            marked.as_ref(),
        );
        let line = window
            .text_system()
            .shape_line(display.into(), font_size, &runs, None);
        let hint = input
            .corrector
            .as_ref()
            .and_then(|corrector| corrector.trailing_hint(&input.buffer.text))
            .map(|hint| {
                let run = plain_run(
                    hint.len(),
                    &text_font,
                    hsla(st.placeholder_fg.resolve(palette)),
                );
                window
                    .text_system()
                    .shape_line(hint, font_size, &[run], None)
            });
        // A pattern hint replaces the placeholder (`setPlaceholderHidden`).
        let placeholder = (!input.placeholder.is_empty() && hint.is_none())
            .then(|| placeholder_line(input, bounds, palette, window));

        let line_height = font_height(st.style.font);
        let inner = input.text_width_inside(bounds.size.width);
        let cursor_x = line.x_for_index(input.text_to_display(input.buffer.cursor()));
        let scroll_x = super::next_scroll(input.scroll_x, cursor_x, line.width, inner);
        let text_bounds = Bounds::new(
            point(bounds.left() + px_of(st.text_margins.left), bounds.top()),
            size(inner, bounds.size.height),
        );
        let origin = point(
            bounds.left() + input.text_start(bounds.size.width, line.width) - scroll_x,
            bounds.top() + px_of(st.text_margins.top),
        );
        let mut quads = if st.border_radius.0 > 0 {
            vec![round_surrounding(input, bounds, palette, window)]
        } else {
            let mut quads = vec![fill(bounds, hsla(st.text_bg.resolve(palette)))];
            quads.extend(border_quads(input, bounds, palette, window));
            quads
        };
        if !selected.is_empty() {
            let clip = |x: Pixels| x.clamp(text_bounds.left(), text_bounds.right());
            let start = clip(origin.x + line.x_for_index(selected.start));
            let end = clip(origin.x + line.x_for_index(selected.end));
            quads.push(fill(
                Bounds::from_corners(point(start, origin.y), point(end, origin.y + line_height)),
                hsla(palette.msg_in_bg_selected),
            ));
        }
        let cursor = (input.focus.is_focused(window)
            && input.cursor_visible
            && selected.is_empty())
        .then(|| {
            fill(
                Bounds::new(
                    point(origin.x + cursor_x, origin.y),
                    size(px(1.0), line_height),
                ),
                hsla(st.text_fg.resolve(palette)),
            )
        });
        Prepaint {
            quads,
            placeholder,
            line,
            hint,
            origin,
            line_height,
            cursor,
            text_bounds,
            scroll_x,
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
        let focus = self.input.read(cx).focus.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        for quad in prepaint.quads.drain(..) {
            window.paint_quad(quad);
        }
        if let Some((placeholder, origin, line_height)) = prepaint.placeholder.take() {
            window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
                if let Err(error) =
                    placeholder.paint(origin, line_height, gpui::TextAlign::Left, None, window, cx)
                {
                    tracing::warn!(%error, "cannot paint a placeholder");
                }
            });
        }
        let mask = gpui::ContentMask {
            bounds: prepaint.text_bounds,
        };
        window.with_content_mask(Some(mask), |window| {
            let Prepaint {
                line,
                origin,
                line_height,
                ..
            } = &*prepaint;
            if let Err(error) = line.paint(
                *origin,
                *line_height,
                gpui::TextAlign::Left,
                None,
                window,
                cx,
            ) {
                tracing::warn!(%error, "cannot paint a field's text");
            }
            if let Some(hint) = prepaint.hint.take() {
                let at = point(prepaint.origin.x + prepaint.line.width, prepaint.origin.y);
                if let Err(error) = hint.paint(
                    at,
                    prepaint.line_height,
                    gpui::TextAlign::Left,
                    None,
                    window,
                    cx,
                ) {
                    tracing::warn!(%error, "cannot paint a field's hint");
                }
            }
        });
        if let Some(cursor) = prepaint.cursor.take() {
            window.paint_quad(cursor);
        }
        let (line, scroll_x) = (prepaint.line.clone(), prepaint.scroll_x);
        self.input.update(cx, |input, _| {
            input.last_line = Some(line);
            input.last_bounds = Some(bounds);
            input.scroll_x = scroll_x;
        });
    }
}

/// The placeholder, moved up and shrunk by the shift animation and colored
/// by focus and error.
fn placeholder_line(
    input: &InputField,
    bounds: Bounds<Pixels>,
    palette: &style::Palette,
    window: &mut Window,
) -> (ShapedLine, Point<Pixels>, Pixels) {
    let st = input.style;
    let shift = input
        .placeholder_animation
        .value_in(window, if input.placeholder_shifted { 1.0 } else { 0.0 });
    let focused = input
        .focused_animation
        .value_in(window, if input.focused { 1.0 } else { 0.0 });
    let error = input
        .error_animation
        .value_in(window, if input.error { 1.0 } else { 0.0 });
    let fg = mix(
        mix(
            st.placeholder_fg.resolve(palette),
            st.placeholder_fg_active.resolve(palette),
            focused,
        ),
        st.placeholder_fg_error.resolve(palette),
        error,
    );
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the scale is between 0 and 1"
    )]
    let scale = 1.0 - (1.0 - st.placeholder_scale as f32) * shift;
    let size = px_of(st.placeholder_font.size) * scale;
    let run = plain_run(
        input.placeholder.len(),
        &font(st.placeholder_font),
        hsla(fg),
    );
    let shaped = window
        .text_system()
        .shape_line(input.placeholder.clone(), size, &[run], None);
    #[expect(clippy::cast_precision_loss, reason = "style sizes are small")]
    let origin = point(
        bounds.left()
            + px_of(st.text_margins.left)
            + px_of(st.placeholder_margins.left)
            + px(interpolate(0.0, st.placeholder_shift_left.0 as f32, shift)),
        bounds.top()
            + px_of(st.text_margins.top)
            + px_of(st.placeholder_margins.top)
            + px(interpolate(0.0, st.placeholder_shift.0 as f32, shift)),
    );
    (shaped, origin, font_height(st.placeholder_font) * scale)
}

/// The resting bottom border, and the active one growing from the click.
/// `paintRoundSurrounding`: a rounded field whose fill and outline turn to
/// their active colors while focused.
// shortcut: switches at focus without tdesktop's fade; animate when the search field gets its cancel button.
fn round_surrounding(
    input: &InputField,
    bounds: Bounds<Pixels>,
    palette: &style::Palette,
    window: &Window,
) -> gpui::PaintQuad {
    let st = input.style;
    let focused = if input.focus.is_focused(window) {
        1.0
    } else {
        0.0
    };
    let divide = if st.border_denominator > 0 {
        st.border_denominator
    } else {
        1
    };
    #[expect(clippy::cast_precision_loss, reason = "border widths are small")]
    let border = px(st.border.0 as f32 / divide as f32);
    gpui::quad(
        bounds,
        px_of(st.border_radius),
        hsla(mix(
            st.text_bg.resolve(palette),
            st.text_bg_active.resolve(palette),
            focused,
        )),
        border,
        hsla(mix(
            st.border_fg.resolve(palette),
            st.border_fg_active.resolve(palette),
            focused,
        )),
        gpui::BorderStyle::Solid,
    )
}

fn border_quads(
    input: &InputField,
    bounds: Bounds<Pixels>,
    palette: &style::Palette,
    window: &Window,
) -> Vec<gpui::PaintQuad> {
    let st = input.style;
    let mut quads = Vec::with_capacity(2);
    if st.border.0 > 0 {
        let height = px_of(st.border);
        quads.push(fill(
            Bounds::new(
                point(bounds.left(), bounds.bottom() - height),
                size(bounds.size.width, height),
            ),
            hsla(st.border_fg.resolve(palette)),
        ));
    }
    let shown = input.border_shown.value_in(window, 1.0);
    let opacity = input
        .border_opacity
        .value_in(window, if input.border_visible { 1.0 } else { 0.0 });
    if st.border_active.0 == 0 || opacity <= 0.0 {
        return quads;
    }
    let width = f32::from(bounds.size.width);
    let start = input
        .border_start
        .map_or(width / 2.0, f32::from)
        .clamp(0.0, width);
    let from = (start * (1.0 - shown)).round();
    let to = start + ((width - start) * shown).round();
    if to <= from {
        return quads;
    }
    let error = input
        .error_animation
        .value_in(window, if input.error { 1.0 } else { 0.0 });
    let mut color = hsla(mix(
        st.border_fg_active.resolve(palette),
        st.border_fg_error.resolve(palette),
        error,
    ));
    color.a *= opacity;
    let height = px_of(st.border_active);
    quads.push(fill(
        Bounds::new(
            point(bounds.left() + px(from), bounds.bottom() - height),
            size(px(to - from), height),
        ),
        color,
    ));
    quads
}

pub(crate) fn plain_run(len: usize, font: &gpui::Font, color: gpui::Hsla) -> TextRun {
    TextRun {
        len,
        font: font.clone(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    }
}

/// Runs for the field's text: selected text in the selection color, marked
/// (composing) text underlined.
pub(crate) fn text_runs(
    text: &str,
    font: &gpui::Font,
    color: gpui::Hsla,
    selected_color: gpui::Hsla,
    selected: &Range<usize>,
    marked: Option<&Range<usize>>,
) -> Vec<TextRun> {
    let mut cuts = vec![0, text.len(), selected.start, selected.end];
    if let Some(marked) = marked {
        cuts.extend([marked.start, marked.end]);
    }
    cuts.retain(|cut| *cut <= text.len());
    cuts.sort_unstable();
    cuts.dedup();
    cuts.windows(2)
        .filter(|pair| pair[1] > pair[0])
        .map(|pair| {
            let range = pair[0]..pair[1];
            let is_selected = selected.start <= range.start && range.end <= selected.end;
            let is_marked =
                marked.is_some_and(|marked| marked.start <= range.start && range.end <= marked.end);
            let run_color = if is_selected { selected_color } else { color };
            TextRun {
                underline: is_marked.then(|| UnderlineStyle {
                    color: Some(run_color),
                    thickness: px(1.0),
                    wavy: false,
                }),
                ..plain_run(range.len(), font, run_color)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_split_at_selection_and_composition() {
        let font = gpui::font("Open Sans");
        let runs = text_runs(
            "hello world",
            &font,
            gpui::black(),
            gpui::white(),
            &(2..5),
            Some(&(4..8)),
        );
        let lens: Vec<usize> = runs.iter().map(|run| run.len).collect();
        assert_eq!(lens, vec![2, 2, 1, 3, 3]);
        assert_eq!(runs[1].color, gpui::white());
        assert!(runs[2].underline.is_some() && runs[3].underline.is_some());
    }
}
