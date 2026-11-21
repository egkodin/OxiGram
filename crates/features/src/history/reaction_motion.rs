//! Reactions that move (`ReactionFlyAnimation`, `Reactions::Strip`): an
//! animated custom emoji loops in its chip, a reaction the user adds plays
//! its center and around animations once, and the strip's emoji play their
//! appear animation when the menu opens.

use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{AnyElement, Context, IntoElement as _, ParentElement as _, RenderImage, Styled as _};
use gpui::{div, img, px};
use model::MessageId;
use model::file::File;
use model::reaction::ReactionKind;
use style::Px;
use style::chat::{REACTION_INLINE_IMAGE, REACTION_INLINE_SIZE};
use style::chat_helpers::REACT_STRIP_IMAGE;

use super::HistoryView;
use crate::lottie::Play;
use crate::power_saving::{Flag, plays};

/// `ReactionFlyAnimation`'s effect around the chip: twice the image.
const AROUND_MULTIPLIER: f32 = 2.0;
/// A flight whose files never land gives up after this long.
const FLIGHT_TIMEOUT: Duration = Duration::from_secs(5);
/// Instances of flights and strips, apart from message ids' instances.
const OWN_INSTANCES: u64 = 1 << 63;

/// The flight under way and the clock that numbers new ones.
#[derive(Debug, Default)]
pub(super) struct Motions {
    flying: Option<Flying>,
    count: u64,
}

impl Motions {
    /// A fresh instance for each reaction menu.
    pub(super) fn next_instance(&mut self) -> u64 {
        self.count += 1;
        OWN_INSTANCES | self.count
    }
}

/// A reaction the user just added.
#[derive(Debug, Clone)]
struct Flying {
    message: MessageId,
    kind: ReactionKind,
    started: Instant,
    center_started: bool,
    around_started: bool,
}

/// What a chip draws in place of its glyph and around it.
#[derive(Default)]
pub(super) struct Motion {
    face: Option<Face>,
    around: Option<Face>,
}

/// A frame and the side it is drawn at, in points.
struct Face {
    image: Arc<RenderImage>,
    side: f32,
}

/// Where a once animation stands.
enum Once {
    Loading,
    Playing(Arc<RenderImage>),
    Done,
}

impl HistoryView {
    /// Plays the reaction the user added, as Power Usage allows.
    pub(super) fn start_flying(
        &mut self,
        message: MessageId,
        kind: &ReactionKind,
        cx: &Context<Self>,
    ) {
        if !plays(Flag::EmojiReactions, cx) {
            return;
        }
        self.motions.flying = Some(Flying {
            message,
            kind: kind.clone(),
            started: Instant::now(),
            center_started: false,
            around_started: false,
        });
    }

    pub(super) fn stop_flying(&mut self, message: MessageId, kind: &ReactionKind) {
        if self
            .motions
            .flying
            .as_ref()
            .is_some_and(|flying| flying.message == message && flying.kind == *kind)
        {
            self.motions.flying = None;
        }
    }

    /// The chip's moving face: its flight, else an animated custom emoji.
    pub(super) fn chip_motion(
        &mut self,
        message: MessageId,
        kind: &ReactionKind,
        cx: &mut Context<Self>,
    ) -> Motion {
        if let Some(motion) = self.flight(message, kind, cx) {
            return motion;
        }
        Motion {
            face: self
                .custom_emoji_frame(kind, REACTION_INLINE_SIZE, cx)
                .map(|image| Face {
                    image,
                    side: points(REACTION_INLINE_SIZE),
                }),
            around: None,
        }
    }

    /// A strip reaction's frame: the emoji's appear animation, once per
    /// menu, or a custom emoji's own.
    pub(super) fn strip_frame(
        &mut self,
        kind: &ReactionKind,
        instance: u64,
        cx: &mut Context<Self>,
    ) -> Option<Arc<RenderImage>> {
        let ReactionKind::Emoji(emoji) = kind else {
            return self.custom_emoji_frame(kind, REACT_STRIP_IMAGE, cx);
        };
        // The appear animation starts from nothing: no still of it.
        if !plays(Flag::EmojiReactions, cx) {
            return None;
        }
        let file = self
            .store
            .update(cx, |store, cx| store.reaction_animations(emoji, cx))?
            .appear?;
        let side = points(REACT_STRIP_IMAGE);
        let key = self.lottie_key(&file, (side, side), instance, cx)?;
        crate::lottie::frame(&key, Play::Once, cx)
    }

