//! Battle presentation: turns logic results into animations and handles taps.
//!
//! Logic changes are applied to `Battle` instantly; the screen then replays
//! them through a queue of visual effects (`Fx`) using its own copy of unit
//! positions/HP, so rendering never leaks into the rules.

use std::collections::{HashSet, VecDeque};
use std::f32::consts::PI;

use macroquad::prelude::*;

use super::gfx::{self, rgb, UnitLook};
use super::widgets::*;
use crate::logic::ai::{self, AiAction};
use crate::logic::battle::{Battle, CombatReport, Outcome, Phase, XpEvent};
use crate::logic::combat::{self, Forecast, Side, SideForecast};
use crate::logic::pathfinding::Reach;
use crate::logic::scenario::BATTLE_NAME;
use crate::logic::unit::{LevelUp, Team, Unit, STAT_NAMES};
use crate::logic::Pos;

pub enum BattleExit {
    Stay,
    Title,
    Retry,
}

const STEP_TIME: f32 = 0.07;
const STRIKE_TIME: f32 = 0.7;

enum Fx {
    Banner { text: String, color: Color },
    Pause(f32),
    Move { unit: usize, path: Vec<Pos> },
    CombatBegin { a: usize, d: usize },
    Strike { by: usize, target: usize, hit: bool, crit: bool, damage: i32, hp_after: i32 },
    CombatEnd,
    Heal { target: usize, amount: i32, hp_after: i32 },
    Die { unit: usize },
    Xp { unit: usize, from: i32, gained: i32 },
    LevelUp { unit: usize, lv: LevelUp },
}

impl Fx {
    /// None = waits for a tap.
    fn duration(&self) -> Option<f32> {
        Some(match self {
            Fx::Banner { .. } => 1.3,
            Fx::Pause(t) => *t,
            Fx::Move { path, .. } => STEP_TIME * path.len().saturating_sub(1) as f32,
            Fx::CombatBegin { .. } => 0.3,
            Fx::Strike { .. } => STRIKE_TIME,
            Fx::CombatEnd => 0.35,
            Fx::Heal { .. } => 0.8,
            Fx::Die { .. } => 0.7,
            Fx::Xp { .. } => 1.1,
            Fx::LevelUp { .. } => return None,
        })
    }
}

struct Float {
    text: String,
    pos: Vec2,
    t: f32,
    color: Color,
}

enum Mode {
    Idle,
    Selected { unit: usize, reach: Reach, dests: HashSet<Pos>, threat: HashSet<Pos> },
    Menu { unit: usize, from: Pos },
    Target { unit: usize, from: Pos, targets: Vec<usize>, heal: bool, chosen: Option<usize> },
    EnemyPhase { queue: VecDeque<usize> },
    Over { victory: bool },
}

pub struct BattleScreen {
    b: Battle,
    mode: Mode,
    fx: VecDeque<Fx>,
    fx_t: f32,
    fx_started: bool,
    vis_pos: Vec<Vec2>,
    vis_hp: Vec<i32>,
    vis_alpha: Vec<f32>,
    floats: Vec<Float>,
    combat_pair: Option<(usize, usize)>,
    inspect: Option<Pos>,
    /// Movement and threat area of an inspected enemy.
    inspect_range: Option<(HashSet<Pos>, HashSet<Pos>)>,
    show_danger: bool,
    danger: Option<HashSet<Pos>>,
    time: f32,
}

fn pv(p: Pos) -> Vec2 {
    vec2(p.x as f32, p.y as f32)
}

fn look(u: &Unit) -> UnitLook {
    UnitLook { class: u.class, team: u.team, weapon: u.weapon.kind, boss: u.is_boss }
}

fn team_ink(team: Team) -> Color {
    match team {
        Team::Player => ALLY,
        Team::Enemy => BAD,
    }
}

impl BattleScreen {
    pub fn new(seed: u64, permadeath: bool) -> BattleScreen {
        let mut b = Battle::first_battle(seed, permadeath);
        b.begin_phase(Phase::Player);
        let vis_pos = b.units.iter().map(|u| pv(u.pos)).collect();
        let vis_hp = b.units.iter().map(|u| u.hp).collect();
        let vis_alpha = b.units.iter().map(|_| 1.0).collect();
        let mut s = BattleScreen {
            b,
            mode: Mode::Idle,
            fx: VecDeque::new(),
            fx_t: 0.0,
            fx_started: false,
            vis_pos,
            vis_hp,
            vis_alpha,
            floats: Vec::new(),
            combat_pair: None,
            inspect: None,
            inspect_range: None,
            show_danger: false,
            danger: None,
            time: 0.0,
        };
        s.fx.push_back(Fx::Banner { text: BATTLE_NAME.to_string(), color: ACCENT });
        s.fx.push_back(Fx::Banner { text: "Player Phase".into(), color: ALLY });
        s
    }

    pub fn frame(&mut self) -> BattleExit {
        let dt = get_frame_time().min(0.05);
        self.time += dt;
        let lay = Layout::compute(self.b.map.width, self.b.map.height);
        let mut input = Input::poll();

        self.update_fx(dt, &mut input);
        for f in &mut self.floats {
            f.t += dt;
        }
        self.floats.retain(|f| f.t < 1.0);
        if self.fx.is_empty() {
            self.advance_flow();
        }
        if self.show_danger && self.danger.is_none() && self.fx.is_empty() {
            self.danger = Some(self.compute_danger());
        }

        self.draw_map(&lay);
        let mut exit = self.panel(&lay, &mut input);
        if self.fx.is_empty() {
            self.handle_map_tap(&lay, &mut input);
        }
        self.draw_overlays(&lay);
        if let Mode::Over { victory } = self.mode {
            if self.fx.is_empty() {
                exit = self.over_screen(&lay, &mut input, victory);
            }
        }
        exit
    }

