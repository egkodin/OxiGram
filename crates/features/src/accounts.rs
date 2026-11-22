//! The accounts (`Main::Domain`): each has its own store, TDLib client and
//! data directory; the window shows the active one and switches without a
//! restart. Three at most, one more for each Premium account, six in all.

use std::rc::Rc;

use gpui::{App, Context, Entity, EventEmitter, Global, Subscription, WeakEntity};
use model::{Environment, auth};

use crate::{Store, settings};

/// `Main::Domain::kMaxAccounts`.
const MAX_ACCOUNTS: usize = 3;
/// `Main::Domain::kPremiumMaxAccounts`.
pub(crate) const PREMIUM_MAX_ACCOUNTS: usize = 6;

/// An account's data directory number. The first account's, 0, is the
/// directory oxigram used before it had accounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AccountId(pub u32);

/// Starts an account's TDLib client and returns the store it feeds.
pub type Start = Rc<dyn Fn(AccountId, WeakEntity<Accounts>, &mut App) -> Entity<Store>>;

#[derive(Debug, Clone)]
pub struct Account {
    pub id: AccountId,
    pub store: Entity<Store>,
}

/// `OthersUnreadState`: the unread messages of the accounts not shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OthersUnread {
    pub count: i32,
    pub is_all_muted: bool,
}

impl Default for OthersUnread {
    fn default() -> Self {
        Self {
            count: 0,
            is_all_muted: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountsEvent {
    /// Another account is shown now.
    Activated,
}

/// "Add Account" past the limit (`AccountsLimitBox`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LimitReached;

pub struct Accounts {
    list: Vec<Account>,
    active: AccountId,
    /// The signed-in account shown before "Add Account": the new account's
    /// intro goes back to it (`accountBeforeIntro`).
    before_intro: Option<AccountId>,
    /// None where accounts cannot be added (the snapshots).
    start: Option<Start>,
    others_unread: OthersUnread,
    subscriptions: Vec<(AccountId, Subscription)>,
}

impl std::fmt::Debug for Accounts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Accounts")
            .field("list", &self.list.iter().map(|a| a.id).collect::<Vec<_>>())
            .field("active", &self.active)
            .finish_non_exhaustive()
    }
}

impl EventEmitter<AccountsEvent> for Accounts {}

/// The window's accounts, for the screens that list them.
struct Domain(Entity<Accounts>);

impl Global for Domain {}

pub(crate) fn domain(cx: &App) -> Option<Entity<Accounts>> {
    cx.try_global::<Domain>().map(|domain| domain.0.clone())
}

pub(crate) fn set_domain(accounts: Entity<Accounts>, cx: &mut App) {
    cx.set_global(Domain(accounts));
}

impl Accounts {
    /// No accounts yet; `insert` them, the first one shown until `activate`.
    #[must_use]
    pub fn new(start: Option<Start>) -> Self {
        Self {
            list: Vec::new(),
            active: AccountId(0),
            before_intro: None,
            start,
            others_unread: OthersUnread::default(),
            subscriptions: Vec::new(),
        }
    }

    /// One account that cannot add others.
    pub fn single(store: Entity<Store>, cx: &mut Context<Self>) -> Self {
        let mut accounts = Self::new(None);
        accounts.insert(AccountId(0), store, cx);
        accounts
    }

    pub fn insert(&mut self, id: AccountId, store: Entity<Store>, cx: &mut Context<Self>) {
        if self.list.is_empty() {
            self.active = id;
        }
        let subscription = cx.observe(&store, move |this, _, cx| {
            this.refresh_unread(cx);
            this.drop_duplicate(id, cx);
        });
        self.subscriptions.push((id, subscription));
        self.list.push(Account { id, store });
        self.refresh_unread(cx);
    }

    /// The store shown in the window.
    #[must_use]
    pub fn active(&self) -> &Entity<Store> {
        &self.account(self.active).store
    }

    #[must_use]
    pub fn active_id(&self) -> AccountId {
        self.active
    }

    #[must_use]
    pub fn list(&self) -> &[Account] {
        &self.list
    }

