//! Tiny immediate-mode UI helpers: tap input, layout, buttons, text.

use macroquad::prelude::*;

use super::gfx::rgb;

/// One tap per frame (a press and release without dragging), plus drag
/// movement for panning. The first widget that claims the tap consumes it.
pub struct Input {
    tap: Option<Vec2>,
    /// (where the drag started, movement this frame)
    pub drag: Option<(Vec2, Vec2)>,
}

#[derive(Default)]
struct Pointer {
    down: bool,
    start: Vec2,
    last: Vec2,
    dragging: bool,
}

thread_local! {
    static INJECTED: std::cell::Cell<Option<Vec2>> = const { std::cell::Cell::new(None) };
    static SCRIPTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static POINTER: std::cell::RefCell<Pointer> = std::cell::RefCell::new(Pointer::default());
}

/// Queue a synthetic tap for the next frame (used by the desktop debug script).
pub fn inject_tap(p: Vec2) {
    SCRIPTED.with(|c| c.set(true));
    INJECTED.with(|c| c.set(Some(p)));
}

impl Input {
    pub fn poll() -> Input {
        if let Some(p) = INJECTED.with(|c| c.take()) {
            return Input { tap: Some(p), drag: None };
        }
        if SCRIPTED.with(|c| c.get()) {
            return Input { tap: None, drag: None };
        }
        let (pressed, held, released, pos) = match touches().first() {
            Some(t) => (
                t.phase == TouchPhase::Started,
                matches!(t.phase, TouchPhase::Moved | TouchPhase::Stationary),
                matches!(t.phase, TouchPhase::Ended | TouchPhase::Cancelled),
                t.position,
            ),
            None => {
                let (x, y) = mouse_position();
                (
                    is_mouse_button_pressed(MouseButton::Left),
                    is_mouse_button_down(MouseButton::Left),
                    is_mouse_button_released(MouseButton::Left),
                    vec2(x, y),
                )
            }
        };
        let threshold = screen_width().min(screen_height()) * 0.025;
        POINTER.with(|ptr| {
            let mut ptr = ptr.borrow_mut();
            let mut out = Input { tap: None, drag: None };
            if pressed {
                *ptr = Pointer { down: true, start: pos, last: pos, dragging: false };
            }
            if ptr.down && (held || pressed) {
                if (pos - ptr.start).length() > threshold {
                    ptr.dragging = true;
                }
                if ptr.dragging {
                    out.drag = Some((ptr.start, pos - ptr.last));
                }
                ptr.last = pos;
            }
            if released && ptr.down {
                if !ptr.dragging {
                    out.tap = Some(ptr.start);
                }
                ptr.down = false;
            }
            out
        })
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
}

/// Battle screen layout: a scrollable map viewport plus the side panel.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    /// Top-left of the map viewport on screen.
    pub ox: f32,
    pub oy: f32,
    pub tile: f32,
    /// Viewport size in pixels.
    pub map_w: f32,
    pub map_h: f32,
    pub panel: Rect,
    /// Base UI unit (font size); scales with screen.
    pub u: f32,
    /// Camera: map tile coordinate at the viewport's top-left.
    pub cam: Vec2,
    pub cols: i32,
    pub rows: i32,
}

pub const ZOOM_LEVELS: usize = 3;

impl Layout {
    /// `zoom` 0 = whole map, 1 = ~12 rows, 2 = ~8 rows.
    pub fn compute(cols: i32, rows: i32, zoom: usize, cam: Vec2) -> Layout {
        let (w, h) = (screen_width(), screen_height());
        let mw = cols as f32;
        let mh = rows as f32;
        let u = (w.min(h) / 26.0).clamp(14.0, 48.0);
        let m = (u * 0.3).floor();
        let landscape = w >= h * 1.05;
        let area = if landscape {
            let panel_min = (w * 0.26).max(u * 11.0).min(w * 0.45);
            Rect::new(m, m, w - panel_min - m * 3.0, h - m * 2.0)
        } else {
            Rect::new(m, m, w - m * 2.0, h * 0.62)
        };
        let fit = (area.w / mw).min(area.h / mh).floor().max(6.0);
        let vis_rows = [mh, 12.0, 8.0][zoom.min(ZOOM_LEVELS - 1)];
        let tile = if zoom == 0 { fit } else { (area.h / vis_rows).max(fit).floor() };
        let map_w = (mw * tile).min(area.w);
        let map_h = (mh * tile).min(area.h);
        let ox = if landscape { area.x } else { area.x + (area.w - map_w) / 2.0 };
        let oy = if landscape { area.y + (area.h - map_h) / 2.0 } else { area.y };
        let max_cam = vec2((mw - map_w / tile).max(0.0), (mh - map_h / tile).max(0.0));
        // Snap to whole pixels so tile edges land exactly.
        let cam = (cam.clamp(Vec2::ZERO, max_cam) * tile).round() / tile;
        let panel = if landscape {
            let px = ox + map_w + m;
            Rect::new(px, m, w - px - m, h - m * 2.0)
        } else {
            let py = oy + map_h + m;
            Rect::new(m, py, w - m * 2.0, h - py - m)
        };
        Layout { ox, oy, tile, map_w, map_h, panel, u, cam, cols, rows }
    }

