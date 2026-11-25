//! Pixel art drawn in code. Sprites and tiles are painted once into small
//! RGBA canvases (32x32 for map tiles and units, 48x48 for portraits),
//! outlined and shaded, then cached as nearest-filtered textures.

use std::cell::RefCell;
use std::collections::HashMap;

use macroquad::prelude::*;

use crate::logic::terrain::Terrain;
use crate::logic::unit::{Class, Team, Unit, WeaponKind};

pub fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgba(r, g, b, 255)
}

fn shade(c: Color, k: f32) -> Color {
    Color::new((c.r * k).min(1.0), (c.g * k).min(1.0), (c.b * k).min(1.0), c.a)
}

fn hash(x: i32, y: i32, k: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263) ^ (k as u32).wrapping_mul(2_246_822_519);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^ (h >> 16)
}

// ---------------------------------------------------------------- canvas

struct Canvas {
    w: i32,
    h: i32,
    px: Vec<Option<Color>>,
}

impl Canvas {
    fn new(w: i32, h: i32) -> Canvas {
        Canvas { w, h, px: vec![None; (w * h) as usize] }
    }

    fn set(&mut self, x: i32, y: i32, c: Color) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.px[(y * self.w + x) as usize] = Some(c);
        }
    }

    fn get(&self, x: i32, y: i32) -> Option<Color> {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.px[(y * self.w + x) as usize]
        } else {
            None
        }
    }

    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, c);
            }
        }
    }

    /// Rect with highlight on the left and shadow on the right/bottom.
    fn shaded(&mut self, x: i32, y: i32, w: i32, h: i32, base: Color) {
        self.rect(x, y, w, h, base);
        self.rect(x, y, 1, h, shade(base, 1.25));
        self.rect(x + w - 1, y, 1, h, shade(base, 0.7));
        if w > 3 {
            self.rect(x + w - 2, y + 1, 1, h - 1, shade(base, 0.85));
        }
        self.rect(x, y + h - 1, w, 1, shade(base, 0.75));
    }

    fn disc(&mut self, cx: f32, cy: f32, r: f32, c: Color) {
        let (x0, x1) = ((cx - r).floor() as i32, (cx + r).ceil() as i32);
        let (y0, y1) = ((cy - r).floor() as i32, (cy + r).ceil() as i32);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 + 0.5 - cy;
                if dx * dx + dy * dy <= r * r {
                    self.set(x, y, c);
                }
            }
        }
    }

    /// Ball lit from the top-left.
    fn ball(&mut self, cx: f32, cy: f32, r: f32, base: Color) {
        self.disc(cx, cy, r, shade(base, 0.72));
        self.disc(cx - r * 0.12, cy - r * 0.12, r * 0.85, base);
        self.disc(cx - r * 0.35, cy - r * 0.35, r * 0.4, shade(base, 1.22));
    }

    fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Color) {
        let n = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
        for i in 0..=n {
            let t = i as f32 / n as f32;
            self.set((x0 as f32 + (x1 - x0) as f32 * t).round() as i32, (y0 as f32 + (y1 - y0) as f32 * t).round() as i32, c);
        }
    }

    /// Dark outline around everything painted so far.
    fn outline(&mut self, c: Color) {
        let mut add = Vec::new();
        for y in 0..self.h {
            for x in 0..self.w {
                if self.get(x, y).is_none() && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(dx, dy)| self.get(x + dx, y + dy).is_some_and(|p| p.a > 0.5)) {
                    add.push((x, y));
                }
            }
        }
        for (x, y) in add {
            self.set(x, y, c);
        }
    }

    fn texture(&self, grey: bool) -> Texture2D {
        let mut bytes = Vec::with_capacity((self.w * self.h * 4) as usize);
        for p in &self.px {
            let c = p.unwrap_or(Color::new(0.0, 0.0, 0.0, 0.0));
            let c = if grey && c.a > 0.0 {
                let l = c.r * 0.3 + c.g * 0.59 + c.b * 0.11;
                let l = l * 0.7 + 0.08;
                Color::new(l, l, (l * 1.08).min(1.0), c.a)
            } else {
                c
            };
            bytes.extend_from_slice(&[(c.r * 255.0) as u8, (c.g * 255.0) as u8, (c.b * 255.0) as u8, (c.a * 255.0) as u8]);
        }
        let t = Texture2D::from_rgba8(self.w as u16, self.h as u16, &bytes);
        t.set_filter(FilterMode::Nearest);
        t
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    Tile(u8, u8, u8),
    Unit(UnitLook, bool, bool),
    Portrait(UnitLook),
}

thread_local! {
    static CACHE: RefCell<HashMap<Key, Texture2D>> = RefCell::new(HashMap::new());
}

fn cached(key: Key, make: impl FnOnce() -> Texture2D) -> Texture2D {
    CACHE.with(|c| {
        if let Some(t) = c.borrow().get(&key) {
            return t.clone();
        }
        let t = make();
        c.borrow_mut().insert(key, t.clone());
        t
    })
}

