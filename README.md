# brutal-pixels

A native window that decodes one image and draws it with `winit` and `wgpu`.

The file is expanded to tightly packed RGBA8 in system RAM. `Queue::write_texture` copies those bytes into a staging allocation once. The first `queue.submit` fills the GPU texture. Later frames only sample it. A resize rewrites the four quad vertices so the image stays letterboxed. The event loop sleeps until the window system asks for a frame.

## Requirements

Rust 1.88 or newer:

```bash
rustup update stable
```

On Linux the build needs `pkg-config` and the X11, xkbcommon, and Wayland development packages. Running also needs `libxkbcommon-x11` and a Vulkan driver (`mesa-vulkan-drivers` is enough, including its software renderer).

## Run

From the repository root:

```bash
cargo run --release
```

That loads `assets/sample.png`. Pass another PNG or JPEG to draw that instead:

```bash
cargo run --release -- path/to/photo.png
```

Escape or the window's close button exits.
