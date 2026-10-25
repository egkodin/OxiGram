//! Regenerates `crates/style` from a Telegram Desktop checkout.
//!
//! Usage: `cargo run -p style-gen [-- <tdesktop dir>]`. The default directory is
//! the `tdesktop` checkout next to this repository. The checkout is only read.

mod palette;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context as _, bail};

const PALETTE_SOURCE: &str = "Telegram/lib_ui/ui/colors.palette";

fn main() -> anyhow::Result<()> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let tdesktop = env::args_os()
        .nth(1)
        .map_or_else(|| repo.join("../tdesktop"), PathBuf::from);

    let source_path = tdesktop.join(PALETTE_SOURCE);
    let source = fs::read_to_string(&source_path)
        .with_context(|| format!("reading {}", source_path.display()))?;
    let lib_ui = tdesktop.join("Telegram/lib_ui");
    let provenance = format!("{PALETTE_SOURCE} at lib_ui {}", git_head(&lib_ui)?);
    let code = palette::generate(&source, &provenance)
        .with_context(|| format!("generating from {}", source_path.display()))?;

    let out_path = repo.join("crates/style/src/palette.rs");
    fs::write(&out_path, code).with_context(|| format!("writing {}", out_path.display()))?;
    Ok(())
}

/// The commit `dir` is checked out at, for the generated file's header.
fn git_head(dir: &Path) -> anyhow::Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .context("running git")?;
    if !output.status.success() {
        bail!("git rev-parse HEAD failed in {}", dir.display());
    }
    Ok(String::from_utf8(output.stdout)
        .context("git printed a non-UTF-8 commit")?
        .trim()
        .to_owned())
}
