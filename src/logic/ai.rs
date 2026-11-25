//! Simple enemy AI: attack the best target reachable this turn, otherwise
//! advance toward the nearest player unit. Stationary units (the boss) only
//! attack what is already in range.

use super::battle::Battle;
use super::combat::Forecast;
use super::pathfinding::distance_field;
use super::unit::Team;
use super::Pos;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AiAction {
    Attack { dest: Pos, target: usize },
    Move { dest: Pos },
    Wait,
}

/// Heuristic value of an attack (higher is better).
pub fn score_attack(f: &Forecast, target_is_lord: bool, dest_avoid: i32) -> f32 {
    let a = &f.attacker;
    let d = &f.defender;
    let p = a.hit as f32 / 100.0;
    let expected = (a.damage * a.strikes).min(d.hp) as f32 * p;
    let kill = if a.damage >= d.hp {
        1.0 - (1.0 - p).powi(a.strikes)
    } else if a.strikes == 2 && a.damage * 2 >= d.hp {
        p * p
    } else {
        0.0
    };
    let taken = (d.damage * d.strikes) as f32 * d.hit as f32 / 100.0;
    expected + kill * 30.0 - taken * 0.5 + dest_avoid as f32 / 10.0 + if target_is_lord { 4.0 } else { 0.0 }
}

pub fn decide(b: &Battle, id: usize) -> AiAction {
    let u = &b.units[id];
    let reach = b.reach(id);

    let mut best: Option<(f32, Pos, usize)> = None;
    for &dest in &reach.stoppable {
        for target in b.targets_from(id, dest) {
            let f = b.forecast_from(id, dest, target);
            if f.attacker.damage == 0 {
                continue;
            }
            let score = score_attack(&f, b.units[target].is_lord, b.map.at(dest).avoid())
                - reach.cost[&dest] as f32 * 0.01;
            #[allow(clippy::unnecessary_map_or)]
            if best.as_ref().map_or(true, |b| score > b.0) {
                best = Some((score, dest, target));
            }
        }
    }
    if let Some((_, dest, target)) = best {
        return AiAction::Attack { dest, target };
    }
    if u.mov() == 0 {
        return AiAction::Wait;
    }

    let foes: Vec<Pos> = b.living(Team::Player).map(|i| b.units[i].pos).collect();
    if foes.is_empty() {
        return AiAction::Wait;
    }
    let field = distance_field(&b.map, &foes, u.class.move_type());
    let here = *field.get(&u.pos).unwrap_or(&i32::MAX);
    let dest = reach
        .stoppable
        .iter()
        .copied()
        .min_by_key(|p| (*field.get(p).unwrap_or(&i32::MAX), reach.cost[p], *p));
    match dest {
        Some(d) if d != u.pos && *field.get(&d).unwrap_or(&i32::MAX) < here => AiAction::Move { dest: d },
        _ => AiAction::Wait,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::map::Map;
    use crate::logic::terrain::Terrain;
    use crate::logic::unit::{weapons, Class, Stats, Unit, Weapon};

    fn u(team: Team, class: Class, pos: Pos, hp: i32, w: Weapon) -> Unit {
        Unit::new("U", class, team, 1, Stats::new(hp, 6, 5, 5, 0, 2, 0), Stats::default(), w, pos)
    }

    #[test]
    fn attacks_target_in_reach() {
        let units = vec![
            u(Team::Player, Class::Lancer, Pos::new(0, 0), 20, weapons::iron_lance()),
            u(Team::Enemy, Class::Brigand, Pos::new(4, 0), 20, weapons::iron_axe()),
        ];
        let b = Battle::new(Map::filled(10, 10, Terrain::Plains), units, 1, true);
        match decide(&b, 1) {
            AiAction::Attack { dest, target } => {
                assert_eq!(target, 0);
                assert_eq!(dest.manhattan(Pos::new(0, 0)), 1);
            }
            other => panic!("expected attack, got {other:?}"),
        }
    }

    #[test]
    fn prefers_kill_and_weak_target() {
        let units = vec![
            u(Team::Player, Class::Lancer, Pos::new(0, 0), 30, weapons::iron_lance()),
            u(Team::Player, Class::Cleric, Pos::new(0, 4), 5, weapons::mend()),
            u(Team::Enemy, Class::Brigand, Pos::new(3, 2), 20, weapons::iron_axe()),
        ];
        let b = Battle::new(Map::filled(10, 10, Terrain::Plains), units, 1, true);
        assert!(matches!(decide(&b, 2), AiAction::Attack { target: 1, .. }));
    }

    #[test]
    fn advances_when_nothing_in_reach() {
        let units = vec![
            u(Team::Player, Class::Lord, Pos::new(0, 0), 20, weapons::iron_sword()),
            u(Team::Enemy, Class::Soldier, Pos::new(14, 0), 20, weapons::iron_lance()),
        ];
        let b = Battle::new(Map::filled(16, 3, Terrain::Plains), units, 1, true);
        match decide(&b, 1) {
            AiAction::Move { dest } => assert_eq!(dest, Pos::new(10, 0)),
            other => panic!("expected move, got {other:?}"),
        }
    }

    #[test]
    fn advances_around_water_via_bridge() {
        let map = Map::from_rows(&[
            ".....~......",
            ".....~......",
            ".....=......",
        ]);
        let units = vec![
            u(Team::Player, Class::Lord, Pos::new(0, 0), 20, weapons::iron_sword()),
            u(Team::Enemy, Class::Mercenary, Pos::new(9, 0), 20, weapons::iron_sword()),
        ];
        let b = Battle::new(map.clone(), units, 1, true);
        let AiAction::Move { dest } = decide(&b, 1) else { panic!("should move") };
        let field = distance_field(&map, &[Pos::new(0, 0)], Class::Mercenary.move_type());
        assert!(field[&dest] < field[&Pos::new(9, 0)]);
        assert!(dest.y > 0, "heads toward the bridge row");
    }

    #[test]
    fn stationary_boss_waits_unless_target_in_range() {
        let mut boss = u(Team::Enemy, Class::Warlord, Pos::new(5, 5), 30, weapons::cleaver()).boss();
        boss.stats.pow = 10;
        let units = vec![u(Team::Player, Class::Lord, Pos::new(0, 0), 20, weapons::iron_sword()), boss];
        let mut b = Battle::new(Map::filled(10, 10, Terrain::Plains), units, 1, true);
        assert_eq!(decide(&b, 1), AiAction::Wait);
        b.units[0].pos = Pos::new(5, 6);
        assert_eq!(decide(&b, 1), AiAction::Attack { dest: Pos::new(5, 5), target: 0 });
    }
}
