//! Regenerates `crates/style` from a Telegram Desktop checkout.
//!
//! Usage: `cargo run -p style-gen [-- <tdesktop dir>]`. The default directory is
//! the `tdesktop` checkout next to this repository. The checkout is only read.

mod emoji;
mod palette;
mod style;
mod zip;

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context as _, bail};

use style::emit::IconFile;

const PALETTE_SOURCE: &str = "Telegram/lib_ui/ui/colors.palette";
/// The embedded themes but Classic, which is the palette itself
/// (`Window::Theme::EmbeddedThemes`): the constant, its doc, the file.
const THEMES: [(&str, &str, &str); 3] = [
    (
        "NIGHT",
        "The night palette, \"Tinted\" in Chat Settings (`night.tdesktop-theme`).",
        "Telegram/Resources/night.tdesktop-theme",
    ),
    (
        "DAY_BLUE",
        "The \"Day\" palette (`day-blue.tdesktop-theme`).",
        "Telegram/Resources/day-blue.tdesktop-theme",
    ),
    (
        "NIGHT_GREEN",
        "The \"Night\" palette (`night-green.tdesktop-theme`).",
        "Telegram/Resources/night-green.tdesktop-theme",
    ),
];
const THEME_COLORS: &str = "colors.tdesktop-theme";

/// The style modules oxigram uses; their `using` includes come along.
const STYLE_ROOTS: &[&str] = &[
    "boxes/connection_box.style",
    "boxes/polls.style",
    "dialogs/dialogs.style",
    "dialogs/dialogs_widget.style",
    "dialogs/ui/dialogs_layout.style",
    "history/view/controls/history_view_voice_record_bar.style",
    "info/info.style",
    "media/view/media_view.style",
    "settings/sections/settings_active_sessions.style",
    "settings/sections/settings_premium.style",
    "intro/intro.style",
    "ui/boxes/choose_date_time.style",
    "ui/boxes/country_select_box.style",
    "ui/chat/chat.style",
    "ui/chat/chat_style.style",
    "ui/effects/credits.style",
    "ui/filter_icons.style",
    "window/window.style",
    "window/window_main_menu.style",
];

/// Telegram's paper-plane logo; oxigram may not use it (ADR 0004), so these masks
/// stay unembedded and the UI draws oxigram's own mark in their place.
const LOGO_ICONS: &[&str] = &[
    "intro_plane_inner",
    "intro_plane_outer",
    "intro_plane_top",
    "intro_plane_trace",
    "intro_qr_plane",
];

/// Icons tdesktop draws from code or Lottie rather than a style, which
/// oxigram draws as masks: the send button's microphone (`paintRecord`) and
/// the Premium page's star (`Ui::Premium::ColorizedSvg`).
const CODE_ICONS: &[&str] = &["chat/input_record", "settings/star"];

fn main() -> anyhow::Result<()> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let tdesktop = env::args_os()
        .nth(1)
        .map_or_else(|| repo.join("../tdesktop"), PathBuf::from);
    let style_crate = repo.join("crates/style");

    let source_path = tdesktop.join(PALETTE_SOURCE);
    let source = fs::read_to_string(&source_path)
        .with_context(|| format!("reading {}", source_path.display()))?;
    let lib_ui_head = git_head(&tdesktop.join("Telegram/lib_ui"))?;
    let provenance = format!("{PALETTE_SOURCE} at lib_ui {lib_ui_head}");
    let mut sources = Vec::new();
    for (_, _, file) in THEMES {
        let path = tdesktop.join(file);
        let archive = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let colors = String::from_utf8(zip::extract(&archive, THEME_COLORS)?)
            .with_context(|| format!("decoding {}", path.display()))?;
        sources.push(colors);
    }
    let themes: Vec<palette::Theme<'_>> = THEMES
        .iter()
        .zip(&sources)
        .map(|((name, doc, _), source)| palette::Theme { name, doc, source })
        .collect();
    let code = palette::generate(&source, &themes, &provenance)
        .with_context(|| format!("generating from {}", source_path.display()))?;
    write(&style_crate.join("src/palette.rs"), &code)?;

    let (include_dirs, icon_dirs) = style::parser::tdesktop_dirs(&tdesktop);
    let mut loader = style::parser::Loader::new(include_dirs, icon_dirs, palette::names(&source)?);
    let mut modules = Vec::new();
    for root in STYLE_ROOTS {
        for module in loader.load(root)?.dependencies() {
            if !modules
                .iter()
                .any(|known| std::rc::Rc::ptr_eq(known, &module))
            {
                modules.push(module);
            }
        }
    }
    let emoji_provenance = format!("lib_ui {lib_ui_head}");
    write(
        &style_crate.join("src/emoji.rs"),
        &emoji::generate(&tdesktop, &emoji_provenance)?,
    )?;

    let provenance = format!("tdesktop {}, lib_ui {lib_ui_head}", git_head(&tdesktop)?);
    let mut emitted = style::emit::emit(&modules, &provenance)?;
    for name in CODE_ICONS {
        let path = loader
            .find_icon(name)
            .with_context(|| format!("icon {name} not found"))?;
        emitted.icons.insert(IconFile {
            name: (*name).to_owned(),
            path,
        });
    }
    for (name, code) in &emitted.modules {
        write(&style_crate.join(format!("src/{name}.rs")), code)?;
    }
    copy_icons(&style_crate, &emitted.icons, &provenance)?;
    Ok(())
}

