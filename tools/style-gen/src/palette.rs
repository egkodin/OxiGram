//! Parses tdesktop's `colors.palette` and emits it as Rust.
//!
//! Each line is `name: value [| fallback]; // comment`, where `value` is
//! `#rrggbb`, `#rrggbbaa` or the name of an earlier color. `fallback` names the
//! color a theme without this entry uses; the default palette ignores it.
//!
//! Themes (`.tdesktop-theme` colors) use the same syntax and set any subset;
//! the rest come from their fallback when the theme set it, else the default.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use anyhow::{Context as _, anyhow, bail, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rgba([u8; 4]);

#[derive(Debug)]
enum Value {
    Color(Rgba),
    Reference(String),
}

#[derive(Debug)]
struct Entry {
    name: String,
    value: Value,
    fallback: Option<String>,
    comment: String,
}

/// A bundled theme emitted as another `Palette` constant.
pub(crate) struct Theme<'a> {
    /// The constant's name, such as `NIGHT`.
    pub name: &'a str,
    pub doc: &'a str,
    /// The theme's `colors.tdesktop-theme`.
    pub source: &'a str,
}

/// Generates `crates/style/src/palette.rs` from the palette source. `provenance`
/// goes into the header and says where `source` came from.
pub(crate) fn generate(
    source: &str,
    themes: &[Theme<'_>],
    provenance: &str,
) -> anyhow::Result<String> {
    let entries = parse(source)?;
    let colors = resolve(&entries)?;
    let themed = themes
        .iter()
        .map(|theme| {
            let colors = theme_colors(&entries, &colors, theme.source)
                .with_context(|| format!("theme {}", theme.name))?;
            Ok((theme, colors))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    emit(&entries, &colors, &themed, provenance)
}

/// The palette under a theme, as `palette::finalize` computes it: the
/// theme's colors, references to colors the theme set earlier, then each
/// unset color from its fallback if the theme set that, else the default.
fn theme_colors<'a>(
    entries: &'a [Entry],
    defaults: &HashMap<&'a str, Rgba>,
    source: &str,
) -> anyhow::Result<HashMap<&'a str, Rgba>> {
    let mut loaded: HashMap<String, Rgba> = HashMap::new();
    for entry in parse(source)? {
        if !defaults.contains_key(entry.name.as_str()) {
            continue;
        }
        let color = match &entry.value {
            Value::Color(color) => Some(*color),
            Value::Reference(target) => loaded.get(target).copied(),
        };
        if let Some(color) = color {
            loaded.insert(entry.name, color);
        }
    }
    for entry in entries {
        if loaded.contains_key(&entry.name) {
            continue;
        }
        if let Some(color) = entry.fallback.as_ref().and_then(|name| loaded.get(name)) {
            loaded.insert(entry.name.clone(), *color);
        }
    }
    entries
        .iter()
        .map(|entry| {
            let color = loaded
                .get(&entry.name)
                .or_else(|| defaults.get(entry.name.as_str()))
                .ok_or_else(|| anyhow!("`{}` was not resolved", entry.name))?;
            Ok((entry.name.as_str(), *color))
        })
        .collect()
}

/// The color names, in palette order; style files refer to them.
pub(crate) fn names(source: &str) -> anyhow::Result<Vec<String>> {
    Ok(parse(source)?.into_iter().map(|entry| entry.name).collect())
}

fn parse(source: &str) -> anyhow::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let entry = parse_entry(line).with_context(|| format!("line {}: {line}", index + 1))?;
        entries.push(entry);
    }
    Ok(entries)
}

fn parse_entry(line: &str) -> anyhow::Result<Entry> {
    let (definition, comment) = line.split_once("//").unwrap_or((line, ""));
    let definition = definition
        .trim()
        .strip_suffix(';')
        .ok_or_else(|| anyhow!("missing `;`"))?;
    let (name, value) = definition
        .split_once(':')
        .ok_or_else(|| anyhow!("missing `:`"))?;
    let name = name.trim();
    ensure!(is_identifier(name), "invalid color name `{name}`");
    // The fallback after `|` only matters to themes that omit this color.
    let (value, fallback) = match value.split_once('|') {
        Some((value, fallback)) => (value.trim(), Some(fallback.trim().to_owned())),
        None => (value.trim(), None),
    };
    let value = match value.strip_prefix('#') {
        Some(hex) => Value::Color(parse_hex(hex)?),
        None if is_identifier(value) => Value::Reference(value.to_owned()),
        None => bail!("invalid value `{value}`"),
    };
    Ok(Entry {
        name: name.to_owned(),
        value,
        fallback,
        comment: comment.trim().to_owned(),
    })
}

fn parse_hex(hex: &str) -> anyhow::Result<Rgba> {
    ensure!(
        matches!(hex.len(), 6 | 8) && hex.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid color `#{hex}`"
    );
    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16);
    let alpha = if hex.len() == 8 { channel(6)? } else { 0xff };
    Ok(Rgba([channel(0)?, channel(2)?, channel(4)?, alpha]))
}

fn is_identifier(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_alphabetic())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Resolves references. tdesktop only refers to colors defined earlier.
fn resolve(entries: &[Entry]) -> anyhow::Result<HashMap<&str, Rgba>> {
    let mut colors = HashMap::with_capacity(entries.len());
    for entry in entries {
        let color = match &entry.value {
            Value::Color(color) => *color,
            Value::Reference(target) => *colors
                .get(target.as_str())
                .ok_or_else(|| anyhow!("`{}` refers to undefined `{target}`", entry.name))?,
        };
        ensure!(
            colors.insert(entry.name.as_str(), color).is_none(),
            "`{}` is defined twice",
            entry.name
        );
    }
    Ok(colors)
}

