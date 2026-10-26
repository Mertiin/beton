//! Window surface, sprite pipeline, and present.

mod texture;

pub use texture::{Gfx, TextureHandle, TextureId};

use std::sync::Arc;

use wgpu::util::DeviceExt;

use crate::core::math::Mat4;
use crate::core::scene::Sprite;

/// Unit-quad corner: local xy + corner uv (0/1).
const FLOATS_PER_CORNER: usize = 4;
/// Per-instance: center3 + size2 + rotation1 + uv4 + color4.
const FLOATS_PER_INSTANCE: usize = 14;

/// Why a frame was not presented.
pub(crate) enum FrameError {
    Retry,
    Reconfigure,
    Lost,
}

pub(crate) struct Renderer {
    _window: Arc<winit::window::Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    ui_camera_buffer: wgpu::Buffer,
    ui_camera_bind_group: wgpu::BindGroup,
    /// Static unit quad (4 corners).
    quad_vertex_buffer: wgpu::Buffer,
    /// Static indices for one quad (6).
    quad_index_buffer: wgpu::Buffer,
    /// Dynamic per-sprite instance data (world, then UI appended).
    instance_buffer: wgpu::Buffer,
    instance_capacity: u64,
    depth_view: wgpu::TextureView,
    _depth_texture: wgpu::Texture,
}