    // ------------------------------------------------------------------ flow

    fn update_fx(&mut self, dt: f32, input: &mut Input) {
        if self.fx.is_empty() {
            return;
        }
        if !self.fx_started {
            self.fx_started = true;
            self.fx_t = 0.0;
            self.fx_start();
        }
        self.fx_t += dt;
        let done = match self.fx.front().and_then(|f| f.duration()) {
            Some(d) => self.fx_t >= d,
            None => self.fx_t > 0.4 && input.take().is_some(),
        };
        if done {
            let fx = self.fx.pop_front().unwrap();
            self.fx_end(fx);
            self.fx_started = false;
        }
    }

    fn fx_start(&mut self) {
        let Some(fx) = self.fx.front() else { return };
        match *fx {
            Fx::Strike { target, hit, crit, damage, hp_after, .. } => {
                let (text, color) = if !hit {
                    ("MISS".to_string(), DIM)
                } else if crit {
                    (format!("CRIT {}", damage), ACCENT)
                } else if damage == 0 {
                    ("No damage".to_string(), DIM)
                } else {
                    (format!("-{}", damage), INK)
                };
                self.floats.push(Float { text, pos: self.vis_pos[target], t: 0.0, color });
                self.vis_hp[target] = hp_after;
            }
            Fx::Heal { target, amount, hp_after } => {
                self.floats.push(Float { text: format!("+{}", amount), pos: self.vis_pos[target], t: 0.0, color: GOOD });
                self.vis_hp[target] = hp_after;
            }
            Fx::CombatBegin { a, d } => self.combat_pair = Some((a, d)),
            _ => {}
        }
    }

    fn fx_end(&mut self, fx: Fx) {
        match fx {
            Fx::Move { unit, path } => {
                if let Some(p) = path.last() {
                    self.vis_pos[unit] = pv(*p);
                }
            }
            Fx::Die { unit } => self.vis_alpha[unit] = 0.0,
            Fx::CombatEnd => self.combat_pair = None,
            _ => {}
        }
    }

    fn advance_flow(&mut self) {
        let mode = std::mem::replace(&mut self.mode, Mode::Idle);
        if let Mode::Over { .. } = mode {
            self.mode = mode;
            return;
        }
        self.mode = match self.b.outcome() {
            Outcome::Victory => {
                self.fx.push_back(Fx::Banner { text: "Victory!".into(), color: ACCENT });
                Mode::Over { victory: true }
            }
            Outcome::Defeat => {
                self.fx.push_back(Fx::Banner { text: "Defeat...".into(), color: BAD });
                Mode::Over { victory: false }
            }
            Outcome::Ongoing => match mode {
                Mode::Idle if self.b.phase == Phase::Player && !self.b.player_has_actions() => self.start_enemy_phase(),
                Mode::EnemyPhase { queue } => self.enemy_step(queue),
                other => other,
            },
        };
    }

    fn start_enemy_phase(&mut self) -> Mode {
        let heals = self.b.begin_phase(Phase::Enemy);
        self.inspect = None;
        self.inspect_range = None;
        self.fx.push_back(Fx::Banner { text: "Enemy Phase".into(), color: BAD });
        for h in heals {
            self.fx.push_back(Fx::Heal { target: h.unit, amount: h.amount, hp_after: h.hp_after });
        }
        Mode::EnemyPhase { queue: self.b.living(Team::Enemy).collect() }
    }

    fn enemy_step(&mut self, mut queue: VecDeque<usize>) -> Mode {
        while let Some(id) = queue.pop_front() {
            if !self.b.units[id].alive {
                continue;
            }
            match ai::decide(&self.b, id) {
                AiAction::Attack { dest, target } => {
                    self.move_with_fx(id, dest);
                    let r = self.b.attack(id, target);
                    self.push_combat(&r);
                }
                AiAction::Move { dest } => self.move_with_fx(id, dest),
                AiAction::Wait => {
                    self.b.finish_action(id);
                    continue;
                }
            }
            self.b.finish_action(id);
            self.fx.push_back(Fx::Pause(0.15));
            self.danger = None;
            return Mode::EnemyPhase { queue };
        }
        let heals = self.b.begin_phase(Phase::Player);
        self.fx.push_back(Fx::Banner { text: format!("Player Phase - Turn {}", self.b.turn), color: ALLY });
        for h in heals {
            self.fx.push_back(Fx::Heal { target: h.unit, amount: h.amount, hp_after: h.hp_after });
        }
        self.danger = None;
        Mode::Idle
    }

    fn move_with_fx(&mut self, id: usize, dest: Pos) {
        let start = self.b.units[id].pos;
        let path = self.b.reach(id).path_to(dest).unwrap_or_else(|| vec![start, dest]);
        self.b.move_unit(id, dest);
        self.fx.push_back(Fx::Move { unit: id, path });
    }

