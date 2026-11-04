//! The Settings layer (`Info::Settings::Widget` in an `Info::LayerWidget`):
//! the main page and the sections it opens, one over another, under
//! tdesktop's top bar: back, the title or the search field, the search
//! button on the main page, the page's menu and close.
// shortcut: a layer only, not the third column; tdesktop picks by the window's width.

mod archive_box;
mod blocked;
mod chat;
mod folder_box;
mod folder_chats_box;
mod folders;
mod global_ttl;
mod information;
mod main_page;
mod messages_box;
mod notifications;
mod payment_box;
mod personal_channel;
mod privacy;
mod privacy_box;
mod qr_box;
mod rgram_page;
mod rows;
mod search;
mod self_destruct_box;
mod session_box;
mod sessions;
mod username_box;
mod websites;

pub(crate) use chat::toggle_night_mode;
pub(crate) use username_box::show as show_username_box;

use gpui::{
    AnyElement, AnyView, App, AppContext as _, Context, DismissEvent, Entity, FocusHandle,
    Focusable, IntoElement, ParentElement, Pixels, Point, Render, Styled, Subscription, WeakEntity,
    Window, anchored, deferred, div, prelude::*, px,
};
use i18n::{Key, key};
use style::info::{
    INFO_DESIRED_WIDTH, INFO_LAYER_TOP_BAR, INFO_LAYER_TOP_BAR_CLOSE, INFO_LAYER_TOP_BAR_MENU,
    INFO_LAYER_TOP_BAR_SEARCH,
};
use style::layers::BOX_RADIUS;
use style::menu_icons::{MENU_ICON_ADD_ACCOUNT, MENU_ICON_EDIT, MENU_ICON_LEAVE_ATTENTION};
use style::settings::{
    SETTINGS_SEARCH_NO_RESULTS, SETTINGS_SEARCH_RESULT, SETTINGS_SEARCH_RESULT_DETAILS,
    SETTINGS_SEARCH_RESULT_NO_ICON,
};
use style::widgets::POPUP_MENU_WITH_ICONS;
use ui::{IconButton, InputEvent, InputField, MenuTone, PopupMenu, SettingsButton, color, font};

use self::search::{Entry, Indexed, Target};
use crate::boxes::Callback;
use crate::lang::tr;
use crate::layers::{CloseLayer, Stacking};
use crate::links::Targets;
use crate::px_of;

/// How far a menu keeps from the window's edges.
const MENU_EDGE: f32 = 4.0;

/// The pages of Settings (`Settings::Type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PageId {
    Main,
    Information,
    Notifications,
    Privacy,
    Blocked,
    Sessions,
    Websites,
    GlobalTtl,
    Chat,
    Folders,
    Advanced,
    Calls,
    Premium,
    Credits,
    Business,
    Rgram,
}

impl PageId {
    fn title(self) -> Key {
        match self {
            Self::Main => key::lng_menu_settings,
            Self::Information => key::lng_settings_section_info,
            Self::Notifications => key::lng_settings_section_notify,
            Self::Privacy => key::lng_settings_section_privacy,
            Self::Blocked => key::lng_settings_blocked_users,
            Self::Sessions => key::lng_settings_sessions_title,
            Self::Websites => key::lng_settings_connected_title,
            Self::GlobalTtl => key::lng_settings_ttl_title,
            Self::Chat => key::lng_settings_section_chat_settings,
            Self::Folders => key::lng_filters_title,
            Self::Advanced => key::lng_settings_advanced,
            Self::Calls => key::lng_settings_section_devices,
            Self::Premium | Self::Business => key::lng_premium_summary_title,
            Self::Credits => key::lng_credits_summary_title,
            Self::Rgram => key::rgram_settings_title,
        }
    }

    fn parent(self) -> Option<Self> {
        match self {
            Self::Main => None,
            Self::Blocked | Self::Sessions | Self::Websites | Self::GlobalTtl => {
                Some(Self::Privacy)
            }
            _ => Some(Self::Main),
        }
    }

    /// How many pages lie above it (`CalculateDepth`).
    fn depth(self) -> usize {
        self.parent().map_or(0, |parent| parent.depth() + 1)
    }

    /// Every page's search entries.
    fn entries() -> impl Iterator<Item = &'static Entry> {
        main_page::entries().chain(notifications::entries())
    }
}

/// What every page works with.
#[derive(Clone)]
pub(crate) struct Env {
    pub targets: Targets,
    pub layer: WeakEntity<SettingsLayer>,
    pub on_logout: Callback,
}

impl Env {
    /// Opens `page` over the current one.
    pub(crate) fn open(&self, page: PageId, window: &mut Window, cx: &mut App) {
        let _ = self
            .layer
            .update(cx, |layer, cx| layer.push(page, window, cx));
    }
}