impl Renderer {
    pub(crate) async fn new(window: Arc<winit::window::Window>) -> Result<(Self, Gfx), String> {
        let mut instance_desc = wgpu::InstanceDescriptor::new_without_display_handle();
        instance_desc.backends = wgpu::Backends::all();
        let instance = wgpu::Instance::new(instance_desc);

        let surface = instance
            .create_surface(window.clone())
            .map_err(|err| format!("failed to create a surface: {err}"))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .map_err(|err| {
                format!(
                    "failed to find a GPU adapter ({err}). A Vulkan driver is required on this machine."
                )
            })?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("beton"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await
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
            label: Some("sprite"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("camera"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
            label: Some("sprite"),
            bind_group_layouts: &[
                Some(&camera_bind_group_layout),
                Some(&texture_bind_group_layout),
            ],
            immediate_size: 0,
        });

        let float_size = std::mem::size_of::<f32>() as u64;
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprite"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[
                    // Slot 0: unit quad corners (per vertex).
                    Some(wgpu::VertexBufferLayout {
                        array_stride: FLOATS_PER_CORNER as u64 * float_size,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 0,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                            wgpu::VertexAttribute {
                                offset: 2 * float_size,
                                shader_location: 1,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                        ],
                    }),
                    // Slot 1: per-sprite instance.
                    Some(wgpu::VertexBufferLayout {
                        array_stride: FLOATS_PER_INSTANCE as u64 * float_size,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 2,
                                format: wgpu::VertexFormat::Float32x3,
                            },
                            wgpu::VertexAttribute {
                                offset: 3 * float_size,
                                shader_location: 3,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                            wgpu::VertexAttribute {
                                offset: 5 * float_size,
                                shader_location: 4,
                                format: wgpu::VertexFormat::Float32,
                            },
                            wgpu::VertexAttribute {
                                offset: 6 * float_size,
                                shader_location: 5,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                            wgpu::VertexAttribute {
                                offset: 10 * float_size,
                                shader_location: 6,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                        ],
                    }),
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });
        let ui_camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui-camera"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let ui_camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ui-camera"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: ui_camera_buffer.as_entire_binding(),
            }],
        });

        // Unit quad: top-left, top-right, bottom-right, bottom-left (Y-up, v=0 at top).
        let unit_quad: [f32; 16] = [
            -0.5, 0.5, 0.0, 0.0, // TL
            0.5, 0.5, 1.0, 0.0, // TR
            0.5, -0.5, 1.0, 1.0, // BR
            -0.5, -0.5, 0.0, 1.0, // BL
        ];
        let quad_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("unit-quad"),
            contents: bytemuck::cast_slice(&unit_quad),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let quad_indices: [u32; 6] = [0, 1, 2, 0, 2, 3];
        let quad_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("unit-quad-indices"),
            contents: bytemuck::cast_slice(&quad_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let instance_capacity = 64;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite-instances"),
            size: instance_capacity,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let (depth_texture, depth_view) =
            create_depth(&device, config.width, config.height);

        let image_format = if config.format.is_srgb() {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        };

        let gfx = Gfx::new(
            device.clone(),
            queue.clone(),
            image_format,
            texture_bind_group_layout.clone(),
        );

        Ok((
            Self {
                _window: window,
                surface,
                device,
                queue,
                config,
                pipeline,
                camera_buffer,
                camera_bind_group,
                ui_camera_buffer,
                ui_camera_bind_group,
                quad_vertex_buffer,
                quad_index_buffer,
                instance_buffer,
                instance_capacity,
                depth_view,
                _depth_texture: depth_texture,
            },
            gfx,
        ))
    }

    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        let (depth_texture, depth_view) = create_depth(&self.device, width, height);
        self._depth_texture = depth_texture;
        self.depth_view = depth_view;
    }

    pub(crate) fn aspect(&self) -> f32 {
        self.config.width as f32 / self.config.height.max(1) as f32
    }

    pub(crate) fn render(
        &mut self,
        view_proj: Mat4,
        sprites: &[Sprite],
        ui_view_proj: Mat4,
        ui_sprites: &[Sprite],
        gfx: &Gfx,
    ) -> Result<(), FrameError> {
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

        self.queue
            .write_buffer(&self.camera_buffer, 0, view_proj.as_bytes());
        self.queue
            .write_buffer(&self.ui_camera_buffer, 0, ui_view_proj.as_bytes());

        let (world_floats, world_ranges) = pack_instances(sprites);
        let (ui_floats, ui_ranges) = pack_instances(ui_sprites);
        let world_count = (world_floats.len() / FLOATS_PER_INSTANCE) as u32;
        let mut floats = world_floats;
        floats.extend_from_slice(&ui_floats);
        let byte_len = (floats.len() * std::mem::size_of::<f32>()) as u64;
        if byte_len > self.instance_capacity {
            let new_cap = byte_len.next_power_of_two().max(64);
            self.instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("sprite-instances"),
                size: new_cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_capacity = new_cap;
        }
        if !floats.is_empty() {
            self.queue
                .write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&floats));
        }

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
                label: Some("sprites"),
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
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
            pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            pass.set_index_buffer(self.quad_index_buffer.slice(..), wgpu::IndexFormat::Uint32);

            for (texture, start, count) in world_ranges {
                let Some(bind_group) = gfx.bind_group(texture) else {
                    continue;
                };
                pass.set_bind_group(1, bind_group, &[]);
                pass.draw_indexed(0..6, 0, start..(start + count));
            }
        }

        if !ui_sprites.is_empty() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.ui_camera_bind_group, &[]);
            pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
            pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            pass.set_index_buffer(self.quad_index_buffer.slice(..), wgpu::IndexFormat::Uint32);

            for (texture, start, count) in ui_ranges {
                let Some(bind_group) = gfx.bind_group(texture) else {
                    continue;
                };
                pass.set_bind_group(1, bind_group, &[]);
                let offset_start = world_count + start;
                pass.draw_indexed(0..6, 0, offset_start..(offset_start + count));
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        self.queue.present(frame);
        Ok(())
    }
}

fn create_depth(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth24Plus,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

/// Pack sprites into instance floats and consecutive texture batch ranges.
fn pack_instances(sprites: &[Sprite]) -> (Vec<f32>, Vec<(TextureId, u32, u32)>) {
    let mut floats = Vec::with_capacity(sprites.len() * FLOATS_PER_INSTANCE);
    let mut ranges = Vec::new();
    let mut i = 0u32;
    while i < sprites.len() as u32 {
        let texture = sprites[i as usize].texture;
        let start = i;
        while i < sprites.len() as u32 && sprites[i as usize].texture == texture {
            push_instance(&mut floats, &sprites[i as usize]);
            i += 1;
        }
        ranges.push((texture, start, i - start));
    }
    (floats, ranges)
}

fn push_instance(out: &mut Vec<f32>, sprite: &Sprite) {
    let c = sprite.center;
    out.extend_from_slice(&[c.x, c.y, c.z]);
    out.extend_from_slice(&sprite.size);
    out.push(sprite.rotation);
    out.extend_from_slice(&sprite.uv);
    out.extend_from_slice(&sprite.color);
}

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
