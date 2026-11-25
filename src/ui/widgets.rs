//! Tiny immediate-mode UI helpers: tap input, layout, buttons, text.

use macroquad::prelude::*;

use super::gfx::rgb;

/// One tap (touch start or left click) per frame, consumed by the first
/// widget that claims it.
pub struct Input {
    tap: Option<Vec2>,
}

thread_local! {
    static INJECTED: std::cell::Cell<Option<Vec2>> = const { std::cell::Cell::new(None) };
    static SCRIPTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Queue a synthetic tap for the next frame (used by the desktop debug script).
pub fn inject_tap(p: Vec2) {
    SCRIPTED.with(|c| c.set(true));
    INJECTED.with(|c| c.set(Some(p)));
}

impl Input {
    pub fn poll() -> Input {
        if let Some(p) = INJECTED.with(|c| c.take()) {
            return Input { tap: Some(p) };
        }
        if SCRIPTED.with(|c| c.get()) {
            return Input { tap: None };
        }
        let mut tap = touches().into_iter().find(|t| t.phase == TouchPhase::Started).map(|t| t.position);
        if tap.is_none() && is_mouse_button_pressed(MouseButton::Left) {
            let (x, y) = mouse_position();
            tap = Some(vec2(x, y));
        }
        Input { tap }
    }

    pub fn tapped_in(&mut self, r: Rect) -> bool {
        match self.tap {
            Some(p) if r.contains(p) => {
                self.tap = None;
                true
            }
            _ => false,
        }
    }

    pub fn take(&mut self) -> Option<Vec2> {
        self.tap.take()
    }

    pub fn any(&self) -> bool {
        self.tap.is_some()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub ox: f32,
    pub oy: f32,
    pub tile: f32,
    pub map_w: f32,
    pub map_h: f32,
    pub panel: Rect,
    /// Base UI unit (font size); scales with screen.
    pub u: f32,
}

impl Layout {
    pub fn compute(map_tiles_w: i32, map_tiles_h: i32) -> Layout {
        let (w, h) = (screen_width(), screen_height());
        let mw = map_tiles_w as f32;
        let mh = map_tiles_h as f32;
        let u = (w.min(h) / 26.0).clamp(14.0, 48.0);
        let margin = (u * 0.3).floor();
        if w >= h * 1.05 {
            let panel_min = (w * 0.26).max(u * 11.0).min(w * 0.45);
            let tile = ((w - panel_min - margin * 2.0) / mw).min((h - margin * 2.0) / mh).floor().max(8.0);
            let map_w = tile * mw;
            let map_h = tile * mh;
            let ox = margin;
            let oy = ((h - map_h) / 2.0).floor();
            let px = ox + map_w + margin;
            Layout { ox, oy, tile, map_w, map_h, panel: Rect::new(px, margin, w - px - margin, h - margin * 2.0), u }
        } else {
            let tile = ((w - margin * 2.0) / mw).min(h * 0.62 / mh).floor().max(8.0);
            let map_w = tile * mw;
            let map_h = tile * mh;
            let ox = ((w - map_w) / 2.0).floor();
            let oy = margin;
            let py = oy + map_h + margin;
            Layout { ox, oy, tile, map_w, map_h, panel: Rect::new(margin, py, w - margin * 2.0, h - py - margin), u }
        }
    }

    pub fn tile_rect(&self, x: f32, y: f32) -> Rect {
        Rect::new(self.ox + x * self.tile, self.oy + y * self.tile, self.tile, self.tile)
    }

    pub fn tile_at(&self, p: Vec2) -> Option<(i32, i32)> {
        let x = ((p.x - self.ox) / self.tile).floor();
        let y = ((p.y - self.oy) / self.tile).floor();
        let tw = (self.map_w / self.tile).round();
        let th = (self.map_h / self.tile).round();
        if x >= 0.0 && y >= 0.0 && x < tw && y < th {
            Some((x as i32, y as i32))
        } else {
            None
        }
    }
}

pub const INK: Color = Color::new(0.95, 0.93, 0.86, 1.0);
pub const DIM: Color = Color::new(0.68, 0.66, 0.62, 1.0);
pub const ACCENT: Color = Color::new(0.93, 0.76, 0.3, 1.0);
pub const GOOD: Color = Color::new(0.45, 0.85, 0.45, 1.0);
pub const BAD: Color = Color::new(0.95, 0.4, 0.35, 1.0);
pub const ALLY: Color = Color::new(0.45, 0.65, 1.0, 1.0);

pub fn panel_bg() -> Color {
    rgb(28, 30, 52)
}

/// Window box with a retro double border.
pub fn frame(r: Rect) {
    draw_rectangle(r.x, r.y, r.w, r.h, panel_bg());
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 4.0, rgb(200, 180, 120));
    draw_rectangle_lines(r.x + 4.0, r.y + 4.0, r.w - 8.0, r.h - 8.0, 2.0, rgb(80, 70, 110));
}

pub fn text(s: &str, x: f32, y: f32, size: f32, c: Color) {
    // Drop shadow for readability on top of the map.
    draw_text(s, x + 2.0, y + 2.0, size, Color::new(0.0, 0.0, 0.0, 0.6 * c.a));
    draw_text(s, x, y, size, c);
}

pub fn text_w(s: &str, size: f32) -> f32 {
    measure_text(s, None, size as u16, 1.0).width
}

pub fn text_centered(s: &str, cx: f32, y: f32, size: f32, c: Color) {
    text(s, cx - text_w(s, size) / 2.0, y, size, c);
}

pub fn text_right(s: &str, rx: f32, y: f32, size: f32, c: Color) {
    text(s, rx - text_w(s, size), y, size, c);
}

#[derive(Clone, Copy, PartialEq)]
pub enum ButtonStyle {
    Normal,
    Primary,
    Danger,
    Disabled,
}

pub fn button(input: &mut Input, r: Rect, label: &str, style: ButtonStyle, u: f32) -> bool {
    let (bg, border) = match style {
        ButtonStyle::Normal => (rgb(52, 60, 110), rgb(150, 160, 220)),
        ButtonStyle::Primary => (rgb(40, 110, 70), rgb(140, 230, 160)),
        ButtonStyle::Danger => (rgb(130, 44, 44), rgb(240, 150, 140)),
        ButtonStyle::Disabled => (rgb(50, 50, 60), rgb(90, 90, 100)),
    };
    draw_rectangle(r.x, r.y, r.w, r.h, bg);
    draw_rectangle(r.x, r.y + r.h - 4.0, r.w, 4.0, Color::new(0.0, 0.0, 0.0, 0.35));
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 3.0, border);
    let size = u * 1.0;
    let c = if style == ButtonStyle::Disabled { DIM } else { INK };
    text_centered(label, r.x + r.w / 2.0, r.y + r.h / 2.0 + size * 0.33, size, c);
    style != ButtonStyle::Disabled && input.tapped_in(r)
}