type Themed<'a, 'b> = (&'b Theme<'b>, HashMap<&'a str, Rgba>);

fn emit(
    entries: &[Entry],
    colors: &HashMap<&str, Rgba>,
    themes: &[Themed<'_, '_>],
    provenance: &str,
) -> anyhow::Result<String> {
    let mut out = String::new();
    writeln!(out, "// @generated by tools/style-gen from {provenance}.")?;
    writeln!(
        out,
        "// Derived from Telegram Desktop (GPLv3); see NOTICE.md. Do not edit."
    )?;
    writeln!(out)?;
    writeln!(out, "use crate::Color;")?;
    writeln!(out)?;
    writeln!(
        out,
        "/// Telegram Desktop's color palette (`colors.palette`)."
    )?;
    writeln!(out, "#[derive(Debug, Clone, PartialEq, Eq)]")?;
    writeln!(out, "pub struct Palette {{")?;
    let mut fields = HashSet::new();
    for entry in entries {
        let field = snake_case(&entry.name);
        ensure!(fields.insert(field.clone()), "two colors map to `{field}`");
        if !entry.comment.is_empty() {
            writeln!(out, "    /// {}", entry.comment)?;
        }
        if let Value::Reference(target) = &entry.value {
            if !entry.comment.is_empty() {
                writeln!(out, "    ///")?;
            }
            writeln!(out, "    /// Same as `{target}` by default.")?;
        }
        writeln!(out, "    pub {field}: Color,")?;
    }
    writeln!(out, "}}")?;
    writeln!(out)?;
    writeln!(out, "impl Palette {{")?;
    emit_constant(
        &mut out,
        "DEFAULT",
        "The default (day) palette.",
        entries,
        colors,
    )?;
    for (theme, colors) in themes {
        writeln!(out)?;
        emit_constant(&mut out, theme.name, theme.doc, entries, colors)?;
    }
    writeln!(out)?;
    writeln!(out, "    /// The color `id` names.")?;
    writeln!(out, "    #[must_use]")?;
    writeln!(out, "    pub const fn get(&self, id: ColorId) -> Color {{")?;
    writeln!(out, "        match id {{")?;
    for entry in entries {
        writeln!(
            out,
            "            ColorId::{} => self.{},",
            pascal_case(&entry.name),
            snake_case(&entry.name)
        )?;
    }
    writeln!(out, "        }}")?;
    writeln!(out, "    }}")?;
    writeln!(out, "}}")?;
    writeln!(out)?;
    writeln!(
        out,
        "/// Names a palette color; style values refer to colors by it, so themes apply."
    )?;
    writeln!(out, "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]")?;
    writeln!(out, "pub enum ColorId {{")?;
    for entry in entries {
        writeln!(out, "    {},", pascal_case(&entry.name))?;
    }
    writeln!(out, "}}")?;
    Ok(out)
}

fn emit_constant(
    out: &mut String,
    name: &str,
    doc: &str,
    entries: &[Entry],
    colors: &HashMap<&str, Rgba>,
) -> anyhow::Result<()> {
    writeln!(out, "    /// {doc}")?;
    writeln!(out, "    pub const {name}: Self = Self {{")?;
    for entry in entries {
        let Rgba([r, g, b, a]) = *colors
            .get(entry.name.as_str())
            .ok_or_else(|| anyhow!("`{}` was not resolved", entry.name))?;
        writeln!(
            out,
            "        {}: Color::rgba(0x{r:02x}, 0x{g:02x}, 0x{b:02x}, 0x{a:02x}),",
            snake_case(&entry.name)
        )?;
    }
    writeln!(out, "    }};")?;
    Ok(())
}

/// `windowBg` → `WindowBg`.
fn pascal_case(name: &str) -> String {
    crate::style::emit::pascal_case(name)
}

/// `windowBgOver` → `window_bg_over`.
fn snake_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            out.push('_');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "\
// basic
windowBg: #ffffff; // white: fallback for background
windowFgOver: windowBg; // reference to an earlier color
shadowFg: #00000018; // with opacity
historyPeer1UserpicBg2: #d45246 | windowBg; // value with a theme fallback
";

    #[test]
    fn generates_the_palette_struct() {
        let code = generate(FIXTURE, &[], "fixture").unwrap();
        insta::assert_snapshot!(code);
    }

    #[test]
    fn themes_fall_back_to_set_colors_then_defaults() {
        let entries = parse(FIXTURE).unwrap();
        let defaults = resolve(&entries).unwrap();
        let theme = "windowBg: #111111;\nshadowFg: windowBg;\nunknownKey: #222222;\n";
        let colors = theme_colors(&entries, &defaults, theme).unwrap();
        let dark = Rgba([0x11, 0x11, 0x11, 0xff]);
        assert_eq!(colors["windowBg"], dark);
        assert_eq!(colors["shadowFg"], dark);
        // Unset with a set fallback: the fallback; without one: the default.
        assert_eq!(colors["historyPeer1UserpicBg2"], dark);
        assert_eq!(colors["windowFgOver"], Rgba([0xff, 0xff, 0xff, 0xff]));
    }

    #[test]
    fn rejects_references_to_later_colors() {
        let error = generate("a: b;\nb: #000000;\n", &[], "fixture").unwrap_err();
        assert!(error.to_string().contains("undefined `b`"), "{error}");
    }

    #[test]
    fn rejects_malformed_colors() {
        let error = generate("a: #12345;\n", &[], "fixture").unwrap_err();
        assert!(
            format!("{error:#}").contains("invalid color `#12345`"),
            "{error:#}"
        );
    }

    #[test]
    fn rejects_duplicates() {
        let error = generate("a: #000000;\na: #ffffff;\n", &[], "fixture").unwrap_err();
        assert!(error.to_string().contains("defined twice"), "{error}");
    }
}
