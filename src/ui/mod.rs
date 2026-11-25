pub mod battle_screen;
pub mod gfx;
pub mod widgets;

use macroquad::prelude::*;

use battle_screen::{BattleExit, BattleScreen};
use gfx::{rgb, UnitLook};
use widgets::*;

use crate::logic::terrain::Terrain;
use crate::logic::unit::{Class, Team, WeaponKind};

enum Screen {
    Title,
    Battle(Box<BattleScreen>),
}

pub struct App {
    screen: Screen,
    permadeath: bool,
    show_help: bool,
    time: f32,
}

fn seed() -> u64 {
    (macroquad::miniquad::date::now() * 1000.0) as u64
}

const HELP: [&str; 8] = [
    "Tap a blue unit, then a blue tile to move.",
    "Then choose Attack, Heal or Wait.",
    "Always check the forecast before attacking.",
    "Sword beats Axe, Axe beats Lance, Lance beats Sword.",
    "4+ more Speed than the foe = strike twice.",
    "Forests, mountains and forts boost Def and Avoid.",
    "Forts restore 20% HP at the start of your phase.",
    "Defeat the boss to win. If Kael falls, you lose.",
];

impl App {
    pub fn new() -> App {
        App { screen: Screen::Title, permadeath: true, show_help: false, time: 0.0 }
    }

    pub fn frame(&mut self) {
        self.time += get_frame_time();
        clear_background(rgb(16, 16, 30));
        let next = match &mut self.screen {
            Screen::Title => {
                if self.title() {
                    Some(Screen::Battle(Box::new(BattleScreen::new(seed(), self.permadeath))))
                } else {
                    None
                }
            }
            Screen::Battle(b) => match b.frame() {
                BattleExit::Stay => None,
                BattleExit::Title => Some(Screen::Title),
                BattleExit::Retry => Some(Screen::Battle(Box::new(BattleScreen::new(seed(), self.permadeath)))),
            },
        };
        if let Some(s) = next {
            self.screen = s;
        }
    }

    /// Returns true when the player starts the battle.
    fn title(&mut self) -> bool {
        let (w, h) = (screen_width(), screen_height());
        let u = (w.min(h) / 26.0).clamp(14.0, 48.0);
        let mut input = Input::poll();

        // Night sky with drifting embers.
        for i in 0..60 {
            let fx = ((i * 7919) % 1000) as f32 / 1000.0;
            let fy = ((i * 104_729) % 1000) as f32 / 1000.0;
            let y = (fy * h - self.time * (10.0 + (i % 5) as f32 * 6.0)).rem_euclid(h);
            let s = 2.0 + (i % 3) as f32;
            let c = if i % 4 == 0 { Color::new(1.0, 0.6, 0.2, 0.8) } else { Color::new(0.8, 0.8, 1.0, 0.35) };
            draw_rectangle(fx * w, y, s, s, c);
        }

        // Ground strip with marching units.
        let tile = (u * 2.6).floor();
        let gy = h - tile;
        let cols = (w / tile).ceil() as i32 + 1;
        for x in 0..cols {
            let t = match x % 7 {
                2 => Terrain::Forest,
                5 => Terrain::Mountain,
                _ => Terrain::Plains,
            };
            gfx::draw_terrain(t, x as f32 * tile, gy, tile, x, 0, self.time);
        }
        let party = [
            (Class::Lord, WeaponKind::Sword),
            (Class::Lancer, WeaponKind::Lance),
            (Class::Archer, WeaponKind::Bow),
            (Class::Mage, WeaponKind::Magic),
            (Class::Cleric, WeaponKind::Staff),
        ];
        let march = (self.time * tile * 0.6).rem_euclid(w + tile * 8.0) - tile * 6.0;
        for (i, (class, weapon)) in party.iter().enumerate() {
            let x = march - i as f32 * tile * 1.1;
            let bob = ((self.time * 6.0) as i32 + i as i32) % 2 == 0;
            gfx::draw_unit(UnitLook { class: *class, team: Team::Player, weapon: *weapon, boss: false }, x, gy - tile * 0.7, tile, false, 1.0, bob);
        }

        // Title
        let cx = w / 2.0;
        let ty = h * 0.2;
        let big = u * 3.6;
        text_centered("EMBERFALL", cx + 4.0, ty + 4.0, big, rgb(120, 40, 20));
        text_centered("EMBERFALL", cx, ty, big, ACCENT);
        text_centered("~ TACTICS ~", cx, ty + u * 1.8, u * 1.3, INK);

        let bw = (u * 14.0).min(w - u * 2.0);
        let bh = (u * 2.2).floor();
        let mut y = ty + u * 3.4;
        if self.show_help {
            let hw = (u * 26.0).min(w - u * 2.0);
            let hh = u * 1.1 * HELP.len() as f32 + u * 1.2;
            let r = Rect::new(cx - hw / 2.0, y, hw, hh);
            frame(r);
            for (i, line) in HELP.iter().enumerate() {
                text(line, r.x + u, r.y + u * 1.4 + i as f32 * u * 1.1, u * 0.82, INK);
            }
            y += hh + u * 0.6;
            if button(&mut input, Rect::new(cx - bw / 2.0, y, bw, bh), "Back", ButtonStyle::Normal, u) {
                self.show_help = false;
            }
            return false;
        }

        let mut start = false;
        if button(&mut input, Rect::new(cx - bw / 2.0, y, bw, bh), "Start Battle", ButtonStyle::Primary, u) {
            start = true;
        }
        y += bh + u * 0.6;
        let label = if self.permadeath { "Permadeath: ON" } else { "Permadeath: OFF" };
        let style = if self.permadeath { ButtonStyle::Danger } else { ButtonStyle::Normal };
        if button(&mut input, Rect::new(cx - bw / 2.0, y, bw, bh), label, style, u) {
            self.permadeath = !self.permadeath;
        }
        y += bh + u * 0.3;
        let note = if self.permadeath { "Fallen allies are lost forever." } else { "Fallen allies retreat and return later." };
        text_centered(note, cx, y + u * 0.8, u * 0.75, DIM);
        y += u * 1.6;
        if button(&mut input, Rect::new(cx - bw / 2.0, y, bw, bh), "How to Play", ButtonStyle::Normal, u) {
            self.show_help = true;
        }
        let _ = input.any();
        start
    }
}