    #[must_use]
    pub fn contains(&self, id: AccountId) -> bool {
        self.list.iter().any(|account| account.id == id)
    }

    pub(crate) fn others_unread(&self) -> OthersUnread {
        self.others_unread
    }

    /// The account the intro of an added one goes back to.
    pub(crate) fn before_intro(&self) -> Option<AccountId> {
        self.before_intro
    }

    pub(crate) fn can_add(&self) -> bool {
        self.start.is_some() && self.list.len() < PREMIUM_MAX_ACCOUNTS
    }

    fn account(&self, id: AccountId) -> &Account {
        self.list
            .iter()
            .find(|account| account.id == id)
            .expect("the active account is in the list")
    }

    /// Shows the account (`Domain::activate`) and remembers it.
    pub fn activate(&mut self, id: AccountId, cx: &mut Context<Self>) {
        if id == self.active || !self.contains(id) {
            return;
        }
        self.active = id;
        self.before_intro = None;
        self.save(cx);
        self.refresh_unread(cx);
        cx.emit(AccountsEvent::Activated);
        cx.notify();
    }

    /// `AccountsList::setupAdd`: a new account's intro, or the unfinished
    /// one's; past `maxAccounts` the limit box shows instead.
    pub(crate) fn add(&mut self, cx: &mut Context<Self>) -> Result<(), LimitReached> {
        let before = self.active;
        if let Some(waiting) = self.list.iter().find(|a| !is_signed_in(&a.store, cx)) {
            let id = waiting.id;
            self.activate(id, cx);
            self.before_intro = Some(before);
            return Ok(());
        }
        let Some(start) = self.start.clone() else {
            return Ok(());
        };
        if self.list.len() >= self.max(cx) {
            return Err(LimitReached);
        }
        let id = free_id(&self.list);
        let store = start(id, cx.weak_entity(), cx);
        self.insert(id, store, cx);
        self.activate(id, cx);
        self.before_intro = Some(before);
        Ok(())
    }

    /// `Intro::Step::finish`: an account signed in again in the intro logs
    /// out and the one already there shows.
    fn drop_duplicate(&mut self, id: AccountId, cx: &mut Context<Self>) {
        if id != self.active {
            return;
        }
        let me = |store: &Entity<Store>| {
            let store = store.read(cx);
            let state = store.state();
            state
                .auth
                .is_signed_in()
                .then_some(state.me)
                .flatten()
                .map(|me| (me, store.environment()))
        };
        let Some(signed) = me(self.active()) else {
            return;
        };
        let Some(existing) = self
            .list
            .iter()
            .find(|account| account.id != id && me(&account.store) == Some(signed))
            .map(|account| account.id)
        else {
            return;
        };
        tracing::info!(account = id.0, "account already signed in; logging out");
        let task = self.active().read(cx).auth(auth::Action::LogOut, cx);
        cx.spawn(async move |_, _| {
            if let Err(error) = task.await {
                tracing::warn!(%error, "logging out a duplicate account failed");
            }
        })
        .detach();
        self.activate(existing, cx);
    }

    /// `Intro::Widget::backRequested` on its first step: the account it was
    /// added from shows again and the unfinished one closes.
    pub(crate) fn back_from_intro(&mut self, cx: &mut Context<Self>) {
        let Some(before) = self.before_intro.take() else {
            return;
        };
        let leaving = self.active;
        self.activate(before, cx);
        if let Some(account) = self.list.iter().find(|a| a.id == leaving) {
            let task = account.store.read(cx).auth(auth::Action::Close, cx);
            cx.spawn(async move |_, _| {
                if let Err(error) = task.await {
                    tracing::warn!(%error, "closing an unfinished account failed");
                }
            })
            .detach();
        }
        self.forget(leaving, cx);
    }

    /// An account whose client ended after it logged out, while others
    /// stay (`removeRedundantAccounts`); another one shows if it was shown.
    pub fn remove(&mut self, id: AccountId, cx: &mut Context<Self>) {
        if id == self.active {
            let next = self
                .list
                .iter()
                .filter(|account| account.id != id)
                .max_by_key(|account| is_signed_in(&account.store, cx))
                .map(|account| account.id);
            if let Some(next) = next {
                self.activate(next, cx);
            }
        }
        self.forget(id, cx);
    }

