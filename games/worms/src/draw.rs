//! UV helpers. `SpriteSheet::uv` insets every cell, so sheets use this instead.

pub fn grid_uv(col: u32, cols: u32, row: u32, rows: u32, flip_x: bool) -> [f32; 4] {
    let cu = 1.0 / cols.max(1) as f32;
    let cv = 1.0 / rows.max(1) as f32;
    let u0 = col as f32 * cu;
    let u1 = u0 + cu;
    let v0 = row as f32 * cv;
    let v1 = v0 + cv;
    if flip_x {
        [u1, v0, u0, v1]
    } else {
        [u0, v0, u1, v1]
    }
}

pub fn team_color(team: u8) -> [f32; 4] {
    if team == 0 {
        [0.93, 0.28, 0.22, 1.0]
    } else {
        [0.30, 0.48, 0.95, 1.0]
    }
}

pub fn hand_col(name: &str) -> u32 {
    if name.contains("grenade") {
        1
    } else if name.contains("shotgun") {
        2
    } else if name.contains("cluster") {
        5
    } else {
        0
    }
}

/// props.png is 96×48: tree (0,0,32,48), cloud (32,0,32,16), grave (32,32,16,16),
/// aim cursor (64,0,32,32), power blob (64,32,32,16).
pub const PROPS_W: f32 = 96.0;
pub const PROPS_H: f32 = 48.0;

pub fn prop_uv(x: f32, y: f32, w: f32, h: f32) -> [f32; 4] {
    [
        x / PROPS_W,
        y / PROPS_H,
        (x + w) / PROPS_W,
        (y + h) / PROPS_H,
    ]
}

pub fn aim_uv() -> [f32; 4] {
    prop_uv(64.0, 0.0, 32.0, 32.0)
}

pub fn power_uv() -> [f32; 4] {
    prop_uv(64.0, 32.0, 32.0, 16.0)
}

pub fn proj_draw(kind: u8) -> (u32, [f32; 2]) {
    match kind {
        1 => (1, [7.0, 7.0]),       // grenade
        2 => (4, [3.0, 3.0]),       // pellet
        3 => (5, [8.0, 8.0]),       // cluster
        _ => (4, [6.0, 6.0]),       // bazooka shell
    }
}
