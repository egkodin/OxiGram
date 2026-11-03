//! "New Group" and "New Channel" from the main menu: the name, photo and
//! description (`GroupInfoBox`), a channel's public link or invite link
//! (`SetupChannelBox`), then the members (`AddParticipantsBoxController`);
//! the new chat opens at the end.

use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, ParentElement, PathPromptOptions, Render, SharedString,
    StatefulInteractiveElement as _, Styled, Subscription, Window, div,
};
use i18n::key;
use model::create::CreatedChannel;
use model::{ChatId, RequestError, UserId};
use style::boxes::INFO_LABEL;
use style::layers::{DEFAULT_BOX, DEFAULT_BOX_CHECKBOX};
use style::widgets::{DEFAULT_INPUT_FIELD, DEFAULT_RADIO};
use ui::{
    BoxFrame, Checkbox, Checked, InputEvent, InputField, RoundButton, color, font, font_height,
};

use crate::contacts_box::{self, Cancel, Many};
use crate::intro::signup::photo_button;
use crate::lang::tr;
use crate::layers::{Dismiss, Layers};
use crate::links::Targets;
use crate::px_of;

/// TDLib's limit for a chat title (`kMaxGroupChannelTitle`).
const MAX_TITLE: usize = 128;
/// `kMaxChannelDescription`.
const MAX_DESCRIPTION: usize = 255;
/// `Ui::kMaxUsernameLength`.
const MAX_USERNAME: usize = 32;
const SKIP: style::Px = style::Px(12);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Group,
    Channel,
}

struct InfoBox {
    kind: Kind,
    name: Entity<InputField>,
    description: Option<Entity<InputField>>,
    photo: Option<PathBuf>,
    targets: Targets,
    focus: FocusHandle,
    _submit: Subscription,
}

impl Focusable for InfoBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

pub(crate) fn show(kind: Kind, targets: Targets, window: &mut Window, cx: &mut App) {
    let layers = targets.layers.clone();
    let view = cx.new(|cx| {
        let placeholder = match kind {
            Kind::Group => key::lng_dlg_new_group_name,
            Kind::Channel => key::lng_dlg_new_channel_name,
        };
        let field = |placeholder, max, window: &mut Window, cx: &mut Context<InfoBox>| {
            cx.new(|cx| {
                InputField::new(&DEFAULT_INPUT_FIELD, tr(cx, placeholder), window, cx)
                    .with_max_length(max)
            })
        };
        let name = field(placeholder, MAX_TITLE, window, cx);
        // Enter in the name goes on, as Next does.
        let submit = cx.subscribe_in(&name, window, |this, _, event, window, cx| {
            if *event == InputEvent::Submitted {
                this.submit(window, cx);
            }
        });
        InfoBox {
            kind,
            name,
            description: (kind == Kind::Channel).then(|| {
                field(
                    key::lng_create_group_description,
                    MAX_DESCRIPTION,
                    window,
                    cx,
                )
            }),
            photo: None,
            targets,
            focus: cx.focus_handle(),
            _submit: submit,
        }
    });
    let name = view.read(cx).name.clone();
    layers.update(cx, |layers, cx| {
        layers.show(view, Dismiss::Allowed, None, window, cx)
    });
    name.update(cx, |field, cx| field.focus(window, cx));
}

/// Runs `task`, then `next` with its result in the window; failures go to
/// the log and `failed`.
fn then<T: Send + 'static>(
    task: impl std::future::Future<Output = Result<T, RequestError>> + Send + 'static,
    what: &'static str,
    next: impl FnOnce(T, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    let task = crate::store::spawn(task, cx);
    window
        .spawn(cx, async move |cx| match task.await {
            Ok(value) => {
                let _ = cx.update(|window, cx| next(value, window, cx));
            }
            Err(error) => tracing::warn!(%error, what, "creating a chat failed"),
        })
        .detach();
}

fn open_chat(targets: &Targets, chat: ChatId, window: &mut Window, cx: &mut App) {
    targets
        .layers
        .update(cx, |layers, cx| layers.close_all(window, cx));
    targets
        .dialogs
        .update(cx, |dialogs, cx| dialogs.select(chat, cx));
}

