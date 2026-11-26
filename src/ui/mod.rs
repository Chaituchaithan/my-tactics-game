pub mod audio;
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

use crate::logic::campaign::{chapter, Campaign, Difficulty, CHAPTER_COUNT, EPILOGUE};
use crate::logic::profile::{Profile, ACHIEVEMENTS};
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

#[derive(PartialEq)]
enum TitleView {
    Main,
    Help,
    Settings,
    Achievements,
    ConfirmNew,
}

pub struct App {
    screen: Screen,
    campaign: Option<Campaign>,
    saved: Option<Campaign>,
    profile: Profile,
    permadeath: bool,
    difficulty: Difficulty,
    view: TitleView,
    /// Achievement pop-ups: (name, seconds left).
    toasts: Vec<(String, f32)>,
    time: f32,
}

fn seed() -> u64 {
    (macroquad::miniquad::date::now() * 1000.0) as u64
}

const HELP: [&str; 12] = [
    "Tap a blue unit, then a blue tile to move. Drag to pan.",
    "Then choose Attack, Heal, Items, Talk, Visit, Open or Wait.",
    "Check the forecast before attacking.",
    "Sword beats Axe, Axe beats Lance, Lance beats Sword.",
    "4+ more Speed than the foe = strike twice.",
    "Forests, mountains and forts boost Def and Avoid.",
    "Bows deal triple damage to fliers. Horses can't climb.",
    "Thieves open chests and doors and can Steal.",
    "Weapons wear out. Buy or forge new ones in camp.",
    "Allies who fight side by side build supports.",
    "Level 10+ units can promote with a Master Seal.",
    "If Kael falls, the game is over.",
];

impl App {
    pub fn new(profile: Profile) -> App {
        let saved = storage::load();
        App {
            screen: Screen::Title,
            campaign: None,
            saved,
            profile,
            permadeath: true,
            difficulty: Difficulty::Normal,
            view: TitleView::Main,
            toasts: Vec::new(),
            time: 0.0,
        }
    }

    pub fn music_enabled(p: &Profile) -> (bool, bool) {
        (p.music, p.sfx)
    }

    fn intro_story(c: &Campaign) -> Screen {
        let def = chapter(c.chapter);
        let heading = format!("Chapter {}: {}", c.chapter + 1, def.title);
        Screen::Story(Box::new(StoryScreen::new(def.intro, c.chapter, Some(heading), &c.army)), AfterStory::Prep)
    }

    fn start_battle(c: &Campaign) -> Screen {
        Screen::Battle(Box::new(BattleScreen::new(c.battle(seed()), Some(c.chapter), c.battle_scenes)))
    }

    fn check_achievements(&mut self) {
        if let Some(c) = &self.campaign {
            let new = self.profile.check(c);
            if !new.is_empty() {
                for n in new {
                    self.toasts.push((n.to_string(), 3.5));
                }
                audio::sfx(audio::Sfx::Achievement);
                storage::save_profile(&self.profile);
            }
        }
    }

