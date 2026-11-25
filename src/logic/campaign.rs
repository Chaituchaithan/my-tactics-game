//! The campaign "Emberfall": 16 chapters in two acts, the story, and the
//! persistent army. Chapters 1-4 use hand-made small maps; the rest are
//! generated (deterministically) at medium, large and extra-large sizes.

use serde::{Deserialize, Serialize};

use super::battle::{Battle, Objective, Reward, Village};
use super::items::ItemId;
use super::map::Map;
use super::mapgen::{self, Generated, Layout, MapSpec, Theme};
use super::rng::Rng;
use super::supports::{self, SupportState};
use super::terrain::Terrain;
use super::unit::{AiMode, Class, Stats, Team, Unit};
use super::Pos;

#[derive(Clone, Copy, Debug)]
pub struct Line {
    /// Character name ("" for narration).
    pub speaker: &'static str,
    pub text: &'static str,
}

const fn l(speaker: &'static str, text: &'static str) -> Line {
    Line { speaker, text }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    Small,
    Medium,
    Large,
    ExtraLarge,
}

impl Tier {
    pub fn name(self) -> &'static str {
        match self {
            Tier::Small => "Small",
            Tier::Medium => "Medium",
            Tier::Large => "Large",
            Tier::ExtraLarge => "Extra Large",
        }
    }

    pub fn size(self) -> (i32, i32) {
        match self {
            Tier::Small => (16, 12),
            Tier::Medium => (22, 15),
            Tier::Large => (28, 18),
            Tier::ExtraLarge => (34, 22),
        }
    }

    pub fn deploy(self) -> usize {
        match self {
            Tier::Small => 8,
            Tier::Medium => 10,
            Tier::Large => 12,
            Tier::ExtraLarge => 14,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Goal {
    Boss,
    Rout,
    /// Seize the throne tile of the map.
    Seize,
    Survive(u32),
}

/// Where a generated squad is placed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Front,
    Mid,
    Rear,
    Castle,
    Throne,
    /// Map edges on the enemy side (used for reinforcements).
    Edge,
}

#[derive(Clone, Copy, Debug)]
pub struct Squad {
    pub name: &'static str,
    pub class: Class,
    pub level: i32,
    pub items: &'static [ItemId],
    pub count: usize,
    pub zone: Zone,
    pub ai: AiMode,
}

const fn sq(name: &'static str, class: Class, level: i32, items: &'static [ItemId], count: usize, zone: Zone) -> Squad {
    Squad { name, class, level, items, count, zone, ai: AiMode::Aggressive }
}

const fn guard(name: &'static str, class: Class, level: i32, items: &'static [ItemId], count: usize, zone: Zone) -> Squad {
    Squad { name, class, level, items, count, zone, ai: AiMode::Guard }
}

pub enum Field {
    Fixed {
        rows: [&'static str; 12],
        starts: Vec<Pos>,
        enemies: Vec<Unit>,
        reinforcements: Vec<(u32, Unit)>,
        villages: Vec<(Pos, Reward)>,
    },
    Generated {
        spec: MapSpec,
        squads: Vec<Squad>,
        /// Bosses and named characters with their zones.
        named: Vec<(Unit, Zone)>,
        reinforcements: Vec<(u32, Squad)>,
        rewards: Vec<Reward>,
    },
}

pub struct ChapterDef {
    pub title: &'static str,
    pub tier: Tier,
    pub goal: Goal,
    pub field: Field,
    /// Allies who join the army at the start of this chapter.
    pub joins: Vec<Unit>,
    pub intro: Vec<Line>,
    pub outro: Vec<Line>,
    /// Conversation when the lord talks to a recruitable enemy (by name).
    pub talks: Vec<(&'static str, Vec<Line>)>,
    pub reward_gold: i32,
}

/// Everything needed to start a battle.
pub struct Setup {
    pub map: Map,
    pub chests: Vec<(Pos, Reward)>,
    pub fog: bool,
    pub starts: Vec<Pos>,
    pub enemies: Vec<Unit>,
    pub reinforcements: Vec<(u32, Unit)>,
    pub villages: Vec<(Pos, Reward)>,
    pub objective: Objective,
}

fn p(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

fn e(name: &str, class: Class, level: i32, items: &[ItemId], x: i32, y: i32) -> Unit {
    Unit::generic(name, class, Team::Enemy, level, items, p(x, y))
}

fn ally(name: &str, class: Class, level: i32, stats: Stats, growths: Stats, items: &[ItemId]) -> Unit {
    Unit::new(name, class, Team::Player, level, stats, growths, items, p(0, 0))
}

fn boss(name: &str, class: Class, level: i32, items: &[ItemId]) -> Unit {
    Unit::generic(name, class, Team::Enemy, level, items, p(0, 0)).boss()
}

fn s(hp: i32, pow: i32, skl: i32, spd: i32, lck: i32, def: i32, res: i32) -> Stats {
    Stats::new(hp, pow, skl, spd, lck, def, res)
}

#[allow(clippy::too_many_arguments)]
fn spec(tier: Tier, seed: u64, theme: Theme, layout: Layout, river: bool, castle: bool, villages: usize, forts: usize) -> MapSpec {
    let (w, h) = tier.size();
    MapSpec { w, h, seed, theme, layout, river, castle, villages, forts, starts: tier.deploy() }
}

pub const CHAPTER_COUNT: usize = 16;

pub fn starting_army() -> Vec<Unit> {
    use Class::*;
    use ItemId::*;
    vec![
        ally("Kael", Lord, 1, s(20, 6, 7, 8, 6, 5, 2), s(80, 50, 55, 60, 50, 30, 25), &[Oathblade, Vulnerary]).lord(),
        ally("Brenna", Lancer, 2, s(24, 7, 5, 5, 3, 7, 1), s(85, 50, 40, 40, 35, 40, 15), &[IronLance, Javelin]),
        ally("Tamsin", Archer, 1, s(18, 6, 7, 6, 4, 4, 2), s(65, 45, 55, 50, 40, 20, 20), &[ShortBow]),
        ally("Orrin", Mage, 1, s(17, 7, 6, 6, 3, 2, 6), s(55, 60, 45, 50, 30, 15, 45), &[Ember]),
        ally("Liese", Cleric, 1, s(16, 5, 4, 7, 7, 1, 7), s(50, 50, 30, 55, 60, 10, 55), &[Mend, Vulnerary]),
    ]
}

fn zone_ok(g: &Generated, sp: &MapSpec, zone: Zone, q: Pos) -> bool {
    let fx = q.x as f32 / sp.w as f32;
    let in_castle = g.castle.is_some_and(|c| c.contains(q));
    let edge = q.x == sp.w - 1 || q.y == 0 || q.y == sp.h - 1 || q.x == 0;
    match sp.layout {
        Layout::March => match zone {
            Zone::Front => (0.33..0.62).contains(&fx),
            Zone::Mid => (0.5..0.82).contains(&fx),
            Zone::Rear => fx >= 0.72,
            Zone::Castle => in_castle,
            Zone::Throne => Some(q) == g.throne,
            Zone::Edge => edge && fx > 0.6,
        },
        Layout::Defend => {
            let c = g.starts[0];
            let d = q.manhattan(c) as f32 / ((sp.w + sp.h) as f32 / 2.0);
            match zone {
                Zone::Front => (0.3..0.5).contains(&d),
                Zone::Mid => (0.42..0.68).contains(&d),
                Zone::Rear => d >= 0.6,
                Zone::Castle => in_castle,
                Zone::Throne => Some(q) == g.throne,
                Zone::Edge => edge,
            }
        }
    }
}

/// Picks free tiles in `zone` for `n` units of `class`.
fn place(g: &Generated, sp: &MapSpec, zone: Zone, class: Class, n: usize, taken: &mut Vec<Pos>, rng: &mut Rng) -> Vec<Pos> {
    let mut cands: Vec<Pos> = g
        .reachable
        .iter()
        .copied()
        .filter(|q| zone_ok(g, sp, zone, *q))
        .filter(|q| g.map.at(*q).move_cost(class.move_type()).is_some() && g.map.at(*q) != Terrain::Village)
        .filter(|q| !taken.contains(q) && g.starts.iter().all(|s| s.manhattan(*q) >= 5))
        .collect();
    cands.sort();
    // Fall back to anywhere on the enemy side if the zone is too crowded.
    if cands.len() < n && zone != Zone::Throne {
        let more: Vec<Pos> = g
            .reachable
            .iter()
            .copied()
            .filter(|q| g.map.at(*q).move_cost(class.move_type()).is_some() && !taken.contains(q) && g.starts.iter().all(|s| s.manhattan(*q) >= 7))
            .collect();
        cands.extend(more);
        cands.sort();
        cands.dedup();
    }
    let mut out = Vec::new();
    for _ in 0..n {
        if cands.is_empty() {
            break;
        }
        let i = rng.next_u64() as usize % cands.len();
        let q = cands.swap_remove(i);
        taken.push(q);
        out.push(q);
    }
    out
}

fn throne_of(map: &Map) -> Option<Pos> {
    (0..map.height).flat_map(|y| (0..map.width).map(move |x| Pos::new(x, y))).find(|q| map.at(*q) == Terrain::Throne)
}

fn objective(goal: Goal, map: &Map) -> Objective {
    match goal {
        Goal::Boss => Objective::Boss,
        Goal::Rout => Objective::Rout,
        Goal::Survive(n) => Objective::Survive(n),
        Goal::Seize => Objective::Seize(throne_of(map).expect("seize map needs a throne")),
    }
}

/// Builds the concrete map and placed units for chapter `n`.
pub fn setup(n: usize) -> Setup {
    let def = chapter(n);
    match def.field {
        Field::Fixed { rows, starts, enemies, reinforcements, villages } => {
            let map = Map::from_rows(&rows);
            let objective = objective(def.goal, &map);
            let chests = chest_rewards(n, &map);
            Setup { map, chests, fog: fog_chapter(n), starts, enemies, reinforcements, villages, objective }
        }
        Field::Generated { spec: sp, squads, named, reinforcements, rewards } => {
            let g = mapgen::generate(&sp);
            // Enemy numbers follow the map size.
            let f = match def.tier {
                Tier::Small => 0.6,
                Tier::Medium => 0.85,
                Tier::Large => 1.0,
                Tier::ExtraLarge => 1.25,
            };
            let scaled = |n: usize| ((n as f32 * f).round() as usize).max(1);
            let mut rng = Rng::new(sp.seed ^ 0xA5A5);
            let mut taken: Vec<Pos> = Vec::new();
            let mut enemies = Vec::new();
            for (u, zone) in named {
                let at = place(&g, &sp, zone, u.class, 1, &mut taken, &mut rng);
                let mut u = u;
                u.pos = at.first().copied().unwrap_or(Pos::new(sp.w - 2, sp.h / 2));
                enemies.push(u);
            }
            for sqd in squads {
                for at in place(&g, &sp, sqd.zone, sqd.class, scaled(sqd.count), &mut taken, &mut rng) {
                    enemies.push(Unit::generic(sqd.name, sqd.class, Team::Enemy, sqd.level, sqd.items, at).with_ai(sqd.ai));
                }
            }
            let mut reinf = Vec::new();
            for (turn, sqd) in reinforcements {
                let mut taken_r = taken.clone();
                for at in place(&g, &sp, sqd.zone, sqd.class, scaled(sqd.count), &mut taken_r, &mut rng) {
                    reinf.push((turn, Unit::generic(sqd.name, sqd.class, Team::Enemy, sqd.level, sqd.items, at).with_ai(sqd.ai)));
                }
            }
            let villages = g.villages.iter().copied().zip(rewards).collect();
            let objective = objective(def.goal, &g.map);
            let chests = chest_rewards(n, &g.map);
            Setup { map: g.map, chests, fog: fog_chapter(n), starts: g.starts, enemies, reinforcements: reinf, villages, objective }
        }
    }
}

/// Chapters with fog of war: marsh mist, a blizzard, a night landing.
pub fn fog_chapter(n: usize) -> bool {
    matches!(n, 4 | 11 | 13)
}

/// Rewards for every chest tile on the map, in reading order.
fn chest_rewards(n: usize, map: &Map) -> Vec<(Pos, Reward)> {
    use ItemId::*;
    let pool = [
        Reward::Item(Elixir),
        Reward::Gold(1500 + 250 * n as i32),
        Reward::Item(MasterSeal),
        Reward::Item(if n < 8 { SteelSword } else { SilverSword }),
        Reward::Item(if n < 8 { SteelLance } else { SilverLance }),
        Reward::Item(if n < 8 { Blaze } else { Inferno }),
    ];
    let mut out = Vec::new();
    for y in 0..map.height {
        for x in 0..map.width {
            if map.at(Pos::new(x, y)) == Terrain::Chest {
                out.push((Pos::new(x, y), pool[(n + out.len()) % pool.len()]));
            }
        }
    }
    out
}

pub fn chapter(n: usize) -> ChapterDef {
    use Class::*;
    use ItemId::*;
    use Zone::*;
    let mut d = chapter_base(n);
    // New recruits: cavalry, a thief and a pegasus knight.
    match n {
        2 => {
            d.joins.push(ally("Garrett", Cavalier, 4, s(24, 8, 6, 7, 4, 7, 1), s(80, 45, 40, 45, 35, 30, 15), &[IronLance, IronSword]));
            d.intro.insert(1, l("Garrett", "Garrett, Hollowmere's last rider. My horse and I are yours, my lord. We're fast on open ground, slower in the woods."));
        }
        3 => {
            d.joins.push(ally("Pip", Thief, 3, s(18, 5, 8, 12, 7, 3, 2), s(60, 35, 50, 65, 50, 15, 25), &[IronSword, ChestKey]));
            d.intro.insert(1, l("Pip", "Name's Pip. I can open any lock in that keep, and lift a purse or two off the guards. Thieves move freely in the dark, too."));
        }
        5 => {
            d.joins.push(ally("Skye", PegasusKnight, 5, s(20, 7, 9, 13, 8, 5, 8), s(60, 40, 45, 60, 55, 15, 40), &[IronLance, Javelin]));
            d.intro.insert(1, l("Skye", "Skye, of the cloud riders! My pegasus can fly over these peaks. Just keep me away from their archers. Arrows are our bane."));
        }
        _ => {}
    }
    if fog_chapter(n) {
        d.intro.push(l("", "Fog of war: you can only see enemies near your units. Thieves and fliers see farther."));
    }
    // Enemy cavalry and fliers.
    if let Field::Generated { squads, .. } = &mut d.field {
        let extra: &[Squad] = match n {
            4 => &[sq("Cavalier", Cavalier, 5, &[IronLance], 2, Mid)],
            5 => &[sq("Pegasus Knight", PegasusKnight, 6, &[IronLance], 2, Rear)],
            7 => &[sq("Paladin", Paladin, 2, &[SteelSword], 2, Mid)],
            8 => &[sq("Falcon Knight", FalconKnight, 3, &[SteelLance], 2, Rear)],
            9 => &[sq("Paladin", Paladin, 4, &[SteelLance], 2, Front), sq("Falcon Knight", FalconKnight, 4, &[SteelLance], 2, Mid)],
            11 | 13 => &[sq("Pale Rider", Paladin, 7, &[SilverLance], 2, Mid), sq("Pale Wing", FalconKnight, 7, &[SteelLance], 2, Rear)],
            14 | 15 => &[sq("Pale Rider", Paladin, 10, &[SilverLance], 2, Mid), sq("Pale Wing", FalconKnight, 10, &[SilverLance], 2, Mid)],
            _ => &[],
        };
        squads.extend_from_slice(extra);
    }
    d
}

fn chapter_base(n: usize) -> ChapterDef {
    use Class::*;
    use ItemId::*;
    use Zone::*;
    match n {
        0 => ChapterDef {
            title: "The Ashford Crossing",
            field: Field::Fixed {
                rows: [
                "MMM..FF..~~..MMM",
                "MM...VF..~~...TM",
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
            ],
            starts: vec![p(1, 10), p(2, 10), p(0, 10), p(1, 11), p(0, 11), p(2, 11), p(3, 11), p(0, 9)],
            enemies: vec![
                e("Brigand", Brigand, 2, &[IronAxe], 6, 3),
                e("Brigand", Brigand, 2, &[IronAxe], 6, 7),
                e("Soldier", Soldier, 2, &[IronLance], 11, 6),
                e("Soldier", Soldier, 2, &[IronLance], 11, 2),
                e("Sellsword", Mercenary, 3, &[KeenSword], 13, 10),
                e("Bowman", Archer, 2, &[ShortBow], 12, 7),
                e("Hexer", Mage, 3, &[Gloom], 12, 2),
                Unit::new("Varga", Warlord, Team::Enemy, 5, s(28, 8, 5, 4, 2, 6, 2), Stats::default(), &[Cleaver], p(14, 1)).boss(),
            ],
            reinforcements: vec![],
            villages: vec![(p(5, 1), Reward::Item(Vulnerary))],
            },
            goal: Goal::Boss,
            tier: Tier::Small,
            joins: vec![],
            intro: vec![
                l("", "The realm of Emberfall. For three hundred years the Dawn Flame burned in the capital, and the kingdom knew peace."),
                l("", "Then the Ashen Legion came out of the north. Their king, Morvaine, put out the Flame in a single night."),
                l("", "Kael, heir of the Ashford March, rides south with what is left of his household guard."),
                l("Brenna", "The bridges at Ashford are held, my lord. Raiders flying Legion colours."),
                l("Kael", "Then we take them back. If the March falls, there is nothing between Morvaine and the southern villages."),
                l("Varga", "Hah! A boy and a handful of guards? Bring me his head, and the bounty is yours!"),
                l("Liese", "Stay near me and I can keep you standing. Please don't do anything reckless."),
            ],
            outro: vec![
                l("Varga", "Morvaine... will burn... you all..."),
                l("Orrin", "His orders bear the Legion seal. They want the old roads south kept open."),
                l("Kael", "Then we close them. We make for the Ember Woods and the free villages beyond."),
            ],
            talks: vec![],
            reward_gold: 1200,
        },
        1 => ChapterDef {
            title: "Ember Woods",
            field: Field::Fixed {
                rows: [
                "FF..FFF...FF..FF",
                "F...F.....F...MF",
                "..FF..V..FFF....",
                ".F...FF.......F.",
                "...F....FF..F...",
                "..F..M....F...FF",
                ".....MM.F...F...",
                "..F......F..FF..",
                ".FF..F.......V..",
                "....FFF..FF.....",
                "F.......F...F.FF",
                "FF..F..FF....FFF",
            ],
            starts: vec![p(1, 5), p(0, 5), p(1, 6), p(0, 6), p(1, 4), p(0, 4), p(1, 7), p(0, 7)],
            enemies: vec![
                e("Brigand", Brigand, 3, &[IronAxe], 5, 3),
                e("Brigand", Brigand, 3, &[IronAxe], 6, 7),
                e("Brigand", Brigand, 4, &[HandAxe], 9, 4),
                e("Archer", Archer, 3, &[ShortBow], 8, 6),
                Unit::new("Dain", Mercenary, Team::Enemy, 5, s(23, 7, 11, 10, 4, 5, 1), s(80, 40, 55, 55, 30, 25, 15), &[SteelSword], p(7, 5)).talkable(),
                e("Brigand", Brigand, 4, &[IronAxe], 12, 6),
                e("Brigand", Brigand, 3, &[IronAxe], 13, 3),
                e("Hexer", Mage, 3, &[Gloom], 14, 2),
                Unit::generic("Grell", Fighter, Team::Enemy, 7, &[SteelAxe, HandAxe], p(14, 7)).boss(),
            ],
            reinforcements: vec![(4, e("Brigand", Brigand, 4, &[IronAxe], 15, 10)), (4, e("Brigand", Brigand, 4, &[IronAxe], 15, 0))],
            villages: vec![(p(6, 2), Reward::Item(Elixir)), (p(13, 8), Reward::Gold(1000))],
            },
            goal: Goal::Rout,
            tier: Tier::Small,
            joins: vec![ally("Holt", Fighter, 4, s(28, 9, 5, 6, 3, 4, 0), s(90, 60, 35, 35, 30, 25, 5), &[IronAxe, HandAxe])],
            intro: vec![
                l("", "The Ember Woods. Bandit gangs have grown bold since the Flame went out."),
                l("Holt", "You're the Ashford lad? I'm Holt. The brigands burned my mill, and I'd like to return the favour."),
                l("Kael", "Glad to have you. What are we up against?"),
                l("Holt", "Grell's gang. And a hired sword they brought in from the coast, name of Dain. He doesn't look happy about the company he's keeping."),
                l("Tamsin", "If he's only in it for the coin, maybe he can be talked round."),
                l("", "Hint: move Kael next to Dain and choose Talk."),
            ],
            outro: vec![
                l("Holt", "The woods will breathe easier tonight."),
                l("Kael", "Scouts say the Legion is marching on Hollowmere. The village has no walls to speak of."),
                l("Brenna", "Then we'll be its wall."),
            ],
            talks: vec![(
                "Dain",
                vec![
                    l("Kael", "You don't fight like a bandit. Why are you with them?"),
                    l("Dain", "Grell paid in advance. Then he started burning farms with children still inside."),
                    l("Kael", "Then stop taking his gold. Fight with us. We pay worse, but we sleep better."),
                    l("Dain", "...Ha. Fair enough. My sword is yours, Ashford."),
                ],
            )],
            reward_gold: 1500,
        },
        2 => ChapterDef {
            title: "Hollowmere Must Stand",
            field: Field::Fixed {
                rows: [
                "MM..FF....FF..MM",
                "M......FF......M",
                "..F..........F..",
                "....V..##..V....",
                ".F....#,,#....F.",
                "......#,,#......",
                "..F...##G##..F..",
                "......T..T......",
                ".V..F......F..V.",
                "....F..T.....F..",
                "M.......FF.....M",
                "MM....FFFF....MM",
            ],
            starts: vec![p(7, 7), p(8, 7), p(6, 7), p(9, 7), p(7, 8), p(8, 8), p(6, 8), p(9, 8), p(7, 9), p(8, 9)],
            enemies: vec![
                e("Brigand", Brigand, 4, &[IronAxe], 2, 1),
                e("Brigand", Brigand, 4, &[SteelAxe], 13, 1),
                e("Fighter", Fighter, 3, &[IronAxe], 7, 1),
                e("Soldier", Soldier, 4, &[IronLance], 4, 0),
                e("Soldier", Soldier, 4, &[Javelin], 11, 0),
                e("Archer", Archer, 3, &[ShortBow], 1, 2),
                e("Archer", Archer, 3, &[ShortBow], 14, 2),
                e("Knight", Knight, 4, &[IronLance], 8, 2),
                Unit::generic("Sable", Mercenary, Team::Enemy, 6, &[SteelSword], p(8, 0)),
            ],
            reinforcements: vec![
                (2, e("Brigand", Brigand, 3, &[IronAxe], 0, 5)),
                (2, e("Brigand", Brigand, 3, &[IronAxe], 15, 5)),
                (3, e("Soldier", Soldier, 4, &[IronLance], 0, 9)),
                (3, e("Soldier", Soldier, 4, &[IronLance], 15, 9)),
                (4, e("Fighter", Fighter, 4, &[HandAxe], 7, 11)),
                (4, e("Mage", Mage, 4, &[Ember], 8, 11)),
                (5, e("Mercenary", Mercenary, 5, &[IronSword], 0, 2)),
                (5, e("Mercenary", Mercenary, 5, &[IronSword], 15, 2)),
            ],
            villages: vec![
                (p(4, 3), Reward::Item(Vulnerary)),
                (p(11, 3), Reward::Item(SteelLance)),
                (p(1, 8), Reward::Gold(1500)),
                (p(14, 8), Reward::Item(Elixir)),
            ],
            },
            goal: Goal::Survive(6),
            tier: Tier::Small,
            joins: vec![ally("Sera", Knight, 5, s(22, 8, 5, 3, 3, 12, 2), s(85, 45, 35, 25, 35, 45, 15), &[IronLance, Javelin])],
            intro: vec![
                l("", "Hollowmere. A village of farmers and one tired old keep."),
                l("Sera", "Lord Kael! Sera, last knight of the Hollowmere watch. The Legion's vanguard will be here by dusk."),
                l("Kael", "How long until the villagers are clear of the valley?"),
                l("Sera", "Six turns of the watch, perhaps. They're slow, there are children and carts..."),
                l("Kael", "Then we hold for six. Everyone, dig in around the keep. Nobody gets past."),
                l("Sable", "Burn the village. Leave nothing the boy can use."),
            ],
            outro: vec![
                l("Sera", "The last cart is over the ridge. They're safe... because of you."),
                l("Orrin", "Look at these maps the Legion left behind. Saltmarsh Keep is their staging fort."),
                l("Kael", "If we take Saltmarsh, we cut their supply line to the south. We march at dawn."),
            ],
            talks: vec![],
            reward_gold: 2000,
        },
        3 => ChapterDef {
            title: "Saltmarsh Keep",
            field: Field::Fixed {
                rows: [
                "~~~~....FF.#####",
                "~~~...F....#,,H#",
                "~~.......F.#CP,#",
                "~...FF.....,,,,#",
                "~.......T..#,,,#",
                "....F......##G##",
                "......FF........",
                "..F.......F.....",
                "~.....T.....FF..",
                "~~.F.....F......",
                "~~~.....V.....F.",
                "~~~~..FF....F...",
            ],
            starts: vec![p(4, 10), p(5, 10), p(4, 11), p(5, 11), p(3, 10), p(6, 10), p(3, 9), p(2, 9), p(7, 10), p(6, 11)],
            enemies: vec![
                e("Soldier", Soldier, 5, &[SteelLance], 12, 3),
                e("Soldier", Soldier, 5, &[IronLance], 13, 4),
                e("Mage", Mage, 5, &[Ember], 12, 1),
                e("Archer", Archer, 5, &[SteelBow], 13, 2),
                e("Fighter", Fighter, 5, &[SteelAxe], 8, 4),
                e("Brigand", Brigand, 5, &[HandAxe], 5, 3),
                e("Mercenary", Mercenary, 5, &[IronSword], 9, 7),
                e("Mercenary", Mercenary, 4, &[IronSword], 10, 7),
                e("Archer", Archer, 5, &[ShortBow], 12, 8),
                Unit::new("Ilse", Mage, Team::Enemy, 6, s(20, 9, 8, 8, 4, 3, 8), s(55, 60, 50, 50, 35, 15, 50), &[Ember, Blaze], p(10, 2)).talkable(),
                Unit::generic("Hask", Knight, Team::Enemy, 8, &[SteelLance, Javelin], p(14, 1)).boss(),
            ],
            reinforcements: vec![
                (5, e("Soldier", Soldier, 5, &[IronLance], 15, 6)),
                (5, e("Soldier", Soldier, 5, &[IronLance], 15, 7)),
                (7, e("Knight", Knight, 5, &[IronLance], 15, 6)),
            ],
            villages: vec![(p(8, 10), Reward::Item(MasterSeal))],
            },
            goal: Goal::Seize,
            tier: Tier::Small,
            joins: vec![],
            intro: vec![
                l("", "Saltmarsh Keep, a grey fortress above the tidal flats. Its stores feed the Legion's whole southern army."),
                l("Kael", "If I take the throne room, the garrison will lose heart."),
                l("Dain", "Their commander's Hask. Thick armour, thick head. Hit him with magic."),
                l("Orrin", "There's a mage in their ranks... I know her. Ilse. We studied together at the Dawn College."),
                l("Kael", "Then maybe she'll listen to us."),
                l("", "Objective: Kael must stand on the throne and choose Seize."),
            ],
            outro: vec![
                l("Kael", "Saltmarsh is ours. Raise the March banner over the gate."),
                l("Ilse", "The Legion shipped prisoners out of here yesterday. They're marching them north along the Drowned Road."),
                l("Liese", "There's a Master Seal in the keep's treasury. A seasoned warrior could grow stronger with it."),
            ],
            talks: vec![(
                "Ilse",
                vec![
                    l("Kael", "Ilse? Orrin says you were the best student the Dawn College ever had."),
                    l("Ilse", "Then Orrin still lies badly. The Legion took the College. I serve or my teachers hang."),
                    l("Kael", "We'll free them. You have my word, as heir of Ashford."),
                    l("Ilse", "...Words are cheap. But yours sound true. Fine. I'm with you."),
                ],
            )],
            reward_gold: 2500,
        },
        4 => ChapterDef {
            title: "The Drowned Road",
            tier: Tier::Medium,
            goal: Goal::Boss,
            field: Field::Generated {
                spec: spec(Tier::Medium, 501, Theme::Marsh, Layout::March, false, false, 3, 2),
                squads: vec![
                    sq("Brigand", Brigand, 5, &[IronAxe], 4, Front),
                    sq("Soldier", Soldier, 5, &[IronLance], 3, Mid),
                    sq("Archer", Archer, 5, &[ShortBow], 2, Mid),
                    guard("Mage", Mage, 5, &[Ember], 2, Rear),
                    guard("Fighter", Fighter, 6, &[SteelAxe], 2, Rear),
                ],
                named: vec![(boss("Oskar", Fighter, 9, &[SteelAxe, HandAxe]), Rear)],
                reinforcements: vec![(4, sq("Brigand", Brigand, 5, &[IronAxe], 2, Edge)), (6, sq("Brigand", Brigand, 6, &[HandAxe], 2, Edge))],
                rewards: vec![Reward::Item(Vulnerary), Reward::Gold(1500), Reward::Item(SteelSword)],
            },
            joins: vec![ally("Wren", Cleric, 5, s(19, 7, 6, 10, 8, 3, 9), s(55, 50, 35, 60, 55, 15, 50), &[Mend, Vulnerary])],
            intro: vec![
                l("", "South of Saltmarsh the old causeway sinks into the fens. The Legion uses it to march prisoners north."),
                l("Wren", "Please! You're the March lord? They took everyone from my village. They're dragging them up the Drowned Road!"),
                l("Kael", "Then we'll get them back. Can you fight?"),
                l("Wren", "I trained at the Dawn Chapel with Liese. I can heal, and I run very fast."),
                l("Liese", "Wren! Thank the Flame you're safe. Stay close to me."),
                l("Oskar", "Prisoners fetch good silver in the north. Nobody touches the cargo."),
            ],
            outro: vec![
                l("", "The prisoners are freed. Among them is an old monk, half frozen. Before he faints, he whispers one word: 'Vigil'."),
                l("Orrin", "The Vigil Monastery, high in the Ironpeaks. If the Legion went there, the monks are in danger."),
                l("Kael", "Then we take the pass."),
            ],
            talks: vec![],
            reward_gold: 2800,
        },
        5 => ChapterDef {
            title: "Ironpeak Pass",
            tier: Tier::Small,
            goal: Goal::Boss,
            field: Field::Generated {
                spec: spec(Tier::Small, 602, Theme::Highlands, Layout::March, false, false, 2, 3),
                squads: vec![
                    sq("Fighter", Fighter, 7, &[SteelAxe], 3, Front),
                    sq("Mercenary", Mercenary, 7, &[SteelSword], 3, Front),
                    sq("Archer", Archer, 7, &[SteelBow], 3, Mid),
                    sq("Knight", Knight, 7, &[SteelLance], 2, Mid),
                    guard("Mage", Mage, 7, &[Blaze], 2, Rear),
                    guard("Brigand", Brigand, 8, &[SteelAxe], 2, Rear),
                ],
                named: vec![(boss("Thorne", Warlord, 2, &[SilverAxe, HandAxe]), Rear)],
                reinforcements: vec![
                    (3, sq("Fighter", Fighter, 7, &[IronAxe], 2, Edge)),
                    (3, sq("Mage", Mage, 7, &[Ember], 1, Edge)),
                    (6, sq("Archer", Archer, 7, &[SteelBow], 2, Edge)),
                ],
                rewards: vec![Reward::Item(SilverLance), Reward::Item(Elixir)],
            },
            joins: vec![ally("Maeve", Lancer, 7, s(26, 9, 8, 10, 6, 8, 3), s(80, 45, 45, 55, 45, 30, 20), &[SteelLance, Javelin])],
            intro: vec![
                l("", "The Ironpeak Pass. A narrow road through frozen mountains, and the only way to the Vigil Monastery."),
                l("Maeve", "You must be the March lord everyone's whispering about. Maeve. I guided caravans through here until Thorne arrived."),
                l("Kael", "Will you guide an army?"),
                l("Maeve", "If that army is going to knock Thorne off his mountain, gladly."),
                l("Holt", "Axe-folk move well on the peaks. Watch your flanks."),
            ],
            outro: vec![
                l("Thorne", "The Legion... is endless..."),
                l("Maeve", "The pass is open. Listen. The monastery bells have stopped ringing."),
                l("Kael", "Then we climb, and fast."),
            ],
            talks: vec![],
            reward_gold: 3000,
        },
        6 => ChapterDef {
            title: "The Frozen Vigil",
            tier: Tier::Medium,
            goal: Goal::Seize,
            field: Field::Generated {
                spec: spec(Tier::Medium, 703, Theme::Ruins, Layout::March, false, true, 2, 2),
                squads: vec![
                    sq("Soldier", Soldier, 8, &[SteelLance], 3, Front),
                    sq("Mercenary", Mercenary, 8, &[SteelSword], 2, Front),
                    sq("Fighter", Fighter, 8, &[SteelAxe], 2, Mid),
                    sq("Archer", Archer, 8, &[SteelBow], 2, Mid),
                    guard("Knight", Knight, 8, &[SteelLance], 2, Castle),
                    guard("Mage", Mage, 8, &[Blaze], 2, Castle),
                ],
                named: vec![(boss("Vorn", Halberdier, 3, &[SilverLance, Javelin]), Throne)],
                reinforcements: vec![(5, sq("Soldier", Soldier, 8, &[IronLance], 3, Edge))],
                rewards: vec![Reward::Item(MasterSeal), Reward::Gold(2000)],
            },
            joins: vec![ally("Aldous", Bishop, 3, s(26, 10, 8, 9, 8, 5, 14), s(45, 50, 35, 40, 45, 15, 55), &[Mend, Ember])],
            intro: vec![
                l("", "The Vigil Monastery clings to the mountainside. For a thousand years its monks have kept the histories of Emberfall."),
                l("Aldous", "Lord Kael... I am Aldous, last abbot of the Vigil. The Legion holds the chapel, and they are burning our books."),
                l("Kael", "What could Morvaine want with old books?"),
                l("Aldous", "Not the books. What is written in them. Take back the chapel and I will show you."),
                l("", "Objective: Kael must stand on the chapel altar (the throne) and choose Seize."),
            ],
            outro: vec![
                l("Aldous", "Here. The oldest chronicle. It says the Dawn Flame was never just a symbol of kings. It was a seal."),
                l("Ilse", "A seal? Against what?"),
                l("Aldous", "Against something in the far north. The rest of the page is torn out. I fear Morvaine tore it."),
                l("Kael", "Then we'll ask Morvaine. The Ashen Wall is all that stands between us and the capital."),
            ],
            talks: vec![],
            reward_gold: 3200,
        },
        7 => ChapterDef {
            title: "The Ashen Wall",
            tier: Tier::Large,
            goal: Goal::Rout,
            field: Field::Generated {
                spec: spec(Tier::Large, 804, Theme::Grass, Layout::March, true, false, 2, 3),
                squads: vec![
                    sq("Warrior", Warrior, 2, &[SteelAxe], 3, Front),
                    sq("Halberdier", Halberdier, 2, &[SteelLance], 3, Front),
                    sq("Hero", Hero, 1, &[SteelSword], 2, Mid),
                    sq("General", General, 2, &[SteelLance], 3, Mid),
                    sq("Sniper", Sniper, 2, &[SteelBow], 2, Rear),
                    sq("Sage", Sage, 2, &[Blaze], 2, Rear),
                ],
                named: vec![(boss("Vex", Hero, 5, &[SilverSword]), Rear)],
                reinforcements: vec![(3, sq("Hero", Hero, 1, &[SteelSword], 2, Edge)), (5, sq("Warrior", Warrior, 2, &[SteelAxe], 1, Edge)), (5, sq("Sage", Sage, 2, &[Ember], 1, Edge))],
                rewards: vec![Reward::Item(SilverSword), Reward::Item(Elixir)],
            },
            joins: vec![ally("Bram", General, 3, s(30, 12, 9, 5, 4, 16, 5), s(70, 35, 30, 15, 25, 35, 15), &[SteelLance, SteelAxe])],
            intro: vec![
                l("", "The Ashen Wall. Black stone and a frozen moat, raised in a single season by Morvaine's sorcery."),
                l("Bram", "Ashford's boy, grown up. I served your grandfather. The name's Bram. The Legion forgot to finish me off."),
                l("Kael", "General Bram! Will you stand with us?"),
                l("Bram", "Stand? Lad, I'll stand in front of you. That's what armour is for."),
                l("Vex", "So this is the little lord. Kill every one of them. Leave none to sing about it."),
            ],
            outro: vec![
                l("Vex", "Morvaine... promised me... a kingdom..."),
                l("Bram", "Past this wall runs the Cinder River. He'll try to burn the bridge with us on it."),
                l("Kael", "Let him try."),
            ],
            talks: vec![],
            reward_gold: 3500,
        },
        8 => ChapterDef {
            title: "The Burning Bridge",
            tier: Tier::Large,
            goal: Goal::Survive(7),
            field: Field::Generated {
                spec: spec(Tier::Large, 905, Theme::Woods, Layout::March, true, false, 2, 4),
                squads: vec![
                    sq("Warrior", Warrior, 3, &[SteelAxe], 3, Front),
                    sq("Hero", Hero, 3, &[SteelSword], 3, Mid),
                    sq("Halberdier", Halberdier, 3, &[SteelLance], 3, Mid),
                    sq("Sage", Sage, 3, &[Blaze], 2, Mid),
                    sq("Sniper", Sniper, 3, &[Longbow], 2, Rear),
                ],
                named: vec![
                    (Unit::new("Corvin", Sniper, Team::Enemy, 4, s(27, 10, 13, 12, 5, 7, 5), s(70, 45, 55, 50, 30, 20, 20), &[SteelBow, Longbow], p(0, 0)).talkable(), Front),
                    (Unit::generic("Ruk", Warrior, Team::Enemy, 6, &[SilverAxe], p(0, 0)), Mid),
                ],
                reinforcements: vec![
                    (2, sq("Hero", Hero, 2, &[SteelSword], 2, Edge)),
                    (3, sq("General", General, 3, &[SteelLance], 2, Edge)),
                    (4, sq("Sage", Sage, 3, &[Blaze], 2, Edge)),
                    (5, sq("Warrior", Warrior, 3, &[SteelAxe], 3, Edge)),
                    (6, sq("Halberdier", Halberdier, 4, &[SteelLance], 2, Edge)),
                ],
                rewards: vec![Reward::Item(Renew), Reward::Gold(2500)],
            },
            joins: vec![],
            intro: vec![
                l("", "The Cinder River. Stone bridges, and on the far bank the whole strength of the Legion's field army."),
                l("Ruk", "Hold them at the river! When the sorcerers finish the ritual, every bridge burns!"),
                l("Ilse", "The ritual needs seven turns. If we survive that long, it collapses and their army is stranded."),
                l("Kael", "Then we hold. Bram, Sera, plug the bridges. Archers and mages behind them."),
                l("Tamsin", "There's a Legion sniper out there who keeps missing on purpose. I think he wants out."),
                l("", "Hint: Corvin can be recruited if Kael talks to him."),
            ],
            outro: vec![
                l("", "The ritual collapses. On the far bank the Legion's banners waver, then scatter."),
                l("Corvin", "Dawnhold lies beyond. Marshal Kreth holds the outer city for Morvaine."),
                l("Kael", "Then we take the capital back, street by street."),
            ],
            talks: vec![(
                "Corvin",
                vec![
                    l("Kael", "You've been firing wide all day. Why?"),
                    l("Corvin", "Because I'm from Hollowmere. You saved my mother and sisters. I won't shoot their saviour."),
                    l("Kael", "Then come home. Fight for your village instead of against it."),
                    l("Corvin", "Thought you'd never ask."),
                ],
            )],
            reward_gold: 4000,
        },
        9 => ChapterDef {
            title: "Siege of Dawnhold",
            tier: Tier::ExtraLarge,
            goal: Goal::Boss,
            field: Field::Generated {
                spec: spec(Tier::ExtraLarge, 1006, Theme::Ruins, Layout::March, false, true, 3, 3),
                squads: vec![
                    sq("Halberdier", Halberdier, 4, &[SteelLance], 3, Front),
                    sq("Hero", Hero, 4, &[SteelSword], 3, Front),
                    sq("Warrior", Warrior, 4, &[SteelAxe], 2, Mid),
                    sq("Sniper", Sniper, 4, &[SteelBow], 3, Mid),
                    sq("Sage", Sage, 4, &[Blaze], 2, Mid),
                    guard("General", General, 5, &[SilverLance], 3, Castle),
                    guard("Sage", Sage, 5, &[Inferno], 1, Castle),
                ],
                named: vec![
                    (boss("Kreth", General, 7, &[SilverLance, SteelAxe]), Throne),
                    (Unit::new("Rhea", Hero, Team::Enemy, 4, s(31, 11, 13, 13, 7, 9, 5), s(75, 45, 50, 50, 40, 30, 20), &[SilverSword, HandAxe], p(0, 0)).talkable(), Front),
                ],
                reinforcements: vec![(4, sq("Hero", Hero, 4, &[SteelSword], 2, Edge)), (6, sq("General", General, 4, &[SteelLance], 2, Edge))],
                rewards: vec![Reward::Item(MasterSeal), Reward::Gold(3000), Reward::Item(Elixir)],
            },
            joins: vec![],
            intro: vec![
                l("", "Dawnhold, the capital. Its outer district is a maze of broken walls held by the Legion's best."),
                l("Bram", "Marshal Kreth commands here. A cold one. He hanged the city council from the gatehouse."),
                l("Corvin", "There's a captain on the barricades, Rhea. She led the old city guard. They're holding her children."),
                l("Kael", "Then we give her a reason to change sides. Talk first, fight second."),
            ],
            outro: vec![
                l("Kreth", "Fools... the Sovereign is coming... Morvaine was only the herald..."),
                l("Kael", "The Sovereign?"),
                l("Aldous", "The torn page. We need to hear the rest from Morvaine himself."),
            ],
            talks: vec![(
                "Rhea",
                vec![
                    l("Kael", "Captain Rhea. Your city needs its guard back."),
                    l("Rhea", "My children are in Kreth's dungeons. If I lower my sword, they die."),
                    l("Kael", "Maeve and Dain are already in the dungeons. Your children will be free before sunset."),
                    l("Rhea", "...Then my blade belongs to the March. Let's take back Dawnhold."),
                ],
            )],
            reward_gold: 4500,
        },
        10 => ChapterDef {
            title: "Throne of Ash",
            tier: Tier::Medium,
            goal: Goal::Boss,
            field: Field::Generated {
                spec: spec(Tier::Medium, 1107, Theme::Fortress, Layout::March, false, true, 0, 0),
                squads: vec![
                    sq("Hero", Hero, 6, &[SilverSword], 3, Front),
                    sq("Warrior", Warrior, 6, &[SilverAxe], 2, Front),
                    sq("Halberdier", Halberdier, 6, &[SilverLance], 3, Mid),
                    sq("Sniper", Sniper, 6, &[Longbow], 2, Mid),
                    sq("Sage", Sage, 6, &[Inferno], 2, Rear),
                    guard("General", General, 6, &[SilverLance], 3, Rear),
                ],
                named: vec![(Unit::new("Morvaine", Sorcerer, Team::Enemy, 10, s(42, 15, 13, 10, 6, 10, 14), Stats::default(), &[Umbra], p(0, 0)).boss(), Throne)],
                reinforcements: vec![(3, sq("Hero", Hero, 5, &[SteelSword], 2, Edge)), (5, sq("Sage", Sage, 5, &[Blaze], 2, Edge))],
                rewards: vec![],
            },
            joins: vec![],
            intro: vec![
                l("", "The Ember Throne. Ash lies on the floors like snow. In the great hall, the brazier of the Dawn Flame stands cold."),
                l("Morvaine", "Little lord of Ashford. You've come a long way just to die in the dark."),
                l("Kael", "The Flame isn't dead, Morvaine. It lives in every village that stood when you said it would kneel."),
                l("Morvaine", "Pretty words. Let's see how they burn."),
                l("", "Defeat Morvaine."),
            ],
            outro: vec![
                l("Morvaine", "You think... I wanted this? I only... opened the door..."),
                l("Kael", "What door, Morvaine?"),
                l("Morvaine", "The Flame was a lock. I put it out. Now Vaelith, the Pale Sovereign, will walk south. Laugh, boy. You've won nothing."),
                l("", "Morvaine is dead, but the brazier stays cold. From the north, a bitter wind begins to blow."),
                l("Aldous", "If the Flame was a seal, it must be relit. The chronicle speaks of an Ember Heart, kept in a vault beyond the Frostmarch."),
                l("Kael", "Then we go north. All of us."),
                l("", "END OF ACT I"),
            ],
            talks: vec![],
            reward_gold: 6000,
        },
        11 => ChapterDef {
            title: "The Frostmarch",
            tier: Tier::Large,
            goal: Goal::Survive(8),
            field: Field::Generated {
                spec: spec(Tier::Large, 1208, Theme::Highlands, Layout::Defend, false, false, 4, 4),
                squads: vec![
                    sq("Pale Axe", Warrior, 6, &[SteelAxe], 2, Front),
                    sq("Pale Blade", Hero, 6, &[SteelSword], 2, Front),
                    sq("Pale Knight", General, 6, &[SteelLance], 3, Mid),
                    sq("Pale Blade", Hero, 6, &[SteelSword], 4, Mid),
                    sq("Pale Archer", Sniper, 6, &[SteelBow], 3, Rear),
                    sq("Pale Seer", Sage, 6, &[Blaze], 2, Rear),
                ],
                named: vec![],
                reinforcements: vec![
                    (2, sq("Pale Blade", Hero, 6, &[SteelSword], 3, Edge)),
                    (3, sq("Pale Axe", Warrior, 6, &[SteelAxe], 3, Edge)),
                    (4, sq("Pale Seer", Sage, 6, &[Blaze], 2, Edge)),
                    (5, sq("Pale Knight", General, 7, &[SteelLance], 3, Edge)),
                    (6, sq("Pale Blade", Hero, 7, &[SilverSword], 3, Edge)),
                    (7, sq("Pale Archer", Sniper, 7, &[Longbow], 2, Edge)),
                ],
                rewards: vec![Reward::Item(Elixir), Reward::Item(MasterSeal), Reward::Gold(3000), Reward::Item(SilverAxe)],
            },
            joins: vec![ally("Toma", Warrior, 6, s(40, 15, 10, 10, 5, 10, 3), s(85, 55, 35, 35, 30, 30, 10), &[SilverAxe, SteelBow])],
            intro: vec![
                l("", "ACT II - The Frostmarch. A frozen moor where the old roads vanish under the snow."),
                l("Toma", "Southerners! You'll freeze out here. I'm Toma, of the moor clans. Something is hunting my people. Pale things in armour."),
                l("Kael", "The Sovereign's army. How long until this storm passes?"),
                l("Toma", "Eight turns of the watch, if the spirits are kind. Make a ring around the old forts and hold."),
                l("Bram", "They don't bleed, and they don't run. Stay in formation."),
            ],
            outro: vec![
                l("Toma", "You held. The clans will remember this."),
                l("", "The storm breaks. Beyond the moor, half buried in ice, the Vault of Embers waits."),
            ],
            talks: vec![],
            reward_gold: 5000,
        },
        12 => ChapterDef {
            title: "Vault of Embers",
            tier: Tier::Medium,
            goal: Goal::Seize,
            field: Field::Generated {
                spec: spec(Tier::Medium, 1309, Theme::Ruins, Layout::March, false, true, 3, 3),
                squads: vec![
                    sq("Pale Blade", Hero, 7, &[SilverSword], 3, Front),
                    sq("Pale Axe", Warrior, 7, &[SteelAxe], 2, Front),
                    sq("Pale Knight", General, 7, &[SilverLance], 3, Mid),
                    sq("Pale Archer", Sniper, 7, &[Longbow], 2, Mid),
                    sq("Pale Seer", Sage, 7, &[Blaze], 3, Mid),
                    guard("Pale Knight", General, 8, &[SilverLance], 2, Castle),
                    guard("Pale Seer", Sage, 8, &[Inferno], 2, Castle),
                ],
                named: vec![
                    (boss("Grimhal", Warlord, 8, &[SilverAxe, HandAxe]), Throne),
                    (Unit::new("Seren", Sage, Team::Enemy, 8, s(30, 15, 14, 13, 8, 7, 14), s(55, 55, 45, 45, 35, 15, 45), &[Inferno, Mend], p(0, 0)).talkable(), Mid),
                ],
                reinforcements: vec![(4, sq("Pale Blade", Hero, 7, &[SteelSword], 3, Edge)), (7, sq("Pale Knight", General, 8, &[SteelLance], 3, Edge))],
                rewards: vec![Reward::Item(SilverLance), Reward::Gold(4000), Reward::Item(Elixir)],
            },
            joins: vec![],
            intro: vec![
                l("", "The Vault of Embers. Built by the first kings to guard the Ember Heart, the spark that once lit the Dawn Flame."),
                l("Seren", "Turn back. No one has entered the Vault in four hundred years. I am its keeper, and I will not fail now."),
                l("Kael", "The Sovereign's army is already inside. If they take the Heart, no one can ever relight the Flame."),
                l("Seren", "Pretty words. Thieves always have them."),
                l("", "Hint: Kael can talk to Seren. Objective: seize the inner sanctum."),
            ],
            outro: vec![
                l("", "In the sanctum, the Ember Heart pulses like a living coal."),
                l("Seren", "Vaelith will feel it wake. He will send everything he has to stop us."),
                l("Kael", "Then we meet him at the coast. The fishermen of Greyhaven can carry us to his citadel."),
            ],
            talks: vec![(
                "Seren",
                vec![
                    l("Kael", "I'm not here to steal the Heart. I'm here to carry it home and light the Flame again."),
                    l("Seren", "...You bear the Ashford crest. The founders' line. The chronicles said one of you would come."),
                    l("Seren", "Very well. The keeper goes wherever the Heart goes."),
                ],
            )],
            reward_gold: 5000,
        },
        13 => ChapterDef {
            title: "The Shattered Coast",
            tier: Tier::ExtraLarge,
            goal: Goal::Rout,
            field: Field::Generated {
                spec: spec(Tier::ExtraLarge, 1410, Theme::Marsh, Layout::March, true, false, 4, 4),
                squads: vec![
                    sq("Pale Blade", Hero, 8, &[SilverSword], 4, Front),
                    sq("Pale Axe", Warrior, 8, &[SilverAxe], 4, Front),
                    sq("Pale Knight", General, 8, &[SilverLance], 3, Mid),
                    sq("Pale Archer", Sniper, 8, &[Longbow], 4, Mid),
                    sq("Pale Seer", Sage, 8, &[Inferno], 3, Rear),
                    guard("Pale Lancer", Halberdier, 8, &[SilverLance], 4, Rear),
                ],
                named: vec![(boss("Captain Hrolf", Warrior, 10, &[SilverAxe, HandAxe]), Rear)],
                reinforcements: vec![(3, sq("Pale Blade", Hero, 8, &[SteelSword], 3, Edge)), (6, sq("Pale Seer", Sage, 8, &[Blaze], 2, Edge))],
                rewards: vec![Reward::Item(MasterSeal), Reward::Item(Elixir), Reward::Gold(5000), Reward::Item(SilverSword)],
            },
            joins: vec![],
            intro: vec![
                l("", "Greyhaven was a fishing port. Now ships of black ice crowd its harbour: the Pale Fleet."),
                l("Maeve", "Clear the docks and the fishermen will give us their boats."),
                l("Rhea", "Every last one of them, then. No retreat."),
                l("Hrolf", "The Heart-thief comes! Break the southerners on the shore!"),
            ],
            outro: vec![
                l("", "The harbour is free. Fishermen raise their sails, and the army of the March sets out across the grey sea."),
                l("Kael", "Vaelith's citadel. The Hollow Bastion. One more wall."),
            ],
            talks: vec![],
            reward_gold: 6000,
        },
        14 => ChapterDef {
            title: "The Hollow Bastion",
            tier: Tier::Large,
            goal: Goal::Boss,
            field: Field::Generated {
                spec: spec(Tier::Large, 1511, Theme::Fortress, Layout::March, false, true, 0, 0),
                squads: vec![
                    sq("Pale Blade", Hero, 9, &[SilverSword], 4, Front),
                    sq("Pale Axe", Warrior, 9, &[SilverAxe], 4, Front),
                    sq("Pale Lancer", Halberdier, 9, &[SilverLance], 4, Mid),
                    sq("Pale Archer", Sniper, 9, &[Longbow], 3, Mid),
                    sq("Pale Seer", Sage, 9, &[Inferno], 3, Rear),
                    guard("Pale Knight", General, 10, &[SilverLance], 4, Rear),
                ],
                named: vec![(boss("Ysolde", General, 12, &[SilverLance, SilverAxe]), Throne)],
                reinforcements: vec![(3, sq("Pale Blade", Hero, 9, &[SilverSword], 3, Edge)), (5, sq("Pale Seer", Sage, 9, &[Inferno], 2, Edge))],
                rewards: vec![],
            },
            joins: vec![],
            intro: vec![
                l("Ysolde", "I was a knight of Emberfall once, before the cold took my heart. Turn back, heir of Ashford."),
                l("Sera", "Ysolde? The Hollowmere histories speak of you. The knight who never came back from the north."),
                l("Ysolde", "I came back. I simply forgot why I ever left."),
                l("Kael", "Then let us remind you."),
            ],
            outro: vec![
                l("Ysolde", "Warm... I remember now... Finish it, Ashford. Free us all."),
                l("Seren", "The Heart is burning hotter. Vaelith's throne lies just beyond these halls."),
            ],
            talks: vec![],
            reward_gold: 6000,
        },
        _ => ChapterDef {
            title: "Heart of Winter",
            tier: Tier::ExtraLarge,
            goal: Goal::Boss,
            field: Field::Generated {
                spec: spec(Tier::ExtraLarge, 1612, Theme::Fortress, Layout::March, false, true, 0, 0),
                squads: vec![
                    sq("Pale Blade", Hero, 11, &[SilverSword], 5, Front),
                    sq("Pale Axe", Warrior, 11, &[SilverAxe], 4, Front),
                    sq("Pale Lancer", Halberdier, 11, &[SilverLance], 4, Mid),
                    sq("Pale Archer", Sniper, 11, &[Longbow], 4, Mid),
                    sq("Pale Seer", Sage, 11, &[Inferno], 4, Rear),
                    guard("Pale Knight", General, 12, &[SilverLance], 4, Rear),
                ],
                named: vec![(Unit::new("Vaelith", Sorcerer, Team::Enemy, 20, s(55, 19, 16, 13, 10, 13, 18), Stats::default(), &[Umbra], p(0, 0)).boss(), Throne)],
                reinforcements: vec![(3, sq("Pale Blade", Hero, 10, &[SilverSword], 3, Edge)), (5, sq("Pale Seer", Sage, 10, &[Inferno], 3, Edge)), (7, sq("Pale Knight", General, 11, &[SilverLance], 3, Edge))],
                rewards: vec![],
            },
            joins: vec![],
            intro: vec![
                l("", "At the heart of the Bastion, under a sky of frozen stars, the Pale Sovereign waits."),
                l("Vaelith", "For a thousand years I slept beneath your little candle. Now the world will be quiet, and cold, and still."),
                l("Kael", "Not while one of us is still standing."),
                l("Liese", "Whatever happens, we're with you. Every one of us."),
                l("", "Final battle: defeat Vaelith."),
            ],
            outro: vec![
                l("Vaelith", "The cold... always... returns..."),
                l("Kael", "Then we'll always be here to light the Flame."),
                l("", "Kael raises the Ember Heart. Golden light pours across the ice, and far to the south, in Dawnhold, the great brazier roars back to life."),
                l("Orrin", "The Dawn Flame... it's burning again."),
                l("Brenna", "My lord. Let's go home."),
            ],
            talks: vec![],
            reward_gold: 0,
        },
    }
}

pub const EPILOGUE: [&str; 5] = [
    "With the Flame rekindled, the Pale Sovereign's armies melted like frost at dawn.",
    "Kael refused the crown until every village burned in the war had been rebuilt.",
    "Aldous and Seren restored the Vigil's chronicles, so the truth of the Flame would never be lost again.",
    "The companions of the March scattered across Emberfall. Wherever they went, songs followed.",
    "And the Dawn Flame has burned ever since.",
];

/// The persistent state between chapters (saved to disk).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Campaign {
    pub chapter: usize,
    pub army: Vec<Unit>,
    pub gold: i32,
    pub permadeath: bool,
    pub fallen: Vec<String>,
    /// Number of chapters whose joins have been applied.
    pub joined_through: usize,
    pub finished: bool,
    #[serde(default)]
    pub supports: Vec<SupportState>,
    #[serde(default = "yes")]
    pub battle_scenes: bool,
    #[serde(default)]
    pub difficulty: Difficulty,
    /// New Game+ cycle (0 = first playthrough).
    #[serde(default)]
    pub ng_plus: u32,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Difficulty {
    Easy,
    #[default]
    Normal,
    Hard,
}

impl Difficulty {
    pub fn name(self) -> &'static str {
        match self {
            Difficulty::Easy => "Easy",
            Difficulty::Normal => "Normal",
            Difficulty::Hard => "Hard",
        }
    }

    pub fn next(self) -> Difficulty {
        match self {
            Difficulty::Easy => Difficulty::Normal,
            Difficulty::Normal => Difficulty::Hard,
            Difficulty::Hard => Difficulty::Easy,
        }
    }
}

/// Adjusts an enemy for difficulty and New Game+ cycles.
pub fn toughen(u: &mut Unit, d: Difficulty, ng: u32) {
    let mut st = u.stats.to_array();
    match d {
        Difficulty::Easy => {
            st[0] = st[0] * 85 / 100;
            for v in st.iter_mut().skip(1).take(3) {
                *v = (*v - 1).max(0);
            }
        }
        Difficulty::Normal => {}
        Difficulty::Hard => {
            st[0] = st[0] * 115 / 100;
            st[1] += 2;
            st[2] += 2;
            st[3] += 1;
            st[5] += 1;
        }
    }
    let n = ng as i32;
    let ngb = [3 * n, 2 * n, n, n, 0, n, n];
    for i in 0..7 {
        st[i] = (st[i] + ngb[i]).min(super::unit::STAT_CAPS[i]);
    }
    u.stats = Stats::from_array(st);
    u.hp = u.stats.hp;
}

/// Result of one arena bout.
#[derive(Clone, Debug)]
pub struct ArenaResult {
    pub won: bool,
    pub bet: i32,
    pub log: Vec<String>,
    pub level_up: Option<super::unit::LevelUp>,
}

impl Campaign {
    #[cfg(test)]
    pub fn new(permadeath: bool) -> Campaign {
        Campaign::new_with(permadeath, Difficulty::Normal)
    }

    pub fn new_with(permadeath: bool, difficulty: Difficulty) -> Campaign {
        Campaign {
            chapter: 0,
            army: starting_army(),
            gold: 0,
            permadeath,
            fallen: Vec::new(),
            joined_through: 0,
            finished: false,
            supports: Vec::new(),
            battle_scenes: true,
            difficulty,
            ng_plus: 0,
        }
    }

    /// New Game+: keep the finished army, gold and bonds; enemies grow stronger.
    pub fn new_game_plus(prev: &Campaign) -> Campaign {
        let mut c = Campaign::new_with(prev.permadeath, prev.difficulty);
        c.army = prev.army.clone();
        for u in c.army.iter_mut() {
            u.hp = u.stats.hp;
            u.alive = true;
        }
        c.gold = prev.gold;
        c.supports = prev.supports.clone();
        c.battle_scenes = prev.battle_scenes;
        c.ng_plus = prev.ng_plus + 1;
        c
    }

    fn harden(&self, u: &mut Unit) {
        toughen(u, self.difficulty, self.ng_plus);
        if self.ng_plus > 0 {
            u.level = (u.level + 3 * self.ng_plus as i32).min(super::unit::MAX_LEVEL);
        }
    }

    /// Average effective level of the army (for skirmishes and the arena).
    pub fn army_level(&self) -> i32 {
        if self.army.is_empty() {
            return 1;
        }
        self.army.iter().map(|u| u.effective_level()).sum::<i32>() / self.army.len() as i32
    }

    /// An optional battle on a random generated map, scaled to the army.
    pub fn skirmish(&self, seed: u64) -> Battle {
        use Class::*;
        use ItemId::*;
        let mut rng = Rng::new(seed);
        let themes = [Theme::Grass, Theme::Woods, Theme::Highlands, Theme::Marsh, Theme::Ruins];
        let theme = themes[rng.next_u64() as usize % themes.len()];
        let tier = if self.army.len() > 10 { Tier::Large } else { Tier::Medium };
        let river = theme != Theme::Highlands && rng.roll(40);
        let castle = theme == Theme::Ruins || rng.roll(25);
        let sp = spec(tier, seed, theme, Layout::March, river, castle, 2, 2);
        let g = mapgen::generate(&sp);
        let lvl = self.army_level();
        let promoted = lvl >= 12;
        let level = if promoted { (lvl - 10).max(1) } else { (lvl - 1).max(1) };
        let pool: [(Class, &[ItemId]); 6] = if promoted {
            [(Hero, &[SteelSword]), (Warrior, &[SteelAxe]), (Halberdier, &[SteelLance]), (Sniper, &[SteelBow]), (Sage, &[Blaze]), (Paladin, &[SteelLance])]
        } else {
            [(Mercenary, &[IronSword]), (Brigand, &[IronAxe]), (Soldier, &[IronLance]), (Archer, &[ShortBow]), (Mage, &[Ember]), (Cavalier, &[IronLance])]
        };
        let count = if tier == Tier::Large { 16 } else { 12 };
        let mut taken = Vec::new();
        let mut enemies = Vec::new();
        for i in 0..count {
            let (class, items) = pool[i % pool.len()];
            let zone = [Zone::Front, Zone::Mid, Zone::Rear][i % 3];
            for at in place(&g, &sp, zone, class, 1, &mut taken, &mut rng) {
                let mut u = Unit::generic(class.name(), class, Team::Enemy, level, items, at);
                self.harden(&mut u);
                enemies.push(u);
            }
        }
        let mut units = Vec::new();
        for (slot, idx) in self.deployment(g.starts.len()).into_iter().enumerate() {
            let mut u = self.army[idx].clone();
            u.hp = u.stats.hp;
            u.done = false;
            u.alive = true;
            u.team = Team::Player;
            u.pos = g.starts[slot];
            units.push(u);
        }
        units.extend(enemies);
        let chests = chest_rewards(self.chapter, &g.map);
        let mut b = Battle::new(g.map, units, seed, self.permadeath);
        b.objective = Objective::Rout;
        b.supports = self.support_list();
        b.villages = g.villages.iter().map(|p| Village { pos: *p, reward: Reward::Gold(300 + 50 * lvl), visited: false }).collect();
        b.chests = chests.into_iter().map(|(pos, reward)| super::battle::Chest { pos, reward, opened: false }).collect();
        b
    }

    /// Applies a won skirmish: experience, items, gold and losses, without advancing the story.
    pub fn finish_skirmish(&mut self, b: &Battle) {
        self.merge_units(b);
        self.gold += b.gold + 400 + 60 * self.army_level();
    }

    /// Gold staked on an arena bout for army unit `idx`.
    pub fn arena_bet(&self, idx: usize) -> i32 {
        300 + 40 * self.army[idx].effective_level()
    }

    /// Fights a one-on-one arena bout. Losing costs the bet, never the unit.
    pub fn arena(&mut self, idx: usize, seed: u64) -> Option<ArenaResult> {
        use super::combat::{forecast, resolve, Combatant};
        let bet = self.arena_bet(idx);
        if self.gold < bet || self.army[idx].weapon().is_none() {
            return None;
        }
        self.gold -= bet;
        let mut rng = Rng::new(seed);
        let me = self.army[idx].clone();
        let promoted = me.class.is_promoted();
        let picks: [(Class, ItemId); 5] = if promoted {
            [(Class::Hero, ItemId::SteelSword), (Class::Warrior, ItemId::SteelAxe), (Class::Halberdier, ItemId::SteelLance), (Class::Sage, ItemId::Blaze), (Class::Sniper, ItemId::SteelBow)]
        } else {
            [(Class::Mercenary, ItemId::IronSword), (Class::Fighter, ItemId::IronAxe), (Class::Soldier, ItemId::IronLance), (Class::Mage, ItemId::Ember), (Class::Archer, ItemId::ShortBow)]
        };
        let (class, item) = picks[rng.next_u64() as usize % picks.len()];
        let lvl = (me.level + (rng.next_u64() % 3) as i32 - 1).clamp(1, super::unit::MAX_LEVEL);
        let mut foe = Unit::generic("Gladiator", class, Team::Enemy, lvl, &[item], Pos::new(0, 0));
        self.harden(&mut foe);
        let mut mine = me.clone();
        let dist = if mine.weapon().is_some_and(|w| w.min_range > 1) { 2 } else { 1 };
        mine.equip_for_range(dist);
        let mut log = vec![format!("{} faces a level {} {}!", me.name, foe.level, class.name())];
        let mut won = false;
        for round in 0..10 {
            let f = forecast(&Combatant::new(&mine, Terrain::Plains), &Combatant::new(&foe, Terrain::Plains), dist);
            let out = resolve(&f, &mut rng);
            mine.hp = out.attacker_hp;
            foe.hp = out.defender_hp;
            log.push(format!("Round {}: {} {} HP, {} {} HP", round + 1, me.name, mine.hp, class.name(), foe.hp));
            if foe.hp == 0 {
                won = true;
                break;
            }
            if mine.hp == 0 {
                break;
            }
        }
        let mut level_up = None;
        if won {
            self.gold += bet * 2;
            let xp = super::combat::combat_xp(me.effective_level(), foe.effective_level(), true, true, false);
            level_up = self.army[idx].gain_xp(xp, &mut rng);
            log.push(format!("Victory! +{} gold, +{} EXP.", bet, xp));
        } else if mine.hp == 0 {
            log.push(format!("{} is carried from the arena. The bet is lost.", me.name));
        } else {
            log.push("The bout ends in a draw. The bet is lost.".to_string());
        }
        Some(ArenaResult { won, bet, log, level_up })
    }

    /// Spends gold to forge item `item` of unit `idx` one level higher.
    pub fn forge(&mut self, idx: usize, item: usize) -> Result<i32, &'static str> {
        let it = self.army.get(idx).and_then(|u| u.items.get(item)).copied().ok_or("No such item.")?;
        if !it.can_forge() {
            return Err("This can't be forged any further.");
        }
        let cost = it.forge_cost();
        if self.gold < cost {
            return Err("Not enough gold.");
        }
        self.gold -= cost;
        self.army[idx].items[item].forge += 1;
        Ok(cost)
    }

    pub fn def(&self) -> ChapterDef {
        chapter(self.chapter)
    }

    /// Adds this chapter's new allies to the army (idempotent).
    pub fn begin_chapter(&mut self) {
        if self.joined_through > self.chapter {
            return;
        }
        for u in self.def().joins {
            if !self.army.iter().any(|a| a.name == u.name) && !self.fallen.contains(&u.name) {
                self.army.push(u);
            }
        }
        self.joined_through = self.chapter + 1;
    }

    /// Lord first, then the rest of the army, up to the number of start tiles.
    pub fn deployment(&self, slots: usize) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.army.len()).filter(|&i| self.army[i].is_lord).collect();
        order.extend((0..self.army.len()).filter(|&i| !self.army[i].is_lord));
        order.truncate(slots);
        order
    }

    pub fn battle(&self, seed: u64) -> Battle {
        let def = setup(self.chapter);
        let mut units = Vec::new();
        for (slot, idx) in self.deployment(def.starts.len()).into_iter().enumerate() {
            let mut u = self.army[idx].clone();
            u.hp = u.stats.hp;
            u.done = false;
            u.alive = true;
            u.team = Team::Player;
            u.pos = def.starts[slot];
            units.push(u);
        }
        let mut enemies = def.enemies;
        for e in enemies.iter_mut() {
            self.harden(e);
        }
        units.extend(enemies);
        let mut b = Battle::new(def.map, units, seed, self.permadeath);
        b.supports = self.support_list();
        b.objective = def.objective;
        b.reinforcements = def.reinforcements;
        for (_, e) in b.reinforcements.iter_mut() {
            self.harden(e);
        }
        b.villages = def.villages.into_iter().map(|(pos, reward)| Village { pos, reward, visited: false }).collect();
        b.chests = def.chests.into_iter().map(|(pos, reward)| super::battle::Chest { pos, reward, opened: false }).collect();
        b.fog = def.fog;
        b
    }

    /// Support bonds between current army members (missing ones start at zero).
    pub fn support_list(&self) -> Vec<SupportState> {
        let has = |n: &str| self.army.iter().any(|u| u.name == n);
        supports::pairs()
            .into_iter()
            .filter(|p| has(p.a) && has(p.b))
            .map(|p| self.supports.iter().find(|s| s.is(p.a, p.b)).cloned().unwrap_or_else(|| SupportState::new(p.a, p.b)))
            .collect()
    }

    /// Views the next conversation of support `i` (from `support_list`), raising its level.
    pub fn view_support(&mut self, a: &str, b: &str) -> Option<Vec<Line>> {
        let mut st = self.support_list().into_iter().find(|s| s.is(a, b))?;
        if !st.ready() {
            return None;
        }
        let lines = supports::conversation(a, b, st.level)?;
        st.level += 1;
        self.store_support(st);
        Some(lines)
    }

    fn store_support(&mut self, st: SupportState) {
        match self.supports.iter_mut().find(|s| s.is(&st.a, &st.b)) {
            Some(s) => *s = st,
            None => self.supports.push(st),
        }
    }

    /// Copies the battle's results back into the army and advances the chapter.
    pub fn finish_chapter(&mut self, b: &Battle) {
        self.merge_units(b);
        self.gold += b.gold + self.def().reward_gold;
        if self.chapter + 1 >= CHAPTER_COUNT {
            self.finished = true;
        } else {
            self.chapter += 1;
        }
    }

    fn merge_units(&mut self, b: &Battle) {
        for st in &b.supports {
            self.store_support(st.clone());
        }
        for u in b.units.iter().filter(|u| u.team == Team::Player) {
            let mut kept = u.clone();
            kept.hp = kept.stats.hp;
            kept.done = false;
            kept.talk = false;
            kept.alive = true;
            kept.ai = AiMode::Aggressive;
            let lost = !u.alive && self.permadeath;
            match self.army.iter().position(|a| a.name == u.name) {
                Some(i) if lost => {
                    self.army.remove(i);
                    self.fallen.push(u.name.clone());
                }
                Some(i) => self.army[i] = kept,
                None if lost => self.fallen.push(u.name.clone()),
                None => self.army.push(kept),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::battle::Outcome;
    use std::collections::HashSet;

    #[test]
    fn every_chapter_is_valid() {
        use crate::logic::terrain::{MoveType, Terrain};
        for n in 0..CHAPTER_COUNT {
            let def = chapter(n);
            let st = setup(n);
            let map = &st.map;
            assert_eq!((map.width, map.height), def.tier.size(), "chapter {n}");
            let mut seen = HashSet::new();
            for &s in &st.starts {
                assert!(map.at(s).move_cost(MoveType::Infantry).is_some(), "chapter {n}: start {s:?} impassable");
                assert!(seen.insert(s), "chapter {n}: duplicate start {s:?}");
            }
            assert!(st.starts.len() >= def.tier.deploy().min(8), "chapter {n}: only {} starts", st.starts.len());
            for u in st.enemies.iter().chain(st.reinforcements.iter().map(|(_, u)| u)) {
                assert!(map.in_bounds(u.pos), "chapter {n}: {} off map", u.name);
                assert!(map.at(u.pos).move_cost(u.class.move_type()).is_some(), "chapter {n}: {} on impassable {:?}", u.name, u.pos);
                assert!(u.weapon().is_some(), "chapter {n}: {} has no usable weapon", u.name);
            }
            for u in &st.enemies {
                assert!(seen.insert(u.pos), "chapter {n}: {} overlaps at {:?}", u.name, u.pos);
            }
            for (v, _) in &st.villages {
                assert_eq!(map.at(*v), Terrain::Village, "chapter {n}: village tile {v:?}");
            }
            if let Objective::Seize(t) = st.objective {
                assert_eq!(map.at(t), Terrain::Throne);
                assert!(st.enemies.iter().any(|u| u.is_boss && u.pos == t), "chapter {n}: boss should hold the throne");
            }
            if matches!(st.objective, Objective::Boss) {
                assert_eq!(st.enemies.iter().filter(|u| u.is_boss).count(), 1, "chapter {n} needs one boss");
            }
            for (name, _) in &def.talks {
                assert!(st.enemies.iter().any(|u| u.name == *name && u.talk), "chapter {n}: talk target {name}");
            }
            for u in &def.joins {
                for k in &u.items {
                    assert!(u.can_use(k.id), "{} can't use {:?}", u.name, k.id);
                }
            }
            assert!(!def.intro.is_empty() && !def.outro.is_empty());
            let expected = match def.tier {
                Tier::Small => 7..=13,
                Tier::Medium => 11..=22,
                Tier::Large => 14..=28,
                Tier::ExtraLarge => 19..=38,
            };
            assert!(expected.contains(&st.enemies.len()), "chapter {n}: {} enemies for {:?}", st.enemies.len(), def.tier);
        }
    }

    #[test]
    fn all_four_map_sizes_are_used() {
        let tiers: Vec<Tier> = (0..CHAPTER_COUNT).map(|n| chapter(n).tier).collect();
        for t in [Tier::Small, Tier::Medium, Tier::Large, Tier::ExtraLarge] {
            assert!(tiers.contains(&t), "{t:?} unused");
        }
        assert_eq!(chapter(CHAPTER_COUNT - 1).tier, Tier::ExtraLarge, "finale is the biggest battle");
    }

    #[test]
    fn starting_army_can_use_its_items() {
        for u in starting_army() {
            for k in &u.items {
                assert!(u.can_use(k.id), "{} can't use {:?}", u.name, k.id);
            }
        }
    }

    #[test]
    fn carry_over_between_chapters() {
        let mut c = Campaign::new(true);
        c.begin_chapter();
        let mut b = c.battle(1);
        // Kael levels up; Tamsin falls; the boss dies.
        b.units[0].level = 3;
        b.units[0].hp = 1;
        let tamsin = b.units.iter().position(|u| u.name == "Tamsin").unwrap();
        b.units[tamsin].alive = false;
        let boss = b.units.iter().position(|u| u.is_boss).unwrap();
        b.units[boss].alive = false;
        b.gold = 100;
        assert_eq!(b.outcome(), Outcome::Victory);
        c.finish_chapter(&b);
        assert_eq!(c.chapter, 1);
        assert_eq!(c.army[0].level, 3);
        assert_eq!(c.army[0].hp, c.army[0].stats.hp, "healed between chapters");
        assert!(!c.army.iter().any(|u| u.name == "Tamsin"), "permadeath");
        assert_eq!(c.fallen, vec!["Tamsin".to_string()]);
        assert_eq!(c.gold, 100 + 1200);
        c.begin_chapter();
        c.begin_chapter();
        assert_eq!(c.army.iter().filter(|u| u.name == "Holt").count(), 1, "joins applied once");
    }

    #[test]
    fn casual_mode_keeps_fallen_and_recruits_join() {
        let mut c = Campaign::new(false);
        c.chapter = 1;
        c.begin_chapter();
        let mut b = c.battle(1);
        let dain = b.units.iter().position(|u| u.name == "Dain").unwrap();
        b.recruit(dain);
        let liese = b.units.iter().position(|u| u.name == "Liese").unwrap();
        b.units[liese].alive = false;
        c.finish_chapter(&b);
        assert!(c.army.iter().any(|u| u.name == "Liese"));
        let d = c.army.iter().find(|u| u.name == "Dain").unwrap();
        assert_eq!(d.team, Team::Player);
        assert!(!d.talk);
    }

    #[test]
    fn deployment_caps_and_lord_first() {
        let mut c = Campaign::new(true);
        c.army.rotate_left(2);
        let d = c.deployment(3);
        assert_eq!(d.len(), 3);
        assert!(c.army[d[0]].is_lord);
        let b = c.battle(1);
        assert!(b.units[0].is_lord);
        assert_eq!(b.units[0].pos, setup(0).starts[0]);
    }

    #[test]
    fn supports_persist_and_unlock() {
        let mut c = Campaign::new(true);
        c.begin_chapter();
        let mut b = c.battle(1);
        assert!(b.supports.iter().any(|s| s.is("Kael", "Brenna")));
        let i = b.supports.iter().position(|s| s.is("Kael", "Brenna")).unwrap();
        b.supports[i].points = 3;
        let boss = b.units.iter().position(|u| u.is_boss).unwrap();
        b.units[boss].alive = false;
        c.finish_chapter(&b);
        assert!(c.support_list().iter().any(|s| s.is("Kael", "Brenna") && s.ready()));
        let lines = c.view_support("Brenna", "Kael").expect("C conversation");
        assert!(!lines.is_empty());
        assert!(c.view_support("Kael", "Brenna").is_none(), "B needs more points");
        assert_eq!(c.battle(2).support_level("Kael", "Brenna"), 1);
    }

    #[test]
    fn old_saves_load_with_defaults() {
        let mut v: serde_json::Value = serde_json::to_value(Campaign::new(true)).unwrap();
        v.as_object_mut().unwrap().remove("supports");
        v.as_object_mut().unwrap().remove("battle_scenes");
        let c: Campaign = serde_json::from_value(v).unwrap();
        assert!(c.supports.is_empty());
        assert!(c.battle_scenes);
    }

    #[test]
    fn difficulty_scales_enemies() {
        let mut easy = Campaign::new_with(true, Difficulty::Easy);
        let mut hard = Campaign::new_with(true, Difficulty::Hard);
        easy.begin_chapter();
        hard.begin_chapter();
        let e = easy.battle(1);
        let h = hard.battle(1);
        let boss = |b: &Battle| b.units.iter().find(|u| u.is_boss).unwrap().stats;
        assert!(boss(&e).hp < boss(&h).hp);
        assert!(boss(&e).pow < boss(&h).pow);
    }

    #[test]
    fn new_game_plus_keeps_army_and_toughens_foes() {
        let mut c = Campaign::new(true);
        c.army[0].level = 15;
        c.gold = 9999;
        c.finished = true;
        let ng = Campaign::new_game_plus(&c);
        assert_eq!(ng.chapter, 0);
        assert_eq!(ng.ng_plus, 1);
        assert_eq!(ng.army[0].level, 15);
        assert_eq!(ng.gold, 9999);
        let base = Campaign::new(true).battle(1);
        let plus = { let mut n = ng.clone(); n.begin_chapter(); n.battle(1) };
        let boss = |b: &Battle| b.units.iter().find(|u| u.is_boss).unwrap().stats.hp;
        assert!(boss(&plus) > boss(&base));
    }

    #[test]
    fn skirmish_is_a_playable_rout() {
        let mut c = Campaign::new(true);
        c.begin_chapter();
        for seed in 0..5 {
            let b = c.skirmish(seed);
            assert_eq!(b.objective, Objective::Rout);
            assert!(b.living(Team::Enemy).count() >= 8);
            assert!(b.units[0].is_lord);
        }
        let mut b = c.skirmish(3);
        b.units[0].level = 9;
        let gold = c.gold;
        c.finish_skirmish(&b);
        assert_eq!(c.chapter, 0, "skirmishes don't advance the story");
        assert_eq!(c.army[0].level, 9);
        assert!(c.gold > gold);
    }

    #[test]
    fn arena_costs_or_pays() {
        let mut c = Campaign::new(true);
        c.gold = 100;
        assert!(c.arena(0, 1).is_none(), "can't afford the bet");
        c.gold = 10_000;
        let mut wins = 0;
        for seed in 0..20 {
            let before = c.gold;
            let r = c.arena(0, seed).unwrap();
            if r.won {
                wins += 1;
                assert_eq!(c.gold, before + r.bet);
            } else {
                assert_eq!(c.gold, before - r.bet);
            }
            assert!(c.army[0].alive);
        }
        assert!(wins > 0);
    }

    #[test]
    fn forging_costs_gold_and_caps() {
        let mut c = Campaign::new(true);
        c.gold = 100_000;
        let start = c.gold;
        for _ in 0..crate::logic::items::MAX_FORGE {
            c.forge(0, 0).unwrap();
        }
        assert_eq!(c.army[0].items[0].forge, crate::logic::items::MAX_FORGE);
        assert!(c.forge(0, 0).is_err());
        assert!(c.gold < start);
        assert!(c.forge(0, 1).is_err(), "vulneraries can't be forged");
    }

    #[test]
    fn castle_maps_have_chests() {
        let with_chests = (0..CHAPTER_COUNT).filter(|n| !setup(*n).chests.is_empty()).count();
        assert!(with_chests >= 4, "{with_chests}");
        assert!(fog_chapter(4) && !fog_chapter(0));
        assert!(setup(4).fog);
    }

    #[test]
    fn save_roundtrip() {
        let mut c = Campaign::new(true);
        c.gold = 777;
        let json = serde_json::to_string(&c).unwrap();
        let back: Campaign = serde_json::from_str(&json).unwrap();
        assert_eq!(back.gold, 777);
        assert_eq!(back.army.len(), c.army.len());
        assert_eq!(back.army[0].items, c.army[0].items);
    }

    #[test]
    fn last_chapter_finishes_campaign() {
        let mut c = Campaign::new(true);
        c.chapter = CHAPTER_COUNT - 1;
        c.begin_chapter();
        let mut b = c.battle(1);
        let boss = b.units.iter().position(|u| u.is_boss).unwrap();
        b.units[boss].alive = false;
        c.finish_chapter(&b);
        assert!(c.finished);
        assert_eq!(c.chapter, CHAPTER_COUNT - 1);
    }
}

#[cfg(test)]
mod sim {
    //! Auto-battles every chapter with the enemy AI controlling both sides,
    //! as a smoke test for panics and a rough difficulty gauge.
    use super::*;
    use crate::logic::ai::{decide, AiAction};
    use crate::logic::battle::{Outcome, Phase};

    fn run(c: &Campaign, seed: u64) -> (Outcome, u32, usize) {
        let mut b = c.battle(seed);
        b.begin_phase(Phase::Player);
        for _ in 0..40 {
            for team in [Team::Player, Team::Enemy] {
                if team == Team::Enemy {
                    b.begin_phase(Phase::Enemy);
                }
                let ids: Vec<usize> = b.living(team).collect();
                for id in ids {
                    if !b.units[id].alive || b.units[id].team != team {
                        continue;
                    }
                    // Players heal first if they can.
                    if team == Team::Player {
                        if let Some(&t) = b.heal_targets_from(id, b.units[id].pos).first() {
                            b.heal(id, t);
                            continue;
                        }
                        if let Some(&t) = b.talk_targets_from(id, b.units[id].pos).first() {
                            b.recruit(t);
                            continue;
                        }
                    }
                    match decide(&b, id) {
                        AiAction::Attack { dest, target } => {
                            b.move_unit(id, dest);
                            b.attack(id, target);
                        }
                        AiAction::Move { .. } if b.units[id].is_lord && !matches!(b.objective, Objective::Seize(_)) => {}
                        AiAction::Move { dest } => {
                            b.move_unit(id, dest);
                            if b.can_visit(id) {
                                b.visit(id);
                            }
                            if b.can_seize(id) {
                                b.seize(id);
                            }
                        }
                        AiAction::Wait => {}
                    }
                    if b.outcome() != Outcome::Ongoing {
                        return (b.outcome(), b.turn, b.fallen.len());
                    }
                }
            }
            b.begin_phase(Phase::Player);
            if b.outcome() != Outcome::Ongoing {
                return (b.outcome(), b.turn, b.fallen.len());
            }
        }
        (b.outcome(), b.turn, b.fallen.len())
    }

    /// Slow on big maps; run with `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn auto_battle_every_chapter() {
        for n in 0..CHAPTER_COUNT {
            let mut c = Campaign::new(false);
            c.chapter = n;
            c.begin_chapter();
            // Give later chapters a plausibly-levelled army.
            for u in c.army.iter_mut() {
                let target = (n as i32 * 2 + 1).min(19);
                let mut rng = crate::logic::rng::Rng::new(7);
                while u.level < target {
                    u.level_up(&mut rng);
                }
                if n >= 5 && u.can_promote() {
                    u.promote();
                    for _ in 0..(n as i32 - 4) * 2 {
                        u.level_up(&mut rng);
                    }
                }
            }
            for _ in 0..(n.saturating_sub(0)) {
                for later in 1..=n {
                    for j in chapter(later).joins {
                        if !c.army.iter().any(|a| a.name == j.name) {
                            c.army.push(j);
                        }
                    }
                }
            }
            let mut wins = 0;
            let mut summary = Vec::new();
            for seed in 0..10 {
                let (o, t, f) = run(&c, seed);
                if o == Outcome::Victory {
                    wins += 1;
                }
                summary.push(format!("{:?}@{}({}lost)", o, t, f));
            }
            println!("chapter {}: {} wins/10  {}", n + 1, wins, summary.join(" "));
        }
    }
}