impl InfoBox {
    fn choose_photo(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr(cx, key::lng_choose_image)),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(mut paths))) = paths.await else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                this.photo = paths.pop();
                cx.notify();
            });
        })
        .detach();
    }

    /// Next for a group, Create for a channel.
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = self.name.read(cx).text().trim().to_owned();
        if title.is_empty() {
            self.name.update(cx, |field, cx| field.show_error(cx));
            return;
        }
        let (targets, photo) = (self.targets.clone(), self.photo.clone());
        match self.kind {
            Kind::Group => pick_members(
                targets,
                key::lng_create_group_create,
                Cancel::Close,
                move |targets, members, window, cx| {
                    let task = targets
                        .store
                        .read(cx)
                        .session()
                        .create_group(title.clone(), &members);
                    let photo = photo.clone();
                    then(
                        task,
                        "create group",
                        move |chat, window, cx| {
                            set_photo(&targets, chat, photo, cx);
                            open_chat(&targets, chat, window, cx);
                        },
                        window,
                        cx,
                    );
                },
                window,
                cx,
            ),
            Kind::Channel => {
                let description = self
                    .description
                    .as_ref()
                    .map(|field| field.read(cx).text().trim().to_owned())
                    .unwrap_or_default();
                let task = targets
                    .store
                    .read(cx)
                    .session()
                    .create_channel(title, description);
                then(
                    task,
                    "create channel",
                    move |created: CreatedChannel, window, cx| {
                        set_photo(&targets, created.chat, photo, cx);
                        setup_channel(targets, created, window, cx);
                    },
                    window,
                    cx,
                );
            }
        }
    }
}

fn set_photo(targets: &Targets, chat: ChatId, photo: Option<PathBuf>, cx: &mut App) {
    let Some(path) = photo else {
        return;
    };
    let task = targets
        .store
        .read(cx)
        .session()
        .set_chat_photo(chat, &path.to_string_lossy());
    crate::peer_menu::send(task, "chat photo", cx);
}

/// The contacts box picking members; `done` gets them.
fn pick_members(
    targets: Targets,
    done_label: i18n::Key,
    cancel: Cancel,
    done: impl Fn(Targets, Vec<UserId>, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    let many = Many {
        title: tr(cx, key::lng_channel_add_users),
        done: tr(cx, done_label),
        on_done: {
            let targets = targets.clone();
            Rc::new(move |members, window, cx| done(targets.clone(), members, window, cx))
        },
        cancel,
    };
    contacts_box::show_many(&targets.store, &targets.layers, many, window, cx);
}

impl Render for InfoBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let photo = photo_button("create-photo", self.photo.clone(), cx)
            .flex_none()
            .on_click(move |_, _, cx| entity.update(cx, |this, cx| this.choose_photo(cx)));
        let pad = px_of(style::layers::BOX_PADDING.left);
        let head = div()
            .px(pad)
            .flex()
            .items_center()
            .gap(px_of(SKIP))
            .child(photo)
            .child(div().flex_1().child(self.name.clone()));
        let description = self
            .description
            .clone()
            .map(|field| div().px(pad).pt(px_of(SKIP)).child(field));
        let (title, button) = match self.kind {
            Kind::Group => (key::lng_create_group_title, key::lng_create_group_next),
            Kind::Channel => (key::lng_create_channel_title, key::lng_create_group_create),
        };
        let (entity, layers) = (cx.entity(), self.targets.layers.clone());
        div().track_focus(&self.focus).child(
            BoxFrame::new("create-info", &DEFAULT_BOX)
                .title(tr(cx, title))
                .row(div().child(head).children(description))
                .button(
                    RoundButton::new("create-next", &DEFAULT_BOX.button, tr(cx, button)).on_click(
                        move |_, window, cx| entity.update(cx, |this, cx| this.submit(window, cx)),
                    ),
                )
                .button(cancel(layers, cx)),
        )
    }
}