pub fn hp_bar(x: f32, y: f32, w: f32, h: f32, hp: i32, max: i32) {
    let frac = if max > 0 { (hp as f32 / max as f32).clamp(0.0, 1.0) } else { 0.0 };
    draw_rectangle(x, y, w, h, rgb(20, 20, 24));
    let c = if frac > 0.5 {
        rgb(90, 210, 100)
    } else if frac > 0.25 {
        rgb(230, 200, 60)
    } else {
        rgb(230, 70, 60)
    };
    draw_rectangle(x + 1.0, y + 1.0, (w - 2.0) * frac, h - 2.0, c);
}

/// Vertical stack of full-width buttons growing upward from the bottom of `area`.
pub struct ButtonColumn {
    area: Rect,
    h: f32,
    gap: f32,
    used: f32,
}

impl ButtonColumn {
    pub fn new(area: Rect, u: f32) -> Self {
        ButtonColumn { area, h: (u * 2.1).floor(), gap: (u * 0.4).floor(), used: 0.0 }
    }

    pub fn next(&mut self) -> Rect {
        self.used += self.h;
        let r = Rect::new(self.area.x, self.area.y + self.area.h - self.used, self.area.w, self.h);
        self.used += self.gap;
        r
    }

    /// Top of the stack so far (for placing content above).
    pub fn top(&self) -> f32 {
        self.area.y + self.area.h - self.used
    }
}
