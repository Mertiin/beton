//! Game-authored sprite pipelines: fragment (and optional vertex) WGSL.

use crate::core::render::texture::Gfx;

/// Generational handle to a registered sprite shader. Slot 0 is the built-in textured sprite.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ShaderId(pub(crate) u32);

impl ShaderId {
    /// Built-in textured sprite shader (`Blend::Alpha`).
    pub const SPRITE: Self = Self(0);

    pub(crate) fn index(self) -> u32 {
        self.0
    }
}

/// How the pipeline blends into the framebuffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blend {
    /// Standard alpha blending.
    Alpha,
    /// Additive blending (good for glows).
    Add,
}

/// Which vertex stage a custom shader uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VertexShader {
    /// Engine `vs` that places the unit quad and writes `uv`, `color`, `local_uv`.
    Engine,
    /// User `@vertex fn vs` in the same WGSL string. Must read the same attributes.
    Custom,
}

/// Arguments to [`Gfx::create_shader`](Gfx::create_shader).
#[derive(Clone, Copy, Debug)]
pub struct ShaderDesc<'a> {
    /// WGSL source. Must define `@fragment fn fs`. With [`VertexShader::Custom`], also `@vertex fn vs`.
    pub wgsl: &'a str,
    /// Framebuffer blend mode.
    pub blend: Blend,
    /// Engine vertex shader, or a custom one in `wgsl`.
    pub vertex: VertexShader,
    /// Size of the group-2 uniform buffer in bytes. `0` means no group 2.
    /// When non-zero, must be a multiple of 16.
    pub uniform_bytes: u32,
}

pub(crate) struct ShaderSlot {
    pub pipeline: wgpu::RenderPipeline,
    pub uniform: Option<(wgpu::Buffer, wgpu::BindGroup)>,
    pub uniform_size: u32,
}

impl Gfx {
    pub(crate) fn install_sprite_pipeline(
        &mut self,
        pipeline: wgpu::RenderPipeline,
    ) {
        debug_assert!(self.shaders.is_empty());
        self.shaders.push(ShaderSlot {
            pipeline,
            uniform: None,
            uniform_size: 0,
        });
    }

    /// Compile a sprite shader. Returns [`Err`] if WGSL fails validation.
    pub fn create_shader(&mut self, desc: ShaderDesc<'_>) -> Result<ShaderId, String> {
        if desc.uniform_bytes != 0 && desc.uniform_bytes % 16 != 0 {
            return Err(format!(
                "uniform_bytes ({}) must be 0 or a multiple of 16",
                desc.uniform_bytes
            ));
        }

        let fragment_module = self.create_validated_module("game-fragment", desc.wgsl)?;
        let vertex_module = match desc.vertex {
            VertexShader::Engine => self
                .engine_shader
                .as_ref()
                .ok_or_else(|| "engine shader module is missing".to_string())?,
            VertexShader::Custom => &fragment_module,
        };

        let blend = match desc.blend {
            Blend::Alpha => wgpu::BlendState::ALPHA_BLENDING,
            Blend::Add => wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            },
        };

        let layout = if desc.uniform_bytes > 0 {
            self.pipeline_layout_uniform
                .as_ref()
                .ok_or_else(|| "pipeline layout missing".to_string())?
        } else {
            self.pipeline_layout
                .as_ref()
                .ok_or_else(|| "pipeline layout missing".to_string())?
        };

        let float_size = std::mem::size_of::<f32>() as u64;
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("sprite-shader"),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: vertex_module,
                    entry_point: Some("vs"),
                    buffers: &[
                        Some(wgpu::VertexBufferLayout {
                            array_stride: 4 * float_size,
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
                        Some(wgpu::VertexBufferLayout {
                            array_stride: 14 * float_size,
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
                    module: &fragment_module,
                    entry_point: Some("fs"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: self.surface_format,
                        blend: Some(blend),
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

        let uniform = if desc.uniform_bytes > 0 {
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("shader-uniform"),
                size: desc.uniform_bytes as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("shader-uniform"),
                layout: self
                    .uniform_bind_group_layout
                    .as_ref()
                    .ok_or_else(|| "uniform layout missing".to_string())?,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            });
            Some((buffer, bind_group))
        } else {
            None
        };

        let id = ShaderId(self.shaders.len() as u32);
        self.shaders.push(ShaderSlot {
            pipeline,
            uniform,
            uniform_size: desc.uniform_bytes,
        });
        Ok(id)
    }

    /// Upload bytes into a shader's group-2 uniform buffer.
    pub fn write_uniform(&mut self, id: ShaderId, bytes: &[u8]) -> Result<(), String> {
        let Some(slot) = self.shaders.get(id.index() as usize) else {
            return Err("unknown shader id".into());
        };
        let Some((buffer, _)) = slot.uniform.as_ref() else {
            return Err("shader has no uniform buffer".into());
        };
        if bytes.len() as u32 != slot.uniform_size {
            return Err(format!(
                "uniform expected {} bytes, got {}",
                slot.uniform_size,
                bytes.len()
            ));
        }
        self.queue.write_buffer(buffer, 0, bytes);
        Ok(())
    }

    pub(crate) fn shader_pipeline(&self, id: ShaderId) -> Option<&wgpu::RenderPipeline> {
        self.shaders.get(id.index() as usize).map(|s| &s.pipeline)
    }

    pub(crate) fn shader_uniform_bind_group(&self, id: ShaderId) -> Option<&wgpu::BindGroup> {
        self.shaders
            .get(id.index() as usize)
            .and_then(|s| s.uniform.as_ref().map(|(_, bg)| bg))
    }

    fn create_validated_module(
        &self,
        label: &str,
        wgsl: &str,
    ) -> Result<wgpu::ShaderModule, String> {
        let scope = self
            .device
            .push_error_scope(wgpu::ErrorFilter::Validation);
        let module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(wgsl.into()),
            });
        let _ = self.device.poll(wgpu::PollType::Poll);
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(err) = pollster::block_on(scope.pop()) {
                return Err(format!("shader compile failed: {err}"));
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            // Drop the scope without awaiting; web validation surfaces on first draw.
            drop(scope);
        }
        Ok(module)
    }
}