fn blit(tex: &Texture2D, x: f32, y: f32, w: f32, h: f32, flip: bool, alpha: f32) {
    // Round both edges so neighbouring tiles always meet exactly.
    let (x0, y0) = (x.round(), y.round());
    let (x1, y1) = ((x + w).round(), (y + h).round());
    draw_texture_ex(
        tex,
        x0,
        y0,
        Color::new(1.0, 1.0, 1.0, alpha),
        DrawTextureParams { dest_size: Some(vec2(x1 - x0, y1 - y0)), flip_x: flip, ..Default::default() },
    );
}

// --------------------------------------------------------------- terrain

const T: i32 = 32;

fn grass(cv: &mut Canvas, v: u8, darker: f32) {
    let base = shade(rgb(104, 170, 72), darker);
    for y in 0..T {
        for x in 0..T {
            let h = hash(x + v as i32 * 57, y, 3) % 100;
            let c = if h < 10 {
                shade(base, 1.12)
            } else if h < 18 {
                shade(base, 0.9)
            } else {
                base
            };
            cv.set(x, y, c);
        }
    }
    for k in 0..6 {
        let h = hash(v as i32, k, 11);
        let x = (h % 28) as i32 + 2;
        let y = ((h >> 8) % 28) as i32 + 2;
        let d = shade(base, 0.72);
        cv.set(x, y, d);
        cv.set(x + 1, y - 1, d);
        cv.set(x + 2, y, d);
        if k == 0 && v.is_multiple_of(2) {
            cv.set(x + 6, y + 3, rgb(250, 240, 160));
            cv.set(x + 9, y + 5, rgb(240, 160, 180));
        }
    }
}

fn tree(cv: &mut Canvas, cx: f32, cy: f32, r: f32, base: Color) {
    let trunk = rgb(102, 68, 40);
    cv.disc(cx + 1.0, cy + r * 1.2, r * 0.8, shade(rgb(104, 170, 72), 0.6));
    cv.rect(cx as i32 - 1, (cy + r * 0.5) as i32, 3, (r * 0.9) as i32 + 1, trunk);
    cv.set(cx as i32 + 1, (cy + r * 0.5) as i32 + 1, shade(trunk, 0.7));
    cv.disc(cx, cy, r + 1.0, shade(base, 0.45));
    cv.ball(cx, cy, r, base);
    cv.disc(cx - r * 0.5, cy + r * 0.1, r * 0.45, base);
    cv.disc(cx - r * 0.45, cy - r * 0.45, r * 0.22, shade(base, 1.35));
}

fn cobble(cv: &mut Canvas, v: u8, base: Color) {
    cv.rect(0, 0, T, T, shade(base, 0.7));
    for row in 0..5 {
        let y = row * 7;
        let off = if row % 2 == 0 { 0 } else { 4 };
        let mut x = -off;
        while x < T {
            let h = hash(x + v as i32 * 3, y, 5);
            let w = 6 + (h % 3) as i32;
            let c = shade(base, 0.92 + (h % 20) as f32 / 100.0);
            cv.rect(x + 1, y + 1, w - 1, 5, c);
            cv.rect(x + 1, y + 1, w - 1, 1, shade(c, 1.15));
            x += w;
        }
    }
}

fn stone_wall(cv: &mut Canvas, v: u8) {
    let base = rgb(118, 116, 130);
    cv.rect(0, 0, T, T, shade(base, 0.55));
    for row in 0..6 {
        let y = row * 5 + 4;
        let off = if (row + v as i32) % 2 == 0 { 0 } else { 5 };
        let mut x = -off;
        while x < T {
            let h = hash(x, y + v as i32, 9);
            let c = shade(base, 0.9 + (h % 20) as f32 / 100.0);
            cv.rect(x + 1, y + 1, 9, 4, c);
            cv.rect(x + 1, y + 1, 9, 1, shade(c, 1.2));
            x += 10;
        }
    }
    cv.rect(0, 0, T, 5, shade(base, 1.1));
    for k in 0..4 {
        cv.rect(k * 8 + 5, 0, 3, 3, shade(base, 0.55));
    }
    cv.rect(0, 4, T, 1, shade(base, 0.45));
}

fn water(cv: &mut Canvas, frame: u8) {
    let base = rgb(56, 108, 200);
    for y in 0..T {
        for x in 0..T {
            let h = hash(x, y, 21) % 100;
            cv.set(x, y, if h < 8 { shade(base, 1.08) } else { base });
        }
    }
    let light = rgb(140, 190, 245);
    for k in 0..5 {
        let h = hash(k, 0, 31);
        let y = (k * 6 + (h % 3) as i32) % T;
        let x = ((h >> 4) % 32) as i32 + frame as i32 * 2;
        for d in 0..5 {
            cv.set((x + d) % T, y, light);
        }
        cv.set((x + 5) % T, y + 1, shade(base, 0.8));
    }
}

