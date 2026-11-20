//! "My Stars" (`Settings::Credits`): the cover, the top-up button, the
//! balance and the transaction history by direction.
// shortcut: no userpics or gift stickers on history rows, no entry box on click, no subscriptions list, no "Gift Stars to Friends", no paging past the first 50; with M13.

use gpui::{
    App, Context, InteractiveElement as _, IntoElement, ParentElement, Render,
    StatefulInteractiveElement as _, Styled, Task, Window, div, px,
};
use i18n::key;
use model::stars::{Direction, History, Party, Transaction};
use style::basic::{NORMAL_FONT, SEMIBOLD_FONT};
use style::credits::{
    CREDITS_HISTORY_TABS_SLIDER, CREDITS_HISTORY_TABS_SLIDER_PADDING, CREDITS_PREMIUM_COVER,
    CREDITS_SETTINGS_BIG_BALANCE_BUTTON,
};
use style::{ColorId, ColorRef, Font, Icon, IconMask, Margins, MonoIcon};
use ui::{FlatLabel, Markup, RoundButton, color, font, font_height};

use super::{Env, rows, stars_box};
use crate::lang::{group_digits, tr, tr_with};
use crate::px_of;

/// The cover's star (`Ui::Premium::ColorizedSvg` in the Stars colors).
const COVER_STAR: Icon = Icon::new(&[MonoIcon::new(
    IconMask::File {
        name: "settings/star",
        modifiers: &[],
    },
    ColorRef::Palette(ColorId::CreditsBg1),
    Margins::new(0, 0, 0, 0),
)]);
const STAR: char = '\u{2B50}';
const MINUS: char = '\u{2212}';
const TITLE_FG: ColorRef = ColorRef::Palette(ColorId::BoxTitleFg);
const NAME_FG: ColorRef = ColorRef::Palette(ColorId::WindowBoldFg);
const DATE_FG: ColorRef = ColorRef::Palette(ColorId::WindowSubTextFg);
/// `boxTextFgGood` and `menuIconAttentionColor` (`attentionButtonFg`).
const INCOMING_FG: ColorRef = ColorRef::Palette(ColorId::BoxTextFgGood);
const OUTGOING_FG: ColorRef = ColorRef::Palette(ColorId::AttentionButtonFg);
/// `creditsHistoryRightSkip` and a two-line row's height.
const ROW_RIGHT: f32 = 10.0;
const ROW_HEIGHT: f32 = 56.0;
const ROW_LEFT: f32 = 22.0;
const TABS: [(Direction, i18n::Key); 3] = [
    (Direction::All, key::lng_credits_summary_history_tab_full),
    (Direction::Incoming, key::lng_credits_summary_history_tab_in),
    (
        Direction::Outgoing,
        key::lng_credits_summary_history_tab_out,
    ),
];

/// "+100" or "−100" (`creditsHistoryRightStyle`).
fn amount_text(stars: i64) -> String {
    let sign = if stars < 0 { MINUS } else { '+' };
    format!(
        "{sign}{}",
        crate::lang::group_unsigned(stars.unsigned_abs(), Some(','))
    )
}

pub(super) struct StarsPage {
    env: Env,
    direction: Direction,
    /// None while TDLib is asked.
    history: Option<History>,
    _load: Task<()>,
}

impl StarsPage {
    pub(super) fn new(env: Env, cx: &mut Context<Self>) -> Self {
        let mut page = Self {
            env,
            direction: Direction::All,
            history: None,
            _load: Task::ready(()),
        };
        page.load(cx);
        page
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let Some(me) = self.env.targets.store.read(cx).state().me else {
            return;
        };
        let request = self
            .env
            .targets
            .store
            .read(cx)
            .session()
            .star_history(me, self.direction);
        let task = crate::store::spawn(request, cx);
        self._load = cx.spawn(async move |this, cx| match task.await {
            Ok(history) => {
                let _ = this.update(cx, |this, cx| {
                    this.history = Some(history);
                    cx.notify();
                });
            }
            Err(error) => tracing::warn!(%error, "reading the Stars history failed"),
        });
    }

    fn choose(&mut self, direction: Direction, cx: &mut Context<Self>) {
        if direction != self.direction {
            self.direction = direction;
            self.load(cx);
            cx.notify();
        }
    }

    fn cover(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let st = &CREDITS_PREMIUM_COVER;
        let title_font = st.title_font;
        let env = self.env.clone();
        let top_up = tr_with(cx, key::lng_credits_topup_button, &[("emoji", "+")]);
        let balance = self.history.as_ref().map_or(0, |history| history.balance);
        let balance_text = tr_with(
            cx,
            key::lng_credits_balance_me_count,
            &[
                ("emoji", &STAR.to_string()),
                ("amount", &format!("**{}**", group_digits(balance))),
            ],
        );
        div()
            .flex()
            .flex_col()
            .items_center()
            .child(div().h(px_of(st.star_top_skip)))
            .child(ui::icon(COVER_STAR).side(px_of(st.star_size.width)))
            .child(
                div()
                    .pt(px_of(st.title_padding.top))
                    .pb(px_of(st.title_padding.bottom))
                    .font(font(title_font))
                    .text_size(px_of(title_font.size))
                    .line_height(font_height(title_font))
                    .text_color(color(cx, TITLE_FG))
                    .child(tr(cx, key::lng_credits_summary_title)),
            )
            .child(
                FlatLabel::new(
                    "stars-about",
                    &st.about,
                    Markup::plain(tr(cx, key::lng_credits_summary_about).to_string()),
                )
                .width(px_of(st.about_max_width)),
            )
            .child(rows::skip())
            .child(
                RoundButton::new("stars-top-up", &CREDITS_SETTINGS_BIG_BALANCE_BUTTON, top_up)
                    .on_click(move |_, window, cx| stars_box::show(&env, window, cx)),
            )
            .child(rows::skip())
            .child(FlatLabel::new(
                "stars-balance",
                &st.about,
                Markup::rich(&balance_text),
            ))
            .child(rows::skip())
            .child(rows::skip())
    }

