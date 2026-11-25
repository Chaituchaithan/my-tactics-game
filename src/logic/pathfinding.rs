use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use super::map::Map;
use super::terrain::MoveType;
use super::Pos;

/// What occupies a tile, from the moving unit's point of view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Occupant {
    Empty,
    /// Friendly unit: can be passed through but not stopped on.
    Ally,
    /// Hostile unit: blocks movement.
    Foe,
}

#[derive(Clone, Debug)]
pub struct Reach {
    pub start: Pos,
    /// Cheapest movement cost to every reachable tile (including pass-through tiles).
    pub cost: HashMap<Pos, i32>,
    prev: HashMap<Pos, Pos>,
    /// Tiles the unit may end its move on.
    pub stoppable: Vec<Pos>,
}

impl Reach {
    #[cfg(test)]
    pub fn can_stop(&self, p: Pos) -> bool {
        self.stoppable.contains(&p)
    }

    /// Path from start to `dest`, inclusive of both ends.
    pub fn path_to(&self, dest: Pos) -> Option<Vec<Pos>> {
        if !self.cost.contains_key(&dest) {
            return None;
        }
        let mut path = vec![dest];
        let mut cur = dest;
        while cur != self.start {
            cur = *self.prev.get(&cur)?;
            path.push(cur);
        }
        path.reverse();
        Some(path)
    }
}

/// Dijkstra flood-fill of all tiles reachable from `start` within `mov` points.
pub fn reachable(map: &Map, start: Pos, mov: i32, mt: MoveType, occupant: impl Fn(Pos) -> Occupant) -> Reach {
    let mut cost = HashMap::new();
    let mut prev = HashMap::new();
    let mut heap = BinaryHeap::new();
    cost.insert(start, 0);
    heap.push(Reverse((0, start)));

    while let Some(Reverse((c, p))) = heap.pop() {
        if c > *cost.get(&p).unwrap_or(&i32::MAX) {
            continue;
        }
        for n in map.neighbors(p) {
            let Some(step) = map.at(n).move_cost(mt) else { continue };
            if occupant(n) == Occupant::Foe {
                continue;
            }
            let nc = c + step;
            if nc > mov {
                continue;
            }
            if nc < *cost.get(&n).unwrap_or(&i32::MAX) {
                cost.insert(n, nc);
                prev.insert(n, p);
                heap.push(Reverse((nc, n)));
            }
        }
    }

    let mut stoppable: Vec<Pos> = cost
        .keys()
        .copied()
        .filter(|&p| p == start || occupant(p) == Occupant::Empty)
        .collect();
    stoppable.sort();
    Reach { start, cost, prev, stoppable }
}

/// All in-bounds tiles whose Manhattan distance from `center` is within [min, max].
pub fn tiles_in_range(map: &Map, center: Pos, min: i32, max: i32) -> Vec<Pos> {
    let mut out = Vec::new();
    for dy in -max..=max {
        for dx in -max..=max {
            let d = dx.abs() + dy.abs();
            let p = Pos::new(center.x + dx, center.y + dy);
            if d >= min && d <= max && map.in_bounds(p) {
                out.push(p);
            }
        }
    }
    out
}

