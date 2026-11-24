use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

fn main() -> io::Result<()> {
    println!("cargo::rerun-if-env-changed=OXIGRAM_PAYLOAD");
    let payload = if let Some(path) = env::var_os("OXIGRAM_PAYLOAD") {
        let path = PathBuf::from(path).canonicalize()?;
        println!("cargo::rerun-if-changed={}", path.display());
        path
    } else {
        // Keep regular workspace builds and tests independent of release assets.
        let path =
            PathBuf::from(env::var("OUT_DIR").map_err(io::Error::other)?).join("empty-payload");
        fs::write(&path, [])?;
        path
    };
    println!("cargo::rustc-env=OXIGRAM_PAYLOAD={}", payload.display());
    Ok(())
}
