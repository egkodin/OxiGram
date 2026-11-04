//! The profile (`Info::Profile::Widget` in an `Info::LayerWidget`): the
//! cover with the userpic, name and status, the details, notifications,
//! the shared media lists, members and the actions; for one's own profile
//! the details edit (`Settings::Information`).
// shortcut: a layer only, not the third column or a separate window; tdesktop picks by the window's width.

mod actions;
mod edit;
mod media_list;
mod report;
mod rows;
pub(crate) use rows::open_user_photos;

use std::collections::HashSet;
use std::future::Future;

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement,
    Render, SharedString, Styled, Subscription, Window, div, prelude::*,
};
use i18n::key;
use model::chat::{Chat, ChatKind};
use model::profile::{GroupProfile, Member, SharedMedia, UserProfile};
use model::{BasicGroupId, ChatId, FileId, RequestError, State, SupergroupId, UserId};
use style::info::{INFO_DESIRED_WIDTH, INFO_LAYER_TOP_BAR, INFO_LAYER_TOP_BAR_CLOSE};
use style::layers::BOX_RADIUS;
use style::widgets::BOX_DIVIDER_HEIGHT;
use style::{ColorId, ColorRef};
use ui::{IconButton, color, font, font_height};

use crate::lang::tr;
use crate::layers::{Layers, Stacking};
use crate::links::Targets;
use crate::px_of;
use crate::store::Store;

/// Userpics download ahead of other files.
const USERPIC_PRIORITY: i32 = 32;

/// Whose profile it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Subject {
    User(UserId),
    Group(BasicGroupId),
    Supergroup { id: SupergroupId, is_channel: bool },
}

impl Subject {
    fn of(state: &State, chat: ChatId) -> Self {
        match state.chats.chat(chat).map(|chat| chat.kind) {
            Some(ChatKind::Group { group }) => Self::Group(group),
            Some(ChatKind::Supergroup {
                supergroup,
                is_channel,
            }) => Self::Supergroup {
                id: supergroup,
                is_channel,
            },
            Some(ChatKind::Private { user } | ChatKind::Secret { user }) => Self::User(user),
            // A private chat's id is its user's id.
            None => Self::User(UserId(chat.0)),
        }
    }
}

/// Opens the profile of `chat` over the window.
pub(crate) fn show(targets: Targets, chat: ChatId, window: &mut Window, cx: &mut App) {
    let layers = targets.layers.clone();
    let view = cx.new(|cx| InfoView::new(targets, chat, cx));
    layers.update(cx, |layers, cx| {
        layers.show_info(view, Stacking::Replace, window, cx)
    });
}

/// Opens the profile of `chat` over the open box, which comes back when it
/// closes (a profile opened from Settings).
pub(crate) fn show_above(targets: Targets, chat: ChatId, window: &mut Window, cx: &mut App) {
    let layers = targets.layers.clone();
    let view = cx.new(|cx| InfoView::new(targets, chat, cx));
    layers.update(cx, |layers, cx| {
        layers.show_info(view, Stacking::Above, window, cx)
    });
}

pub(crate) struct InfoView {
    targets: Targets,
    chat: ChatId,
    subject: Subject,
    user: Option<UserProfile>,
    group: Option<GroupProfile>,
    members: Vec<Member>,
    /// The lists with something in them, in tdesktop's order.
    counts: Vec<(SharedMedia, i32)>,
    requested: HashSet<FileId>,
    /// The member row under the mouse.
    hovered: Option<usize>,
    focus: FocusHandle,
    _store: Subscription,
}

impl std::fmt::Debug for InfoView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InfoView")
            .field("chat", &self.chat)
            .finish_non_exhaustive()
    }
}

impl Focusable for InfoView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl InfoView {
    fn new(targets: Targets, chat: ChatId, cx: &mut Context<Self>) -> Self {
        let subject = Subject::of(targets.store.read(cx).state(), chat);
        let store = cx.observe(&targets.store, |_, _, cx| cx.notify());
        let mut view = Self {
            targets,
            chat,
            subject,
            user: None,
            group: None,
            members: Vec::new(),
            counts: Vec::new(),
            requested: HashSet::new(),
            hovered: None,
            focus: cx.focus_handle(),
            _store: store,
        };
        view.reload(cx);
        view
    }

    fn store(&self) -> &Entity<Store> {
        &self.targets.store
    }

    fn layers(&self) -> &Entity<Layers> {
        &self.targets.layers
    }

