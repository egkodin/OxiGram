//! The ad after a channel's last post (`SponsoredMessages`, the sponsored
//! `HistoryView::WebPage`): an "Ad" box with the title, the text and the
//! button, and hide and menu buttons beside the bubble.

use std::rc::Rc;
use std::time::Duration;

use gpui::{
    Anchor, AnyElement, AppContext as _, Context, InteractiveElement as _, IntoElement,
    MouseButton, ParentElement, Pixels, Point, StatefulInteractiveElement as _, Styled, Window,
    div,
};
use i18n::key;
use model::RequestError;
use model::chat::ChatKind;
use model::message::LinkPreview;
use model::sponsored::{ReportResult, Sponsored};
use style::ColorId;
use style::chat::MSG_MARGIN;
use style::chat::{HISTORY_FAST_CLOSE_SIZE, HISTORY_FAST_SHARE_BOTTOM, HISTORY_FAST_SHARE_LEFT};
use style::chat_style::{HISTORY_FAST_CLOSE_ICON, HISTORY_FAST_MORE_ICON};
use style::menu_icons::{MENU_ICON_BLOCK, MENU_ICON_CANCEL, MENU_ICON_INFO};
use style::widgets::{DEFAULT_TOAST, POPUP_MENU_WITH_ICONS};
use ui::{Markup, MenuTone, PopupMenu, color};

use super::bubble::{self, Run};
use super::web_page::{self, AdParts, WebPage};
use super::{HistoryEvent, HistoryView, pinned_section};
use crate::boxes::{self, ConfirmArgs};
use crate::lang::{tr, tr_plural, tr_with};
use crate::px_of;
use crate::store::spawn;

/// The final report toasts stay this long (`kToastDuration`).
const REPORT_TOAST_DURATION: Duration = Duration::from_secs(4);
/// The "I don't like it" option, which reports without a toast; TDLib
/// sends option ids as base64 bytes ("1").
const DISLIKE_OPTION: &str = "MQ==";
/// The boost level that lets channels remove ads, when the app config
/// does not say (`channelRestrictSponsoredLevelMin`).
// shortcut: the default level, not `channel_restrict_sponsored_level_min` from the app config; read with M13.
const RESTRICT_ADS_LEVEL: i64 = 20;

/// The ad TDLib gave for the chat, and whether TDLib heard it was seen.
#[derive(Debug, Default)]
pub(super) struct Ad {
    sponsored: Option<Sponsored>,
    is_viewed: bool,
}