    fn push_xp(&mut self, x: &XpEvent) {
        self.fx.push_back(Fx::Xp { unit: x.unit, from: x.before, gained: x.gained });
        if let Some(lv) = &x.level_up {
            self.fx.push_back(Fx::LevelUp { unit: x.unit, lv: lv.clone() });
        }
    }

    fn push_combat(&mut self, r: &CombatReport) {
        self.fx.push_back(Fx::CombatBegin { a: r.attacker, d: r.defender });
        for s in &r.outcome.strikes {
            let (by, target) = match s.by {
                Side::Attacker => (r.attacker, r.defender),
                Side::Defender => (r.defender, r.attacker),
            };
            self.fx.push_back(Fx::Strike { by, target, hit: s.hit, crit: s.crit, damage: s.damage, hp_after: s.target_hp });
        }
        self.fx.push_back(Fx::CombatEnd);
        for &d in &r.deaths {
            self.fx.push_back(Fx::Die { unit: d });
        }
        if let Some(x) = &r.xp {
            self.push_xp(x);
        }
        self.danger = None;
    }

    fn compute_danger(&self) -> HashSet<Pos> {
        let mut set = HashSet::new();
        for id in self.b.living(Team::Enemy) {
            let reach = self.b.reach(id);
            set.extend(self.b.threat_tiles(id, &reach));
        }
        set
    }

    // ----------------------------------------------------------------- input

    fn select(&mut self, id: usize) -> Mode {
        let reach = self.b.reach(id);
        let dests = reach.stoppable.iter().copied().collect();
        let threat = self.b.threat_tiles(id, &reach);
        self.inspect = Some(self.b.units[id].pos);
        self.inspect_range = None;
        Mode::Selected { unit: id, reach, dests, threat }
    }

    fn tap_idle(&mut self, p: Pos) -> Mode {
        self.inspect = Some(p);
        self.inspect_range = None;
        if let Some(id) = self.b.unit_at(p) {
            let u = &self.b.units[id];
            if u.team == Team::Player && !u.done {
                return self.select(id);
            }
            if u.team == Team::Enemy {
                let reach = self.b.reach(id);
                let threat = self.b.threat_tiles(id, &reach);
                self.inspect_range = Some((reach.stoppable.into_iter().collect(), threat));
            }
        }
        Mode::Idle
    }

    fn handle_map_tap(&mut self, lay: &Layout, input: &mut Input) {
        if self.b.phase != Phase::Player {
            return;
        }
        let Some(tap) = input.take() else { return };
        let Some((x, y)) = lay.tile_at(tap) else { return };
        let p = Pos::new(x, y);
        let mode = std::mem::replace(&mut self.mode, Mode::Idle);
        self.mode = match mode {
            Mode::Idle => self.tap_idle(p),
            Mode::Selected { unit, reach, dests, threat } => {
                if dests.contains(&p) {
                    let from = self.b.units[unit].pos;
                    let path = reach.path_to(p).unwrap_or_else(|| vec![from, p]);
                    self.b.move_unit(unit, p);
                    self.fx.push_back(Fx::Move { unit, path });
                    self.inspect = Some(p);
                    Mode::Menu { unit, from }
                } else if self.b.unit_at(p).is_some() || !threat.contains(&p) {
                    self.tap_idle(p)
                } else {
                    Mode::Selected { unit, reach, dests, threat }
                }
            }
            Mode::Target { unit, from, targets, heal, chosen } => match self.b.unit_at(p).filter(|t| targets.contains(t)) {
                Some(t) if chosen == Some(t) => self.execute(unit, t, heal),
                Some(t) => Mode::Target { unit, from, targets, heal, chosen: Some(t) },
                None => Mode::Target { unit, from, targets, heal, chosen },
            },
            other => other,
        };
    }

    fn execute(&mut self, unit: usize, target: usize, heal: bool) -> Mode {
        if heal {
            let r = self.b.heal(unit, target);
            self.fx.push_back(Fx::Heal { target: r.target, amount: r.amount, hp_after: r.hp_after });
            if let Some(x) = &r.xp {
                self.push_xp(x);
            }
        } else {
            let r = self.b.attack(unit, target);
            self.push_combat(&r);
        }
        self.b.finish_action(unit);
        self.inspect = None;
        self.danger = None;
        Mode::Idle
    }

    // ----------------------------------------------------------------- panel

