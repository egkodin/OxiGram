//! One's own details edited from the profile (`Settings::Information`):
//! the name (`EditNameBox`), bio, username (`UsernamesBox`), birthday and
//! photo.

use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, ParentElement, PathPromptOptions, Render, SharedString, Styled, WeakEntity,
    Window, div,
};
use i18n::key;
use model::profile::Birthday;
use model::{RequestError, Session};
use style::boxes::INFO_LABEL;
use style::layers::DEFAULT_BOX;
use style::widgets::DEFAULT_INPUT_FIELD;
use ui::{BoxFrame, InputField, RoundButton};

use super::{InfoView, text};
use crate::Store;
use crate::lang::tr;
use crate::layers::Layers;
use crate::px_of;

/// `kMaxBioLength` without Premium.
// shortcut: 70 characters for everyone; Premium's 140 comes with the Premium checks (M13).
const MAX_BIO: usize = 70;
/// tdesktop's `kMaxNameLength`... TDLib's limit for a first or last name.
const MAX_NAME: usize = 64;
const FIELD_SKIP: style::Px = style::Px(12);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Field {
    Name,
    Bio,
    Username,
    Birthday,
}

/// Runs after a change is saved.
pub(crate) type Saved = Rc<dyn Fn(&mut App)>;

type Save = Rc<
    dyn Fn(
        &Session,
        &[String],
    ) -> Option<Pin<Box<dyn Future<Output = Result<(), RequestError>> + Send>>>,
>;

/// A box of text fields whose Save sends one request.
struct EditBox {
    title: SharedString,
    about: Option<SharedString>,
    fields: Vec<Entity<InputField>>,
    session: Session,
    save: Save,
    /// Runs once saved, as the profile reloads.
    on_saved: Saved,
    layers: WeakEntity<Layers>,
    focus: FocusHandle,
}

impl Focusable for EditBox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EditBox {
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let values: Vec<String> = self
            .fields
            .iter()
            .map(|field| field.read(cx).text().trim().to_owned())
            .collect();
        let Some(task) = (self.save)(&self.session, &values) else {
            if let Some(first) = self.fields.first() {
                first.update(cx, |field, cx| field.show_error(cx));
            }
            return;
        };
        let task = crate::store::spawn(task, cx);
        let first = self.fields.first().cloned();
        cx.spawn_in(window, async move |this, cx| match task.await {
            Ok(()) => {
                let _ = this.update_in(cx, |this, window, cx| {
                    (this.on_saved)(cx);
                    if let Some(layers) = this.layers.upgrade() {
                        layers.update(cx, |layers, cx| layers.close(window, cx));
                    }
                });
            }
            Err(error) => {
                tracing::warn!(%error, "saving the profile failed");
                if let Some(first) = first {
                    first.update(cx, |field, cx| field.show_error(cx));
                }
            }
        })
        .detach();
    }
}

impl Render for EditBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let fields = div()
            .flex()
            .flex_col()
            .gap(px_of(FIELD_SKIP))
            .children(self.fields.iter().cloned());
        let about = self.about.clone().map(|about| {
            div()
                .pt(px_of(FIELD_SKIP))
                .child(text(about, &INFO_LABEL, cx).whitespace_normal())
        });
        let entity = cx.entity();
        let layers = self.layers.clone();
        div().track_focus(&self.focus).child(
            BoxFrame::new("info-edit-box", &DEFAULT_BOX)
                .title(self.title.clone())
                .row(
                    div()
                        .px(px_of(style::layers::BOX_PADDING.left))
                        .child(fields)
                        .children(about),
                )
                .button(
                    RoundButton::new(
                        "info-edit-save",
                        &DEFAULT_BOX.button,
                        tr(cx, key::lng_settings_save),
                    )
                    .on_click(move |_, window, cx| {
                        entity.update(cx, |this, cx| this.submit(window, cx));
                    }),
                )
                .button(
                    RoundButton::new(
                        "info-edit-cancel",
                        &DEFAULT_BOX.button,
                        tr(cx, key::lng_cancel),
                    )
                    .on_click(move |_, window, cx| {
                        if let Some(layers) = layers.upgrade() {
                            layers.update(cx, |layers, cx| layers.close(window, cx));
                        }
                    }),
                ),
        )
    }
}