    pub fn frame(&mut self) {
        let dt = get_frame_time();
        self.time += dt;
        clear_background(rgb(16, 16, 30));
        match self.screen {
            Screen::Title | Screen::Ending => audio::music(audio::Track::Title),
            Screen::Story(..) | Screen::Prep(_) => audio::music(audio::Track::Story),
            Screen::Battle(_) => {}
        }
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
                let exit = p.frame(c, &mut self.profile);
                match exit {
                    PrepExit::Stay => None,
                    PrepExit::Begin => {
                        storage::save(c);
                        Some(Self::start_battle(c))
                    }
                    PrepExit::Skirmish => {
                        storage::save(c);
                        Some(Screen::Battle(Box::new(BattleScreen::new(c.skirmish(seed()), None, c.battle_scenes))))
                    }
                    PrepExit::Title => {
                        storage::save(c);
                        self.saved = Some(c.clone());
                        storage::save_profile(&self.profile);
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
                BattleExit::Camp => Some(Screen::Prep(PrepScreen::new())),
                BattleExit::Retry => Some(Self::start_battle(self.campaign.as_ref().unwrap())),
                BattleExit::Victory => {
                    let c = self.campaign.as_mut().unwrap();
                    let st = &b.battle().stats;
                    self.profile.chests += st.chests;
                    self.profile.steals += st.steals;
                    self.profile.promotions += st.promotions;
                    if b.is_skirmish() {
                        c.finish_skirmish(b.battle());
                        self.profile.skirmish_wins += 1;
                        storage::save(c);
                        self.saved = Some(c.clone());
                        storage::save_profile(&self.profile);
                        Some(Screen::Prep(PrepScreen::new()))
                    } else {
                        let finished_ch = b.chapter();
                        c.finish_chapter(b.battle());
                        let outro = chapter(finished_ch).outro;
                        let story = Box::new(StoryScreen::new(outro, finished_ch, None, &c.army));
                        if c.finished {
                            storage::delete();
                            self.saved = None;
                            self.profile.completed = Some(c.clone());
                            storage::save_profile(&self.profile);
                            Some(Screen::Story(story, AfterStory::Ending))
                        } else {
                            c.begin_chapter();
                            storage::save(c);
                            self.saved = Some(c.clone());
                            storage::save_profile(&self.profile);
                            Some(Screen::Story(story, AfterStory::Intro))
                        }
                    }
                }
            },
            Screen::Ending => self.ending(),
        };
        if let Some(s) = next {
            self.screen = s;
        }
        self.check_achievements();
        self.draw_toasts(dt);
    }