struct Page {
    id: PageId,
    view: AnyView,
}

struct Searching {
    field: Entity<InputField>,
    index: Vec<Indexed>,
    _subscription: Subscription,
}

struct OpenMenu {
    position: Point<Pixels>,
    menu: Entity<PopupMenu>,
    _subscription: Subscription,
}

pub(crate) struct SettingsLayer {
    env: Env,
    stack: Vec<Page>,
    searching: Option<Searching>,
    menu: Option<OpenMenu>,
    focus: FocusHandle,
}

impl std::fmt::Debug for SettingsLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SettingsLayer")
            .field("depth", &self.stack.len())
            .finish_non_exhaustive()
    }
}

/// Opens Settings over the window (`showSettings`).
pub(crate) fn show(targets: Targets, on_logout: Callback, window: &mut Window, cx: &mut App) {
    let layers = targets.layers.clone();
    let layer = cx.new(|cx| {
        let env = Env {
            targets,
            layer: cx.weak_entity(),
            on_logout,
        };
        let mut layer = SettingsLayer {
            env,
            stack: Vec::new(),
            searching: None,
            menu: None,
            focus: cx.focus_handle(),
        };
        layer.push(PageId::Main, window, cx);
        layer
    });
    layers.update(cx, |layers, cx| {
        layers.show_info(layer, Stacking::Replace, window, cx)
    });
}

impl Focusable for SettingsLayer {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl SettingsLayer {
    fn push(&mut self, id: PageId, window: &mut Window, cx: &mut Context<Self>) {
        let env = self.env.clone();
        let view: AnyView = match id {
            PageId::Main => cx.new(|cx| main_page::MainPage::new(env, cx)).into(),
            PageId::Information => cx
                .new(|cx| information::InformationPage::new(env, window, cx))
                .into(),
            PageId::Notifications => cx
                .new(|cx| notifications::NotificationsPage::new(env, cx))
                .into(),
            PageId::Privacy => cx.new(|cx| privacy::PrivacyPage::new(env, cx)).into(),
            PageId::Blocked => cx.new(|cx| blocked::BlockedPage::new(env, cx)).into(),
            PageId::Sessions => cx.new(|cx| sessions::SessionsPage::new(env, cx)).into(),
            PageId::Websites => cx.new(|cx| websites::WebsitesPage::new(env, cx)).into(),
            PageId::GlobalTtl => cx.new(|cx| global_ttl::GlobalTtlPage::new(env, cx)).into(),
            PageId::Chat => cx.new(|cx| chat::ChatPage::new(env, cx)).into(),
            PageId::Folders => cx.new(|cx| folders::FoldersPage::new(env, cx)).into(),
            PageId::Rgram => cx.new(|_| rgram_page::RgramPage).into(),
            // shortcut: the sections not built yet open empty; each lands with its own step of M7.
            _ => cx.new(|_| Blank).into(),
        };
        self.stack.push(Page { id, view });
        cx.notify();
    }

    /// Goes back a page, or out of the search.
    fn back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.searching.take().is_some() {
            self.focus.focus(window, cx);
        } else if self.stack.len() > 1 {
            self.stack.pop();
            // Privacy's rows show what its pages change; ask Telegram again (tdesktop's values are live).
            if self.current() == PageId::Privacy {
                self.stack.pop();
                self.push(PageId::Privacy, window, cx);
            }
        }
        cx.notify();
    }

    fn close(&self, window: &mut Window, cx: &mut App) {
        self.env
            .targets
            .layers
            .update(cx, |layers, cx| layers.close_all(window, cx));
    }

    fn current(&self) -> PageId {
        self.stack.last().map_or(PageId::Main, |page| page.id)
    }

