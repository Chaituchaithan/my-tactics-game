use std::collections::HashSet;

use super::combat::{self, CombatOutcome, Forecast, Side};
use super::map::Map;
use super::pathfinding::{reachable, tiles_in_range, Occupant, Reach};
use super::rng::Rng;
use super::scenario;
use super::unit::{LevelUp, Team, Unit};
use super::Pos;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Player,
    Enemy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Ongoing,
    Victory,
    Defeat,
}

#[derive(Clone, Debug)]
pub struct XpEvent {
    pub unit: usize,
    pub before: i32,
    pub gained: i32,
    pub level_up: Option<LevelUp>,
}

#[derive(Clone, Debug)]
pub struct CombatReport {
    pub attacker: usize,
    pub defender: usize,
    pub outcome: CombatOutcome,
    pub xp: Option<XpEvent>,
    pub deaths: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct HealReport {
    pub target: usize,
    pub amount: i32,
    pub hp_after: i32,
    pub xp: Option<XpEvent>,
}

/// Fort/terrain healing applied at the start of a phase.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainHeal {
    pub unit: usize,
    pub amount: i32,
    pub hp_after: i32,
}

#[derive(Clone, Debug)]
pub struct Battle {
    pub map: Map,
    pub units: Vec<Unit>,
    pub phase: Phase,
    pub turn: u32,
    pub permadeath: bool,
    pub rng: Rng,
    /// Player units that fell, in order.
    pub fallen: Vec<usize>,
}

impl Battle {
    pub fn new(map: Map, units: Vec<Unit>, seed: u64, permadeath: bool) -> Battle {
        Battle { map, units, phase: Phase::Player, turn: 1, permadeath, rng: Rng::new(seed), fallen: Vec::new() }
    }

    pub fn first_battle(seed: u64, permadeath: bool) -> Battle {
        Battle::new(scenario::map(), scenario::units(), seed, permadeath)
    }

    pub fn unit_at(&self, p: Pos) -> Option<usize> {
        self.units.iter().position(|u| u.alive && u.pos == p)
    }

