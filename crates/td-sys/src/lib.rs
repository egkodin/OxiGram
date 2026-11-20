//! Safe bindings to TDLib's JSON interface (`td/telegram/td_json_client.h`).
//!
//! One of the two places in the workspace with `unsafe` code (ADR 0003); the
//! other is `media::hw`.
//! Every string TDLib returns is copied before the wrapper returns, because TDLib
//! reuses the buffer on the next `td_receive` or `td_execute` call.
#![expect(
    unsafe_code,
    reason = "FFI to libtdjson; every unsafe block carries a SAFETY comment"
)]

use std::cell::Cell;
use std::ffi::{CStr, c_char, c_double, c_int};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

// SAFETY: signatures match td_json_client.h in the pinned TDLib. Creating an
// ID takes no pointers and is thread-safe. The other calls stay private.
unsafe extern "C" {
    safe fn td_create_client_id() -> c_int;
    fn td_send(client_id: c_int, request: *const c_char);
    fn td_receive(timeout: c_double) -> *const c_char;
    fn td_execute(request: *const c_char) -> *const c_char;
}

/// Identifier of a TDLib client instance, as TDLib reports it in `@client_id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(pub i32);

/// Creates a TDLib client instance. It starts working after the first request.
#[must_use]
pub fn create_client_id() -> ClientId {
    ClientId(td_create_client_id())
}

/// Sends a JSON request to a client. May be called from any thread.
pub fn send(client: ClientId, request: &CStr) {
    // SAFETY: `request` is NUL-terminated and outlives the call; td_send may be
    // called from any thread and does not keep the pointer.
    unsafe { td_send(client.0, request.as_ptr()) }
}

/// Executes a JSON request synchronously. Only some requests support this.
///
/// Returns `None` if TDLib returned no response.
#[must_use]
pub fn execute(request: &CStr) -> Option<String> {
    // SAFETY: `request` is NUL-terminated and outlives the call; td_execute may be
    // called from any thread. Its result lives in a thread-local buffer
    // (`current_output` in ClientJson.cpp) that only the next td_receive or
    // td_execute on this thread frees, and it is copied before this returns.
    unsafe { copy_response(td_execute(request.as_ptr())) }
}

/// Whether a [`Receiver`] currently exists in this process.
static RECEIVER_TAKEN: AtomicBool = AtomicBool::new(false);

/// The right to call `td_receive`.
///
/// TDLib forbids concurrent `td_receive` calls and the call returns objects of
/// every client in the process, so at most one `Receiver` exists at a time and
/// it is `!Sync`. It is released when dropped.
#[derive(Debug)]
pub struct Receiver {
    _not_sync: PhantomData<Cell<()>>,
}

impl Receiver {
    /// Takes the process-wide receiver, or returns `None` if it is already taken.
    #[must_use]
    pub fn take() -> Option<Self> {
        RECEIVER_TAKEN
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| Self {
                _not_sync: PhantomData,
            })
    }

    /// Waits up to `timeout` for the next update or response of any client.
    #[must_use]
    pub fn receive(&mut self, timeout: Duration) -> Option<String> {
        // SAFETY: only one Receiver exists (see `take`) and `&mut self` serializes
        // its calls, so td_receive is never called concurrently. The result is
        // null or a thread-local buffer valid until the next td_receive or
        // td_execute on this thread; it is copied before this returns.
        unsafe { copy_response(td_receive(timeout.as_secs_f64())) }
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        RECEIVER_TAKEN.store(false, Ordering::Release);
    }
}

/// Copies a string returned by TDLib.
///
/// # Safety
///
/// `ptr` is null or points to a NUL-terminated string that stays valid for the
/// duration of this call.
unsafe fn copy_response(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: `ptr` is non-null and valid per this function's contract.
    let response = unsafe { CStr::from_ptr(ptr) };
    Some(response.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execute_returns_the_linked_tdlib_version() {
        let response = execute(cr#"{"@type":"getOption","name":"version"}"#).unwrap();
        let expected = include_str!("../../../tdlib/VERSION").trim();
        assert_eq!(
            response,
            format!(r#"{{"@type":"optionValueString","value":"{expected}"}}"#)
        );
    }

    #[test]
    fn only_one_receiver_exists_at_a_time() {
        let first = Receiver::take().unwrap();
        assert!(Receiver::take().is_none());
        drop(first);
        assert!(Receiver::take().is_some());
    }
}