    fn panel(&mut self, lay: &Layout, input: &mut Input) -> BattleExit {
        let r = lay.panel;
        let u = lay.u;
        frame(r);
        let pad = (u * 0.7).floor();
        let inner = Rect::new(r.x + pad, r.y + pad, r.w - pad * 2.0, r.h - pad * 2.0);
        let mut y = inner.y + u;

        let (phase_text, phase_color) = match self.b.phase {
            Phase::Player => ("Player Phase", ALLY),
            Phase::Enemy => ("Enemy Phase", BAD),
        };
        text(&format!("Turn {}", self.b.turn), inner.x, y, u, INK);
        text_right(phase_text, inner.x + inner.w, y, u, phase_color);
        y += u * 1.1;
        let boss = self.b.units.iter().find(|u| u.is_boss).map(|u| u.name.clone()).unwrap_or_default();
        let lord = self.b.units.iter().find(|u| u.is_lord).map(|u| u.name.clone()).unwrap_or_default();
        text(&format!("Rout: defeat {}", boss), inner.x, y, u * 0.8, ACCENT);
        y += u * 0.9;
        text(&format!("{} must survive", lord), inner.x, y, u * 0.8, DIM);
        y += u * 0.6;
        draw_line(inner.x, y, inner.x + inner.w, y, 2.0, rgb(80, 70, 110));
        y += u * 1.2;

        let mut col = ButtonColumn::new(inner, u);
        let mut exit = BattleExit::Stay;
        let idle_fx = self.fx.is_empty();

        // Buttons first (bottom-up) so the content knows how much room it has.
        let mode = std::mem::replace(&mut self.mode, Mode::Idle);
        self.mode = if !idle_fx || self.b.phase != Phase::Player {
            mode
        } else {
            match mode {
                Mode::Idle => {
                    let label = if self.show_danger { "Enemy Range: On" } else { "Enemy Range: Off" };
                    if button(input, col.next(), label, ButtonStyle::Normal, u) {
                        self.show_danger = !self.show_danger;
                        self.danger = None;
                    }
                    if button(input, col.next(), "End Turn", ButtonStyle::Danger, u) {
                        for i in self.b.living(Team::Player).collect::<Vec<_>>() {
                            self.b.finish_action(i);
                        }
                        self.inspect = None;
                    }
                    Mode::Idle
                }
                Mode::Selected { unit, reach, dests, threat } => {
                    if button(input, col.next(), "Cancel", ButtonStyle::Normal, u) {
                        self.inspect = None;
                        Mode::Idle
                    } else {
                        Mode::Selected { unit, reach, dests, threat }
                    }
                }
                Mode::Menu { unit, from } => {
                    let pos = self.b.units[unit].pos;
                    let targets = self.b.targets_from(unit, pos);
                    let heals = self.b.heal_targets_from(unit, pos);
                    let mut next = None;
                    if button(input, col.next(), "Cancel", ButtonStyle::Normal, u) {
                        self.b.move_unit(unit, from);
                        self.vis_pos[unit] = pv(from);
                        next = Some(self.select(unit));
                    }
                    if button(input, col.next(), "Wait", ButtonStyle::Normal, u) {
                        self.b.finish_action(unit);
                        self.inspect = None;
                        self.danger = None;
                        next = Some(Mode::Idle);
                    }
                    if !heals.is_empty() && button(input, col.next(), "Heal", ButtonStyle::Primary, u) {
                        next = Some(Mode::Target { unit, from, chosen: heals.first().copied(), targets: heals, heal: true });
                    }
                    if !targets.is_empty() && button(input, col.next(), "Attack", ButtonStyle::Primary, u) {
                        next = Some(Mode::Target { unit, from, chosen: targets.first().copied(), targets, heal: false });
                    }
                    next.unwrap_or(Mode::Menu { unit, from })
                }
                Mode::Target { unit, from, targets, heal, chosen } => {
                    if button(input, col.next(), "Back", ButtonStyle::Normal, u) {
                        Mode::Menu { unit, from }
                    } else if let Some(t) = chosen {
                        let label = if heal { "Heal!" } else { "Attack!" };
                        if button(input, col.next(), label, ButtonStyle::Primary, u) {
                            self.execute(unit, t, heal)
                        } else {
                            Mode::Target { unit, from, targets, heal, chosen }
                        }
                    } else {
                        Mode::Target { unit, from, targets, heal, chosen }
                    }
                }
                other => other,
            }
        };
        if let Mode::Over { .. } = self.mode {
            // handled by over_screen
        }
        let bottom = col.top() - u * 0.5;

        // Content
        match &self.mode {
            Mode::Target { unit, chosen: Some(t), heal, .. } => {
                if *heal {
                    self.heal_preview(inner.x, y, inner.w, u, *unit, *t);
                } else {
                    let f = self.b.forecast(*unit, *t);
                    self.forecast_box(inner.x, y, inner.w, u, *unit, *t, &f);
                }
                text("Tap the target again or press the button", inner.x, bottom, u * 0.6, DIM);
            }
            Mode::Target { .. } => {}
            Mode::EnemyPhase { .. } => {
                text("The enemy is moving...", inner.x, y, u * 0.9, DIM);
            }
            _ => {
                let unit = match &self.mode {
                    Mode::Selected { unit, .. } | Mode::Menu { unit, .. } => Some(*unit),
                    _ => self.inspect.and_then(|p| self.b.unit_at(p)),
                };
                let tile = match unit {
                    Some(id) => Some(self.b.units[id].pos),
                    None => self.inspect,
                };
                if let Some(id) = unit {
                    y = self.unit_info(inner.x, y, inner.w, u, id);
                    y += u * 0.4;
                }
                if let Some(p) = tile {
                    if y < bottom {
                        self.terrain_info(inner.x, y, u, p);
                    }
                } else if y + u * 3.0 < bottom {
                    let hints = match self.mode {
                        Mode::Idle => ["Tap a blue unit to move it.", "Tap an enemy to see its range.", "Grey units have already acted."],
                        _ => ["", "", ""],
                    };
                    for h in hints {
                        text(h, inner.x, y, u * 0.75, DIM);
                        y += u * 1.0;
                    }
                }
                if let Mode::Selected { .. } = self.mode {
                    text("Tap a blue tile to move.", inner.x, bottom, u * 0.7, DIM);
                }
                if let Mode::Menu { .. } = self.mode {
                    text("Choose an action.", inner.x, bottom, u * 0.7, DIM);
                }
            }
        }
        if !idle_fx {
            exit = BattleExit::Stay;
        }
        exit
    }

