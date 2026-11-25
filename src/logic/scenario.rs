//! The first battle: "The Ashford Crossing".

use super::map::Map;
use super::unit::{weapons, Class, Stats, Team, Unit};
use super::Pos;

pub const BATTLE_NAME: &str = "The Ashford Crossing";

/// 16x12. Legend: . plains, F forest, M mountain, T fort, ~ water, = bridge.
pub const MAP_ROWS: [&str; 12] = [
    "MMM..FF..~~..MMM",
    "MM....F..~~...TM",
    "M..F.....==.F..M",
    "..FF..M..~~..F..",
    "......MM.~~.....",
    "..T......~~..FF.",
    ".F...F...==...F.",
    ".....FF..~~.M...",
    "..M......~~.MM..",
    "..MM..F..~~.....",
    "...T....F==..F..",
    "........FF~~~...",
];

pub fn map() -> Map {
    Map::from_rows(&MAP_ROWS)
}

pub fn units() -> Vec<Unit> {
    use Class::*;
    use Team::*;
    let p = Pos::new;
    let none = Stats::default();
    let lvl = |mut u: Unit, level: i32| {
        u.level = level;
        u
    };
    vec![
        // ---- Player ----
        Unit::new("Kael", Lord, Player, 1, Stats::new(20, 6, 7, 8, 6, 5, 2), Stats::new(80, 50, 55, 60, 50, 30, 25), weapons::oathblade(), p(1, 10)).lord(),
        lvl(Unit::new("Brenna", Lancer, Player, 1, Stats::new(24, 7, 5, 5, 3, 7, 1), Stats::new(85, 50, 40, 40, 35, 40, 15), weapons::iron_lance(), p(2, 10)), 2),
        Unit::new("Tamsin", Archer, Player, 1, Stats::new(18, 6, 7, 6, 4, 4, 2), Stats::new(65, 45, 55, 50, 40, 20, 20), weapons::short_bow(), p(0, 10)),
        Unit::new("Orrin", Mage, Player, 1, Stats::new(17, 7, 6, 6, 3, 2, 6), Stats::new(55, 60, 45, 50, 30, 15, 45), weapons::ember(), p(1, 11)),
        Unit::new("Liese", Cleric, Player, 1, Stats::new(16, 5, 4, 7, 7, 1, 7), Stats::new(50, 50, 30, 55, 60, 10, 55), weapons::mend(), p(0, 11)),
        // ---- Enemy ----
        lvl(Unit::new("Brigand", Brigand, Enemy, 1, Stats::new(22, 6, 2, 3, 0, 3, 0), none, weapons::iron_axe(), p(6, 3)), 2),
        lvl(Unit::new("Brigand", Brigand, Enemy, 1, Stats::new(22, 6, 2, 3, 0, 3, 0), none, weapons::iron_axe(), p(6, 7)), 2),
        lvl(Unit::new("Soldier", Soldier, Enemy, 1, Stats::new(20, 5, 3, 3, 0, 5, 0), none, weapons::iron_lance(), p(11, 6)), 2),
        lvl(Unit::new("Soldier", Soldier, Enemy, 1, Stats::new(20, 5, 3, 3, 0, 5, 0), none, weapons::iron_lance(), p(11, 2)), 2),
        lvl(Unit::new("Sellsword", Mercenary, Enemy, 1, Stats::new(21, 5, 7, 7, 2, 4, 1), none, weapons::keen_sword(), p(13, 10)), 3),
        lvl(Unit::new("Bowman", Archer, Enemy, 1, Stats::new(18, 5, 5, 4, 1, 3, 1), none, weapons::short_bow(), p(12, 7)), 2),
        lvl(Unit::new("Hexer", Mage, Enemy, 1, Stats::new(17, 5, 4, 4, 1, 2, 4), none, weapons::gloom(), p(12, 2)), 3),
        lvl(Unit::new("Varga Ironjaw", Warlord, Enemy, 1, Stats::new(28, 8, 5, 4, 2, 6, 2), none, weapons::cleaver(), p(14, 1)), 5).boss(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::unit::Team;
    use std::collections::HashSet;

    #[test]
    fn map_is_16_by_12() {
        let m = map();
        assert_eq!((m.width, m.height), (16, 12));
    }

    #[test]
    fn roster_is_valid() {
        let m = map();
        let us = units();
        assert_eq!(us.iter().filter(|u| u.team == Team::Player).count(), 5);
        assert_eq!(us.iter().filter(|u| u.team == Team::Enemy).count(), 8);
        assert_eq!(us.iter().filter(|u| u.is_lord).count(), 1);
        assert_eq!(us.iter().filter(|u| u.is_boss).count(), 1);
        let mut seen = HashSet::new();
        for u in &us {
            assert!(seen.insert(u.pos), "{} overlaps another unit", u.name);
            assert!(m.at(u.pos).move_cost(u.class.move_type()).is_some(), "{} starts on impassable tile", u.name);
        }
    }
}
