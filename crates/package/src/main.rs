#![cfg_attr(windows, windows_subsystem = "windows")]

use std::env;
use std::fs::OpenOptions;
use std::io::Cursor;
use std::process::{Command, ExitCode, ExitStatus};
use std::thread;
use std::time::Duration;

use anyhow::{Context as _, ensure};

#[cfg(target_os = "macos")]
const APP_ROOT: &str = "OxiGram.app";
#[cfg(not(target_os = "macos"))]
const APP_ROOT: &str = "OxiGram";

#[cfg(target_os = "macos")]
const APP_EXECUTABLE: &str = "Contents/MacOS/OxiGram";
#[cfg(target_os = "linux")]
const APP_EXECUTABLE: &str = "oxigram";
#[cfg(windows)]
const APP_EXECUTABLE: &str = "oxigram.exe";

// Set only when the updater is waiting to confirm the replacement starts.
const UPDATE_READY: &str = "OXIGRAM_UPDATE_READY";

fn main() -> ExitCode {
    match run() {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(_) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("Could not start OxiGram: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> anyhow::Result<ExitStatus> {
    let payload = include_bytes!(env!("OXIGRAM_PAYLOAD"));
    ensure!(!payload.is_empty(), "the release payload is missing");
    let temporary = tempfile::Builder::new().prefix("OxiGram-").tempdir()?;
    oxigram_package::unpack(Cursor::new(payload), temporary.path(), APP_ROOT, false)
        .context("unpacking the embedded release")?;
    let executable = temporary.path().join(APP_ROOT).join(APP_EXECUTABLE);
    let mut command = Command::new(executable);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;

        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command
        .args(env::args_os().skip(1))
        .env(oxigram_package::LAUNCHER_PATH, env::current_exe()?)
        .env(
            oxigram_package::LAUNCHER_PID,
            std::process::id().to_string(),
        )
        .env_remove(UPDATE_READY)
        .spawn()
        .context("opening the embedded application")?;
    if let Some(ready) = env::var_os(UPDATE_READY) {
        // Allow immediate loader or startup errors to reach the installer.
        thread::sleep(Duration::from_millis(500));
        if child.try_wait()?.is_none()
            && let Err(error) = OpenOptions::new().create_new(true).write(true).open(ready)
        {
            eprintln!("Could not report OxiGram startup: {error}");
        }
    }
    let status = child.wait()?;
    temporary
        .close()
        .context("removing temporary release files")?;
    Ok(status)
}
