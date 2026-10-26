//! Puts the TDLib runtime libraries next to the rgram executable and points the
//! executable's rpath at its own directory, so a built binary starts without
//! `cargo run`. Windows looks for DLLs next to the executable on its own.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    if let Err(message) = run() {
        println!("cargo::error={message}");
    }
}

fn run() -> Result<(), String> {
    let runtime_dir = PathBuf::from(var("DEP_TDJSON_RUNTIME_DIR")?);
    // OUT_DIR is <target>[/<triple>]/<profile>/build/rgram-<hash>/out.
    let out_dir = PathBuf::from(var("OUT_DIR")?);
    let exe_dir = out_dir
        .ancestors()
        .nth(3)
        .ok_or_else(|| format!("unexpected OUT_DIR layout: {}", out_dir.display()))?;

    let entries = fs::read_dir(&runtime_dir)
        .map_err(|e| format!("reading {}: {e}", runtime_dir.display()))?;
    for entry in entries {
        let file = entry
            .map_err(|e| format!("reading {}: {e}", runtime_dir.display()))?
            .path();
        println!("cargo::rerun-if-changed={}", file.display());
        let name = file.file_name().ok_or("runtime file without a name")?;
        link_or_copy(&file, &exe_dir.join(name))?;
    }

    match var("CARGO_CFG_TARGET_OS")?.as_str() {
        "macos" => println!("cargo::rustc-link-arg-bins=-Wl,-rpath,@executable_path"),
        "windows" => {}
        _ => println!("cargo::rustc-link-arg-bins=-Wl,-rpath,$ORIGIN"),
    }
    Ok(())
}

fn link_or_copy(from: &Path, to: &Path) -> Result<(), String> {
    if to.exists() {
        fs::remove_file(to).map_err(|e| format!("removing {}: {e}", to.display()))?;
    }
    if fs::hard_link(from, to).is_ok() {
        return Ok(());
    }
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| format!("copying {} to {}: {e}", from.display(), to.display()))
}

fn var(name: &str) -> Result<String, String> {
    env::var(name).map_err(|e| format!("{name}: {e}"))
}
