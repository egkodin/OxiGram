//! "Local storage" (`Settings::LocalStorage`): the cache by kind, ticked
//! for clearing, and "Clear all" / "Clear selected".
// shortcut: no usage chart, device usage line, clearing progress box or size and age limits; with row "Manage local storage".

use std::collections::BTreeSet;

use gpui::{Context, IntoElement, ParentElement, Render, Styled, Task, Window, div};
use i18n::{Key, key};
use model::storage::{StorageKind, StorageUsage};
use style::settings::{SETTINGS_BUTTON_ACTIVE, SETTINGS_CHECKBOX, SETTINGS_CHECKBOX_PADDING};
use style::widgets::{DEFAULT_CHECK, DEFAULT_TOAST};
use ui::{Checkbox, Checked, SettingsButton};

use super::{Env, rows};
use crate::file_sizes::size_text;
use crate::lang::{tr, tr_with};
use crate::px_of;

fn title(kind: StorageKind) -> Key {
    match kind {
        StorageKind::Images => key::lng_local_storage_image_title,
        StorageKind::Stickers => key::lng_local_storage_sticker_title,
        StorageKind::Voice => key::lng_local_storage_voice_title,
        StorageKind::Round => key::lng_local_storage_round_title,
        StorageKind::Animations => key::lng_local_storage_animation_title,
        StorageKind::Media => key::lng_local_storage_media_title,
    }
}

pub(super) struct StoragePage {
    env: Env,
    /// None while TDLib counts.
    usage: Option<StorageUsage>,
    /// Kinds left out of clearing.
    unticked: BTreeSet<usize>,
    is_clearing: bool,
    _task: Task<()>,
}

impl StoragePage {
    pub(super) fn new(env: Env, cx: &mut Context<Self>) -> Self {
        let task = crate::store::spawn(env.targets.store.read(cx).session().storage_usage(), cx);
        let load = cx.spawn(async move |this, cx| match task.await {
            Ok(usage) => {
                let _ = this.update(cx, |this, cx| {
                    this.usage = Some(usage);
                    cx.notify();
                });
            }
            Err(error) => tracing::warn!(%error, "storage statistics failed"),
        });
        Self {
            env,
            usage: None,
            unticked: BTreeSet::new(),
            is_clearing: false,
            _task: load,
        }
    }

    fn chosen(&self) -> Vec<StorageKind> {
        let usage = self.usage.clone().unwrap_or_default();
        StorageKind::ALL
            .iter()
            .enumerate()
            .filter(|(index, kind)| !self.unticked.contains(index) && usage.usage(**kind).size > 0)
            .map(|(_, kind)| *kind)
            .collect()
    }

    fn clear(&mut self, cx: &mut Context<Self>) {
        let kinds = self.chosen();
        let Some(before) = self.usage.as_ref().map(StorageUsage::total) else {
            return;
        };
        if kinds.is_empty() || self.is_clearing {
            return;
        }
        self.is_clearing = true;
        let task = crate::store::spawn(
            self.env
                .targets
                .store
                .read(cx)
                .session()
                .clear_storage(&kinds),
            cx,
        );
        self._task = cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.is_clearing = false;
                match result {
                    Ok(usage) => {
                        let freed = size_text((before - usage.total()).max(0));
                        let text = tr_with(cx, key::lng_local_storage_cleared, &[("size", &freed)]);
                        this.env.targets.layers.update(cx, |layers, cx| {
                            layers.show_toast(
                                text.into(),
                                &DEFAULT_TOAST,
                                crate::toast::DEFAULT_DURATION,
                                cx,
                            );
                        });
                        this.usage = Some(usage);
                    }
                    Err(error) => tracing::warn!(%error, "clearing the cache failed"),
                }
                cx.notify();
            });
        });
        cx.notify();
    }

    fn kind_row(
        &self,
        index: usize,
        kind: StorageKind,
        usage: &StorageUsage,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Div> {
        let size = usage.usage(kind).size;
        if size == 0 {
            return None;
        }
        let padding = SETTINGS_CHECKBOX_PADDING;
        let is_ticked = !self.unticked.contains(&index);
        let weak = cx.weak_entity();
        Some(
            div()
                .relative()
                .pl(px_of(padding.left))
                .pr(px_of(padding.right))
                .pt(px_of(padding.top))
                .pb(px_of(padding.bottom))
                .child(
                    Checkbox::new(
                        ("storage-kind", index),
                        &SETTINGS_CHECKBOX,
                        &DEFAULT_CHECK,
                        tr(cx, title(kind)),
                        if is_ticked { Checked::Yes } else { Checked::No },
                    )
                    .on_toggle(move |_, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            if !this.unticked.remove(&index) {
                                this.unticked.insert(index);
                            }
                            cx.notify();
                        });
                    }),
                )
                .child(
                    div()
                        .absolute()
                        .right(px_of(padding.right))
                        .top(px_of(padding.top))
                        .child(rows::text(
                            size_text(size),
                            &style::settings::SETTINGS_UPDATE_STATE,
                            cx,
                        )),
                ),
        )
    }
}

impl Render for StoragePage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(usage) = self.usage.clone() else {
            return div().child(rows::divider_text(tr(cx, key::lng_contacts_loading), cx));
        };
        if usage.total() == 0 {
            return div()
                .flex()
                .flex_col()
                .child(rows::skip())
                .child(rows::subtitle(tr(cx, key::lng_local_storage_empty), cx))
                .child(rows::skip())
                .child(rows::divider_text(tr(cx, key::lng_local_storage_about), cx));
        }
        let kinds: Vec<_> = StorageKind::ALL
            .iter()
            .enumerate()
            .filter_map(|(index, kind)| self.kind_row(index, *kind, &usage, cx))
            .collect();
        let text = if self.is_clearing {
            key::lng_local_storage_clearing
        } else if self.unticked.is_empty() {
            key::lng_local_storage_clear
        } else {
            key::lng_local_storage_clear_selected
        };
        let selected: i64 = self
            .chosen()
            .iter()
            .map(|kind| usage.usage(*kind).size)
            .sum();
        let weak = cx.weak_entity();
        let clear = SettingsButton::new("storage-clear", &SETTINGS_BUTTON_ACTIVE, tr(cx, text))
            .label(size_text(selected))
            .on_click(move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.clear(cx));
            });
        div()
            .flex()
            .flex_col()
            .child(rows::skip())
            .child(rows::subtitle(
                format!(
                    "{} \u{2014} {}",
                    tr(cx, key::lng_local_storage_summary),
                    size_text(usage.total())
                ),
                cx,
            ))
            .children(kinds)
            .child(rows::skip())
            .child(clear)
            .child(rows::skip())
            .child(rows::divider_text(tr(cx, key::lng_local_storage_about), cx))
    }
}