    fn unit_info(&self, x: f32, mut y: f32, w: f32, u: f32, id: usize) -> f32 {
        let un = &self.b.units[id];
        let hp = self.vis_hp[id];
        text(&un.name, x, y, u * 1.15, team_ink(un.team));
        text_right(&format!("Lv {}", un.level), x + w, y, u, INK);
        y += u * 1.05;
        let role = if un.is_lord {
            format!("{} (Lord)", un.class.name())
        } else if un.is_boss {
            format!("{} (Boss)", un.class.name())
        } else {
            un.class.name().to_string()
        };
        text(&role, x, y, u * 0.85, DIM);
        if un.team == Team::Player {
            text_right(&format!("EXP {}", un.xp), x + w, y, u * 0.85, DIM);
        }
        y += u * 0.6;
        hp_bar(x, y, w * 0.6, u * 0.55, hp, un.stats.hp);
        text_right(&format!("HP {}/{}", hp, un.stats.hp), x + w, y + u * 0.55, u * 0.85, INK);
        y += u * 1.75;
        let s = un.stats;
        let vals = [
            (un.pow_label(), s.pow),
            ("Skl", s.skl),
            ("Spd", s.spd),
            ("Lck", s.lck),
            ("Def", s.def),
            ("Res", s.res),
        ];
        let cw = w / 3.0;
        for (i, (name, v)) in vals.iter().enumerate() {
            let cx = x + (i % 3) as f32 * cw;
            let cy = y + (i / 3) as f32 * u * 1.05;
            text(name, cx, cy, u * 0.85, DIM);
            text_right(&v.to_string(), cx + cw - u * 0.6, cy, u * 0.85, INK);
        }
        y += u * 2.2;
        let wpn = &un.weapon;
        text(&format!("{} ({})", wpn.name, wpn.kind.name()), x, y, u * 0.85, ACCENT);
        y += u * 0.95;
        let detail = if wpn.can_attack() {
            format!("Mt {}  Hit {}  Crt {}  Rng {}  Mov {}", wpn.might, wpn.hit, wpn.crit, wpn.range_label(), un.mov())
        } else {
            format!("Heals {} HP  Rng {}  Mov {}", combat::heal_amount(un), wpn.range_label(), un.mov())
        };
        text(&detail, x, y, u * 0.72, INK);
        y + u * 0.6
    }

    fn terrain_info(&self, x: f32, y: f32, u: f32, p: Pos) {
        let t = self.b.map.at(p);
        let mut s = format!("{}  Def +{}  Avo +{}", t.name(), t.defense(), t.avoid());
        if t.heal_percent() > 0 {
            s.push_str(&format!("  Heal {}%", t.heal_percent()));
        }
        if t.move_cost(crate::logic::terrain::MoveType::Infantry).is_none() {
            s = format!("{}  (impassable)", t.name());
        }
        text(&s, x, y + u * 0.3, u * 0.75, rgb(170, 200, 150));
    }

    #[allow(clippy::too_many_arguments)]
    fn forecast_box(&self, x: f32, mut y: f32, w: f32, u: f32, att: usize, def: usize, f: &Forecast) {
        let a = &self.b.units[att];
        let d = &self.b.units[def];
        let lx = x + w * 0.22;
        let rx = x + w * 0.78;
        text_centered("Combat Forecast", x + w / 2.0, y, u * 0.9, ACCENT);
        y += u * 1.4;
        text_centered(&a.name, lx, y, u, team_ink(a.team));
        text_centered(&d.name, rx, y, u, team_ink(d.team));
        y += u * 0.8;
        let tri = |s: &SideForecast| match s.triangle {
            1 => ("Advantage", GOOD),
            -1 => ("Disadvantage", BAD),
            _ => ("", DIM),
        };
        let (ta, ca) = tri(&f.attacker);
        let (td, cd) = tri(&f.defender);
        text_centered(&format!("{} {}", a.weapon.name, ta), lx, y, u * 0.6, ca);
        text_centered(&format!("{} {}", d.weapon.name, td), rx, y, u * 0.6, cd);
        y += u * 1.3;

        let after = |me: &SideForecast, foe: &SideForecast| (me.hp - foe.damage * foe.strikes).max(0);
        let rows: [(&str, String, String); 4] = [
            ("HP", format!("{} > {}", f.attacker.hp, after(&f.attacker, &f.defender)), format!("{} > {}", f.defender.hp, after(&f.defender, &f.attacker))),
            ("Dmg", dmg_str(&f.attacker), dmg_str(&f.defender)),
            ("Hit", pct_str(&f.attacker, f.attacker.hit), pct_str(&f.defender, f.defender.hit)),
            ("Crit", pct_str(&f.attacker, f.attacker.crit), pct_str(&f.defender, f.defender.crit)),
        ];
        for (label, l, r) in rows.iter() {
            draw_rectangle(x, y - u * 0.85, w, u * 1.15, Color::new(1.0, 1.0, 1.0, 0.04));
            text_centered(l, lx, y, u, INK);
            text_centered(label, x + w / 2.0, y, u * 0.8, ACCENT);
            text_centered(r, rx, y, u, INK);
            y += u * 1.3;
        }
        if f.defender.strikes == 0 {
            text_centered(&format!("{} cannot counter", d.name), x + w / 2.0, y, u * 0.7, GOOD);
        }
    }

