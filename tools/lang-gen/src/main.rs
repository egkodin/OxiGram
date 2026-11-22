//! Regenerates `crates/i18n` from a Telegram Desktop checkout: the English
//! strings of `Telegram/Resources/langs/lang.strings` plus oxigram's own
//! `crates/i18n/oxigram.strings`, and the plural rules of
//! `Telegram/SourceFiles/lang/lang_tag.cpp`, translated to Rust.
//!
//! Usage: `cargo run -p lang-gen [-- <tdesktop dir>]`. The default directory is
//! the `tdesktop` checkout next to this repository. The checkout is only read.

mod plural;
mod strings;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context as _, bail};

const STRINGS_SOURCE: &str = "Telegram/Resources/langs/lang.strings";
const PLURAL_SOURCE: &str = "Telegram/SourceFiles/lang/lang_tag.cpp";

fn main() -> anyhow::Result<()> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let tdesktop = env::args_os()
        .nth(1)
        .map_or_else(|| repo.join("../tdesktop"), PathBuf::from);
    let i18n = repo.join("crates/i18n");
    let provenance = format!("tdesktop {}", git_head(&tdesktop)?);

    let tdesktop_strings = read(&tdesktop.join(STRINGS_SOURCE))?;
    let oxigram_strings = read(&i18n.join("oxigram.strings"))?;
    let code = strings::generate(
        &[
            (STRINGS_SOURCE, tdesktop_strings.as_str()),
            ("crates/i18n/oxigram.strings", oxigram_strings.as_str()),
        ],
        &provenance,
    )?;
    write(&i18n.join("src/english.rs"), &code)?;

    let plural_source = read(&tdesktop.join(PLURAL_SOURCE))?;
    let code = plural::generate(&plural_source, &format!("{PLURAL_SOURCE} at {provenance}"))
        .with_context(|| format!("translating {PLURAL_SOURCE}"))?;
    write(&i18n.join("src/plural_rules.rs"), &code)
}

fn read(path: &Path) -> anyhow::Result<String> {
    fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))
}

fn write(path: &Path, code: &str) -> anyhow::Result<()> {
    fs::write(path, code).with_context(|| format!("writing {}", path.display()))
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
