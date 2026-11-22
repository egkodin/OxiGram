//! Links `tdjson` from the directory `scripts/build-tdlib.sh` installs into.
//!
//! The runtime library is also copied into `OUT_DIR`, which Cargo adds to the
//! dynamic library search path for `cargo run` and `cargo test`. That directory
//! is published as `DEP_TDJSON_RUNTIME_DIR`, so the app's build script can put
//! the library next to the executable.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    if let Err(message) = run() {
        println!("cargo::error={message}");
    }
}

fn run() -> Result<(), String> {
    let manifest_dir = PathBuf::from(var("CARGO_MANIFEST_DIR")?);
    let pin_dir = manifest_dir.join("../../tdlib");
    let commit = read_trimmed(&pin_dir.join("COMMIT"))?;
    let version = read_trimmed(&pin_dir.join("VERSION"))?;

    println!("cargo::rerun-if-env-changed=OXIGRAM_TDLIB_DIR");
    let install_dir = match env::var_os("OXIGRAM_TDLIB_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => default_install_dir(&commit)?,
    };

    let marker = install_dir.join(".oxigram-tdlib-commit");
    println!("cargo::rerun-if-changed={}", marker.display());
    let built_commit = read_trimmed(&marker).map_err(|_| {
        format!(
            "TDLib not found in {}; run scripts/build-tdlib.sh (or set OXIGRAM_TDLIB_DIR)",
            install_dir.display()
        )
    })?;
    if built_commit != commit {
        return Err(format!(
            "TDLib in {} was built from {built_commit}, but tdlib/COMMIT pins {commit}; \
             run scripts/build-tdlib.sh",
            install_dir.display()
        ));
    }

    println!(
        "cargo::rustc-link-search=native={}",
        install_dir.join("lib").display()
    );
    println!("cargo::rustc-link-lib=dylib=tdjson");

    let runtime_dir = PathBuf::from(var("OUT_DIR")?).join("tdlib");
    fs::create_dir_all(&runtime_dir)
        .map_err(|e| format!("creating {}: {e}", runtime_dir.display()))?;
    for file in runtime_files(&install_dir, &version)? {
        println!("cargo::rerun-if-changed={}", file.display());
        let name = file.file_name().ok_or("runtime file without a name")?;
        link_or_copy(&file, &runtime_dir.join(name))?;
    }
    println!("cargo::rustc-link-search=native={}", runtime_dir.display());
    println!("cargo::metadata=runtime_dir={}", runtime_dir.display());
    Ok(())
}

/// Files the dynamic loader needs at run time.
fn runtime_files(install_dir: &Path, version: &str) -> Result<Vec<PathBuf>, String> {
    match var("CARGO_CFG_TARGET_OS")?.as_str() {
        "macos" => Ok(vec![
            install_dir.join(format!("lib/libtdjson.{version}.dylib")),
        ]),
        // tdjson.dll plus the OpenSSL and zlib DLLs the build script copies next to it.
        "windows" => {
            let bin = install_dir.join("bin");
            let entries =
                fs::read_dir(&bin).map_err(|e| format!("reading {}: {e}", bin.display()))?;
            let mut dlls = Vec::new();
            for entry in entries {
                let path = entry
                    .map_err(|e| format!("reading {}: {e}", bin.display()))?
                    .path();
                if path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("dll"))
                {
                    dlls.push(path);
                }
            }
            Ok(dlls)
        }
        _ => Ok(vec![
            install_dir.join(format!("lib/libtdjson.so.{version}")),
        ]),
    }
}

fn default_install_dir(commit: &str) -> Result<PathBuf, String> {
    // Same default as scripts/build-tdlib.sh: $HOME/.cache/oxigram/tdlib-<short commit>.
    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .ok_or("neither HOME nor USERPROFILE is set; set OXIGRAM_TDLIB_DIR")?;
    let short = commit
        .get(..9)
        .ok_or("tdlib/COMMIT is shorter than 9 characters")?;
    Ok(PathBuf::from(home).join(format!(".cache/oxigram/tdlib-{short}")))
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

fn read_trimmed(path: &Path) -> Result<String, String> {
    println!("cargo::rerun-if-changed={}", path.display());
    fs::read_to_string(path)
        .map(|s| s.trim().to_owned())
        .map_err(|e| format!("reading {}: {e}", path.display()))
}

fn var(name: &str) -> Result<String, String> {
    env::var(name).map_err(|e| format!("{name}: {e}"))
}
