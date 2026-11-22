//! The device and system TDLib reports, which the account's list of active
//! sessions shows (`Platform::DeviceModelPretty`, `SystemVersionPretty`).
//! TDLib refuses empty values.

use std::process::Command;

/// What a device shows as when the system does not say.
const FALLBACK_MODEL: &str = "Desktop";

pub(crate) fn describe() -> (String, String) {
    if cfg!(target_os = "macos") {
        // sysctl can return one value even when the other key is unavailable.
        let output = Command::new("/usr/sbin/sysctl")
            .args(["hw.model", "kern.osproductversion"])
            .output()
            .ok();
        return mac_description(
            output
                .as_ref()
                .map_or(&[], |output| output.stdout.as_slice()),
        );
    }
    (FALLBACK_MODEL.to_owned(), std::env::consts::OS.to_owned())
}

fn mac_description(output: &[u8]) -> (String, String) {
    let output = String::from_utf8_lossy(output);
    let value = |key: &str| {
        output
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(name, _)| *name == key)
            .map(|(_, value)| value.trim())
            .filter(|value| !value.is_empty())
    };
    let model = value("hw.model").unwrap_or(FALLBACK_MODEL).to_owned();
    let version = value("kern.osproductversion")
        .map_or_else(|| "macOS".to_owned(), |version| format!("macOS {version}"));
    (model, version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_and_system_are_never_empty() {
        let (model, system) = describe();
        assert!(!model.is_empty());
        assert!(!system.is_empty());
    }

    #[test]
    fn mac_properties_are_independent() {
        assert_eq!(
            mac_description(b"hw.model: Mac16,10\nkern.osproductversion: 26.1\n"),
            ("Mac16,10".into(), "macOS 26.1".into()),
        );
        assert_eq!(
            mac_description(b"kern.osproductversion: 26.1\n"),
            ("Desktop".into(), "macOS 26.1".into()),
        );
    }
}
