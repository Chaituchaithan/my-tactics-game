//! Preparations between chapters: briefing, army management and the shop.

use macroquad::prelude::*;

use super::gfx::{self, rgb, UnitLook};
use super::widgets::*;
use super::battle_screen::dialogue_box;
use crate::logic::campaign::{Campaign, Line};
use crate::logic::supports::{self, THRESHOLDS};
use crate::logic::items::{ItemKind, MAX_ITEMS, SHOP};
use crate::logic::unit::{Unit, STAT_NAMES};

pub enum PrepExit {
    Stay,
    Begin,
    Title,
}

#[derive(PartialEq)]
enum Tab {
    Main,
    Army,
    Shop,
    Supports,
}

pub struct PrepScreen {
    tab: Tab,
    unit: usize,
    item: Option<usize>,
    shop_item: usize,
    msg: Option<(String, f32)>,
    time: f32,
    /// (chapter, deploy slots, objective, map size) computed once per chapter.
    info: Option<(usize, usize, String, String)>,
    /// Support conversation being shown: (lines, index, time on line).
    convo: Option<(Vec<Line>, usize, f32)>,
}

impl PrepScreen {
    pub fn new() -> PrepScreen {
        PrepScreen { tab: Tab::Main, unit: 0, item: None, shop_item: 0, msg: None, time: 0.0, info: None, convo: None }
    }

    fn say(&mut self, s: impl Into<String>) {
        self.msg = Some((s.into(), 2.0));
    }