fn cancel(layers: Entity<Layers>, cx: &App) -> RoundButton {
    RoundButton::new(
        "create-cancel",
        &DEFAULT_BOX.button,
        tr(cx, key::lng_cancel),
    )
    .on_click(move |_, window, cx| layers.update(cx, |layers, cx| layers.close(window, cx)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Privacy {
    Public,
    Private,
}

/// `SetupChannelBox`: public with a t.me link, or private with the
/// invite link.
struct SetupBox {
    created: CreatedChannel,
    privacy: Privacy,
    username: Entity<InputField>,
    invite_link: String,
    targets: Targets,
    focus: FocusHandle,
}

impl Focusable for SetupBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

fn setup_channel(targets: Targets, created: CreatedChannel, window: &mut Window, cx: &mut App) {
    let layers = targets.layers.clone();
    let session = targets.store.read(cx).session().clone();
    let view = cx.new(|cx| SetupBox {
        created,
        privacy: Privacy::Private,
        username: cx.new(|cx| {
            InputField::new(
                &DEFAULT_INPUT_FIELD,
                tr(cx, key::lng_create_group_link),
                window,
                cx,
            )
            .with_max_length(MAX_USERNAME)
        }),
        invite_link: String::new(),
        targets,
        focus: cx.focus_handle(),
    });
    let weak = view.downgrade();
    then(
        session.invite_link(created.supergroup),
        "invite link",
        move |link, _, cx| {
            let _ = weak.update(cx, |this, cx| {
                this.invite_link = link;
                cx.notify();
            });
        },
        window,
        cx,
    );
    layers.update(cx, |layers, cx| {
        layers.show(view, Dismiss::Allowed, None, window, cx)
    });
}

impl SetupBox {
    /// Saves the link, then picks the subscribers.
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (targets, created) = (self.targets.clone(), self.created);
        let next = move |(), window: &mut Window, cx: &mut App| {
            add_subscribers(targets, created, window, cx)
        };
        if self.privacy == Privacy::Private {
            next((), window, cx);
            return;
        }
        let username = self
            .username
            .read(cx)
            .text()
            .trim()
            .trim_start_matches('@')
            .to_owned();
        if username.is_empty() {
            self.username.update(cx, |field, cx| field.show_error(cx));
            return;
        }
        let task = crate::store::spawn(
            self.targets
                .store
                .read(cx)
                .session()
                .set_channel_username(created.supergroup, username),
            cx,
        );
        let field = self.username.clone();
        cx.spawn_in(window, async move |_, cx| match task.await {
            Ok(()) => {
                let _ = cx.update(|window, cx| next((), window, cx));
            }
            Err(error) => {
                // shortcut: any refusal marks the field; tdesktop says why ("occupied", "too short") as it is typed (`checkUsernameAvailability`).
                tracing::warn!(%error, "setting the channel link failed");
                field.update(cx, |field, cx| field.show_error(cx));
            }
        })
        .detach();
    }

    fn choice(&self, privacy: Privacy, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (title, about, id) = match privacy {
            Privacy::Public => (
                key::lng_create_public_channel_title,
                key::lng_create_public_channel_about,
                "create-public",
            ),
            Privacy::Private => (
                key::lng_create_private_channel_title,
                key::lng_create_private_channel_about,
                "create-private",
            ),
        };
        let checked = if self.privacy == privacy {
            Checked::Yes
        } else {
            Checked::No
        };
        let entity = cx.entity();
        let pad = px_of(style::layers::BOX_PADDING.left);
        div()
            .px(pad)
            .pt(px_of(SKIP))
            .child(
                Checkbox::radio(
                    id,
                    &DEFAULT_BOX_CHECKBOX,
                    &DEFAULT_RADIO,
                    tr(cx, title),
                    checked,
                )
                .on_toggle(move |_, cx| {
                    entity.update(cx, |this, cx| {
                        this.privacy = privacy;
                        cx.notify();
                    });
                }),
            )
            .child(label(tr(cx, about), cx).pl(px_of(DEFAULT_BOX_CHECKBOX.text_position.x)))
    }
}

fn label(text: SharedString, cx: &App) -> gpui::Div {
    let st = &INFO_LABEL;
    div()
        .font(font(st.style.font))
        .text_size(px_of(st.style.font.size))
        .line_height(font_height(st.style.font))
        .text_color(color(cx, st.text_fg))
        .child(text)
}

fn add_subscribers(targets: Targets, created: CreatedChannel, window: &mut Window, cx: &mut App) {
    pick_members(
        targets,
        key::lng_participant_invite,
        Cancel::Skip,
        move |targets, members, window, cx| {
            if members.is_empty() {
                open_chat(&targets, created.chat, window, cx);
                return;
            }
            let task = targets
                .store
                .read(cx)
                .session()
                .add_members(created.chat, &members);
            then(
                task,
                "add subscribers",
                move |(), window, cx| open_chat(&targets, created.chat, window, cx),
                window,
                cx,
            );
        },
        window,
        cx,
    );
}

impl Render for SetupBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pad = px_of(style::layers::BOX_PADDING.left);
        let link = match self.privacy {
            Privacy::Public => div()
                .px(pad)
                .pt(px_of(SKIP))
                .flex()
                .items_center()
                .child(label("t.me/".into(), cx))
                .child(div().flex_1().child(self.username.clone())),
            Privacy::Private => div()
                .px(pad)
                .pt(px_of(SKIP))
                .child(label(self.invite_link.clone().into(), cx)),
        };
        let public = self.choice(Privacy::Public, cx);
        let private = self.choice(Privacy::Private, cx);
        let entity = cx.entity();
        let skip = {
            let (targets, created) = (self.targets.clone(), self.created);
            RoundButton::new(
                "create-skip",
                &DEFAULT_BOX.button,
                tr(cx, key::lng_create_group_skip),
            )
            .on_click(move |_, window, cx| add_subscribers(targets.clone(), created, window, cx))
        };
        div().track_focus(&self.focus).child(
            BoxFrame::new("create-setup", &DEFAULT_BOX)
                .title(tr(cx, key::lng_create_channel_title))
                .row(
                    div()
                        .id("create-setup-rows")
                        .child(public)
                        .child(private)
                        .child(link),
                )
                .button(
                    RoundButton::new(
                        "create-save",
                        &DEFAULT_BOX.button,
                        tr(cx, key::lng_settings_save),
                    )
                    .on_click(move |_, window, cx| {
                        entity.update(cx, |this, cx| this.save(window, cx))
                    }),
                )
                .button(skip),
        )
    }
}
