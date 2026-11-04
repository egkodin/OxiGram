//! "Folders" (`Settings::Folders`): the user's folders, creating one,
//! Telegram's suggestions and where the folder tabs go.
// shortcut: no Lottie icon at the top, no folder count limit box (TDLib's error shows instead), no drag to reorder (the tabs reorder), no "Folder tags" (Premium) and no tab style radios; removing applies at once instead of on leaving the page. With rows "Chat folders settings".

use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement as _, Styled, Subscription, Task, Window, div,
};
use i18n::key;
use model::chat::{ChatList, Folder};
use model::chat_folders::Suggested;
use style::boxes::{CONTACTS_CHECK_POSITION, CONTACTS_PADDING, CONTACTS_STATUS_FONT};
use style::chat_helpers::{FILTERS_REMOVE, STICKERS_REMOVE_SKIP};
use style::layers::DEFAULT_SUBSECTION_TITLE_PADDING;
use style::settings::{
    SETTINGS_BUTTON_ACTIVE, SETTINGS_FILTER_ADD_RECOMMENDED, SETTINGS_FILTER_DIVIDER_LABEL,
    SETTINGS_FILTER_DIVIDER_LABEL_PADDING, SETTINGS_ICON_ADD, SETTINGS_SEND_TYPE,
    SETTINGS_SEND_TYPE_PADDING,
};
use style::widgets::{DEFAULT_DIVIDER_LABEL, DEFAULT_PEER_LIST_ITEM, DEFAULT_RADIO};
use style::{ColorId, ColorRef};
use ui::{Checkbox, Checked, IconButton, RoundButton, color, font, font_height};

use super::{Env, folder_box, rows};
use crate::boxes::Callback;
use crate::lang::{tr, tr_plural};
use crate::px_of;
use crate::settings::{self, FoldersPlacement};

/// " • " between the count and "shareable" (`Ui::kQBullet`).
const BULLET: &str = " \u{2022} ";

pub(super) struct FoldersPage {
    env: Env,
    /// Telegram's suggestions; None while asked.
    suggested: Option<Vec<Suggested>>,
    _load: Task<()>,
    _store: Subscription,
}

impl FoldersPage {
    pub(super) fn new(env: Env, cx: &mut Context<Self>) -> Self {
        let task = crate::store::spawn(
            env.targets.store.read(cx).session().recommended_folders(),
            cx,
        );
        let load = cx.spawn(async move |this, cx| match task.await {
            Ok(suggested) => {
                let _ = this.update(cx, |this, cx| {
                    this.suggested = Some(suggested);
                    cx.notify();
                });
            }
            Err(error) => tracing::warn!(%error, "recommended folders failed"),
        });
        let store = cx.observe(&env.targets.store, |_, _, cx| cx.notify());
        Self {
            env,
            suggested: None,
            _load: load,
            _store: store,
        }
    }

    /// "5 chats" or "No chats", and whether it is shared (`ComputeCountString`).
    // shortcut: counts the chats TDLib has loaded for the folder, as tdesktop's list does; a folder never opened may show fewer.
    fn status(&self, folder: &Folder, cx: &App) -> String {
        let count = self
            .env
            .targets
            .store
            .read(cx)
            .state()
            .chats
            .list_len(ChatList::Folder(folder.id));
        let mut text = if count > 0 {
            tr_plural(
                cx,
                key::lng_filters_chats_count,
                i64::try_from(count).unwrap_or(i64::MAX),
                &[],
            )
        } else {
            tr(cx, key::lng_filters_no_chats).to_string()
        };
        if folder.is_shareable {
            text.push_str(BULLET);
            text.push_str(&tr(cx, key::lng_filters_shareable_status));
        }
        text
    }