/// Multi-source movement-cost field: how expensive it is to walk from each
/// tile to the nearest source. Used by the AI to advance toward targets.
pub fn distance_field(map: &Map, sources: &[Pos], mt: MoveType) -> HashMap<Pos, i32> {
    let mut dist = HashMap::new();
    let mut heap = BinaryHeap::new();
    for &s in sources {
        dist.insert(s, 0);
        heap.push(Reverse((0, s)));
    }
    while let Some(Reverse((c, p))) = heap.pop() {
        if c > *dist.get(&p).unwrap_or(&i32::MAX) {
            continue;
        }
        // Cost of stepping from n into p: p's terrain cost (approximation of the reverse walk).
        let Some(step) = map.at(p).move_cost(mt).or(if sources.contains(&p) { Some(1) } else { None }) else {
            continue;
        };
        for n in map.neighbors(p) {
            if map.at(n).move_cost(mt).is_none() {
                continue;
            }
            let nc = c + step;
            if nc < *dist.get(&n).unwrap_or(&i32::MAX) {
                dist.insert(n, nc);
                heap.push(Reverse((nc, n)));
            }
        }
    }
    dist
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::terrain::Terrain;

    fn empty(_: Pos) -> Occupant {
        Occupant::Empty
    }

    #[test]
    fn open_field_diamond() {
        let map = Map::filled(11, 11, Terrain::Plains);
        let r = reachable(&map, Pos::new(5, 5), 3, MoveType::Infantry, empty);
        // 1 + 4 + 8 + 12 tiles within distance 3
        assert_eq!(r.stoppable.len(), 25);
        assert_eq!(r.cost[&Pos::new(8, 5)], 3);
        assert!(!r.cost.contains_key(&Pos::new(9, 5)));
    }

    #[test]
    fn clipped_by_map_edge() {
        let map = Map::filled(4, 4, Terrain::Plains);
        let r = reachable(&map, Pos::new(0, 0), 1, MoveType::Infantry, empty);
        assert_eq!(r.stoppable.len(), 3);
    }

    #[test]
    fn terrain_costs() {
        // Start at left, forest costs 2, mountain 3 for infantry, 2 for mountaineers
        let map = Map::from_rows(&[".FM.."]);
        let r = reachable(&map, Pos::new(0, 0), 4, MoveType::Infantry, empty);
        assert_eq!(r.cost[&Pos::new(1, 0)], 2);
        assert!(!r.cost.contains_key(&Pos::new(2, 0)), "2 + 3 > 4");
        let r = reachable(&map, Pos::new(0, 0), 4, MoveType::Mountaineer, empty);
        assert_eq!(r.cost[&Pos::new(2, 0)], 4);
    }

    #[test]
    fn water_is_impassable_bridge_is_not() {
        let map = Map::from_rows(&[
            "..~..",
            "..~..",
            "..=..",
        ]);
        let r = reachable(&map, Pos::new(0, 0), 20, MoveType::Infantry, empty);
        assert!(!r.cost.contains_key(&Pos::new(2, 0)));
        assert!(!r.cost.contains_key(&Pos::new(2, 1)));
        assert!(r.cost.contains_key(&Pos::new(2, 2)));
        // Must detour through the bridge: (0,0)->(0,2)->(4,2)->(4,0) = 2 + 4 + 2
        assert_eq!(r.cost[&Pos::new(4, 0)], 8);
    }

    #[test]
    fn prefers_cheap_path_around_forest() {
        let map = Map::from_rows(&[
            "...",
            ".F.",
            "...",
        ]);
        let r = reachable(&map, Pos::new(0, 1), 5, MoveType::Infantry, empty);
        // Through the forest: 2 + 1 = 3; around it via the top row: 4
        assert_eq!(r.cost[&Pos::new(2, 1)], 3);
        let path = r.path_to(Pos::new(2, 1)).unwrap();
        assert_eq!(path.first(), Some(&Pos::new(0, 1)));
        assert_eq!(path.last(), Some(&Pos::new(2, 1)));
        // Path cost must equal the recorded cost.
        let cost: i32 = path[1..].iter().map(|p| map.at(*p).move_cost(MoveType::Infantry).unwrap()).sum();
        assert_eq!(cost, 3);
        // Each step is adjacent.
        for w in path.windows(2) {
            assert_eq!(w[0].manhattan(w[1]), 1);
        }
    }

    #[test]
    fn foes_block_movement() {
        let map = Map::from_rows(&[
            "~~~~~",
            ".....",
            "~~~~~",
        ]);
        let foe = Pos::new(2, 1);
        let r = reachable(&map, Pos::new(0, 1), 10, MoveType::Infantry, |p| if p == foe { Occupant::Foe } else { Occupant::Empty });
        assert!(r.cost.contains_key(&Pos::new(1, 1)));
        assert!(!r.cost.contains_key(&foe));
        assert!(!r.cost.contains_key(&Pos::new(3, 1)));
    }

    #[test]
    fn allies_pass_but_cannot_stop() {
        let map = Map::from_rows(&[
            "~~~~~",
            ".....",
            "~~~~~",
        ]);
        let ally = Pos::new(2, 1);
        let r = reachable(&map, Pos::new(0, 1), 10, MoveType::Infantry, |p| if p == ally { Occupant::Ally } else { Occupant::Empty });
        assert!(r.cost.contains_key(&ally));
        assert!(!r.can_stop(ally));
        assert!(r.can_stop(Pos::new(4, 1)));
        assert!(r.can_stop(Pos::new(0, 1)), "can always stay in place");
    }

    #[test]
    fn zero_move_only_start() {
        let map = Map::filled(5, 5, Terrain::Plains);
        let r = reachable(&map, Pos::new(2, 2), 0, MoveType::Infantry, empty);
        assert_eq!(r.stoppable, vec![Pos::new(2, 2)]);
        assert_eq!(r.path_to(Pos::new(2, 2)), Some(vec![Pos::new(2, 2)]));
        assert_eq!(r.path_to(Pos::new(3, 2)), None);
    }

    #[test]
    fn range_rings() {
        let map = Map::filled(9, 9, Terrain::Plains);
        assert_eq!(tiles_in_range(&map, Pos::new(4, 4), 1, 1).len(), 4);
        assert_eq!(tiles_in_range(&map, Pos::new(4, 4), 2, 2).len(), 8);
        assert_eq!(tiles_in_range(&map, Pos::new(4, 4), 1, 2).len(), 12);
        assert_eq!(tiles_in_range(&map, Pos::new(0, 0), 1, 1).len(), 2);
    }

    #[test]
    fn distance_field_routes_around_water() {
        let map = Map::from_rows(&[
            "..~..",
            "..~..",
            "..=..",
        ]);
        let d = distance_field(&map, &[Pos::new(4, 0)], MoveType::Infantry);
        assert_eq!(d[&Pos::new(4, 0)], 0);
        assert!(!d.contains_key(&Pos::new(2, 0)));
        assert!(d[&Pos::new(0, 0)] > d[&Pos::new(3, 0)]);
        assert_eq!(d[&Pos::new(0, 0)], 8);
    }
}
