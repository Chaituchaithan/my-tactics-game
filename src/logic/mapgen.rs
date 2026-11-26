//! Deterministic map generator. Each chapter describes a `MapSpec` (size,
//! theme, features); the same spec always produces the same map.

use std::collections::{HashSet, VecDeque};

use super::map::Map;
use super::rng::Rng;
use super::terrain::{MoveType, Terrain};
use super::Pos;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Grass,
    Woods,
    Highlands,
    Marsh,
    Ruins,
    /// Most of the map is one large castle with rooms.
    Fortress,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// Player starts on the left edge, the enemy holds the right.
    March,
    /// Player starts in the middle; enemies come from all sides.
    Defend,
}

#[derive(Clone, Copy, Debug)]
pub struct MapSpec {
    pub w: i32,
    pub h: i32,
    pub seed: u64,
    pub theme: Theme,
    pub layout: Layout,
    pub river: bool,
    pub castle: bool,
    pub villages: usize,
    pub forts: usize,
    pub starts: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl Area {
    pub fn contains(&self, p: Pos) -> bool {
        p.x >= self.x0 && p.x <= self.x1 && p.y >= self.y0 && p.y <= self.y1
    }
}

pub struct Generated {
    pub map: Map,
    /// Chest tiles (also marked on the map).
    #[allow(dead_code)]
    pub chests: Vec<Pos>,
    pub starts: Vec<Pos>,
    pub throne: Option<Pos>,
    pub castle: Option<Area>,
    pub villages: Vec<Pos>,
    /// Tiles an infantry unit can walk to from the first start tile.
    pub reachable: HashSet<Pos>,
}

/// Smoothed value noise normalised to 0..1.
fn noise(w: i32, h: i32, rng: &mut Rng, passes: usize) -> Vec<f32> {
    let mut v: Vec<f32> = (0..w * h).map(|_| rng.percent() as f32 / 100.0).collect();
    for _ in 0..passes {
        let mut n = v.clone();
        for y in 0..h {
            for x in 0..w {
                let mut s = 0.0;
                let mut c = 0.0;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let (nx, ny) = (x + dx, y + dy);
                        if nx >= 0 && ny >= 0 && nx < w && ny < h {
                            s += v[(ny * w + nx) as usize];
                            c += 1.0;
                        }
                    }
                }
                n[(y * w + x) as usize] = s / c;
            }
        }
        v = n;
    }
    let min = v.iter().cloned().fold(f32::MAX, f32::min);
    let max = v.iter().cloned().fold(f32::MIN, f32::max);
    v.iter().map(|x| (x - min) / (max - min).max(1e-6)).collect()
}

/// Value such that `frac` of the noise lies above it.
fn quantile(v: &[f32], frac: f32) -> f32 {
    if frac <= 0.0 {
        return 2.0;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let i = ((1.0 - frac) * (s.len() - 1) as f32).round() as usize;
    s[i.min(s.len() - 1)]
}

struct Grid {
    w: i32,
    h: i32,
    t: Vec<Terrain>,
}

impl Grid {
    fn get(&self, x: i32, y: i32) -> Terrain {
        self.t[(y * self.w + x) as usize]
    }
    fn set(&mut self, x: i32, y: i32, t: Terrain) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.t[(y * self.w + x) as usize] = t;
        }
    }
    fn inb(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h
    }
}

fn reach_from(g: &Grid, start: Pos) -> HashSet<Pos> {
    reach_with(g, start, false)
}

/// Flood fill; `doors_open` treats locked doors as passable.
fn reach_with(g: &Grid, start: Pos, doors_open: bool) -> HashSet<Pos> {
    let mut seen = HashSet::new();
    let mut q = VecDeque::new();
    seen.insert(start);
    q.push_back(start);
    while let Some(p) = q.pop_front() {
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = Pos::new(p.x + dx, p.y + dy);
            let t = if g.inb(n.x, n.y) { g.get(n.x, n.y) } else { Terrain::Wall };
            let ok = t.move_cost(MoveType::Infantry).is_some() || (doors_open && t == Terrain::Door);
            if g.inb(n.x, n.y) && ok && seen.insert(n) {
                q.push_back(n);
            }
        }
    }
    seen
}

