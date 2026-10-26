//! GPU texture store: register on load, upload on first draw, unload after a pause.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

type FileBytes = Arc<HashMap<PathBuf, Vec<u8>>>;

use crate::core::asset::{AssetLifetime, ResolveAction, UNUSED_GRACE_SECS};

/// Generational handle to a registered texture. Stale after the store is cleared.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureId(pub(crate) u32, pub(crate) u32);

impl TextureId {
    /// Build an id from a raw index (generation 0). Prefer ids returned by [`Gfx`].
    pub const fn from_raw(id: u32) -> Self {
        Self(id, 0)
    }

    fn index(self) -> u32 {
        self.0
    }

    fn generation(self) -> u32 {
        self.1
    }
}

/// Keeps a texture resident while alive, even when nothing draws it.
#[derive(Clone, Debug)]
pub struct TextureHandle {
    id: TextureId,
    _pin: Arc<()>,
}

impl TextureHandle {
    /// Id to store in sprites and sheets.
    pub fn id(&self) -> TextureId {
        self.id
    }
}

pub(crate) struct GpuTexture {
    pub width: u32,
    pub height: u32,
    pub bind_group: wgpu::BindGroup,
    _texture: wgpu::Texture,
    _view: wgpu::TextureView,
    _sampler: wgpu::Sampler,
}

enum TextureSource {
    Path(PathBuf),
    Rgba {
        width: u32,
        height: u32,
        bytes: Vec<u8>,
    },
}

struct TextureSlot {
    source: TextureSource,
    generation: u32,
    life: AssetLifetime,
    resident: Option<GpuTexture>,
    /// Cached pixel size for path sources (header read without a full upload).
    cached_size: Option<(u32, u32)>,
}

/// Loads and creates textures. Opaque to game code beyond its methods.
pub struct Gfx {
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    bind_group_layout: wgpu::BindGroupLayout,
    slots: Vec<TextureSlot>,
    by_path: HashMap<PathBuf, TextureId>,
    files: Option<FileBytes>,
    /// 1×1 white pixel, pinned for UI backgrounds and solid rects.
    white: TextureHandle,
}

