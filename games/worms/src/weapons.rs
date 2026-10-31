//! Weapon file parser.

use beton::{Blend, ShaderId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Behavior {
    Contact,
    Fuse,
    Pellets,
    Cluster,
    Banana,
}

/// Stats loaded from one `assets/weapons/*.txt` file.
#[derive(Clone, Debug)]
pub struct Weapon {
    pub name: String,
    pub blast_radius: f32,
    pub damage: f32,
    pub speed: f32,
    pub gravity: f32,
    pub trail: f32,
    pub fragments: u32,
    pub fragment_speed: f32,
    pub fragment_life: f32,
    pub effect: String,
    pub blend: Blend,
    pub shader: ShaderId,
    pub behavior: Behavior,
    pub ammo: u8,
    pub fuse: f32,
    pub pellets: u32,
    pub spread: f32,
    pub children: u32,
}

impl Weapon {
    pub fn parse(name: &str, text: &str) -> Result<Self, String> {
        let mut blast_radius = None;
        let mut damage = None;
        let mut speed = None;
        let mut gravity = None;
        let mut trail = None;
        let mut fragments = None;
        let mut fragment_speed = None;
        let mut fragment_life = None;
        let mut effect = None;
        let mut blend = None;
        let mut behavior = None;
        let mut ammo = None;
        let mut fuse = None;
        let mut pellets = None;
        let mut spread = None;
        let mut children = None;

        for (line_no, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(format!(
                    "{name}: line {}: expected key = value",
                    line_no + 1
                ));
            };
            let key = key.trim();
            let value = value.trim();
            match key {
                "blast_radius" => blast_radius = Some(parse_f32(name, key, value)?),
                "damage" => damage = Some(parse_f32(name, key, value)?),
                "speed" => speed = Some(parse_f32(name, key, value)?),
                "gravity" => gravity = Some(parse_f32(name, key, value)?),
                "trail" => trail = Some(parse_f32(name, key, value)?),
                "fragments" => fragments = Some(parse_u32(name, key, value)?),
                "fragment_speed" => fragment_speed = Some(parse_f32(name, key, value)?),
                "fragment_life" => fragment_life = Some(parse_f32(name, key, value)?),
                "effect" => effect = Some(value.to_string()),
                "blend" => blend = Some(parse_blend(name, value)?),
                "behavior" => behavior = Some(parse_behavior(name, value)?),
                "ammo" => ammo = Some(parse_u32(name, key, value)? as u8),
                "fuse" => fuse = Some(parse_f32(name, key, value)?),
                "pellets" => pellets = Some(parse_u32(name, key, value)?),
                "spread" => spread = Some(parse_f32(name, key, value)?),
                "children" => children = Some(parse_u32(name, key, value)?),
                other => return Err(format!("{name}: unknown key `{other}`")),
            }
        }

        Ok(Self {
            name: name.to_string(),
            blast_radius: require(name, "blast_radius", blast_radius)?,
            damage: require(name, "damage", damage)?,
            speed: require(name, "speed", speed)?,
            gravity: require(name, "gravity", gravity)?,
            trail: require(name, "trail", trail)?,
            fragments: require(name, "fragments", fragments)?,
            fragment_speed: require(name, "fragment_speed", fragment_speed)?,
            fragment_life: require(name, "fragment_life", fragment_life)?,
            effect: require(name, "effect", effect)?,
            blend: require(name, "blend", blend)?,
            shader: ShaderId::SPRITE,
            behavior: behavior.unwrap_or(Behavior::Contact),
            ammo: ammo.unwrap_or(5).min(9),
            fuse: fuse.unwrap_or(0.0),
            pellets: pellets.unwrap_or(1).max(1),
            spread: spread.unwrap_or(0.2),
            children: children.unwrap_or(0),
        })
    }
}

fn parse_f32(name: &str, key: &str, value: &str) -> Result<f32, String> {
    value
        .parse()
        .map_err(|_| format!("{name}: `{key}` is not a number ({value})"))
}

fn parse_u32(name: &str, key: &str, value: &str) -> Result<u32, String> {
    value
        .parse()
        .map_err(|_| format!("{name}: `{key}` is not an integer ({value})"))
}

fn parse_blend(name: &str, value: &str) -> Result<Blend, String> {
    match value {
        "alpha" => Ok(Blend::Alpha),
        "add" => Ok(Blend::Add),
        other => Err(format!("{name}: unknown blend `{other}`")),
    }
}

fn parse_behavior(name: &str, value: &str) -> Result<Behavior, String> {
    match value {
        "contact" => Ok(Behavior::Contact),
        "fuse" => Ok(Behavior::Fuse),
        "pellets" => Ok(Behavior::Pellets),
        "cluster" => Ok(Behavior::Cluster),
        "banana" => Ok(Behavior::Banana),
        other => Err(format!("{name}: unknown behavior `{other}`")),
    }
}

fn require<T>(name: &str, key: &str, value: Option<T>) -> Result<T, String> {
    value.ok_or_else(|| format!("{name}: missing `{key}`"))
}

/// Load every `*.txt` weapon file from a directory (dedicated / tests).
pub fn load_weapons_dir(dir: &std::path::Path) -> Result<Vec<Weapon>, String> {
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("read {}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            let is_txt = p
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("txt"))
                .unwrap_or(false);
            let is_index = p
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.eq_ignore_ascii_case("_index.txt"))
                .unwrap_or(false);
            is_txt && !is_index
        })
        .collect();
    paths.sort();
    if paths.is_empty() {
        return Err(format!("no weapons in {}", dir.display()));
    }
    let mut out = Vec::new();
    for path in paths {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("weapon")
            .to_string();
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{stem}: {e}"))?;
        out.push(Weapon::parse(&stem, &text)?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_checked_in_files() {
        for name in ["01bazooka", "02grenade", "03shotgun", "04cluster", "05banana"] {
            let path = format!("{}/assets/weapons/{name}.txt", env!("CARGO_MANIFEST_DIR"));
            let text = std::fs::read_to_string(&path).expect(name);
            let w = Weapon::parse(name, &text).expect(name);
            assert!(w.ammo > 0);
        }
    }
}