    pub(super) fn chat<'a>(&self, state: &'a State) -> Option<&'a Chat> {
        state.chats.chat(self.chat)
    }

    pub(super) fn is_self(&self, cx: &App) -> bool {
        let me = self.store().read(cx).state().me;
        matches!(self.subject, Subject::User(user) if Some(user) == me)
    }

    /// Asks TDLib for everything the chat list does not have; edits and
    /// blocks call it again.
    pub(super) fn reload(&mut self, cx: &mut Context<Self>) {
        let session = self.store().read(cx).session().clone();
        match self.subject {
            Subject::User(user) => {
                self.load(
                    session.user_profile(user),
                    "user info",
                    |this, info, _| {
                        this.user = Some(info);
                    },
                    cx,
                );
            }
            Subject::Group(group) => {
                self.load(
                    session.group_profile(group),
                    "group info",
                    |this, info, _| {
                        this.members.clone_from(&info.members);
                        this.group = Some(info);
                    },
                    cx,
                );
            }
            Subject::Supergroup { id, is_channel } => {
                self.load(
                    session.supergroup_profile(id),
                    "supergroup info",
                    |this, info, _| {
                        this.group = Some(info);
                    },
                    cx,
                );
                if !is_channel {
                    self.load(
                        session.supergroup_members(id),
                        "members",
                        |this, members, _| {
                            this.members = members;
                        },
                        cx,
                    );
                }
            }
        }
        for media in SharedMedia::ALL {
            let task = session.shared_media_count(self.chat, media);
            self.load(
                task,
                "shared media count",
                move |this, count, _| {
                    this.set_count(media, count);
                },
                cx,
            );
        }
    }

    fn set_count(&mut self, media: SharedMedia, count: i32) {
        self.counts.retain(|(shown, _)| *shown != media);
        if count > 0 {
            self.counts.push((media, count));
        }
        let order = |media: &SharedMedia| SharedMedia::ALL.iter().position(|m| m == media);
        self.counts.sort_by_key(|(media, _)| order(media));
    }

    pub(super) fn load<T: Send + 'static>(
        &self,
        task: impl Future<Output = Result<T, RequestError>> + Send + 'static,
        what: &'static str,
        apply: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let task = crate::store::spawn(task, cx);
        cx.spawn(async move |this, cx| match task.await {
            Ok(value) => {
                let _ = this.update(cx, |this, cx| {
                    apply(this, value, cx);
                    cx.notify();
                });
            }
            Err(error) => tracing::warn!(%error, what, "profile request failed"),
        })
        .detach();
    }

    pub(super) fn download(&mut self, file: FileId, cx: &mut Context<Self>) {
        if !self.requested.insert(file) {
            return;
        }
        let task = self
            .store()
            .read(cx)
            .session()
            .download_file(file, USERPIC_PRIORITY);
        crate::peer_menu::send(async move { task.await.map(drop) }, "profile photo", cx);
    }

    pub(super) fn close(&self, window: &mut Window, cx: &mut App) {
        self.layers()
            .update(cx, |layers, cx| layers.close_all(window, cx));
    }

    /// Opens `chat` in the window and closes the profile.
    pub(super) fn open_chat(&self, chat: ChatId, window: &mut Window, cx: &mut App) {
        self.close(window, cx);
        self.targets
            .dialogs
            .update(cx, |dialogs, cx| dialogs.select(chat, cx));
    }

    fn title(&self) -> i18n::Key {
        match self.subject {
            Subject::User(_) => key::lng_info_user_title,
            Subject::Group(_)
            | Subject::Supergroup {
                is_channel: false, ..
            } => key::lng_info_group_title,
            Subject::Supergroup {
                is_channel: true, ..
            } => key::lng_info_channel_title,
        }
    }

    fn top_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let st = &INFO_LAYER_TOP_BAR;
        let title_font = st.title.style.font;
        let entity = cx.entity();
        div()
            .flex_none()
            .flex()
            .justify_between()
            .h(px_of(st.height))
            .child(
                div()
                    .pl(px_of(st.title_position.x))
                    .pt(px_of(st.title_position.y))
                    .font(font(title_font))
                    .text_size(px_of(title_font.size))
                    .line_height(font_height(title_font))
                    .text_color(color(cx, st.title.text_fg))
                    .child(tr(cx, self.title())),
            )
            .child(
                IconButton::new("info-close", &INFO_LAYER_TOP_BAR_CLOSE).on_click(
                    move |_, window, cx| entity.update(cx, |this, cx| this.close(window, cx)),
                ),
            )
    }
}

/// The gray band between sections (`Ui::AddDivider`).
pub(super) fn divider(cx: &App) -> impl IntoElement + use<> {
    div()
        .flex_none()
        .h(px_of(BOX_DIVIDER_HEIGHT))
        .bg(color(cx, ColorRef::Palette(ColorId::BoxDividerBg)))
}

/// One line of text in a style's font and color.
pub(super) fn text(
    value: impl Into<SharedString>,
    st: &'static style::widgets::FlatLabel,
    cx: &App,
) -> gpui::Div {
    let text_font = st.style.font;
    div()
        .font(font(text_font))
        .text_size(px_of(text_font.size))
        .line_height(font_height(text_font))
        .text_color(color(cx, st.text_fg))
        .child(value.into())
}

impl Render for InfoView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let max_height = window.viewport_size().height - crate::layers::info_top(window) * 2.0;
        let top_bar = self.top_bar(cx);
        let cover = self.cover(cx);
        let details = self.details(cx);
        let media = self.media_rows(cx);
        let members = self.members_section(cx);
        let actions = self.actions(cx);
        div()
            .track_focus(&self.focus)
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
                    .id("info-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(cover)
                    .children(details)
                    .children(media)
                    .children(members)
                    .children(actions),
            )
    }
}