    fn start_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let placeholder = tr(cx, key::lng_dlg_filter);
        let field = cx.new(|cx| {
            InputField::new(
                &INFO_LAYER_TOP_BAR.search_row.field,
                placeholder,
                window,
                cx,
            )
        });
        let subscription = cx.subscribe(&field, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Changed) {
                cx.notify();
            }
        });
        crate::focus(&field, window, cx);
        let index = PageId::entries()
            .map(|entry| Indexed::new(*entry, tr(cx, entry.title).to_string()))
            .collect();
        self.searching = Some(Searching {
            field,
            index,
            _subscription: subscription,
        });
        cx.notify();
    }

    /// Opens what a found entry points to (`Search::createEntryButton`).
    fn open_found(&mut self, entry: Entry, window: &mut Window, cx: &mut Context<Self>) {
        crate::settings::bump_settings_search(entry.id, cx);
        self.searching = None;
        self.stack.truncate(1);
        let page = match entry.target {
            Target::Page(page) => page,
            Target::Item => entry.page,
        };
        let mut path = Vec::new();
        let mut at = Some(page);
        while let Some(id) = at.filter(|id| *id != PageId::Main) {
            path.push(id);
            at = id.parent();
        }
        for id in path.into_iter().rev() {
            self.push(id, window, cx);
        }
        cx.notify();
    }

    /// The main page's menu (`Main::fillTopBarMenu`).
    fn show_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let env = self.env.clone();
        let can_add =
            crate::accounts::domain(cx).is_some_and(|accounts| accounts.read(cx).can_add());
        let menu = cx.new(|cx| {
            let mut menu = PopupMenu::new(&POPUP_MENU_WITH_ICONS, cx);
            if can_add {
                menu.action(
                    tr(cx, key::lng_menu_add_account),
                    MENU_ICON_ADD_ACCOUNT,
                    MenuTone::Normal,
                    |_, cx| {
                        if let Some(accounts) = crate::accounts::domain(cx) {
                            let _ = accounts.update(cx, |accounts, cx| accounts.add(cx));
                        }
                    },
                );
            }
            let edit = env.clone();
            menu.action(
                tr(cx, key::lng_settings_information),
                MENU_ICON_EDIT,
                MenuTone::Normal,
                move |window, cx| edit.open(PageId::Information, window, cx),
            );
            let logout = env.on_logout.clone();
            menu.action(
                tr(cx, key::lng_settings_logout),
                MENU_ICON_LEAVE_ATTENTION,
                MenuTone::Attention,
                move |window, cx| logout(window, cx),
            );
            menu
        });
        let subscription = cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.menu = None;
            cx.notify();
        });
        menu.read(cx).focus_handle(cx).focus(window, cx);
        self.menu = Some(OpenMenu {
            position: window.mouse_position(),
            menu,
            _subscription: subscription,
        });
        cx.notify();
    }

    fn top_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let st = &INFO_LAYER_TOP_BAR;
        let has_back = self.searching.is_some() || self.stack.len() > 1;
        let weak = cx.weak_entity();
        let back = has_back.then(|| {
            let weak = weak.clone();
            IconButton::new("settings-back", &st.back).on_click(move |_, window, cx| {
                let _ = weak.update(cx, |this, cx| this.back(window, cx));
            })
        });
        let middle = match &self.searching {
            Some(searching) => div()
                .flex_1()
                .min_w_0()
                .pt(px_of(st.search_row.padding.top))
                .child(searching.field.clone())
                .into_any_element(),
            None => {
                let title_font = st.title.style.font;
                let left = if has_back {
                    px(0.0)
                } else {
                    px_of(st.title_position.x)
                };
                div()
                    .flex_1()
                    .min_w_0()
                    .pl(left)
                    .pt(px_of(st.title_position.y))
                    .truncate()
                    .font(font(title_font))
                    .text_size(px_of(title_font.size))
                    .line_height(ui::font_height(title_font))
                    .text_color(color(cx, st.title.text_fg))
                    .child(tr(cx, self.current().title()))
                    .into_any_element()
            }
        };
        let is_main = self.current() == PageId::Main && self.searching.is_none();
        let search = is_main.then(|| {
            let weak = weak.clone();
            IconButton::new("settings-search", &INFO_LAYER_TOP_BAR_SEARCH).on_click(
                move |_, window, cx| {
                    let _ = weak.update(cx, |this, cx| this.start_search(window, cx));
                },
            )
        });
        let menu = is_main.then(|| {
            let weak = weak.clone();
            IconButton::new("settings-menu", &INFO_LAYER_TOP_BAR_MENU).on_click(
                move |_, window, cx| {
                    let _ = weak.update(cx, |this, cx| this.show_menu(window, cx));
                },
            )
        });
        let close = IconButton::new("settings-close", &INFO_LAYER_TOP_BAR_CLOSE).on_click(
            move |_, window, cx| {
                let _ = weak.update(cx, |this, cx| this.close(window, cx));
            },
        );
        div()
            .flex_none()
            .flex()
            .h(px_of(st.height))
            .children(back)
            .child(middle)
            .children(search)
            .children(menu)
            .child(close)
            .into_any_element()
    }

    /// The found entries, or the recent ones for an empty query
    /// (`Search::rebuildResults`).
    // shortcut: no FAQ articles from the FAQ page's Instant View (`FaqSuggestions`), no keyboard selection; with row "Settings search with keyboard navigation".
    fn results(&self, searching: &Searching, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let query = searching.field.read(cx).text().to_owned();
        let found: Vec<&Indexed> = if search::is_query(&query) {
            search::find(&searching.index, &query)
        } else {
            crate::settings::recent_settings_searches(cx)
                .iter()
                .filter_map(|id| {
                    searching
                        .index
                        .iter()
                        .find(|indexed| indexed.entry.id == id.as_str())
                })
                .collect()
        };
        if found.is_empty() {
            if !search::is_query(&query) {
                return Vec::new();
            }
            let label = &SETTINGS_SEARCH_NO_RESULTS;
            return vec![
                div()
                    .flex()
                    .justify_center()
                    .pt(px_of(
                        style::settings::SETTINGS_SEARCH_NO_RESULTS_PADDING.top,
                    ))
                    .pb(px_of(
                        style::settings::SETTINGS_SEARCH_NO_RESULTS_PADDING.bottom,
                    ))
                    .font(font(label.style.font))
                    .text_size(px_of(label.style.font.size))
                    .text_color(color(cx, label.text_fg))
                    .child(tr(cx, key::lng_search_tab_no_results))
                    .into_any_element(),
            ];
        }
        let mut rows = Vec::new();
        if !search::is_query(&query) {
            rows.push(rows::subtitle(tr(cx, key::lng_recent_title), cx));
        }
        for (index, indexed) in found.into_iter().enumerate() {
            rows.push(self.result_row(index, indexed, cx));
        }
        rows
    }

    fn result_row(&self, index: usize, indexed: &Indexed, cx: &mut Context<Self>) -> AnyElement {
        let entry = indexed.entry;
        let path = page_path(entry, cx);
        let st = if entry.icon.is_some() {
            &SETTINGS_SEARCH_RESULT
        } else {
            &SETTINGS_SEARCH_RESULT_NO_ICON
        };
        let weak = cx.weak_entity();
        let mut button = SettingsButton::new(("settings-found", index), st, indexed.title.clone())
            .on_click(move |_, window, cx| {
                let _ = weak.update(cx, |this, cx| this.open_found(entry, window, cx));
            });
        if let Some(icon) = entry.icon {
            button = button.icon(icon);
        }
        let details = &SETTINGS_SEARCH_RESULT_DETAILS;
        div()
            .relative()
            .child(button)
            .when(!path.is_empty(), |this| {
                this.child(
                    div()
                        .absolute()
                        .left(px_of(st.padding.left))
                        .bottom(px_of(st.padding.bottom))
                        .font(font(details.style.font))
                        .text_size(px_of(details.style.font.size))
                        .text_color(color(cx, details.text_fg))
                        .child(path),
                )
            })
            .into_any_element()
    }
}

