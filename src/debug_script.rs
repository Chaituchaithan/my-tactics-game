//! Desktop-only automation for screenshots / smoke tests.
//!
//! EMBERFALL_SCRIPT="30 shot title; 40 tap 0.5 0.52; 120 tile 1 10; 140 shot selected"
//!   <frame> shot <name>      save <EMBERFALL_SHOT_DIR>/<name>.png
//!   <frame> tap <fx> <fy>    tap at a fraction of the screen
//!   <frame> tile <x> <y>     tap the centre of a map tile
//!   <frame> quit

use macroquad::prelude::*;

use crate::ui::widgets::{inject_tap, Layout};

pub struct Script {
    steps: Vec<(u64, String)>,
    frame: u64,
    dir: String,
}

impl Script {
    pub fn from_env() -> Option<Script> {
        let raw = std::env::var("EMBERFALL_SCRIPT").ok()?;
        let dir = std::env::var("EMBERFALL_SHOT_DIR").unwrap_or_else(|_| ".".into());
        let steps = raw
            .split(';')
            .filter_map(|s| {
                let s = s.trim();
                let (f, rest) = s.split_once(' ')?;
                Some((f.parse().ok()?, rest.to_string()))
            })
            .collect();
        Some(Script { steps, frame: 0, dir })
    }

    /// Runs before the frame's update. Returns false to quit.
    pub fn before(&mut self) -> bool {
        self.frame += 1;
        for (f, cmd) in &self.steps {
            if *f != self.frame {
                continue;
            }
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            match parts.as_slice() {
                ["tap", x, y] => inject_tap(vec2(x.parse::<f32>().unwrap() * screen_width(), y.parse::<f32>().unwrap() * screen_height())),
                ["tile", x, y] => {
                    let l = Layout::compute(16, 12, 1, Vec2::ZERO);
                    let r = l.tile_rect(x.parse().unwrap(), y.parse().unwrap());
                    inject_tap(r.center());
                }
                ["quit"] => return false,
                _ => {}
            }
        }
        true
    }

    /// Runs after drawing, before presenting.
    pub fn after(&self) {
        for (f, cmd) in &self.steps {
            if *f == self.frame {
                if let Some(name) = cmd.strip_prefix("shot ") {
                    get_screen_data().export_png(&format!("{}/{}.png", self.dir, name.trim()));
                }
            }
        }
    }
}
