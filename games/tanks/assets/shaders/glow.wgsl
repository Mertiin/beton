struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) local_uv: vec2<f32>,
}

struct Time {
    seconds: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

@group(2) @binding(0) var<uniform> time: Time;

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let p = in.local_uv * 2.0 - 1.0;
    let d = length(p);
    let pulse = 0.85 + 0.15 * sin(time.seconds * 8.0);
    let a = saturate(1.0 - d) * saturate(1.0 - d) * in.color.a * pulse;
    return vec4<f32>(in.color.rgb, a);
}