fn build_castle(g: &mut Grid, rng: &mut Rng, a: Area, gate_left: bool, rooms: bool) -> Pos {
    for y in a.y0..=a.y1 {
        for x in a.x0..=a.x1 {
            let edge = x == a.x0 || x == a.x1 || y == a.y0 || y == a.y1;
            g.set(x, y, if edge { Terrain::Wall } else { Terrain::Floor });
        }
    }
    let cy = (a.y0 + a.y1) / 2;
    // Interior partitions with doorways.
    let inner_w = a.x1 - a.x0 - 1;
    let step = if rooms { 6 } else { inner_w.max(1) / 2 + 1 };
    let mut x = a.x0 + step;
    while x < a.x1 - 3 {
        for y in a.y0 + 1..a.y1 {
            g.set(x, y, Terrain::Wall);
        }
        let h = a.y1 - a.y0 - 1;
        let d1 = a.y0 + 1 + (rng.percent() % (h / 2).max(1));
        let d2 = a.y1 - 1 - (rng.percent() % (h / 2).max(1));
        for d in [d1, d2] {
            // Side passages are sometimes locked.
            g.set(x, d, if rng.roll(45) { Terrain::Door } else { Terrain::Floor });
        }
        g.set(x, cy, Terrain::Floor);
        x += step;
    }
    if rooms {
        // A horizontal hall wall with gaps in each room.
        let hy = a.y0 + (a.y1 - a.y0) / 3;
        for x in a.x0 + 1..a.x1 {
            if g.get(x, hy) == Terrain::Floor && (x - a.x0) % 4 != 2 {
                g.set(x, hy, Terrain::Wall);
            }
        }
        let hy2 = a.y1 - (a.y1 - a.y0) / 3;
        for x in a.x0 + 1..a.x1 {
            if g.get(x, hy2) == Terrain::Floor && (x - a.x0) % 4 != 0 {
                g.set(x, hy2, Terrain::Wall);
            }
        }
        // Keep the central corridor open.
        for x in a.x0 + 1..a.x1 {
            g.set(x, cy, Terrain::Floor);
        }
    }
    // Pillars
    for _ in 0..((a.x1 - a.x0) * (a.y1 - a.y0) / 18).max(2) {
        let px = a.x0 + 2 + (rng.percent() % (a.x1 - a.x0 - 3).max(1));
        let py = a.y0 + 2 + (rng.percent() % (a.y1 - a.y0 - 3).max(1));
        if g.get(px, py) == Terrain::Floor && py != cy {
            g.set(px, py, Terrain::Pillar);
        }
    }
    // Gates
    if gate_left {
        g.set(a.x0, cy, Terrain::Gate);
        g.set(a.x0, cy - 1, Terrain::Floor);
        for x in (a.x0 - 3).max(0)..a.x0 {
            for y in cy - 1..=cy {
                g.set(x, y, Terrain::Plains);
            }
        }
    }
    let gx = (a.x0 + a.x1) / 2;
    g.set(gx, a.y0, Terrain::Gate);
    g.set(gx, a.y1, Terrain::Gate);
    for d in 1..3 {
        g.set(gx, a.y0 - d, Terrain::Plains);
        g.set(gx, a.y1 + d, Terrain::Plains);
    }
    let throne = Pos::new(a.x1 - 2, cy);
    g.set(throne.x, throne.y, Terrain::Throne);
    g.set(throne.x - 1, throne.y, Terrain::Floor);
    throne
}