    /// Asks before removing a folder with the user's links
    /// (`markForRemoval`).
    fn remove(&self, folder: &Folder, window: &mut Window, cx: &mut App) {
        let session = self.env.targets.store.read(cx).session().clone();
        let id = folder.id;
        let remove: Callback = Rc::new(move |_, cx| {
            crate::peer_menu::send(session.delete_folder(id), "remove folder", cx);
        });
        if !folder.has_my_invite_links {
            remove(window, cx);
            return;
        }
        crate::boxes::confirm_attention(
            &self.env.targets.layers,
            tr(cx, key::lng_filters_delete_sure).to_string(),
            tr(cx, key::lng_box_delete),
            remove,
            window,
            cx,
        );
    }

    fn folder_row(&self, index: usize, folder: &Folder, cx: &mut Context<Self>) -> AnyElement {
        let id = folder.id;
        let env = self.env.clone();
        let removing = folder.clone();
        let weak = cx.weak_entity();
        let remove = IconButton::new(("folder-remove", index), &FILTERS_REMOVE).on_click(
            move |_, window, cx| {
                cx.stop_propagation();
                let _ = weak.update(cx, |this, cx| this.remove(&removing, window, cx));
            },
        );
        let right = CONTACTS_PADDING.right.0 + CONTACTS_CHECK_POSITION.x.0 - STICKERS_REMOVE_SKIP.0;
        list_row(
            ("folder-row", index),
            folder.name.text.clone().into(),
            self.status(folder, cx).into(),
            div()
                .absolute()
                .right(px_of(style::Px(right)))
                .child(remove),
            cx,
        )
        .on_click(move |_, window, cx| folder_box::show(&env, Some(id), window, cx))
        .into_any_element()
    }

    fn suggestion_row(&self, index: usize, suggested: &Suggested, cx: &App) -> AnyElement {
        let session = self.env.targets.store.read(cx).session().clone();
        let rules = suggested.rules.clone();
        let add = RoundButton::new(
            ("folder-add-recommended", index),
            &SETTINGS_FILTER_ADD_RECOMMENDED,
            tr(cx, key::lng_filters_recommended_add),
        )
        .on_click(move |_, _, cx| {
            let task = session.create_folder(&rules);
            crate::peer_menu::send(
                async move { task.await.map(drop) },
                "add recommended folder",
                cx,
            );
        });
        let right = CONTACTS_PADDING.right.0 + CONTACTS_CHECK_POSITION.x.0;
        list_row(
            ("folder-suggested", index),
            suggested.rules.name().to_owned().into(),
            suggested.description.clone().into(),
            div().absolute().right(px_of(style::Px(right))).child(add),
            cx,
        )
        .into_any_element()
    }

    /// The suggestions no folder already matches (`SetupRecommendedSection`).
    fn recommended(&self, folders: &[Folder], cx: &App) -> Option<gpui::Div> {
        let rows: Vec<_> = self
            .suggested
            .as_deref()?
            .iter()
            .filter(|suggested| {
                !folders
                    .iter()
                    .any(|folder| folder.name.text == suggested.rules.name())
            })
            .enumerate()
            .map(|(index, suggested)| self.suggestion_row(index, suggested, cx))
            .collect();
        if rows.is_empty() {
            return None;
        }
        Some(
            div()
                .flex()
                .flex_col()
                .child(rows::divider(cx))
                .child(rows::skip())
                .child(rows::subtitle(tr(cx, key::lng_filters_recommended), cx))
                .children(rows)
                .child(rows::skip()),
        )
    }

    /// "Tabs view": the left column or the top (`BuildViewSection`).
    fn view(cx: &App) -> gpui::Div {
        let padding = SETTINGS_SEND_TYPE_PADDING;
        let current = settings::folders_placement(cx);
        let radio = |placement: FoldersPlacement, text: i18n::Key, cx: &App| {
            let checked = if current == placement {
                Checked::Yes
            } else {
                Checked::No
            };
            div()
                .pl(px_of(padding.left))
                .pr(px_of(padding.right))
                .pt(px_of(padding.top))
                .pb(px_of(padding.bottom))
                .child(
                    Checkbox::radio(
                        ("folders-view", placement as usize),
                        &SETTINGS_SEND_TYPE,
                        &DEFAULT_RADIO,
                        tr(cx, text),
                        checked,
                    )
                    .on_toggle(move |window, cx| {
                        settings::set_folders_placement(placement, cx);
                        window.refresh();
                    }),
                )
        };
        div()
            .flex()
            .flex_col()
            .child(rows::divider(cx))
            .child(rows::skip())
            .child(rows::subtitle(tr(cx, key::lng_filters_view_subtitle), cx))
            .child(radio(
                FoldersPlacement::Sidebar,
                key::lng_filters_vertical,
                cx,
            ))
            .child(radio(
                FoldersPlacement::Top,
                key::lng_filters_horizontal,
                cx,
            ))
            .child(rows::skip())
            .child(rows::skip())
    }
}