/// Copies every referenced icon image (with its `@2x`/`@3x` variants) into
/// `crates/style/icons` and writes the table that embeds them.
fn copy_icons(
    style_crate: &Path,
    icons: &std::collections::BTreeSet<IconFile>,
    provenance: &str,
) -> anyhow::Result<()> {
    let target = style_crate.join("icons");
    if target.exists() {
        fs::remove_dir_all(&target).with_context(|| format!("clearing {}", target.display()))?;
    }
    let mut table = String::new();
    writeln!(
        table,
        "// @generated by tools/style-gen from tdesktop icons at {provenance}."
    )?;
    writeln!(
        table,
        "// Derived from Telegram Desktop (GPLv3); see NOTICE.md. Do not edit."
    )?;
    writeln!(table)?;
    writeln!(table, "use crate::IconFiles;")?;
    writeln!(table)?;
    writeln!(
        table,
        "/// The embedded image files of the icon mask `name`."
    )?;
    writeln!(table, "#[must_use]")?;
    writeln!(
        table,
        "pub fn icon_files(name: &str) -> Option<IconFiles> {{"
    )?;
    writeln!(table, "    Some(match name {{")?;
    for icon in icons
        .iter()
        .filter(|icon| !LOGO_ICONS.contains(&icon.name.as_str()))
    {
        let extension = icon
            .path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();
        let copy = |suffix: &str| -> anyhow::Result<Option<String>> {
            let source = icon.path.with_file_name(format!(
                "{}{suffix}.{extension}",
                icon.path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default()
            ));
            if !source.is_file() {
                return Ok(None);
            }
            let relative = format!("{}{suffix}.{extension}", icon.name);
            let destination = target.join(&relative);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&source, &destination)
                .with_context(|| format!("copying {}", source.display()))?;
            Ok(Some(format!("include_bytes!(\"../icons/{relative}\")")))
        };
        let entry = match extension {
            "svg" => {
                let Some(file) = copy("")? else {
                    bail!("{} vanished", icon.path.display())
                };
                format!("IconFiles::Svg({file})")
            }
            "png" => {
                let Some(x1) = copy("")? else {
                    bail!("{} vanished", icon.path.display())
                };
                let optional = |file: Option<String>| {
                    file.map_or_else(|| "None".to_owned(), |f| format!("Some({f})"))
                };
                format!(
                    "IconFiles::Png {{ x1: {x1}, x2: {}, x3: {} }}",
                    optional(copy("@2x")?),
                    optional(copy("@3x")?)
                )
            }
            other => bail!("unsupported icon format {other} for {}", icon.name),
        };
        writeln!(table, "        {:?} => {entry},", icon.name)?;
    }
    writeln!(table, "        _ => return None,")?;
    writeln!(table, "    }})")?;
    writeln!(table, "}}")?;
    write(&style_crate.join("src/icons.rs"), &table)
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
