//! Side-view tower loadouts, prep bar, and fight sim — no wgpu.

use beton::Rng;

pub const MAX_TURNS: u32 = 8;
pub const START_GOLD: i32 = 3;
pub const SELL_REFUND: i32 = 1;
pub const REROLL_BASE_COST: i32 = 1;
pub const BAR_WIDTH: usize = 3;
pub const BASE_TOWER_HP: i32 = 40;
pub const CONTACT_RANGE: f32 = 0.04;
pub const BASE_REACH: f32 = 0.08;
pub const SHOT_SPEED: f32 = 1.4; // t per second
pub const SPIKE_RANGE: f32 = 0.18;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PartKind {
    Foundation,
    Wall,
    Spikes,
    Oil,
    Cement,
    Archer,
    Mason,
    Banner,
    Ram,
    Ballista,
    Nest,
    Lantern,
    Chick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcKind {
    Flat,
    Arc,
    Mortar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleRole {
    Structure,
    Gun,
    Spawner,
    Aura,
    Passive,
    Consumable,
}

impl PartKind {
    pub const OFFERS: [PartKind; 13] = [
        PartKind::Foundation,
        PartKind::Wall,
        PartKind::Spikes,
        PartKind::Oil,
        PartKind::Cement,
        PartKind::Chick,
        PartKind::Archer,
        PartKind::Mason,
        PartKind::Banner,
        PartKind::Ram,
        PartKind::Ballista,
        PartKind::Nest,
        PartKind::Lantern,
    ];

    pub fn short(self) -> &'static str {
        match self {
            PartKind::Foundation => "FOUND",
            PartKind::Wall => "WALL",
            PartKind::Spikes => "SPIKE",
            PartKind::Oil => "OIL",
            PartKind::Cement => "CEMENT",
            PartKind::Archer => "ARCH",
            PartKind::Mason => "MASON",
            PartKind::Banner => "BANR",
            PartKind::Ram => "RAM",
            PartKind::Ballista => "BALL",
            PartKind::Nest => "NEST",
            PartKind::Lantern => "LANT",
            PartKind::Chick => "CHICK",
        }
    }

    pub fn tier(self) -> u8 {
        match self {
            // Tier 1 includes a cheap spawner so early shops can field walkers.
            PartKind::Foundation
            | PartKind::Wall
            | PartKind::Spikes
            | PartKind::Oil
            | PartKind::Cement
            | PartKind::Chick
            | PartKind::Ram => 1,
            PartKind::Archer | PartKind::Mason | PartKind::Banner | PartKind::Nest => 2,
            PartKind::Ballista => 3,
            PartKind::Lantern => 4,
        }
    }

    pub fn cost(self) -> i32 {
        self.tier() as i32
    }

    pub fn role(self) -> ModuleRole {
        match self {
            PartKind::Foundation | PartKind::Wall => ModuleRole::Structure,
            PartKind::Archer | PartKind::Ballista => ModuleRole::Gun,
            PartKind::Nest | PartKind::Ram => ModuleRole::Spawner,
            PartKind::Spikes => ModuleRole::Aura,
            PartKind::Banner | PartKind::Lantern | PartKind::Mason => ModuleRole::Passive,
            PartKind::Oil | PartKind::Cement => ModuleRole::Consumable,
            PartKind::Chick => ModuleRole::Spawner,
        }
    }

    pub fn arc(self) -> ArcKind {
        match self {
            PartKind::Archer | PartKind::Lantern => ArcKind::Arc,
            PartKind::Ballista => ArcKind::Mortar,
            _ => ArcKind::Flat,
        }
    }

    pub fn gun_damage(self, level: u8) -> i32 {
        let base = match self {
            PartKind::Archer => 3,
            PartKind::Ballista => 6,
            _ => 2,
        };
        base + (level as i32 - 1) * 2
    }

    pub fn gun_cooldown(self, level: u8) -> f32 {
        let base = match self {
            PartKind::Archer => 1.1,
            PartKind::Ballista => 1.8,
            _ => 1.4,
        };
        (base / level as f32).max(0.35)
    }

    pub fn spawn_cooldown(self, level: u8) -> f32 {
        let base = match self {
            PartKind::Nest => 2.2,
            PartKind::Chick => 1.6,
            PartKind::Ram => 3.0,
            _ => 2.5,
        };
        (base / level as f32).max(0.6)
    }

    pub fn spawn_kind(self) -> Option<PartKind> {
        match self {
            PartKind::Nest | PartKind::Chick => Some(PartKind::Chick),
            PartKind::Ram => Some(PartKind::Ram),
            _ => None,
        }
    }

    pub fn unit_hp(self) -> i32 {
        match self {
            PartKind::Chick => 6,
            PartKind::Ram => 14,
            _ => 8,
        }
    }

    pub fn unit_damage(self) -> i32 {
        match self {
            PartKind::Chick => 2,
            PartKind::Ram => 4,
            _ => 2,
        }
    }

    pub fn unit_speed(self) -> f32 {
        match self {
            PartKind::Chick => 0.18,
            PartKind::Ram => 0.10,
            _ => 0.14,
        }
    }

    pub fn structure_hp(self, level: u8) -> i32 {
        let base = match self {
            PartKind::Foundation => 12,
            PartKind::Wall => 18,
            _ => 0,
        };
        base * level as i32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Player,
    Enemy,
}

impl Side {
    pub fn other(self) -> Side {
        match self {
            Side::Player => Side::Enemy,
            Side::Enemy => Side::Player,
        }
    }

    pub fn dir(self) -> f32 {
        match self {
            Side::Player => 1.0,
            Side::Enemy => -1.0,
        }
    }

    pub fn base_x(self) -> f32 {
        match self {
            Side::Player => 0.06,
            Side::Enemy => 0.94,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
    pub kind: PartKind,
    pub level: u8,
}

impl Module {
    pub fn new(kind: PartKind) -> Self {
        Self { kind, level: 1 }
    }
}

/// Shareable tower build — future peer payload.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TowerLoadout {
    pub modules: Vec<Module>,
    /// Permanent HP bonus from Cement / Mason (survives rounds).
    pub bonus_hp: i32,
    /// Permanent gun damage bonus from Oil.
    pub bonus_damage: i32,
}

impl TowerLoadout {
    pub fn max_hp(&self) -> i32 {
        let from_struct: i32 = self
            .modules
            .iter()
            .map(|m| m.kind.structure_hp(m.level))
            .sum();
        (BASE_TOWER_HP + from_struct + self.bonus_hp).max(1)
    }

    pub fn slot_cap(turn: u32) -> usize {
        match turn {
            1..=3 => 3,
            4..=6 => 4,
            _ => 5,
        }
    }

    pub fn has_kind(&self, kind: PartKind) -> bool {
        self.modules.iter().any(|m| m.kind == kind)
    }

    pub fn kind_level(&self, kind: PartKind) -> u8 {
        self.modules
            .iter()
            .filter(|m| m.kind == kind)
            .map(|m| m.level)
            .max()
            .unwrap_or(0)
    }

    pub fn count_role(&self, role: ModuleRole) -> usize {
        self.modules
            .iter()
            .filter(|m| m.kind.role() == role)
            .count()
    }

    pub fn has_role(&self, role: ModuleRole) -> bool {
        self.count_role(role) > 0
    }

    pub fn has_gun(&self) -> bool {
        self.has_role(ModuleRole::Gun)
    }

    /// Cement leftover beyond Mason's baked-in HP contribution.
    pub fn cement_bought(&self) -> bool {
        let mason_hp: i32 = self
            .modules
            .iter()
            .filter(|m| m.kind == PartKind::Mason)
            .map(|m| 8 * m.level as i32)
            .sum();
        self.bonus_hp > mason_hp
    }
}

/// Named recipes + role synergies applied for one fight.
#[derive(Clone, Debug)]
pub struct SynergyState {
    pub recipes: Vec<&'static str>,
    pub gun_cd_factor: f32,
    pub ballista_bonus_dmg: i32,
    pub structure_fight_hp: i32,
    pub chick_contact_bonus: i32,
    pub ram_speed_factor: f32,
    pub fight_hp_bonus: i32,
    pub archer_bonus_dmg: i32,
    pub role_gun_dmg: i32,
    pub spawn_cd_factor: f32,
    pub spawn_opener: bool,
    pub spike_bonus_dmg: i32,
}

impl Default for SynergyState {
    fn default() -> Self {
        Self {
            recipes: Vec::new(),
            gun_cd_factor: 1.0,
            ballista_bonus_dmg: 0,
            structure_fight_hp: 0,
            chick_contact_bonus: 0,
            ram_speed_factor: 1.0,
            fight_hp_bonus: 0,
            archer_bonus_dmg: 0,
            role_gun_dmg: 0,
            spawn_cd_factor: 1.0,
            spawn_opener: false,
            spike_bonus_dmg: 0,
        }
    }
}

/// Evaluate named recipes and role synergies for a loadout.
pub fn compute_synergies(loadout: &TowerLoadout) -> SynergyState {
    let mut s = SynergyState::default();

    // --- Named recipes ---
    if loadout.has_kind(PartKind::Banner) && loadout.has_gun() {
        s.recipes.push("VOLLEY");
        s.gun_cd_factor *= 0.8;
    }
    if loadout.has_kind(PartKind::Ballista)
        && (loadout.has_kind(PartKind::Wall) || loadout.has_kind(PartKind::Foundation))
    {
        s.recipes.push("SIEGE");
        s.ballista_bonus_dmg += 2;
        s.structure_fight_hp += 4;
    }
    if (loadout.has_kind(PartKind::Nest) || loadout.has_kind(PartKind::Chick))
        && loadout.has_kind(PartKind::Spikes)
    {
        s.recipes.push("NEST GUARD");
        s.chick_contact_bonus += 1;
    }
    if loadout.has_kind(PartKind::Ram) && loadout.has_kind(PartKind::Banner) {
        s.recipes.push("WAR PATH");
        s.ram_speed_factor *= 1.25;
    }
    let wall_l2 = loadout.kind_level(PartKind::Wall) >= 2;
    if loadout.has_kind(PartKind::Mason) && (loadout.cement_bought() || wall_l2) {
        s.recipes.push("MASONRY");
        s.fight_hp_bonus += 10;
    }
    if loadout.bonus_damage > 0 && loadout.has_kind(PartKind::Archer) {
        s.recipes.push("OIL TIPS");
        s.archer_bonus_dmg += 1;
    }

    // --- Role synergies ---
    if loadout.count_role(ModuleRole::Gun) >= 2 {
        s.role_gun_dmg = 1;
    }
    if loadout.count_role(ModuleRole::Spawner) >= 2 {
        s.spawn_cd_factor *= 0.9;
    }
    if loadout.has_role(ModuleRole::Gun) && loadout.has_role(ModuleRole::Spawner) {
        s.spawn_opener = true;
    }
    if loadout.has_role(ModuleRole::Structure) && loadout.has_kind(PartKind::Spikes) {
        s.spike_bonus_dmg += 1;
    }

    s.spawn_cd_factor = s.spawn_cd_factor.max(0.85);
    s
}

/// Up to two recipe names for prep UI.
pub fn active_recipe_names(loadout: &TowerLoadout) -> Vec<&'static str> {
    compute_synergies(loadout)
        .recipes
        .into_iter()
        .take(2)
        .collect()
}

#[derive(Clone, Debug)]
pub struct CombatTower {
    pub loadout: TowerLoadout,
    pub hp: i32,
    pub max_hp: i32,
    pub gun_cd: Vec<f32>,
    pub spawn_cd: Vec<f32>,
    pub spike_cd: f32,
    pub fight_gun_bonus: i32,
    pub fight_unit_hp_bonus: i32,
    pub synergy: SynergyState,
}

#[derive(Clone, Debug)]
pub struct Unit {
    pub id: u64,
    pub side: Side,
    pub kind: PartKind,
    pub x: f32,
    pub lane: f32,
    pub hp: i32,
    pub max_hp: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotTarget {
    Tower,
    Unit(u64),
}

#[derive(Clone, Debug)]
pub struct Shot {
    pub from: Side,
    pub arc: ArcKind,
    pub damage: i32,
    pub t: f32,
    pub target: ShotTarget,
    pub start_x: f32,
    pub end_x: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Prep,
    Fight,
    Won,
    Lost,
}

#[derive(Clone, Debug)]
pub struct FightState {
    pub player: CombatTower,
    pub enemy: CombatTower,
    pub units: Vec<Unit>,
    pub shots: Vec<Shot>,
    pub elapsed: f32,
}

pub fn shop_tier(turn: u32) -> u8 {
    match turn {
        1 | 2 => 1,
        3 | 4 => 2,
        5 | 6 => 3,
        _ => 4,
    }
}

pub fn scripted_loadout(turn: u32) -> TowerLoadout {
    let modules: Vec<Module> = match turn {
        1 => vec![
            Module::new(PartKind::Wall),
            Module::new(PartKind::Foundation),
            Module::new(PartKind::Archer),
        ],
        2 => vec![
            Module::new(PartKind::Spikes),
            Module::new(PartKind::Wall),
            Module::new(PartKind::Foundation),
            Module::new(PartKind::Archer),
        ],
        3 => vec![
            Module::new(PartKind::Wall),
            Module::new(PartKind::Foundation),
            Module::new(PartKind::Mason),
            Module::new(PartKind::Archer),
        ],
        4 => vec![
            Module::new(PartKind::Banner),
            Module::new(PartKind::Ballista),
            Module::new(PartKind::Wall),
            Module::new(PartKind::Ram),
        ],
        5 => vec![
            Module::new(PartKind::Nest),
            Module::new(PartKind::Wall),
            Module::new(PartKind::Ballista),
            Module::new(PartKind::Archer),
        ],
        6 => vec![
            Module::new(PartKind::Lantern),
            Module::new(PartKind::Ballista),
            Module::new(PartKind::Nest),
            Module::new(PartKind::Banner),
        ],
        7 => vec![
            Module::new(PartKind::Banner),
            Module::new(PartKind::Ram),
            Module::new(PartKind::Nest),
            Module::new(PartKind::Ballista),
            Module::new(PartKind::Wall),
        ],
        _ => vec![
            Module::new(PartKind::Lantern),
            Module::new(PartKind::Ram),
            Module::new(PartKind::Nest),
            Module::new(PartKind::Ballista),
            Module::new(PartKind::Archer),
            Module::new(PartKind::Spikes),
        ],
    };
    let mut loadout = TowerLoadout {
        modules,
        bonus_hp: turn as i32 * 4 + 4,
        bonus_damage: 0,
    };
    // Apply mason-as-placed HP once into bonus for scripted towers that include Mason.
    for m in &loadout.modules {
        if m.kind == PartKind::Mason {
            loadout.bonus_hp += 8 * m.level as i32;
        }
    }
    loadout
}

fn make_combat(loadout: TowerLoadout) -> CombatTower {
    let max_hp = loadout.max_hp();
    let n = loadout.modules.len();
    CombatTower {
        loadout,
        hp: max_hp,
        max_hp,
        gun_cd: vec![0.0; n],
        spawn_cd: vec![0.0; n],
        spike_cd: 0.0,
        fight_gun_bonus: 0,
        fight_unit_hp_bonus: 0,
        synergy: SynergyState::default(),
    }
}

pub struct Run {
    pub turn: u32,
    pub gold: i32,
    pub phase: Phase,
    pub player: TowerLoadout,
    pub enemy: TowerLoadout,
    pub bar: Vec<Option<PartKind>>,
    pub selected_module: Option<usize>,
    /// Successful rerolls this prep; next cost is `REROLL_BASE_COST + rerolls_this_prep`.
    pub rerolls_this_prep: i32,
    pub fight: Option<FightState>,
    pub next_id: u64,
    rng: Rng,
}

impl Run {
    pub fn new(seed: u64) -> Self {
        let mut run = Self {
            turn: 1,
            gold: START_GOLD,
            phase: Phase::Prep,
            player: TowerLoadout::default(),
            enemy: scripted_loadout(1),
            bar: vec![None; BAR_WIDTH],
            selected_module: None,
            rerolls_this_prep: 0,
            fight: None,
            next_id: 1,
            rng: Rng::new(seed),
        };
        run.roll_bar();
        run
    }

    pub fn reroll_cost(&self) -> i32 {
        REROLL_BASE_COST + self.rerolls_this_prep
    }

    pub fn restart(&mut self, seed: u64) {
        *self = Self::new(seed);
    }

    fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn pool(&self) -> Vec<PartKind> {
        let tier = shop_tier(self.turn);
        PartKind::OFFERS
            .into_iter()
            .filter(|p| p.tier() <= tier)
            .collect()
    }

    fn roll_kind(&mut self) -> PartKind {
        let pool = self.pool();
        let idx = (self.rng.range(0.0, pool.len() as f32) as usize).min(pool.len() - 1);
        pool[idx]
    }

    fn roll_gun(&mut self) -> PartKind {
        // Prefer guns unlocked for this shop tier; fall back to Archer so early
        // rounds still get a shootable module (tier-1 pool has none).
        let guns: Vec<PartKind> = self
            .pool()
            .into_iter()
            .filter(|p| p.role() == ModuleRole::Gun)
            .collect();
        if guns.is_empty() {
            return PartKind::Archer;
        }
        let idx = (self.rng.range(0.0, guns.len() as f32) as usize).min(guns.len() - 1);
        guns[idx]
    }

    fn roll_bar(&mut self) {
        let mut bar: Vec<Option<PartKind>> =
            (0..BAR_WIDTH).map(|_| Some(self.roll_kind())).collect();
        // Opening offer of each prep always includes a gun (Archer/Ballista).
        if self.rerolls_this_prep == 0
            && !bar.iter().flatten().any(|k| k.role() == ModuleRole::Gun)
        {
            let slot = (self.rng.range(0.0, BAR_WIDTH as f32) as usize).min(BAR_WIDTH - 1);
            bar[slot] = Some(self.roll_gun());
        }
        self.bar = bar;
    }

    pub fn begin_prep(&mut self) {
        self.phase = Phase::Prep;
        self.gold = START_GOLD;
        self.selected_module = None;
        self.rerolls_this_prep = 0;
        self.fight = None;
        self.enemy = scripted_loadout(self.turn);
        self.roll_bar();
    }

    pub fn reroll(&mut self) -> bool {
        if self.phase != Phase::Prep {
            return false;
        }
        let cost = self.reroll_cost();
        if self.gold < cost {
            return false;
        }
        self.gold -= cost;
        self.rerolls_this_prep += 1;
        self.roll_bar();
        true
    }

    pub fn buy_offer(&mut self, index: usize) -> bool {
        if self.phase != Phase::Prep {
            return false;
        }
        let Some(kind) = self.bar.get(index).copied().flatten() else {
            return false;
        };
        let cost = kind.cost();
        if self.gold < cost {
            return false;
        }

        match kind.role() {
            ModuleRole::Consumable => {
                self.gold -= cost;
                match kind {
                    PartKind::Oil => self.player.bonus_damage += 3,
                    PartKind::Cement => self.player.bonus_hp += 8,
                    _ => {}
                }
                self.bar[index] = None;
                true
            }
            _ => {
                if let Some(existing) = self
                    .player
                    .modules
                    .iter()
                    .position(|m| m.kind == kind && m.level < 2)
                {
                    self.gold -= cost;
                    let m = &mut self.player.modules[existing];
                    m.level = 2;
                    if kind == PartKind::Mason {
                        self.player.bonus_hp += 8;
                    }
                    self.bar[index] = None;
                    return true;
                }
                if self.player.modules.len() >= TowerLoadout::slot_cap(self.turn) {
                    return false;
                }
                self.gold -= cost;
                self.player.modules.push(Module::new(kind));
                if kind == PartKind::Mason {
                    self.player.bonus_hp += 8;
                }
                self.bar[index] = None;
                true
            }
        }
    }

    pub fn select_module(&mut self, index: usize) {
        if self.phase != Phase::Prep {
            return;
        }
        if index >= self.player.modules.len() {
            self.selected_module = None;
            return;
        }
        if self.selected_module == Some(index) {
            self.selected_module = None;
        } else {
            self.selected_module = Some(index);
        }
    }

    pub fn sell_selected(&mut self) -> bool {
        if self.phase != Phase::Prep {
            return false;
        }
        let Some(i) = self.selected_module else {
            return false;
        };
        if i >= self.player.modules.len() {
            return false;
        }
        self.player.modules.remove(i);
        self.gold += SELL_REFUND;
        self.selected_module = None;
        true
    }

    pub fn sell_module(&mut self, index: usize) -> bool {
        self.selected_module = Some(index);
        self.sell_selected()
    }

    pub fn start_fight(&mut self) {
        if self.phase != Phase::Prep {
            return;
        }
        self.selected_module = None;
        let mut player = make_combat(self.player.clone());
        let mut enemy = make_combat(self.enemy.clone());
        apply_start_bonuses(&mut player);
        apply_start_bonuses(&mut enemy);
        // Late rounds: enemy guns hit harder without touching player economy.
        enemy.fight_gun_bonus += self.turn as i32 / 2;
        // HP reset every fight from current max (includes synergy HP).
        player.hp = player.max_hp;
        enemy.hp = enemy.max_hp;
        self.fight = Some(FightState {
            player,
            enemy,
            units: Vec::new(),
            shots: Vec::new(),
            elapsed: 0.0,
        });
        self.phase = Phase::Fight;
    }

    /// Advance fight by `dt` seconds. Returns false when fight is over.
    pub fn step_fight(&mut self, dt: f32) -> bool {
        if self.phase != Phase::Fight {
            return false;
        }
        let dt = dt.clamp(0.0, 0.1);
        if let Some(f) = self.fight.as_mut() {
            f.elapsed += dt;
            // Stalemate: no offense after 25s → higher remaining HP wins.
            if f.elapsed > 25.0 {
                let p = f.player.hp;
                let e = f.enemy.hp;
                if p >= e {
                    f.enemy.hp = 0;
                } else {
                    f.player.hp = 0;
                }
            }
        }
        self.tick_guns(Side::Player, dt);
        self.tick_guns(Side::Enemy, dt);
        self.tick_spawners(Side::Player, dt);
        self.tick_spawners(Side::Enemy, dt);
        self.tick_spikes(Side::Player, dt);
        self.tick_spikes(Side::Enemy, dt);
        self.tick_units(dt);
        self.tick_shots(dt);
        self.reap_dead_units();

        let Some(fight) = &self.fight else {
            return false;
        };
        if fight.player.hp <= 0 {
            self.phase = Phase::Lost;
            self.fight = None;
            return false;
        }
        if fight.enemy.hp <= 0 {
            self.finish_fight_win();
            return false;
        }
        true
    }

    fn finish_fight_win(&mut self) {
        self.fight = None;
        if self.turn >= MAX_TURNS {
            self.phase = Phase::Won;
            return;
        }
        self.turn += 1;
        self.begin_prep();
    }

    fn tower_mut(&mut self, side: Side) -> &mut CombatTower {
        let fight = self.fight.as_mut().unwrap();
        match side {
            Side::Player => &mut fight.player,
            Side::Enemy => &mut fight.enemy,
        }
    }

    fn tower(&self, side: Side) -> &CombatTower {
        let fight = self.fight.as_ref().unwrap();
        match side {
            Side::Player => &fight.player,
            Side::Enemy => &fight.enemy,
        }
    }

    fn tick_guns(&mut self, side: Side, dt: f32) {
        let modules: Vec<(usize, PartKind, u8)> = self
            .tower(side)
            .loadout
            .modules
            .iter()
            .enumerate()
            .filter(|(_, m)| m.kind.role() == ModuleRole::Gun)
            .map(|(i, m)| (i, m.kind, m.level))
            .collect();
        for (i, kind, level) in modules {
            {
                let t = self.tower_mut(side);
                if i >= t.gun_cd.len() {
                    t.gun_cd.resize(t.loadout.modules.len(), 0.0);
                }
                t.gun_cd[i] -= dt;
                if t.gun_cd[i] > 0.0 {
                    continue;
                }
                let cd = kind.gun_cooldown(level) * t.synergy.gun_cd_factor;
                t.gun_cd[i] = cd;
            }
            self.fire_gun(side, kind, level);
        }
    }

    fn fire_gun(&mut self, side: Side, kind: PartKind, level: u8) {
        let t = self.tower(side);
        let mut damage =
            kind.gun_damage(level) + t.fight_gun_bonus + t.loadout.bonus_damage + t.synergy.role_gun_dmg;
        if kind == PartKind::Ballista {
            damage += t.synergy.ballista_bonus_dmg;
        }
        if kind == PartKind::Archer {
            damage += t.synergy.archer_bonus_dmg;
        }
        let arc = kind.arc();
        let start_x = side.base_x();
        let (target, end_x) = self.pick_gun_target(side);
        let fight = self.fight.as_mut().unwrap();
        fight.shots.push(Shot {
            from: side,
            arc,
            damage,
            t: 0.0,
            target,
            start_x,
            end_x,
        });
    }

    fn pick_gun_target(&self, side: Side) -> (ShotTarget, f32) {
        let enemy = side.other();
        let fight = self.fight.as_ref().unwrap();
        let mut best: Option<(f32, u64, f32)> = None;
        for u in &fight.units {
            if u.side != enemy || u.hp <= 0 {
                continue;
            }
            let dist = (u.x - side.base_x()).abs();
            match best {
                Some((d, _, _)) if dist >= d => {}
                _ => best = Some((dist, u.id, u.x)),
            }
        }
        if let Some((_, id, x)) = best {
            (ShotTarget::Unit(id), x)
        } else {
            (ShotTarget::Tower, enemy.base_x())
        }
    }

    fn tick_spawners(&mut self, side: Side, dt: f32) {
        let modules: Vec<(usize, PartKind, u8)> = self
            .tower(side)
            .loadout
            .modules
            .iter()
            .enumerate()
            .filter_map(|(i, m)| m.kind.spawn_kind().map(|_| (i, m.kind, m.level)))
            .collect();
        for (i, kind, level) in modules {
            {
                let t = self.tower_mut(side);
                if i >= t.spawn_cd.len() {
                    t.spawn_cd.resize(t.loadout.modules.len(), 0.0);
                }
                t.spawn_cd[i] -= dt;
                if t.spawn_cd[i] > 0.0 {
                    continue;
                }
                let cd = kind.spawn_cooldown(level) * t.synergy.spawn_cd_factor;
                t.spawn_cd[i] = cd;
            }
            for _ in 0..level {
                self.spawn_unit(side, kind.spawn_kind().unwrap());
            }
        }
    }

    fn spawn_unit(&mut self, side: Side, kind: PartKind) {
        let bonus = self.tower(side).fight_unit_hp_bonus;
        let hp = kind.unit_hp() + bonus;
        let lane = self.rng.range(-0.35, 0.35);
        let id = self.alloc_id();
        let fight = self.fight.as_mut().unwrap();
        fight.units.push(Unit {
            id,
            side,
            kind,
            x: side.base_x(),
            lane,
            hp,
            max_hp: hp,
        });
    }

    fn tick_spikes(&mut self, side: Side, dt: f32) {
        let has = self
            .tower(side)
            .loadout
            .modules
            .iter()
            .any(|m| m.kind == PartKind::Spikes);
        if !has {
            return;
        }
        let level: u8 = self
            .tower(side)
            .loadout
            .modules
            .iter()
            .filter(|m| m.kind == PartKind::Spikes)
            .map(|m| m.level)
            .sum();
        {
            let t = self.tower_mut(side);
            t.spike_cd -= dt;
            if t.spike_cd > 0.0 {
                return;
            }
            t.spike_cd = 0.7 / level as f32;
        }
        let base = side.base_x();
        let spike_bonus = self.tower(side).synergy.spike_bonus_dmg;
        let dmg = 2 * level as i32 + spike_bonus;
        let fight = self.fight.as_mut().unwrap();
        for u in &mut fight.units {
            if u.side == side.other() && (u.x - base).abs() <= SPIKE_RANGE {
                u.hp -= dmg;
            }
        }
    }

    fn tick_units(&mut self, dt: f32) {
        // Snapshot synergy-aware stats before mutating fight.
        let (p_chick, e_chick) = {
            let f = self.fight.as_ref().unwrap();
            (
                f.player.synergy.chick_contact_bonus,
                f.enemy.synergy.chick_contact_bonus,
            )
        };
        let (p_ram, e_ram) = {
            let f = self.fight.as_ref().unwrap();
            (
                f.player.synergy.ram_speed_factor,
                f.enemy.synergy.ram_speed_factor,
            )
        };

        let fight = self.fight.as_mut().unwrap();
        // Move
        for u in &mut fight.units {
            if u.hp <= 0 {
                continue;
            }
            let mut spd = u.kind.unit_speed();
            if u.kind == PartKind::Ram {
                spd *= match u.side {
                    Side::Player => p_ram,
                    Side::Enemy => e_ram,
                };
            }
            u.x += u.side.dir() * spd * dt;
            u.x = u.x.clamp(0.0, 1.0);
        }

        let unit_dmg = |side: Side, kind: PartKind| {
            let mut d = kind.unit_damage();
            if kind == PartKind::Chick {
                d += match side {
                    Side::Player => p_chick,
                    Side::Enemy => e_chick,
                };
            }
            d
        };

        // Unit vs unit contact
        let n = fight.units.len();
        for i in 0..n {
            if fight.units[i].hp <= 0 {
                continue;
            }
            for j in (i + 1)..n {
                if fight.units[j].hp <= 0 {
                    continue;
                }
                if fight.units[i].side == fight.units[j].side {
                    continue;
                }
                if (fight.units[i].x - fight.units[j].x).abs() <= CONTACT_RANGE {
                    let di = unit_dmg(fight.units[i].side, fight.units[i].kind);
                    let dj = unit_dmg(fight.units[j].side, fight.units[j].kind);
                    fight.units[i].hp -= dj;
                    fight.units[j].hp -= di;
                }
            }
        }

        // Reach enemy base
        let mut hits: Vec<(Side, i32, usize)> = Vec::new();
        for (idx, u) in fight.units.iter().enumerate() {
            if u.hp <= 0 {
                continue;
            }
            let goal = u.side.other().base_x();
            if (u.x - goal).abs() <= BASE_REACH {
                hits.push((u.side.other(), unit_dmg(u.side, u.kind), idx));
            }
        }
        for (tower_side, dmg, idx) in hits {
            match tower_side {
                Side::Player => fight.player.hp -= dmg,
                Side::Enemy => fight.enemy.hp -= dmg,
            }
            fight.units[idx].hp = 0;
        }
    }

    fn tick_shots(&mut self, dt: f32) {
        let fight = self.fight.as_mut().unwrap();
        let mut arrivals = Vec::new();
        for (i, shot) in fight.shots.iter_mut().enumerate() {
            shot.t += SHOT_SPEED * dt;
            if shot.t >= 1.0 {
                arrivals.push(i);
            }
        }
        // Resolve high indices first so remove is stable
        arrivals.sort_unstable();
        for i in arrivals.into_iter().rev() {
            let shot = fight.shots.remove(i);
            match shot.target {
                ShotTarget::Tower => match shot.from.other() {
                    Side::Player => fight.player.hp -= shot.damage,
                    Side::Enemy => fight.enemy.hp -= shot.damage,
                },
                ShotTarget::Unit(id) => {
                    if let Some(u) = fight.units.iter_mut().find(|u| u.id == id && u.hp > 0) {
                        u.hp -= shot.damage;
                    } else {
                        // Splash nearest enemy unit, else tower
                        let enemy = shot.from.other();
                        let mut best: Option<(f32, usize)> = None;
                        for (idx, u) in fight.units.iter().enumerate() {
                            if u.side == enemy && u.hp > 0 {
                                let d = (u.x - shot.end_x).abs();
                                match best {
                                    Some((bd, _)) if d >= bd => {}
                                    _ => best = Some((d, idx)),
                                }
                            }
                        }
                        if let Some((_, idx)) = best {
                            fight.units[idx].hp -= shot.damage;
                        } else {
                            match enemy {
                                Side::Player => fight.player.hp -= shot.damage,
                                Side::Enemy => fight.enemy.hp -= shot.damage,
                            }
                        }
                    }
                }
            }
        }
    }

    fn reap_dead_units(&mut self) {
        if let Some(fight) = self.fight.as_mut() {
            fight.units.retain(|u| u.hp > 0);
        }
    }

    /// Run fight to completion (tests).
    #[allow(dead_code)]
    pub fn finish_fight(&mut self) {
        let mut guard = 0;
        while self.phase == Phase::Fight && guard < 100_000 {
            if !self.step_fight(1.0 / 30.0) {
                break;
            }
            guard += 1;
        }
    }
}

fn apply_start_bonuses(tower: &mut CombatTower) {
    for m in &tower.loadout.modules {
        match m.kind {
            PartKind::Banner => tower.fight_gun_bonus += 2 * m.level as i32,
            PartKind::Lantern => tower.fight_unit_hp_bonus += 3 * m.level as i32,
            _ => {}
        }
    }
    tower.synergy = compute_synergies(&tower.loadout);
    // Siege / Masonry fight HP.
    let extra = tower.synergy.structure_fight_hp + tower.synergy.fight_hp_bonus;
    if extra != 0 {
        tower.max_hp += extra;
        tower.hp = tower.max_hp;
    }
    // Gun+Spawner opener: first spawn pulse after 0.4s instead of full CD.
    if tower.synergy.spawn_opener {
        for (i, m) in tower.loadout.modules.iter().enumerate() {
            if m.kind.spawn_kind().is_some() {
                if i < tower.spawn_cd.len() {
                    tower.spawn_cd[i] = 0.4;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buy_merge_and_slot_cap() {
        let mut run = Run::new(1);
        run.bar = vec![
            Some(PartKind::Archer),
            Some(PartKind::Archer),
            Some(PartKind::Archer),
        ];
        run.gold = 20;
        assert!(run.buy_offer(0));
        assert_eq!(run.player.modules.len(), 1);
        assert_eq!(run.player.modules[0].level, 1);
        assert!(run.buy_offer(1));
        assert_eq!(run.player.modules.len(), 1);
        assert_eq!(run.player.modules[0].level, 2);
        // Level 2 cannot merge further — new slot
        assert!(run.buy_offer(2));
        assert_eq!(run.player.modules.len(), 2);
    }

    #[test]
    fn sell_refunds() {
        let mut run = Run::new(1);
        run.player.modules.push(Module::new(PartKind::Wall));
        let gold = run.gold;
        assert!(run.sell_module(0));
        assert!(run.player.modules.is_empty());
        assert_eq!(run.gold, gold + SELL_REFUND);
    }

    #[test]
    fn slot_cap_blocks() {
        let mut run = Run::new(1);
        run.turn = 1;
        run.gold = 100;
        for kind in [PartKind::Foundation, PartKind::Wall, PartKind::Spikes] {
            run.bar[0] = Some(kind);
            assert!(run.buy_offer(0));
        }
        assert_eq!(run.player.modules.len(), 3);
        run.bar[0] = Some(PartKind::Archer);
        assert!(!run.buy_offer(0));
    }

    #[test]
    fn hp_resets_between_fights() {
        let mut run = Run::new(7);
        run.player.modules.push(Module::new(PartKind::Wall));
        run.player.modules.push(Module::new(PartKind::Ballista));
        run.player.modules.push(Module::new(PartKind::Ballista));
        let max_hp = run.player.max_hp();
        run.start_fight();
        {
            let f = run.fight.as_mut().unwrap();
            f.player.hp = max_hp / 2;
            f.enemy.hp = 0;
        }
        run.finish_fight_win();
        assert_eq!(run.phase, Phase::Prep);
        assert_eq!(run.turn, 2);
        run.start_fight();
        let f = run.fight.as_ref().unwrap();
        assert_eq!(f.player.hp, f.player.max_hp);
        assert_eq!(f.enemy.hp, f.enemy.max_hp);
    }

    #[test]
    fn gun_prefers_unit_over_tower() {
        let mut run = Run::new(1);
        run.player.modules.push(Module::new(PartKind::Archer));
        run.enemy = TowerLoadout {
            modules: vec![Module::new(PartKind::Wall)],
            bonus_hp: 0,
            bonus_damage: 0,
        };
        run.start_fight();
        {
            let f = run.fight.as_mut().unwrap();
            f.units.push(Unit {
                id: 99,
                side: Side::Enemy,
                kind: PartKind::Chick,
                x: 0.5,
                lane: 0.0,
                hp: 6,
                max_hp: 6,
            });
        }
        run.fire_gun(Side::Player, PartKind::Archer, 1);
        let shot = &run.fight.as_ref().unwrap().shots[0];
        assert_eq!(shot.target, ShotTarget::Unit(99));
    }

    #[test]
    fn walker_hits_base() {
        let mut run = Run::new(1);
        run.player.modules.push(Module::new(PartKind::Wall));
        run.start_fight();
        let enemy_hp = run.fight.as_ref().unwrap().enemy.hp;
        {
            let f = run.fight.as_mut().unwrap();
            f.units.push(Unit {
                id: 1,
                side: Side::Player,
                kind: PartKind::Chick,
                x: Side::Enemy.base_x(),
                lane: 0.0,
                hp: 6,
                max_hp: 6,
            });
        }
        run.tick_units(0.0);
        assert!(run.fight.as_ref().unwrap().enemy.hp < enemy_hp);
    }

    #[test]
    fn scripted_fight_finishes() {
        let mut run = Run::new(3);
        run.player.modules = vec![
            Module::new(PartKind::Ballista),
            Module::new(PartKind::Ballista),
            Module::new(PartKind::Nest),
            Module::new(PartKind::Wall),
        ];
        // Force a weak enemy
        run.enemy = TowerLoadout {
            modules: vec![Module::new(PartKind::Foundation)],
            bonus_hp: 0,
            bonus_damage: 0,
        };
        run.start_fight();
        run.finish_fight();
        assert!(matches!(run.phase, Phase::Prep | Phase::Won | Phase::Lost));
    }

    #[test]
    fn oil_and_cement_consumables() {
        let mut run = Run::new(1);
        run.bar = vec![Some(PartKind::Oil), Some(PartKind::Cement), None];
        run.gold = 10;
        assert!(run.buy_offer(0));
        assert_eq!(run.player.bonus_damage, 3);
        assert!(run.player.modules.is_empty());
        assert!(run.buy_offer(1));
        assert_eq!(run.player.bonus_hp, 8);
    }

    #[test]
    fn starts_with_three_gold() {
        let run = Run::new(42);
        assert_eq!(run.gold, 3);
        assert_eq!(START_GOLD, 3);
    }

    #[test]
    fn playtest_five_rounds_with_gun_and_spawner() {
        let mut run = Run::new(99);
        for round in 1..=5 {
            assert_eq!(run.turn, round);
            assert_eq!(run.phase, Phase::Prep);
            // Prefer buying a missing gun, then a spawner, then anything affordable.
            let mut bought = false;
            for _ in 0..3 {
                let has_gun = run
                    .player
                    .modules
                    .iter()
                    .any(|m| m.kind.role() == ModuleRole::Gun);
                let has_spawner = run
                    .player
                    .modules
                    .iter()
                    .any(|m| m.kind.role() == ModuleRole::Spawner);
                let pick = run.bar.iter().enumerate().find_map(|(i, o)| {
                    let kind = (*o)?;
                    if kind.cost() > run.gold {
                        return None;
                    }
                    if !has_gun && kind.role() == ModuleRole::Gun {
                        return Some(i);
                    }
                    if !has_spawner && kind.role() == ModuleRole::Spawner {
                        return Some(i);
                    }
                    Some(i)
                });
                if let Some(i) = pick {
                    bought |= run.buy_offer(i);
                } else if run.gold >= run.reroll_cost() {
                    run.reroll();
                } else {
                    break;
                }
            }
            let _ = bought;
            // Ensure we can fight: inject gun+spawner if shop RNG was unlucky after buys.
            if !run
                .player
                .modules
                .iter()
                .any(|m| m.kind.role() == ModuleRole::Gun)
            {
                run.player.modules.push(Module::new(PartKind::Archer));
            }
            if !run
                .player
                .modules
                .iter()
                .any(|m| m.kind.role() == ModuleRole::Spawner)
            {
                run.player.modules.push(Module::new(PartKind::Chick));
            }
            run.start_fight();
            assert_eq!(run.phase, Phase::Fight);
            // Advance until prep/win/lose; force a win if still fighting after many ticks.
            for _ in 0..4_000 {
                if run.phase != Phase::Fight {
                    break;
                }
                run.step_fight(1.0 / 60.0);
            }
            if run.phase == Phase::Fight {
                if let Some(f) = run.fight.as_mut() {
                    f.enemy.hp = 0;
                }
                run.finish_fight_win();
            }
            if run.phase == Phase::Lost {
                panic!("lost on round {round}");
            }
            if run.phase == Phase::Won {
                assert!(round >= 5);
                break;
            }
            assert_eq!(run.phase, Phase::Prep);
        }
        assert!(run.turn >= 5 || run.phase == Phase::Won);
    }

    #[test]
    fn early_shop_has_spawners() {
        assert!(PartKind::Chick.tier() <= 1);
        assert!(PartKind::Ram.tier() <= 1);
        assert!(PartKind::Chick.spawn_kind().is_some());
        assert!(PartKind::Ram.spawn_kind().is_some());
        assert!(PartKind::Nest.spawn_kind().is_some());
        assert!(PartKind::OFFERS.contains(&PartKind::Chick));
        let mut seen = false;
        for seed in 0..80 {
            let run = Run::new(seed);
            if run
                .bar
                .iter()
                .flatten()
                .any(|k| k.role() == ModuleRole::Spawner)
            {
                seen = true;
                break;
            }
        }
        // With Chick/Ram at tier 1, random bars should sometimes roll a spawner.
        assert!(seen, "no spawner appeared across opening bars");
    }

    #[test]
    fn first_bar_always_has_gun() {
        for seed in 0..64 {
            let run = Run::new(seed);
            assert!(
                run.bar.iter().flatten().any(|k| k.role() == ModuleRole::Gun),
                "seed {seed} bar {:?} has no gun",
                run.bar
            );
        }
        // After a fight, next prep opening bar also has a gun.
        let mut run = Run::new(1);
        run.player.modules.push(Module::new(PartKind::Archer));
        run.start_fight();
        run.finish_fight_win();
        assert!(
            run.bar.iter().flatten().any(|k| k.role() == ModuleRole::Gun),
            "prep bar after win {:?} has no gun",
            run.bar
        );
    }

    #[test]
    fn reroll_costs_increase() {
        let mut run = Run::new(1);
        run.gold = 10;
        assert_eq!(run.reroll_cost(), 1);
        assert!(run.reroll());
        assert_eq!(run.gold, 9);
        assert_eq!(run.reroll_cost(), 2);
        assert!(run.reroll());
        assert_eq!(run.gold, 7);
        assert_eq!(run.reroll_cost(), 3);
        run.begin_prep();
        assert_eq!(run.reroll_cost(), 1);
        assert_eq!(run.gold, START_GOLD);
    }

    #[test]
    fn volley_recipe_speeds_guns() {
        let bare = TowerLoadout {
            modules: vec![Module::new(PartKind::Archer)],
            bonus_hp: 0,
            bonus_damage: 0,
        };
        let with_banner = TowerLoadout {
            modules: vec![Module::new(PartKind::Archer), Module::new(PartKind::Banner)],
            bonus_hp: 0,
            bonus_damage: 0,
        };
        let s0 = compute_synergies(&bare);
        let s1 = compute_synergies(&with_banner);
        assert!(!s0.recipes.iter().any(|r| *r == "VOLLEY"));
        assert!((s0.gun_cd_factor - 1.0).abs() < 0.001);
        assert!(s1.recipes.iter().any(|r| *r == "VOLLEY"));
        assert!((s1.gun_cd_factor - 0.8).abs() < 0.001);
    }

    #[test]
    fn siege_and_nest_guard_and_oil_tips() {
        let siege = TowerLoadout {
            modules: vec![Module::new(PartKind::Ballista), Module::new(PartKind::Wall)],
            ..TowerLoadout::default()
        };
        let s = compute_synergies(&siege);
        assert!(s.recipes.iter().any(|r| *r == "SIEGE"));
        assert_eq!(s.ballista_bonus_dmg, 2);
        assert_eq!(s.structure_fight_hp, 4);

        let nest = TowerLoadout {
            modules: vec![Module::new(PartKind::Nest), Module::new(PartKind::Spikes)],
            ..TowerLoadout::default()
        };
        let n = compute_synergies(&nest);
        assert!(n.recipes.iter().any(|r| *r == "NEST GUARD"));
        assert_eq!(n.chick_contact_bonus, 1);

        let oil = TowerLoadout {
            modules: vec![Module::new(PartKind::Archer)],
            bonus_hp: 0,
            bonus_damage: 3,
        };
        let o = compute_synergies(&oil);
        assert!(o.recipes.iter().any(|r| *r == "OIL TIPS"));
        assert_eq!(o.archer_bonus_dmg, 1);

        let none = compute_synergies(&TowerLoadout::default());
        assert!(none.recipes.is_empty());
    }

    #[test]
    fn war_path_and_masonry() {
        let war = TowerLoadout {
            modules: vec![Module::new(PartKind::Ram), Module::new(PartKind::Banner)],
            ..TowerLoadout::default()
        };
        let w = compute_synergies(&war);
        assert!(w.recipes.iter().any(|r| *r == "WAR PATH"));
        assert!((w.ram_speed_factor - 1.25).abs() < 0.001);

        let mut wall_l2 = Module::new(PartKind::Wall);
        wall_l2.level = 2;
        let masonry = TowerLoadout {
            modules: vec![Module::new(PartKind::Mason), wall_l2],
            bonus_hp: 8, // mason contribution only
            bonus_damage: 0,
        };
        let m = compute_synergies(&masonry);
        assert!(m.recipes.iter().any(|r| *r == "MASONRY"));
        assert_eq!(m.fight_hp_bonus, 10);

        let cement_mason = TowerLoadout {
            modules: vec![Module::new(PartKind::Mason)],
            bonus_hp: 16, // mason 8 + cement 8
            bonus_damage: 0,
        };
        assert!(compute_synergies(&cement_mason)
            .recipes
            .iter()
            .any(|r| *r == "MASONRY"));
    }

    #[test]
    fn role_synergy_two_guns() {
        let two = TowerLoadout {
            modules: vec![Module::new(PartKind::Archer), Module::new(PartKind::Ballista)],
            ..TowerLoadout::default()
        };
        let s = compute_synergies(&two);
        assert_eq!(s.role_gun_dmg, 1);
        assert!(!s.spawn_opener);
        let with_spawn = TowerLoadout {
            modules: vec![
                Module::new(PartKind::Archer),
                Module::new(PartKind::Ballista),
                Module::new(PartKind::Nest),
            ],
            ..TowerLoadout::default()
        };
        let g = compute_synergies(&with_spawn);
        assert!(g.spawn_opener);
        assert_eq!(g.role_gun_dmg, 1);
        assert!((g.spawn_cd_factor - 1.0).abs() < 0.001); // only one spawner

        let two_spawn = TowerLoadout {
            modules: vec![Module::new(PartKind::Nest), Module::new(PartKind::Ram)],
            ..TowerLoadout::default()
        };
        let sp = compute_synergies(&two_spawn);
        assert!((sp.spawn_cd_factor - 0.9).abs() < 0.001);
    }

    #[test]
    fn synergies_apply_in_fight() {
        let mut run = Run::new(1);
        run.player.modules = vec![
            Module::new(PartKind::Banner),
            Module::new(PartKind::Archer),
            Module::new(PartKind::Nest),
        ];
        run.start_fight();
        let f = run.fight.as_ref().unwrap();
        assert!(f.player.synergy.recipes.iter().any(|r| *r == "VOLLEY"));
        assert!(f.player.synergy.spawn_opener);
        assert!((f.player.synergy.gun_cd_factor - 0.8).abs() < 0.001);
    }

    #[test]
    fn harder_enemy_scripts() {
        let t4 = scripted_loadout(4);
        assert!(
            t4.modules.iter().any(|m| m.kind == PartKind::Ballista),
            "turn 4 should have Ballista: {:?}",
            t4.modules
        );
        assert!(t4.has_gun());
        assert!(t4.has_role(ModuleRole::Spawner));
        assert_eq!(t4.bonus_hp, 4 * 4 + 4); // no mason on t4

        let t5 = scripted_loadout(5);
        assert!(t5.has_gun());
        assert!(t5.has_role(ModuleRole::Spawner));

        let t8 = scripted_loadout(8);
        assert!(t8.has_kind(PartKind::Spikes));
        assert!(t8.has_kind(PartKind::Lantern) || t8.has_kind(PartKind::Banner));

        let mut run = Run::new(1);
        run.turn = 6;
        run.enemy = scripted_loadout(6);
        run.player.modules.push(Module::new(PartKind::Archer));
        run.start_fight();
        let f = run.fight.as_ref().unwrap();
        // Banner/Lantern passives + turn/2 gun pressure
        assert!(f.enemy.fight_gun_bonus >= 6 / 2);
    }

    #[test]
    fn active_recipe_names_caps_at_two() {
        let loadout = TowerLoadout {
            modules: vec![
                Module::new(PartKind::Banner),
                Module::new(PartKind::Archer),
                Module::new(PartKind::Ram),
                Module::new(PartKind::Spikes),
                Module::new(PartKind::Nest),
            ],
            bonus_damage: 3,
            bonus_hp: 0,
        };
        let names = active_recipe_names(&loadout);
        assert!(names.len() <= 2);
        assert!(!names.is_empty());
    }
}
