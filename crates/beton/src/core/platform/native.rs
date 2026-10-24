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