    fn flight(
        &mut self,
        message: MessageId,
        kind: &ReactionKind,
        cx: &mut Context<Self>,
    ) -> Option<Motion> {
        let mut flying = self.motions.flying.clone()?;
        if flying.message != message || flying.kind != *kind {
            return None;
        }
        // shortcut: custom emoji fly without the generic around animation; with `chooseGenericAnimation`'s set.
        let ReactionKind::Emoji(emoji) = kind else {
            self.motions.flying = None;
            return None;
        };
        let animations = self
            .store
            .update(cx, |store, cx| store.reaction_animations(emoji, cx));
        let image = points(REACTION_INLINE_IMAGE);
        // The history has one flight. Reuse its decoded animation when the
        // same reaction is added again and restart each asset on arrival.
        let instance = OWN_INSTANCES | (1 << 62) | cx.entity_id().as_u64();
        let mut once = |file: Option<&File>, side: f32, started: &mut bool| match file {
            Some(file) => self.play_once(file, side, instance, started, cx),
            None => Once::Done,
        };
        let (center, around) = match &animations {
            Some(found) => (
                once(found.center.as_ref(), image, &mut flying.center_started),
                once(
                    found.around.as_ref(),
                    image * AROUND_MULTIPLIER,
                    &mut flying.around_started,
                ),
            ),
            None => (Once::Loading, Once::Loading),
        };
        if is_over(&center, &around, flying.started.elapsed()) {
            self.motions.flying = None;
            return None;
        }
        self.motions.flying = Some(flying);
        let playing = |once: Once, side: f32| match once {
            Once::Playing(image) => Some(Face { image, side }),
            Once::Loading | Once::Done => None,
        };
        Some(Motion {
            face: playing(center, image),
            around: playing(around, image * AROUND_MULTIPLIER),
        })
    }

    fn play_once(
        &mut self,
        file: &File,
        side: f32,
        instance: u64,
        started: &mut bool,
        cx: &mut Context<Self>,
    ) -> Once {
        let Some(key) = self.lottie_key(file, (side, side), instance, cx) else {
            return Once::Loading;
        };
        if !std::mem::replace(started, true) {
            crate::lottie::replay(&key, cx);
        }
        if crate::lottie::is_done(&key, cx) {
            return Once::Done;
        }
        crate::lottie::frame(&key, Play::Once, cx).map_or(Once::Loading, Once::Playing)
    }

    /// A custom emoji reaction's frame (`kEmojiChat` in
    /// `Reactions::InlineList`).
    fn custom_emoji_frame(
        &mut self,
        kind: &ReactionKind,
        side: Px,
        cx: &mut Context<Self>,
    ) -> Option<Arc<RenderImage>> {
        let ReactionKind::CustomEmoji(id) = kind else {
            return None;
        };
        self.custom_emoji(*id, points(side), cx)
    }
}

#[expect(clippy::cast_precision_loss, reason = "a size in points")]
fn points(side: Px) -> f32 {
    side.0 as f32
}

/// The chip's face over its square: the moving frame centered on it at its
/// size, and the around animation over all.
pub(super) fn chip_face(motion: Motion, still: AnyElement) -> AnyElement {
    let Motion { face, around } = motion;
    if face.is_none() && around.is_none() {
        return still;
    }
    let square = points(REACTION_INLINE_SIZE);
    let centered = |frame: Face| {
        let skip = (square - frame.side) / 2.0;
        img(frame.image)
            .absolute()
            .left(px(skip))
            .top(px(skip))
            .size(px(frame.side))
    };
    div()
        .relative()
        .flex_none()
        .size(px(square))
        .child(match face {
            Some(frame) => centered(frame).into_any_element(),
            None => still,
        })
        .children(around.map(centered))
        .into_any_element()
}

/// A flight ends when both animations played, or when neither plays after
/// `FLIGHT_TIMEOUT` (their files never landed).
fn is_over(center: &Once, around: &Once, elapsed: Duration) -> bool {
    let is_playing = |once: &Once| matches!(once, Once::Playing(_));
    matches!((center, around), (Once::Done, Once::Done))
        || (elapsed > FLIGHT_TIMEOUT && !is_playing(center) && !is_playing(around))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flight_ends_when_both_played_or_nothing_came() {
        let soon = Duration::from_millis(100);
        assert!(is_over(&Once::Done, &Once::Done, soon));
        assert!(!is_over(&Once::Loading, &Once::Done, soon));
        assert!(is_over(&Once::Loading, &Once::Done, FLIGHT_TIMEOUT * 2));
        let frame = Arc::new(RenderImage::new(Vec::new()));
        assert!(!is_over(
            &Once::Playing(frame),
            &Once::Loading,
            FLIGHT_TIMEOUT * 2
        ));
    }

    #[test]
    fn each_flight_and_menu_gets_its_own_clock() {
        let mut motions = Motions::default();
        let (first, second) = (motions.next_instance(), motions.next_instance());
        assert_ne!(first, second);
        assert!(first & OWN_INSTANCES != 0, "apart from message instances");
    }
}