    fn draw_toasts(&mut self, dt: f32) {
        let u = ui_unit();
        let w = screen_width();
        for (i, (name, t)) in self.toasts.iter_mut().enumerate() {
            *t -= dt;
            let a = t.clamp(0.0, 1.0);
            let bw = (text_w(name, u) + u * 4.0).max(u * 14.0);
            let r = Rect::new(w - bw - u * 0.5, u * 0.5 + i as f32 * u * 3.2, bw, u * 2.8);
            draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.1, 0.08, 0.2, 0.92 * a));
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 3.0, Color::new(0.93, 0.76, 0.3, a));
            text("Achievement unlocked!", r.x + u * 0.6, r.y + u * 1.1, u * 0.7, Color::new(0.9, 0.9, 0.9, a));
            text(name, r.x + u * 0.6, r.y + u * 2.2, u, Color::new(0.93, 0.76, 0.3, a));
        }
        self.toasts.retain(|(_, t)| *t > 0.0);
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
        text_centered("New Game+ is now available from the title screen.", w / 2.0, h - u * 4.2, u * 0.75, ACCENT);
        let bw = u * 10.0;
        if button(&mut input, Rect::new((w - bw) / 2.0, h - u * 3.2, bw, u * 2.2), "Title", ButtonStyle::Primary, u) {
            self.campaign = None;
            return Some(Screen::Title);
        }
        None
    }

    fn title_backdrop(&self, w: f32, h: f32, u: f32) {
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
            (Class::Cavalier, WeaponKind::Lance),
            (Class::Lancer, WeaponKind::Lance),
            (Class::Archer, WeaponKind::Bow),
            (Class::Mage, WeaponKind::Magic),
            (Class::PegasusKnight, WeaponKind::Lance),
            (Class::Cleric, WeaponKind::Staff),
            (Class::Thief, WeaponKind::Sword),
        ];
        let march = (self.time * tile * 0.6).rem_euclid(w + tile * 11.0) - tile * 9.0;
        for (i, (class, weapon)) in party.iter().enumerate() {
            let x = march - i as f32 * tile * 1.1;
            let bob = ((self.time * 6.0) as i32 + i as i32) % 2 == 0;
            gfx::draw_unit(UnitLook::plain(*class, Team::Player, *weapon), x, gy - tile * 0.7, tile, false, 1.0, bob);
        }
        let cx = w / 2.0;
        let ty = h * 0.15;
        let big = u * 3.6;
        text_centered("EMBERFALL", cx + 4.0, ty + 4.0, big, rgb(120, 40, 20));
        text_centered("EMBERFALL", cx, ty, big, ACCENT);
        text_centered("~ TACTICS ~", cx, ty + u * 1.8, u * 1.3, INK);
    }

    fn title(&mut self) -> Option<Screen> {
        let (w, h) = (screen_width(), screen_height());
        let u = ui_unit();
        let mut input = Input::poll();
        self.title_backdrop(w, h, u);

        let cx = w / 2.0;
        let full = (u * 18.0).min(w - u * 2.0);
        let gap = u * 0.45;
        let half = (full - gap) / 2.0;
        let bh = (u * 2.0).floor();
        let bx = cx - full / 2.0;
        let mut y = h * 0.15 + u * 2.9;

        match self.view {
            TitleView::Help => {
                let hw = (u * 28.0).min(w - u * 2.0);
                let hh = u * 1.0 * HELP.len() as f32 + u * 1.2;
                let r = Rect::new(cx - hw / 2.0, y, hw, hh);
                frame(r);
                for (i, line) in HELP.iter().enumerate() {
                    text(line, r.x + u, r.y + u * 1.4 + i as f32 * u, u * 0.75, INK);
                }
                y += hh + u * 0.4;
                if button(&mut input, Rect::new(bx, y.min(h - bh - u), full, bh), "Back", ButtonStyle::Normal, u) {
                    self.view = TitleView::Main;
                }
                return None;
            }
            TitleView::Settings => {
                let m = if self.profile.music { "Music: On" } else { "Music: Off" };
                if button(&mut input, Rect::new(bx, y, full, bh), m, ButtonStyle::Normal, u) {
                    self.profile.music = !self.profile.music;
                    audio::set_enabled(self.profile.music, self.profile.sfx);
                    storage::save_profile(&self.profile);
                }
                y += bh + gap;
                let s = if self.profile.sfx { "Sound Effects: On" } else { "Sound Effects: Off" };
                if button(&mut input, Rect::new(bx, y, full, bh), s, ButtonStyle::Normal, u) {
                    self.profile.sfx = !self.profile.sfx;
                    audio::set_enabled(self.profile.music, self.profile.sfx);
                    storage::save_profile(&self.profile);
                }
                y += bh + gap;
                if button(&mut input, Rect::new(bx, y, full, bh), "Back", ButtonStyle::Normal, u) {
                    self.view = TitleView::Main;
                }
                return None;
            }
            TitleView::Achievements => {
                let aw = (u * 32.0).min(w - u * 2.0);
                let rows = ACHIEVEMENTS.len().div_ceil(2);
                let rh = u * 1.75;
                let r = Rect::new(cx - aw / 2.0, y - u * 0.4, aw, rows as f32 * rh + u * 2.4);
                frame(r);
                let got = ACHIEVEMENTS.iter().filter(|a| self.profile.has(a.id)).count();
                text(&format!("Achievements  {}/{}", got, ACHIEVEMENTS.len()), r.x + u, r.y + u * 1.3, u * 0.9, ACCENT);
                for (i, a) in ACHIEVEMENTS.iter().enumerate() {
                    let col = (i / rows) as f32;
                    let row = (i % rows) as f32;
                    let x = r.x + u + col * (aw / 2.0);
                    let yy = r.y + u * 2.6 + row * rh;
                    let on = self.profile.has(a.id);
                    text(if on { "*" } else { "-" }, x, yy, u * 0.8, if on { ACCENT } else { DIM });
                    text(a.name, x + u * 0.8, yy, u * 0.72, if on { INK } else { DIM });
                    text(a.desc, x + u * 0.8, yy + u * 0.7, u * 0.5, DIM);
                }
                let by = (r.y + r.h + gap).min(h - bh - u * 0.3);
                if button(&mut input, Rect::new(bx, by, full, bh), "Back", ButtonStyle::Normal, u) {
                    self.view = TitleView::Main;
                }
                return None;
            }
            TitleView::ConfirmNew => {
                text_centered("Start over? Your saved progress will be lost.", cx, y + u, u * 0.9, BAD);
                y += u * 2.0;
                if button(&mut input, Rect::new(bx, y, full, bh), "Yes, new game", ButtonStyle::Danger, u) {
                    self.view = TitleView::Main;
                    return Some(self.new_game(None));
                }
                y += bh + gap;
                if button(&mut input, Rect::new(bx, y, full, bh), "Cancel", ButtonStyle::Normal, u) {
                    self.view = TitleView::Main;
                }
                return None;
            }
            TitleView::Main => {}
        }

        let mut next = None;
        if let Some(saved) = &self.saved {
            let ng = if saved.ng_plus > 0 { format!(" (NG+{})", saved.ng_plus) } else { String::new() };
            let label = format!("Continue - Chapter {}/{}{}", saved.chapter + 1, CHAPTER_COUNT, ng);
            if button(&mut input, Rect::new(bx, y, full, bh), &label, ButtonStyle::Primary, u) {
                let mut c = saved.clone();
                c.begin_chapter();
                self.campaign = Some(c);
                next = Some(Screen::Prep(PrepScreen::new()));
            }
            y += bh + gap;
        }
        let style = if self.saved.is_some() { ButtonStyle::Normal } else { ButtonStyle::Primary };
        if button(&mut input, Rect::new(bx, y, half, bh), "New Game", style, u) {
            if self.saved.is_some() {
                self.view = TitleView::ConfirmNew;
            } else {
                next = Some(self.new_game(None));
            }
        }
        let ng_style = if self.profile.completed.is_some() { ButtonStyle::Primary } else { ButtonStyle::Disabled };
        if button(&mut input, Rect::new(bx + half + gap, y, half, bh), "New Game+", ng_style, u) {
            let prev = self.profile.completed.clone();
            next = Some(self.new_game(prev));
        }
        y += bh + gap;
        if button(&mut input, Rect::new(bx, y, half, bh), &format!("Difficulty: {}", self.difficulty.name()), ButtonStyle::Normal, u) {
            self.difficulty = self.difficulty.next();
        }
        let label = if self.permadeath { "Permadeath: ON" } else { "Permadeath: OFF" };
        let st = if self.permadeath { ButtonStyle::Danger } else { ButtonStyle::Normal };
        if button(&mut input, Rect::new(bx + half + gap, y, half, bh), label, st, u) {
            self.permadeath = !self.permadeath;
        }
        y += bh + u * 0.15;
        let note = match self.difficulty {
            Difficulty::Easy => "Easy: weaker foes.",
            Difficulty::Normal => "Normal: the intended challenge.",
            Difficulty::Hard => "Hard: tougher, deadlier foes.",
        };
        let pd = if self.permadeath { " Fallen allies are lost forever." } else { " Fallen allies return." };
        text_centered(&format!("{}{}", note, pd), cx, y + u * 0.75, u * 0.65, DIM);
        y += u * 1.2;
        if button(&mut input, Rect::new(bx, y, half, bh), "Achievements", ButtonStyle::Normal, u) {
            self.view = TitleView::Achievements;
        }
        if button(&mut input, Rect::new(bx + half + gap, y, half, bh), "Settings", ButtonStyle::Normal, u) {
            self.view = TitleView::Settings;
        }
        y += bh + gap;
        if button(&mut input, Rect::new(bx, y, full, bh), "How to Play", ButtonStyle::Normal, u) {
            self.view = TitleView::Help;
        }
        next
    }

    /// Starts a fresh campaign, or New Game+ from a finished one.
    fn new_game(&mut self, prev: Option<Campaign>) -> Screen {
        let mut c = match prev {
            Some(p) => Campaign::new_game_plus(&p),
            None => Campaign::new_with(self.permadeath, self.difficulty),
        };
        c.begin_chapter();
        storage::save(&c);
        self.saved = Some(c.clone());
        let s = Self::intro_story(&c);
        self.campaign = Some(c);
        s
    }
}
