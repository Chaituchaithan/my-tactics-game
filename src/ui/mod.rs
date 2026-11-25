pub mod battle_screen;
pub mod gfx;
pub mod prep;
pub mod storage;
pub mod story;
pub mod widgets;

use macroquad::prelude::*;

use battle_screen::{BattleExit, BattleScreen};
use gfx::{rgb, UnitLook};
use prep::{PrepExit, PrepScreen};
use story::StoryScreen;
use widgets::*;

use crate::logic::campaign::{chapter, Campaign, CHAPTER_COUNT, EPILOGUE};
use crate::logic::terrain::Terrain;
use crate::logic::unit::{Class, Team, WeaponKind};

enum AfterStory {
    /// Show the current chapter's intro next.
    Intro,
    Prep,
    Ending,
}

enum Screen {
    Title,
    Story(Box<StoryScreen>, AfterStory),
    Prep(PrepScreen),
    Battle(Box<BattleScreen>),
    Ending,
}

pub struct App {
    screen: Screen,
    campaign: Option<Campaign>,
    saved: Option<Campaign>,
    permadeath: bool,
    show_help: bool,
    confirm_new: bool,
    time: f32,
}

fn seed() -> u64 {
    (macroquad::miniquad::date::now() * 1000.0) as u64
}

const HELP: [&str; 10] = [
    "Tap a blue unit, then a blue tile to move.",
    "Then choose Attack, Heal, Items, Talk, Visit or Wait.",
    "Check the forecast before attacking.",
    "Sword beats Axe, Axe beats Lance, Lance beats Sword.",
    "4+ more Speed than the foe = strike twice.",
    "Forests, mountains and forts boost Def and Avoid.",
    "Weapons wear out. Buy new ones between chapters.",
    "Visit villages for gifts. Kael can Talk to some foes.",
    "Level 10+ units can promote with a Master Seal.",
    "If Kael falls, the game is over.",
];

impl App {
    pub fn new() -> App {
        let saved = storage::load();
        App { screen: Screen::Title, campaign: None, saved, permadeath: true, show_help: false, confirm_new: false, time: 0.0 }
    }

    fn intro_story(c: &Campaign) -> Screen {
        let def = chapter(c.chapter);
        let heading = format!("Chapter {}: {}", c.chapter + 1, def.title);
        Screen::Story(Box::new(StoryScreen::new(def.intro, c.chapter, Some(heading), &c.army)), AfterStory::Prep)
    }

    fn start_battle(c: &Campaign) -> Screen {
        Screen::Battle(Box::new(BattleScreen::new(c.battle(seed()), c.chapter, c.battle_scenes)))
    }

    pub fn frame(&mut self) {
        self.time += get_frame_time();
        clear_background(rgb(16, 16, 30));
        let next = match &mut self.screen {
            Screen::Title => self.title(),
            Screen::Story(s, after) => {
                if s.frame() {
                    let c = self.campaign.as_ref().unwrap();
                    Some(match after {
                        AfterStory::Intro => Self::intro_story(c),
                        AfterStory::Prep => Screen::Prep(PrepScreen::new()),
                        AfterStory::Ending => Screen::Ending,
                    })
                } else {
                    None
                }
            }
            Screen::Prep(p) => {
                let c = self.campaign.as_mut().unwrap();
                match p.frame(c) {
                    PrepExit::Stay => None,
                    PrepExit::Begin => {
                        storage::save(c);
                        Some(Self::start_battle(c))
                    }
                    PrepExit::Title => {
                        storage::save(c);
                        self.saved = Some(c.clone());
                        Some(Screen::Title)
                    }
                }
            }
            Screen::Battle(b) => match b.frame() {
                BattleExit::Stay => None,
                BattleExit::Title => {
                    self.saved = storage::load();
                    Some(Screen::Title)
                }
                BattleExit::Retry => Some(Self::start_battle(self.campaign.as_ref().unwrap())),
                BattleExit::Victory => {
                    let c = self.campaign.as_mut().unwrap();
                    let finished_ch = b.chapter();
                    c.finish_chapter(b.battle());
                    let outro = chapter(finished_ch).outro;
                    let story = Box::new(StoryScreen::new(outro, finished_ch, None, &c.army));
                    if c.finished {
                        storage::delete();
                        self.saved = None;
                        Some(Screen::Story(story, AfterStory::Ending))
                    } else {
                        c.begin_chapter();
                        storage::save(c);
                        self.saved = Some(c.clone());
                        Some(Screen::Story(story, AfterStory::Intro))
                    }
                }
            },
            Screen::Ending => self.ending(),
        };
        if let Some(s) = next {
            self.screen = s;
        }
    }