    pub fn living(&self, team: Team) -> impl Iterator<Item = usize> + '_ {
        self.units.iter().enumerate().filter(move |(_, u)| u.alive && u.team == team).map(|(i, _)| i)
    }

    pub fn occupant_for(&self, id: usize, p: Pos) -> Occupant {
        match self.unit_at(p) {
            None => Occupant::Empty,
            Some(o) if o == id => Occupant::Empty,
            Some(o) if self.units[o].team == self.units[id].team => Occupant::Ally,
            Some(_) => Occupant::Foe,
        }
    }

    pub fn reach(&self, id: usize) -> Reach {
        let u = &self.units[id];
        reachable(&self.map, u.pos, u.mov(), u.class.move_type(), |p| self.occupant_for(id, p))
    }

    /// Every tile the unit could strike (or heal, for staves) after moving.
    pub fn threat_tiles(&self, id: usize, reach: &Reach) -> HashSet<Pos> {
        let w = &self.units[id].weapon;
        let mut set = HashSet::new();
        for &from in &reach.stoppable {
            for t in tiles_in_range(&self.map, from, w.min_range, w.max_range) {
                set.insert(t);
            }
        }
        set
    }

    /// Living foes the unit could attack if standing on `from`.
    pub fn targets_from(&self, id: usize, from: Pos) -> Vec<usize> {
        let u = &self.units[id];
        if !u.weapon.can_attack() {
            return Vec::new();
        }
        self.units
            .iter()
            .enumerate()
            .filter(|(_, t)| t.alive && u.team.is_foe(t.team) && u.weapon.in_range(from.manhattan(t.pos)))
            .map(|(i, _)| i)
            .collect()
    }

    /// Wounded allies a staff user could heal from `from`.
    pub fn heal_targets_from(&self, id: usize, from: Pos) -> Vec<usize> {
        let u = &self.units[id];
        if u.weapon.can_attack() {
            return Vec::new();
        }
        self.units
            .iter()
            .enumerate()
            .filter(|(i, t)| {
                *i != id && t.alive && t.team == u.team && t.hp < t.stats.hp && u.weapon.in_range(from.manhattan(t.pos))
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Forecast for `att` attacking `def` while standing on `from`.
    pub fn forecast_from(&self, att: usize, from: Pos, def: usize) -> Forecast {
        let a = &self.units[att];
        let d = &self.units[def];
        combat::forecast(a, self.map.at(from), d, self.map.at(d.pos), from.manhattan(d.pos))
    }

    pub fn forecast(&self, att: usize, def: usize) -> Forecast {
        self.forecast_from(att, self.units[att].pos, def)
    }

    pub fn move_unit(&mut self, id: usize, dest: Pos) {
        self.units[id].pos = dest;
    }

    fn award_xp(&mut self, unit: usize, amount: i32) -> Option<XpEvent> {
        let u = &self.units[unit];
        if u.team != Team::Player || !u.alive || u.level >= super::unit::MAX_LEVEL {
            return None;
        }
        let before = u.xp;
        let level_up = self.units[unit].gain_xp(amount, &mut self.rng);
        Some(XpEvent { unit, before, gained: amount, level_up })
    }

    fn kill(&mut self, id: usize) {
        self.units[id].alive = false;
        self.units[id].hp = 0;
        if self.units[id].team == Team::Player {
            self.fallen.push(id);
        }
    }

    /// Resolves a full exchange and applies HP, deaths and experience.
    pub fn attack(&mut self, att: usize, def: usize) -> CombatReport {
        let forecast = self.forecast(att, def);
        let outcome = combat::resolve(&forecast, &mut self.rng);
        self.units[att].hp = outcome.attacker_hp;
        self.units[def].hp = outcome.defender_hp;

        let mut deaths = Vec::new();
        if outcome.defender_hp == 0 {
            self.kill(def);
            deaths.push(def);
        }
        if outcome.attacker_hp == 0 {
            self.kill(att);
            deaths.push(att);
        }

        // Only one side of any fight is a player unit.
        let (player, foe, side) = if self.units[att].team == Team::Player {
            (att, def, Side::Attacker)
        } else {
            (def, att, Side::Defender)
        };
        let xp = if self.units[player].alive && self.units[player].team == Team::Player {
            let p = &self.units[player];
            let f = &self.units[foe];
            let amount = combat::combat_xp(p.level, f.level, outcome.landed_damage(side), !f.alive, f.is_boss);
            self.award_xp(player, amount)
        } else {
            None
        };

        CombatReport { attacker: att, defender: def, outcome, xp, deaths }
    }

    pub fn heal(&mut self, healer: usize, target: usize) -> HealReport {
        let amount = combat::heal_amount(&self.units[healer]);
        let t = &mut self.units[target];
        let healed = amount.min(t.stats.hp - t.hp);
        t.hp += healed;
        let hp_after = t.hp;
        let xp = self.award_xp(healer, combat::HEAL_XP);
        HealReport { target, amount: healed, hp_after, xp }
    }

    pub fn finish_action(&mut self, id: usize) {
        self.units[id].done = true;
    }

    pub fn player_has_actions(&self) -> bool {
        self.units.iter().any(|u| u.alive && u.team == Team::Player && !u.done)
    }

    /// Switches to `phase`: resets action flags and applies fort healing to
    /// that side. Advances the turn counter when a new player phase begins.
    pub fn begin_phase(&mut self, phase: Phase) -> Vec<TerrainHeal> {
        if phase == Phase::Player && self.phase == Phase::Enemy {
            self.turn += 1;
        }
        self.phase = phase;
        let team = match phase {
            Phase::Player => Team::Player,
            Phase::Enemy => Team::Enemy,
        };
        let mut heals = Vec::new();
        for (i, u) in self.units.iter_mut().enumerate() {
            u.done = false;
            if !u.alive || u.team != team {
                continue;
            }
            let pct = self.map.at(u.pos).heal_percent();
            if pct > 0 && u.hp < u.stats.hp {
                let amount = ((u.stats.hp * pct + 99) / 100).min(u.stats.hp - u.hp);
                u.hp += amount;
                heals.push(TerrainHeal { unit: i, amount, hp_after: u.hp });
            }
        }
        heals
    }

    pub fn outcome(&self) -> Outcome {
        if self.units.iter().any(|u| u.is_lord && !u.alive) {
            return Outcome::Defeat;
        }
        let boss_dead = self.units.iter().any(|u| u.is_boss && !u.alive);
        if boss_dead || self.living(Team::Enemy).next().is_none() {
            return Outcome::Victory;
        }
        Outcome::Ongoing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::terrain::Terrain;
    use crate::logic::unit::{weapons, Class, Stats};

    fn duel(att: Unit, def: Unit) -> Battle {
        Battle::new(Map::filled(6, 6, Terrain::Plains), vec![att, def], 1, true)
    }

    fn unit(class: Class, team: Team, pos: Pos, hp: i32, pow: i32, spd: i32, w: crate::logic::unit::Weapon) -> Unit {
        Unit::new("U", class, team, 1, Stats::new(hp, pow, 30, spd, 0, 0, 0), Stats::default(), w, pos)
    }

    #[test]
    fn lord_death_is_defeat() {
        let mut b = Battle::first_battle(1, true);
        assert_eq!(b.outcome(), Outcome::Ongoing);
        let lord = b.units.iter().position(|u| u.is_lord).unwrap();
        b.kill(lord);
        assert_eq!(b.outcome(), Outcome::Defeat);
    }

    #[test]
    fn boss_death_is_victory() {
        let mut b = Battle::first_battle(1, true);
        let boss = b.units.iter().position(|u| u.is_boss).unwrap();
        b.kill(boss);
        assert_eq!(b.outcome(), Outcome::Victory);
    }

    #[test]
    fn attack_applies_damage_death_and_xp() {
        // Overwhelming attacker kills in one blow.
        let a = unit(Class::Lord, Team::Player, Pos::new(0, 0), 20, 30, 10, weapons::iron_sword());
        let d = unit(Class::Brigand, Team::Enemy, Pos::new(1, 0), 10, 1, 0, weapons::iron_axe());
        let mut b = duel(a, d);
        let r = b.attack(0, 1);
        assert_eq!(r.deaths, vec![1]);
        assert!(!b.units[1].alive);
        assert_eq!(b.unit_at(Pos::new(1, 0)), None);
        let xp = r.xp.expect("player gets xp");
        assert_eq!(xp.unit, 0);
        assert_eq!(xp.gained, 30);
        assert_eq!(b.units[0].xp, 30);
    }

    #[test]
    fn defending_player_also_gains_xp() {
        let a = unit(Class::Brigand, Team::Enemy, Pos::new(1, 0), 30, 1, 0, weapons::iron_axe());
        let d = unit(Class::Lord, Team::Player, Pos::new(0, 0), 30, 10, 0, weapons::iron_sword());
        let mut b = duel(a, d);
        let r = b.attack(0, 1);
        assert_eq!(r.xp.as_ref().map(|x| x.unit), Some(1));
    }

    #[test]
    fn allies_and_foes_in_movement() {
        let mut b = Battle::first_battle(1, true);
        let kael = 0;
        let reach = b.reach(kael);
        // Can't stop on Brenna's tile but can stay put.
        assert!(!reach.can_stop(b.units[1].pos));
        assert!(reach.can_stop(b.units[0].pos));
        // Place a foe adjacent and check it is targetable.
        b.units[5].pos = Pos::new(1, 9);
        assert_eq!(b.targets_from(kael, b.units[kael].pos), vec![5]);
        assert!(!b.reach(kael).cost.contains_key(&Pos::new(1, 9)));
    }

    #[test]
    fn healer_targets_only_wounded_allies() {
        let mut b = Battle::first_battle(1, true);
        let liese = 4;
        assert!(b.heal_targets_from(liese, b.units[liese].pos).is_empty());
        b.units[3].hp -= 10; // Orrin, adjacent at (1,11)
        assert_eq!(b.heal_targets_from(liese, b.units[liese].pos), vec![3]);
        let r = b.heal(liese, 3);
        assert_eq!(r.amount, 10, "heal capped at missing HP");
        assert_eq!(b.units[3].hp, b.units[3].stats.hp);
        assert!(r.xp.is_some());
        assert!(b.targets_from(liese, b.units[liese].pos).is_empty(), "staff can't attack");
    }

    #[test]
    fn forts_heal_at_phase_start() {
        let mut b = Battle::first_battle(1, true);
        b.units[0].pos = Pos::new(3, 10); // fort
        b.units[0].hp = 5;
        b.units[0].done = true;
        let heals = b.begin_phase(Phase::Enemy);
        assert!(heals.is_empty(), "only the phase owner heals");
        let heals = b.begin_phase(Phase::Player);
        assert_eq!(heals, vec![TerrainHeal { unit: 0, amount: 4, hp_after: 9 }]);
        assert!(!b.units[0].done);
        assert_eq!(b.turn, 2);
    }
}
