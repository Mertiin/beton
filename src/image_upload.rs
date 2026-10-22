//! Disk bytes to a GPU texture.
//!
//! The file on disk stays compressed. `decode_rgba8` expands it into an owned
//! RGBA8 buffer in system RAM. `upload_texture` hands that buffer to
//! `Queue::write_texture`, which copies it into a staging allocation before
//! the function returns. The GPU texture itself is filled on the next
//! `queue.submit` (the first frame in `gpu::Gpu::render`).

use std::path::Path;

/// Tightly packed RGBA8 texels. Row 0 is the top of the image.
/// `rgba.len() == width * height * 4`.
pub struct CpuImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// A GPU texture plus the view and sampler the fragment shader binds.
///
/// `wgpu` handles are reference counted. The `Texture` is stored so the
/// allocation is obviously owned by the renderer for the rest of the process.
pub struct UploadedImage {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub width: u32,
    pub height: u32,
}

/// Read `path` and expand it to RGBA8 in system RAM.
pub fn decode_rgba8(path: &Path) -> Result<CpuImage, String> {
    let decoded =
        image::open(path).map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let rgba = decoded.to_rgba8();
    let (width, height) = rgba.dimensions();
    if width == 0 || height == 0 {
        return Err(format!("{} has a zero dimension", path.display()));
    }

    // `into_raw` moves the decoder's buffer out. Four bytes per pixel,
    // R, G, B, A, row-major, top to bottom. No row padding.
    let rgba = rgba.into_raw();
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| format!("{} is too large to address", path.display()))?;
    if rgba.len() != expected {
        return Err(format!(
            "{} decoded to {} bytes, expected {expected}",
            path.display(),
            rgba.len()
        ));
    }

    Ok(CpuImage {
        width,
        height,
        rgba,
    })
}

/// Allocate a 2D texture and copy `image` into staging memory.
///
/// `format` must match the surface: `Rgba8Unorm` when the swapchain is
/// non-sRGB, `Rgba8UnormSrgb` when it is sRGB. The copy is a straight
/// transfer of the bytes above; the format only tells the sampler whether
/// to treat them as already-displayed sRGB.
pub fn upload_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    image: CpuImage,
    format: wgpu::TextureFormat,
) -> Result<UploadedImage, String> {
    let CpuImage {
        width,
        height,
        rgba,
    } = image;

    let max_edge = device.limits().max_texture_dimension_2d;
    if width > max_edge || height > max_edge {
        return Err(format!(
            "image is {width}x{height}; this GPU allows textures up to {max_edge} on a side"
        ));
    }

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("image"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        // COPY_DST receives the upload. TEXTURE_BINDING lets the shader sample it.
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    // `bytes_per_row` is the tight CPU stride (`width * 4`). `write_texture`
    // is allowed to use that stride. It copies `rgba` into a staging
    // allocation before returning, and pads that staging buffer to
    // `COPY_BYTES_PER_ROW_ALIGNMENT` (256) itself. The 256-byte rule is for
    // `copy_buffer_to_texture`, which we do not call.
    //
    // The GPU texture is still empty until the next `queue.submit`.
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &rgba,
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

    // Staging owns the texels now. The CPU buffer can go.
    drop(rgba);

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("linear-clamp"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });

    Ok(UploadedImage {
        texture,
        view,
        sampler,
        width,
        height,
    })
}
