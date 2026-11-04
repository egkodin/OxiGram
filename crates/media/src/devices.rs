//! The speakers and microphone sound goes through (`Webrtc::Environment`):
//! the system's lists and the one chosen in Settings, else the default.

use std::str::FromStr as _;
use std::sync::RwLock;
use std::sync::atomic::Ordering;

use cpal::traits::{DeviceTrait as _, HostTrait as _};

use crate::audio::IS_SILENT;

/// One device in a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDevice {
    /// cpal's `host:device` id, as saved in the settings.
    pub id: String,
    pub name: String,
}

/// Which list a device comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Output,
    Input,
}

static OUTPUT: RwLock<Option<String>> = RwLock::new(None);
static INPUT: RwLock<Option<String>> = RwLock::new(None);

fn chosen(direction: Direction) -> &'static RwLock<Option<String>> {
    match direction {
        Direction::Output => &OUTPUT,
        Direction::Input => &INPUT,
    }
}

/// The system's devices; none in automated runs, which never touch them.
#[must_use]
pub fn list(direction: Direction) -> Vec<AudioDevice> {
    if IS_SILENT.load(Ordering::Relaxed) {
        return Vec::new();
    }
    let host = cpal::default_host();
    let devices = match direction {
        Direction::Output => host.output_devices().map(|list| list.collect::<Vec<_>>()),
        Direction::Input => host.input_devices().map(|list| list.collect::<Vec<_>>()),
    };
    match devices {
        Ok(devices) => devices.iter().filter_map(describe).collect(),
        Err(error) => {
            tracing::warn!(%error, ?direction, "cannot list the audio devices");
            Vec::new()
        }
    }
}

fn describe(device: &cpal::Device) -> Option<AudioDevice> {
    let id = device.id().ok()?.to_string();
    let name = device.description().ok()?.name().to_owned();
    Some(AudioDevice { id, name })
}

/// Sound from now on goes through `id`; None is the system's default.
pub fn choose(direction: Direction, id: Option<String>) {
    *chosen(direction)
        .write()
        .expect("the device lock is never poisoned: nothing panics under it") = id;
}

/// The chosen device while it is connected, else the system's default.
pub(crate) fn device(direction: Direction) -> Option<cpal::Device> {
    let host = cpal::default_host();
    let id = chosen(direction)
        .read()
        .expect("the device lock is never poisoned: nothing panics under it")
        .clone();
    let picked = id
        .and_then(|id| cpal::DeviceId::from_str(&id).ok())
        .and_then(|id| host.device_by_id(&id));
    picked.or_else(|| match direction {
        Direction::Output => host.default_output_device(),
        Direction::Input => host.default_input_device(),
    })
}