    fn ending(&mut self) -> Option<Screen> {
        let (w, h) = (screen_width(), screen_height());
        let u = ui_unit();
        let mut input = Input::poll();
        for i in 0..80 {
            let fx = ((i * 7919) % 1000) as f32 / 1000.0;
            let fy = ((i * 104_729) % 1000) as f32 / 1000.0;
            let y = (fy * h - self.time * (15.0 + (i % 5) as f32 * 8.0)).rem_euclid(h);
            draw_rectangle(fx * w, y, 3.0, 3.0, Color::new(1.0, 0.7 + (i % 3) as f32 * 0.1, 0.3, 0.8));
        }
        text_centered("THE END", w / 2.0, h * 0.16, u * 3.0, ACCENT);
        let mut y = h * 0.26;
        for line in EPILOGUE {
            for l in wrap(line, w * 0.8, u * 0.9) {
                text_centered(&l, w / 2.0, y, u * 0.9, INK);
                y += u * 1.15;
            }
            y += u * 0.3;
        }
        if let Some(c) = &self.campaign {
            y += u * 0.4;
            let names: Vec<String> = c.army.iter().map(|u| format!("{} ({})", u.name, u.class.name())).collect();
            for l in wrap(&names.join(", "), w * 0.85, u * 0.75) {
                text_centered(&l, w / 2.0, y, u * 0.75, ALLY);
                y += u * 0.95;
            }
            if !c.fallen.is_empty() {
                text_centered(&format!("In memory of: {}", c.fallen.join(", ")), w / 2.0, y + u * 0.3, u * 0.75, DIM);
            }
        }
        let bw = u * 10.0;
        if button(&mut input, Rect::new((w - bw) / 2.0, h - u * 3.2, bw, u * 2.2), "Title", ButtonStyle::Primary, u) {
            self.campaign = None;
            return Some(Screen::Title);
        }
        None
    }