    pub fn view(&self) -> Rect {
        Rect::new(self.ox, self.oy, self.map_w, self.map_h)
    }

    /// Size of the viewport in tiles.
    pub fn view_tiles(&self) -> Vec2 {
        vec2(self.map_w / self.tile, self.map_h / self.tile)
    }

    pub fn scrollable(&self) -> bool {
        self.view_tiles().x + 0.01 < self.cols as f32 || self.view_tiles().y + 0.01 < self.rows as f32
    }

    pub fn tile_rect(&self, x: f32, y: f32) -> Rect {
        Rect::new(self.ox + (x - self.cam.x) * self.tile, self.oy + (y - self.cam.y) * self.tile, self.tile, self.tile)
    }

    pub fn tile_at(&self, p: Vec2) -> Option<(i32, i32)> {
        if !self.view().contains(p) {
            return None;
        }
        let x = (self.cam.x + (p.x - self.ox) / self.tile).floor() as i32;
        let y = (self.cam.y + (p.y - self.oy) / self.tile).floor() as i32;
        if x >= 0 && y >= 0 && x < self.cols && y < self.rows {
            Some((x, y))
        } else {
            None
        }
    }

    /// Range of tiles at least partly visible: (x0, y0, x1, y1) inclusive.
    pub fn visible(&self) -> (i32, i32, i32, i32) {
        let v = self.view_tiles();
        let x0 = self.cam.x.floor() as i32;
        let y0 = self.cam.y.floor() as i32;
        let x1 = ((self.cam.x + v.x).ceil() as i32).min(self.cols - 1);
        let y1 = ((self.cam.y + v.y).ceil() as i32).min(self.rows - 1);
        (x0.max(0), y0.max(0), x1, y1)
    }

    /// Camera position that centres tile `p`.
    pub fn centred_on(&self, p: Vec2) -> Vec2 {
        let v = self.view_tiles();
        p + vec2(0.5, 0.5) - v / 2.0
    }

    /// Whether tile `p` lies inside the viewport, keeping a margin of `m` tiles.
    pub fn shows(&self, p: Vec2, m: f32) -> bool {
        let v = self.view_tiles();
        let m = m.min(v.x / 3.0).min(v.y / 3.0);
        p.x >= self.cam.x + m - 0.01 && p.y >= self.cam.y + m - 0.01 && p.x + 1.0 <= self.cam.x + v.x - m + 0.01 && p.y + 1.0 <= self.cam.y + v.y - m + 0.01
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

/// Text with a thick dark outline, readable on any background.
pub fn text_outlined(s: &str, cx: f32, y: f32, size: f32, c: Color) {
    let x = cx - text_w(s, size) / 2.0;
    let o = (size / 14.0).max(2.0);
    let ink = Color::new(0.06, 0.04, 0.08, c.a);
    for (dx, dy) in [(-o, 0.0), (o, 0.0), (0.0, -o), (0.0, o), (-o, -o), (o, o), (-o, o), (o, -o)] {
        draw_text(s, x + dx, y + dy, size, ink);
    }
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

/// Grid of buttons growing upward from the bottom of `area`, `cols` per row.
pub struct ButtonColumn {
    area: Rect,
    h: f32,
    gap: f32,
    used: f32,
    cols: usize,
    in_row: usize,
}

impl ButtonColumn {
    pub fn new(area: Rect, u: f32) -> Self {
        Self::grid(area, u, 1)
    }

    pub fn grid(area: Rect, u: f32, cols: usize) -> Self {
        ButtonColumn { area, h: (u * 2.1).floor(), gap: (u * 0.4).floor(), used: 0.0, cols: cols.max(1), in_row: 0 }
    }

    pub fn next(&mut self) -> Rect {
        if self.in_row == 0 {
            self.used += self.h;
        }
        let w = (self.area.w - self.gap * (self.cols - 1) as f32) / self.cols as f32;
        let x = self.area.x + self.in_row as f32 * (w + self.gap);
        let r = Rect::new(x, self.area.y + self.area.h - self.used, w, self.h);
        self.in_row += 1;
        if self.in_row == self.cols {
            self.in_row = 0;
            self.used += self.gap;
        }
        r
    }

    /// Top of the stack so far (for placing content above).
    pub fn top(&self) -> f32 {
        let partial = if self.in_row > 0 { self.gap } else { 0.0 };
        self.area.y + self.area.h - self.used - partial
    }
}

/// Greedy word wrap to `max_w` pixels.
pub fn wrap(s: &str, max_w: f32, size: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        let candidate = if cur.is_empty() { word.to_string() } else { format!("{} {}", cur, word) };
        if text_w(&candidate, size) > max_w && !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        } else {
            cur = candidate;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Base UI unit (font size) for full-screen menus.
pub fn ui_unit() -> f32 {
    (screen_width().min(screen_height()) / 26.0).clamp(14.0, 48.0)
}
