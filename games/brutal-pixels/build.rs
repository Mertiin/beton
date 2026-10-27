//! Copy `assets/` next to the built executable (`target/<profile>/assets`).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let src = manifest_dir.join("assets");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    // OUT_DIR is `<target>/<profile>/build/<pkg>-<hash>/out`.
    let build_dir = out_dir
        .parent()
        .and_then(Path::parent)
        .expect("OUT_DIR has a build directory");
    assert_eq!(
        build_dir.file_name().and_then(|name| name.to_str()),
        Some("build"),
        "unexpected OUT_DIR layout: {}",
        out_dir.display()
    );
    let dest = build_dir
        .parent()
        .expect("profile directory")
        .join("assets");

    println!("cargo:rerun-if-changed=assets");
    if src.is_dir() {
        copy_dir(&src, &dest).unwrap_or_else(|err| {
            panic!(
                "failed to copy {} to {}: {err}",
                src.display(),
                dest.display()
            )
        });
    }
}

fn copy_dir(src: &Path, dest: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let to = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&path, &to)?;
        } else {
            fs::copy(&path, &to)?;
        }
    }
    Ok(())
}
