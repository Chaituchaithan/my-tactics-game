//! Combat formulas, forecast and resolution.
//!
//! Attack  = Pow + Mt (+1 with triangle advantage, -1 with disadvantage)
//! Defense = Def (or Res vs magic) + terrain defense
//! Hit     = Wpn hit + 2*Skl + Lck/2 (+/-15 triangle) - (2*Spd + Lck + terrain avoid of foe)
//! Crit    = Wpn crit + Skl/2 - foe Lck, crits deal triple damage
//! A unit strikes twice when its Spd exceeds the foe's by 4 or more.

use super::rng::Rng;
use super::terrain::Terrain;
use super::unit::Unit;

pub const TRIANGLE_HIT: i32 = 15;
pub const TRIANGLE_DMG: i32 = 1;
pub const DOUBLE_THRESHOLD: i32 = 4;
pub const CRIT_MULT: i32 = 3;

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

pub fn triangle(u: &Unit, foe: &Unit) -> i32 {
    u.weapon.kind.triangle(foe.weapon.kind)
}

pub fn attack_power(u: &Unit, foe: &Unit) -> i32 {
    u.stats.pow + u.weapon.might + TRIANGLE_DMG * triangle(u, foe)
}

pub fn defense_against(u: &Unit, terrain: Terrain, foe: &Unit) -> i32 {
    let base = if foe.weapon.kind.is_magic() { u.stats.res } else { u.stats.def };
    base + terrain.defense()
}

pub fn damage(att: &Unit, def: &Unit, def_terrain: Terrain) -> i32 {
    (attack_power(att, def) - defense_against(def, def_terrain, att)).max(0)
}

pub fn accuracy(u: &Unit, foe: &Unit) -> i32 {
    u.weapon.hit + u.stats.skl * 2 + u.stats.lck / 2 + TRIANGLE_HIT * triangle(u, foe)
}

pub fn avoid(u: &Unit, terrain: Terrain) -> i32 {
    u.stats.spd * 2 + u.stats.lck + terrain.avoid()
}

pub fn hit_chance(att: &Unit, def: &Unit, def_terrain: Terrain) -> i32 {
    (accuracy(att, def) - avoid(def, def_terrain)).clamp(0, 100)
}

pub fn crit_chance(att: &Unit, def: &Unit) -> i32 {
    (att.weapon.crit + att.stats.skl / 2 - def.stats.lck).clamp(0, 100)
}

pub fn doubles(u: &Unit, foe: &Unit) -> bool {
    u.stats.spd - foe.stats.spd >= DOUBLE_THRESHOLD
}

fn side(u: &Unit, foe: &Unit, foe_terrain: Terrain, can_strike: bool) -> SideForecast {
    SideForecast {
        hp: u.hp,
        max_hp: u.stats.hp,
        damage: damage(u, foe, foe_terrain),
        hit: hit_chance(u, foe, foe_terrain),
        crit: crit_chance(u, foe),
        strikes: if !can_strike {
            0
        } else if doubles(u, foe) {
            2
        } else {
            1
        },
        triangle: triangle(u, foe),
    }
}