    pub fn frame(&mut self, c: &mut Campaign) -> PrepExit {
        let dt = get_frame_time().min(0.05);
        self.time += dt;
        if let Some((_, t)) = &mut self.msg {
            *t -= dt;
            if *t <= 0.0 {
                self.msg = None;
            }
        }
        let (w, h) = (screen_width(), screen_height());
        let u = ui_unit();
        let mut input = Input::poll();
        draw_rectangle(0.0, 0.0, w, h, rgb(20, 22, 40));
        self.unit = self.unit.min(c.army.len().saturating_sub(1));
        if self.info.as_ref().is_none_or(|i| i.0 != c.chapter) {
            let b = c.battle(0);
            let def = c.def();
            let size = format!("{} map {}x{}, {} foes", def.tier.name(), b.map.width, b.map.height, b.living(crate::logic::unit::Team::Enemy).count() + b.reinforcements.len());
            let slots = b.living(crate::logic::unit::Team::Player).count().max(crate::logic::campaign::setup(c.chapter).starts.len());
            self.info = Some((c.chapter, slots, b.objective_text(), size));
        }
        let (_, slots, objective, size) = self.info.clone().unwrap();

        if let Some((lines, idx, t)) = &mut self.convo {
            *t += dt;
            let line = lines[*idx];
            let look = c.army.iter().find(|u| u.name == line.speaker).map(UnitLook::of);
            draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.5));
            dialogue_box(Rect::new(0.0, 0.0, w, h), u, line.speaker, line.text, look, *t);
            if input.take().is_some() {
                if *t < line.text.len() as f32 / 90.0 {
                    *t = 100.0;
                } else {
                    *idx += 1;
                    *t = 0.0;
                    if *idx >= lines.len() {
                        self.convo = None;
                    }
                }
            }
            return PrepExit::Stay;
        }

        let m = u * 0.6;
        let left = Rect::new(m, m, (w * 0.42).max(u * 12.0).min(w - u * 14.0), h - m * 2.0);
        let right = Rect::new(left.x + left.w + m, m, w - left.w - m * 3.0, h - m * 2.0);
        let exit = match self.tab {
            Tab::Main => self.main(c, &mut input, left, right, u, slots, &objective, &size),
            Tab::Army => {
                self.army(c, &mut input, left, right, u, slots);
                PrepExit::Stay
            }
            Tab::Shop => {
                self.shop(c, &mut input, left, right, u);
                PrepExit::Stay
            }
            Tab::Supports => {
                self.supports(c, &mut input, left, right, u);
                PrepExit::Stay
            }
        };
        if let Some((s, t)) = &self.msg {
            let a = t.min(1.0);
            let mw = text_w(s, u) + u * 2.0;
            let r = Rect::new((w - mw) / 2.0, h - u * 3.0, mw, u * 2.0);
            draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.0, 0.0, 0.0, 0.8 * a));
            text_centered(s, w / 2.0, r.y + u * 1.35, u, Color::new(1.0, 0.9, 0.5, a));
        }
        exit
    }

    #[allow(clippy::too_many_arguments)]
    fn main(&mut self, c: &mut Campaign, input: &mut Input, left: Rect, right: Rect, u: f32, slots: usize, objective: &str, size: &str) -> PrepExit {
        let def = c.def();
        frame(left);
        let mut y = left.y + u * 1.8;
        text(&format!("Chapter {}", c.chapter + 1), left.x + u, y, u, DIM);
        y += u * 1.6;
        for line in wrap(def.title, left.w - u * 2.0, u * 1.5) {
            text(&line, left.x + u, y, u * 1.5, ACCENT);
            y += u * 1.7;
        }
        text(&format!("Goal: {}", objective), left.x + u, y, u * 0.85, INK);
        y += u * 1.1;
        text(size, left.x + u, y, u * 0.8, DIM);
        y += u * 1.0;
        text(&format!("Deploy {} of {} units", slots.min(c.army.len()), c.army.len()), left.x + u, y, u * 0.8, DIM);
        y += u * 1.0;
        text(&format!("Gold: {}", c.gold), left.x + u, y, u * 0.85, ACCENT);
        y += u * 1.0;
        let mode = if c.permadeath { "Permadeath: ON" } else { "Permadeath: OFF" };
        text(mode, left.x + u, y, u * 0.8, if c.permadeath { BAD } else { DIM });
        y += u * 1.4;
        let dep = c.deployment(slots);
        let size = (u * 2.6).min((left.w - u * 2.0) / 6.0);
        let mut x = left.x + u;
        for (k, unit) in c.army.iter().enumerate() {
            if x + size > left.x + left.w - u {
                x = left.x + u;
                y += size + u * 0.3;
            }
            if y + size > left.y + left.h - u * 0.5 {
                break;
            }
            let bob = ((self.time * 2.0) as usize + k) & 1 == 0;
            gfx::draw_unit(UnitLook::of(unit), x, y, size, !dep.contains(&k), 1.0, bob);
            x += size + u * 0.2;
        }

        let mut col = ButtonColumn::new(right, u);
        let mut exit = PrepExit::Stay;
        if button(input, col.next(), "Save & Quit", ButtonStyle::Normal, u) {
            exit = PrepExit::Title;
        }
        let scenes = if c.battle_scenes { "Battle Scenes: On" } else { "Battle Scenes: Off" };
        if button(input, col.next(), scenes, ButtonStyle::Normal, u) {
            c.battle_scenes = !c.battle_scenes;
        }
        let ready = c.support_list().iter().filter(|s| s.ready()).count();
        let label = if ready > 0 { format!("Supports ({} new!)", ready) } else { "Supports".to_string() };
        if button(input, col.next(), &label, if ready > 0 { ButtonStyle::Primary } else { ButtonStyle::Normal }, u) {
            self.tab = Tab::Supports;
        }
        if button(input, col.next(), "Shop", ButtonStyle::Normal, u) {
            self.tab = Tab::Shop;
        }
        if button(input, col.next(), "Army & Items", ButtonStyle::Normal, u) {
            self.tab = Tab::Army;
            self.item = None;
        }
        if button(input, col.next(), "Begin Battle", ButtonStyle::Primary, u) {
            exit = PrepExit::Begin;
        }
        let top = col.top();
        let notes = [
            "Units heal fully between chapters.",
            "Level 10+ units can promote",
            "with a Master Seal.",
            "Units at the top of the army",
            "list are deployed first.",
        ];
        let mut ny = right.y + u * 1.5;
        for n in notes {
            if ny > top - u {
                break;
            }
            text(n, right.x, ny, u * 0.75, DIM);
            ny += u * 1.0;
        }
        exit
    }

    fn unit_list(&mut self, c: &Campaign, input: &mut Input, area: Rect, u: f32, slots: usize) {
        let n = c.army.len().max(1);
        let cols = if n > 7 { 2 } else { 1 };
        let rows = n.div_ceil(cols);
        let bh = ((area.h - u * 3.0) / rows as f32 - u * 0.25).min(u * 2.0);
        let bw = (area.w - u * 0.5 * (cols - 1) as f32) / cols as f32;
        let deployed = c.deployment(slots);
        for (i, unit) in c.army.iter().enumerate() {
            let col = i / rows;
            let row = i % rows;
            let r = Rect::new(area.x + col as f32 * (bw + u * 0.5), area.y + u * 2.4 + row as f32 * (bh + u * 0.25), bw, bh);
            let style = if i == self.unit { ButtonStyle::Primary } else { ButtonStyle::Normal };
            let mark = if deployed.contains(&i) { "" } else { " (bench)" };
            if button(input, r, &format!("{} Lv{}{}", unit.name, unit.level, mark), style, u * 0.8) {
                self.unit = i;
                self.item = None;
            }
        }
    }

    fn army(&mut self, c: &mut Campaign, input: &mut Input, left: Rect, right: Rect, u: f32, slots: usize) {
        text("Army", left.x, left.y + u * 1.4, u * 1.4, ACCENT);
        self.unit_list(c, input, left, u, slots);
        if c.army.is_empty() {
            return;
        }
        frame(right);
        let x = right.x + u;
        let w = right.w - u * 2.0;
        let un = &c.army[self.unit];
        let mut y = unit_sheet(un, x, right.y + u * 1.6, w, u, self.time);

        let eq = un.weapon_index();
        let items = un.items.clone();
        let ih = u * 1.7;
        for (i, it) in items.iter().enumerate() {
            let r = Rect::new(x, y, w, ih);
            let style = if self.item == Some(i) { ButtonStyle::Primary } else { ButtonStyle::Normal };
            let usable = if c.army[self.unit].can_use(it.id) { "" } else { " (can't use)" };
            let label = format!("{}{}{}", if eq == Some(i) { "* " } else { "" }, it.label(), usable);
            if button(input, r, &label, style, u * 0.8) {
                self.item = if self.item == Some(i) { None } else { Some(i) };
            }
            y += ih + u * 0.2;
        }
        if items.is_empty() {
            text("No items.", x, y + u, u * 0.8, DIM);
        }

        let mut col = ButtonColumn::grid(Rect::new(x, right.y, w, right.h - u * 0.8), u, 2);
        if button(input, col.next(), "Back", ButtonStyle::Normal, u) {
            self.tab = Tab::Main;
            return;
        }
        let n = c.army.len();
        if button(input, col.next(), "Move Up", ButtonStyle::Normal, u) && self.unit > 0 && !c.army[self.unit - 1].is_lord {
            c.army.swap(self.unit, self.unit - 1);
            self.unit -= 1;
        }
        if let Some(i) = self.item.filter(|i| *i < c.army[self.unit].items.len()) {
            let id = c.army[self.unit].items[i].id;
            if button(input, col.next(), "Discard", ButtonStyle::Danger, u) {
                c.army[self.unit].items.remove(i);
                self.item = None;
                self.say(format!("Discarded {}", id.name()));
            }
            if n > 1 && button(input, col.next(), "Give Next", ButtonStyle::Normal, u) {
                let from = self.unit;
                let to = (1..n).map(|k| (from + k) % n).find(|&j| c.army[j].items.len() < MAX_ITEMS);
                if let Some(j) = to {
                    let it = c.army[from].items.remove(i);
                    c.army[j].items.push(it);
                    self.item = None;
                    self.say(format!("Gave {} to {}", id.name(), c.army[j].name));
                }
            }
            match id.info().kind {
                ItemKind::Weapon(_) => {
                    if c.army[self.unit].can_use(id) && button(input, col.next(), "Equip", ButtonStyle::Primary, u) {
                        c.army[self.unit].equip(i);
                        self.item = Some(0);
                    }
                }
                ItemKind::Seal => {
                    if button(input, col.next(), "Use Seal", ButtonStyle::Primary, u) {
                        if c.army[self.unit].can_promote() {
                            let p = c.army[self.unit].promote().unwrap();
                            c.army[self.unit].spend(i);
                            c.army[self.unit].hp = c.army[self.unit].stats.hp;
                            self.item = None;
                            self.say(format!("{} became a {}!", c.army[self.unit].name, p.to.name()));
                        } else {
                            self.say("Needs level 10 or higher.");
                        }
                    }
                }
                ItemKind::Heal(_) => {}
            }
        }
    }

    fn supports(&mut self, c: &mut Campaign, input: &mut Input, left: Rect, right: Rect, u: f32) {
        text("Supports", left.x, left.y + u * 1.4, u * 1.4, ACCENT);
        let list = c.support_list();
        let rh = u * 2.0;
        let mut y = left.y + u * 2.4;
        let mut view = None;
        for st in &list {
            if y + rh > left.y + left.h {
                break;
            }
            let r = Rect::new(left.x, y, left.w, rh);
            draw_rectangle(r.x, r.y, r.w, r.h, rgb(36, 40, 70));
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, rgb(80, 70, 110));
            text(&format!("{} & {}", st.a, st.b), r.x + u * 0.5, r.y + u * 1.35, u * 0.85, ALLY);
            text(supports::level_name(st.level), r.x + r.w * 0.5, r.y + u * 1.35, u, ACCENT);
            // progress pips
            for k in 0..3u8 {
                let filled = st.level > k;
                let px = r.x + r.w * 0.56 + k as f32 * u * 0.9;
                let c = if filled { ACCENT } else { rgb(70, 70, 90) };
                draw_rectangle(px, r.y + u * 0.7, u * 0.6, u * 0.6, c);
            }
            let br = Rect::new(r.x + r.w - u * 5.5, r.y + u * 0.2, u * 5.2, rh - u * 0.4);
            if st.ready() {
                if button(input, br, "Talk!", ButtonStyle::Primary, u * 0.8) {
                    view = Some((st.a.clone(), st.b.clone()));
                }
            } else if st.level < supports::MAX_LEVEL {
                let need = THRESHOLDS[st.level as usize];
                text_right(&format!("{}/{}", st.points, need), br.x + br.w, r.y + u * 1.35, u * 0.8, DIM);
            } else {
                text_right("Max", br.x + br.w, r.y + u * 1.35, u * 0.8, GOOD);
            }
            y += rh + u * 0.25;
        }
        if list.is_empty() {
            text("No bonds yet.", left.x, y + u, u * 0.85, DIM);
        }
        if let Some((a, b)) = view {
            if let Some(lines) = c.view_support(&a, &b) {
                self.convo = Some((lines, 0, 0.0));
            }
        }

        frame(right);
        let x = right.x + u;
        let mut ty = right.y + u * 1.8;
        for line in [
            "Allies who end their turn within",
            "2 tiles of each other grow closer.",
            "",
            "Each support level (C, B, A) gives",
            "+5 Hit, +5 Avoid, +2 Crit and +2",
            "crit-dodge per level while the",
            "partner is within 3 tiles.",
            "",
            "View a conversation to raise",
            "the level.",
        ] {
            text(line, x, ty, u * 0.8, if line.is_empty() { DIM } else { INK });
            ty += u * 1.0;
        }
        let mut col = ButtonColumn::new(Rect::new(x, right.y, right.w - u * 2.0, right.h - u * 0.8), u);
        if button(input, col.next(), "Back", ButtonStyle::Normal, u) {
            self.tab = Tab::Main;
        }
    }

    fn shop(&mut self, c: &mut Campaign, input: &mut Input, left: Rect, right: Rect, u: f32) {
        text("Armory", left.x, left.y + u * 1.4, u * 1.4, ACCENT);
        text_right(&format!("{} G", c.gold), left.x + left.w, left.y + u * 1.4, u, ACCENT);
        let cols = 2;
        let rows = SHOP.len().div_ceil(cols);
        let bh = ((left.h - u * 2.6) / rows as f32 - u * 0.2).min(u * 2.0);
        let bw = (left.w - u * 0.4) / 2.0;
        for (i, id) in SHOP.iter().enumerate() {
            let col = i / rows;
            let row = i % rows;
            let r = Rect::new(left.x + col as f32 * (bw + u * 0.4), left.y + u * 2.4 + row as f32 * (bh + u * 0.2), bw, bh);
            let style = if i == self.shop_item { ButtonStyle::Primary } else { ButtonStyle::Normal };
            if button(input, r, id.name(), style, u * 0.75) {
                self.shop_item = i;
            }
        }

        frame(right);
        let x = right.x + u;
        let w = right.w - u * 2.0;
        let id = SHOP[self.shop_item];
        let mut y = right.y + u * 1.8;
        text(id.name(), x, y, u * 1.2, ACCENT);
        text_right(&format!("{} G", id.info().price), x + w, y, u, INK);
        y += u * 1.3;
        for line in wrap(&id.description(), w, u * 0.8) {
            text(&line, x, y, u * 0.8, INK);
            y += u * 1.0;
        }
        y += u * 0.6;
        if c.army.is_empty() {
            return;
        }
        let buyer = &c.army[self.unit];
        text("Buy for:", x, y, u * 0.8, DIM);
        y += u * 0.4;
        let half = (w - u * 0.4) / 2.0;
        let rb = Rect::new(x, y, half, u * 1.9);
        if button(input, rb, &format!("< {} >", buyer.name), ButtonStyle::Normal, u * 0.85) {
            self.unit = (self.unit + 1) % c.army.len();
        }
        let buyer = &c.army[self.unit];
        let ok = buyer.can_use(id);
        let can = if ok { "Can use" } else { "Can't use" };
        text(&format!("{}  ({}/{})", can, buyer.items.len(), MAX_ITEMS), x + half + u * 0.4, y + u * 1.25, u * 0.75, if ok { GOOD } else { BAD });
        y += u * 2.4;
        for it in &buyer.items {
            text(&format!("  {}", it.label()), x, y, u * 0.72, DIM);
            y += u * 0.85;
        }

        let mut col = ButtonColumn::grid(Rect::new(x, right.y, w, right.h - u * 0.8), u, 2);
        if button(input, col.next(), "Back", ButtonStyle::Normal, u) {
            self.tab = Tab::Main;
        }
        let price = id.info().price;
        let style = if c.gold >= price { ButtonStyle::Primary } else { ButtonStyle::Disabled };
        if button(input, col.next(), &format!("Buy {}G", price), style, u) {
            if c.army[self.unit].items.len() >= MAX_ITEMS {
                self.say(format!("{}'s bag is full.", c.army[self.unit].name));
            } else {
                c.gold -= price;
                c.army[self.unit].give(id);
                self.say(format!("{} bought a {}.", c.army[self.unit].name, id.name()));
            }
        }
    }
}

