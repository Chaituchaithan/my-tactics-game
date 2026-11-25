//! Pixel art drawn in code on a 16x16 grid per tile.

use macroquad::prelude::*;

use crate::logic::terrain::Terrain;
use crate::logic::unit::{Class, Team, WeaponKind};

pub fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgba(r, g, b, 255)
}

/// Draws "virtual pixels" of a 16x16 sprite at an on-screen tile.
struct Px {
    x: f32,
    y: f32,
    p: f32,
    flip: bool,
    grey: bool,
    alpha: f32,
}

impl Px {
    fn new(x: f32, y: f32, size: f32) -> Px {
        Px { x, y, p: size / 16.0, flip: false, grey: false, alpha: 1.0 }
    }

    fn tint(&self, c: Color) -> Color {
        let mut c = c;
        if self.grey {
            let l = c.r * 0.3 + c.g * 0.59 + c.b * 0.11;
            let l = l * 0.75 + 0.05;
            c = Color::new(l, l, l * 1.05, c.a);
        }
        c.a *= self.alpha;
        c
    }

    fn r(&self, gx: f32, gy: f32, w: f32, h: f32, c: Color) {
        let gx = if self.flip { 16.0 - gx - w } else { gx };
        let x0 = (self.x + gx * self.p).floor();
        let y0 = (self.y + gy * self.p).floor();
        let x1 = (self.x + (gx + w) * self.p).floor();
        let y1 = (self.y + (gy + h) * self.p).floor();
        draw_rectangle(x0, y0, (x1 - x0).max(1.0), (y1 - y0).max(1.0), self.tint(c));
    }

    fn dot(&self, gx: f32, gy: f32, c: Color) {
        self.r(gx, gy, 1.0, 1.0, c);
    }
}

fn hash(x: i32, y: i32, k: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263) ^ (k as u32).wrapping_mul(2_246_822_519);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^ (h >> 16)
}

const GRASS: (u8, u8, u8) = (96, 164, 76);

fn grass(px: &Px, tx: i32, ty: i32) {
    let (r, g, b) = GRASS;
    px.r(0.0, 0.0, 16.0, 16.0, rgb(r, g, b));
    for k in 0..5 {
        let h = hash(tx, ty, k);
        let gx = (h % 15) as f32;
        let gy = ((h >> 8) % 15) as f32;
        if k % 2 == 0 {
            px.r(gx, gy, 1.0, 1.0, rgb(124, 190, 92));
        } else {
            px.r(gx, gy, 1.0, 2.0, rgb(72, 132, 60));
            px.r(gx + 1.0, gy + 1.0, 1.0, 1.0, rgb(72, 132, 60));
        }
    }
}

fn tree(px: &Px, cx: f32, top: f32) {
    let dark = rgb(30, 92, 46);
    let mid = rgb(44, 122, 58);
    let light = rgb(70, 152, 72);
    px.r(cx - 0.5, top + 6.0, 2.0, 2.0, rgb(98, 64, 36));
    px.r(cx, top, 1.0, 1.0, mid);
    px.r(cx - 1.0, top + 1.0, 3.0, 1.0, mid);
    px.r(cx - 2.0, top + 2.0, 5.0, 2.0, mid);
    px.r(cx - 3.0, top + 4.0, 7.0, 2.0, dark);
    px.r(cx - 1.0, top + 2.0, 1.0, 1.0, light);
    px.r(cx - 2.0, top + 4.0, 2.0, 1.0, mid);
}

fn floor(px: &Px, tx: i32, ty: i32) {
    let a = rgb(118, 112, 124);
    let b = rgb(104, 98, 112);
    px.r(0.0, 0.0, 16.0, 16.0, if (tx + ty) % 2 == 0 { a } else { b });
    px.r(0.0, 0.0, 16.0, 1.0, rgb(90, 84, 98));
    px.r(0.0, 0.0, 1.0, 16.0, rgb(90, 84, 98));
}

fn bricks(px: &Px, tx: i32, ty: i32) {
    px.r(0.0, 0.0, 16.0, 16.0, rgb(70, 66, 82));
    let mortar = rgb(48, 44, 58);
    for row in 0..4 {
        let y = row as f32 * 4.0;
        px.r(0.0, y, 16.0, 1.0, mortar);
        let off = if (row + ty) % 2 == 0 { 0.0 } else { 4.0 };
        for k in 0..3 {
            px.r((off + k as f32 * 8.0 + (tx % 2) as f32) % 16.0, y, 1.0, 4.0, mortar);
        }
    }
    px.r(0.0, 0.0, 16.0, 2.0, rgb(96, 92, 108));
}