/// "3.11.1990" or "3.11"; None when it is not a date.
pub(super) fn parse_birthday(value: &str) -> Option<Birthday> {
    let parts: Vec<i32> = value
        .split(['.', '/', '-'])
        .map(|part| part.trim().parse().ok())
        .collect::<Option<_>>()?;
    let (day, month, year) = match parts[..] {
        [day, month] => (day, month, None),
        [day, month, year] if (1900..=9999).contains(&year) => (day, month, Some(year)),
        _ => return None,
    };
    ((1..=31).contains(&day) && (1..=12).contains(&month)).then_some(Birthday { day, month, year })
}

fn birthday_value(birthday: Option<Birthday>) -> String {
    match birthday {
        Some(Birthday {
            day,
            month,
            year: Some(year),
        }) => format!("{day}.{month}.{year}"),
        Some(Birthday {
            day,
            month,
            year: None,
        }) => format!("{day}.{month}"),
        None => String::new(),
    }
}

/// One's own details the boxes start from.
#[derive(Debug, Clone, Default)]
pub(crate) struct OwnDetails {
    pub bio: String,
    pub birthday: Option<Birthday>,
}

/// Opens the box for one's own `field` over the open layer; `on_saved`
/// runs once Telegram has it.
pub(crate) fn edit_own(
    field: Field,
    store: &Entity<Store>,
    layers: &Entity<Layers>,
    details: OwnDetails,
    on_saved: Saved,
    window: &mut Window,
    cx: &mut App,
) {
    let state = store.read(cx).state();
    let Some(user) = state.me.and_then(|me| state.chats.user(me)) else {
        return;
    };
    if field == Field::Username {
        crate::settings_screen::show_username_box(store, layers, on_saved, window, cx);
        return;
    }
    let OwnDetails { bio, birthday } = details;
    // (title, about, fields as placeholder, value and length, save)
    let (title, about, specs, save): (_, _, Vec<(SharedString, String, usize)>, Save) = match field
    {
        Field::Name => (
            tr(cx, key::lng_edit_self_title),
            None,
            vec![
                (
                    tr(cx, key::lng_signup_firstname),
                    user.first_name.clone(),
                    MAX_NAME,
                ),
                (
                    tr(cx, key::lng_signup_lastname),
                    user.last_name.clone(),
                    MAX_NAME,
                ),
            ],
            Rc::new(|session: &Session, values: &[String]| {
                let first = values.first().filter(|first| !first.is_empty())?.clone();
                let last = values.get(1).cloned().unwrap_or_default();
                Some(Box::pin(session.set_name(first, last)) as Pin<Box<_>>)
            }),
        ),
        Field::Bio => (
            tr(cx, key::lng_bio_placeholder),
            Some(tr(cx, key::lng_settings_about_bio)),
            vec![(tr(cx, key::lng_bio_placeholder), bio, MAX_BIO)],
            Rc::new(|session: &Session, values: &[String]| {
                Some(Box::pin(session.set_bio(values.first()?.clone())) as Pin<Box<_>>)
            }),
        ),
        Field::Username | Field::Birthday => (
            tr(cx, key::lng_settings_birthday_label),
            None,
            vec![(
                SharedString::from("31.12.1990"),
                birthday_value(birthday),
                10,
            )],
            Rc::new(|session: &Session, values: &[String]| {
                let value = values.first()?;
                // An empty field removes the birthday.
                let birthday = if value.is_empty() {
                    None
                } else {
                    Some(parse_birthday(value)?)
                };
                Some(Box::pin(session.set_birthday(birthday)) as Pin<Box<_>>)
            }),
        ),
    };
    // shortcut: the birthday is typed as day.month.year instead of tdesktop's wheels (`EditBirthdayBox`); add the picker with the calendar's date wheels.
    let session = store.read(cx).session().clone();
    let weak_layers = layers.downgrade();
    let view = cx.new(|cx| {
        let fields: Vec<Entity<InputField>> = specs
            .into_iter()
            .map(|(placeholder, value, max_length)| {
                cx.new(|cx| {
                    let mut field = InputField::new(&DEFAULT_INPUT_FIELD, placeholder, window, cx)
                        .with_max_length(max_length);
                    field.set_text(&value, None, cx);
                    field
                })
            })
            .collect();
        EditBox {
            title,
            about,
            fields,
            session,
            save,
            on_saved,
            layers: weak_layers,
            focus: cx.focus_handle(),
        }
    });
    let first = view.read(cx).fields.first().cloned();
    layers.update(cx, |layers, cx| layers.show_above(view, window, cx));
    if let Some(first) = first {
        first.update(cx, |field, cx| field.focus(window, cx));
    }
}

