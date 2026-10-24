struct Camera {
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> camera: Camera;

struct VsIn {
    // Unit quad corner in local space (x,y) and corner UV in 0..1 (v=0 is top).
    @location(0) corner: vec2<f32>,
    @location(1) corner_uv: vec2<f32>,
    // Per-instance sprite.
    @location(2) center: vec3<f32>,
    @location(3) size: vec2<f32>,
    @location(4) rotation: f32,
    @location(5) uv_rect: vec4<f32>,
    @location(6) color: vec4<f32>,
}

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs(in: VsIn) -> VsOut {
    let local = vec2<f32>(in.corner.x * in.size.x, in.corner.y * in.size.y);
    let c = cos(in.rotation);
    let s = sin(in.rotation);
    let world_xy = in.center.xy + vec2<f32>(
        local.x * c - local.y * s,
        local.x * s + local.y * c,
    );
    let u = in.uv_rect.x + in.corner_uv.x * (in.uv_rect.z - in.uv_rect.x);
    let v = in.uv_rect.y + in.corner_uv.y * (in.uv_rect.w - in.uv_rect.y);

    var out: VsOut;
    out.clip = camera.view_proj * vec4<f32>(world_xy, in.center.z, 1.0);
    out.uv = vec2<f32>(u, v);
    out.color = in.color;
    return out;
}

@group(1) @binding(0) var image_tex: texture_2d<f32>;
@group(1) @binding(1) var image_sampler: sampler;

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(image_tex, image_sampler, in.uv) * in.color;
}