    fn forget(&mut self, id: AccountId, cx: &mut Context<Self>) {
        self.list.retain(|account| account.id != id);
        self.subscriptions.retain(|(account, _)| *account != id);
        self.save(cx);
        self.refresh_unread(cx);
        cx.notify();
    }

    /// `Domain::maxAccounts`: one more for each Premium or test account.
    fn max(&self, cx: &App) -> usize {
        let premium = self
            .list
            .iter()
            .filter(|account| {
                let store = account.store.read(cx);
                is_signed_in(&account.store, cx)
                    && (store.environment() == Environment::Test || is_premium(store))
            })
            .count();
        max_accounts(premium)
    }

    fn save(&self, cx: &mut App) {
        let list = self.list.iter().map(|account| account.id.0).collect();
        settings::set_accounts(list, self.active.0, cx);
    }

    fn refresh_unread(&mut self, cx: &mut Context<Self>) {
        let others = others_unread(&self.list, self.active, cx);
        if others != self.others_unread {
            self.others_unread = others;
            cx.notify();
        }
    }
}

/// The menu button's icon while other accounts have unread messages
/// (`Widget::updateUnreadState`): `muted` when they all are.
pub(crate) fn others_unread_icon(
    cx: &App,
    unread: style::Icon,
    muted: style::Icon,
) -> Option<style::Icon> {
    let others = domain(cx)?.read(cx).others_unread();
    (others.count > 0).then_some(if others.is_all_muted { muted } else { unread })
}

pub(crate) fn is_signed_in(store: &Entity<Store>, cx: &App) -> bool {
    store.read(cx).state().auth.is_signed_in()
}

fn is_premium(store: &Store) -> bool {
    let state = store.state();
    state
        .me
        .and_then(|me| state.chats.user(me))
        .is_some_and(|user| user.is_premium)
}

/// An account's unread badge: the main list's unread messages, muted when
/// all of them are in muted chats (`unreadWithMentionsBadge`).
pub(crate) fn unread(store: &Entity<Store>, cx: &App) -> (i32, bool) {
    let unread = store.read(cx).state().unread_messages;
    (unread.count, unread.unmuted == 0)
}

/// `OtherAccountsUnreadStateCurrent`.
fn others_unread(list: &[Account], active: AccountId, cx: &App) -> OthersUnread {
    sum_unread(
        list.iter()
            .filter(|account| account.id != active && is_signed_in(&account.store, cx))
            .map(|account| unread(&account.store, cx)),
    )
}

fn sum_unread(badges: impl Iterator<Item = (i32, bool)>) -> OthersUnread {
    badges.fold(OthersUnread::default(), |total, (count, is_muted)| {
        OthersUnread {
            count: total.count + count,
            is_all_muted: total.is_all_muted && is_muted,
        }
    })
}

fn max_accounts(premium: usize) -> usize {
    (MAX_ACCOUNTS + premium).min(PREMIUM_MAX_ACCOUNTS)
}

/// The lowest directory number no account uses, so a removed account's
/// directory serves the next one.
fn free_id(list: &[Account]) -> AccountId {
    (0..)
        .map(AccountId)
        .find(|id| list.iter().all(|account| account.id != *id))
        .expect("fewer than u32::MAX accounts")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_accounts_and_one_more_per_premium_up_to_six() {
        assert_eq!(max_accounts(0), 3);
        assert_eq!(max_accounts(2), 5);
        assert_eq!(max_accounts(5), 6);
    }

    #[test]
    fn others_unread_is_muted_only_when_every_badge_is() {
        assert_eq!(sum_unread(std::iter::empty()), OthersUnread::default());
        let mixed = sum_unread([(3, true), (4, false)].into_iter());
        assert_eq!(
            mixed,
            OthersUnread {
                count: 7,
                is_all_muted: false
            }
        );
        assert!(sum_unread([(3, true), (0, true)].into_iter()).is_all_muted);
    }
}