impl Gfx {
    pub(crate) fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        bind_group_layout: wgpu::BindGroupLayout,
    ) -> Self {
        let mut gfx = Self {
            device,
            queue,
            format,
            bind_group_layout,
            slots: Vec::new(),
            by_path: HashMap::new(),
            files: None,
            // Placeholder replaced immediately below.
            white: TextureHandle {
                id: TextureId::from_raw(0),
                _pin: Arc::new(()),
            },
        };
        let white_id = gfx
            .create_texture(1, 1, &[255, 255, 255, 255])
            .expect("white texture");
        gfx.white = gfx.pin(white_id).expect("pin white texture");
        gfx
    }

    /// 1×1 white texture for tinted rects and UI backgrounds.
    pub fn white_texture(&self) -> TextureId {
        self.white.id()
    }

    /// Attach bytes preloaded by the app (from [`Game::assets`](crate::Game::assets)).
    pub(crate) fn install_files(&mut self, files: FileBytes) {
        self.files = Some(files);
    }

    /// Read a preloaded asset path, or the filesystem on desktop.
    pub fn read_bytes(&self, path: &Path) -> Result<Vec<u8>, String> {
        let path = path.to_path_buf();
        if let Some(bytes) = self.files.as_ref().and_then(|f| f.get(&path).cloned()) {
            return Ok(bytes);
        }
        std::fs::read(&path).map_err(|err| format!("failed to read {}: {err}", path.display()))
    }

    /// Register a PNG or JPEG path. Checks that the file exists; does not decode yet.
    ///
    /// Uses preloaded [`Game::assets`](crate::Game::assets) bytes when present (required on
    /// the web). Otherwise reads from the filesystem. The same path returns the same id.
    pub fn load_texture(&mut self, path: &Path) -> Result<TextureId, String> {
        let path = path.to_path_buf();
        if let Some(&id) = self.by_path.get(&path) {
            return Ok(id);
        }
        if let Some(bytes) = self.files.as_ref().and_then(|f| f.get(&path).cloned()) {
            let id = self.load_texture_bytes(&bytes)?;
            self.by_path.insert(path, id);
            return Ok(id);
        }
        std::fs::metadata(&path).map_err(|err| {
            format!("failed to read {}: {err}", path.display())
        })?;
        let id = self.alloc_slot(TextureSource::Path(path.clone()));
        self.by_path.insert(path, id);
        Ok(id)
    }

    /// Register a PNG or JPEG from memory. Decodes now; GPU upload waits until first use.
    pub fn load_texture_bytes(&mut self, bytes: &[u8]) -> Result<TextureId, String> {
        let decoded = image::load_from_memory(bytes)
            .map_err(|err| format!("failed to decode image: {err}"))?;
        let rgba = decoded.to_rgba8();
        let (width, height) = rgba.dimensions();
        self.create_texture(width, height, &rgba.into_raw())
    }

    /// Register tightly packed RGBA8 pixels (`len == width * height * 4`).
    ///
    /// Bytes are kept so a later unload can re-upload. GPU upload waits until first use.
    pub fn create_texture(
        &mut self,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<TextureId, String> {
        validate_rgba(width, height, rgba, self.device.limits().max_texture_dimension_2d)?;
        Ok(self.alloc_slot(TextureSource::Rgba {
            width,
            height,
            bytes: rgba.to_vec(),
        }))
    }

    /// Pin a texture so it stays resident until the handle is dropped.
    pub fn pin(&self, id: TextureId) -> Option<TextureHandle> {
        let slot = self.slots.get(id.index() as usize)?;
        if slot.generation != id.generation() {
            return None;
        }
        Some(TextureHandle {
            id,
            _pin: slot.life.pin().into_arc(),
        })
    }

    /// Upload any texture referenced this frame that is not resident yet.
    /// Call **before** draw so bind groups exist.
    pub(crate) fn ensure_resident(&mut self, used: &HashSet<TextureId>) {
        let mut to_upload = Vec::new();
        for &id in used {
            let Some(slot) = self.slots.get_mut(id.index() as usize) else {
                continue;
            };
            if slot.generation != id.generation() {
                continue;
            }
            if slot.resident.is_some() {
                slot.life.mark_resident();
            } else {
                to_upload.push(id.index() as usize);
            }
        }
        for index in 0..self.slots.len() {
            if !self.slots[index].life.is_pinned() {
                continue;
            }
            if self.slots[index].resident.is_some() {
                self.slots[index].life.mark_resident();
            } else if !to_upload.contains(&index) {
                to_upload.push(index);
            }
        }
        for index in to_upload {
            if self.slots[index].resident.is_some() {
                continue;
            }
            if let Err(err) = self.upload_slot(index) {
                crate::core::platform::warn(&format!(
                    "texture: failed to load #{index}: {err}"
                ));
                self.slots[index].life.mark_unloaded();
            }
        }
    }

    /// Advance unused timers and drop GPU textures past the grace period.
    /// Call **after** present so in-flight draws keep valid bind groups.
    pub(crate) fn reap(&mut self, used: &HashSet<TextureId>, dt: f32) {
        let _ = UNUSED_GRACE_SECS;
        for index in 0..self.slots.len() {
            let id = TextureId(index as u32, self.slots[index].generation);
            let referenced = used.contains(&id);
            let action = self.slots[index].life.resolve(referenced, dt);
            match action {
                ResolveAction::Keep | ResolveAction::Load => {}
                ResolveAction::Unload => {
                    self.slots[index].resident = None;
                    self.slots[index].life.mark_unloaded();
                }
            }
        }
    }

    /// Number of textures currently on the GPU.
    pub fn live_count(&self) -> usize {
        self.slots.iter().filter(|s| s.resident.is_some()).count()
    }

    /// Approximate RAM/VRAM for resident textures (RGBA8: `width * height * 4` each).
    pub fn resident_bytes(&self) -> usize {
        self.slots
            .iter()
            .filter_map(|s| s.resident.as_ref())
            .map(|t| (t.width as usize).saturating_mul(t.height as usize).saturating_mul(4))
            .sum()
    }

    /// CPU bytes kept so an unloaded `create_texture` source can re-upload.
    /// Path-backed textures contribute 0 (only the path string is retained).
    pub fn retained_source_bytes(&self) -> usize {
        self.slots
            .iter()
            .map(|s| match &s.source {
                TextureSource::Path(_) => 0,
                TextureSource::Rgba { bytes, .. } => bytes.len(),
            })
            .sum()
    }

    /// Pixel size of a texture, or `(0, 0)` if unknown.
    ///
    /// Prefers the resident GPU size, then RGBA source dimensions, then a cached
    /// path header read (without uploading).
    pub fn texture_size(&mut self, id: TextureId) -> (u32, u32) {
        let Some(slot) = self.slots.get(id.index() as usize) else {
            return (0, 0);
        };
        if slot.generation != id.generation() {
            return (0, 0);
        }
        if let Some(gpu) = slot.resident.as_ref() {
            return (gpu.width, gpu.height);
        }
        match &slot.source {
            TextureSource::Rgba { width, height, .. } => (*width, *height),
            TextureSource::Path(_) => {
                if let Some(size) = slot.cached_size {
                    return size;
                }
                let size = self.read_path_size(id.index() as usize);
                if let Some(slot) = self.slots.get_mut(id.index() as usize) {
                    slot.cached_size = size;
                }
                size.unwrap_or((0, 0))
            }
        }
    }

    fn read_path_size(&self, index: usize) -> Option<(u32, u32)> {
        let path = match &self.slots.get(index)?.source {
            TextureSource::Path(path) => path.clone(),
            TextureSource::Rgba { .. } => return None,
        };
        if let Some(bytes) = self.files.as_ref().and_then(|f| f.get(&path)) {
            return image::ImageReader::new(std::io::Cursor::new(bytes.as_slice()))
                .with_guessed_format()
                .ok()?
                .into_dimensions()
                .ok();
        }
        image::image_dimensions(&path).ok()
    }

    pub(crate) fn bind_group(&self, id: TextureId) -> Option<&wgpu::BindGroup> {
        self.slots
            .get(id.index() as usize)
            .filter(|s| s.generation == id.generation())
            .and_then(|s| s.resident.as_ref())
            .map(|t| &t.bind_group)
    }

    fn alloc_slot(&mut self, source: TextureSource) -> TextureId {
        let cached_size = match &source {
            TextureSource::Rgba { width, height, .. } => Some((*width, *height)),
            TextureSource::Path(_) => None,
        };
        let index = self.slots.len() as u32;
        self.slots.push(TextureSlot {
            source,
            generation: 0,
            life: AssetLifetime::new(),
            resident: None,
            cached_size,
        });
        TextureId(index, 0)
    }

    fn upload_slot(&mut self, index: usize) -> Result<(), String> {
        let (width, height, bytes) = match &self.slots[index].source {
            TextureSource::Path(path) => {
                let decoded = image::open(path)
                    .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
                let rgba = decoded.to_rgba8();
                let (width, height) = rgba.dimensions();
                (width, height, rgba.into_raw())
            }
            TextureSource::Rgba {
                width,
                height,
                bytes,
            } => (*width, *height, bytes.clone()),
        };
        validate_rgba(
            width,
            height,
            &bytes,
            self.device.limits().max_texture_dimension_2d,
        )?;
        let gpu = self.upload_rgba(width, height, &bytes)?;
        self.slots[index].resident = Some(gpu);
        self.slots[index].cached_size = Some((width, height));
        self.slots[index].life.mark_resident();
        Ok(())
    }

    fn upload_rgba(&self, width: u32, height: u32, rgba: &[u8]) -> Result<GpuTexture, String> {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("image"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("nearest-clamp"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("image"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Ok(GpuTexture {
            width,
            height,
            bind_group,
            _texture: texture,
            _view: view,
            _sampler: sampler,
        })
    }
}

fn validate_rgba(width: u32, height: u32, rgba: &[u8], max_edge: u32) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("texture has a zero dimension".into());
    }
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| "texture is too large to address".to_string())?;
    if rgba.len() != expected {
        return Err(format!(
            "texture expected {expected} bytes, got {}",
            rgba.len()
        ));
    }
    if width > max_edge || height > max_edge {
        return Err(format!(
            "image is {width}x{height}; this GPU allows textures up to {max_edge} on a side"
        ));
    }
    Ok(())
}