/// Portrait + stats block; returns the y below it.
fn unit_sheet(un: &Unit, x: f32, mut y: f32, w: f32, u: f32, time: f32) -> f32 {
    let ps = u * 4.0;
    let bob = (time * 2.0) as i32 % 2 == 0;
    gfx::draw_unit(UnitLook::of(un), x, y - u * 0.8, ps, false, 1.0, bob);
    let tx = x + ps + u * 0.5;
    text(&un.name, tx, y, u * 1.2, ALLY);
    y += u * 1.2;
    text(&format!("{}  Lv {}  EXP {}", un.class.name(), un.level, un.xp), tx, y, u * 0.8, DIM);
    y += u * 1.0;
    text(&format!("HP {}   Mov {}", un.stats.hp, un.mov()), tx, y, u * 0.8, INK);
    y += u * 1.4;
    let s = un.stats.to_array();
    let g = un.growths.to_array();
    let cw = w / 3.0;
    for i in 1..7 {
        let cx = x + ((i - 1) % 3) as f32 * cw;
        let cy = y + ((i - 1) / 3) as f32 * u * 1.0;
        let name = if i == 1 { un.pow_label() } else { STAT_NAMES[i] };
        text(name, cx, cy, u * 0.8, DIM);
        text_right(&s[i].to_string(), cx + cw * 0.62, cy, u * 0.8, INK);
        text(&format!("{}%", g[i]), cx + cw * 0.66, cy, u * 0.6, rgb(120, 130, 170));
    }
    y + u * 2.3
}
