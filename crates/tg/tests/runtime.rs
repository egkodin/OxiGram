//! Runs against the real libtdjson. TDLib allows one receiver per process, so
//! everything that needs a runtime lives in one test.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use futures::StreamExt;
use futures::executor::block_on;
use td_api::enums::{AuthorizationState, OptionValue, Update};
use td_api::functions::{Close, GetChat, GetOption, SetLogVerbosityLevel};
use tg::{Runtime, StartError, TdError};

const TIMEOUT: Duration = Duration::from_secs(30);
const VERSION: &str = include_str!("../../../tdlib/VERSION");

#[test]
fn version_is_available_synchronously() {
    let value = tg::execute(&GetOption {
        name: "version".to_owned(),
    })
    .unwrap();
    assert_eq!(value, string_option(VERSION.trim()));
}

#[test]
fn requests_errors_updates_and_close() {
    // A hung route would block forever; fail the test instead.
    let (done, finished) = mpsc::channel();
    thread::spawn(move || {
        lifecycle();
        done.send(()).unwrap();
    });
    finished
        .recv_timeout(TIMEOUT)
        .expect("lifecycle failed or timed out");
}

fn lifecycle() {
    tg::execute(&SetLogVerbosityLevel {
        new_verbosity_level: 1,
    })
    .expect("setLogVerbosityLevel is a synchronous request");
    let runtime = Runtime::start().expect("this is the only runtime in the process");
    assert!(matches!(Runtime::start(), Err(StartError::AlreadyRunning)));
    let (client, mut updates) = runtime.create_client();

    // TDLib starts an instance on its first request; a new client must report
    // its state before the caller sends anything.
    // Option updates come first.
    let first = block_on(async {
        while let Some(update) = updates.next().await {
            if let Update::AuthorizationState(update) = update {
                return Some(*update.authorization_state);
            }
        }
        None
    });
    assert!(
        matches!(first, Some(AuthorizationState::WaitTdlibParameters(_))),
        "{first:?}"
    );

    let version = block_on(client.request(&GetOption {
        name: "version".to_owned(),
    }))
    .expect("the version option is available before setTdlibParameters");
    assert_eq!(version, string_option(VERSION.trim()));

    // Requests that need setTdlibParameters fail with a TDLib error object.
    let error = block_on(client.request(&GetChat { chat_id: 1 }))
        .expect_err("getChat needs setTdlibParameters");
    assert!(matches!(error, TdError::Td { code: 400, .. }), "{error:?}");

    block_on(client.request(&Close {})).expect("close is accepted in any state");
    let states: Vec<AuthorizationState> = block_on(
        updates
            .by_ref()
            .filter_map(|update| async move {
                match update {
                    Update::AuthorizationState(update) => Some(*update.authorization_state),
                    _ => None,
                }
            })
            .collect(),
    );
    // The stream ends right after the client reports that it is closed.
    assert!(matches!(states.last(), Some(AuthorizationState::Closed(_))));

    let after_close = block_on(client.request(&GetOption {
        name: "version".to_owned(),
    }));
    assert_eq!(after_close, Err(TdError::Closed));
}

fn string_option(value: &str) -> OptionValue {
    OptionValue::String(td_api::types::OptionValueString {
        value: value.to_owned(),
    })
}
