use super::rng::Rng;
use super::terrain::MoveType;
use super::Pos;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Team {
    Player,
    Enemy,
}

impl Team {
    pub fn is_foe(self, other: Team) -> bool {
        self != other
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponKind {
    Sword,
    Lance,
    Axe,
    Bow,
    Magic,
    Staff,
}

impl WeaponKind {
    /// Weapon triangle: Sword > Axe > Lance > Sword.
    /// Returns +1 for advantage, -1 for disadvantage, 0 otherwise.
    pub fn triangle(self, other: WeaponKind) -> i32 {
        use WeaponKind::*;
        match (self, other) {
            (Sword, Axe) | (Axe, Lance) | (Lance, Sword) => 1,
            (Axe, Sword) | (Lance, Axe) | (Sword, Lance) => -1,
            _ => 0,
        }
    }

    pub fn is_magic(self) -> bool {
        matches!(self, WeaponKind::Magic | WeaponKind::Staff)
    }

    pub fn name(self) -> &'static str {
        match self {
            WeaponKind::Sword => "Sword",
            WeaponKind::Lance => "Lance",
            WeaponKind::Axe => "Axe",
            WeaponKind::Bow => "Bow",
            WeaponKind::Magic => "Tome",
            WeaponKind::Staff => "Staff",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Weapon {
    pub name: &'static str,
    pub kind: WeaponKind,
    pub might: i32,
    pub hit: i32,
    pub crit: i32,
    pub min_range: i32,
    pub max_range: i32,
}

impl Weapon {
    pub fn in_range(&self, dist: i32) -> bool {
        dist >= self.min_range && dist <= self.max_range
    }

    pub fn can_attack(&self) -> bool {
        self.kind != WeaponKind::Staff
    }

    pub fn range_label(&self) -> String {
        if self.min_range == self.max_range {
            format!("{}", self.min_range)
        } else {
            format!("{}-{}", self.min_range, self.max_range)
        }
    }
}

/// Weapon catalogue (all original names).
pub mod weapons {
    use super::{Weapon, WeaponKind::*};

    const fn w(name: &'static str, kind: super::WeaponKind, might: i32, hit: i32, crit: i32, min_range: i32, max_range: i32) -> Weapon {
        Weapon { name, kind, might, hit, crit, min_range, max_range }
    }

    pub fn oathblade() -> Weapon { w("Oathblade", Sword, 6, 95, 10, 1, 1) }
    #[cfg(test)]
    pub fn iron_sword() -> Weapon { w("Iron Sword", Sword, 5, 90, 0, 1, 1) }
    pub fn keen_sword() -> Weapon { w("Keen Sword", Sword, 4, 80, 25, 1, 1) }
    pub fn iron_lance() -> Weapon { w("Iron Lance", Lance, 7, 80, 0, 1, 1) }
    pub fn iron_axe() -> Weapon { w("Iron Axe", Axe, 8, 75, 0, 1, 1) }
    pub fn cleaver() -> Weapon { w("Cleaver", Axe, 11, 75, 10, 1, 1) }
    pub fn short_bow() -> Weapon { w("Short Bow", Bow, 6, 85, 0, 2, 2) }
    pub fn ember() -> Weapon { w("Ember", Magic, 5, 90, 0, 1, 2) }
    pub fn gloom() -> Weapon { w("Gloom", Magic, 6, 80, 5, 1, 2) }
    pub fn mend() -> Weapon { w("Mend Staff", Staff, 0, 100, 0, 1, 1) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Lord,
    Mercenary,
    Lancer,
    Soldier,
    Brigand,
    Warlord,
    Archer,
    Mage,
    Cleric,
}

impl Class {
    pub fn name(self) -> &'static str {
        match self {
            Class::Lord => "Lord",
            Class::Mercenary => "Mercenary",
            Class::Lancer => "Lancer",
            Class::Soldier => "Soldier",
            Class::Brigand => "Brigand",
            Class::Warlord => "Warlord",
            Class::Archer => "Archer",
            Class::Mage => "Mage",
            Class::Cleric => "Cleric",
        }
    }

    pub fn move_range(self) -> i32 {
        match self {
            Class::Soldier => 4,
            _ => 5,
        }
    }

    pub fn move_type(self) -> MoveType {
        match self {
            Class::Brigand | Class::Warlord => MoveType::Mountaineer,
            _ => MoveType::Infantry,
        }
    }

    pub fn uses_magic(self) -> bool {
        matches!(self, Class::Mage | Class::Cleric)
    }
}

pub const STAT_NAMES: [&str; 7] = ["HP", "Str", "Skl", "Spd", "Lck", "Def", "Res"];
pub const STAT_CAPS: [i32; 7] = [60, 30, 30, 30, 30, 30, 30];
pub const MAX_LEVEL: i32 = 20;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub hp: i32,
    /// Strength or Magic depending on class.
    pub pow: i32,
    pub skl: i32,
    pub spd: i32,
    pub lck: i32,
    pub def: i32,
    pub res: i32,
}

impl Stats {
    pub const fn new(hp: i32, pow: i32, skl: i32, spd: i32, lck: i32, def: i32, res: i32) -> Self {
        Stats { hp, pow, skl, spd, lck, def, res }
    }

    pub fn to_array(self) -> [i32; 7] {
        [self.hp, self.pow, self.skl, self.spd, self.lck, self.def, self.res]
    }

    pub fn from_array(a: [i32; 7]) -> Self {
        Stats::new(a[0], a[1], a[2], a[3], a[4], a[5], a[6])
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LevelUp {
    pub new_level: i32,
    pub gains: Stats,
    /// Stats after the level-up, for display.
    pub after: Stats,
}

#[derive(Clone, Debug)]
pub struct Unit {
    pub name: String,
    pub class: Class,
    pub team: Team,
    pub level: i32,
    pub xp: i32,
    pub hp: i32,
    pub stats: Stats,
    /// Growth rates in percent.
    pub growths: Stats,
    pub weapon: Weapon,
    pub pos: Pos,
    pub is_lord: bool,
    pub is_boss: bool,
    /// Never moves (boss guarding the throne).
    pub stationary: bool,
    /// Has acted this phase.
    pub done: bool,
    pub alive: bool,
}

impl Unit {
    #[allow(clippy::too_many_arguments)]
    pub fn new(name: &str, class: Class, team: Team, level: i32, stats: Stats, growths: Stats, weapon: Weapon, pos: Pos) -> Unit {
        Unit {
            name: name.to_string(),
            class,
            team,
            level,
            xp: 0,
            hp: stats.hp,
            stats,
            growths,
            weapon,
            pos,
            is_lord: false,
            is_boss: false,
            stationary: false,
            done: false,
            alive: true,
        }
    }

    pub fn lord(mut self) -> Self {
        self.is_lord = true;
        self
    }

    pub fn boss(mut self) -> Self {
        self.is_boss = true;
        self.stationary = true;
        self
    }

    pub fn mov(&self) -> i32 {
        if self.stationary {
            0
        } else {
            self.class.move_range()
        }
    }

    pub fn pow_label(&self) -> &'static str {
        if self.class.uses_magic() {
            "Mag"
        } else {
            "Str"
        }
    }

    /// Adds experience; at 100 the unit levels up (at most once per call).
    pub fn gain_xp(&mut self, amount: i32, rng: &mut Rng) -> Option<LevelUp> {
        if self.level >= MAX_LEVEL {
            self.xp = 0;
            return None;
        }
        self.xp += amount.clamp(0, 100);
        if self.xp >= 100 {
            self.xp -= 100;
            let lv = self.level_up(rng);
            if self.level >= MAX_LEVEL {
                self.xp = 0;
            }
            Some(lv)
        } else {
            None
        }
    }

    /// Rolls each stat against its growth rate. If nothing grows, the stat
    /// with the highest growth gets +1 so a level is never empty.
    pub fn level_up(&mut self, rng: &mut Rng) -> LevelUp {
        let growths = self.growths.to_array();
        let mut stats = self.stats.to_array();
        let mut gains = [0; 7];
        for i in 0..7 {
            if stats[i] < STAT_CAPS[i] && rng.roll(growths[i]) {
                gains[i] = 1;
            }
        }
        if gains.iter().all(|g| *g == 0) {
            let best = (0..7)
                .filter(|&i| stats[i] < STAT_CAPS[i])
                .max_by_key(|&i| (growths[i], -(i as i32)));
            if let Some(i) = best {
                gains[i] = 1;
            }
        }
        for i in 0..7 {
            stats[i] += gains[i];
        }
        self.level += 1;
        self.stats = Stats::from_array(stats);
        self.hp += gains[0];
        LevelUp {
            new_level: self.level,
            gains: Stats::from_array(gains),
            after: self.stats,
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn dummy(class: Class, team: Team, stats: Stats, weapon: Weapon) -> Unit {
        Unit::new("Test", class, team, 1, stats, Stats::new(50, 50, 50, 50, 50, 50, 50), weapon, Pos::new(0, 0))
    }

    #[test]
    fn weapon_triangle() {
        use WeaponKind::*;
        assert_eq!(Sword.triangle(Axe), 1);
        assert_eq!(Axe.triangle(Lance), 1);
        assert_eq!(Lance.triangle(Sword), 1);
        assert_eq!(Axe.triangle(Sword), -1);
        assert_eq!(Lance.triangle(Axe), -1);
        assert_eq!(Sword.triangle(Lance), -1);
        assert_eq!(Sword.triangle(Sword), 0);
        assert_eq!(Bow.triangle(Sword), 0);
        assert_eq!(Magic.triangle(Axe), 0);
    }

    #[test]
    fn level_up_with_full_growths_raises_everything() {
        let mut u = dummy(Class::Lord, Team::Player, Stats::new(20, 5, 5, 5, 5, 5, 5), weapons::iron_sword());
        u.growths = Stats::new(100, 100, 100, 100, 100, 100, 100);
        u.hp = 10;
        let mut rng = Rng::new(1);
        let lv = u.level_up(&mut rng);
        assert_eq!(lv.new_level, 2);
        assert_eq!(lv.gains, Stats::new(1, 1, 1, 1, 1, 1, 1));
        assert_eq!(u.stats, Stats::new(21, 6, 6, 6, 6, 6, 6));
        assert_eq!(u.hp, 11, "current HP rises with max HP");
    }

    #[test]
    fn level_up_never_empty() {
        let mut u = dummy(Class::Lord, Team::Player, Stats::new(20, 5, 5, 5, 5, 5, 5), weapons::iron_sword());
        u.growths = Stats::new(0, 0, 0, 10, 0, 0, 0);
        // Seed irrelevant: growths are tiny; even if spd misses, fallback gives spd.
        for seed in 0..20 {
            let mut v = u.clone();
            let lv = v.level_up(&mut Rng::new(seed));
            let total: i32 = lv.gains.to_array().iter().sum();
            assert!(total >= 1);
        }
        u.growths = Stats::default();
        let lv = u.level_up(&mut Rng::new(3));
        assert_eq!(lv.gains.to_array().iter().sum::<i32>(), 1);
    }

    #[test]
    fn stats_respect_caps() {
        let mut u = dummy(Class::Lord, Team::Player, Stats::new(60, 30, 30, 30, 30, 30, 29), weapons::iron_sword());
        u.growths = Stats::new(100, 100, 100, 100, 100, 100, 100);
        let lv = u.level_up(&mut Rng::new(9));
        assert_eq!(lv.gains, Stats::new(0, 0, 0, 0, 0, 0, 1));
        assert_eq!(u.stats.res, 30);
    }

    #[test]
    fn xp_rolls_over_into_level() {
        let mut u = dummy(Class::Lord, Team::Player, Stats::new(20, 5, 5, 5, 5, 5, 5), weapons::iron_sword());
        let mut rng = Rng::new(5);
        u.xp = 95;
        assert!(u.gain_xp(4, &mut rng).is_none());
        assert_eq!(u.xp, 99);
        let lv = u.gain_xp(10, &mut rng);
        assert!(lv.is_some());
        assert_eq!(u.level, 2);
        assert_eq!(u.xp, 9);
    }

    #[test]
    fn no_xp_at_max_level() {
        let mut u = dummy(Class::Lord, Team::Player, Stats::new(20, 5, 5, 5, 5, 5, 5), weapons::iron_sword());
        u.level = MAX_LEVEL;
        assert!(u.gain_xp(100, &mut Rng::new(1)).is_none());
        assert_eq!(u.xp, 0);
    }
}
