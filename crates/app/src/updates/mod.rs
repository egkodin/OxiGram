//! Optional release updates, entirely independent of the Telegram account.

mod install;
mod releases;

use std::ffi::OsStr;
use std::process::Command;
use std::time::Duration;

use gpui::{AnyWindowHandle, App, AsyncApp, PromptButton, PromptLevel};
use i18n::key;

use crate::shutdown;

const FIRST_CHECK: Duration = Duration::from_secs(20);
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

pub(crate) fn schedule(cx: &mut App, profile: Option<std::path::PathBuf>) {
    if matches!(
        std::env::var("OXIGRAM_DISABLE_UPDATES").as_deref(),
        Ok("1" | "true")
    ) {
        return;
    }
    let Some(window) = cx.windows().into_iter().next() else {
        return;
    };
    let background = cx.background_executor().clone();
    cx.spawn(async move |cx| {
        background.timer(FIRST_CHECK).await;
        let Some(package) = background.spawn(async { install::Package::current() }).await else {
            return;
        };
        let mut offered = None;
        loop {
            let suffix = package.suffix();
            let version = package.version.clone();
            let found = background.spawn(async move { releases::check(&suffix, &version) }).await;
            match found {
                Ok(Some(update)) if offered.as_ref() != Some(&update.version) => {
                    offered = Some(update.version.clone());
                    let Some(approved) = offer(window, cx, &update).await else {
                        return;
                    };
                    if approved {
                        let local = package.clone();
                        let page = update.page.clone();
                        let profile = profile.clone();
                        match background.spawn(async move { local.prepare(&update, profile.as_deref()) }).await {
                            Ok(prepared) => {
                                let result = cx.update(|cx| {
                                    if !shutdown::is_quitting(cx) && !cx.windows().is_empty() {
                                        prepared.launch()?;
                                        shutdown::quit(cx);
                                    }
                                    Ok::<_, anyhow::Error>(())
                                });
                                match result {
                                    Ok(()) => return,
                                    Err(error) => {
                                        tracing::warn!(%error, "starting the update installer failed");
                                        show_failure(window, cx, &page).await;
                                    }
                                }
                            }
                            Err(error) => {
                                tracing::warn!(%error, "preparing the update failed");
                                show_failure(window, cx, &page).await;
                            }
                        }
                    }
                }
                Err(error) => tracing::debug!(%error, "checking for updates failed"),
                _ => {}
            }
            background.timer(CHECK_INTERVAL).await;
        }
    })
    .detach();
}

async fn offer(
    window: AnyWindowHandle,
    cx: &mut AsyncApp,
    update: &releases::Update,
) -> Option<bool> {
    let decision = window
        .update(cx, |_, window, cx| {
            use features::lang::{tr, tr_with};

            let version = update.version.to_string();
            let title = tr_with(cx, key::oxigram_update_available, &[("version", &version)]);
            let detail = tr(cx, key::oxigram_update_detail);
            let buttons = [
                PromptButton::ok(tr(cx, key::oxigram_update_install)),
                PromptButton::cancel(tr(cx, key::lng_cancel)),
            ];
            window.prompt(PromptLevel::Info, &title, Some(&detail), &buttons, cx)
        })
        .ok()?;
    decision.await.ok().map(|choice| choice == 0)
}

async fn show_failure(window: AnyWindowHandle, cx: &mut AsyncApp, page: &str) {
    let Ok(decision) = window.update(cx, |_, window, cx| {
        use features::lang::tr;

        let title = tr(cx, key::oxigram_update_failed);
        let detail = tr(cx, key::oxigram_update_failed_detail);
        let buttons = [
            PromptButton::ok(tr(cx, key::oxigram_update_release_page)),
            PromptButton::cancel(tr(cx, key::lng_cancel)),
        ];
        window.prompt(PromptLevel::Warning, &title, Some(&detail), &buttons, cx)
    }) else {
        return;
    };
    if decision.await == Ok(0) {
        cx.update(|cx| cx.open_url(page));
    }
}

fn quiet_command(program: impl AsRef<OsStr>) -> Command {
    let command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;

        let mut command = command;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
        command
    }
    #[cfg(not(windows))]
    command
}