/// `alt` marks a visited village.
#[allow(clippy::too_many_arguments)]
pub fn draw_terrain(t: Terrain, x: f32, y: f32, size: f32, tx: i32, ty: i32, time: f32, alt: bool) {
    let px = Px::new(x, y, size);
    match t {
        Terrain::Village => {
            grass(&px, tx, ty);
            let roof = if alt { rgb(110, 90, 80) } else { rgb(196, 70, 52) };
            let wall = rgb(226, 206, 160);
            px.r(3.0, 8.0, 10.0, 6.0, wall);
            px.r(2.0, 6.0, 12.0, 2.0, roof);
            px.r(4.0, 4.0, 8.0, 2.0, roof);
            px.r(6.0, 2.0, 4.0, 2.0, roof);
            px.r(7.0, 10.0, 2.0, 4.0, if alt { rgb(60, 50, 44) } else { rgb(120, 76, 40) });
            px.r(4.0, 9.0, 2.0, 2.0, rgb(120, 170, 220));
            px.r(10.0, 9.0, 2.0, 2.0, rgb(120, 170, 220));
            px.r(2.0, 14.0, 12.0, 1.0, rgb(70, 120, 56));
        }
        Terrain::Floor => floor(&px, tx, ty),
        Terrain::Wall => bricks(&px, tx, ty),
        Terrain::Pillar => {
            floor(&px, tx, ty);
            px.r(5.0, 1.0, 6.0, 14.0, rgb(170, 166, 180));
            px.r(4.0, 1.0, 8.0, 2.0, rgb(196, 192, 206));
            px.r(4.0, 13.0, 8.0, 2.0, rgb(196, 192, 206));
            px.r(9.0, 3.0, 2.0, 10.0, rgb(130, 126, 142));
        }
        Terrain::Throne => {
            floor(&px, tx, ty);
            px.r(4.0, 0.0, 8.0, 16.0, rgb(150, 30, 40));
            px.r(5.0, 2.0, 6.0, 9.0, rgb(210, 170, 60));
            px.r(6.0, 3.0, 4.0, 6.0, rgb(120, 24, 60));
            px.r(4.0, 9.0, 8.0, 3.0, rgb(210, 170, 60));
            px.r(7.0, 1.0, 2.0, 1.0, rgb(250, 230, 120));
        }
        Terrain::Gate => {
            bricks(&px, tx, ty);
            px.r(3.0, 3.0, 10.0, 13.0, rgb(40, 32, 30));
            for i in 0..4 {
                px.r(4.0 + i as f32 * 2.6, 3.0, 1.0, 13.0, rgb(120, 116, 130));
            }
            px.r(3.0, 7.0, 10.0, 1.0, rgb(120, 116, 130));
            px.r(3.0, 11.0, 10.0, 1.0, rgb(120, 116, 130));
        }
        Terrain::Plains => grass(&px, tx, ty),
        Terrain::Forest => {
            grass(&px, tx, ty);
            px.r(1.0, 12.0, 14.0, 3.0, rgb(70, 128, 58));
            tree(&px, 4.0, 2.0);
            tree(&px, 11.0, 5.0);
            tree(&px, 6.0, 8.0);
        }
        Terrain::Mountain => {
            px.r(0.0, 0.0, 16.0, 16.0, rgb(150, 156, 92));
            let rock = rgb(128, 112, 96);
            let light = rgb(176, 160, 138);
            let shadow = rgb(92, 80, 70);
            for row in 0..12 {
                let half = (row as f32 * 0.65 + 1.0).min(7.5);
                let y0 = 3.0 + row as f32;
                px.r(8.0 - half, y0, half, 1.0, light);
                px.r(8.0, y0, half, 1.0, rock);
            }
            px.r(11.0, 10.0, 3.0, 5.0, shadow);
            px.r(7.0, 3.0, 2.0, 1.0, rgb(240, 240, 248));
            px.r(6.0, 4.0, 4.0, 1.0, rgb(240, 240, 248));
            px.r(5.0, 5.0, 2.0, 1.0, rgb(240, 240, 248));
            px.r(0.0, 15.0, 16.0, 1.0, shadow);
        }
        Terrain::Fort => {
            grass(&px, tx, ty);
            let stone = rgb(168, 166, 176);
            let dark = rgb(106, 104, 118);
            px.r(2.0, 5.0, 12.0, 10.0, stone);
            for i in 0..4 {
                px.r(2.0 + i as f32 * 3.5, 3.0, 2.0, 2.0, stone);
            }
            px.r(2.0, 9.0, 12.0, 1.0, dark);
            px.r(6.0, 10.0, 4.0, 5.0, rgb(70, 50, 40));
            px.r(2.0, 14.0, 12.0, 1.0, dark);
            px.r(7.0, -1.0, 1.0, 4.0, rgb(80, 60, 40));
            let wave = ((time * 3.0 + tx as f32).sin() > 0.0) as i32 as f32;
            px.r(8.0, -1.0, 3.0 + wave, 2.0, rgb(214, 170, 60));
        }
        Terrain::Water => {
            px.r(0.0, 0.0, 16.0, 16.0, rgb(52, 102, 196));
            let light = rgb(108, 158, 232);
            for k in 0..3 {
                let h = hash(tx, ty, k + 10);
                let gy = 2.0 + k as f32 * 5.0 + (h % 2) as f32;
                let shift = ((time * 2.0 + (h % 7) as f32).floor() as i32).rem_euclid(12) as f32;
                px.r(shift, gy, 3.0, 1.0, light);
                px.r((shift + 7.0) % 13.0, gy + 2.0, 2.0, 1.0, light);
            }
        }
        Terrain::Bridge => {
            px.r(0.0, 0.0, 16.0, 16.0, rgb(52, 102, 196));
            px.r(0.0, 2.0, 16.0, 12.0, rgb(160, 110, 62));
            for i in 0..4 {
                px.r(i as f32 * 4.0 + 3.0, 2.0, 1.0, 12.0, rgb(116, 76, 42));
            }
            px.r(0.0, 2.0, 16.0, 1.0, rgb(96, 62, 34));
            px.r(0.0, 13.0, 16.0, 1.0, rgb(96, 62, 34));
        }
    }
}

