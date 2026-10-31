//! Weapon file parser and component.

use beton::{Blend, ShaderId};

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
}

impl Weapon {
    /// Parse `key = value` lines. `name` is the file stem.
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
        other => Err(format!("{name}: unknown blend `{other}` (use alpha or add)")),
    }
}

fn require<T>(name: &str, key: &str, value: Option<T>) -> Result<T, String> {
    value.ok_or_else(|| format!("{name}: missing `{key}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cannon_block() {
        let w = Weapon::parse(
            "cannon",
            "blast_radius = 18\ndamage = 45\nspeed = 220\ngravity = 1\ntrail = 40\nfragments = 16\nfragment_speed = 90\nfragment_life = 0.5\neffect = glow\nblend = add\n",
        )
        .expect("parse");
        assert_eq!(w.name, "cannon");
        assert_eq!(w.blast_radius, 18.0);
        assert_eq!(w.fragments, 16);
        assert!(matches!(w.blend, Blend::Add));
        assert_eq!(w.effect, "glow");
    }

    #[test]
    fn parse_checked_in_files() {
        for name in ["cannon", "mortar", "needle"] {
            let path = format!(
                "{}/assets/weapons/{name}.txt",
                env!("CARGO_MANIFEST_DIR")
            );
            let text = std::fs::read_to_string(&path).expect(name);
            let w = Weapon::parse(name, &text).expect(name);
            assert!(!w.effect.is_empty());
        }
    }
}