/// Predicts a fight where `att` (standing on `att_terrain`) attacks `def`
/// from `distance` tiles away.
pub fn forecast(att: &Unit, att_terrain: Terrain, def: &Unit, def_terrain: Terrain, distance: i32) -> Forecast {
    let att_can = att.weapon.can_attack() && att.weapon.in_range(distance);
    let def_can = def.weapon.can_attack() && def.weapon.in_range(distance);
    Forecast {
        attacker: side(att, def, def_terrain, att_can),
        defender: side(def, att, att_terrain, def_can),
    }
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

/// Experience for a player unit after a fight.
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

pub fn heal_amount(healer: &Unit) -> i32 {
    healer.stats.pow + 10
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::unit::tests::dummy;
    use crate::logic::unit::{weapons, Class, Stats, Team};

    fn stats(pow: i32, skl: i32, spd: i32, lck: i32, def: i32, res: i32) -> Stats {
        Stats::new(30, pow, skl, spd, lck, def, res)
    }

    #[test]
    fn damage_includes_terrain_and_triangle() {
        // Sword (adv) vs Axe: pow 5 + mt 5 + 1 = 11 vs def 3 + forest 1 = 4 -> 7
        let a = dummy(Class::Lord, Team::Player, stats(5, 5, 5, 0, 3, 0), weapons::iron_sword());
        let d = dummy(Class::Brigand, Team::Enemy, stats(5, 0, 0, 0, 3, 0), weapons::iron_axe());
        assert_eq!(damage(&a, &d, Terrain::Forest), 7);
        // Axe (disadv) vs Sword: 5 + 8 - 1 = 12 vs 3 -> 9
        assert_eq!(damage(&d, &a, Terrain::Plains), 9);
        // Same weapon class: no bonus
        let l = dummy(Class::Mercenary, Team::Enemy, stats(5, 0, 0, 0, 3, 0), weapons::iron_sword());
        assert_eq!(damage(&a, &l, Terrain::Plains), 7);
    }

    #[test]
    fn magic_targets_resistance() {
        let m = dummy(Class::Mage, Team::Player, stats(6, 5, 5, 0, 1, 5), weapons::ember());
        let d = dummy(Class::Soldier, Team::Enemy, stats(5, 0, 0, 0, 10, 1), weapons::iron_lance());
        // 6 + 5 - res 1 = 10 (def 10 ignored)
        assert_eq!(damage(&m, &d, Terrain::Plains), 10);
    }

    #[test]
    fn damage_never_negative() {
        let a = dummy(Class::Archer, Team::Player, stats(0, 0, 0, 0, 0, 0), weapons::short_bow());
        let d = dummy(Class::Soldier, Team::Enemy, stats(0, 0, 0, 0, 25, 0), weapons::iron_lance());
        assert_eq!(damage(&a, &d, Terrain::Fort), 0);
    }

    #[test]
    fn hit_and_avoid_with_terrain() {
        // Hit: 90 + 2*5 + 4/2 = 102. Avoid: 2*5 + 4 + 20(forest) = 34 -> 68
        let a = dummy(Class::Mercenary, Team::Player, stats(5, 5, 5, 4, 3, 0), weapons::iron_sword());
        let d = dummy(Class::Mercenary, Team::Enemy, stats(5, 5, 5, 4, 3, 0), weapons::iron_sword());
        assert_eq!(hit_chance(&a, &d, Terrain::Forest), 68);
        // Triangle changes hit by 15
        let ax = dummy(Class::Brigand, Team::Enemy, stats(5, 5, 5, 4, 3, 0), weapons::iron_axe());
        assert_eq!(hit_chance(&a, &ax, Terrain::Forest), 83);
    }

    #[test]
    fn hit_is_clamped() {
        let a = dummy(Class::Lord, Team::Player, stats(5, 20, 0, 20, 0, 0), weapons::oathblade());
        let d = dummy(Class::Soldier, Team::Enemy, stats(5, 0, 0, 0, 0, 0), weapons::iron_axe());
        assert_eq!(hit_chance(&a, &d, Terrain::Plains), 100);
        let fast = dummy(Class::Mercenary, Team::Enemy, stats(5, 0, 30, 30, 0, 0), weapons::iron_lance());
        let weak = dummy(Class::Brigand, Team::Player, stats(5, 0, 0, 0, 0, 0), weapons::iron_axe());
        assert_eq!(hit_chance(&weak, &fast, Terrain::Mountain), 0);
    }

    #[test]
    fn crit_reduced_by_luck() {
        let a = dummy(Class::Mercenary, Team::Enemy, stats(5, 10, 5, 0, 3, 0), weapons::keen_sword());
        let d = dummy(Class::Lord, Team::Player, stats(5, 5, 5, 7, 3, 0), weapons::oathblade());
        assert_eq!(crit_chance(&a, &d), 25 + 5 - 7);
    }

    #[test]
    fn doubling_needs_four_speed() {
        let a = dummy(Class::Lord, Team::Player, stats(5, 5, 9, 0, 3, 0), weapons::iron_sword());
        let b = dummy(Class::Brigand, Team::Enemy, stats(5, 5, 5, 0, 3, 0), weapons::iron_axe());
        let c = dummy(Class::Brigand, Team::Enemy, stats(5, 5, 6, 0, 3, 0), weapons::iron_axe());
        assert_eq!(forecast(&a, Terrain::Plains, &b, Terrain::Plains, 1).attacker.strikes, 2);
        assert_eq!(forecast(&a, Terrain::Plains, &c, Terrain::Plains, 1).attacker.strikes, 1);
        // Defender doubling
        let f = forecast(&b, Terrain::Plains, &a, Terrain::Plains, 1);
        assert_eq!(f.attacker.strikes, 1);
        assert_eq!(f.defender.strikes, 2);
    }

    #[test]
    fn counterattack_requires_range() {
        let archer = dummy(Class::Archer, Team::Player, stats(5, 5, 5, 0, 3, 0), weapons::short_bow());
        let soldier = dummy(Class::Soldier, Team::Enemy, stats(5, 5, 5, 0, 3, 0), weapons::iron_lance());
        let mage = dummy(Class::Mage, Team::Enemy, stats(5, 5, 5, 0, 3, 0), weapons::ember());
        let cleric = dummy(Class::Cleric, Team::Player, stats(5, 5, 5, 0, 3, 0), weapons::mend());
        // Archer shooting from 2: lance can't reply, mage can.
        assert_eq!(forecast(&archer, Terrain::Plains, &soldier, Terrain::Plains, 2).defender.strikes, 0);
        assert_eq!(forecast(&archer, Terrain::Plains, &mage, Terrain::Plains, 2).defender.strikes, 1);
        // Soldier attacks archer in melee: archer cannot counter at range 1.
        assert_eq!(forecast(&soldier, Terrain::Plains, &archer, Terrain::Plains, 1).defender.strikes, 0);
        // Healers never counter.
        assert_eq!(forecast(&soldier, Terrain::Plains, &cleric, Terrain::Plains, 1).defender.strikes, 0);
    }

    fn sure_forecast(att_dmg: i32, att_strikes: i32, att_hp: i32, def_dmg: i32, def_strikes: i32, def_hp: i32, crit: i32) -> Forecast {
        Forecast {
            attacker: SideForecast { hp: att_hp, max_hp: att_hp, damage: att_dmg, hit: 100, crit, strikes: att_strikes, triangle: 0 },
            defender: SideForecast { hp: def_hp, max_hp: def_hp, damage: def_dmg, hit: 100, crit, strikes: def_strikes, triangle: 0 },
        }
    }

    #[test]
    fn resolve_order_and_doubling() {
        let f = sure_forecast(5, 2, 30, 3, 1, 30, 0);
        let out = resolve(&f, &mut Rng::new(1));
        let order: Vec<Side> = out.strikes.iter().map(|s| s.by).collect();
        assert_eq!(order, vec![Side::Attacker, Side::Defender, Side::Attacker]);
        assert_eq!(out.defender_hp, 20);
        assert_eq!(out.attacker_hp, 27);
    }

    #[test]
    fn resolve_stops_on_kill() {
        let f = sure_forecast(10, 2, 30, 3, 2, 8, 0);
        let out = resolve(&f, &mut Rng::new(1));
        assert_eq!(out.strikes.len(), 1);
        assert_eq!(out.defender_hp, 0);
        assert_eq!(out.strikes[0].damage, 8, "overkill damage is capped at remaining HP");
        assert_eq!(out.attacker_hp, 30);
    }

    #[test]
    fn counter_can_kill_attacker() {
        let f = sure_forecast(1, 2, 4, 5, 1, 30, 0);
        let out = resolve(&f, &mut Rng::new(1));
        assert_eq!(out.attacker_hp, 0);
        assert_eq!(out.strikes.len(), 2, "attacker's follow-up never happens");
    }

    #[test]
    fn crits_triple_damage() {
        let f = sure_forecast(4, 1, 30, 0, 0, 30, 100);
        let out = resolve(&f, &mut Rng::new(3));
        assert!(out.strikes[0].crit);
        assert_eq!(out.defender_hp, 30 - 12);
    }

    #[test]
    fn zero_hit_always_misses() {
        let mut f = sure_forecast(9, 2, 30, 9, 2, 30, 100);
        f.attacker.hit = 0;
        f.defender.hit = 0;
        let out = resolve(&f, &mut Rng::new(11));
        assert_eq!(out.strikes.len(), 4);
        assert!(out.strikes.iter().all(|s| !s.hit && !s.crit && s.damage == 0));
        assert_eq!((out.attacker_hp, out.defender_hp), (30, 30));
    }

    #[test]
    fn xp_formula() {
        assert_eq!(combat_xp(1, 1, false, false, false), 1);
        assert_eq!(combat_xp(1, 1, true, false, false), 10);
        assert_eq!(combat_xp(1, 1, true, true, false), 30);
        assert_eq!(combat_xp(1, 5, true, true, true), 11 + 32 + 40);
        // Much weaker foe still gives something.
        assert!(combat_xp(15, 1, true, true, false) >= 6);
        assert!(combat_xp(1, 30, true, true, true) <= 100);
    }
}
