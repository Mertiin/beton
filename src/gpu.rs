//! Window surface, shader, and the one draw call.
//!
//! The quad vertices live in a GPU buffer. A resize rewrites those 64 bytes.
//! It does not touch the image texture.

use std::sync::Arc;

use wgpu::util::DeviceExt;

use crate::image_upload::UploadedImage;

/// One corner of the letterboxed quad. Clip position plus texture coordinate.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
}

const SHADER: &str = r#"
struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs(@location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>) -> VsOut {
    var out: VsOut;
    out.clip = vec4<f32>(pos, 0.0, 1.0);
    out.uv = uv;
    return out;
}

@group(0) @binding(0) var image_tex: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(image_tex, image_sampler, in.uv);
}
"#;

/// Device, swapchain, and the pipeline that samples the uploaded image.
pub struct Gpu {
    /// Keeps the `winit` window alive. The surface holds a raw handle into it.
    _window: Arc<winit::window::Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    image_width: u32,
    image_height: u32,
    // Owned for the process lifetime. The bind group also retains them.
    _texture: wgpu::Texture,
    _sampler: wgpu::Sampler,
}

impl Gpu {
    /// Open a device compatible with `window` and build the quad pipeline.
    ///
    /// The image is not uploaded here. The caller does that once, then
    /// [`Self::set_image`] installs the bind group.
    pub fn new(window: Arc<winit::window::Window>) -> Result<GpuParts, String> {
        let mut instance_desc = wgpu::InstanceDescriptor::new_without_display_handle();
        instance_desc.backends = wgpu::Backends::all();
        let instance = wgpu::Instance::new(instance_desc);

        // Safety of the handle: `window` is an `Arc` stored in `Gpu` for the
        // whole life of the surface.
        let surface = instance
            .create_surface(window.clone())
            .map_err(|err| format!("failed to create a surface: {err}"))?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .map_err(|err| {
            format!(
                "failed to find a GPU adapter ({err}). A Vulkan driver is required on this machine."
            )
        })?;

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("brutal-pixels"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: Default::default(),
            trace: wgpu::Trace::Off,
        }))
        .map_err(|err| format!("failed to open the GPU device: {err}"))?;

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = choose_surface_format(&caps.formats)?;
        let color_space = if caps
            .color_spaces(format)
            .contains(wgpu::SurfaceColorSpaces::SRGB)
        {
            wgpu::SurfaceColorSpace::Srgb
        } else {
            wgpu::SurfaceColorSpace::Auto
        };
        let alpha = caps
            .alpha_modes
            .first()
            .copied()
            .ok_or_else(|| "surface exposes no alpha modes".to_string())?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: alpha,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("quad"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("image"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("quad"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("quad"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: std::mem::size_of::<[f32; 2]>() as u64,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
                })],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("quad-vertices"),
            size: std::mem::size_of::<[Vertex; 4]>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Two triangles: TL, TR, BR and TL, BR, BL.
        const INDICES: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("quad-indices"),
            contents: bytemuck::cast_slice(&INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        Ok(GpuParts {
            window,
            surface,
            device,
            queue,
            config,
            pipeline,
            bind_group_layout,
            vertex_buffer,
            index_buffer,
        })
    }
}

/// Device and pipeline before the image bind group exists.
pub struct GpuParts {
    window: Arc<winit::window::Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
}

impl GpuParts {
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Texture format that matches the swapchain's color space.
    ///
    /// A non-sRGB surface stores the file's 8-bit values unchanged, so the
    /// texture is `Rgba8Unorm`. An sRGB surface encodes on store, so the
    /// texture is `Rgba8UnormSrgb` and the hardware decode on sample cancels
    /// that encode.
    pub fn image_format(&self) -> wgpu::TextureFormat {
        if self.config.format.is_srgb() {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        }
    }

    pub fn set_image(self, image: UploadedImage) -> Gpu {
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("image"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&image.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&image.sampler),
                },
            ],
        });

        Gpu {
            _window: self.window,
            surface: self.surface,
            device: self.device,
            queue: self.queue,
            config: self.config,
            pipeline: self.pipeline,
            bind_group,
            vertex_buffer: self.vertex_buffer,
            index_buffer: self.index_buffer,
            image_width: image.width,
            image_height: image.height,
            _texture: image.texture,
            _sampler: image.sampler,
        }
    }
}

impl Gpu {
    pub fn image_size(&self) -> (u32, u32) {
        (self.image_width, self.image_height)
    }

    /// Replace the four quad corners. 64 bytes, no texture traffic.
    pub fn write_quad(&self, vertices: &[Vertex; 4]) {
        self.queue
            .write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(vertices));
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Record one pass and present. The first call also kicks the texture
    /// upload that `write_texture` staged at startup.
    pub fn render(&mut self) -> Result<(), FrameError> {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Err(FrameError::Retry);
            }
            wgpu::CurrentSurfaceTexture::Outdated => return Err(FrameError::Reconfigure),
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Validation => {
                return Err(FrameError::Lost);
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear-and-quad"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.08,
                            g: 0.08,
                            b: 0.09,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..6, 0, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        self.queue.present(frame);
        Ok(())
    }
}

/// Why a frame was not presented.
pub enum FrameError {
    /// Acquire timed out, or the window is covered. Ask for another frame later.
    Retry,
    /// The swapchain size is stale. Reconfigure, then draw again.
    Reconfigure,
    /// The surface is gone. Leave the process.
    Lost,
}

/// Prefer an 8-bit non-sRGB swapchain so the shader is a straight copy.
fn choose_surface_format(formats: &[wgpu::TextureFormat]) -> Result<wgpu::TextureFormat, String> {
    for wanted in [
        wgpu::TextureFormat::Bgra8Unorm,
        wgpu::TextureFormat::Rgba8Unorm,
    ] {
        if formats.contains(&wanted) {
            return Ok(wanted);
        }
    }
    formats
        .iter()
        .copied()
        .find(|format| format.is_srgb())
        .or_else(|| formats.first().copied())
        .ok_or_else(|| "surface exposes no formats".to_string())
}