    fn title(&mut self) -> Option<Screen> {
        let (w, h) = (screen_width(), screen_height());
        let u = ui_unit();
        let mut input = Input::poll();

        for i in 0..60 {
            let fx = ((i * 7919) % 1000) as f32 / 1000.0;
            let fy = ((i * 104_729) % 1000) as f32 / 1000.0;
            let y = (fy * h - self.time * (10.0 + (i % 5) as f32 * 6.0)).rem_euclid(h);
            let s = 2.0 + (i % 3) as f32;
            let c = if i % 4 == 0 { Color::new(1.0, 0.6, 0.2, 0.8) } else { Color::new(0.8, 0.8, 1.0, 0.35) };
            draw_rectangle(fx * w, y, s, s, c);
        }

        let tile = (u * 2.6).floor();
        let gy = h - tile;
        let cols = (w / tile).ceil() as i32 + 1;
        for x in 0..cols {
            let t = match x % 7 {
                2 => Terrain::Forest,
                5 => Terrain::Mountain,
                _ => Terrain::Plains,
            };
            gfx::draw_terrain(t, x as f32 * tile, gy, tile, x, 0, self.time, false);
        }
        let party = [
            (Class::Lord, WeaponKind::Sword),
            (Class::Lancer, WeaponKind::Lance),
            (Class::Archer, WeaponKind::Bow),
            (Class::Mage, WeaponKind::Magic),
            (Class::Cleric, WeaponKind::Staff),
            (Class::Knight, WeaponKind::Lance),
            (Class::Fighter, WeaponKind::Axe),
        ];
        let march = (self.time * tile * 0.6).rem_euclid(w + tile * 10.0) - tile * 8.0;
        for (i, (class, weapon)) in party.iter().enumerate() {
            let x = march - i as f32 * tile * 1.1;
            let bob = ((self.time * 6.0) as i32 + i as i32) % 2 == 0;
            gfx::draw_unit(UnitLook::plain(*class, Team::Player, *weapon), x, gy - tile * 0.7, tile, false, 1.0, bob);
        }

        let cx = w / 2.0;
        let ty = h * 0.17;
        let big = u * 3.6;
        text_centered("EMBERFALL", cx + 4.0, ty + 4.0, big, rgb(120, 40, 20));
        text_centered("EMBERFALL", cx, ty, big, ACCENT);
        text_centered("~ TACTICS ~", cx, ty + u * 1.8, u * 1.3, INK);

        let bw = (u * 14.0).min(w - u * 2.0);
        let bh = (u * 2.1).floor();
        let gap = u * 0.5;
        let mut y = ty + u * 3.0;
        let bx = cx - bw / 2.0;

        if self.show_help {
            let hw = (u * 27.0).min(w - u * 2.0);
            let hh = u * 1.05 * HELP.len() as f32 + u * 1.2;
            let r = Rect::new(cx - hw / 2.0, y, hw, hh);
            frame(r);
            for (i, line) in HELP.iter().enumerate() {
                text(line, r.x + u, r.y + u * 1.4 + i as f32 * u * 1.05, u * 0.8, INK);
            }
            y += hh + u * 0.5;
            if button(&mut input, Rect::new(bx, y.min(h - bh - u), bw, bh), "Back", ButtonStyle::Normal, u) {
                self.show_help = false;
            }
            return None;
        }

        if self.confirm_new {
            text_centered("Start over? Your saved progress will be lost.", cx, y + u, u * 0.9, BAD);
            y += u * 2.0;
            if button(&mut input, Rect::new(bx, y, bw, bh), "Yes, new game", ButtonStyle::Danger, u) {
                self.confirm_new = false;
                return Some(self.new_game());
            }
            y += bh + gap;
            if button(&mut input, Rect::new(bx, y, bw, bh), "Cancel", ButtonStyle::Normal, u) {
                self.confirm_new = false;
            }
            return None;
        }

        let mut next = None;
        if let Some(saved) = &self.saved {
            let label = format!("Continue - Chapter {}/{}", saved.chapter + 1, CHAPTER_COUNT);
            if button(&mut input, Rect::new(bx, y, bw, bh), &label, ButtonStyle::Primary, u) {
                let mut c = saved.clone();
                c.begin_chapter();
                self.campaign = Some(c);
                next = Some(Screen::Prep(PrepScreen::new()));
            }
            y += bh + gap;
        }
        let style = if self.saved.is_some() { ButtonStyle::Normal } else { ButtonStyle::Primary };
        if button(&mut input, Rect::new(bx, y, bw, bh), "New Game", style, u) {
            if self.saved.is_some() {
                self.confirm_new = true;
            } else {
                next = Some(self.new_game());
            }
        }
        y += bh + gap;
        let label = if self.permadeath { "Permadeath: ON" } else { "Permadeath: OFF" };
        let style = if self.permadeath { ButtonStyle::Danger } else { ButtonStyle::Normal };
        if button(&mut input, Rect::new(bx, y, bw, bh), label, style, u) {
            self.permadeath = !self.permadeath;
        }
        y += bh + u * 0.2;
        let note = if self.permadeath { "New game: fallen allies are lost forever." } else { "New game: fallen allies retreat and return." };
        text_centered(note, cx, y + u * 0.8, u * 0.7, DIM);
        y += u * 1.4;
        if button(&mut input, Rect::new(bx, y, bw, bh), "How to Play", ButtonStyle::Normal, u) {
            self.show_help = true;
        }
        next
    }

    fn new_game(&mut self) -> Screen {
        let mut c = Campaign::new(self.permadeath);
        c.begin_chapter();
        storage::save(&c);
        self.saved = Some(c.clone());
        let s = Self::intro_story(&c);
        self.campaign = Some(c);
        s
    }
}
