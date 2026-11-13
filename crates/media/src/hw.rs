//! The platform's hardware decoder through FFmpeg's device contexts
//! (VideoToolbox, VA-API, D3D11VA), which ffmpeg-next does not wrap. A
//! decoder without one decodes in software.
#![expect(
    unsafe_code,
    reason = "FFmpeg's hardware device API has no safe wrapper in ffmpeg-next (ADR 0003, revisited in M5)"
)]

use std::ptr;

use ffmpeg_next::{Error, codec, ffi, frame};

#[cfg(target_os = "macos")]
const DEVICE: ffi::AVHWDeviceType = ffi::AVHWDeviceType::AV_HWDEVICE_TYPE_VIDEOTOOLBOX;
#[cfg(target_os = "windows")]
const DEVICE: ffi::AVHWDeviceType = ffi::AVHWDeviceType::AV_HWDEVICE_TYPE_D3D11VA;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const DEVICE: ffi::AVHWDeviceType = ffi::AVHWDeviceType::AV_HWDEVICE_TYPE_VAAPI;

/// Gives a decoder that is not open yet the platform's device; FFmpeg's
/// default `get_format` then picks the hardware format and falls back to
/// software for streams the device cannot decode. False without a device.
pub(crate) fn attach(context: &mut codec::Context) -> bool {
    let mut device: *mut ffi::AVBufferRef = ptr::null_mut();
    // SAFETY: `device` is a valid out-pointer; FFmpeg leaves it null on failure.
    let created = unsafe {
        ffi::av_hwdevice_ctx_create(&raw mut device, DEVICE, ptr::null(), ptr::null_mut(), 0)
    };
    if created < 0 || device.is_null() {
        return false;
    }
    // SAFETY: the context is valid and not opened; it takes over this
    // reference and unrefs it when it is freed.
    unsafe { (*context.as_mut_ptr()).hw_device_ctx = device };
    true
}

/// A copy of a hardware frame in system memory, for the scaler; `None`
/// for a frame already there.
pub(crate) fn download(frame: &frame::Video) -> Result<Option<frame::Video>, Error> {
    // SAFETY: reads a field of a valid frame.
    if unsafe { (*frame.as_ptr()).hw_frames_ctx.is_null() } {
        return Ok(None);
    }
    let mut system = frame::Video::empty();
    // SAFETY: both frames are valid and `system` holds no buffers, so
    // FFmpeg allocates them in the device's first readable format.
    let copied = unsafe { ffi::av_hwframe_transfer_data(system.as_mut_ptr(), frame.as_ptr(), 0) };
    if copied < 0 {
        return Err(Error::from(copied));
    }
    Ok(Some(system))
}