pub fn generate(spec: &MapSpec) -> Generated {
    let (w, h) = (spec.w, spec.h);
    let mut rng = Rng::new(spec.seed);
    let mut g = Grid { w, h, t: vec![Terrain::Plains; (w * h) as usize] };

    let (forest, mountain, water) = match spec.theme {
        Theme::Grass => (0.13, 0.05, 0.0),
        Theme::Woods => (0.34, 0.05, 0.0),
        Theme::Highlands => (0.12, 0.26, 0.0),
        Theme::Marsh => (0.16, 0.02, 0.2),
        Theme::Ruins => (0.15, 0.08, 0.0),
        Theme::Fortress => (0.12, 0.05, 0.0),
    };
    let nf = noise(w, h, &mut rng, 1);
    let nm = noise(w, h, &mut rng, 2);
    let nw = noise(w, h, &mut rng, 3);
    let (tf, tm, tw) = (quantile(&nf, forest), quantile(&nm, mountain), quantile(&nw, water));
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            let t = if nw[i] >= tw {
                Terrain::Water
            } else if nm[i] >= tm {
                Terrain::Mountain
            } else if nf[i] >= tf {
                Terrain::Forest
            } else {
                Terrain::Plains
            };
            g.set(x, y, t);
        }
    }
    if spec.theme == Theme::Ruins {
        for _ in 0..(w * h / 40) {
            let x = rng.percent() % w;
            let y = rng.percent() % h;
            let len = 2 + rng.percent() % 3;
            let horiz = rng.roll(50);
            for k in 0..len {
                let (px, py) = if horiz { (x + k, y) } else { (x, y + k) };
                if g.inb(px, py) {
                    g.set(px, py, if k == len / 2 && rng.roll(40) { Terrain::Pillar } else { Terrain::Wall });
                }
            }
        }
    }

    // River with bridges (vertical, between the two sides).
    if spec.river {
        let mut rx = (w as f32 * (0.42 + rng.percent() as f32 / 1000.0)) as i32;
        let n_bridges = (h / 6).max(2);
        let bridge_rows: Vec<i32> = (0..n_bridges).map(|k| ((k as f32 + 0.5) * h as f32 / n_bridges as f32) as i32).collect();
        for y in 0..h {
            let step = rng.percent() % 3;
            if step == 0 && rx > w / 3 {
                rx -= 1;
            } else if step == 1 && rx < w * 3 / 5 {
                rx += 1;
            }
            let bridge = bridge_rows.contains(&y);
            for x in rx..rx + 2 {
                g.set(x, y, if bridge { Terrain::Bridge } else { Terrain::Water });
            }
            if bridge {
                for x in [rx - 2, rx - 1, rx + 2, rx + 3] {
                    if g.inb(x, y) {
                        g.set(x, y, Terrain::Plains);
                    }
                }
            }
        }
    }

    // Castle
    let mut castle = None;
    let mut throne = None;
    if spec.theme == Theme::Fortress {
        let a = Area { x0: 6, y0: 1, x1: w - 2, y1: h - 2 };
        throne = Some(build_castle(&mut g, &mut rng, a, true, true));
        castle = Some(a);
    } else if spec.castle {
        let cw = (w * 3 / 10).clamp(8, 13);
        let ch = (h / 2).clamp(7, 11);
        let x0 = if spec.layout == Layout::Defend { (w - cw) / 2 } else { w - cw - 1 };
        let y0 = if spec.layout == Layout::Defend { 1 } else { (h - ch) / 2 };
        let a = Area { x0, y0, x1: x0 + cw - 1, y1: y0 + ch - 1 };
        throne = Some(build_castle(&mut g, &mut rng, a, true, false));
        castle = Some(a);
    }

    // Player deployment zone, cleared of obstacles.
    let center = match spec.layout {
        Layout::March => Pos::new(1, h / 2),
        Layout::Defend => Pos::new(w / 2, h / 2 + if castle.is_some() { 3 } else { 0 }),
    };
    let (zx0, zx1, zy0, zy1) = match spec.layout {
        Layout::March => (0, 3, center.y - 4, center.y + 4),
        Layout::Defend => (center.x - 3, center.x + 3, center.y - 2, center.y + 2),
    };
    for y in zy0..=zy1 {
        for x in zx0..=zx1 {
            if g.inb(x, y) && castle.is_none_or(|c| !c.contains(Pos::new(x, y))) {
                let t = g.get(x, y);
                if t != Terrain::Forest || (x + y) % 3 == 0 {
                    g.set(x, y, Terrain::Plains);
                }
            }
        }
    }
    let mut zone: Vec<Pos> = Vec::new();
    for y in zy0..=zy1 {
        for x in zx0..=zx1 {
            if g.inb(x, y) && g.get(x, y).move_cost(MoveType::Infantry).is_some() {
                zone.push(Pos::new(x, y));
            }
        }
    }
    zone.sort_by_key(|p| (p.manhattan(center), p.y, p.x));
    let starts: Vec<Pos> = zone.into_iter().take(spec.starts).collect();

    // Forts
    let mut forts = Vec::new();
    let reach0 = reach_from(&g, starts[0]);
    let mut cands: Vec<Pos> = reach0.iter().copied().filter(|p| g.get(p.x, p.y) == Terrain::Plains && !starts.contains(p)).collect();
    cands.sort();
    for _ in 0..spec.forts {
        if cands.is_empty() {
            break;
        }
        let i = rng.next_u64() as usize % cands.len();
        let p = cands.swap_remove(i);
        if forts.iter().all(|f: &Pos| f.manhattan(p) >= 5) && p.manhattan(starts[0]) >= 3 {
            g.set(p.x, p.y, Terrain::Fort);
            forts.push(p);
        }
    }
    // Villages: prefer the player's half / near the defended centre.
    let mut villages = Vec::new();
    let mut tries = 0;
    while villages.len() < spec.villages && tries < 400 && !cands.is_empty() {
        tries += 1;
        let i = rng.next_u64() as usize % cands.len();
        let p = cands[i];
        let ok_side = match spec.layout {
            Layout::March => p.x < w * 2 / 3 && p.x > 3,
            Layout::Defend => p.manhattan(center) <= w / 2,
        };
        if ok_side && g.get(p.x, p.y) == Terrain::Plains && villages.iter().all(|v: &Pos| v.manhattan(p) >= 5) && p.manhattan(starts[0]) >= 3 {
            g.set(p.x, p.y, Terrain::Village);
            villages.push(p);
            cands.swap_remove(i);
        }
    }

    // Treasure chests in castle rooms (possibly behind locked doors).
    let mut chests = Vec::new();
    if let Some(c) = castle {
        let open = reach_with(&g, starts[0], true);
        let cy = (c.y0 + c.y1) / 2;
        let mut spots: Vec<Pos> = open
            .iter()
            .copied()
            .filter(|p| c.contains(*p) && g.get(p.x, p.y) == Terrain::Floor && p.y != cy && p.y != cy - 1 && throne.is_none_or(|t| t.manhattan(*p) > 1))
            .collect();
        spots.sort();
        let n = if spec.theme == Theme::Fortress { 3 } else { 2 };
        for _ in 0..n {
            if spots.is_empty() {
                break;
            }
            let i = rng.next_u64() as usize % spots.len();
            let p = spots.swap_remove(i);
            if chests.iter().all(|q: &Pos| q.manhattan(p) > 2) {
                g.set(p.x, p.y, Terrain::Chest);
                chests.push(p);
            }
        }
    }

    let reachable = reach_from(&g, starts[0]);
    let rows: Vec<String> = (0..h)
        .map(|y| (0..w).map(|x| terrain_char(g.get(x, y))).collect())
        .collect();
    let refs: Vec<&str> = rows.iter().map(|s| s.as_str()).collect();
    Generated { map: Map::from_rows(&refs), chests, starts, throne, castle, villages, reachable }
}