impl InfoView {
    /// Opens the box for `field` over the profile.
    pub(super) fn edit(&mut self, field: Field, window: &mut Window, cx: &mut Context<Self>) {
        let details = OwnDetails {
            bio: self
                .user
                .as_ref()
                .map(|p| p.bio.text.clone())
                .unwrap_or_default(),
            birthday: self.user.as_ref().and_then(|p| p.birthday),
        };
        let profile = cx.weak_entity();
        let on_saved: Saved = Rc::new(move |cx| {
            let _ = profile.update(cx, |profile, cx| profile.reload(cx));
        });
        let (store, layers) = (self.store().clone(), self.layers().clone());
        edit_own(field, &store, &layers, details, on_saved, window, cx);
    }

    /// "Set Photo": picks an image and makes it the profile photo.
    pub(super) fn pick_photo(&mut self, cx: &mut Context<Self>) {
        let profile = cx.weak_entity();
        let on_done: Saved = Rc::new(move |cx| {
            let _ = profile.update(cx, |profile, cx| {
                profile.requested.clear();
                profile.reload(cx);
            });
        });
        pick_own_photo(self.store(), on_done, cx);
    }
}

/// Picks an image and makes it one's profile photo; `on_done` runs once
/// it is set.
// shortcut: no crop editor (`Editor::PrepareProfilePhoto`); the photo goes as it is, TDLib crops it square.
pub(crate) fn pick_own_photo(store: &Entity<Store>, on_done: Saved, cx: &mut App) {
    let paths = cx.prompt_for_paths(PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: Some(tr(cx, key::lng_choose_image)),
    });
    let store = store.clone();
    cx.spawn(async move |cx| {
        let Ok(Ok(Some(mut paths))) = paths.await else {
            return;
        };
        let Some(path) = paths.pop() else {
            return;
        };
        let task = cx.update(|cx| {
            let request = store
                .read(cx)
                .session()
                .set_profile_photo(&path.to_string_lossy());
            crate::store::spawn(request, cx)
        });
        match task.await {
            Ok(()) => {
                cx.update(|cx| on_done(cx));
            }
            Err(error) => tracing::warn!(%error, "setting the profile photo failed"),
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn birthdays_parse_with_or_without_the_year() {
        let day = |day, month, year| Some(Birthday { day, month, year });
        assert_eq!(parse_birthday("3.11.1990"), day(3, 11, Some(1990)));
        assert_eq!(parse_birthday("3/11"), day(3, 11, None));
        assert_eq!(parse_birthday("32.11"), None);
        assert_eq!(parse_birthday("3.13.1990"), None);
        assert_eq!(parse_birthday("3.11.90"), None);
        assert_eq!(parse_birthday("tomorrow"), None);
        assert_eq!(birthday_value(day(3, 11, None)), "3.11");
    }
}
