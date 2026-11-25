use std::collections::HashSet;

use super::combat::{self, CombatOutcome, Combatant, Forecast, Side};
use super::items::{ItemId, ItemKind};
use super::map::Map;
use super::pathfinding::{reachable, tiles_in_range, Occupant, Reach};
use super::rng::Rng;
use super::unit::{AiMode, LevelUp, Promotion, Team, Unit, MAX_LEVEL};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Objective {
    /// Defeat every enemy (including reinforcements still to come).
    Rout,
    /// Defeat the boss.
    Boss,
    /// The lord must stand on this tile and choose Seize.
    Seize(Pos),
    /// Hold out until this many turns have passed.
    Survive(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reward {
    Gold(i32),
    Item(ItemId),
}

#[derive(Clone, Debug)]
pub struct Village {
    pub pos: Pos,
    pub reward: Reward,
    pub visited: bool,
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
    /// (unit, item name) of weapons that broke during the fight.
    pub broken: Vec<(usize, &'static str)>,
}

#[derive(Clone, Debug)]
pub struct HealReport {
    pub target: usize,
    pub amount: i32,
    pub hp_after: i32,
    pub xp: Option<XpEvent>,
    pub broken: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ItemUse {
    Healed { amount: i32, hp_after: i32 },
    Promoted(Promotion),
    Equipped,
    CannotUse,
}

/// Fort/terrain healing applied at the start of a phase.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainHeal {
    pub unit: usize,
    pub amount: i32,
    pub hp_after: i32,
}

#[derive(Clone, Debug, Default)]
pub struct PhaseStart {
    pub heals: Vec<TerrainHeal>,
    /// Indices of reinforcements that just arrived.
    pub spawned: Vec<usize>,
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
    pub objective: Objective,
    /// (turn, unit): enemies that arrive at the start of that turn's enemy phase.
    pub reinforcements: Vec<(u32, Unit)>,
    pub villages: Vec<Village>,
    /// Gold picked up during this battle.
    pub gold: i32,
    pub seized: bool,
}

impl Battle {
    pub fn new(map: Map, units: Vec<Unit>, seed: u64, permadeath: bool) -> Battle {
        Battle {
            map,
            units,
            phase: Phase::Player,
            turn: 1,
            permadeath,
            rng: Rng::new(seed),
            fallen: Vec::new(),
            objective: Objective::Boss,
            reinforcements: Vec::new(),
            villages: Vec::new(),
            gold: 0,
            seized: false,
        }
    }

    pub fn unit_at(&self, p: Pos) -> Option<usize> {
        self.units.iter().position(|u| u.alive && u.pos == p)
    }

    pub fn living(&self, team: Team) -> impl Iterator<Item = usize> + '_ {
        self.units.iter().enumerate().filter(move |(_, u)| u.alive && u.team == team).map(|(i, _)| i)
    }

    pub fn village_at(&self, p: Pos) -> Option<&Village> {
        self.villages.iter().find(|v| v.pos == p)
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

    /// Every tile the unit could strike (or heal, for staff-only units) after moving.
    pub fn threat_tiles(&self, id: usize, reach: &Reach) -> HashSet<Pos> {
        let u = &self.units[id];
        let range = u.attack_range().or_else(|| u.staff().map(|s| (s.min_range, s.max_range)));
        let mut set = HashSet::new();
        if let Some((min, max)) = range {
            for &from in &reach.stoppable {
                set.extend(tiles_in_range(&self.map, from, min, max));
            }
        }
        set
    }

    /// Living foes the unit could attack (with any of its weapons) from `from`.
    pub fn targets_from(&self, id: usize, from: Pos) -> Vec<usize> {
        let u = &self.units[id];
        let ws = u.attack_weapons();
        self.units
            .iter()
            .enumerate()
            .filter(|(_, t)| t.alive && u.team.is_foe(t.team) && ws.iter().any(|(_, w)| w.in_range(from.manhattan(t.pos))))
            .map(|(i, _)| i)
            .collect()
    }

    /// Wounded allies a staff user could heal from `from`.
    pub fn heal_targets_from(&self, id: usize, from: Pos) -> Vec<usize> {
        let u = &self.units[id];
        let Some(staff) = u.staff() else { return Vec::new() };
        self.units
            .iter()
            .enumerate()
            .filter(|(i, t)| *i != id && t.alive && t.team == u.team && t.hp < t.stats.hp && staff.in_range(from.manhattan(t.pos)))
            .map(|(i, _)| i)
            .collect()
    }

    /// Recruitable enemies adjacent to the lord.
    pub fn talk_targets_from(&self, id: usize, from: Pos) -> Vec<usize> {
        let u = &self.units[id];
        if !u.is_lord {
            return Vec::new();
        }
        self.units
            .iter()
            .enumerate()
            .filter(|(_, t)| t.alive && t.talk && t.team.is_foe(u.team) && from.manhattan(t.pos) == 1)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn can_visit(&self, id: usize) -> bool {
        let u = &self.units[id];
        u.team == Team::Player && self.village_at(u.pos).is_some_and(|v| !v.visited)
    }

    pub fn can_seize(&self, id: usize) -> bool {
        let u = &self.units[id];
        matches!(self.objective, Objective::Seize(p) if u.is_lord && u.pos == p)
    }

    /// Forecast for `att` attacking `def` while standing on `from`, using equipped weapons.
    pub fn forecast_from(&self, att: usize, from: Pos, def: usize) -> Forecast {
        let a = &self.units[att];
        let d = &self.units[def];
        let ca = Combatant::new(a, self.map.at(from));
        let cd = Combatant::new(d, self.map.at(d.pos));
        combat::forecast(&ca, &cd, from.manhattan(d.pos))
    }

    /// Forecast as if the attacker equipped the first weapon that reaches the target.
    pub fn forecast_best(&self, att: usize, from: Pos, def: usize) -> Forecast {
        let d = &self.units[def];
        let dist = from.manhattan(d.pos);
        let mut a = self.units[att].clone();
        a.equip_for_range(dist);
        let ca = Combatant::new(&a, self.map.at(from));
        let cd = Combatant::new(d, self.map.at(d.pos));
        combat::forecast(&ca, &cd, dist)
    }

    pub fn forecast(&self, att: usize, def: usize) -> Forecast {
        self.forecast_from(att, self.units[att].pos, def)
    }

    pub fn move_unit(&mut self, id: usize, dest: Pos) {
        self.units[id].pos = dest;
    }

    fn award_xp(&mut self, unit: usize, amount: i32) -> Option<XpEvent> {
        let u = &self.units[unit];
        if u.team != Team::Player || !u.alive || u.level >= MAX_LEVEL {
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

    /// Resolves a full exchange and applies HP, weapon wear, deaths and experience.
    /// The attacker should already have the weapon it wants equipped.
    pub fn attack(&mut self, att: usize, def: usize) -> CombatReport {
        let dist = self.units[att].pos.manhattan(self.units[def].pos);
        self.units[att].equip_for_range(dist);
        let forecast = self.forecast(att, def);
        let outcome = combat::resolve(&forecast, &mut self.rng);
        self.units[att].hp = outcome.attacker_hp;
        self.units[def].hp = outcome.defender_hp;

        let mut broken = Vec::new();
        for (id, side) in [(att, Side::Attacker), (def, Side::Defender)] {
            let n = outcome.strikes_by(side);
            for _ in 0..n {
                if let Some(idx) = self.units[id].weapon_index() {
                    if let Some(name) = self.units[id].spend(idx) {
                        broken.push((id, name));
                        break;
                    }
                }
            }
        }

        let mut deaths = Vec::new();
        if outcome.defender_hp == 0 {
            self.kill(def);
            deaths.push(def);
        }
        if outcome.attacker_hp == 0 {
            self.kill(att);
            deaths.push(att);
        }

        let (player, foe, side) = if self.units[att].team == Team::Player { (att, def, Side::Attacker) } else { (def, att, Side::Defender) };
        let xp = if self.units[player].alive && self.units[player].team == Team::Player {
            let p = &self.units[player];
            let f = &self.units[foe];
            let amount = combat::combat_xp(p.effective_level(), f.effective_level(), outcome.landed_damage(side), !f.alive, f.is_boss);
            self.award_xp(player, amount)
        } else {
            None
        };

        CombatReport { attacker: att, defender: def, outcome, xp, deaths, broken }
    }

    pub fn heal(&mut self, healer: usize, target: usize) -> HealReport {
        let Some(idx) = self.units[healer].staff_index() else {
            return HealReport { target, amount: 0, hp_after: self.units[target].hp, xp: None, broken: None };
        };
        let staff = self.units[healer].items[idx].id.weapon().unwrap();
        let amount = combat::heal_amount(&self.units[healer], &staff);
        let t = &mut self.units[target];
        let healed = amount.min(t.stats.hp - t.hp);
        t.hp += healed;
        let hp_after = t.hp;
        let broken = self.units[healer].spend(idx);
        let xp = self.award_xp(healer, combat::HEAL_XP);
        HealReport { target, amount: healed, hp_after, xp, broken }
    }

    /// Uses or equips inventory item `idx`.
    pub fn use_item(&mut self, id: usize, idx: usize) -> ItemUse {
        let u = &mut self.units[id];
        let Some(item) = u.items.get(idx).copied() else { return ItemUse::CannotUse };
        match item.id.info().kind {
            ItemKind::Weapon(w) => {
                if u.class.can_wield(w.kind) {
                    u.equip(idx);
                    ItemUse::Equipped
                } else {
                    ItemUse::CannotUse
                }
            }
            ItemKind::Heal(n) => {
                if u.hp >= u.stats.hp {
                    return ItemUse::CannotUse;
                }
                let amount = n.min(u.stats.hp - u.hp);
                u.hp += amount;
                let hp_after = u.hp;
                u.spend(idx);
                ItemUse::Healed { amount, hp_after }
            }
            ItemKind::Seal => {
                if !u.can_promote() {
                    return ItemUse::CannotUse;
                }
                let p = u.promote().unwrap();
                u.spend(idx);
                ItemUse::Promoted(p)
            }
        }
    }

    /// Visits the village under the unit. Items go to the unit (or are sold if its bag is full).
    pub fn visit(&mut self, id: usize) -> Option<Reward> {
        let pos = self.units[id].pos;
        let v = self.villages.iter_mut().find(|v| v.pos == pos && !v.visited)?;
        v.visited = true;
        let reward = v.reward;
        match reward {
            Reward::Gold(g) => self.gold += g,
            Reward::Item(item) => {
                if !self.units[id].give(item) {
                    self.gold += item.info().price / 2;
                }
            }
        }
        Some(reward)
    }

    /// The lord persuades an enemy to join.
    pub fn recruit(&mut self, target: usize) {
        let u = &mut self.units[target];
        u.team = Team::Player;
        u.talk = false;
        u.ai = AiMode::Aggressive;
        u.done = true;
    }

    pub fn seize(&mut self, id: usize) {
        if self.can_seize(id) {
            self.seized = true;
        }
    }

    pub fn finish_action(&mut self, id: usize) {
        self.units[id].done = true;
    }

    pub fn player_has_actions(&self) -> bool {
        self.units.iter().any(|u| u.alive && u.team == Team::Player && !u.done)
    }

    /// Switches to `phase`: resets action flags, spawns reinforcements (enemy
    /// phase) and applies fort healing to that side. Advances the turn counter
    /// when a new player phase begins.
    pub fn begin_phase(&mut self, phase: Phase) -> PhaseStart {
        if phase == Phase::Player && self.phase == Phase::Enemy {
            self.turn += 1;
        }
        self.phase = phase;
        let mut start = PhaseStart::default();
        if phase == Phase::Enemy {
            let turn = self.turn;
            let (now, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.reinforcements).into_iter().partition(|(t, _)| *t <= turn);
            self.reinforcements = later;
            for (t, u) in now {
                if self.unit_at(u.pos).is_some() {
                    // Tile occupied: try again next turn.
                    self.reinforcements.push((t + 1, u));
                    continue;
                }
                self.units.push(u);
                start.spawned.push(self.units.len() - 1);
            }
        }
        let team = match phase {
            Phase::Player => Team::Player,
            Phase::Enemy => Team::Enemy,
        };
        for (i, u) in self.units.iter_mut().enumerate() {
            u.done = false;
            if !u.alive || u.team != team {
                continue;
            }
            let pct = self.map.at(u.pos).heal_percent();
            if pct > 0 && u.hp < u.stats.hp {
                let amount = ((u.stats.hp * pct + 99) / 100).min(u.stats.hp - u.hp);
                u.hp += amount;
                start.heals.push(TerrainHeal { unit: i, amount, hp_after: u.hp });
            }
        }
        start
    }

    pub fn enemies_left(&self) -> usize {
        self.living(Team::Enemy).count() + self.reinforcements.len()
    }

    pub fn boss_name(&self) -> Option<&str> {
        self.units.iter().find(|u| u.is_boss).map(|u| u.name.as_str())
    }

    pub fn outcome(&self) -> Outcome {
        if self.units.iter().any(|u| u.is_lord && !u.alive) {
            return Outcome::Defeat;
        }
        let routed = self.enemies_left() == 0;
        let won = match self.objective {
            Objective::Rout => routed,
            Objective::Boss => routed || self.units.iter().any(|u| u.is_boss && !u.alive),
            Objective::Seize(_) => self.seized,
            Objective::Survive(n) => routed || (self.phase == Phase::Player && self.turn > n),
        };
        if won {
            Outcome::Victory
        } else {
            Outcome::Ongoing
        }
    }

    pub fn objective_text(&self) -> String {
        match self.objective {
            Objective::Rout => format!("Rout the enemy ({} left)", self.enemies_left()),
            Objective::Boss => format!("Defeat {}", self.boss_name().unwrap_or("the commander")),
            Objective::Seize(_) => "Seize the throne".to_string(),
            Objective::Survive(n) => format!("Survive {} turns ({} left)", n, (n + 1).saturating_sub(self.turn)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::campaign;
    use crate::logic::terrain::Terrain;
    use crate::logic::unit::{Class, Stats};

    fn plains(w: i32, h: i32) -> Map {
        Map::filled(w, h, Terrain::Plains)
    }

    fn unit(class: Class, team: Team, pos: Pos, hp: i32, pow: i32, spd: i32, w: ItemId) -> Unit {
        Unit::new("U", class, team, 1, Stats::new(hp, pow, 30, spd, 0, 0, 0), Stats::default(), &[w], pos)
    }

    fn first_battle() -> Battle {
        let mut c = campaign::Campaign::new(true);
        c.begin_chapter();
        c.battle(1)
    }

    #[test]
    fn lord_death_is_defeat() {
        let mut b = first_battle();
        assert_eq!(b.outcome(), Outcome::Ongoing);
        let lord = b.units.iter().position(|u| u.is_lord).unwrap();
        b.kill(lord);
        assert_eq!(b.outcome(), Outcome::Defeat);
    }

    #[test]
    fn boss_death_is_victory() {
        let mut b = first_battle();
        let boss = b.units.iter().position(|u| u.is_boss).unwrap();
        b.kill(boss);
        assert_eq!(b.outcome(), Outcome::Victory);
    }

    #[test]
    fn rout_waits_for_reinforcements() {
        let a = unit(Class::Lord, Team::Player, Pos::new(0, 0), 20, 5, 5, ItemId::IronSword).lord();
        let e = unit(Class::Brigand, Team::Enemy, Pos::new(5, 5), 20, 5, 5, ItemId::IronAxe);
        let mut b = Battle::new(plains(8, 8), vec![a, e.clone()], 1, true);
        b.objective = Objective::Rout;
        b.reinforcements.push((2, e.clone()));
        b.kill(1);
        assert_eq!(b.outcome(), Outcome::Ongoing, "one reinforcement still to come");
        b.begin_phase(Phase::Enemy); // turn 1: not yet
        assert_eq!(b.units.len(), 2);
        b.begin_phase(Phase::Player);
        let start = b.begin_phase(Phase::Enemy); // turn 2
        assert_eq!(start.spawned, vec![2]);
        assert_eq!(b.outcome(), Outcome::Ongoing);
        b.kill(2);
        assert_eq!(b.outcome(), Outcome::Victory);
    }

    #[test]
    fn blocked_reinforcement_is_delayed() {
        let a = unit(Class::Lord, Team::Player, Pos::new(5, 5), 20, 5, 5, ItemId::IronSword).lord();
        let e = unit(Class::Brigand, Team::Enemy, Pos::new(5, 5), 20, 5, 5, ItemId::IronAxe);
        let mut b = Battle::new(plains(8, 8), vec![a], 1, true);
        b.reinforcements.push((1, e));
        assert!(b.begin_phase(Phase::Enemy).spawned.is_empty());
        assert_eq!(b.reinforcements[0].0, 2);
    }

    #[test]
    fn survive_objective() {
        let a = unit(Class::Lord, Team::Player, Pos::new(0, 0), 20, 5, 5, ItemId::IronSword).lord();
        let e = unit(Class::Brigand, Team::Enemy, Pos::new(5, 5), 20, 5, 5, ItemId::IronAxe);
        let mut b = Battle::new(plains(8, 8), vec![a, e], 1, true);
        b.objective = Objective::Survive(2);
        b.begin_phase(Phase::Enemy);
        b.begin_phase(Phase::Player); // turn 2
        assert_eq!(b.outcome(), Outcome::Ongoing);
        b.begin_phase(Phase::Enemy);
        b.begin_phase(Phase::Player); // turn 3
        assert_eq!(b.outcome(), Outcome::Victory);
    }

    #[test]
    fn seize_needs_lord_on_tile() {
        let lord = unit(Class::Lord, Team::Player, Pos::new(0, 0), 20, 5, 5, ItemId::IronSword).lord();
        let other = unit(Class::Lancer, Team::Player, Pos::new(3, 3), 20, 5, 5, ItemId::IronLance);
        let e = unit(Class::Brigand, Team::Enemy, Pos::new(7, 7), 20, 5, 5, ItemId::IronAxe);
        let mut b = Battle::new(plains(8, 8), vec![lord, other, e], 1, true);
        b.objective = Objective::Seize(Pos::new(3, 3));
        assert!(!b.can_seize(1), "only the lord can seize");
        b.units[1].pos = Pos::new(4, 4);
        b.move_unit(0, Pos::new(3, 3));
        assert!(b.can_seize(0));
        b.seize(0);
        assert_eq!(b.outcome(), Outcome::Victory);
    }

    #[test]
    fn attack_applies_damage_death_xp_and_wear() {
        let a = unit(Class::Lord, Team::Player, Pos::new(0, 0), 20, 30, 10, ItemId::IronSword);
        let d = unit(Class::Brigand, Team::Enemy, Pos::new(1, 0), 10, 1, 0, ItemId::IronAxe);
        let mut b = Battle::new(plains(6, 6), vec![a, d], 1, true);
        let r = b.attack(0, 1);
        assert_eq!(r.deaths, vec![1]);
        assert!(!b.units[1].alive);
        assert_eq!(b.unit_at(Pos::new(1, 0)), None);
        assert_eq!(r.xp.expect("player gets xp").gained, 30);
        assert_eq!(b.units[0].items[0].uses, 45, "one strike, one use");
    }

    #[test]
    fn weapons_break_mid_fight() {
        let a = unit(Class::Lord, Team::Player, Pos::new(0, 0), 40, 1, 10, ItemId::IronSword);
        let d = unit(Class::Brigand, Team::Enemy, Pos::new(1, 0), 40, 1, 0, ItemId::IronAxe);
        let mut b = Battle::new(plains(6, 6), vec![a, d], 1, true);
        b.units[0].items[0].uses = 1;
        let r = b.attack(0, 1);
        assert_eq!(r.broken, vec![(0, "Iron Sword")]);
        assert!(b.units[0].items.is_empty());
    }

    #[test]
    fn attack_auto_equips_weapon_for_range() {
        let mut a = unit(Class::Lancer, Team::Player, Pos::new(0, 0), 20, 5, 5, ItemId::IronLance);
        a.give(ItemId::Javelin);
        let d = unit(Class::Archer, Team::Enemy, Pos::new(2, 0), 20, 5, 5, ItemId::ShortBow);
        let mut b = Battle::new(plains(6, 6), vec![a, d], 1, true);
        assert_eq!(b.targets_from(0, Pos::new(0, 0)), vec![1]);
        b.attack(0, 1);
        assert_eq!(b.units[0].items[0].id, ItemId::Javelin);
    }

    #[test]
    fn defending_player_also_gains_xp() {
        let a = unit(Class::Brigand, Team::Enemy, Pos::new(1, 0), 30, 1, 0, ItemId::IronAxe);
        let d = unit(Class::Lord, Team::Player, Pos::new(0, 0), 30, 10, 0, ItemId::IronSword);
        let mut b = Battle::new(plains(6, 6), vec![a, d], 1, true);
        assert_eq!(b.attack(0, 1).xp.map(|x| x.unit), Some(1));
    }

    #[test]
    fn allies_and_foes_in_movement() {
        let mut b = first_battle();
        let reach = b.reach(0);
        assert!(!reach.can_stop(b.units[1].pos));
        assert!(reach.can_stop(b.units[0].pos));
        let foe = b.living(Team::Enemy).next().unwrap();
        let adj = Pos::new(b.units[0].pos.x, b.units[0].pos.y - 1);
        b.units[foe].pos = adj;
        assert_eq!(b.targets_from(0, b.units[0].pos), vec![foe]);
        assert!(!b.reach(0).cost.contains_key(&adj));
    }

    #[test]
    fn healing_uses_staff() {
        let healer = Unit::generic("L", Class::Cleric, Team::Player, 1, &[ItemId::Mend], Pos::new(0, 0));
        let mut hurt = Unit::generic("O", Class::Mage, Team::Player, 1, &[ItemId::Ember], Pos::new(1, 0));
        hurt.hp -= 10;
        let mut b = Battle::new(plains(4, 4), vec![healer, hurt], 1, true);
        assert_eq!(b.heal_targets_from(0, Pos::new(0, 0)), vec![1]);
        assert!(b.targets_from(0, Pos::new(0, 0)).is_empty(), "staff can't attack");
        let r = b.heal(0, 1);
        assert_eq!(r.amount, 10, "heal capped at missing HP");
        assert_eq!(b.units[1].hp, b.units[1].stats.hp);
        assert!(r.xp.is_some());
        assert_eq!(b.units[0].items[0].uses, 29);
    }

    #[test]
    fn items_heal_promote_and_equip() {
        let mut u = Unit::generic("K", Class::Lord, Team::Player, 10, &[ItemId::IronSword, ItemId::Vulnerary, ItemId::MasterSeal, ItemId::SteelSword], Pos::new(0, 0));
        u.hp -= 5;
        let mut b = Battle::new(plains(4, 4), vec![u], 1, true);
        assert!(matches!(b.use_item(0, 1), ItemUse::Healed { amount: 5, .. }));
        assert_eq!(b.units[0].items[1].uses, 2);
        assert_eq!(b.use_item(0, 1), ItemUse::CannotUse, "already at full HP");
        assert!(matches!(b.use_item(0, 2), ItemUse::Promoted(_)));
        assert_eq!(b.units[0].class, Class::Highlord);
        assert_eq!(b.units[0].items.len(), 3, "seal consumed");
        assert_eq!(b.use_item(0, 2), ItemUse::Equipped);
        assert_eq!(b.units[0].weapon().unwrap().name, "Steel Sword");
    }

    #[test]
    fn villages_reward_once() {
        let u = Unit::generic("K", Class::Lord, Team::Player, 1, &[ItemId::IronSword], Pos::new(1, 1));
        let mut b = Battle::new(plains(4, 4), vec![u], 1, true);
        b.villages.push(Village { pos: Pos::new(1, 1), reward: Reward::Item(ItemId::Elixir), visited: false });
        b.villages.push(Village { pos: Pos::new(2, 2), reward: Reward::Gold(500), visited: false });
        assert!(b.can_visit(0));
        assert_eq!(b.visit(0), Some(Reward::Item(ItemId::Elixir)));
        assert!(!b.can_visit(0));
        assert_eq!(b.units[0].items.len(), 2);
        b.move_unit(0, Pos::new(2, 2));
        b.visit(0);
        assert_eq!(b.gold, 500);
    }

    #[test]
    fn lord_recruits_adjacent_talker() {
        let lord = Unit::generic("K", Class::Lord, Team::Player, 1, &[ItemId::IronSword], Pos::new(0, 0)).lord();
        let other = Unit::generic("B", Class::Lancer, Team::Player, 1, &[ItemId::IronLance], Pos::new(2, 1));
        let merc = Unit::generic("D", Class::Mercenary, Team::Enemy, 3, &[ItemId::IronSword], Pos::new(1, 0)).talkable();
        let mut b = Battle::new(plains(4, 4), vec![lord, other, merc], 1, true);
        assert_eq!(b.talk_targets_from(0, Pos::new(0, 0)), vec![2]);
        assert!(b.talk_targets_from(1, Pos::new(1, 1)).is_empty(), "only the lord talks");
        b.recruit(2);
        assert_eq!(b.units[2].team, Team::Player);
        assert!(b.talk_targets_from(0, Pos::new(0, 0)).is_empty());
    }

    #[test]
    fn forts_heal_at_phase_start() {
        let mut map = plains(4, 4);
        map = {
            let mut rows: Vec<String> = (0..4).map(|_| "....".to_string()).collect();
            rows[1] = ".T..".into();
            let r: Vec<&str> = rows.iter().map(|s| s.as_str()).collect();
            let _ = map;
            Map::from_rows(&r)
        };
        let mut u = Unit::generic("K", Class::Lord, Team::Player, 1, &[ItemId::IronSword], Pos::new(1, 1));
        u.hp = 5;
        u.done = true;
        let mut b = Battle::new(map, vec![u], 1, true);
        assert!(b.begin_phase(Phase::Enemy).heals.is_empty(), "only the phase owner heals");
        let s = b.begin_phase(Phase::Player);
        assert_eq!(s.heals, vec![TerrainHeal { unit: 0, amount: 4, hp_after: 9 }]);
        assert!(!b.units[0].done);
        assert_eq!(b.turn, 2);
    }
}