fn paint_tile(t: Terrain, v: u8, frame: u8, alt: bool) -> Canvas {
    let mut cv = Canvas::new(T, T);
    match t {
        Terrain::Plains => grass(&mut cv, v, 1.0),
        Terrain::Forest => {
            grass(&mut cv, v, 0.9);
            let g = [rgb(58, 140, 62), rgb(66, 150, 58), rgb(50, 128, 70)][(v % 3) as usize];
            if v.is_multiple_of(2) {
                tree(&mut cv, 9.0, 10.0, 7.0, g);
                tree(&mut cv, 23.0, 13.0, 7.5, shade(g, 0.95));
                tree(&mut cv, 14.0, 22.0, 7.0, shade(g, 1.05));
            } else {
                tree(&mut cv, 11.0, 12.0, 9.0, g);
                tree(&mut cv, 24.0, 21.0, 6.5, shade(g, 1.05));
            }
        }
        Terrain::Mountain => {
            grass(&mut cv, v, 0.85);
            let rock = rgb(150, 124, 92);
            let dark = rgb(98, 78, 60);
            let light = rgb(196, 172, 132);
            let peak = 13 + (v % 3) as i32 * 2;
            for row in 0..26 {
                let y = 4 + row;
                let half = (row as f32 * 0.62 + 1.0) as i32;
                cv.rect(peak - half, y, half, 1, light);
                cv.rect(peak, y, half, 1, rock);
                cv.rect(peak + half - 2, y, 2, 1, dark);
            }
            for k in 0..6 {
                let h = hash(v as i32, k, 41);
                let x = peak - 6 + (h % 12) as i32;
                let y = 14 + ((h >> 6) % 12) as i32;
                cv.rect(x, y, 3, 1, dark);
            }
            cv.rect(peak - 2, 4, 4, 2, rgb(245, 245, 250));
            cv.rect(peak - 3, 6, 3, 2, rgb(230, 232, 240));
            cv.rect(0, 30, T, 2, shade(dark, 0.8));
        }
        Terrain::Fort => {
            grass(&mut cv, v, 1.0);
            let st = rgb(176, 172, 180);
            let ink = rgb(40, 36, 44);
            cv.rect(3, 5, 26, 25, ink);
            cv.shaded(4, 10, 24, 19, st);
            for k in 0..5 {
                cv.shaded(4 + k * 5, 6, 3, 5, st);
            }
            cv.rect(4, 17, 24, 1, shade(st, 0.6));
            cv.rect(12, 18, 8, 11, rgb(70, 46, 30));
            cv.rect(13, 19, 6, 10, rgb(110, 74, 42));
            cv.rect(15, 19, 1, 10, rgb(70, 46, 30));
            cv.rect(15, 0, 1, 7, rgb(80, 60, 40));
            cv.rect(16, 0, 6, 4, rgb(214, 70, 50));
        }
        Terrain::Water => water(&mut cv, frame),
        Terrain::Bridge => {
            water(&mut cv, frame);
            let wood = rgb(164, 112, 64);
            cv.rect(0, 5, T, 22, wood);
            for k in 0..8 {
                cv.rect(k * 4 + 3, 5, 1, 22, shade(wood, 0.65));
                cv.rect(k * 4, 5, 1, 22, shade(wood, 1.15));
            }
            cv.rect(0, 4, T, 2, rgb(100, 66, 36));
            cv.rect(0, 26, T, 2, rgb(100, 66, 36));
        }
        Terrain::Village => {
            grass(&mut cv, v, 1.0);
            let wall = rgb(232, 214, 170);
            let roof = if alt { rgb(120, 104, 90) } else { rgb(196, 150, 70) };
            cv.rect(3, 4, 26, 25, rgb(50, 36, 30));
            grass_patch(&mut cv, v);
            cv.shaded(6, 15, 20, 13, wall);
            for k in 0..9 {
                let w = 26 - k * 2;
                cv.rect(16 - w / 2, 14 - k, w, 1, if k % 2 == 0 { roof } else { shade(roof, 0.88) });
            }
            cv.rect(4, 14, 24, 2, shade(roof, 0.7));
            let door = if alt { rgb(60, 50, 44) } else { rgb(120, 76, 40) };
            cv.rect(14, 20, 5, 8, door);
            cv.rect(8, 18, 4, 4, rgb(110, 170, 220));
            cv.rect(21, 18, 4, 4, rgb(110, 170, 220));
            cv.rect(9, 19, 1, 1, rgb(220, 240, 255));
            cv.rect(23, 6, 3, 5, rgb(150, 90, 70));
        }
        Terrain::Floor => cobble(&mut cv, v, rgb(138, 144, 160)),
        Terrain::Wall => stone_wall(&mut cv, v),
        Terrain::Pillar => {
            cobble(&mut cv, v, rgb(138, 144, 160));
            let col = rgb(196, 194, 206);
            cv.disc(17.0, 28.0, 8.0, rgb(80, 84, 96));
            cv.shaded(10, 3, 12, 26, col);
            cv.rect(13, 3, 2, 26, shade(col, 1.2));
            cv.shaded(8, 2, 16, 3, col);
            cv.shaded(8, 27, 16, 3, col);
        }
        Terrain::Throne => {
            cobble(&mut cv, v, rgb(138, 144, 160));
            cv.rect(6, 0, 20, T, rgb(150, 30, 44));
            cv.rect(7, 0, 1, T, rgb(210, 170, 60));
            cv.rect(24, 0, 1, T, rgb(210, 170, 60));
            let gold = rgb(222, 180, 64);
            cv.shaded(9, 3, 14, 18, gold);
            cv.rect(11, 5, 10, 11, rgb(120, 24, 64));
            cv.shaded(8, 18, 16, 6, gold);
            cv.rect(15, 1, 2, 2, rgb(255, 240, 150));
        }
        Terrain::Gate => {
            stone_wall(&mut cv, v);
            cv.rect(6, 7, 20, 25, rgb(40, 30, 26));
            cv.rect(7, 8, 18, 24, rgb(122, 80, 44));
            cv.rect(16, 8, 1, 24, rgb(70, 46, 26));
            for y in [12, 20, 28] {
                cv.rect(7, y, 18, 1, rgb(80, 74, 90));
            }
            cv.set(14, 20, rgb(230, 200, 90));
            cv.set(18, 20, rgb(230, 200, 90));
        }
    }
    cv
}