pub fn terrain_char(t: Terrain) -> char {
    match t {
        Terrain::Plains => '.',
        Terrain::Forest => 'F',
        Terrain::Mountain => 'M',
        Terrain::Fort => 'T',
        Terrain::Water => '~',
        Terrain::Bridge => '=',
        Terrain::Village => 'V',
        Terrain::Throne => 'H',
        Terrain::Wall => '#',
        Terrain::Floor => ',',
        Terrain::Pillar => 'P',
        Terrain::Gate => 'G',
        Terrain::Door => 'D',
        Terrain::Chest => 'C',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(theme: Theme, layout: Layout, river: bool, castle: bool) -> MapSpec {
        MapSpec { w: 28, h: 18, seed: 9, theme, layout, river, castle, villages: 3, forts: 2, starts: 12 }
    }

    #[test]
    fn deterministic() {
        let s = spec(Theme::Woods, Layout::March, true, true);
        let a = generate(&s);
        let b = generate(&s);
        for y in 0..18 {
            for x in 0..28 {
                assert_eq!(a.map.at(Pos::new(x, y)), b.map.at(Pos::new(x, y)));
            }
        }
    }

    #[test]
    fn every_theme_is_playable() {
        for theme in [Theme::Grass, Theme::Woods, Theme::Highlands, Theme::Marsh, Theme::Ruins, Theme::Fortress] {
            for layout in [Layout::March, Layout::Defend] {
                for seed in 0..6 {
                    let mut s = spec(theme, layout, layout == Layout::March && seed % 2 == 0, seed % 3 != 0);
                    s.seed = seed;
                    let g = generate(&s);
                    assert_eq!(g.starts.len(), 12, "{theme:?} {layout:?} {seed}");
                    for st in &g.starts {
                        assert!(g.reachable.contains(st), "start cut off");
                    }
                    if let Some(t) = g.throne {
                        assert!(g.reachable.contains(&t), "{theme:?} {layout:?} seed {seed}: throne unreachable");
                    }
                    for v in &g.villages {
                        assert!(g.reachable.contains(v));
                    }
                    for c in &g.chests {
                        assert_eq!(g.map.at(*c), Terrain::Chest);
                    }
                    assert!(g.reachable.len() > (28 * 18) / 3, "{theme:?} {layout:?} {seed}: too little walkable ground ({})", g.reachable.len());
                }
            }
        }
    }

    #[test]
    fn river_has_crossings() {
        let g = generate(&spec(Theme::Grass, Layout::March, true, false));
        let bridges = (0..18).filter(|&y| (0..28).any(|x| g.map.at(Pos::new(x, y)) == Terrain::Bridge)).count();
        assert!(bridges >= 2);
        assert!(g.reachable.contains(&Pos::new(27, 9)) || g.reachable.iter().any(|p| p.x > 20));
    }
}
