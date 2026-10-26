//! Reads one file out of a zip archive, such as the colors inside a
//! `.tdesktop-theme`. Only stored and deflated entries are supported.

use anyhow::{Context as _, anyhow, bail, ensure};

const END_SIGNATURE: u32 = 0x0605_4b50;
const ENTRY_SIGNATURE: u32 = 0x0201_4b50;
const LOCAL_SIGNATURE: u32 = 0x0403_4b50;
const END_SIZE: usize = 22;
const ENTRY_SIZE: usize = 46;
const LOCAL_SIZE: usize = 30;

fn u16_at(data: &[u8], at: usize) -> anyhow::Result<usize> {
    let bytes = data
        .get(at..at + 2)
        .ok_or_else(|| anyhow!("truncated archive"))?;
    Ok(usize::from(u16::from_le_bytes([bytes[0], bytes[1]])))
}

fn u32_at(data: &[u8], at: usize) -> anyhow::Result<u32> {
    let bytes = data
        .get(at..at + 4)
        .ok_or_else(|| anyhow!("truncated archive"))?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn usize_at(data: &[u8], at: usize) -> anyhow::Result<usize> {
    usize::try_from(u32_at(data, at)?).context("offset out of range")
}

/// The contents of `name` in `archive`. Sizes come from the central
/// directory, since local headers may leave them to a trailing descriptor.
pub(crate) fn extract(archive: &[u8], name: &str) -> anyhow::Result<Vec<u8>> {
    ensure!(archive.len() >= END_SIZE, "not a zip archive");
    let end = (0..=archive.len() - END_SIZE)
        .rev()
        .find(|&at| u32_at(archive, at).ok() == Some(END_SIGNATURE))
        .ok_or_else(|| anyhow!("no end of central directory"))?;
    let count = u16_at(archive, end + 10)?;
    let mut at = usize_at(archive, end + 16)?;
    for _ in 0..count {
        ensure!(
            u32_at(archive, at)? == ENTRY_SIGNATURE,
            "bad central directory"
        );
        let method = u16_at(archive, at + 10)?;
        let compressed = usize_at(archive, at + 20)?;
        let name_len = u16_at(archive, at + 28)?;
        let extra_len = u16_at(archive, at + 30)?;
        let comment_len = u16_at(archive, at + 32)?;
        let local = usize_at(archive, at + 42)?;
        let entry_name = archive
            .get(at + ENTRY_SIZE..at + ENTRY_SIZE + name_len)
            .ok_or_else(|| anyhow!("truncated archive"))?;
        at += ENTRY_SIZE + name_len + extra_len + comment_len;
        if entry_name != name.as_bytes() {
            continue;
        }
        ensure!(
            u32_at(archive, local)? == LOCAL_SIGNATURE,
            "bad local header"
        );
        let start =
            local + LOCAL_SIZE + u16_at(archive, local + 26)? + u16_at(archive, local + 28)?;
        let data = archive
            .get(start..start + compressed)
            .ok_or_else(|| anyhow!("truncated archive"))?;
        return match method {
            0 => Ok(data.to_vec()),
            8 => miniz_oxide::inflate::decompress_to_vec(data)
                .map_err(|error| anyhow!("inflating {name}: {error:?}")),
            other => bail!("{name} uses unsupported compression method {other}"),
        };
    }
    bail!("{name} is not in the archive")
}