impl HistoryView {
    /// Asks for the channel's ads; only broadcast channels have them here.
    // shortcut: one ad after the last post; ads between posts (`messages_between`) and bots' top-bar ads are not shown.
    pub(super) fn request_ad(&mut self, cx: &mut Context<Self>) {
        let state = self.store.read(cx).state();
        let is_channel = state.chats.chat(self.chat).is_some_and(|chat| {
            matches!(
                chat.kind,
                ChatKind::Supergroup {
                    is_channel: true,
                    ..
                }
            )
        });
        if !is_channel || self.topic.is_some() {
            return;
        }
        let task = spawn(
            self.store.read(cx).session().sponsored_messages(self.chat),
            cx,
        );
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(list) => {
                    this.ad.sponsored = list.into_iter().next();
                    this.place_ad(cx);
                }
                Err(error) => tracing::warn!(%error, "loading ads failed"),
            });
        })
        .detach();
    }

    /// Adds the ad's item after the last message once the newest messages
    /// are loaded, or takes it away (`setBottomContentRequest`).
    pub(super) fn place_ad(&mut self, cx: &mut Context<Self>) {
        let has_newest = self
            .store
            .read(cx)
            .state()
            .history(self.key())
            .is_some_and(|history| history.has_newest());
        let wants = self.section == pinned_section::Section::Chat
            && self.ad.sponsored.is_some()
            && has_newest;
        let end = self.shown.len();
        let has = self.list.item_count() > end;
        match (wants, has) {
            (true, false) => self.list.splice(end..end, 1),
            (false, true) => self.list.splice(end..end + 1, 0),
            _ => return,
        }
        self.needs_preload_check = true;
        cx.notify();
    }

    fn drop_ad(&mut self, cx: &mut Context<Self>) {
        self.ad.sponsored = None;
        self.place_ad(cx);
    }

    /// Tells TDLib the ad was seen once all of it is in view.
    // shortcut: seen once its item is not below the view, tdesktop waits for the text's bottom (`markSponsoredViewed`).
    pub(super) fn view_ad(&mut self, cx: &mut Context<Self>) {
        let end = self.shown.len();
        let Some(ad) = self.ad.sponsored.as_ref().filter(|_| !self.ad.is_viewed) else {
            return;
        };
        if self.list.item_count() <= end || self.list.item_is_below_viewport(end) != Some(false) {
            return;
        }
        self.ad.is_viewed = true;
        let task = self.store.read(cx).session().view_message(self.chat, ad.id);
        spawn(
            async move {
                if let Err(error) = task.await {
                    tracing::warn!(%error, "marking an ad seen failed");
                }
            },
            cx,
        )
        .detach();
    }

    /// The ad's row: an incoming bubble and the buttons beside it.
    pub(super) fn ad_item(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(ad) = self.ad.sponsored.as_ref() else {
            return div().into_any_element();
        };
        let run = Run {
            is_attached_to_previous: false,
            is_attached_to_next: false,
        };
        let (frame, _) = bubble::frame(false, false, &run, bubble::Foot::Tail, cx);
        let weak = cx.weak_entity();
        let on_menu = move |_: &gpui::MouseDownEvent, window: &mut Window, cx: &mut gpui::App| {
            let _ = weak.update(cx, |this, cx| this.open_ad_menu(window, cx));
        };
        div()
            .flex()
            .items_start()
            .pt(px_of(MSG_MARGIN.top))
            .pb(px_of(MSG_MARGIN.bottom))
            .pl(px_of(MSG_MARGIN.left))
            .pr(px_of(MSG_MARGIN.right))
            .child(
                frame
                    .on_mouse_down(MouseButton::Right, on_menu)
                    .child(self.ad_box(ad, cx)),
            )
            .child(self.ad_buttons(ad.can_be_reported, cx))
            .into_any_element()
    }

    fn ad_box(&self, ad: &Sponsored, cx: &mut Context<Self>) -> AnyElement {
        let site = if ad.is_recommended {
            key::lng_recommended_message_title
        } else {
            key::lng_sponsored_message_title
        };
        let preview = LinkPreview {
            url: ad.url.clone(),
            site_name: tr(cx, site).to_string(),
            title: ad.title.clone(),
            description: ad.text.clone(),
            is_above_text: false,
            is_url_in_text: true,
            ..LinkPreview::default()
        };
        let tint = style::ColorRef::Palette(bubble::name_color(ad.accent_color));
        let (weak, hint_weak) = (cx.weak_entity(), cx.weak_entity());
        let id = ad.id;
        let on_link: super::body::OnLink = Rc::new(move |link, _, cx| {
            let _ = weak.update(cx, |this, cx| {
                let task = this.store.read(cx).session().click_sponsored(this.chat, id);
                spawn(
                    async move {
                        if let Err(error) = task.await {
                            tracing::warn!(%error, "reporting an ad click failed");
                        }
                    },
                    cx,
                )
                .detach();
                cx.emit(HistoryEvent::Link(link));
            });
        });
        web_page::element(
            WebPage {
                preview: &preview,
                id: ("ad", id.0.unsigned_abs()).into(),
                colors: [
                    tint,
                    tint,
                    style::ColorRef::Palette(ColorId::HistoryTextInFg),
                ],
                room: None,
                photo: None,
                on_link,
                ad: Some(AdParts {
                    // tdesktop offers it only for ads that can be reported.
                    hint: tr(cx, key::lng_sponsored_message_revenue_button),
                    on_hint: Rc::new(move |window, cx| {
                        let _ = hint_weak.update(cx, |this, cx| this.show_ad_about(window, cx));
                    }),
                    button: (!ad.button_text.is_empty()).then(|| ad.button_text.clone().into()),
                }),
            },
            cx,
        )
    }

    /// Hide on top and, for ads that can be reported, the menu under it,
    /// in one service-colored pill (`drawRightAction`).
    fn ad_buttons(&self, can_report: bool, cx: &mut Context<Self>) -> AnyElement {
        let size = px_of(HISTORY_FAST_CLOSE_SIZE);
        let (hide, more) = (cx.weak_entity(), cx.weak_entity());
        let button = |id: &'static str, icon| {
            div()
                .id(id)
                .size(size)
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .child(ui::icon(icon))
        };
        div()
            .flex_none()
            .flex()
            .flex_col()
            .ml(px_of(HISTORY_FAST_SHARE_LEFT))
            .mt(px_of(HISTORY_FAST_SHARE_BOTTOM))
            .rounded(size / 2.0)
            .bg(color(cx, style::ColorRef::Palette(ColorId::MsgServiceBg)))
            .child(
                button("ad-hide", HISTORY_FAST_CLOSE_ICON).on_click(move |_, window, cx| {
                    let _ = hide.update(cx, |this, cx| this.hide_ad(window, cx));
                }),
            )
            .children(can_report.then(|| {
                button("ad-more", HISTORY_FAST_MORE_ICON).on_click(move |_, window, cx| {
                    let _ = more.update(cx, |this, cx| this.open_ad_menu(window, cx));
                })
            }))
            .into_any_element()
    }

    /// Premium users drop the ad; others learn that Premium hides ads
    /// (`HideSponsoredClickHandler`).
    fn hide_ad(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.store.read(cx).state();
        let is_premium = state
            .me
            .and_then(|me| state.chats.user(me))
            .is_some_and(|me| me.is_premium);
        if is_premium {
            self.drop_ad(cx);
        } else {
            self.show_no_ads(window, cx);
        }
    }

    // shortcut: a box with tdesktop's "No Ads" text instead of the Premium preview box (M13).
    fn show_no_ads(&self, window: &mut Window, cx: &mut Context<Self>) {
        let feature = (
            key::lng_premium_summary_subtitle_no_ads,
            key::lng_premium_summary_about_no_ads,
        );
        self.show_premium_feature(feature, window, cx);
    }

    /// A Premium feature's title and what it gives, for a user without it.
    // shortcut: a plain box instead of the Premium preview with its video and subscribe button; with Premium (M13).
    pub(super) fn show_premium_feature(
        &self,
        (title, about): (i18n::Key, i18n::Key),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut args = ConfirmArgs::new(Markup::plain(tr(cx, about)));
        args.title = Some(tr(cx, title));
        boxes::confirm(&self.layers, args, window, cx);
    }

    /// "Advertiser info", "About These Ads", "Report Ad" and "Hide"
    /// (`FillSponsored`).
    // shortcut: the advertiser info is listed in the menu instead of a submenu that copies it.
    fn open_ad_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ad) = self.ad.sponsored.clone() else {
            return;
        };
        let (about, report, hide) = (
            tr(cx, key::lng_sponsored_menu_revenued_about),
            tr(cx, key::lng_sponsored_menu_revenued_report),
            tr(cx, key::lng_sponsored_hide_ads),
        );
        let weak = cx.weak_entity();
        let menu = cx.new(|cx| {
            let mut menu = PopupMenu::new(&POPUP_MENU_WITH_ICONS, cx);
            for info in &ad.info {
                menu.note(info.clone());
            }
            if !ad.info.is_empty() {
                menu.separator();
            }
            if ad.can_be_reported {
                let on_about = weak.clone();
                menu.action(
                    about,
                    MENU_ICON_INFO,
                    MenuTone::Normal,
                    move |window, cx| {
                        let _ = on_about.update(cx, |this, cx| this.show_ad_about(window, cx));
                    },
                );
                let on_report = weak.clone();
                menu.action(
                    report,
                    MENU_ICON_BLOCK,
                    MenuTone::Normal,
                    move |window, cx| {
                        let _ = on_report.update(cx, |this, cx| {
                            this.report_ad(String::new(), window, cx);
                        });
                    },
                );
                menu.separator();
            }
            menu.action(
                hide,
                MENU_ICON_CANCEL,
                MenuTone::Normal,
                move |window, cx| {
                    let _ = weak.update(cx, |this, cx| this.hide_ad(window, cx));
                },
            );
            menu
        });
        self.present_menu(menu, window.mouse_position(), Anchor::TopLeft, window, cx);
    }

    /// What channel ads are (`AboutBox`).
    // shortcut: tdesktop's icons and the links to Premium and the ad platform are left out; the text is the same.
    fn show_ad_about(&self, window: &mut Window, cx: &mut Context<Self>) {
        let premium = tr(cx, key::lng_settings_privacy_premium_link);
        let level = RESTRICT_ADS_LEVEL.to_string();
        let learn = tr_with(cx, key::lng_channel_earn_about_link, &[("emoji", "")]);
        let text = format!(
            "{}\n\n**{}**\n{}\n\n**{}**\n{}\n\n**{}**\n{}\n\n**{}**\n{}",
            tr(cx, key::lng_sponsored_revenued_subtitle),
            tr(cx, key::lng_sponsored_revenued_info1_title),
            tr(cx, key::lng_sponsored_revenued_info1_description),
            tr(cx, key::lng_sponsored_revenued_info2_title),
            tr(cx, key::lng_sponsored_revenued_info2_description),
            tr(cx, key::lng_sponsored_revenued_info3_title),
            tr_plural(
                cx,
                key::lng_sponsored_revenued_info3_description,
                RESTRICT_ADS_LEVEL,
                &[("count", &level), ("link", &premium)],
            ),
            tr(cx, key::lng_sponsored_revenued_footer_title),
            tr_with(
                cx,
                key::lng_sponsored_revenued_footer_description,
                &[("link", learn.trim_end())],
            ),
        );
        let mut args = ConfirmArgs::new(Markup::rich(&text));
        args.title = Some(tr(cx, key::lng_sponsored_menu_revenued_about));
        boxes::confirm(&self.layers, args, window, cx);
    }

    /// Reports the ad; empty `option` first, then the one the user chose
    /// (`ShowReportSponsoredBox`).
    fn report_ad(&mut self, option: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.ad.sponsored.as_ref().map(|ad| ad.id) else {
            return;
        };
        let session = self.store.read(cx).session();
        let task = spawn(session.report_sponsored(self.chat, id, option.clone()), cx);
        let position = window.mouse_position();
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                this.on_ad_reported(&option, result, position, window, cx);
            })
        })
        .detach();
    }

    fn on_ad_reported(
        &mut self,
        option: &str,
        result: Result<ReportResult, RequestError>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let toast = match result {
            Ok(ReportResult::OptionRequired { title, options }) => {
                return self.show_report_options(title, options, position, window, cx);
            }
            Ok(ReportResult::Reported) => {
                self.drop_ad(cx);
                let link = tr(cx, key::lng_report_sponsored_reported_link);
                (option != DISLIKE_OPTION)
                    .then(|| tr_with(cx, key::lng_report_sponsored_reported, &[("link", &link)]))
            }
            Ok(ReportResult::AdsHidden) => {
                self.drop_ad(cx);
                Some(tr(cx, key::lng_report_sponsored_hidden).to_string())
            }
            Ok(ReportResult::PremiumRequired) => {
                return self.show_no_ads(window, cx);
            }
            // tdesktop says nothing when the ad is gone.
            Ok(ReportResult::Failed) => None,
            Err(error) => {
                tracing::warn!(%error, "reporting an ad failed");
                Some(tr(cx, key::lng_attach_failed).to_string())
            }
        };
        if let Some(text) = toast {
            self.layers.update(cx, |layers, cx| {
                layers.show_toast(text.into(), &DEFAULT_TOAST, REPORT_TOAST_DURATION, cx);
            });
        }
    }

    // shortcut: the options open as a menu under their title, not tdesktop's box with Back and Close.
    fn show_report_options(
        &mut self,
        title: String,
        options: Vec<model::sponsored::ReportOption>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let weak = cx.weak_entity();
        let menu = cx.new(|cx| {
            let mut menu = PopupMenu::new(&POPUP_MENU_WITH_ICONS, cx);
            menu.note(title);
            menu.separator();
            for option in options {
                let weak = weak.clone();
                menu.option(option.text, None, move |window, cx| {
                    let id = option.id.clone();
                    let _ = weak.update(cx, |this, cx| this.report_ad(id, window, cx));
                });
            }
            menu
        });
        self.present_menu(menu, position, Anchor::TopLeft, window, cx);
    }
}
