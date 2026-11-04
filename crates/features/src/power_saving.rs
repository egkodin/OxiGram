//! What Power Usage turns off (`PowerSaving::Flag`), in tdesktop's bits.

use gpui::App;

/// One animation the owner can turn off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Flag {
    Animations,
    StickersPanel,
    StickersChat,
    EmojiPanel,
    EmojiReactions,
    EmojiChat,
    ChatBackground,
    ChatSpoiler,
    Calls,
    EmojiStatus,
    ChatEffects,
}

impl Flag {
    /// tdesktop's bit, so the saved value means the same thing.
    pub(crate) const fn bit(self) -> u32 {
        1 << self as u32
    }
}

/// The flags turned off, as saved.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Saving(pub u32);

impl Saving {
    #[must_use]
    pub(crate) fn has(self, flag: Flag) -> bool {
        self.0 & flag.bit() != 0
    }

    #[must_use]
    pub(crate) fn with(self, flag: Flag, is_saving: bool) -> Self {
        if is_saving {
            Self(self.0 | flag.bit())
        } else {
            Self(self.0 & !flag.bit())
        }
    }
}

/// The animation still plays (`PowerSaving::On` is false).
pub(crate) fn plays(flag: Flag, cx: &App) -> bool {
    !crate::settings::power_saving(cx).has(flag)
}

/// Makes `saving` take effect where it is read once.
pub(crate) fn apply(saving: Saving) {
    if saving.has(Flag::Animations) {
        ui::anim::disable();
    } else {
        ui::anim::enable();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_keep_tdesktops_bits() {
        assert_eq!(Flag::Animations.bit(), 1);
        assert_eq!(Flag::StickersChat.bit(), 1 << 2);
        assert_eq!(Flag::ChatEffects.bit(), 1 << 10);
        let saving = Saving::default().with(Flag::EmojiChat, true);
        assert!(saving.has(Flag::EmojiChat) && !saving.has(Flag::EmojiPanel));
        assert_eq!(saving.with(Flag::EmojiChat, false), Saving::default());
    }
}