    fn heal_preview(&self, x: f32, mut y: f32, w: f32, u: f32, healer: usize, target: usize) {
        let h = &self.b.units[healer];
        let t = &self.b.units[target];
        let amount = combat::heal_amount(h).min(t.stats.hp - t.hp);
        text_centered("Heal", x + w / 2.0, y, u * 0.9, ACCENT);
        y += u * 1.5;
        text_centered(&t.name, x + w / 2.0, y, u * 1.1, ALLY);
        y += u * 1.4;
        text_centered(&format!("HP {} > {} / {}", t.hp, t.hp + amount, t.stats.hp), x + w / 2.0, y, u, GOOD);
        y += u * 1.2;
        text_centered(&format!("+{} HP, +{} EXP for {}", amount, combat::HEAL_XP, h.name), x + w / 2.0, y, u * 0.7, DIM);
    }

    // ------------------------------------------------------------------- map

    fn draw_pos(&self, i: usize) -> Vec2 {
        let mut v = self.vis_pos[i];
        if !self.fx_started {
            return v;
        }
        match self.fx.front() {
            Some(Fx::Move { unit, path }) if *unit == i && path.len() > 1 => {
                let f = (self.fx_t / STEP_TIME).min((path.len() - 1) as f32);
                let k = (f.floor() as usize).min(path.len() - 1);
                let frac = f - k as f32;
                let a = pv(path[k]);
                let b = pv(path[(k + 1).min(path.len() - 1)]);
                v = a.lerp(b, frac);
            }
            Some(Fx::Strike { by, target, hit, .. }) => {
                if *by == i {
                    let dir = (self.vis_pos[*target] - self.vis_pos[i]).normalize_or_zero();
                    let k = (self.fx_t / 0.25).min(1.0);
                    v += dir * 0.3 * (k * PI).sin();
                }
                if *target == i && *hit && self.fx_t > 0.12 && self.fx_t < 0.4 {
                    v.x += (self.fx_t * 90.0).sin() * 0.06;
                }
            }
            _ => {}
        }
        v
    }

    fn draw_alpha(&self, i: usize) -> f32 {
        if self.fx_started {
            if let Some(Fx::Die { unit }) = self.fx.front() {
                if *unit == i {
                    return (1.0 - self.fx_t / 0.7).max(0.0) * if (self.fx_t * 20.0) as i32 % 2 == 0 { 1.0 } else { 0.5 };
                }
            }
        }
        self.vis_alpha[i]
    }

    fn shade(&self, lay: &Layout, p: Pos, c: Color) {
        let r = lay.tile_rect(p.x as f32, p.y as f32);
        draw_rectangle(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, c);
    }

    fn cursor(&self, lay: &Layout, p: Pos, c: Color) {
        let r = lay.tile_rect(p.x as f32, p.y as f32);
        let pulse = ((self.time * 5.0).sin() * 0.5 + 0.5) * lay.tile * 0.05;
        let t = (lay.tile * 0.08).max(2.0);
        let (x, y, w, h) = (r.x - pulse, r.y - pulse, r.w + pulse * 2.0, r.h + pulse * 2.0);
        let l = w * 0.3;
        for (cx, cy, sx, sy) in [(x, y, 1.0, 1.0), (x + w, y, -1.0, 1.0), (x, y + h, 1.0, -1.0), (x + w, y + h, -1.0, -1.0)] {
            draw_rectangle(cx.min(cx + l * sx), cy.min(cy + t * sy), l, t, c);
            draw_rectangle(cx.min(cx + t * sx), cy.min(cy + l * sy), t, l, c);
        }
    }

