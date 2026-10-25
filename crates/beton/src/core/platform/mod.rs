//! Desktop and web differences in one place.
//!
//! `#[cfg(target_arch = "wasm32")]` is allowed only in this file. Callers use the
//! functions below and never branch on the target.

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
mod native_net;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
mod web_net;

#[cfg(not(target_arch = "wasm32"))]
use native as backend;
#[cfg(target_arch = "wasm32")]
use web as backend;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native_net::NetDriver;
#[cfg(target_arch = "wasm32")]
pub(crate) use web_net::NetDriver;

use std::future::Future;

use winit::application::ApplicationHandler;
use winit::event_loop::EventLoop;
use winit::window::WindowAttributes;

pub use web_time::{Duration, Instant};

/// Route panics to the browser console on web. No-op on desktop.
pub(crate) fn install_panic_hook() {
    backend::install_panic_hook();
}

/// Log a warning (`eprintln!` on desktop, `console.warn` on web).
pub(crate) fn warn(msg: &str) {
    backend::warn(msg);
}

/// Window title, size, and web canvas attachment.
pub(crate) fn window_attributes(title: &str, width: u32, height: u32) -> WindowAttributes {
    backend::window_attributes(title, width, height)
}

/// Run `fut` to completion on the desktop; spawn it on the web event loop.
pub(crate) fn schedule<F>(fut: F)
where
    F: Future<Output = ()> + 'static,
{
    backend::schedule(fut);
}

/// Drive the event loop until the window closes (desktop) or schedule it (web).
///
/// On web this returns `Ok(())` as soon as the loop is scheduled. Later failures
/// are reported through [`warn`].
pub(crate) fn run_loop<T, A>(event_loop: EventLoop<T>, app: A) -> Result<(), String>
where
    T: 'static,
    A: ApplicationHandler<T> + 'static,
{
    backend::run_loop(event_loop, app)
}

/// Whether to open the audio device during GPU install (desktop yes, web no).
pub(crate) fn open_audio_at_boot() -> bool {
    backend::open_audio_at_boot()
}

/// Whether this press should open the audio device (web yes, desktop no).
pub(crate) fn is_audio_gesture(pressed_key_or_mouse: bool) -> bool {
    backend::is_audio_gesture(pressed_key_or_mouse)
}

/// Whether Escape should exit the event loop (desktop yes, web no).
pub(crate) fn exit_on_escape() -> bool {
    backend::exit_on_escape()
}

/// Read a file relative to the process (desktop) or page origin (web).
///
/// Desktop also tries the path beside the executable (e.g. `assets/…` from `build.rs`).
pub(crate) async fn read_bytes(path: &str) -> Result<Vec<u8>, String> {
    backend::read_bytes(path).await
}
