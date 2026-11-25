//! Story scenes between battles: chapter title card and dialogue.

use std::collections::HashMap;

use macroquad::prelude::*;

use super::battle_screen::dialogue_box;
use super::gfx::{self, UnitLook};
use super::widgets::*;
use crate::logic::campaign::{chapter, Line};
use crate::logic::map::Map;
use crate::logic::unit::Unit;
use crate::logic::Pos;

pub struct StoryScreen {
    lines: Vec<Line>,
    idx: usize,
    t: f32,
    time: f32,
    heading: Option<String>,
    map: Map,
    looks: HashMap<String, UnitLook>,
}

impl StoryScreen {
    pub fn new(lines: Vec<Line>, chapter_index: usize, heading: Option<String>, army: &[Unit]) -> StoryScreen {
        let def = chapter(chapter_index);
        let mut looks = HashMap::new();
        for u in def.enemies.iter().chain(def.joins.iter()).chain(army.iter()) {
            looks.insert(u.name.clone(), UnitLook::of(u));
        }
        StoryScreen { lines, idx: 0, t: 0.0, time: 0.0, heading, map: Map::from_rows(&def.map), looks }
    }

    /// Returns true when the scene is over.
    pub fn frame(&mut self) -> bool {
        let dt = get_frame_time().min(0.05);
        self.t += dt;
        self.time += dt;
        let (w, h) = (screen_width(), screen_height());
        let u = ui_unit();
        let mut input = Input::poll();

        // Backdrop: the chapter's map, darkened.
        let tile = (w / self.map.width as f32).max(h / self.map.height as f32).ceil();
        for y in 0..self.map.height {
            for x in 0..self.map.width {
                gfx::draw_terrain(self.map.at(Pos::new(x, y)), x as f32 * tile, y as f32 * tile, tile, x, y, self.time, false);
            }
        }
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.02, 0.02, 0.08, 0.6));

        if let Some(hd) = &self.heading {
            let size = (u * 2.2).min(w * 0.9 / (hd.len() as f32 * 0.55).max(1.0));
            draw_rectangle(0.0, h * 0.12, w, size * 1.8, Color::new(0.0, 0.0, 0.0, 0.5));
            text_centered(hd, w / 2.0, h * 0.12 + size * 1.2, size, ACCENT);
        }

        let skip = Rect::new(w - u * 6.0, u * 0.5, u * 5.5, u * 1.8);
        if button(&mut input, skip, "Skip", ButtonStyle::Normal, u * 0.8) {
            return true;
        }

        let Some(line) = self.lines.get(self.idx) else { return true };
        let look = self.looks.get(line.speaker).copied();
        let area = Rect::new(0.0, 0.0, w, h);
        dialogue_box(area, u, line.speaker, line.text, look, self.t);
        if input.take().is_some() {
            let full = (line.text.len() as f32) / 90.0;
            if self.t < full {
                self.t = 100.0; // reveal
            } else {
                self.idx += 1;
                self.t = 0.0;
            }
        }
        self.idx >= self.lines.len()
    }
}