/// A folder's row: name and status (`FilterRowButton`), `right` at the end.
fn list_row(
    id: impl Into<gpui::ElementId>,
    name: SharedString,
    status: SharedString,
    right: gpui::Div,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    let st = &DEFAULT_PEER_LIST_ITEM;
    let left = px_of(DEFAULT_SUBSECTION_TITLE_PADDING.left);
    let line = |top: i32, text: SharedString, font_style: style::Font, fg: ColorRef| {
        div()
            .absolute()
            .left(left)
            .right(px_of(style::Px(120)))
            .top(px_of(style::Px(CONTACTS_PADDING.top.0 + top)))
            .font(font(font_style))
            .text_size(px_of(font_style.size))
            .line_height(font_height(font_style))
            .text_color(color(cx, fg))
            .whitespace_nowrap()
            .overflow_hidden()
            .text_ellipsis()
            .child(text)
    };
    div()
        .id(id)
        .relative()
        .h(px_of(st.height))
        .cursor_pointer()
        .hover(|this| this.bg(color(cx, ColorRef::Palette(ColorId::WindowBgOver))))
        .child(line(
            style::boxes::CONTACTS_NAME_TOP.0,
            name,
            st.name_style.font,
            st.name_fg,
        ))
        .child(line(
            style::boxes::CONTACTS_STATUS_TOP.0,
            status,
            CONTACTS_STATUS_FONT,
            st.status_fg,
        ))
        .child(right.top_0().bottom_0().flex().items_center())
}

impl Render for FoldersPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let folders = self
            .env
            .targets
            .store
            .read(cx)
            .state()
            .chats
            .folders()
            .to_vec();
        let label = &SETTINGS_FILTER_DIVIDER_LABEL;
        let padding = SETTINGS_FILTER_DIVIDER_LABEL_PADDING;
        let about = div()
            .bg(color(cx, DEFAULT_DIVIDER_LABEL.bar.bg))
            .pt(px_of(padding.top))
            .pb(px_of(padding.bottom))
            .px(px_of(DEFAULT_SUBSECTION_TITLE_PADDING.left))
            .text_center()
            .font(font(label.style.font))
            .text_size(px_of(label.style.font.size))
            .line_height(font_height(label.style.font))
            .text_color(color(cx, label.text_fg))
            .child(tr(cx, key::lng_filters_about));
        let rows_list: Vec<_> = folders
            .iter()
            .enumerate()
            .map(|(index, folder)| self.folder_row(index, folder, cx))
            .collect();
        let env = self.env.clone();
        let create = ui::SettingsButton::new(
            "folders/create",
            &SETTINGS_BUTTON_ACTIVE,
            tr(cx, key::lng_filters_create),
        )
        .icon_round(
            SETTINGS_ICON_ADD,
            ColorRef::Palette(ColorId::WindowBgActive),
        )
        .on_click(move |_, window, cx| folder_box::show(&env, None, window, cx));
        div()
            .flex()
            .flex_col()
            .child(about)
            .child(rows::skip())
            .child(rows::subtitle(tr(cx, key::lng_filters_subtitle), cx))
            .children(rows_list)
            .child(create)
            .child(rows::skip())
            .children(self.recommended(&folders, cx))
            .child(Self::view(cx))
    }
}
