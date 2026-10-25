//! Generates the TDLib API bindings from `tl/td_api.tl` into `OUT_DIR`.

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    if let Err(message) = run() {
        println!("cargo::error={message}");
    }
}

fn run() -> Result<(), String> {
    let schema_path = "tl/td_api.tl";
    println!("cargo::rerun-if-changed={schema_path}");
    let schema =
        fs::read_to_string(schema_path).map_err(|e| format!("reading {schema_path}: {e}"))?;
    let code = td_api_gen::generate(&schema).map_err(|e| format!("{schema_path}: {e}"))?;
    let out_dir = env::var("OUT_DIR").map_err(|e| format!("OUT_DIR: {e}"))?;
    let out_path = PathBuf::from(out_dir).join("td_api.rs");
    fs::write(&out_path, code).map_err(|e| format!("writing {}: {e}", out_path.display()))
}