#[derive(Clone, Copy)]
pub struct UnitLook {
    pub class: Class,
    pub team: Team,
    pub weapon: WeaponKind,
    pub boss: bool,
}

impl UnitLook {
    pub fn of(u: &crate::logic::unit::Unit) -> UnitLook {
        let weapon = u.weapon().map(|w| w.kind).or_else(|| u.staff().map(|w| w.kind)).unwrap_or(match u.class.info().kinds.first() {
            Some(k) => *k,
            None => WeaponKind::Sword,
        });
        UnitLook { class: u.class, team: u.team, weapon, boss: u.is_boss }
    }
}

pub fn team_colors(team: Team, boss: bool) -> (Color, Color) {
    match (team, boss) {
        (Team::Player, _) => (rgb(64, 116, 222), rgb(32, 58, 140)),
        (Team::Enemy, false) => (rgb(204, 60, 50), rgb(120, 30, 30)),
        (Team::Enemy, true) => (rgb(150, 52, 140), rgb(84, 24, 80)),
    }
}

/// Draws a unit sprite. `bob` shifts the sprite up by one virtual pixel.
pub fn draw_unit(look: UnitLook, x: f32, y: f32, size: f32, done: bool, alpha: f32, bob: bool) {
    let mut px = Px::new(x, y + if bob { -size / 16.0 } else { 0.0 }, size);
    px.flip = look.team == Team::Enemy;
    px.grey = done;
    px.alpha = alpha;

    let (main, dark) = team_colors(look.team, look.boss);
    let skin = rgb(240, 196, 150);
    let outline = rgb(30, 24, 30);
    let metal = rgb(210, 214, 224);
    let wood = rgb(130, 84, 44);
    let gold = rgb(232, 190, 64);
    let boots = rgb(70, 46, 30);
    let white = rgb(236, 236, 240);

    // Shadow (not greyed / flipped meaningfully)
    draw_rectangle(x + size * 0.22, y + size * 0.86, size * 0.56, size * 0.1, Color::new(0.0, 0.0, 0.0, 0.25 * alpha));

    let promoted = look.class.is_promoted();
    let sorcerer = look.class == Class::Sorcerer;
    let class = if sorcerer { Class::Mage } else { look.class.base_look() };
    let (main, dark) = if sorcerer { (rgb(60, 30, 80), rgb(28, 12, 40)) } else { (main, dark) };
    let robed = matches!(class, Class::Mage | Class::Cleric);
    let robe = if class == Class::Cleric { white } else { main };

    // Cape for lords, bosses and promoted units
    if matches!(class, Class::Lord | Class::Warlord) || promoted {
        px.r(3.0, 7.0, 9.0, 7.0, dark);
        if promoted {
            px.r(3.0, 13.0, 9.0, 1.0, gold);
        }
    }

    // Legs or robe
    if robed {
        px.r(4.0, 7.0, 8.0, 7.0, robe);
        px.r(4.0, 13.0, 8.0, 1.0, dark);
        px.r(7.0, 7.0, 2.0, 7.0, if class == Class::Cleric { main } else { dark });
    } else {
        px.r(5.0, 11.0, 2.0, 3.0, dark);
        px.r(9.0, 11.0, 2.0, 3.0, dark);
        px.r(5.0, 13.0, 2.0, 1.0, boots);
        px.r(9.0, 13.0, 2.0, 1.0, boots);
        // Torso
        let torso = if matches!(class, Class::Brigand) { rgb(150, 110, 70) } else { main };
        px.r(4.0, 7.0, 8.0, 4.0, torso);
        px.r(4.0, 10.0, 8.0, 1.0, wood);
        if matches!(class, Class::Soldier | Class::Lancer | Class::Warlord) {
            px.r(5.0, 7.0, 6.0, 2.0, metal);
        }
        if class == Class::Knight {
            px.r(3.0, 6.0, 10.0, 6.0, metal);
            px.r(4.0, 7.0, 8.0, 3.0, main);
            px.r(4.0, 11.0, 3.0, 3.0, metal);
            px.r(9.0, 11.0, 3.0, 3.0, metal);
        }
        if class == Class::Warlord {
            px.r(3.0, 6.0, 3.0, 2.0, gold);
            px.r(10.0, 6.0, 3.0, 2.0, gold);
        }
    }

    // Arms
    let arm = if matches!(class, Class::Brigand | Class::Fighter) { skin } else { dark };
    px.r(3.0, 7.0, 1.0, 3.0, arm);
    px.r(12.0, 7.0, 1.0, 3.0, arm);
    px.dot(3.0, 10.0, skin);
    px.dot(12.0, 10.0, skin);

    // Head
    px.r(5.0, 2.0, 6.0, 5.0, skin);
    px.r(5.0, 6.0, 6.0, 1.0, rgb(220, 170, 128));
    px.dot(9.0, 4.0, outline);
    px.dot(7.0, 4.0, outline);

    // Headgear / hair
    match class {
        Class::Lord => {
            px.r(5.0, 1.0, 6.0, 2.0, rgb(120, 70, 40));
            px.r(4.0, 2.0, 1.0, 3.0, rgb(120, 70, 40));
            px.r(6.0, 1.0, 4.0, 1.0, gold);
        }
        Class::Mercenary => {
            px.r(5.0, 1.0, 6.0, 2.0, rgb(180, 80, 40));
            px.r(5.0, 2.0, 6.0, 1.0, white);
            px.r(4.0, 2.0, 1.0, 2.0, white);
        }
        Class::Lancer | Class::Soldier => {
            px.r(4.0, 0.0, 8.0, 3.0, metal);
            px.r(4.0, 3.0, 1.0, 2.0, metal);
            px.r(11.0, 3.0, 1.0, 2.0, metal);
            px.r(7.0, -1.0, 2.0, 1.0, main);
        }
        Class::Brigand => {
            px.r(5.0, 1.0, 6.0, 2.0, main);
            px.r(4.0, 2.0, 1.0, 2.0, main);
            px.r(6.0, 6.0, 4.0, 1.0, rgb(90, 50, 30));
        }
        Class::Fighter => {
            px.r(5.0, 1.0, 6.0, 2.0, rgb(90, 60, 40));
            px.r(4.0, 2.0, 8.0, 1.0, main);
        }
        Class::Knight => {
            px.r(4.0, 0.0, 8.0, 7.0, metal);
            px.r(5.0, 3.0, 6.0, 2.0, rgb(40, 40, 50));
            px.r(6.0, -1.0, 4.0, 1.0, main);
        }
        Class::Warlord => {
            px.r(4.0, 0.0, 8.0, 3.0, rgb(70, 70, 82));
            px.r(3.0, -2.0, 1.0, 3.0, white);
            px.r(12.0, -2.0, 1.0, 3.0, white);
            px.r(6.0, 6.0, 4.0, 1.0, rgb(60, 40, 30));
        }
        Class::Archer => {
            px.r(4.0, 1.0, 8.0, 2.0, dark);
            px.r(4.0, 3.0, 1.0, 4.0, dark);
            px.r(11.0, 3.0, 1.0, 3.0, dark);
        }
        Class::Mage | Class::Sage | Class::Sorcerer => {
            if sorcerer {
                px.r(3.0, -2.0, 1.0, 3.0, rgb(200, 190, 210));
                px.r(12.0, -2.0, 1.0, 3.0, rgb(200, 190, 210));
            }
            px.r(4.0, 2.0, 8.0, 1.0, dark);
            px.r(5.0, 1.0, 6.0, 1.0, main);
            px.r(6.0, 0.0, 4.0, 1.0, main);
            px.r(7.0, -1.0, 3.0, 1.0, main);
            px.r(9.0, -2.0, 2.0, 1.0, main);
        }
        Class::Cleric | Class::Bishop => {
            px.r(4.0, 1.0, 8.0, 2.0, white);
            px.r(4.0, 3.0, 1.0, 4.0, white);
            px.r(11.0, 3.0, 1.0, 4.0, white);
            px.r(5.0, 1.0, 6.0, 1.0, gold);
        }
        _ => {}
    }
    if promoted {
        px.r(7.0, 7.0, 2.0, 2.0, gold);
    }

    // Weapon on the facing side
    match look.weapon {
        WeaponKind::Sword => {
            px.r(13.0, 2.0, 1.0, 7.0, metal);
            px.r(12.0, 9.0, 3.0, 1.0, gold);
            px.r(13.0, 10.0, 1.0, 2.0, wood);
        }
        WeaponKind::Lance => {
            px.r(13.0, 2.0, 1.0, 12.0, wood);
            px.r(13.0, -1.0, 1.0, 3.0, metal);
            px.r(12.0, 1.0, 3.0, 1.0, metal);
        }
        WeaponKind::Axe => {
            px.r(13.0, 3.0, 1.0, 10.0, wood);
            px.r(14.0, 2.0, 2.0, 4.0, metal);
            px.r(12.0, 3.0, 1.0, 2.0, metal);
        }
        WeaponKind::Bow => {
            px.dot(13.0, 2.0, wood);
            px.dot(14.0, 3.0, wood);
            px.r(15.0, 4.0, 1.0, 6.0, wood);
            px.dot(14.0, 10.0, wood);
            px.dot(13.0, 11.0, wood);
            px.r(13.0, 3.0, 1.0, 8.0, white);
        }
        WeaponKind::Magic => {
            px.r(11.0, 8.0, 4.0, 3.0, if look.team == Team::Player { rgb(200, 60, 40) } else { rgb(80, 40, 120) });
            px.r(11.0, 8.0, 4.0, 1.0, gold);
            px.dot(14.0, 6.0, rgb(255, 200, 80));
        }
        WeaponKind::Staff => {
            px.r(13.0, 2.0, 1.0, 12.0, wood);
            px.r(12.0, 0.0, 3.0, 3.0, gold);
            px.dot(13.0, 1.0, white);
        }
    }
}

/// Character portrait for dialogue: the sprite, large, in a framed box.
pub fn draw_portrait(look: UnitLook, x: f32, y: f32, size: f32) {
    draw_rectangle(x, y, size, size, rgb(54, 60, 96));
    draw_rectangle(x, y + size * 0.6, size, size * 0.4, rgb(44, 48, 80));
    let mut l = look;
    l.team = Team::Player; // face right
    let (main, _) = team_colors(look.team, look.boss);
    draw_unit(UnitLook { team: look.team, ..l }, x + size * 0.05, y + size * 0.12, size * 0.9, false, 1.0, false);
    draw_rectangle_lines(x, y, size, size, 3.0, main);
}