/// "Section > Subsection" under a found entry (`sectionPath`).
fn page_path(entry: Entry, cx: &App) -> String {
    let start = match entry.target {
        Target::Page(page) => page.parent(),
        Target::Item => Some(entry.page),
    };
    let mut titles = Vec::new();
    let mut at = start;
    while let Some(id) = at.filter(|id| *id != PageId::Main) {
        titles.push(tr(cx, id.title()).to_string());
        at = id.parent();
    }
    titles.reverse();
    titles.join(" > ")
}

impl Render for SettingsLayer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let max_height = window.viewport_size().height - crate::layers::info_top(window) * 2.0;
        let top_bar = self.top_bar(cx);
        let depth = self.stack.len();
        let content: Vec<AnyElement> = match &self.searching {
            Some(searching) => self.results(searching, cx),
            None => self
                .stack
                .last()
                .map(|page| page.view.clone().into_any_element())
                .into_iter()
                .collect(),
        };
        let menu = self.menu.as_ref().map(|open| {
            deferred(
                anchored()
                    .anchor(gpui::Anchor::TopRight)
                    .position(open.position)
                    .snap_to_window_with_margin(px(MENU_EDGE))
                    .child(div().child(open.menu.clone())),
            )
            .with_priority(1)
        });
        let has_back = self.searching.is_some() || self.stack.len() > 1;
        div()
            .track_focus(&self.focus)
            .when(has_back, |this| {
                this.on_action(cx.listener(|this, _: &CloseLayer, window, cx| {
                    this.back(window, cx);
                    cx.stop_propagation();
                }))
            })
            .w(px_of(INFO_DESIRED_WIDTH))
            .max_h(max_height)
            .flex()
            .flex_col()
            .rounded(px_of(BOX_RADIUS))
            .overflow_hidden()
            .bg(color(cx, INFO_LAYER_TOP_BAR.bg))
            .child(top_bar)
            .child(
                div()
                    .id((
                        "settings-scroll",
                        depth + usize::from(self.searching.is_some()) * 100,
                    ))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .children(content),
            )
            .children(menu)
    }
}

/// A section that is not built yet.
struct Blank;

impl Render for Blank {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
