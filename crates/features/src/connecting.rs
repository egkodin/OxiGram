//! `Window::ConnectionState`: the pill in the bottom left corner that shows
//! a spinner while TDLib is not connected.

use std::time::{Duration, Instant};

use gpui::{
    Bounds, Context, Entity, IntoElement, ParentElement, Render, Styled, Subscription, Task,
    Window, canvas, div, point, prelude::*, size,
};
use i18n::key;
use model::ConnectionState;
use style::window::{
    CONNECTING_DURATION, CONNECTING_LEFT, CONNECTING_LEFT_SHADOW, CONNECTING_MARGIN,
    CONNECTING_RADIAL, CONNECTING_RIGHT, CONNECTING_RIGHT_SHADOW, CONNECTING_TEXT_PADDING,
};
use style::{ColorId, ColorRef};
use ui::anim::{Easing, Simple};
use ui::{InfiniteRadial, color, font, font_height, icon, icon_size, paint_arc};

use crate::Store;
use crate::lang::tr;
use crate::px_of;

/// `kIgnoreStartConnectingFor`: no pill in the first seconds after launch.
const IGNORE_START_CONNECTING_FOR: Duration = Duration::from_secs(3);
/// `kConnectingStateDelay`: a short reconnect shows nothing.
const CONNECTING_STATE_DELAY: Duration = Duration::from_secs(1);

pub struct Connecting {
    store: Entity<Store>,
    started: Instant,
    is_shown: bool,
    visibility: Simple,
    radial: InfiniteRadial,
    is_hovered: bool,
    pending: Option<Task<()>>,
    _subscription: Subscription,
}

impl std::fmt::Debug for Connecting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connecting")
            .field("is_shown", &self.is_shown)
            .finish_non_exhaustive()
    }
}

/// Whether the state counts as not connected; `Updating` is connected.
fn is_connecting(state: ConnectionState) -> bool {
    matches!(
        state,
        ConnectionState::WaitingForNetwork
            | ConnectionState::ConnectingToProxy
            | ConnectionState::Connecting
    )
}

impl Connecting {
    pub fn new(store: Entity<Store>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&store, |this, _, cx| this.refresh(cx));
        let mut this = Self {
            store,
            started: Instant::now(),
            is_shown: false,
            visibility: Simple::default(),
            radial: InfiniteRadial::new(&CONNECTING_RADIAL),
            is_hovered: false,
            pending: None,
            _subscription: subscription,
        };
        this.refresh(cx);
        this
    }

    /// `ConnectionState::refreshState`: hides at once, shows after the delays.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        let wants = is_connecting(self.store.read(cx).state().connection);
        if wants == self.is_shown {
            self.pending = None;
            return;
        }
        if !wants {
            self.pending = None;
            self.set_shown(false, cx);
            return;
        }
        if self.pending.is_some() {
            return;
        }
        let now = Instant::now();
        let at = (now + CONNECTING_STATE_DELAY).max(self.started + IGNORE_START_CONNECTING_FOR);
        let timer = cx.background_executor().timer(at - now);
        self.pending = Some(cx.spawn(async move |this, cx| {
            timer.await;
            this.update(cx, |this, cx| {
                this.pending = None;
                if is_connecting(this.store.read(cx).state().connection) {
                    this.set_shown(true, cx);
                }
            })
            .ok();
        }));
    }

    fn set_shown(&mut self, shown: bool, cx: &mut Context<Self>) {
        self.is_shown = shown;
        let (from, to) = if shown { (0.0, 1.0) } else { (1.0, 0.0) };
        self.visibility
            .start(from, to, CONNECTING_DURATION, Easing::Linear);
        if shown {
            self.radial.start();
        }
        cx.notify();
    }
}

impl Render for Connecting {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visible = self
            .visibility
            .value_in(window, if self.is_shown { 1.0 } else { 0.0 });
        if visible == 0.0 {
            self.radial.stop();
            return div().into_any_element();
        }
        let left = icon_size(CONNECTING_LEFT);
        let right = icon_size(CONNECTING_RIGHT);
        let margin = CONNECTING_MARGIN;
        let height = left.height + px_of(margin.top) + px_of(margin.bottom);
        let text_font = style::basic::NORMAL_FONT;
        // shortcut: the label shows only on hover, as tdesktop's Connecting state; the proxy icon and box come with M6 proxies.
        let text = self.is_hovered.then(|| tr(cx, key::lng_connecting));
        let radial = self.radial;
        let progress = canvas(
            |_, _, _| {},
            move |bounds, (), window, cx| {
                if let Some(state) = radial.state_in(window) {
                    let st = &CONNECTING_RADIAL;
                    let side = px_of(st.size.width);
                    let arc = Bounds::new(
                        point(
                            bounds.left() + (bounds.size.height - side) / 2.0,
                            bounds.top() + (bounds.size.height - side) / 2.0,
                        ),
                        size(side, side),
                    );
                    let thickness = px_of(st.thickness);
                    let mut fg = color(cx, st.color);
                    fg.a *= state.shown;
                    paint_arc(window, arc.dilate(-thickness / 2.0), state, thickness, fg);
                }
            },
        )
        .absolute()
        .left_0()
        .top_0()
        .size(height);
        let body = div()
            .h(left.height)
            .bg(color(cx, ColorRef::Palette(ColorId::WindowBg)))
            .children(text.map(|text| {
                div()
                    .pl(px_of(CONNECTING_TEXT_PADDING.left))
                    .pr(px_of(CONNECTING_TEXT_PADDING.right))
                    .pt(px_of(CONNECTING_TEXT_PADDING.top))
                    .font(font(text_font))
                    .text_size(px_of(text_font.size))
                    .line_height(font_height(text_font))
                    .text_color(color(cx, ColorRef::Palette(ColorId::WindowSubTextFg)))
                    .child(text)
            }));
        let side_icon = |shadow, fill, size: gpui::Size<gpui::Pixels>| {
            div()
                .relative()
                .w(size.width)
                .h(size.height)
                .child(div().absolute().child(icon(shadow)))
                .child(div().absolute().child(icon(fill)))
        };
        // Slides up from under the bottom edge while it fades in.
        let offset = (height - px_of(margin.top)) * (1.0 - visible);
        div()
            .id("connecting")
            .absolute()
            .left_0()
            .bottom(-offset)
            .h(height)
            .p(px_of(margin.top))
            .flex()
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.is_hovered = *hovered;
                cx.notify();
            }))
            .child(side_icon(CONNECTING_LEFT_SHADOW, CONNECTING_LEFT, left))
            .child(body)
            .child(side_icon(CONNECTING_RIGHT_SHADOW, CONNECTING_RIGHT, right))
            .child(progress)
            .into_any_element()
    }
}
