//! Copy `assets/` next to the built executable and write `_index.txt` for scanned folders.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let src = manifest_dir.join("assets");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
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
    copy_dir(&src, &dest).unwrap_or_else(|err| {
        panic!(
            "failed to copy {} to {}: {err}",
            src.display(),
            dest.display()
        )
    });
    write_index(&src.join("weapons"));
    write_index(&src.join("shaders"));
    write_index(&dest.join("weapons"));
    write_index(&dest.join("shaders"));
}

fn write_index(dir: &Path) {
    if !dir.is_dir() {
        return;
    }
    let mut names = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if name == "_index.txt" {
                continue;
            }
            names.push(name.to_string());
        }
    }
    names.sort();
    let body = names.join("\n") + "\n";
    let path = dir.join("_index.txt");
    let unchanged = fs::read_to_string(&path)
        .map(|existing| existing == body)
        .unwrap_or(false);
    if !unchanged {
        let _ = fs::write(path, body);
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