    fn draw_map(&self, lay: &Layout) {
        let m = &self.b.map;
        for y in 0..m.height {
            for x in 0..m.width {
                let r = lay.tile_rect(x as f32, y as f32);
                gfx::draw_terrain(m.at(Pos::new(x, y)), r.x, r.y, r.w, x, y, self.time);
            }
        }
        // Subtle grid
        let gc = Color::new(0.0, 0.0, 0.0, 0.12);
        for x in 0..=m.width {
            let px = lay.ox + x as f32 * lay.tile;
            draw_line(px, lay.oy, px, lay.oy + lay.map_h, 1.0, gc);
        }
        for y in 0..=m.height {
            let py = lay.oy + y as f32 * lay.tile;
            draw_line(lay.ox, py, lay.ox + lay.map_w, py, 1.0, gc);
        }

        if let Some(d) = &self.danger {
            if self.show_danger {
                for p in d {
                    self.shade(lay, *p, Color::new(0.6, 0.1, 0.5, 0.28));
                }
            }
        }
        if let Some((moves, threat)) = &self.inspect_range {
            for p in threat.difference(moves) {
                self.shade(lay, *p, Color::new(1.0, 0.55, 0.1, 0.32));
            }
            for p in moves {
                self.shade(lay, *p, Color::new(0.95, 0.25, 0.25, 0.38));
            }
        }
        let pulse = (self.time * 4.0).sin() * 0.08;
        match &self.mode {
            Mode::Selected { unit, dests, threat, .. } => {
                let heal = !self.b.units[*unit].weapon.can_attack();
                let edge = if heal { Color::new(0.2, 0.9, 0.4, 0.35) } else { Color::new(1.0, 0.2, 0.2, 0.38) };
                for p in threat.difference(dests) {
                    self.shade(lay, *p, edge);
                }
                for p in dests {
                    self.shade(lay, *p, Color::new(0.25, 0.5, 1.0, 0.45 + pulse));
                }
            }
            Mode::Target { targets, chosen, heal, .. } => {
                let c = if *heal { Color::new(0.2, 0.95, 0.4, 0.4 + pulse) } else { Color::new(1.0, 0.2, 0.2, 0.42 + pulse) };
                for t in targets {
                    self.shade(lay, self.b.units[*t].pos, c);
                }
                if let Some(t) = chosen {
                    self.cursor(lay, self.b.units[*t].pos, ACCENT);
                }
            }
            _ => {}
        }

        // Units, back to front.
        let mut order: Vec<usize> = (0..self.b.units.len()).collect();
        order.sort_by(|a, b| self.draw_pos(*a).y.partial_cmp(&self.draw_pos(*b).y).unwrap_or(std::cmp::Ordering::Equal));
        for i in order {
            let alpha = self.draw_alpha(i);
            if alpha <= 0.0 {
                continue;
            }
            let un = &self.b.units[i];
            let v = self.draw_pos(i);
            let r = lay.tile_rect(v.x, v.y);
            let moving = matches!(self.mode, Mode::Selected { unit, .. } if unit == i);
            let bob = !un.done && (((self.time * if moving { 6.0 } else { 2.0 }) + i as f32 * 0.37) as i32 % 2 == 0);
            let inset = r.w * 0.06;
            gfx::draw_unit(look(un), r.x + inset, r.y + inset * 1.5, r.w - inset * 2.0, un.done, alpha, bob);
            if un.is_boss || un.is_lord {
                let c = if un.is_boss { ACCENT } else { rgb(120, 200, 255) };
                draw_rectangle(r.x + r.w * 0.06, r.y + r.h * 0.06, r.w * 0.12, r.w * 0.12, Color::new(c.r, c.g, c.b, alpha));
            }
            let bw = r.w * 0.8;
            hp_bar(r.x + (r.w - bw) / 2.0, r.y + r.h - r.h * 0.1, bw, (r.h * 0.09).max(3.0), self.vis_hp[i], un.stats.hp);
        }

        // Cursor on the inspected / selected tile
        match &self.mode {
            Mode::Selected { unit, .. } | Mode::Menu { unit, .. } => self.cursor(lay, self.b.units[*unit].pos, INK),
            Mode::Idle => {
                if let Some(p) = self.inspect {
                    self.cursor(lay, p, INK);
                }
            }
            _ => {}
        }

        // Floating combat text
        for f in &self.floats {
            let r = lay.tile_rect(f.pos.x, f.pos.y - f.t * 0.6);
            let size = lay.tile * 0.42;
            let mut c = f.color;
            c.a = (1.0 - f.t).clamp(0.0, 1.0) * 2.0_f32.min(1.0);
            text_centered(&f.text, r.x + r.w / 2.0, r.y + r.h * 0.2, size, c);
        }
    }

    // -------------------------------------------------------------- overlays

