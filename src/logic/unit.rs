use serde::{Deserialize, Serialize};

use super::items::{Item, ItemId, ItemKind, MAX_ITEMS};
use super::rng::Rng;
use super::terrain::MoveType;
use super::Pos;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Team {
    Player,
    Enemy,
}

impl Team {
    pub fn is_foe(self, other: Team) -> bool {
        self != other
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

/// Static weapon stats (built from the item catalogue).
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Class {
    // Base classes
    Lord,
    Mercenary,
    Lancer,
    Soldier,
    Fighter,
    Brigand,
    Knight,
    Archer,
    Mage,
    Cleric,
    Cavalier,
    PegasusKnight,
    Thief,
    // Promoted classes
    Highlord,
    Hero,
    Halberdier,
    Warrior,
    General,
    Sniper,
    Sage,
    Bishop,
    Paladin,
    FalconKnight,
    Rogue,
    // Enemy-only bosses
    Warlord,
    Sorcerer,
}

pub struct ClassInfo {
    pub name: &'static str,
    pub kinds: &'static [WeaponKind],
    pub mov: i32,
    pub move_type: MoveType,
    pub base: Stats,
    pub growth: Stats,
    pub promotes_to: Option<Class>,
    pub promoted: bool,
}

impl Class {
    pub fn info(self) -> ClassInfo {
        use Class::*;
        use MoveType::*;
        use WeaponKind::*;
        let s = Stats::new;
        let (name, kinds, mov, mt, base, growth, promotes_to, promoted): (_, &'static [WeaponKind], _, _, _, _, _, _) = match self {
            Lord => ("Lord", &[Sword], 5, Infantry, s(18, 5, 6, 7, 5, 5, 2), s(80, 45, 50, 55, 45, 30, 25), Some(Highlord), false),
            Mercenary => ("Mercenary", &[Sword], 5, Infantry, s(20, 5, 8, 7, 2, 4, 1), s(80, 40, 45, 45, 25, 25, 15), Some(Hero), false),
            Lancer => ("Lancer", &[Lance], 5, Infantry, s(20, 6, 4, 5, 2, 6, 1), s(85, 45, 35, 35, 25, 35, 10), Some(Halberdier), false),
            Soldier => ("Soldier", &[Lance], 5, Infantry, s(20, 5, 3, 3, 0, 5, 0), s(80, 40, 30, 30, 20, 30, 10), Some(Halberdier), false),
            Fighter => ("Fighter", &[Axe], 5, Mountaineer, s(24, 7, 3, 4, 1, 3, 0), s(90, 55, 30, 30, 20, 20, 5), Some(Warrior), false),
            Brigand => ("Brigand", &[Axe], 5, Mountaineer, s(22, 6, 2, 3, 0, 3, 0), s(85, 50, 25, 25, 10, 20, 5), Some(Warrior), false),
            Knight => ("Knight", &[Lance], 4, Infantry, s(18, 6, 3, 1, 1, 10, 1), s(80, 40, 30, 15, 20, 45, 10), Some(General), false),
            Archer => ("Archer", &[Bow], 5, Infantry, s(17, 5, 6, 5, 2, 3, 2), s(70, 45, 50, 45, 30, 20, 20), Some(Sniper), false),
            Mage => ("Mage", &[Magic], 5, Infantry, s(16, 6, 5, 5, 2, 2, 5), s(55, 55, 45, 45, 25, 15, 40), Some(Sage), false),
            Cleric => ("Cleric", &[Staff], 5, Infantry, s(15, 5, 3, 6, 6, 1, 7), s(50, 50, 30, 50, 55, 10, 55), Some(Bishop), false),
            Cavalier => ("Cavalier", &[Sword, Lance], 7, Mounted, s(20, 6, 5, 6, 3, 6, 1), s(80, 45, 40, 45, 35, 30, 15), Some(Paladin), false),
            PegasusKnight => ("Pegasus Knight", &[Lance], 7, Flying, s(16, 5, 6, 8, 5, 4, 6), s(60, 40, 45, 60, 50, 15, 40), Some(FalconKnight), false),
            Thief => ("Thief", &[Sword], 6, Infantry, s(16, 4, 6, 9, 4, 2, 1), s(60, 35, 50, 60, 45, 15, 20), Some(Rogue), false),
            Highlord => ("Highlord", &[Sword, Lance], 6, Infantry, s(24, 8, 9, 10, 6, 8, 5), s(80, 45, 50, 55, 45, 30, 25), None, true),
            Hero => ("Hero", &[Sword, Axe], 6, Infantry, s(26, 8, 11, 10, 3, 7, 3), s(80, 40, 45, 45, 25, 25, 15), None, true),
            Halberdier => ("Halberdier", &[Lance, Sword], 6, Infantry, s(26, 9, 7, 8, 3, 9, 3), s(85, 45, 35, 35, 25, 35, 10), None, true),
            Warrior => ("Warrior", &[Axe, Bow], 6, Mountaineer, s(30, 10, 6, 6, 2, 6, 2), s(90, 55, 30, 30, 20, 20, 5), None, true),
            General => ("General", &[Lance, Axe], 5, Infantry, s(24, 9, 6, 3, 2, 13, 4), s(80, 40, 30, 15, 20, 45, 10), None, true),
            Sniper => ("Sniper", &[Bow, Sword], 6, Infantry, s(22, 7, 10, 8, 3, 6, 4), s(70, 45, 50, 45, 30, 20, 20), None, true),
            Sage => ("Sage", &[Magic, Staff], 6, Infantry, s(21, 9, 8, 8, 3, 4, 8), s(55, 55, 45, 45, 25, 15, 40), None, true),
            Bishop => ("Bishop", &[Staff, Magic], 6, Infantry, s(20, 8, 5, 8, 7, 3, 10), s(50, 50, 30, 50, 55, 10, 55), None, true),
            Paladin => ("Paladin", &[Sword, Lance, Axe], 8, Mounted, s(26, 9, 8, 9, 4, 9, 4), s(80, 45, 40, 45, 35, 30, 15), None, true),
            FalconKnight => ("Falcon Knight", &[Lance, Sword], 8, Flying, s(22, 8, 10, 12, 6, 6, 9), s(60, 40, 45, 60, 50, 15, 40), None, true),
            Rogue => ("Rogue", &[Sword], 7, Infantry, s(22, 7, 11, 13, 6, 5, 4), s(60, 35, 50, 60, 45, 15, 20), None, true),
            Warlord => ("Warlord", &[Axe, Sword], 6, Mountaineer, s(30, 10, 8, 7, 3, 8, 3), s(80, 45, 30, 30, 20, 30, 15), None, true),
            Sorcerer => ("Sorcerer", &[Magic], 5, Infantry, s(30, 12, 10, 8, 5, 7, 12), s(60, 55, 40, 40, 30, 25, 45), None, true),
        };
        ClassInfo { name, kinds, mov, move_type: mt, base, growth, promotes_to, promoted }
    }

    pub fn name(self) -> &'static str {
        self.info().name
    }

    pub fn move_range(self) -> i32 {
        self.info().mov
    }

    pub fn move_type(self) -> MoveType {
        self.info().move_type
    }

    pub fn is_promoted(self) -> bool {
        self.info().promoted
    }

    pub fn can_wield(self, kind: WeaponKind) -> bool {
        self.info().kinds.contains(&kind)
    }

    pub fn is_flying(self) -> bool {
        self.info().move_type == MoveType::Flying
    }

    /// Thieves pick locks and steal.
    pub fn is_thief(self) -> bool {
        matches!(self, Class::Thief | Class::Rogue)
    }

    pub fn uses_magic(self) -> bool {
        self.info().kinds.iter().any(|k| k.is_magic())
    }

    /// Unpromoted class whose sprite this class builds on.
    pub fn base_look(self) -> Class {
        use Class::*;
        match self {
            Highlord => Lord,
            Hero => Mercenary,
            Halberdier => Lancer,
            Warrior => Fighter,
            General => Knight,
            Sniper => Archer,
            Sage => Mage,
            Bishop => Cleric,
            Paladin => Cavalier,
            FalconKnight => PegasusKnight,
            Rogue => Thief,
            c => c,
        }
    }
}

pub const STAT_NAMES: [&str; 7] = ["HP", "Str", "Skl", "Spd", "Lck", "Def", "Res"];
pub const STAT_CAPS: [i32; 7] = [60, 30, 30, 30, 30, 30, 30];
pub const MAX_LEVEL: i32 = 20;
pub const PROMOTE_LEVEL: i32 = 10;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AiMode {
    /// Attack if possible, otherwise march on the nearest foe.
    #[default]
    Aggressive,
    /// Attack anything that comes within reach, but don't advance.
    Guard,
    /// Never moves; attacks what is already in range.
    Stationary,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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
    /// Inventory; the first usable attack weapon is the equipped one.
    pub items: Vec<Item>,
    pub pos: Pos,
    pub is_lord: bool,
    pub is_boss: bool,
    pub ai: AiMode,
    /// Can be recruited by the lord with Talk.
    pub talk: bool,
    /// Has acted this phase.
    pub done: bool,
    pub alive: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Promotion {
    pub from: Class,
    pub to: Class,
    pub gains: Stats,
}

impl Unit {
    #[allow(clippy::too_many_arguments)]
    pub fn new(name: &str, class: Class, team: Team, level: i32, stats: Stats, growths: Stats, items: &[ItemId], pos: Pos) -> Unit {
        Unit {
            name: name.to_string(),
            class,
            team,
            level,
            xp: 0,
            hp: stats.hp,
            stats,
            growths,
            items: items.iter().map(|i| Item::new(*i)).collect(),
            pos,
            is_lord: false,
            is_boss: false,
            ai: AiMode::Aggressive,
            talk: false,
            done: false,
            alive: true,
        }
    }

    /// A generic soldier whose stats are the class base plus average growths.
    pub fn generic(name: &str, class: Class, team: Team, level: i32, items: &[ItemId], pos: Pos) -> Unit {
        let info = class.info();
        let base = info.base.to_array();
        let growth = info.growth.to_array();
        let mut st = [0; 7];
        for i in 0..7 {
            st[i] = (base[i] + growth[i] * (level - 1) / 100).min(STAT_CAPS[i]);
        }
        Unit::new(name, class, team, level, Stats::from_array(st), info.growth, items, pos)
    }

    pub fn lord(mut self) -> Self {
        self.is_lord = true;
        self
    }

    pub fn boss(mut self) -> Self {
        self.is_boss = true;
        self.ai = AiMode::Stationary;
        self
    }

    pub fn with_ai(mut self, ai: AiMode) -> Self {
        self.ai = ai;
        self
    }

    pub fn talkable(mut self) -> Self {
        self.talk = true;
        self.ai = AiMode::Guard;
        self
    }

    pub fn mov(&self) -> i32 {
        if self.ai == AiMode::Stationary {
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

    /// Level used for experience math (promoted units count as 10 higher).
    pub fn effective_level(&self) -> i32 {
        self.level + if self.class.is_promoted() { 10 } else { 0 }
    }

    pub fn can_use(&self, id: ItemId) -> bool {
        match id.info().kind {
            ItemKind::Weapon(w) => self.class.can_wield(w.kind),
            ItemKind::Heal(_) | ItemKind::Key(_) => true,
            ItemKind::Seal => self.class.info().promotes_to.is_some(),
        }
    }

    /// Indices and stats of all usable attack weapons, in inventory order.
    pub fn attack_weapons(&self) -> Vec<(usize, Weapon)> {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(i, it)| it.weapon().filter(|w| w.can_attack() && self.class.can_wield(w.kind)).map(|w| (i, w)))
            .collect()
    }

    pub fn weapon_index(&self) -> Option<usize> {
        self.attack_weapons().first().map(|(i, _)| *i)
    }

    /// The equipped weapon: first usable attack weapon.
    pub fn weapon(&self) -> Option<Weapon> {
        self.attack_weapons().into_iter().next().map(|(_, w)| w)
    }

    pub fn staff_index(&self) -> Option<usize> {
        self.items.iter().position(|it| it.id.weapon().is_some_and(|w| w.kind == WeaponKind::Staff && self.class.can_wield(w.kind)))
    }

    pub fn staff(&self) -> Option<Weapon> {
        self.staff_index().and_then(|i| self.items[i].weapon())
    }

    /// Combined min/max attack range over all usable weapons.
    pub fn attack_range(&self) -> Option<(i32, i32)> {
        let ws = self.attack_weapons();
        if ws.is_empty() {
            return None;
        }
        let min = ws.iter().map(|(_, w)| w.min_range).min().unwrap();
        let max = ws.iter().map(|(_, w)| w.max_range).max().unwrap();
        Some((min, max))
    }

    /// Moves item `idx` to the front of the inventory (equipping it if it's a weapon).
    pub fn equip(&mut self, idx: usize) {
        if idx < self.items.len() {
            let it = self.items.remove(idx);
            self.items.insert(0, it);
        }
    }

    /// Equips the first usable weapon that can hit at `dist`. Returns false if none can.
    pub fn equip_for_range(&mut self, dist: i32) -> bool {
        if self.weapon().is_some_and(|w| w.in_range(dist)) {
            return true;
        }
        match self.attack_weapons().into_iter().find(|(_, w)| w.in_range(dist)) {
            Some((i, _)) => {
                self.equip(i);
                true
            }
            None => false,
        }
    }

    /// Spends one use of item `idx`. Returns the item's name if it broke / ran out.
    pub fn spend(&mut self, idx: usize) -> Option<&'static str> {
        let it = self.items.get_mut(idx)?;
        it.uses -= 1;
        if it.uses <= 0 {
            let name = it.id.name();
            self.items.remove(idx);
            Some(name)
        } else {
            None
        }
    }

    pub fn give(&mut self, id: ItemId) -> bool {
        if self.items.len() >= MAX_ITEMS {
            return false;
        }
        self.items.push(Item::new(id));
        true
    }

    pub fn can_promote(&self) -> bool {
        self.level >= PROMOTE_LEVEL && self.class.info().promotes_to.is_some()
    }

    /// Changes class, raising stats to at least the new class's base plus a flat bonus. Level resets to 1.
    pub fn promote(&mut self) -> Option<Promotion> {
        let to = self.class.info().promotes_to?;
        let from = self.class;
        let bonus = [4, 2, 2, 2, 0, 2, 2];
        let base = to.info().base.to_array();
        let mut st = self.stats.to_array();
        let mut gains = [0; 7];
        for i in 0..7 {
            let new = (st[i] + bonus[i]).max(base[i]).min(STAT_CAPS[i]);
            gains[i] = new - st[i];
            st[i] = new;
        }
        self.stats = Stats::from_array(st);
        self.hp = (self.hp + gains[0]).min(self.stats.hp);
        self.class = to;
        self.level = 1;
        self.xp = 0;
        Some(Promotion { from, to, gains: Stats::from_array(gains) })
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
            let best = (0..7).filter(|&i| stats[i] < STAT_CAPS[i]).max_by_key(|&i| (growths[i], -(i as i32)));
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
        LevelUp { new_level: self.level, gains: Stats::from_array(gains), after: self.stats }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn dummy(class: Class, team: Team, stats: Stats, weapon: ItemId) -> Unit {
        Unit::new("Test", class, team, 1, stats, Stats::new(50, 50, 50, 50, 50, 50, 50), &[weapon], Pos::new(0, 0))
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
        let mut u = dummy(Class::Lord, Team::Player, Stats::new(20, 5, 5, 5, 5, 5, 5), ItemId::IronSword);
        u.growths = Stats::new(100, 100, 100, 100, 100, 100, 100);
        u.hp = 10;
        let lv = u.level_up(&mut Rng::new(1));
        assert_eq!(lv.new_level, 2);
        assert_eq!(lv.gains, Stats::new(1, 1, 1, 1, 1, 1, 1));
        assert_eq!(u.stats, Stats::new(21, 6, 6, 6, 6, 6, 6));
        assert_eq!(u.hp, 11, "current HP rises with max HP");
    }

    #[test]
    fn level_up_never_empty() {
        let mut u = dummy(Class::Lord, Team::Player, Stats::new(20, 5, 5, 5, 5, 5, 5), ItemId::IronSword);
        u.growths = Stats::default();
        let lv = u.level_up(&mut Rng::new(3));
        assert_eq!(lv.gains.to_array().iter().sum::<i32>(), 1);
    }

    #[test]
    fn stats_respect_caps() {
        let mut u = dummy(Class::Lord, Team::Player, Stats::new(60, 30, 30, 30, 30, 30, 29), ItemId::IronSword);
        u.growths = Stats::new(100, 100, 100, 100, 100, 100, 100);
        let lv = u.level_up(&mut Rng::new(9));
        assert_eq!(lv.gains, Stats::new(0, 0, 0, 0, 0, 0, 1));
    }

    #[test]
    fn xp_rolls_over_into_level() {
        let mut u = dummy(Class::Lord, Team::Player, Stats::new(20, 5, 5, 5, 5, 5, 5), ItemId::IronSword);
        let mut rng = Rng::new(5);
        u.xp = 95;
        assert!(u.gain_xp(4, &mut rng).is_none());
        assert!(u.gain_xp(10, &mut rng).is_some());
        assert_eq!((u.level, u.xp), (2, 9));
    }

    #[test]
    fn no_xp_at_max_level() {
        let mut u = dummy(Class::Lord, Team::Player, Stats::new(20, 5, 5, 5, 5, 5, 5), ItemId::IronSword);
        u.level = MAX_LEVEL;
        assert!(u.gain_xp(100, &mut Rng::new(1)).is_none());
        assert_eq!(u.xp, 0);
    }

    #[test]
    fn equipped_weapon_skips_unusable_and_staves() {
        let mut u = Unit::generic("B", Class::Bishop, Team::Player, 1, &[ItemId::IronSword, ItemId::Mend, ItemId::Ember], Pos::new(0, 0));
        assert_eq!(u.weapon().unwrap().name, "Ember");
        assert_eq!(u.staff().unwrap().name, "Mend Staff");
        assert_eq!(u.attack_range(), Some((1, 2)));
        u.equip(2);
        assert_eq!(u.items[0].id, ItemId::Ember);
        let c = Unit::generic("C", Class::Cleric, Team::Player, 1, &[ItemId::Mend], Pos::new(0, 0));
        assert!(c.weapon().is_none());
        assert_eq!(c.attack_range(), None);
    }

    #[test]
    fn equip_for_range_picks_ranged_weapon() {
        let mut u = Unit::generic("L", Class::Lancer, Team::Player, 1, &[ItemId::IronLance, ItemId::Javelin], Pos::new(0, 0));
        assert_eq!(u.attack_range(), Some((1, 2)));
        assert!(u.equip_for_range(2));
        assert_eq!(u.weapon().unwrap().name, "Javelin");
        assert!(u.equip_for_range(1), "javelin also reaches 1");
        assert!(!u.equip_for_range(3));
    }

    #[test]
    fn durability_breaks_weapons() {
        let mut u = Unit::generic("L", Class::Lancer, Team::Player, 1, &[ItemId::IronLance], Pos::new(0, 0));
        u.items[0].uses = 2;
        assert_eq!(u.spend(0), None);
        assert_eq!(u.spend(0), Some("Iron Lance"));
        assert!(u.items.is_empty());
        assert!(u.weapon().is_none());
    }

    #[test]
    fn promotion() {
        let mut u = Unit::generic("K", Class::Lord, Team::Player, 9, &[ItemId::IronSword], Pos::new(0, 0));
        assert!(!u.can_promote());
        u.level = 10;
        assert!(u.can_promote());
        let before = u.stats;
        let p = u.promote().unwrap();
        assert_eq!((p.from, p.to), (Class::Lord, Class::Highlord));
        assert_eq!(u.level, 1);
        assert!(u.stats.hp >= before.hp + 4);
        assert!(u.stats.spd >= Class::Highlord.info().base.spd);
        assert!(u.class.can_wield(WeaponKind::Lance));
        assert_eq!(u.effective_level(), 11);
        assert!(u.promote().is_none(), "can't promote twice");
    }

    #[test]
    fn generic_stats_scale_with_level() {
        let a = Unit::generic("a", Class::Fighter, Team::Enemy, 1, &[], Pos::new(0, 0));
        let b = Unit::generic("b", Class::Fighter, Team::Enemy, 11, &[], Pos::new(0, 0));
        assert_eq!(a.stats, Class::Fighter.info().base);
        assert_eq!(b.stats.hp, 24 + 9);
        assert_eq!(b.stats.pow, 7 + 5);
    }

    #[test]
    fn inventory_limit() {
        let mut u = Unit::generic("a", Class::Fighter, Team::Player, 1, &[], Pos::new(0, 0));
        for _ in 0..MAX_ITEMS {
            assert!(u.give(ItemId::Vulnerary));
        }
        assert!(!u.give(ItemId::Vulnerary));
    }
}
