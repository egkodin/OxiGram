//! "Archive settings" (`ArchiveSettingsBox`): which archived chats stay
//! there on new messages, and archiving new chats from strangers.

use gpui::{
    App, AppContext as _, Context, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    ParentElement, Render, Styled, Task, Window, div,
};
use i18n::key;
use model::archive_settings::ArchiveSettings;
use style::layers::{BOX_WIDE_WIDTH, DEFAULT_BOX};
use ui::{BoxFrame, RoundButton};

use super::{Env, rows};
use crate::lang::tr;
use crate::px_of;

/// TDLib's flag: the account may archive new chats from strangers.
const CAN_ARCHIVE: &str = "can_archive_and_mute_new_chats_from_unknown_users";

/// What the folders switch shows: on while unmuted chats stay archived
/// (`unarchiveOnNewMessage() != AnyUnmuted`).
fn keeps_from_folders(settings: ArchiveSettings) -> bool {
    settings.keep_unmuted || settings.keep_from_folders
}

/// The first switch (`lng_settings_always_in_archive`); turning it off keeps
/// what the folders switch shows.
fn with_keep_unmuted(settings: ArchiveSettings, keep: bool) -> ArchiveSettings {
    ArchiveSettings {
        keep_unmuted: keep,
        keep_from_folders: keeps_from_folders(settings),
        ..settings
    }
}

pub(super) struct ArchiveBox {
    env: Env,
    /// None while Telegram is asked.
    loaded: Option<(ArchiveSettings, bool)>,
    focus: FocusHandle,
    _load: Task<()>,
}

impl Focusable for ArchiveBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

pub(super) fn show(env: &Env, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx: &mut Context<ArchiveBox>| {
        let session = env.targets.store.read(cx).session().clone();
        let (settings, can) = (session.archive_settings(), session.bool_option(CAN_ARCHIVE));
        let task = crate::store::spawn(
            async move { Ok::<_, model::RequestError>((settings.await?, can.await?)) },
            cx,
        );
        let load = cx.spawn(async move |this, cx| match task.await {
            Ok(loaded) => {
                let _ = this.update(cx, |this, cx| {
                    this.loaded = Some(loaded);
                    cx.notify();
                });
            }
            Err(error) => tracing::warn!(%error, "the archive settings failed"),
        });
        ArchiveBox {
            env: env.clone(),
            loaded: None,
            focus: cx.focus_handle(),
            _load: load,
        }
    });
    env.targets
        .layers
        .update(cx, |layers, cx| layers.show_above(view, window, cx));
}

impl ArchiveBox {
    fn save(&mut self, settings: ArchiveSettings, cx: &mut Context<Self>) {
        let Some((current, can)) = self.loaded else {
            return;
        };
        if current == settings {
            return;
        }
        self.loaded = Some((settings, can));
        let task = self
            .env
            .targets
            .store
            .read(cx)
            .session()
            .set_archive_settings(settings);
        crate::peer_menu::send(task, "archive settings", cx);
        cx.notify();
    }

    /// A switch row that saves `change(current, !on)`.
    fn switch(
        &self,
        id: &'static str,
        text: i18n::Key,
        is_on: bool,
        change: fn(ArchiveSettings, bool) -> ArchiveSettings,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let weak = cx.weak_entity();
        rows::plain(id, tr(cx, text))
            .toggle(is_on)
            .on_click(move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| {
                    if let Some((current, _)) = this.loaded {
                        this.save(change(current, !is_on), cx);
                    }
                });
            })
            .into_any_element()
    }

    fn content(&self, cx: &mut Context<Self>) -> gpui::Div {
        let Some((settings, can)) = self.loaded else {
            return div().child(rows::divider_text(tr(cx, key::lng_contacts_loading), cx));
        };
        let mut column = div()
            .flex()
            .flex_col()
            .child(rows::skip())
            .child(rows::subtitle(tr(cx, key::lng_settings_unmuted_chats), cx))
            .child(self.switch(
                "archive/unmuted",
                key::lng_settings_always_in_archive,
                settings.keep_unmuted,
                with_keep_unmuted,
                cx,
            ))
            .child(rows::skip())
            .child(rows::divider_text(
                tr(cx, key::lng_settings_unmuted_chats_about),
                cx,
            ));
        if !settings.keep_unmuted {
            column = column
                .child(rows::skip())
                .child(rows::subtitle(
                    tr(cx, key::lng_settings_chats_from_folders),
                    cx,
                ))
                .child(self.switch(
                    "archive/folders",
                    key::lng_settings_always_in_archive,
                    keeps_from_folders(settings),
                    |s, keep| ArchiveSettings {
                        keep_from_folders: keep,
                        ..s
                    },
                    cx,
                ))
                .child(rows::skip())
                .child(rows::divider_text(
                    tr(cx, key::lng_settings_chats_from_folders_about),
                    cx,
                ));
        }
        // shortcut: shown when TDLib allows it, not also for Premium users as tdesktop does; TDLib's flag covers Premium on current servers.
        if can {
            column = column
                .child(rows::skip())
                .child(rows::subtitle(tr(cx, key::lng_settings_new_unknown), cx))
                .child(self.switch(
                    "archive/unknown",
                    key::lng_settings_auto_archive,
                    settings.archive_unknown,
                    |s, archive| ArchiveSettings {
                        archive_unknown: archive,
                        ..s
                    },
                    cx,
                ))
                .child(rows::skip())
                .child(rows::divider_text(
                    tr(cx, key::lng_settings_auto_archive_about),
                    cx,
                ));
        }
        column
    }
}

impl Render for ArchiveBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = cx.weak_entity();
        let frame = BoxFrame::new("archive-settings-box", &DEFAULT_BOX)
            .width(px_of(BOX_WIDE_WIDTH))
            .title(tr(cx, key::lng_settings_archive_title))
            .row(self.content(cx))
            .button(
                RoundButton::new(
                    "archive-settings-done",
                    &DEFAULT_BOX.button,
                    tr(cx, key::lng_about_done),
                )
                .on_click(move |_, window, cx| {
                    let _ = close.update(cx, |this, cx| {
                        this.env
                            .targets
                            .layers
                            .update(cx, |layers, cx| layers.close(window, cx));
                    });
                }),
            );
        div().track_focus(&self.focus).child(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turning_off_keep_unmuted_keeps_the_folders_switch_as_shown() {
        let all = ArchiveSettings {
            archive_unknown: false,
            keep_unmuted: true,
            keep_from_folders: false,
        };
        let off = with_keep_unmuted(all, false);
        assert!(!off.keep_unmuted);
        assert!(off.keep_from_folders, "the folders switch showed on");
        let none = ArchiveSettings {
            keep_from_folders: false,
            ..off
        };
        assert!(!with_keep_unmuted(none, false).keep_from_folders);
    }
}
