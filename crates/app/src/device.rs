//! The device and system TDLib reports, which the account's list of active
//! sessions shows (`Platform::DeviceModelPretty`, `SystemVersionPretty`).
//! TDLib refuses empty values.

use std::process::Command;

/// What a device shows as when the system does not say.
const FALLBACK_MODEL: &str = "Desktop";

// shortcut: the raw model id ("Mac16,10"); tdesktop maps ids to names like "MacBook Pro", worth doing with the M7 OS integration.
pub(crate) fn model() -> String {
    if cfg!(target_os = "macos") {
        return output("sysctl", &["-n", "hw.model"]).unwrap_or_else(|| FALLBACK_MODEL.to_owned());
    }
    FALLBACK_MODEL.to_owned()
}

pub(crate) fn system_version() -> String {
    if cfg!(target_os = "macos") {
        return output("sw_vers", &["-productVersion"])
            .map_or_else(|| "macOS".to_owned(), |version| format!("macOS {version}"));
    }
    std::env::consts::OS.to_owned()
}

/// A command's trimmed output, `None` if it fails or prints nothing.
fn output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_and_system_are_never_empty() {
        assert!(!model().is_empty());
        assert!(!system_version().is_empty());
    }
}
