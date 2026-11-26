//! Combat formulas, forecast and resolution.
//!
//! Attack  = Pow + Mt (+1 with triangle advantage, -1 with disadvantage)
//! Defense = Def (or Res vs magic) + terrain defense
//! Hit     = Wpn hit + 2*Skl + Lck/2 (+/-15 triangle) - (2*Spd + Lck + terrain avoid of foe)
//! Crit    = Wpn crit + Skl/2 - foe Lck, crits deal triple damage
//! A unit strikes twice when its Spd exceeds the foe's by 4 or more.

use super::rng::Rng;
use super::terrain::Terrain;
use super::unit::{Unit, Weapon, WeaponKind};

pub const TRIANGLE_HIT: i32 = 15;
pub const TRIANGLE_DMG: i32 = 1;
pub const DOUBLE_THRESHOLD: i32 = 4;
pub const CRIT_MULT: i32 = 3;

/// Situational modifiers (from nearby support partners).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bonus {
    pub hit: i32,
    pub avoid: i32,
    pub crit: i32,
    /// Reduces the foe's crit chance.
    pub dodge: i32,
}

/// One side of a fight: the unit, the weapon it fights with (if any) and its tile.
pub struct Combatant<'a> {
    pub unit: &'a Unit,
    pub weapon: Option<Weapon>,
    pub terrain: Terrain,
    pub bonus: Bonus,
}

impl<'a> Combatant<'a> {
    pub fn new(unit: &'a Unit, terrain: Terrain) -> Self {
        Combatant { unit, weapon: unit.weapon(), terrain, bonus: Bonus::default() }
    }

