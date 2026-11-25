//! The campaign "Emberfall": chapters, story, and the persistent army.

use serde::{Deserialize, Serialize};

use super::battle::{Battle, Objective, Reward, Village};
use super::items::ItemId;
use super::map::Map;
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

pub struct ChapterDef {
    pub title: &'static str,
    pub map: [&'static str; 12],
    pub objective: Objective,
    pub starts: Vec<Pos>,
    pub enemies: Vec<Unit>,
    pub reinforcements: Vec<(u32, Unit)>,
    pub villages: Vec<(Pos, Reward)>,
    /// Allies who join the army at the start of this chapter.
    pub joins: Vec<Unit>,
    pub intro: Vec<Line>,
    pub outro: Vec<Line>,
    /// Conversation when the lord talks to a recruitable enemy (by name).
    pub talks: Vec<(&'static str, Vec<Line>)>,
    pub reward_gold: i32,
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

fn s(hp: i32, pow: i32, skl: i32, spd: i32, lck: i32, def: i32, res: i32) -> Stats {
    Stats::new(hp, pow, skl, spd, lck, def, res)
}

pub const CHAPTER_COUNT: usize = 8;

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

pub fn chapter(n: usize) -> ChapterDef {
    use Class::*;
    use ItemId::*;
    match n {
        0 => ChapterDef {
            title: "The Ashford Crossing",
            map: [
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
            objective: Objective::Boss,
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
            map: [
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
            objective: Objective::Rout,
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
            map: [
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
            objective: Objective::Survive(6),
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
            map: [
                "~~~~....FF.#####",
                "~~~...F....#,,H#",
                "~~.......F.#,P,#",
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
            objective: Objective::Seize(p(14, 1)),
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
                l("Ilse", "The Legion's next fort is in the Ironpeak Pass. Thorne holds it. He's worse than Hask ever was."),
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
            title: "Ironpeak Pass",
            map: [
                "MMMMM..FF..MMMMM",
                "MMMM....F...MMMM",
                "MMM...MM.....MMM",
                "MM..F.MMM..F..MM",
                "M.....MM.....T.M",
                "M..F......F....M",
                "MM....T...MM...M",
                "MMM..FF...MMM..M",
                "M.....M.......MM",
                "M..V..M...F..MMM",
                "MM.......F..MMMM",
                "MMM....FF.MMMMMM",
            ],
            objective: Objective::Boss,
            starts: vec![p(4, 10), p(3, 10), p(5, 10), p(6, 10), p(3, 11), p(4, 11), p(5, 11), p(6, 11), p(2, 10), p(7, 10), p(8, 10)],
            enemies: vec![
                e("Fighter", Fighter, 7, &[SteelAxe], 6, 2),
                e("Fighter", Fighter, 7, &[HandAxe], 7, 3),
                e("Archer", Archer, 7, &[SteelBow], 11, 3),
                e("Archer", Archer, 7, &[Longbow], 10, 6),
                e("Knight", Knight, 7, &[SteelLance], 6, 6),
                e("Mage", Mage, 7, &[Blaze], 8, 1),
                e("Mercenary", Mercenary, 7, &[SteelSword], 4, 4),
                e("Mercenary", Mercenary, 7, &[KeenSword], 12, 8),
                e("Brigand", Brigand, 8, &[SteelAxe], 10, 9).with_ai(AiMode::Guard),
                Unit::generic("Thorne", Warlord, Team::Enemy, 2, &[SilverAxe, HandAxe], p(13, 4)).boss(),
            ],
            reinforcements: vec![(3, e("Fighter", Fighter, 7, &[IronAxe], 7, 0)), (3, e("Mage", Mage, 7, &[Ember], 8, 0)), (6, e("Archer", Archer, 7, &[SteelBow], 7, 0))],
            villages: vec![(p(3, 9), Reward::Item(SilverLance))],
            joins: vec![ally("Maeve", Lancer, 7, s(26, 9, 8, 10, 6, 8, 3), s(80, 45, 45, 55, 45, 30, 20), &[SteelLance, Javelin])],
            intro: vec![
                l("", "The Ironpeak Pass. A narrow road through frozen mountains, and the only way north to the capital."),
                l("Maeve", "You must be the March lord everyone's whispering about. Maeve. I guide caravans through here, or I did until Thorne arrived."),
                l("Kael", "Will you guide an army?"),
                l("Maeve", "If that army is going to knock Thorne off his mountain, gladly."),
                l("Holt", "Axe-folk move well on the peaks. Watch your flanks."),
            ],
            outro: vec![
                l("Thorne", "The Legion... is endless..."),
                l("Maeve", "The pass is open. Beyond it lies the Ashen Wall, Morvaine's outer defences."),
                l("Kael", "Then we go through it."),
            ],
            talks: vec![],
            reward_gold: 3000,
        },
        5 => ChapterDef {
            title: "The Ashen Wall",
            map: [
                "..FF....##...FF.",
                ".......T##......",
                "..##........##..",
                "..#.....F....#..",
                "....FF....FF....",
                "~~=~~~~==~~~~=~~",
                "................",
                "..F..T....T..F..",
                ".##..........##.",
                "....FF....FF....",
                "..F....FF....F..",
                "...............V",
            ],
            objective: Objective::Rout,
            starts: vec![p(7, 11), p(8, 11), p(6, 11), p(9, 11), p(5, 11), p(10, 11), p(7, 10), p(8, 10), p(4, 11), p(11, 11), p(6, 9), p(9, 9)],
            enemies: vec![
                e("General", General, 2, &[SteelLance], 7, 4),
                e("General", General, 2, &[SteelAxe], 8, 4),
                e("Sniper", Sniper, 2, &[SteelBow], 3, 1),
                e("Sniper", Sniper, 2, &[Longbow], 12, 1),
                e("Sage", Sage, 2, &[Blaze], 11, 0),
                e("Warrior", Warrior, 2, &[SteelAxe], 2, 4),
                e("Warrior", Warrior, 2, &[HandAxe], 13, 4),
                e("Halberdier", Halberdier, 2, &[SteelLance], 4, 3),
                e("Halberdier", Halberdier, 2, &[Javelin], 11, 3),
                Unit::generic("Vex", Hero, Team::Enemy, 5, &[SilverSword], p(7, 1)).boss(),
            ],
            reinforcements: vec![
                (3, e("Hero", Hero, 1, &[SteelSword], 0, 6)),
                (3, e("Hero", Hero, 1, &[SteelSword], 15, 6)),
                (5, e("Warrior", Warrior, 2, &[SteelAxe], 0, 9)),
                (5, e("Sage", Sage, 2, &[Ember], 15, 9)),
            ],
            villages: vec![(p(15, 11), Reward::Item(SilverSword))],
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
                l("Bram", "Past this wall is the Cinder River. He'll try to burn the bridge with us on it."),
                l("Kael", "Let him try."),
            ],
            talks: vec![],
            reward_gold: 3500,
        },
        6 => ChapterDef {
            title: "The Burning Bridge",
            map: [
                "MM....FF....F.MM",
                "M.....F..~~....M",
                "..F......~~..T..",
                "....F....~~.....",
                ".........~~..F..",
                "..FF.....==.....",
                "........F==.....",
                "..T......~~..F..",
                ".........~~.....",
                "..F..F...~~.T...",
                "M.......F~~....M",
                "MM......~~~...MM",
            ],
            objective: Objective::Survive(7),
            starts: vec![p(13, 5), p(12, 5), p(12, 6), p(13, 6), p(12, 4), p(14, 5), p(14, 6), p(12, 7), p(11, 3), p(14, 3), p(13, 2), p(12, 8)],
            enemies: vec![
                e("Warrior", Warrior, 3, &[SteelAxe], 3, 5),
                e("Warrior", Warrior, 3, &[HandAxe], 2, 6),
                e("Hero", Hero, 3, &[SteelSword], 5, 4),
                e("Halberdier", Halberdier, 3, &[SteelLance], 6, 6),
                e("Sage", Sage, 3, &[Blaze], 4, 8),
                e("Sniper", Sniper, 3, &[Longbow], 7, 2),
                Unit::new("Corvin", Sniper, Team::Enemy, 4, s(27, 10, 13, 12, 5, 7, 5), s(70, 45, 55, 50, 30, 20, 20), &[SteelBow, Longbow], p(6, 2)).talkable(),
                Unit::generic("Ruk", Warrior, Team::Enemy, 6, &[SilverAxe], p(2, 7)),
            ],
            reinforcements: vec![
                (2, e("Hero", Hero, 2, &[SteelSword], 0, 3)),
                (2, e("Warrior", Warrior, 2, &[SteelAxe], 0, 6)),
                (3, e("General", General, 3, &[SteelLance], 0, 8)),
                (4, e("Sage", Sage, 3, &[Blaze], 1, 2)),
                (4, e("Hero", Hero, 3, &[SteelSword], 0, 5)),
                (5, e("Warrior", Warrior, 3, &[SteelAxe], 0, 4)),
                (5, e("Sniper", Sniper, 3, &[SteelBow], 1, 9)),
                (6, e("Halberdier", Halberdier, 4, &[SteelLance], 0, 6)),
            ],
            villages: vec![],
            joins: vec![],
            intro: vec![
                l("", "The Cinder River. One stone bridge, and on the far bank, the whole strength of the Legion's field army."),
                l("Ruk", "Hold them on the bridge! When the sorcerers finish the ritual, the whole span burns!"),
                l("Ilse", "The ritual needs seven turns. If we hold the east bank that long, it breaks, and their army is stranded."),
                l("Kael", "Then we hold the east bank. Bram, Sera, plug the bridge. Archers and mages behind them."),
                l("Tamsin", "There's a Legion sniper on the far side who keeps missing on purpose. I think he wants out."),
                l("", "Hint: Corvin can be recruited if Kael talks to him."),
            ],
            outro: vec![
                l("", "The ritual collapses. On the far bank the Legion's banners waver and start to scatter."),
                l("Corvin", "Morvaine waits in the Ember Throne. Without his army, he has only his own sorcery left."),
                l("Kael", "Then this ends tomorrow."),
            ],
            talks: vec![(
                "Corvin",
                vec![
                    l("Kael", "You've been firing wide all day. Why?"),
                    l("Corvin", "Because I'm from Hollowmere. You saved my mother and sisters. I'm not going to shoot their saviour."),
                    l("Kael", "Then come home. Fight for your village instead of against it."),
                    l("Corvin", "Thought you'd never ask."),
                ],
            )],
            reward_gold: 4000,
        },
        _ => ChapterDef {
            title: "Throne of Ash",
            map: [
                "################",
                "#,,P,,,H,,,,P,,#",
                "#,,,,,,,,,,,,,,#",
                "#,,P,,#,,#,,P,,#",
                "#,,,,,#,,#,,,,,#",
                "####,,#,,#,,####",
                "#,,,,,,,,,,,,,,#",
                "#,P,,,,P,,,,P,,#",
                "#,,,,##,,##,,,,#",
                "#,,,,#,,,,#,,,,#",
                "#,,,,,,,,,,,,,,#",
                "######,,,,######",
            ],
            objective: Objective::Boss,
            starts: vec![p(7, 11), p(8, 11), p(6, 11), p(9, 11), p(7, 10), p(8, 10), p(6, 10), p(9, 10), p(5, 10), p(10, 10), p(7, 9), p(8, 9)],
            enemies: vec![
                e("General", General, 6, &[SilverLance], 7, 2),
                e("General", General, 6, &[SteelAxe], 8, 2),
                e("Sage", Sage, 6, &[Inferno], 3, 1),
                e("Sage", Sage, 6, &[Blaze], 12, 1),
                e("Hero", Hero, 6, &[SilverSword], 4, 4),
                e("Hero", Hero, 6, &[SteelSword], 11, 4),
                e("Sniper", Sniper, 6, &[Longbow], 2, 7),
                e("Sniper", Sniper, 6, &[SteelBow], 13, 7),
                e("Warrior", Warrior, 6, &[SilverAxe], 7, 6),
                e("Halberdier", Halberdier, 6, &[SilverLance], 8, 6),
                Unit::new("Morvaine", Sorcerer, Team::Enemy, 10, s(42, 15, 13, 10, 6, 10, 14), Stats::default(), &[Umbra], p(7, 1)).boss(),
            ],
            reinforcements: vec![
                (3, e("Hero", Hero, 5, &[SteelSword], 1, 6)),
                (3, e("Warrior", Warrior, 5, &[SteelAxe], 14, 6)),
                (5, e("Sage", Sage, 5, &[Blaze], 1, 6)),
                (5, e("General", General, 5, &[SteelLance], 14, 6)),
            ],
            villages: vec![],
            joins: vec![],
            intro: vec![
                l("", "The capital. Ash lies on the streets like snow. In the great hall, the cold brazier of the Dawn Flame still stands."),
                l("Morvaine", "Little lord of Ashford. You've come a long way just to die in the dark."),
                l("Kael", "The Flame isn't dead, Morvaine. It's in every village that stood when you said it would kneel."),
                l("Morvaine", "Pretty words. Let's see how they burn."),
                l("Liese", "Whatever happens, I'm right behind you. All of us are."),
                l("", "Final battle: defeat Morvaine."),
            ],
            outro: vec![
                l("Morvaine", "Impossible... the dark was... eternal..."),
                l("", "As the sorcerer falls, a single spark rises from the brazier. It catches, then roars upward, gold and white."),
                l("Orrin", "The Dawn Flame... it's burning again."),
                l("Brenna", "My lord. Your people are waiting."),
                l("Kael", "Not my people. Our people. Every one of you made this possible."),
            ],
            talks: vec![],
            reward_gold: 0,
        },
    }
}

pub const EPILOGUE: [&str; 4] = [
    "With the Flame rekindled, the Ashen Legion broke apart within a season.",
    "Kael refused the crown until every village Morvaine burned had been rebuilt.",
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
}

impl Campaign {
    pub fn new(permadeath: bool) -> Campaign {
        Campaign { chapter: 0, army: starting_army(), gold: 0, permadeath, fallen: Vec::new(), joined_through: 0, finished: false }
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
        let def = self.def();
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
        units.extend(def.enemies);
        let mut b = Battle::new(Map::from_rows(&def.map), units, seed, self.permadeath);
        b.objective = def.objective;
        b.reinforcements = def.reinforcements;
        b.villages = def.villages.into_iter().map(|(pos, reward)| Village { pos, reward, visited: false }).collect();
        b
    }

    /// Copies the battle's results back into the army and advances the chapter.
    pub fn finish_chapter(&mut self, b: &Battle) {
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
        self.gold += b.gold + self.def().reward_gold;
        if self.chapter + 1 >= CHAPTER_COUNT {
            self.finished = true;
        } else {
            self.chapter += 1;
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
        for n in 0..CHAPTER_COUNT {
            let def = chapter(n);
            let map = Map::from_rows(&def.map);
            assert_eq!((map.width, map.height), (16, 12), "chapter {n}");
            let mut seen = HashSet::new();
            for &s in &def.starts {
                assert!(map.at(s).move_cost(super::super::terrain::MoveType::Infantry).is_some(), "chapter {n}: start {s:?} impassable");
                assert!(seen.insert(s), "chapter {n}: duplicate start {s:?}");
            }
            for u in def.enemies.iter().chain(def.reinforcements.iter().map(|(_, u)| u)) {
                assert!(map.in_bounds(u.pos), "chapter {n}: {} off map", u.name);
                assert!(map.at(u.pos).move_cost(u.class.move_type()).is_some(), "chapter {n}: {} on impassable {:?}", u.name, u.pos);
                assert!(u.weapon().is_some(), "chapter {n}: {} has no usable weapon", u.name);
            }
            for u in &def.enemies {
                assert!(seen.insert(u.pos), "chapter {n}: {} overlaps at {:?}", u.name, u.pos);
            }
            for (v, _) in &def.villages {
                assert_eq!(map.at(*v), super::super::terrain::Terrain::Village, "chapter {n}: village tile {v:?}");
            }
            if let Objective::Seize(t) = def.objective {
                assert_eq!(map.at(t), super::super::terrain::Terrain::Throne);
            }
            if matches!(def.objective, Objective::Boss) {
                assert_eq!(def.enemies.iter().filter(|u| u.is_boss).count(), 1, "chapter {n} needs one boss");
            }
            for (name, _) in &def.talks {
                assert!(def.enemies.iter().any(|u| u.name == *name && u.talk), "chapter {n}: talk target {name}");
            }
            for u in &def.joins {
                for k in &u.items {
                    assert!(u.can_use(k.id), "{} can't use {:?}", u.name, k.id);
                }
            }
            assert!(!def.intro.is_empty() && !def.outro.is_empty());
        }
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
        assert_eq!(b.units[0].pos, chapter(0).starts[0]);
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

    #[test]
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