    fn draw_overlays(&self, lay: &Layout) {
        let u = lay.u;
        let map = Rect::new(lay.ox, lay.oy, lay.map_w, lay.map_h);

        if let Some((a, d)) = self.combat_pair {
            let w = map.w * 0.8;
            let h = u * 3.4;
            let r = Rect::new(map.x + (map.w - w) / 2.0, map.y + u * 0.5, w, h);
            frame(r);
            for (k, id) in [a, d].into_iter().enumerate() {
                let un = &self.b.units[id];
                let x = r.x + u + k as f32 * (w / 2.0);
                let bw = w / 2.0 - u * 2.0;
                text(&un.name, x, r.y + u * 1.4, u, team_ink(un.team));
                text_right(&format!("{}", self.vis_hp[id]), x + bw, r.y + u * 1.4, u, INK);
                hp_bar(x, r.y + u * 2.0, bw, u * 0.6, self.vis_hp[id], un.stats.hp);
            }
            text_centered("vs", r.x + w / 2.0, r.y + u * 1.4, u * 0.8, DIM);
        }

        if !self.fx_started {
            return;
        }
        match self.fx.front() {
            Some(Fx::Banner { text: t, color }) => {
                let k = self.fx_t / 1.3;
                let a = (k * 6.0).min(1.0).min((1.0 - k) * 6.0).clamp(0.0, 1.0);
                let h = u * 3.2;
                let y = map.y + map.h / 2.0 - h / 2.0;
                let slide = (1.0 - (k * 5.0).min(1.0)) * map.w * 0.2;
                draw_rectangle(map.x, y, map.w, h, Color::new(0.05, 0.05, 0.1, 0.75 * a));
                draw_rectangle(map.x, y, map.w, 3.0, Color::new(color.r, color.g, color.b, a));
                draw_rectangle(map.x, y + h - 3.0, map.w, 3.0, Color::new(color.r, color.g, color.b, a));
                let mut c = *color;
                c.a = a;
                text_centered(t, map.x + map.w / 2.0 + slide, y + h / 2.0 + u * 0.6, u * 1.9, c);
            }
            Some(Fx::Xp { unit, from, gained }) => {
                let un = &self.b.units[*unit];
                let w = map.w * 0.5;
                let h = u * 3.0;
                let r = Rect::new(map.x + (map.w - w) / 2.0, map.y + map.h - h - u, w, h);
                frame(r);
                let k = (self.fx_t / 0.8).min(1.0);
                let shown = (*from as f32 + *gained as f32 * k).min(100.0);
                text(&format!("{}  +{} EXP", un.name, gained), r.x + u, r.y + u * 1.3, u * 0.9, INK);
                let bx = r.x + u;
                let bw = w - u * 2.0;
                draw_rectangle(bx, r.y + u * 1.8, bw, u * 0.6, rgb(20, 20, 24));
                draw_rectangle(bx + 1.0, r.y + u * 1.8 + 1.0, (bw - 2.0) * shown / 100.0, u * 0.6 - 2.0, ACCENT);
            }
            Some(Fx::LevelUp { unit, lv }) => {
                let un = &self.b.units[*unit];
                let w = (map.w * 0.6).max(u * 14.0).min(map.w);
                let h = u * 9.5;
                let r = Rect::new(map.x + (map.w - w) / 2.0, map.y + (map.h - h) / 2.0, w, h);
                frame(r);
                text_centered("LEVEL UP!", r.x + w / 2.0, r.y + u * 1.6, u * 1.4, ACCENT);
                text_centered(&format!("{}  {}  Lv {}", un.name, un.class.name(), lv.new_level), r.x + w / 2.0, r.y + u * 2.8, u * 0.9, INK);
                let gains = lv.gains.to_array();
                let after = lv.after.to_array();
                let cw = (w - u * 2.0) / 2.0;
                for i in 0..7 {
                    let col = (i % 2) as f32;
                    let row = (i / 2) as f32;
                    let x = r.x + u + col * cw;
                    let y = r.y + u * 4.4 + row * u * 1.15;
                    let name = if i == 1 { un.pow_label() } else { STAT_NAMES[i] };
                    text(name, x, y, u * 0.9, DIM);
                    text_right(&after[i].to_string(), x + cw * 0.6, y, u * 0.9, INK);
                    if gains[i] > 0 {
                        text(&format!("+{}", gains[i]), x + cw * 0.66, y, u * 0.9, GOOD);
                    }
                }
                if self.fx_t > 0.4 {
                    text_centered("Tap to continue", r.x + w / 2.0, r.y + h - u * 0.6, u * 0.7, DIM);
                }
            }
            _ => {}
        }
    }

    fn over_screen(&mut self, lay: &Layout, input: &mut Input, victory: bool) -> BattleExit {
        let u = lay.u;
        let map = Rect::new(lay.ox, lay.oy, lay.map_w, lay.map_h);
        draw_rectangle(map.x, map.y, map.w, map.h, Color::new(0.0, 0.0, 0.0, 0.45));
        let w = (map.w * 0.7).max(u * 16.0).min(screen_width() - u);
        let h = u * 15.0;
        let r = Rect::new(map.x + (map.w - w) / 2.0, (map.y + (map.h - h) / 2.0).max(0.0), w, h);
        frame(r);
        let (title, c) = if victory { ("VICTORY", ACCENT) } else { ("DEFEAT", BAD) };
        text_centered(title, r.x + w / 2.0, r.y + u * 2.2, u * 2.0, c);
        let sub = if victory {
            format!("{} is won in {} turns.", BATTLE_NAME, self.b.turn)
        } else {
            let lord = self.b.units.iter().find(|u| u.is_lord).map(|u| u.name.as_str()).unwrap_or("The lord");
            format!("{} has fallen. The crossing is lost.", lord)
        };
        text_centered(&sub, r.x + w / 2.0, r.y + u * 3.5, u * 0.8, INK);
        let mut y = r.y + u * 5.0;
        for un in self.b.units.iter().filter(|u| u.team == Team::Player) {
            text(&format!("{}  Lv {} {}", un.name, un.level, un.class.name()), r.x + u, y, u * 0.85, ALLY);
            let (status, sc) = if un.alive {
                (format!("HP {}/{}", un.hp, un.stats.hp), INK)
            } else if self.b.permadeath {
                ("Fallen - lost forever".to_string(), BAD)
            } else {
                ("Retreated - will return".to_string(), DIM)
            };
            text_right(&status, r.x + w - u, y, u * 0.85, sc);
            y += u * 1.05;
        }
        let bw = (w - u * 3.0) / 2.0;
        let bh = u * 2.1;
        let by = r.y + h - bh - u * 0.8;
        let mut exit = BattleExit::Stay;
        if button(input, Rect::new(r.x + u, by, bw, bh), "Play Again", ButtonStyle::Primary, u) {
            exit = BattleExit::Retry;
        }
        if button(input, Rect::new(r.x + u * 2.0 + bw, by, bw, bh), "Title", ButtonStyle::Normal, u) {
            exit = BattleExit::Title;
        }
        exit
    }
}

fn dmg_str(s: &SideForecast) -> String {
    match s.strikes {
        0 => "--".into(),
        1 => s.damage.to_string(),
        n => format!("{} x{}", s.damage, n),
    }
}

fn pct_str(s: &SideForecast, v: i32) -> String {
    if s.strikes == 0 {
        "--".into()
    } else {
        format!("{}%", v)
    }
}
