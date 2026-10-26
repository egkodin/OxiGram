//! Renders the sign-in screens to PNG files for comparison with Telegram
//! Desktop, without signing in: each window gets preset model events, and
//! TDLib never receives parameters, so nothing goes online.
//!
//! cargo run -p rgram --example snapshots -- [output directory]

mod chats;

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context as _;
use features::{Root, Store};
use gpui::{
    AnyWindowHandle, App, AppContext as _, AsyncApp, Bounds, Entity, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, PlatformInput, Point, Window,
    WindowBounds, WindowOptions, point, px, size,
};
use model::auth::{Code, Delivery, Password, Recovery, State, Terms, TermsId};
use model::text::FormattedText;
use model::{Environment, Event, Secret, Session};

/// Long enough for the intro's slide and cover animations to finish.
const SETTLE: Duration = Duration::from_millis(1500);
const WIDTH: f32 = 800.0;
const HEIGHT: f32 = 600.0;

/// One window: the events it receives, each step saved under its name.
struct Scenario {
    /// Applied before the first step.
    setup: Vec<Event>,
    steps: Vec<(&'static str, Event)>,
    /// Clicks at window points after the steps, each saved.
    clicks: Vec<(&'static str, MouseButton, (f32, f32))>,
}

fn auth(state: State) -> Event {
    Event::Auth(state)
}

fn code() -> Event {
    auth(State::WaitCode(Code {
        phone_number: Secret::new("+12125550100".to_owned()),
        delivery: Delivery::Telegram { length: 5 },
        next_delivery: Some(Delivery::Sms { length: 5 }),
        resend_after: Duration::from_secs(60),
    }))
}

fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            setup: Vec::new(),
            clicks: Vec::new(),
            steps: vec![("01-start", auth(State::WaitPhoneNumber))],
        },
        Scenario {
            setup: Vec::new(),
            clicks: Vec::new(),
            steps: vec![(
                "02-qr",
                auth(State::WaitQrScan {
                    link: Secret::new("tg://login?token=c25hcHNob3Qtb25seQ".to_owned()),
                }),
            )],
        },
        Scenario {
            // Back from the code to the number, as a wrong number does.
            setup: Vec::new(),
            clicks: Vec::new(),
            steps: vec![
                ("04-code", code()),
                ("03-phone", auth(State::WaitPhoneNumber)),
            ],
        },
        Scenario {
            setup: Vec::new(),
            clicks: Vec::new(),
            steps: vec![(
                "05-password",
                auth(State::WaitPassword(Password {
                    hint: "pet's name".to_owned(),
                    recovery: Recovery::Email {
                        pattern: "a***@example.org".to_owned(),
                    },
                    has_passport_data: false,
                })),
            )],
        },
        Scenario {
            setup: Vec::new(),
            clicks: Vec::new(),
            steps: vec![(
                "06-signup",
                auth(State::WaitRegistration {
                    terms: Terms {
                        text: FormattedText::plain(
                            "By signing up you accept the Terms of Service.",
                        ),
                        min_age: 0,
                        show_popup: true,
                    },
                }),
            )],
        },
        Scenario {
            setup: Vec::new(),
            clicks: Vec::new(),
            steps: vec![("07-email", auth(State::WaitEmailAddress))],
        },
        Scenario {
            setup: Vec::new(),
            clicks: Vec::new(),
            steps: vec![(
                "08-premium-required",
                auth(State::WaitPremiumPurchase {
                    support_email: "support@example.org".to_owned(),
                }),
            )],
        },
        Scenario {
            setup: Vec::new(),
            clicks: Vec::new(),
            steps: vec![
                ("09-signed-in", auth(State::Ready)),
                (
                    "10-terms-update",
                    Event::TermsUpdate {
                        id: TermsId("snapshot".to_owned()),
                        terms: Terms {
                            text: FormattedText::plain(
                                "We have updated our Terms of Service. Please read and accept them to keep using the app.",
                            ),
                            min_age: 16,
                            show_popup: true,
                        },
                    },
                ),
            ],
        },
        // Last: the night mode it switches on stays for later windows.
        Scenario {
            setup: chats::events(),
            clicks: vec![
                ("12-main-menu", MouseButton::Left, MENU_BUTTON),
                ("13-night-menu", MouseButton::Left, NIGHT_MODE_ROW),
                ("14-night-chat-list", MouseButton::Left, OUTSIDE_MENU),
                ("15-night-archive", MouseButton::Left, ARCHIVE_ROW),
                // The archive's first chat sits where the archive row was.
                ("16-night-chat-menu", MouseButton::Right, ARCHIVE_ROW),
            ],
            steps: vec![("11-chat-list", auth(State::Ready))],
        },
    ]
}

