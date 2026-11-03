//! Links clang's runtime under the static FFmpeg on macOS: its VideoToolbox
//! code uses `@available`, whose `__isPlatformVersionAtLeast` lives in
//! `libclang_rt.osx.a`, and rustc links with `-nodefaultlibs`.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    let is_static = env::var_os("CARGO_FEATURE_STATIC_FFMPEG").is_some();
    let is_macos = env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "macos");
    if !(is_static && is_macos) {
        return;
    }
    let output = Command::new("clang")
        .arg("--print-resource-dir")
        .output()
        .expect("clang --print-resource-dir runs (Xcode command line tools)");
    assert!(
        output.status.success(),
        "clang --print-resource-dir failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let resource_dir = String::from_utf8(output.stdout).expect("clang prints a UTF-8 path");
    let lib_dir = PathBuf::from(resource_dir.trim()).join("lib/darwin");
    assert!(
        lib_dir.join("libclang_rt.osx.a").is_file(),
        "libclang_rt.osx.a is missing in {}",
        lib_dir.display()
    );
    println!("cargo::rustc-link-search=native={}", lib_dir.display());
    println!("cargo::rustc-link-lib=static=clang_rt.osx");
}