    fn tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let st = &CREDITS_HISTORY_TABS_SLIDER;
        let label_font = st.label_style.font;
        let padding = CREDITS_HISTORY_TABS_SLIDER_PADDING;
        let tabs = TABS.iter().map(|(direction, label)| {
            let direction = *direction;
            let is_active = direction == self.direction;
            let fg = if is_active {
                st.label_fg_active
            } else {
                st.label_fg
            };
            let weak = cx.weak_entity();
            div()
                .id(("stars-tab", direction as usize))
                .relative()
                .h(px_of(st.height))
                .pt(px_of(st.label_top))
                .px(px(10.0))
                .cursor_pointer()
                .font(font(label_font))
                .text_size(px_of(label_font.size))
                .line_height(font_height(label_font))
                .text_color(color(cx, fg))
                .child(tr(cx, *label))
                .children(is_active.then(|| {
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(px_of(st.bar_top))
                        .h(px_of(st.bar_stroke) / 2.0)
                        .rounded(px_of(st.bar_radius))
                        .bg(color(cx, st.bar_fg_active))
                }))
                .on_click(move |_, _, cx| {
                    let _ = weak.update(cx, |this, cx| this.choose(direction, cx));
                })
        });
        div()
            .flex()
            .pl(px_of(padding.left))
            .pr(px_of(padding.right))
            .children(tabs)
    }

    fn row(&self, index: usize, item: &Transaction, cx: &App) -> impl IntoElement + use<> {
        let text = |value: String, st: Font, fg: ColorRef| {
            div()
                .font(font(st))
                .text_size(px_of(st.size))
                .line_height(font_height(st))
                .text_color(color(cx, fg))
                .truncate()
                .child(value)
        };
        let date = crate::history::layout::date_time_full(
            crate::lang::strings(cx),
            item.date,
            &chrono::Local::now(),
        );
        let date = if item.is_refund {
            format!("{date} {}", tr(cx, key::lng_channel_earn_history_return))
        } else {
            date
        };
        let fg = if item.stars < 0 {
            OUTGOING_FG
        } else {
            INCOMING_FG
        };
        div()
            .id(("stars-transaction", index))
            .h(px(ROW_HEIGHT))
            .flex()
            .items_center()
            .pl(px(ROW_LEFT))
            .pr(px(ROW_LEFT - ROW_RIGHT))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(text(
                        self.party_name(item.party, cx),
                        SEMIBOLD_FONT,
                        NAME_FG,
                    ))
                    .child(text(date, NORMAL_FONT, DATE_FG)),
            )
            .child(div().pr(px(ROW_RIGHT)).child(text(
                format!("{} {STAR}", amount_text(item.stars)),
                SEMIBOLD_FONT,
                fg,
            )))
    }

    /// `Info::BotStarRef` and `PeerData::name` for the row's title.
    fn party_name(&self, party: Party, cx: &App) -> String {
        let chats = &self.env.targets.store.read(cx).state().chats;
        let named = match party {
            Party::User(user) => chats.user(user).map(model::user::User::name),
            Party::Chat(chat) => chats.chat(chat).map(|chat| chat.title.clone()),
            _ => None,
        };
        if let Some(name) = named {
            return name;
        }
        let label = match party {
            Party::PremiumBot => key::lng_credits_box_history_entry_premium_bot,
            Party::AppStore => key::lng_credits_box_history_entry_app_store,
            Party::GooglePlay => key::lng_credits_box_history_entry_play_market,
            Party::Fragment => key::lng_credits_box_history_entry_fragment,
            Party::Ads => key::lng_credits_box_history_entry_ads,
            _ => key::lng_credits_summary_title,
        };
        tr(cx, label).to_string()
    }
}

impl Render for StarsPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let transactions: Vec<_> = self
            .history
            .as_ref()
            .map(|history| {
                history
                    .transactions
                    .iter()
                    .enumerate()
                    .map(|(index, item)| self.row(index, item, cx))
                    .collect()
            })
            .unwrap_or_default();
        let has_history = !transactions.is_empty() || self.direction != Direction::All;
        let mut page = div()
            .flex()
            .flex_col()
            .child(self.cover(cx))
            .child(rows::divider(cx));
        if has_history {
            page = page
                .child(rows::skip())
                .child(self.tabs(cx))
                .children(transactions)
                .child(rows::skip());
        }
        page
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amounts_carry_their_sign() {
        assert_eq!(amount_text(1500), "+1,500");
        assert_eq!(amount_text(i64::MIN), "−9,223,372,036,854,775,808");
        assert_eq!(amount_text(-25), "\u{2212}25");
    }
}
