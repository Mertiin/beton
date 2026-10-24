//! Browser implementations of the platform API.

use std::future::Future;

use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event_loop::EventLoop;
use winit::platform::web::{EventLoopExtWebSys, WindowAttributesExtWebSys};
use winit::window::WindowAttributes;

pub(super) fn install_panic_hook() {
    console_error_panic_hook::set_once();
}

pub(super) fn warn(msg: &str) {
    web_sys::console::warn_1(&msg.into());
}

pub(super) async fn read_bytes(path: &str) -> Result<Vec<u8>, String> {
    let window = web_sys::window().ok_or_else(|| "no window".to_string())?;
    let resp_value = JsFuture::from(window.fetch_with_str(path))
        .await
        .map_err(|err| format!("fetch {path}: {err:?}"))?;
    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|_| format!("fetch {path}: bad response"))?;
    if !resp.ok() {
        return Err(format!("fetch {path}: HTTP {}", resp.status()));
    }
    let buf = JsFuture::from(
        resp.array_buffer()
            .map_err(|err| format!("fetch {path}: array_buffer: {err:?}"))?,
    )
    .await
    .map_err(|err| format!("fetch {path}: body: {err:?}"))?;
    Ok(js_sys::Uint8Array::new(&buf).to_vec())
}

pub(super) fn window_attributes(title: &str, width: u32, height: u32) -> WindowAttributes {
    WindowAttributes::default()
        .with_title(title)
        .with_inner_size(LogicalSize::new(width, height))
        .with_append(true)
}

pub(super) fn schedule<F>(fut: F)
where
    F: Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(fut);
}

pub(super) fn run_loop<T, A>(event_loop: EventLoop<T>, app: A) -> Result<(), String>
where
    T: 'static,
    A: ApplicationHandler<T> + 'static,
{
    event_loop.spawn_app(app);
    Ok(())
}

pub(super) fn open_audio_at_boot() -> bool {
    false
}

pub(super) fn is_audio_gesture(pressed_key_or_mouse: bool) -> bool {
    pressed_key_or_mouse
}

pub(super) fn exit_on_escape() -> bool {
    false
}