/// Re-grass the area around a building after its outline block.
fn grass_patch(cv: &mut Canvas, v: u8) {
    let mut g = Canvas::new(T, T);
    grass(&mut g, v, 1.0);
    for y in 0..T {
        for x in 0..T {
            let inside = (4..28).contains(&x) && (5..28).contains(&y);
            if !inside {
                if let Some(c) = g.get(x, y) {
                    cv.set(x, y, c);
                }
            }
        }
    }
}

/// `alt` marks a visited village.
#[allow(clippy::too_many_arguments)]
pub fn draw_terrain(t: Terrain, x: f32, y: f32, size: f32, tx: i32, ty: i32, time: f32, alt: bool) {
    let v = (hash(tx, ty, 1) % 4) as u8;
    let frame = if matches!(t, Terrain::Water | Terrain::Bridge) { ((time * 2.5) as i32).rem_euclid(4) as u8 } else { 0 };
    let tex = cached(Key::Tile(t as u8, v, frame + if alt { 8 } else { 0 }), || paint_tile(t, v, frame, alt).texture(false));
    blit(&tex, x, y, size, size, false, 1.0);
}

// ----------------------------------------------------------------- units

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Hair {
    Short,
    Long,
    Bearded,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct UnitLook {
    pub class: Class,
    pub team: Team,
    pub weapon: WeaponKind,
    pub boss: bool,
    /// Icy palette for the Pale Sovereign's army.
    pub pale: bool,
    pub hair: u8,
    pub skin: u8,
    pub style: Hair,
}

const LONG_HAIR: [&str; 11] = ["Brenna", "Tamsin", "Liese", "Sera", "Ilse", "Wren", "Maeve", "Rhea", "Seren", "Ysolde", "Vex"];
const BEARDS: [&str; 9] = ["Varga", "Grell", "Thorne", "Bram", "Toma", "Hrolf", "Hask", "Oskar", "Kreth"];

impl UnitLook {
    pub fn plain(class: Class, team: Team, weapon: WeaponKind) -> UnitLook {
        UnitLook { class, team, weapon, boss: false, pale: false, hair: 0, skin: 0, style: Hair::Short }
    }

    pub fn of(u: &Unit) -> UnitLook {
        let weapon = u.weapon().map(|w| w.kind).or_else(|| u.staff().map(|w| w.kind)).unwrap_or(match u.class.info().kinds.first() {
            Some(k) => *k,
            None => WeaponKind::Sword,
        });
        let h = u.name.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
        let style = if LONG_HAIR.contains(&u.name.as_str()) {
            Hair::Long
        } else if BEARDS.contains(&u.name.as_str()) {
            Hair::Bearded
        } else {
            Hair::Short
        };
        // Named heroes get fixed looks; generic soldiers vary by name.
        let (hair, skin) = match u.name.as_str() {
            "Kael" => (0, 0),
            "Brenna" => (3, 1),
            "Tamsin" => (2, 0),
            "Orrin" => (1, 2),
            "Liese" => (2, 0),
            "Holt" => (0, 1),
            "Dain" => (1, 3),
            "Sera" => (4, 0),
            "Ilse" => (5, 0),
            "Wren" => (3, 0),
            "Maeve" => (1, 2),
            "Aldous" => (4, 1),
            "Bram" => (4, 1),
            "Corvin" => (1, 1),
            "Rhea" => (0, 3),
            "Toma" => (3, 2),
            "Seren" => (2, 1),
            "Morvaine" | "Vaelith" => (4, 0),
            _ => ((h % 6) as u8, ((h >> 4) % 4) as u8),
        };
        let pale = u.name.starts_with("Pale") || u.name == "Vaelith" || u.name == "Ysolde";
        UnitLook { class: u.class, team: u.team, weapon, boss: u.is_boss, pale, hair, skin, style }
    }
}

pub fn team_colors(team: Team, boss: bool) -> (Color, Color) {
    match (team, boss) {
        (Team::Player, _) => (rgb(64, 116, 222), rgb(32, 58, 140)),
        (Team::Enemy, false) => (rgb(204, 60, 50), rgb(120, 30, 30)),
        (Team::Enemy, true) => (rgb(150, 52, 140), rgb(84, 24, 80)),
    }
}

fn palette(l: &UnitLook) -> Color {
    if l.team == Team::Player {
        rgb(62, 112, 222)
    } else if l.class == Class::Sorcerer {
        rgb(70, 40, 96)
    } else if l.pale {
        rgb(150, 170, 200)
    } else if l.boss {
        rgb(148, 50, 140)
    } else {
        rgb(204, 56, 48)
    }
}

const SKINS: [(u8, u8, u8); 4] = [(250, 212, 172), (236, 184, 140), (196, 140, 98), (140, 94, 64)];
const HAIRS: [(u8, u8, u8); 6] = [(120, 70, 36), (44, 36, 40), (232, 200, 110), (196, 76, 40), (210, 214, 224), (60, 70, 120)];

fn skin_of(l: &UnitLook) -> Color {
    if l.pale {
        return rgb(212, 222, 236);
    }
    let (r, g, b) = SKINS[l.skin as usize % 4];
    rgb(r, g, b)
}

fn hair_of(l: &UnitLook) -> Color {
    let (r, g, b) = HAIRS[l.hair as usize % 6];
    rgb(r, g, b)
}

/// Paints a 32x32 map sprite facing right. `up` lifts the upper body one pixel (idle bob).
fn paint_unit(l: &UnitLook, up: bool) -> Canvas {
    let mut cv = Canvas::new(32, 32);
    let main = palette(l);
    let dark = shade(main, 0.6);
    let skin = skin_of(l);
    let hair = hair_of(l);
    let metal = rgb(200, 206, 218);
    let gold = rgb(226, 184, 70);
    let leather = rgb(128, 84, 50);
    let white = rgb(238, 236, 228);
    let sorcerer = l.class == Class::Sorcerer;
    let class = if sorcerer { Class::Mage } else { l.class.base_look() };
    let promoted = l.class.is_promoted();
    let b = if up { -1 } else { 0 };
    let robed = matches!(class, Class::Mage | Class::Cleric);
    let armored = class == Class::Knight;
    let heavy = armored || matches!(l.class, Class::General | Class::Warlord);

    // Cape behind the body
    if class == Class::Lord || promoted || l.boss {
        let cape = if l.team == Team::Player { shade(main, 0.75) } else { dark };
        cv.rect(8, 13 + b, 15, 15, cape);
        cv.rect(8, 13 + b, 2, 15, shade(cape, 0.8));
        if promoted {
            cv.rect(8, 27 + b, 15, 1, gold);
        }
    }

    // Legs / robe skirt
    if robed {
        let robe = if class == Class::Cleric { white } else { main };
        for k in 0..10 {
            let w = 12 + k / 2;
            cv.rect(16 - w / 2, 20 + k + b, w, 1, robe);
        }
        cv.rect(10, 20 + b, 2, 10, shade(robe, 1.15));
        cv.rect(19, 21 + b, 3, 9, shade(robe, 0.78));
        let trim = if class == Class::Cleric { main } else { gold };
        cv.rect(10, 29, 13, 1, trim);
        cv.rect(15, 14 + b, 2, 15, trim);
    } else {
        let pants = if heavy { metal } else { shade(dark, 0.9) };
        cv.shaded(11, 22, 4, 6, pants);
        cv.shaded(17, 22, 4, 6, pants);
        cv.shaded(10, 27, 5, 3, leather);
        cv.shaded(17, 27, 5, 3, leather);
    }

    // Torso
    let bare = matches!(class, Class::Brigand | Class::Fighter);
    let torso = if bare { rgb(150, 108, 66) } else { main };
    let (tx, tw) = if heavy || bare { (9, 14) } else { (10, 12) };
    if robed {
        cv.shaded(11, 13 + b, 10, 8, if class == Class::Cleric { white } else { main });
    } else {
        cv.shaded(tx, 13 + b, tw, 9, torso);
        cv.rect(tx, 20 + b, tw, 2, leather);
        cv.set(16, 20 + b, gold);
    }
    if armored || matches!(l.class, Class::General | Class::Halberdier) || matches!(class, Class::Soldier | Class::Lancer) {
        cv.shaded(tx + 1, 13 + b, tw - 2, 5, metal);
        cv.rect(tx + 2, 14 + b, 2, 3, shade(metal, 1.2));
    }
    if class == Class::Mercenary {
        cv.shaded(11, 13 + b, 10, 7, leather);
        cv.rect(14, 13 + b, 4, 7, main);
    }
    if l.class == Class::Warlord || (l.boss && !robed) {
        cv.ball(10.0, 14.0 + b as f32, 3.0, gold);
        cv.ball(22.0, 14.0 + b as f32, 3.0, gold);
    } else if heavy {
        cv.ball(9.5, 14.5 + b as f32, 3.0, metal);
        cv.ball(22.5, 14.5 + b as f32, 3.0, metal);
    }
    if promoted && !robed {
        cv.rect(tx + 1, 13 + b, tw - 2, 1, gold);
    }

    // Arms
    let sleeve = if bare {
        skin
    } else if heavy {
        metal
    } else {
        shade(main, 0.85)
    };
    cv.shaded(7, 14 + b, 3, 7, sleeve);
    cv.shaded(22, 14 + b, 3, 7, sleeve);
    cv.rect(7, 21 + b, 3, 2, skin);
    cv.rect(22, 21 + b, 3, 2, skin);

    // Head
    cv.rect(14, 11 + b, 4, 2, shade(skin, 0.85));
    cv.shaded(12, 4 + b, 9, 8, skin);
    cv.rect(13, 4 + b, 7, 1, shade(skin, 1.08));
    let eye = rgb(30, 24, 30);
    for (ex, ey) in [(18, 7), (18, 8), (15, 7), (15, 8)] {
        cv.set(ex, ey + b, eye);
    }
    cv.set(20, 9 + b, shade(skin, 0.75));
    cv.rect(16, 10 + b, 3, 1, shade(skin, 0.7));
    if l.style == Hair::Bearded {
        cv.rect(13, 9 + b, 8, 4, shade(hair, 0.9));
        cv.rect(16, 10 + b, 3, 1, shade(hair, 0.6));
    }

    // Headgear / hair
    let helm = if l.pale { rgb(170, 186, 210) } else { metal };
    if matches!(class, Class::Lancer | Class::Soldier) {
        cv.shaded(11, 2 + b, 11, 5, helm);
        cv.rect(11, 6 + b, 2, 4, helm);
        cv.rect(16, 6 + b, 2, 1, helm);
        cv.rect(15, b, 3, 2, main);
    } else if armored {
        cv.shaded(11, 2 + b, 11, 10, helm);
        cv.rect(14, 7 + b, 7, 2, rgb(26, 26, 34));
        cv.rect(15, b, 3, 3, if promoted { gold } else { main });
        if promoted {
            cv.rect(14, b - 1, 5, 2, main);
        }
    } else if l.class == Class::Warlord {
        cv.shaded(11, 2 + b, 11, 5, rgb(70, 66, 80));
        cv.line(11, 3 + b, 8, b - 1, white);
        cv.line(12, 3 + b, 9, b - 1, white);
        cv.line(21, 3 + b, 24, b - 1, white);
        cv.line(20, 3 + b, 23, b - 1, white);
    } else if bare {
        let band = if class == Class::Brigand { main } else { rgb(200, 190, 160) };
        cv.rect(12, 3 + b, 9, 3, hair);
        cv.rect(11, 5 + b, 11, 2, band);
        cv.rect(10, 6 + b, 2, 4, band);
    } else if class == Class::Archer {
        let hood = if l.team == Team::Player { rgb(70, 130, 80) } else { shade(main, 0.8) };
        cv.shaded(11, 2 + b, 11, 5, hood);
        cv.rect(11, 6 + b, 2, 7, hood);
        cv.line(13, 2 + b, 9, b - 2, white);
    } else if class == Class::Mage {
        let hat = if sorcerer { rgb(40, 20, 56) } else { main };
        cv.rect(9, 4 + b, 15, 2, shade(hat, 0.8));
        for k in 0..6 {
            cv.rect(12 + k / 2, 3 - k + b, 8 - k, 1, hat);
        }
        cv.rect(17, b - 3, 3, 1, hat);
        if sorcerer {
            for k in 0..4 {
                cv.rect(12 + k * 3, 1 + b, 1, 3, gold);
            }
            cv.set(15, 8 + b, rgb(255, 80, 80));
            cv.set(18, 8 + b, rgb(255, 80, 80));
        }
        cv.rect(12, 6 + b, 2, 3, hair);
    } else if class == Class::Cleric {
        cv.shaded(11, 2 + b, 11, 4, white);
        cv.rect(11, 5 + b, 2, 8, white);
        if l.class == Class::Bishop {
            cv.rect(13, b - 2, 7, 4, white);
            cv.rect(16, b - 2, 1, 4, gold);
        } else {
            cv.rect(12, 2 + b, 9, 1, gold);
        }
    } else {
        cv.shaded(11, 2 + b, 11, 4, hair);
        cv.rect(11, 5 + b, 2, 4, hair);
        cv.rect(19, 3 + b, 3, 2, shade(hair, 1.2));
        if l.style == Hair::Long {
            cv.rect(10, 5 + b, 3, 9, hair);
            cv.rect(10, 13 + b, 2, 2, shade(hair, 0.8));
        }
        if class == Class::Lord {
            cv.rect(13, 3 + b, 7, 1, gold);
            cv.set(16, 2 + b, rgb(120, 220, 255));
        }
        if class == Class::Mercenary {
            cv.rect(11, 4 + b, 11, 1, rgb(220, 210, 190));
        }
    }
    if l.style == Hair::Long && !matches!(class, Class::Lord | Class::Mercenary) && !armored {
        cv.rect(10, 7 + b, 2, 8, hair);
    }

    // Weapon
    let wood = rgb(130, 86, 46);
    match l.weapon {
        WeaponKind::Sword => {
            cv.rect(25, 4 + b, 2, 17, rgb(226, 230, 240));
            cv.rect(26, 4 + b, 1, 17, rgb(160, 168, 186));
            cv.rect(23, 20 + b, 6, 2, gold);
            cv.rect(25, 22 + b, 2, 3, leather);
        }
        WeaponKind::Lance => {
            cv.rect(25, 1 + b, 2, 29, wood);
            cv.rect(25, 1 + b, 1, 29, shade(wood, 1.2));
            for k in 0..5 {
                cv.rect(26 - k / 2, k + b - 4, 1 + k, 1, metal);
            }
            cv.rect(24, 5 + b, 4, 1, gold);
        }
        WeaponKind::Axe => {
            cv.rect(25, 6 + b, 2, 20, wood);
            let head = if l.class == Class::Warlord { rgb(170, 170, 190) } else { metal };
            cv.shaded(27, 5 + b, 5, 8, head);
            cv.rect(26, 7 + b, 1, 4, shade(head, 0.7));
            cv.rect(22, 7 + b, 3, 4, head);
        }
        WeaponKind::Bow => {
            for y in 0..22 {
                let dy = y as f32 - 11.0;
                let x = 29 - (dy * dy / 40.0) as i32;
                cv.rect(x, 3 + y + b, 2, 1, wood);
            }
            cv.rect(25, 3 + b, 1, 22, rgb(240, 240, 240));
        }
        WeaponKind::Magic => {
            let tome = if sorcerer {
                rgb(60, 20, 80)
            } else if l.team == Team::Player {
                rgb(180, 50, 40)
            } else {
                rgb(80, 40, 120)
            };
            cv.shaded(22, 17 + b, 7, 6, tome);
            cv.rect(22, 17 + b, 7, 1, gold);
            cv.disc(27.0, 13.0 + b as f32, 2.0, if sorcerer { rgb(170, 90, 255) } else { rgb(255, 200, 90) });
        }
        WeaponKind::Staff => {
            cv.rect(25, 3 + b, 2, 27, wood);
            cv.ball(26.0, 3.0 + b as f32, 3.0, gold);
            cv.set(25, 2 + b, white);
        }
    }

    cv.outline(rgb(24, 18, 28));
    cv
}

/// Draws a unit sprite in a tile-sized box. `bob` = idle animation frame.
pub fn draw_unit(look: UnitLook, x: f32, y: f32, size: f32, done: bool, alpha: f32, bob: bool) {
    draw_ellipse(x + size * 0.5, y + size * 0.93, size * 0.3, size * 0.07, 0.0, Color::new(0.0, 0.0, 0.0, 0.28 * alpha));
    let tex = cached(Key::Unit(look, bob, done), || paint_unit(&look, bob).texture(done));
    blit(&tex, x, y, size, size, look.team == Team::Enemy, alpha);
}

// ------------------------------------------------------------- portraits

fn paint_portrait(l: &UnitLook) -> Canvas {
    let mut cv = Canvas::new(48, 48);
    let main = palette(l);
    let skin = skin_of(l);
    let hair = hair_of(l);
    let class = if l.class == Class::Sorcerer { Class::Mage } else { l.class.base_look() };
    let metal = rgb(196, 202, 216);
    let gold = rgb(226, 184, 70);
    let white = rgb(238, 236, 228);
    let ink = rgb(30, 22, 30);

    if l.style == Hair::Long {
        cv.rect(11, 12, 26, 26, shade(hair, 0.8));
    }
    // Shoulders / outfit
    let outfit = match class {
        Class::Cleric => white,
        Class::Knight | Class::Lancer | Class::Soldier => metal,
        Class::Brigand | Class::Fighter => rgb(150, 108, 66),
        _ => main,
    };
    for k in 0..12 {
        let w = 30 + k;
        cv.rect(24 - w / 2, 36 + k, w, 1, outfit);
    }
    cv.rect(4, 40, 40, 8, outfit);
    cv.rect(4, 40, 3, 8, shade(outfit, 1.2));
    cv.rect(40, 40, 4, 8, shade(outfit, 0.75));
    cv.rect(18, 36, 12, 6, shade(outfit, 0.8));
    if class == Class::Lord || l.class.is_promoted() {
        cv.rect(12, 38, 4, 10, gold);
        cv.rect(32, 38, 4, 10, gold);
    }
    // Neck and face
    cv.rect(19, 32, 10, 6, shade(skin, 0.82));
    cv.rect(14, 12, 20, 22, skin);
    cv.rect(15, 33, 18, 2, skin);
    cv.rect(17, 35, 14, 1, shade(skin, 0.9));
    cv.rect(14, 12, 2, 20, shade(skin, 1.06));
    cv.rect(31, 13, 3, 20, shade(skin, 0.86));
    cv.rect(12, 20, 2, 6, shade(skin, 0.9));
    cv.rect(34, 20, 2, 6, shade(skin, 0.82));
    // Eyes and brows
    let iris = if l.pale { rgb(140, 200, 255) } else { rgb(70, 110, 160) };
    for ex in [18, 27] {
        cv.rect(ex, 21, 4, 3, white);
        cv.rect(ex + 1, 21, 2, 3, iris);
        cv.set(ex + 1, 22, ink);
        cv.set(ex + 2, 22, ink);
        cv.rect(ex - 1, 18, 6, 1, shade(hair, 0.7));
        cv.rect(ex, 20, 4, 1, ink);
    }
    if l.class == Class::Sorcerer {
        cv.rect(19, 22, 2, 1, rgb(255, 70, 70));
        cv.rect(28, 22, 2, 1, rgb(255, 70, 70));
    }
    // Nose and mouth
    cv.rect(24, 24, 1, 4, shade(skin, 0.8));
    cv.rect(23, 28, 3, 1, shade(skin, 0.78));
    cv.rect(21, 30, 7, 1, rgb(160, 80, 80));
    if l.style == Hair::Bearded {
        cv.rect(15, 27, 18, 8, hair);
        cv.rect(17, 34, 14, 3, shade(hair, 0.85));
        cv.rect(21, 30, 7, 1, shade(hair, 0.6));
    }
    // Hair / headgear
    match class {
        Class::Lancer | Class::Soldier | Class::Knight => {
            cv.rect(11, 6, 26, 10, metal);
            cv.rect(11, 6, 26, 2, shade(metal, 1.2));
            cv.rect(11, 14, 4, 14, metal);
            cv.rect(33, 14, 4, 14, shade(metal, 0.8));
            cv.rect(22, 3, 4, 4, main);
        }
        Class::Archer => {
            let hood = if l.team == Team::Player { rgb(70, 130, 80) } else { shade(main, 0.8) };
            cv.rect(10, 6, 28, 10, hood);
            cv.rect(10, 14, 5, 22, hood);
            cv.rect(33, 14, 5, 22, shade(hood, 0.8));
            cv.rect(14, 15, 20, 2, hair);
        }
        Class::Mage => {
            let hat = if l.class == Class::Sorcerer { rgb(40, 20, 56) } else { main };
            cv.rect(6, 13, 36, 3, shade(hat, 0.8));
            for k in 0..11 {
                cv.rect(13 + k, 12 - k, 22 - k * 2, 1, hat);
            }
            cv.rect(15, 16, 18, 2, hair);
            if l.class == Class::Sorcerer {
                for k in 0..5 {
                    cv.rect(13 + k * 5, 6, 2, 7, gold);
                }
            }
        }
        Class::Cleric => {
            cv.rect(10, 7, 28, 9, white);
            cv.rect(10, 14, 5, 22, white);
            cv.rect(33, 14, 5, 22, shade(white, 0.85));
            cv.rect(12, 9, 24, 1, gold);
            cv.rect(15, 15, 18, 2, hair);
        }
        Class::Warlord => {
            cv.rect(11, 6, 26, 9, rgb(70, 66, 80));
            cv.line(11, 9, 5, 0, white);
            cv.line(12, 9, 6, 0, white);
            cv.line(36, 9, 42, 0, white);
            cv.line(35, 9, 41, 0, white);
        }
        _ => {
            cv.rect(12, 7, 24, 7, hair);
            cv.rect(11, 10, 4, 12, hair);
            cv.rect(33, 10, 4, 10, shade(hair, 0.85));
            cv.rect(16, 13, 8, 3, hair);
            cv.rect(26, 13, 6, 2, hair);
            cv.rect(15, 8, 10, 2, shade(hair, 1.25));
            if class == Class::Lord {
                cv.rect(14, 12, 20, 1, gold);
                cv.rect(23, 11, 2, 2, rgb(120, 220, 255));
            }
            if class == Class::Brigand || class == Class::Fighter {
                cv.rect(12, 11, 24, 2, if class == Class::Brigand { main } else { rgb(200, 190, 160) });
            }
        }
    }
    cv.outline(ink);
    cv
}

/// Character portrait for dialogue: a framed face bust.
pub fn draw_portrait(look: UnitLook, x: f32, y: f32, size: f32) {
    let (main, _) = team_colors(look.team, look.boss);
    draw_rectangle(x, y, size, size, rgb(54, 60, 96));
    draw_rectangle(x, y + size * 0.55, size, size * 0.45, rgb(44, 48, 80));
    let mut key = look;
    key.weapon = WeaponKind::Sword; // portraits don't depend on the weapon
    let tex = cached(Key::Portrait(key), || paint_portrait(&key).texture(false));
    blit(&tex, x + size * 0.04, y + size * 0.04, size * 0.92, size * 0.92, false, 1.0);
    draw_rectangle_lines(x, y, size, size, 3.0, main);
}