    pub fn with_bonus(mut self, bonus: Bonus) -> Self {
        self.bonus = bonus;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SideForecast {
    pub hp: i32,
    pub max_hp: i32,
    pub damage: i32,
    pub hit: i32,
    pub crit: i32,
    /// 0 = cannot strike back, 1 = normal, 2 = doubles.
    pub strikes: i32,
    /// +1 advantage, -1 disadvantage.
    pub triangle: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Forecast {
    pub attacker: SideForecast,
    pub defender: SideForecast,
}

pub fn triangle(w: &Weapon, foe: Option<&Weapon>) -> i32 {
    foe.map_or(0, |f| w.kind.triangle(f.kind))
}

pub fn attack_power(u: &Unit, w: &Weapon, foe_w: Option<&Weapon>) -> i32 {
    u.stats.pow + w.might + TRIANGLE_DMG * triangle(w, foe_w)
}

pub fn defense_against(u: &Unit, terrain: Terrain, foe_w: &Weapon) -> i32 {
    let base = if foe_w.kind.is_magic() { u.stats.res } else { u.stats.def };
    base + terrain.defense()
}

/// Bows are deadly against flying units (triple might).
pub fn effective(w: &Weapon, foe: &Unit) -> bool {
    w.kind == WeaponKind::Bow && foe.class.is_flying()
}

/// Fliers get no terrain bonuses.
fn terrain_for(u: &Unit, t: Terrain) -> Terrain {
    if u.class.is_flying() {
        Terrain::Plains
    } else {
        t
    }
}

pub fn damage(a: &Combatant, d: &Combatant) -> i32 {
    match &a.weapon {
        Some(w) => {
            let bonus = if effective(w, d.unit) { w.might * 2 } else { 0 };
            (attack_power(a.unit, w, d.weapon.as_ref()) + bonus - defense_against(d.unit, terrain_for(d.unit, d.terrain), w)).max(0)
        }
        None => 0,
    }
}

pub fn accuracy(u: &Unit, w: &Weapon, foe_w: Option<&Weapon>) -> i32 {
    w.hit + u.stats.skl * 2 + u.stats.lck / 2 + TRIANGLE_HIT * triangle(w, foe_w)
}

pub fn avoid(u: &Unit, terrain: Terrain) -> i32 {
    u.stats.spd * 2 + u.stats.lck + terrain.avoid()
}

pub fn hit_chance(a: &Combatant, d: &Combatant) -> i32 {
    match &a.weapon {
        Some(w) => (accuracy(a.unit, w, d.weapon.as_ref()) + a.bonus.hit - avoid(d.unit, terrain_for(d.unit, d.terrain)) - d.bonus.avoid).clamp(0, 100),
        None => 0,
    }
}

pub fn crit_chance(a: &Combatant, d: &Combatant) -> i32 {
    match &a.weapon {
        Some(w) => (w.crit + a.unit.stats.skl / 2 + a.bonus.crit - d.unit.stats.lck - d.bonus.dodge).clamp(0, 100),
        None => 0,
    }
}

pub fn doubles(u: &Unit, foe: &Unit) -> bool {
    u.stats.spd - foe.stats.spd >= DOUBLE_THRESHOLD
}

fn side(me: &Combatant, foe: &Combatant, distance: i32) -> SideForecast {
    let can = me.weapon.as_ref().is_some_and(|w| w.can_attack() && w.in_range(distance));
    SideForecast {
        hp: me.unit.hp,
        max_hp: me.unit.stats.hp,
        damage: damage(me, foe),
        hit: hit_chance(me, foe),
        crit: crit_chance(me, foe),
        strikes: if !can {
            0
        } else if doubles(me.unit, foe.unit) {
            2
        } else {
            1
        },
        triangle: me.weapon.as_ref().map_or(0, |w| triangle(w, foe.weapon.as_ref())),
    }
}

/// Predicts a fight where `a` attacks `d` from `distance` tiles away.
pub fn forecast(a: &Combatant, d: &Combatant, distance: i32) -> Forecast {
    Forecast { attacker: side(a, d, distance), defender: side(d, a, distance) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Attacker,
    Defender,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Strike {
    pub by: Side,
    pub hit: bool,
    pub crit: bool,
    pub damage: i32,
    /// Target HP after this strike.
    pub target_hp: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CombatOutcome {
    pub strikes: Vec<Strike>,
    pub attacker_hp: i32,
    pub defender_hp: i32,
}

impl CombatOutcome {
    pub fn landed_damage(&self, by: Side) -> bool {
        self.strikes.iter().any(|s| s.by == by && s.hit && s.damage > 0)
    }

    pub fn strikes_by(&self, by: Side) -> usize {
        self.strikes.iter().filter(|s| s.by == by).count()
    }
}

/// Plays out the fight: attacker, defender counter, then follow-up strikes
/// for whoever doubles. Stops as soon as someone reaches 0 HP.
pub fn resolve(f: &Forecast, rng: &mut Rng) -> CombatOutcome {
    let mut hp_a = f.attacker.hp;
    let mut hp_d = f.defender.hp;
    let mut order = Vec::with_capacity(4);
    if f.attacker.strikes >= 1 {
        order.push(Side::Attacker);
    }
    if f.defender.strikes >= 1 {
        order.push(Side::Defender);
    }
    if f.attacker.strikes >= 2 {
        order.push(Side::Attacker);
    }
    if f.defender.strikes >= 2 {
        order.push(Side::Defender);
    }

    let mut strikes = Vec::new();
    for by in order {
        if hp_a <= 0 || hp_d <= 0 {
            break;
        }
        let (me, target_hp) = match by {
            Side::Attacker => (&f.attacker, &mut hp_d),
            Side::Defender => (&f.defender, &mut hp_a),
        };
        let hit = rng.roll(me.hit);
        let crit = hit && rng.roll(me.crit);
        let dmg = if !hit {
            0
        } else if crit {
            me.damage * CRIT_MULT
        } else {
            me.damage
        };
        let dealt = dmg.min(*target_hp);
        *target_hp -= dealt;
        strikes.push(Strike { by, hit, crit, damage: dealt, target_hp: *target_hp });
    }
    CombatOutcome { strikes, attacker_hp: hp_a, defender_hp: hp_d }
}

/// Experience for a player unit after a fight (levels are effective levels).
pub fn combat_xp(level: i32, foe_level: i32, landed_damage: bool, killed: bool, foe_is_boss: bool) -> i32 {
    if !landed_damage && !killed {
        return 1;
    }
    let diff = foe_level - level;
    let mut xp = ((31 + diff) / 3).max(1);
    if killed {
        xp += (20 + diff * 3).max(5);
        if foe_is_boss {
            xp += 40;
        }
    }
    xp.clamp(1, 100)
}

pub const HEAL_XP: i32 = 12;

pub fn heal_amount(healer: &Unit, staff: &Weapon) -> i32 {
    healer.stats.pow + staff.might
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::items::ItemId;
    use crate::logic::unit::tests::dummy;
    use crate::logic::unit::{Class, Stats, Team};

    fn stats(pow: i32, skl: i32, spd: i32, lck: i32, def: i32, res: i32) -> Stats {
        Stats::new(30, pow, skl, spd, lck, def, res)
    }

    fn c(u: &Unit, t: Terrain) -> Combatant<'_> {
        Combatant::new(u, t)
    }

    #[test]
    fn damage_includes_terrain_and_triangle() {
        // Sword (adv) vs Axe: pow 5 + mt 5 + 1 = 11 vs def 3 + forest 1 = 4 -> 7
        let a = dummy(Class::Lord, Team::Player, stats(5, 5, 5, 0, 3, 0), ItemId::IronSword);
        let d = dummy(Class::Brigand, Team::Enemy, stats(5, 0, 0, 0, 3, 0), ItemId::IronAxe);
        assert_eq!(damage(&c(&a, Terrain::Plains), &c(&d, Terrain::Forest)), 7);
        // Axe (disadv) vs Sword: 5 + 8 - 1 = 12 vs 3 -> 9
        assert_eq!(damage(&c(&d, Terrain::Plains), &c(&a, Terrain::Plains)), 9);
        let l = dummy(Class::Mercenary, Team::Enemy, stats(5, 0, 0, 0, 3, 0), ItemId::IronSword);
        assert_eq!(damage(&c(&a, Terrain::Plains), &c(&l, Terrain::Plains)), 7);
    }

    #[test]
    fn magic_targets_resistance() {
        let m = dummy(Class::Mage, Team::Player, stats(6, 5, 5, 0, 1, 5), ItemId::Ember);
        let d = dummy(Class::Soldier, Team::Enemy, stats(5, 0, 0, 0, 10, 1), ItemId::IronLance);
        assert_eq!(damage(&c(&m, Terrain::Plains), &c(&d, Terrain::Plains)), 10);
    }

    #[test]
    fn damage_never_negative() {
        let a = dummy(Class::Archer, Team::Player, stats(0, 0, 0, 0, 0, 0), ItemId::ShortBow);
        let d = dummy(Class::Soldier, Team::Enemy, stats(0, 0, 0, 0, 25, 0), ItemId::IronLance);
        assert_eq!(damage(&c(&a, Terrain::Plains), &c(&d, Terrain::Fort)), 0);
    }

    #[test]
    fn hit_and_avoid_with_terrain() {
        // Hit: 90 + 2*5 + 4/2 = 102. Avoid: 2*5 + 4 + 20(forest) = 34 -> 68
        let a = dummy(Class::Mercenary, Team::Player, stats(5, 5, 5, 4, 3, 0), ItemId::IronSword);
        let d = dummy(Class::Mercenary, Team::Enemy, stats(5, 5, 5, 4, 3, 0), ItemId::IronSword);
        assert_eq!(hit_chance(&c(&a, Terrain::Plains), &c(&d, Terrain::Forest)), 68);
        let ax = dummy(Class::Brigand, Team::Enemy, stats(5, 5, 5, 4, 3, 0), ItemId::IronAxe);
        assert_eq!(hit_chance(&c(&a, Terrain::Plains), &c(&ax, Terrain::Forest)), 83);
    }

    #[test]
    fn hit_is_clamped() {
        let a = dummy(Class::Lord, Team::Player, stats(5, 20, 0, 20, 0, 0), ItemId::Oathblade);
        let d = dummy(Class::Brigand, Team::Enemy, stats(5, 0, 0, 0, 0, 0), ItemId::IronAxe);
        assert_eq!(hit_chance(&c(&a, Terrain::Plains), &c(&d, Terrain::Plains)), 100);
        let fast = dummy(Class::Soldier, Team::Enemy, stats(5, 0, 30, 30, 0, 0), ItemId::IronLance);
        let weak = dummy(Class::Brigand, Team::Player, stats(5, 0, 0, 0, 0, 0), ItemId::IronAxe);
        assert_eq!(hit_chance(&c(&weak, Terrain::Plains), &c(&fast, Terrain::Mountain)), 0);
    }

    #[test]
    fn support_bonus_shifts_hit_and_crit() {
        let a = dummy(Class::Mercenary, Team::Player, stats(5, 5, 5, 4, 3, 0), ItemId::IronSword);
        let d = dummy(Class::Mercenary, Team::Enemy, stats(5, 5, 5, 4, 3, 0), ItemId::KeenSword);
        let base = hit_chance(&c(&a, Terrain::Plains), &c(&d, Terrain::Forest));
        let bonus = Bonus { hit: 10, avoid: 5, crit: 4, dodge: 6 };
        let ca = c(&a, Terrain::Plains).with_bonus(bonus);
        assert_eq!(hit_chance(&ca, &c(&d, Terrain::Forest)), base + 10);
        let cd = c(&d, Terrain::Forest);
        assert_eq!(hit_chance(&cd, &c(&a, Terrain::Plains).with_bonus(bonus)), hit_chance(&cd, &c(&a, Terrain::Plains)) - 5);
        assert_eq!(crit_chance(&cd, &c(&a, Terrain::Plains).with_bonus(bonus)), 25 + 2 - 4 - 6);
    }

    #[test]
    fn bows_are_effective_against_fliers() {
        let archer = dummy(Class::Archer, Team::Player, stats(5, 5, 5, 0, 3, 0), ItemId::ShortBow);
        let peg = dummy(Class::PegasusKnight, Team::Enemy, stats(5, 0, 0, 0, 3, 0), ItemId::IronLance);
        let knight = dummy(Class::Knight, Team::Enemy, stats(5, 0, 0, 0, 3, 0), ItemId::IronLance);
        // 5 + 6 + 12 - 3 = 20 vs 5 + 6 - 3 = 8
        assert_eq!(damage(&c(&archer, Terrain::Plains), &c(&peg, Terrain::Plains)), 20);
        assert_eq!(damage(&c(&archer, Terrain::Plains), &c(&knight, Terrain::Plains)), 8);
        // Fliers ignore forest bonuses.
        assert_eq!(damage(&c(&archer, Terrain::Plains), &c(&peg, Terrain::Forest)), 20);
    }

    #[test]
    fn forged_weapons_are_stronger() {
        use crate::logic::items::Item;
        let mut it = Item::new(ItemId::IronSword);
        let base = it.weapon().unwrap();
        it.forge = 2;
        let w = it.weapon().unwrap();
        assert_eq!((w.might, w.hit, w.crit), (base.might + 2, base.hit + 10, base.crit + 6));
        assert_eq!(it.name(), "Iron Sword+2");
        assert!(it.can_forge());
        it.forge = crate::logic::items::MAX_FORGE;
        assert!(!it.can_forge());
        assert!(!Item::new(ItemId::Mend).can_forge());
    }

    #[test]
    fn crit_reduced_by_luck() {
        let a = dummy(Class::Mercenary, Team::Enemy, stats(5, 10, 5, 0, 3, 0), ItemId::KeenSword);
        let d = dummy(Class::Lord, Team::Player, stats(5, 5, 5, 7, 3, 0), ItemId::Oathblade);
        assert_eq!(crit_chance(&c(&a, Terrain::Plains), &c(&d, Terrain::Plains)), 25 + 5 - 7);
    }

    #[test]
    fn doubling_needs_four_speed() {
        let a = dummy(Class::Lord, Team::Player, stats(5, 5, 9, 0, 3, 0), ItemId::IronSword);
        let b = dummy(Class::Brigand, Team::Enemy, stats(5, 5, 5, 0, 3, 0), ItemId::IronAxe);
        let s = dummy(Class::Brigand, Team::Enemy, stats(5, 5, 6, 0, 3, 0), ItemId::IronAxe);
        let p = Terrain::Plains;
        assert_eq!(forecast(&c(&a, p), &c(&b, p), 1).attacker.strikes, 2);
        assert_eq!(forecast(&c(&a, p), &c(&s, p), 1).attacker.strikes, 1);
        let f = forecast(&c(&b, p), &c(&a, p), 1);
        assert_eq!((f.attacker.strikes, f.defender.strikes), (1, 2));
    }

    #[test]
    fn counterattack_requires_range_and_weapon() {
        let p = Terrain::Plains;
        let archer = dummy(Class::Archer, Team::Player, stats(5, 5, 5, 0, 3, 0), ItemId::ShortBow);
        let soldier = dummy(Class::Soldier, Team::Enemy, stats(5, 5, 5, 0, 3, 0), ItemId::IronLance);
        let mage = dummy(Class::Mage, Team::Enemy, stats(5, 5, 5, 0, 3, 0), ItemId::Ember);
        let cleric = dummy(Class::Cleric, Team::Player, stats(5, 5, 5, 0, 3, 0), ItemId::Mend);
        assert_eq!(forecast(&c(&archer, p), &c(&soldier, p), 2).defender.strikes, 0);
        assert_eq!(forecast(&c(&archer, p), &c(&mage, p), 2).defender.strikes, 1);
        assert_eq!(forecast(&c(&soldier, p), &c(&archer, p), 1).defender.strikes, 0);
        let f = forecast(&c(&soldier, p), &c(&cleric, p), 1);
        assert_eq!(f.defender.strikes, 0, "healers never counter");
        assert_eq!(f.defender.damage, 0);
        // Javelin reaches 2: lancer with javelin counters an archer.
        let mut jav = dummy(Class::Lancer, Team::Enemy, stats(5, 5, 5, 0, 3, 0), ItemId::Javelin);
        jav.give(ItemId::IronLance);
        assert_eq!(forecast(&c(&archer, p), &c(&jav, p), 2).defender.strikes, 1);
    }

    #[test]
    fn unusable_weapon_counts_as_unarmed() {
        let p = Terrain::Plains;
        // A mage holding a sword can't use it.
        let m = dummy(Class::Mage, Team::Enemy, stats(5, 5, 5, 0, 3, 0), ItemId::IronSword);
        let a = dummy(Class::Lord, Team::Player, stats(5, 5, 5, 0, 3, 0), ItemId::IronSword);
        assert!(m.weapon().is_none());
        assert_eq!(forecast(&c(&a, p), &c(&m, p), 1).defender.strikes, 0);
    }

    fn sure_forecast(att_dmg: i32, att_strikes: i32, att_hp: i32, def_dmg: i32, def_strikes: i32, def_hp: i32, crit: i32) -> Forecast {
        Forecast {
            attacker: SideForecast { hp: att_hp, max_hp: att_hp, damage: att_dmg, hit: 100, crit, strikes: att_strikes, triangle: 0 },
            defender: SideForecast { hp: def_hp, max_hp: def_hp, damage: def_dmg, hit: 100, crit, strikes: def_strikes, triangle: 0 },
        }
    }

    #[test]
    fn resolve_order_and_doubling() {
        let out = resolve(&sure_forecast(5, 2, 30, 3, 1, 30, 0), &mut Rng::new(1));
        let order: Vec<Side> = out.strikes.iter().map(|s| s.by).collect();
        assert_eq!(order, vec![Side::Attacker, Side::Defender, Side::Attacker]);
        assert_eq!((out.attacker_hp, out.defender_hp), (27, 20));
        assert_eq!(out.strikes_by(Side::Attacker), 2);
    }

    #[test]
    fn resolve_stops_on_kill() {
        let out = resolve(&sure_forecast(10, 2, 30, 3, 2, 8, 0), &mut Rng::new(1));
        assert_eq!(out.strikes.len(), 1);
        assert_eq!(out.defender_hp, 0);
        assert_eq!(out.strikes[0].damage, 8, "overkill damage is capped at remaining HP");
    }

    #[test]
    fn counter_can_kill_attacker() {
        let out = resolve(&sure_forecast(1, 2, 4, 5, 1, 30, 0), &mut Rng::new(1));
        assert_eq!(out.attacker_hp, 0);
        assert_eq!(out.strikes.len(), 2, "attacker's follow-up never happens");
    }

    #[test]
    fn crits_triple_damage() {
        let out = resolve(&sure_forecast(4, 1, 30, 0, 0, 30, 100), &mut Rng::new(3));
        assert!(out.strikes[0].crit);
        assert_eq!(out.defender_hp, 18);
    }

    #[test]
    fn zero_hit_always_misses() {
        let mut f = sure_forecast(9, 2, 30, 9, 2, 30, 100);
        f.attacker.hit = 0;
        f.defender.hit = 0;
        let out = resolve(&f, &mut Rng::new(11));
        assert_eq!(out.strikes.len(), 4);
        assert!(out.strikes.iter().all(|s| !s.hit && !s.crit && s.damage == 0));
    }

    #[test]
    fn xp_formula() {
        assert_eq!(combat_xp(1, 1, false, false, false), 1);
        assert_eq!(combat_xp(1, 1, true, false, false), 10);
        assert_eq!(combat_xp(1, 1, true, true, false), 30);
        assert_eq!(combat_xp(1, 5, true, true, true), 11 + 32 + 40);
        assert!(combat_xp(15, 1, true, true, false) >= 6);
        assert!(combat_xp(1, 30, true, true, true) <= 100);
    }
}