/// The main menu button, the menu's night mode row, and a point beside the
/// open menu.
const MENU_BUTTON: (f32, f32) = (27.0, 27.0);
const NIGHT_MODE_ROW: (f32, f32) = (100.0, 447.0);
const OUTSIDE_MENU: (f32, f32) = (600.0, 300.0);
const ARCHIVE_ROW: (f32, f32) = (150.0, 85.0);

fn main() -> anyhow::Result<()> {
    let out = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("target/snapshots"), PathBuf::from);
    std::fs::create_dir_all(&out).with_context(|| format!("creating {}", out.display()))?;
    tg::execute(&td_api::functions::SetLogVerbosityLevel {
        new_verbosity_level: 1,
    })
    .context("setting TDLib's log verbosity")?;
    let runtime = tg::Runtime::start().context("starting TDLib")?;
    let (client, updates) = runtime.create_client();
    gpui_platform::application().run(move |cx: &mut App| {
        // Kept alive so the idle client's updates have somewhere to go.
        let _updates = updates;
        if let Err(error) = ui::init(cx) {
            eprintln!("{error:#}");
            cx.quit();
            return;
        }
        features::init(cx);
        let session = Session::new(client);
        cx.spawn(async move |cx| {
            if let Err(error) = run(session, &out, cx).await {
                eprintln!("{error:#}");
            }
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
    drop(runtime);
    Ok(())
}

async fn run(session: Session, out: &Path, cx: &mut AsyncApp) -> anyhow::Result<()> {
    for scenario in scenarios() {
        let mut steps = scenario.steps.into_iter();
        let setup = scenario.setup;
        let Some((first_name, first)) = steps.next() else {
            continue;
        };
        let store = cx.new(|cx| {
            let mut store = Store::new(session.clone(), Environment::Production);
            for event in setup {
                store.apply(event, cx);
            }
            store.apply(first, cx);
            store
        });
        let window = cx.update(|cx| open(store.clone(), cx))?;
        settle_and_save(window, first_name, out, cx).await?;
        for (name, event) in steps {
            store.update(cx, |store, cx| store.apply(event, cx));
            settle_and_save(window, name, out, cx).await?;
        }
        for (name, button, (x, y)) in scenario.clicks {
            window.update(cx, |_, window, cx| {
                click(button, point(px(x), px(y)), window, cx);
            })?;
            settle_and_save(window, name, out, cx).await?;
        }
        window.update(cx, |_, window, _| window.remove_window())?;
    }
    Ok(())
}

fn click(button: MouseButton, position: Point<Pixels>, window: &mut Window, cx: &mut App) {
    let modifiers = Modifiers::default();
    window.dispatch_event(
        PlatformInput::MouseMove(MouseMoveEvent {
            position,
            pressed_button: None,
            modifiers,
        }),
        cx,
    );
    window.draw(cx).clear(cx);
    window.dispatch_event(
        PlatformInput::MouseDown(MouseDownEvent {
            button,
            position,
            modifiers,
            click_count: 1,
            first_mouse: false,
        }),
        cx,
    );
    window.dispatch_event(
        PlatformInput::MouseUp(MouseUpEvent {
            button,
            position,
            modifiers,
            click_count: 1,
        }),
        cx,
    );
}

fn open(store: Entity<Store>, cx: &mut App) -> anyhow::Result<AnyWindowHandle> {
    let bounds = Bounds::centered(None, size(px(WIDTH), px(HEIGHT)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    };
    let window = cx.open_window(options, |window, cx| {
        cx.new(|cx| Root::new(store, window, cx))
    })?;
    Ok(window.into())
}

async fn settle_and_save(
    window: AnyWindowHandle,
    name: &str,
    out: &Path,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    // A window that is not on screen gets no display-link frames, so draw by
    // hand: once to start the transitions, once after they settled.
    window.update(cx, |_, window, cx| window.draw(cx).clear(cx))?;
    cx.background_executor().timer(SETTLE).await;
    window.update(cx, |_, window, cx| window.draw(cx).clear(cx))?;
    let image = window.update(cx, |_, window, _| window.render_to_image())??;
    let path = out.join(format!("{name}.png"));
    image
        .save(&path)
        .with_context(|| format!("saving {}", path.display()))?;
    println!("{}", path.display());
    Ok(())
}
