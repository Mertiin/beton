//! Desktop implementations of the platform API.

use std::future::Future;
use std::path::Path;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event_loop::EventLoop;
use winit::window::WindowAttributes;

pub(super) fn install_panic_hook() {}

pub(super) fn warn(msg: &str) {
    eprintln!("{msg}");
}

pub(super) async fn read_bytes(path: &str) -> Result<Vec<u8>, String> {
    if let Ok(bytes) = std::fs::read(path) {
        return Ok(bytes);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let beside = dir.join(path);
            if let Ok(bytes) = std::fs::read(&beside) {
                return Ok(bytes);
            }
            return Err(format!(
                "failed to read {} (also tried {})",
                path,
                beside.display()
            ));
        }
    }
    let _ = Path::new(path);
    Err(format!("failed to read {path}"))
}

/// List files under `dir` as relative paths (`dir/name`). Skips nested directories.
pub(super) async fn list_asset_dir(dir: &str) -> Result<Vec<String>, String> {
    let root = resolve_dir(dir)?;
    let mut out = Vec::new();
    let entries = std::fs::read_dir(&root)
        .map_err(|err| format!("failed to read dir {}: {err}", root.display()))?;
    for entry in entries {
        let entry =
            entry.map_err(|err| format!("failed to read dir {}: {err}", root.display()))?;
        if !entry
            .file_type()
            .map(|t| t.is_file())
            .unwrap_or(false)
        {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name == "_index.txt" {
            continue;
        }
        out.push(format!("{}/{}", dir.trim_end_matches('/'), name));
    }
    out.sort();
    Ok(out)
}

fn resolve_dir(dir: &str) -> Result<std::path::PathBuf, String> {
    let path = Path::new(dir);
    if path.is_dir() {
        return Ok(path.to_path_buf());
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let beside = parent.join(dir);
            if beside.is_dir() {
                return Ok(beside);
            }
            return Err(format!(
                "failed to open dir {} (also tried {})",
                dir,
                beside.display()
            ));
        }
    }
    Err(format!("failed to open dir {dir}"))
}

pub(super) fn window_attributes(title: &str, width: u32, height: u32) -> WindowAttributes {
    WindowAttributes::default()
        .with_title(title)
        .with_inner_size(LogicalSize::new(width, height))
}

pub(super) fn schedule<F>(fut: F)
where
    F: Future<Output = ()> + 'static,
{
    pollster::block_on(fut);
}

pub(super) fn run_loop<T, A>(event_loop: EventLoop<T>, mut app: A) -> Result<(), String>
where
    T: 'static,
    A: ApplicationHandler<T> + 'static,
{
    event_loop
        .run_app(&mut app)
        .map_err(|err| format!("event loop stopped: {err}"))
}

pub(super) fn open_audio_at_boot() -> bool {
    true
}

pub(super) fn is_audio_gesture(_pressed_key_or_mouse: bool) -> bool {
    false
}

pub(super) fn exit_on_escape() -> bool {
    true
}
