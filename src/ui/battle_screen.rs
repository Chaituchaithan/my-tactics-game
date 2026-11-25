//! Battle presentation: turns logic results into animations and handles taps.
//!
//! Logic changes are applied to `Battle` instantly; the screen then replays
//! them through a queue of visual effects (`Fx`) using its own copy of unit
//! positions/HP/teams, so rendering never leaks into the rules.

use std::collections::{HashSet, VecDeque};
use std::f32::consts::PI;

use macroquad::prelude::*;

use super::gfx::{self, rgb, UnitLook};
use super::widgets::*;
use crate::logic::ai::{self, AiAction};
use crate::logic::battle::{Battle, CombatReport, ItemUse, Objective, Outcome, Phase, PhaseStart, Reward, XpEvent};
use crate::logic::campaign::{chapter, Line};
use crate::logic::combat::{self, Forecast, Side, SideForecast};
use crate::logic::items::ItemKind;
use crate::logic::pathfinding::Reach;
use crate::logic::terrain::{MoveType, Terrain};
use crate::logic::unit::WeaponKind;
use crate::logic::unit::{LevelUp, Promotion, Team, STAT_NAMES};
use crate::logic::Pos;

pub enum BattleExit {
    Stay,
    Victory,
    Retry,
    Title,
}

const STEP_TIME: f32 = 0.07;
const STRIKE_TIME: f32 = 0.7;

enum Fx {
    Banner { text: String, color: Color },
    Notice { text: String, color: Color },
    Pause(f32),
    Move { unit: usize, path: Vec<Pos> },
    CombatBegin { a: usize, d: usize, f: Box<Forecast> },
    Strike { by: usize, target: usize, hit: bool, crit: bool, damage: i32, hp_after: i32 },
    CombatEnd,
    Heal { target: usize, amount: i32, hp_after: i32 },
    Die { unit: usize },
    Spawn { unit: usize },
    Xp { unit: usize, from: i32, gained: i32 },
    LevelUp { unit: usize, lv: LevelUp },
    Promote { unit: usize, p: Promotion },
    Say { speaker: String, text: String },
    Recruit { unit: usize },
}

impl Fx {
    /// None = waits for a tap.
    fn duration(&self) -> Option<f32> {
        Some(match self {
            Fx::Banner { .. } => 1.3,
            Fx::Notice { .. } => 1.2,
            Fx::Pause(t) => *t,
            Fx::Move { path, .. } => STEP_TIME * path.len().saturating_sub(1) as f32,
            Fx::CombatBegin { .. } => 0.3,
            Fx::Strike { .. } => STRIKE_TIME,
            Fx::CombatEnd => 0.35,
            Fx::Heal { .. } => 0.8,
            Fx::Die { .. } => 0.7,
            Fx::Spawn { .. } => 0.5,
            Fx::Xp { .. } => 1.1,
            Fx::Recruit { .. } => 0.6,
            Fx::LevelUp { .. } | Fx::Promote { .. } | Fx::Say { .. } => return None,
        })
    }
}

struct Float {
    text: String,
    pos: Vec2,
    t: f32,
    color: Color,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TargetKind {
    Attack,
    Heal,
    Talk,
}

enum Mode {
    Idle,
    Selected { unit: usize, reach: Reach, dests: HashSet<Pos>, threat: HashSet<Pos> },
    Menu { unit: usize, from: Pos },
    Items { unit: usize, from: Pos },
    Target { unit: usize, from: Pos, targets: Vec<usize>, kind: TargetKind, chosen: Option<usize> },
    EnemyPhase { queue: VecDeque<usize> },
    Over { victory: bool },
}

pub struct BattleScreen {
    b: Battle,
    chapter: usize,
    title: String,
    talks: Vec<(&'static str, Vec<Line>)>,
    mode: Mode,
    fx: VecDeque<Fx>,
    fx_t: f32,
    fx_started: bool,
    vis_pos: Vec<Vec2>,
    vis_hp: Vec<i32>,
    vis_alpha: Vec<f32>,
    vis_team: Vec<Team>,
    floats: Vec<Float>,
    combat_pair: Option<(usize, usize)>,
    inspect: Option<Pos>,
    /// Movement and threat area of an inspected enemy.
    inspect_range: Option<(HashSet<Pos>, HashSet<Pos>)>,
    show_danger: bool,
    danger: Option<HashSet<Pos>>,
    time: f32,
    cam: Vec2,
    cam_target: Option<Vec2>,
    zoom: usize,
    /// Enemy phase fast-forward.
    fast: bool,
    /// Last layout, for camera decisions made outside drawing.
    lay: Option<Layout>,
    /// Show close-up battle scenes for fights.
    scenes: bool,
    scene: Option<Scene>,
    scene_skip: bool,
    impact_done: bool,
}

/// The fight currently shown as a close-up scene.
struct Scene {
    a: usize,
    d: usize,
    f: Forecast,
    terrain: crate::logic::terrain::Terrain,
}

const SCENE_STRIKE: f32 = 1.15;
const SCENE_IMPACT: f32 = 0.5;

fn pv(p: Pos) -> Vec2 {
    vec2(p.x as f32, p.y as f32)
}

fn team_ink(team: Team) -> Color {
    match team {
        Team::Player => ALLY,
        Team::Enemy => BAD,
    }
}

impl BattleScreen {
    pub fn new(mut b: Battle, chapter_index: usize, scenes: bool) -> BattleScreen {
        b.begin_phase(Phase::Player);
        let def = chapter(chapter_index);
        let mut s = BattleScreen {
            b,
            chapter: chapter_index,
            title: def.title.to_string(),
            talks: def.talks,
            mode: Mode::Idle,
            fx: VecDeque::new(),
            fx_t: 0.0,
            fx_started: false,
            vis_pos: Vec::new(),
            vis_hp: Vec::new(),
            vis_alpha: Vec::new(),
            vis_team: Vec::new(),
            floats: Vec::new(),
            combat_pair: None,
            inspect: None,
            inspect_range: None,
            show_danger: false,
            danger: None,
            time: 0.0,
            cam: Vec2::ZERO,
            cam_target: None,
            zoom: 1,
            fast: false,
            lay: None,
            scenes,
            scene: None,
            scene_skip: false,
            impact_done: false,
        };
        if let Some(lord) = s.b.units.iter().find(|u| u.is_lord) {
            let l = Layout::compute(s.b.map.width, s.b.map.height, s.zoom, Vec2::ZERO);
            s.cam = l.centred_on(pv(lord.pos));
        }
        s.sync_vis();
        for a in s.vis_alpha.iter_mut() {
            *a = 1.0;
        }
        s.fx.push_back(Fx::Banner { text: format!("Chapter {}: {}", chapter_index + 1, s.title), color: ACCENT });
        s.fx.push_back(Fx::Notice { text: s.b.objective_text(), color: ACCENT });
        s
    }

    pub fn battle(&self) -> &Battle {
        &self.b
    }

    pub fn chapter(&self) -> usize {
        self.chapter
    }

    /// Grows the visual arrays when units are added (reinforcements). New units start invisible.
    fn sync_vis(&mut self) {
        while self.vis_pos.len() < self.b.units.len() {
            let u = &self.b.units[self.vis_pos.len()];
            self.vis_pos.push(pv(u.pos));
            self.vis_hp.push(u.hp);
            self.vis_alpha.push(0.0);
            self.vis_team.push(u.team);
        }
    }

    pub fn frame(&mut self) -> BattleExit {
        let raw_dt = get_frame_time().min(0.05);
        self.time += raw_dt;
        self.sync_vis();
        let mut input = Input::poll();
        let mut lay = Layout::compute(self.b.map.width, self.b.map.height, self.zoom, self.cam);

        // Camera: drag to pan, otherwise glide towards the follow target.
        if let Some((start, delta)) = input.drag {
            if lay.view().contains(start) {
                self.cam = lay.cam - delta / lay.tile;
                self.cam_target = None;
            }
        } else if let Some(t) = self.cam_target {
            let d = t - self.cam;
            self.cam += d * (raw_dt * 9.0).min(1.0);
            if d.length() < 0.02 {
                self.cam = t;
                self.cam_target = None;
            }
        }
        lay = Layout::compute(self.b.map.width, self.b.map.height, self.zoom, self.cam);
        self.cam = lay.cam;
        self.lay = Some(lay);

        let dt = if self.fast && self.b.phase == Phase::Enemy { raw_dt * 3.0 } else { raw_dt };
        self.update_fx(dt, &mut input);
        self.follow_fx(&lay);
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
        if self.b.phase == Phase::Enemy && input.take().is_some() {
            self.fast = true;
        }
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

    // ---------------------------------------------------------------- camera

    fn look_at(&mut self, p: Vec2) {
        if let Some(l) = self.lay {
            if !l.shows(p, 2.0) {
                self.cam_target = Some(l.centred_on(p));
            }
        }
    }

    /// Keep the action on screen: moving units, fights, spawns.
    fn follow_fx(&mut self, lay: &Layout) {
        if !lay.scrollable() {
            return;
        }
        let target = match self.fx.front() {
            Some(Fx::Move { unit, path }) if self.fx_started || self.fx_t == 0.0 => {
                let _ = unit;
                path.last().map(|p| pv(*p))
            }
            Some(Fx::CombatBegin { d, .. }) => Some(self.vis_pos[*d]),
            Some(Fx::Spawn { unit }) => Some(pv(self.b.units[*unit].pos)),
            _ => None,
        };
        if let Some(p) = target {
            if self.cam_target.is_none() && !lay.shows(p, 1.5) {
                self.cam_target = Some(lay.centred_on(p));
            }
        }
    }

    /// Zoom controls in the side panel, right-aligned at (`right`, `y`).
    fn zoom_buttons(&mut self, lay: &Layout, input: &mut Input, right: f32, y: f32) {
        let u = lay.u;
        let s = (u * 1.6).floor();
        let plus = Rect::new(right - s, y, s * 1.0, s);
        let minus = Rect::new(right - s * 2.0 - u * 0.3, y, s, s);
        let names = ["Whole map", "Normal", "Close"];
        text_right(names[self.zoom.min(2)], minus.x - u * 0.4, y + s * 0.68, u * 0.7, DIM);
        let zs = if self.zoom > 0 { ButtonStyle::Normal } else { ButtonStyle::Disabled };
        let zp = if self.zoom + 1 < ZOOM_LEVELS { ButtonStyle::Normal } else { ButtonStyle::Disabled };
        let centre = self.cam + lay.view_tiles() / 2.0;
        let mut changed = false;
        if button(input, minus, "-", zs, u) {
            self.zoom -= 1;
            changed = true;
        }
        if button(input, plus, "+", zp, u) {
            self.zoom += 1;
            changed = true;
        }
        if changed {
            let l = Layout::compute(self.b.map.width, self.b.map.height, self.zoom, Vec2::ZERO);
            self.cam = centre - l.view_tiles() / 2.0;
            self.cam_target = None;
        }
    }

    // ------------------------------------------------------------------ flow

    /// Duration of the front effect; battle scenes play combat more slowly.
    fn fx_duration(&self) -> Option<f32> {
        let fx = self.fx.front()?;
        if self.scene.is_some() {
            let k = if self.scene_skip { 0.15 } else { 1.0 };
            match fx {
                Fx::CombatBegin { .. } => return Some(0.6 * k),
                Fx::Strike { .. } => return Some(SCENE_STRIKE * k),
                Fx::CombatEnd => return Some(0.7 * k),
                _ => {}
            }
        }
        fx.duration()
    }

    fn update_fx(&mut self, dt: f32, input: &mut Input) {
        if self.fx.is_empty() {
            return;
        }
        if !self.fx_started {
            self.fx_started = true;
            self.fx_t = 0.0;
            self.impact_done = false;
            self.fx_start();
        }
        if self.scene.is_some() && input.take().is_some() {
            self.scene_skip = true;
        }
        self.fx_t += dt;
        let impact = if self.scene_skip { SCENE_IMPACT * 0.15 } else { SCENE_IMPACT };
        if let Some(Fx::Strike { target, hp_after, .. }) = self.fx.front() {
            if self.scene.is_some() && !self.impact_done && self.fx_t >= impact {
                self.vis_hp[*target] = *hp_after;
                self.impact_done = true;
            }
        }
        let done = match self.fx_duration() {
            Some(d) => self.fx_t >= d,
            None => self.fx_t > 0.3 && input.take().is_some(),
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
            Fx::Strike { target, hp_after, .. } if self.scene.is_some() => {
                let _ = (target, hp_after);
            }
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
            Fx::CombatBegin { a, d, ref f } => {
                self.combat_pair = Some((a, d));
                if self.scenes {
                    let terrain = self.b.map.at(self.b.units[d].pos);
                    self.scene = Some(Scene { a, d, f: (**f).clone(), terrain });
                }
            }
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
            Fx::Spawn { unit } => {
                self.vis_alpha[unit] = 1.0;
                self.vis_hp[unit] = self.b.units[unit].hp;
            }
            Fx::CombatEnd => {
                self.combat_pair = None;
                self.scene = None;
                self.scene_skip = false;
            }
            Fx::Strike { target, hp_after, .. } => self.vis_hp[target] = hp_after,
            Fx::Recruit { unit } => self.vis_team[unit] = Team::Player,
            Fx::Promote { unit, .. } => self.vis_hp[unit] = self.b.units[unit].hp,
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

    fn push_phase_start(&mut self, start: PhaseStart) {
        self.sync_vis();
        if !start.spawned.is_empty() {
            self.fx.push_back(Fx::Notice { text: "Enemy reinforcements!".into(), color: BAD });
            for id in start.spawned {
                self.fx.push_back(Fx::Spawn { unit: id });
            }
        }
        for h in start.heals {
            self.fx.push_back(Fx::Heal { target: h.unit, amount: h.amount, hp_after: h.hp_after });
        }
    }

    fn start_enemy_phase(&mut self) -> Mode {
        let start = self.b.begin_phase(Phase::Enemy);
        self.inspect = None;
        self.inspect_range = None;
        self.fx.push_back(Fx::Banner { text: "Enemy Phase".into(), color: BAD });
        self.push_phase_start(start);
        self.danger = None;
        Mode::EnemyPhase { queue: self.b.living(Team::Enemy).collect() }
    }

    fn enemy_step(&mut self, mut queue: VecDeque<usize>) -> Mode {
        while let Some(id) = queue.pop_front() {
            if !self.b.units[id].alive || self.b.units[id].team != Team::Enemy {
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
        let start = self.b.begin_phase(Phase::Player);
        self.fast = false;
        if let Some(lord) = self.b.units.iter().find(|u| u.is_lord && u.alive) {
            let p = pv(lord.pos);
            self.look_at(p);
        }
        if self.b.outcome() == Outcome::Ongoing {
            self.fx.push_back(Fx::Banner { text: format!("Player Phase - Turn {}", self.b.turn), color: ALLY });
        }
        self.push_phase_start(start);
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

    fn push_broken(&mut self, unit: usize, item: &str) {
        let name = self.b.units[unit].name.clone();
        self.fx.push_back(Fx::Notice { text: format!("{}'s {} broke!", name, item), color: BAD });
    }

    fn push_combat(&mut self, r: &CombatReport) {
        self.fx.push_back(Fx::CombatBegin { a: r.attacker, d: r.defender, f: Box::new(r.forecast.clone()) });
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
        for (u, item) in r.broken.clone() {
            if self.b.units[u].team == Team::Player {
                self.push_broken(u, item);
            }
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

    fn choose_target(&mut self, unit: usize, t: usize, kind: TargetKind) {
        if kind == TargetKind::Attack {
            let d = self.b.units[unit].pos.manhattan(self.b.units[t].pos);
            self.b.units[unit].equip_for_range(d);
        }
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
            Mode::Target { unit, from, targets, kind, chosen } => match self.b.unit_at(p).filter(|t| targets.contains(t)) {
                Some(t) if chosen == Some(t) => self.execute(unit, t, kind),
                Some(t) => {
                    self.choose_target(unit, t, kind);
                    Mode::Target { unit, from, targets, kind, chosen: Some(t) }
                }
                None => Mode::Target { unit, from, targets, kind, chosen },
            },
            other => other,
        };
    }

    fn execute(&mut self, unit: usize, target: usize, kind: TargetKind) -> Mode {
        match kind {
            TargetKind::Heal => {
                let r = self.b.heal(unit, target);
                self.fx.push_back(Fx::Heal { target: r.target, amount: r.amount, hp_after: r.hp_after });
                if let Some(name) = r.broken {
                    self.push_broken(unit, name);
                }
                if let Some(x) = &r.xp {
                    self.push_xp(x);
                }
            }
            TargetKind::Attack => {
                let r = self.b.attack(unit, target);
                self.push_combat(&r);
            }
            TargetKind::Talk => {
                let name = self.b.units[target].name.clone();
                let lines = self.talks.iter().find(|(n, _)| *n == name).map(|(_, l)| l.clone()).unwrap_or_default();
                for line in lines {
                    self.fx.push_back(Fx::Say { speaker: line.speaker.to_string(), text: line.text.to_string() });
                }
                self.b.recruit(target);
                self.fx.push_back(Fx::Recruit { unit: target });
                self.fx.push_back(Fx::Notice { text: format!("{} joined the army!", name), color: GOOD });
            }
        }
        self.b.finish_action(unit);
        self.inspect = None;
        self.danger = None;
        Mode::Idle
    }

    fn reward_text(&self, r: Reward) -> String {
        match r {
            Reward::Gold(g) => format!("The villagers give you {} gold.", g),
            Reward::Item(i) => format!("The villagers give you a {}.", i.name()),
        }
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
        text(&format!("Ch.{}  Turn {}", self.chapter + 1, self.b.turn), inner.x, y, u, INK);
        text_right(phase_text, inner.x + inner.w, y, u * 0.9, phase_color);
        y += u * 1.1;
        text(&self.b.objective_text(), inner.x, y, u * 0.8, ACCENT);
        y += u * 0.9;
        let lord = self.b.units.iter().find(|u| u.is_lord).map(|u| u.name.clone()).unwrap_or_default();
        let gold = if self.b.gold > 0 { format!("   +{}G", self.b.gold) } else { String::new() };
        text(&format!("{} must survive{}", lord, gold), inner.x, y, u * 0.75, DIM);
        y += u * 0.6;
        draw_line(inner.x, y, inner.x + inner.w, y, 2.0, rgb(80, 70, 110));
        y += u * 0.4;
        text("Zoom", inner.x, y + u * 1.1, u * 0.75, DIM);
        self.zoom_buttons(lay, input, inner.x + inner.w, y);
        y += u * 2.0;
        draw_line(inner.x, y, inner.x + inner.w, y, 2.0, rgb(80, 70, 110));
        y += u * 1.2;

        let idle_fx = self.fx.is_empty();
        let mut col = ButtonColumn::grid(inner, u, if matches!(self.mode, Mode::Menu { .. }) { 2 } else { 1 });

        let mode = std::mem::replace(&mut self.mode, Mode::Idle);
        self.mode = if !idle_fx || self.b.phase != Phase::Player {
            mode
        } else {
            self.panel_buttons(mode, &mut col, input, u)
        };
        let bottom = col.top() - u * 0.5;

        // Content
        match &self.mode {
            Mode::Target { unit, chosen: Some(t), kind, .. } => {
                match kind {
                    TargetKind::Heal => self.heal_preview(inner.x, y, inner.w, u, *unit, *t),
                    TargetKind::Attack => {
                        let f = self.b.forecast(*unit, *t);
                        self.forecast_box(inner.x, y, inner.w, u, *unit, *t, &f);
                    }
                    TargetKind::Talk => {
                        text_centered("Talk", inner.x + inner.w / 2.0, y, u * 0.9, ACCENT);
                        text_centered(&self.b.units[*t].name, inner.x + inner.w / 2.0, y + u * 1.5, u * 1.1, BAD);
                        text_centered("seems willing to listen...", inner.x + inner.w / 2.0, y + u * 2.8, u * 0.75, DIM);
                    }
                }
                text("Tap the target again to confirm.", inner.x, bottom, u * 0.6, DIM);
            }
            Mode::Target { .. } => {}
            Mode::EnemyPhase { .. } => {
                text("The enemy is moving...", inner.x, y, u * 0.9, DIM);
                let hint = if self.fast { "Fast-forward on" } else { "Tap to fast-forward" };
                text(hint, inner.x, y + u * 1.2, u * 0.75, if self.fast { ACCENT } else { DIM });
            }
            Mode::Items { unit, .. } => {
                let un = &self.b.units[*unit];
                text(&format!("{}'s items", un.name), inner.x, y, u, ALLY);
                y += u * 1.2;
                text("Tap an item: weapons are equipped,", inner.x, y, u * 0.7, DIM);
                y += u * 0.9;
                text("other items are used (ends turn).", inner.x, y, u * 0.7, DIM);
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
                    y = self.unit_info(inner.x, y, inner.w, u, id, bottom);
                    y += u * 0.3;
                }
                if let Some(p) = tile {
                    if y < bottom {
                        self.terrain_info(inner.x, y, u, p);
                    }
                } else if y + u * 3.0 < bottom && matches!(self.mode, Mode::Idle) && self.b.phase == Phase::Player {
                    for h in ["Tap a blue unit to move it.", "Tap an enemy to see its range.", "Grey units have already acted."] {
                        text(h, inner.x, y, u * 0.75, DIM);
                        y += u * 1.0;
                    }
                }
            }
        }
        BattleExit::Stay
    }

    fn panel_buttons(&mut self, mode: Mode, col: &mut ButtonColumn, input: &mut Input, u: f32) -> Mode {
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
                let talks = self.b.talk_targets_from(unit, pos);
                let mut next = None;
                // Bottom-up, two per row.
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
                if !self.b.units[unit].items.is_empty() && button(input, col.next(), "Items", ButtonStyle::Normal, u) {
                    next = Some(Mode::Items { unit, from });
                }
                if self.b.can_visit(unit) && button(input, col.next(), "Visit", ButtonStyle::Primary, u) {
                    if let Some(r) = self.b.visit(unit) {
                        let t = self.reward_text(r);
                        self.fx.push_back(Fx::Say { speaker: String::new(), text: t });
                    }
                    self.b.finish_action(unit);
                    next = Some(Mode::Idle);
                }
                if self.b.can_seize(unit) && button(input, col.next(), "Seize", ButtonStyle::Primary, u) {
                    self.b.seize(unit);
                    self.b.finish_action(unit);
                    next = Some(Mode::Idle);
                }
                if !talks.is_empty() && button(input, col.next(), "Talk", ButtonStyle::Primary, u) {
                    next = Some(Mode::Target { unit, from, chosen: talks.first().copied(), targets: talks, kind: TargetKind::Talk });
                }
                if !heals.is_empty() && button(input, col.next(), "Heal", ButtonStyle::Primary, u) {
                    next = Some(Mode::Target { unit, from, chosen: heals.first().copied(), targets: heals, kind: TargetKind::Heal });
                }
                if !targets.is_empty() && button(input, col.next(), "Attack", ButtonStyle::Primary, u) {
                    let first = targets[0];
                    self.choose_target(unit, first, TargetKind::Attack);
                    next = Some(Mode::Target { unit, from, chosen: Some(first), targets, kind: TargetKind::Attack });
                }
                next.unwrap_or(Mode::Menu { unit, from })
            }
            Mode::Items { unit, from } => {
                if button(input, col.next(), "Back", ButtonStyle::Normal, u) {
                    return Mode::Menu { unit, from };
                }
                let items = self.b.units[unit].items.clone();
                let mut next = Mode::Items { unit, from };
                for (idx, it) in items.iter().enumerate().rev() {
                    let usable = self.b.units[unit].can_use(it.id);
                    let equipped = self.b.units[unit].weapon_index() == Some(idx);
                    let label = format!("{}{}", if equipped { "* " } else { "" }, it.label());
                    let style = if usable { ButtonStyle::Normal } else { ButtonStyle::Disabled };
                    if button(input, col.next(), &label, style, u) {
                        match self.b.use_item(unit, idx) {
                            ItemUse::Equipped => {
                                self.fx.push_back(Fx::Notice { text: format!("Equipped {}", it.id.name()), color: INK });
                            }
                            ItemUse::Healed { amount, hp_after } => {
                                self.fx.push_back(Fx::Heal { target: unit, amount, hp_after });
                                self.b.finish_action(unit);
                                self.inspect = None;
                                next = Mode::Idle;
                            }
                            ItemUse::Promoted(p) => {
                                self.fx.push_back(Fx::Promote { unit, p });
                                self.b.finish_action(unit);
                                self.inspect = None;
                                next = Mode::Idle;
                            }
                            ItemUse::CannotUse => {
                                let why = match it.id.info().kind {
                                    ItemKind::Seal => "Needs level 10 or higher.",
                                    ItemKind::Heal(_) => "HP is already full.",
                                    ItemKind::Weapon(_) => "Can't wield that.",
                                };
                                self.fx.push_back(Fx::Notice { text: why.into(), color: DIM });
                            }
                        }
                    }
                }
                next
            }
            Mode::Target { unit, from, targets, kind, chosen } => {
                if button(input, col.next(), "Back", ButtonStyle::Normal, u) {
                    return Mode::Menu { unit, from };
                }
                let Some(t) = chosen else { return Mode::Target { unit, from, targets, kind, chosen } };
                let label = match kind {
                    TargetKind::Attack => "Attack!",
                    TargetKind::Heal => "Heal!",
                    TargetKind::Talk => "Talk!",
                };
                if button(input, col.next(), label, ButtonStyle::Primary, u) {
                    return self.execute(unit, t, kind);
                }
                if kind == TargetKind::Attack {
                    let dist = self.b.units[unit].pos.manhattan(self.b.units[t].pos);
                    let options: Vec<usize> = self.b.units[unit].attack_weapons().into_iter().filter(|(_, w)| w.in_range(dist)).map(|(i, _)| i).collect();
                    if options.len() > 1 {
                        let name = self.b.units[unit].weapon().map(|w| w.name).unwrap_or("-");
                        if button(input, col.next(), &format!("Weapon: {} >", name), ButtonStyle::Normal, u) {
                            // Equip the next in-range weapon (inventory order rotates).
                            let cur = self.b.units[unit].weapon_index().unwrap_or(0);
                            let nxt = options.iter().copied().find(|&i| i > cur).unwrap_or(options[0]);
                            self.b.units[unit].equip(nxt);
                        }
                    }
                }
                Mode::Target { unit, from, targets, kind, chosen }
            }
            other => other,
        }
    }

    fn unit_info(&self, x: f32, mut y: f32, w: f32, u: f32, id: usize, bottom: f32) -> f32 {
        let un = &self.b.units[id];
        let hp = self.vis_hp[id];
        text(&un.name, x, y, u * 1.15, team_ink(self.vis_team[id]));
        text_right(&format!("Lv {}", un.level), x + w, y, u, INK);
        y += u * 1.05;
        let role = if un.is_lord {
            format!("{} (Lord)", un.class.name())
        } else if un.is_boss {
            format!("{} (Boss)", un.class.name())
        } else if un.talk {
            format!("{} (Talk?)", un.class.name())
        } else {
            un.class.name().to_string()
        };
        text(&role, x, y, u * 0.85, DIM);
        if un.team == Team::Player {
            text_right(&format!("EXP {}", un.xp), x + w, y, u * 0.85, DIM);
        }
        let bonds: Vec<String> = self
            .b
            .supports
            .iter()
            .filter(|st| st.level > 0 && st.involves(&un.name))
            .map(|st| {
                let p = st.partner(&un.name).unwrap_or("");
                let near = self.b.active_partners(id, un.pos).iter().any(|(n, _)| n == p);
                format!("{} {}{}", p, crate::logic::supports::level_name(st.level), if near { "*" } else { "" })
            })
            .collect();
        if !bonds.is_empty() {
            y += u * 0.85;
            text(&format!("Support: {}", bonds.join(", ")), x, y, u * 0.65, GOOD);
        }
        y += u * 0.6;
        hp_bar(x, y, w * 0.6, u * 0.55, hp, un.stats.hp);
        text_right(&format!("HP {}/{}", hp, un.stats.hp), x + w, y + u * 0.55, u * 0.85, INK);
        y += u * 1.75;
        let s = un.stats;
        let vals = [(un.pow_label(), s.pow), ("Skl", s.skl), ("Spd", s.spd), ("Lck", s.lck), ("Def", s.def), ("Res", s.res)];
        let cw = w / 3.0;
        for (i, (name, v)) in vals.iter().enumerate() {
            let cx = x + (i % 3) as f32 * cw;
            let cy = y + (i / 3) as f32 * u * 1.05;
            text(name, cx, cy, u * 0.85, DIM);
            text_right(&v.to_string(), cx + cw - u * 0.6, cy, u * 0.85, INK);
        }
        y += u * 2.2;
        let eq = un.weapon_index();
        for (i, it) in un.items.iter().enumerate() {
            if y > bottom - u * 1.6 {
                break;
            }
            let c = if Some(i) == eq { ACCENT } else if un.can_use(it.id) { INK } else { DIM };
            text(&format!("{}{}", if Some(i) == eq { "* " } else { "  " }, it.label()), x, y, u * 0.72, c);
            y += u * 0.85;
        }
        text(&format!("Mov {}", un.mov()), x, y, u * 0.72, DIM);
        y + u * 0.4
    }

    fn terrain_info(&self, x: f32, y: f32, u: f32, p: Pos) {
        let t = self.b.map.at(p);
        let mut s = format!("{}  Def +{}  Avo +{}", t.name(), t.defense(), t.avoid());
        if t.heal_percent() > 0 {
            s.push_str(&format!("  Heal {}%", t.heal_percent()));
        }
        if t.move_cost(MoveType::Infantry).is_none() {
            s = format!("{}  (impassable)", t.name());
        }
        if let Some(v) = self.b.village_at(p) {
            s.push_str(if v.visited { "  (visited)" } else { "  Visit!" });
        }
        text(&s, x, y + u * 0.3, u * 0.72, rgb(170, 200, 150));
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
            1 => ("+", GOOD),
            -1 => ("-", BAD),
            _ => ("", DIM),
        };
        let (ta, ca) = tri(&f.attacker);
        let (td, cd) = tri(&f.defender);
        let aw = a.weapon().map(|w| w.name).unwrap_or("-");
        let dw = d.weapon().map(|w| w.name).unwrap_or("-");
        text_centered(&format!("{} {}", aw, ta), lx, y, u * 0.65, ca);
        text_centered(&format!("{} {}", dw, td), rx, y, u * 0.65, cd);
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
        let amount = h.staff().map(|s| combat::heal_amount(h, &s)).unwrap_or(0).min(t.stats.hp - t.hp);
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
            match self.fx.front() {
                Some(Fx::Die { unit }) if *unit == i => {
                    return (1.0 - self.fx_t / 0.7).max(0.0) * if (self.fx_t * 20.0) as i32 % 2 == 0 { 1.0 } else { 0.5 };
                }
                Some(Fx::Spawn { unit }) if *unit == i => return (self.fx_t / 0.5).min(1.0),
                _ => {}
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
        let (vx0, vy0, vx1, vy1) = lay.visible();
        for y in vy0..=vy1 {
            for x in vx0..=vx1 {
                let p = Pos::new(x, y);
                let r = lay.tile_rect(x as f32, y as f32);
                let visited = self.b.village_at(p).is_some_and(|v| v.visited);
                gfx::draw_terrain(m.at(p), r.x, r.y, r.w, x, y, self.time, visited);
            }
        }
        let gc = Color::new(0.0, 0.0, 0.0, 0.12);
        for x in vx0..=vx1 + 1 {
            let px = lay.tile_rect(x as f32, 0.0).x;
            if px >= lay.ox && px <= lay.ox + lay.map_w {
                draw_line(px, lay.oy, px, lay.oy + lay.map_h, 1.0, gc);
            }
        }
        for y in vy0..=vy1 + 1 {
            let py = lay.tile_rect(0.0, y as f32).y;
            if py >= lay.oy && py <= lay.oy + lay.map_h {
                draw_line(lay.ox, py, lay.ox + lay.map_w, py, 1.0, gc);
            }
        }
        if let Objective::Seize(p) = self.b.objective {
            let a = 0.5 + 0.5 * (self.time * 3.0).sin();
            let r = lay.tile_rect(p.x as f32, p.y as f32);
            draw_rectangle_lines(r.x + 2.0, r.y + 2.0, r.w - 4.0, r.h - 4.0, 3.0, Color::new(1.0, 0.85, 0.3, a));
        }

        if let (Some(d), true) = (&self.danger, self.show_danger) {
            for p in d {
                self.shade(lay, *p, Color::new(0.6, 0.1, 0.5, 0.28));
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
                let heal = self.b.units[*unit].attack_range().is_none();
                let edge = if heal { Color::new(0.2, 0.9, 0.4, 0.35) } else { Color::new(1.0, 0.2, 0.2, 0.38) };
                for p in threat.difference(dests) {
                    self.shade(lay, *p, edge);
                }
                for p in dests {
                    self.shade(lay, *p, Color::new(0.25, 0.5, 1.0, 0.45 + pulse));
                }
            }
            Mode::Target { targets, chosen, kind, .. } => {
                let c = match kind {
                    TargetKind::Attack => Color::new(1.0, 0.2, 0.2, 0.42 + pulse),
                    TargetKind::Heal => Color::new(0.2, 0.95, 0.4, 0.4 + pulse),
                    TargetKind::Talk => Color::new(0.3, 0.7, 1.0, 0.42 + pulse),
                };
                for t in targets {
                    self.shade(lay, self.b.units[*t].pos, c);
                }
                if let Some(t) = chosen {
                    self.cursor(lay, self.b.units[*t].pos, ACCENT);
                }
            }
            _ => {}
        }

        let mut order: Vec<usize> = (0..self.vis_pos.len()).collect();
        order.sort_by(|a, b| self.draw_pos(*a).y.partial_cmp(&self.draw_pos(*b).y).unwrap_or(std::cmp::Ordering::Equal));
        for i in order {
            let alpha = self.draw_alpha(i);
            if alpha <= 0.0 {
                continue;
            }
            let un = &self.b.units[i];
            let v = self.draw_pos(i);
            if v.x < lay.cam.x - 1.0 || v.y < lay.cam.y - 1.0 || v.x > lay.cam.x + lay.view_tiles().x || v.y > lay.cam.y + lay.view_tiles().y {
                continue;
            }
            let r = lay.tile_rect(v.x, v.y);
            let moving = matches!(self.mode, Mode::Selected { unit, .. } if unit == i);
            let bob = !un.done && (((self.time * if moving { 6.0 } else { 2.0 }) + i as f32 * 0.37) as i32 % 2 == 0);
            let inset = r.w * 0.06;
            let mut lk = UnitLook::of(un);
            lk.team = self.vis_team[i];
            gfx::draw_unit(lk, r.x + inset, r.y + inset * 1.5, r.w - inset * 2.0, un.done, alpha, bob);
            if un.is_boss || un.is_lord || un.talk {
                let c = if un.is_boss {
                    ACCENT
                } else if un.talk {
                    GOOD
                } else {
                    rgb(120, 200, 255)
                };
                draw_rectangle(r.x + r.w * 0.06, r.y + r.h * 0.06, r.w * 0.12, r.w * 0.12, Color::new(c.r, c.g, c.b, alpha));
            }
            let bw = r.w * 0.8;
            hp_bar(r.x + (r.w - bw) / 2.0, r.y + r.h - r.h * 0.1, bw, (r.h * 0.09).max(3.0), self.vis_hp[i], un.stats.hp);
        }

        match &self.mode {
            Mode::Selected { unit, .. } | Mode::Menu { unit, .. } | Mode::Items { unit, .. } => self.cursor(lay, self.b.units[*unit].pos, INK),
            Mode::Idle => {
                if let Some(p) = self.inspect {
                    self.cursor(lay, p, INK);
                }
            }
            _ => {}
        }

        for f in &self.floats {
            let r = lay.tile_rect(f.pos.x, f.pos.y - f.t * 0.6);
            let mut c = f.color;
            c.a = (1.0 - f.t).clamp(0.0, 1.0);
            text_centered(&f.text, r.x + r.w / 2.0, r.y + r.h * 0.2, lay.tile * 0.42, c);
        }

        // Hide anything scrolled outside the viewport.
        let bg = rgb(16, 16, 30);
        let (sw, sh) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, sw, lay.oy, bg);
        draw_rectangle(0.0, lay.oy + lay.map_h, sw, sh, bg);
        draw_rectangle(0.0, 0.0, lay.ox, sh, bg);
        draw_rectangle(lay.ox + lay.map_w, 0.0, sw, sh, bg);
        if lay.scrollable() {
            // Scroll position indicators.
            let v = lay.view_tiles();
            let bw = lay.map_w * (v.x / lay.cols as f32).min(1.0);
            let bx = lay.ox + lay.map_w * (lay.cam.x / lay.cols as f32);
            draw_rectangle(bx, lay.oy + lay.map_h - 4.0, bw, 4.0, Color::new(1.0, 1.0, 1.0, 0.35));
            let bh = lay.map_h * (v.y / lay.rows as f32).min(1.0);
            let by = lay.oy + lay.map_h * (lay.cam.y / lay.rows as f32);
            draw_rectangle(lay.ox + lay.map_w - 4.0, by, 4.0, bh, Color::new(1.0, 1.0, 1.0, 0.35));
        }
    }

    // -------------------------------------------------------------- overlays

    fn draw_overlays(&self, lay: &Layout) {
        let u = lay.u;
        let map = Rect::new(lay.ox, lay.oy, lay.map_w, lay.map_h);

        if self.scene.is_some() {
            self.draw_scene();
            return;
        }
        if let Some((a, d)) = self.combat_pair {
            let w = map.w * 0.8;
            let h = u * 3.4;
            let r = Rect::new(map.x + (map.w - w) / 2.0, map.y + u * 0.5, w, h);
            frame(r);
            for (k, id) in [a, d].into_iter().enumerate() {
                let un = &self.b.units[id];
                let x = r.x + u + k as f32 * (w / 2.0);
                let bw = w / 2.0 - u * 2.0;
                text(&un.name, x, r.y + u * 1.4, u, team_ink(self.vis_team[id]));
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
                draw_rectangle(map.x, y, map.w, h, Color::new(0.05, 0.05, 0.1, 0.85 * a));
                draw_rectangle(map.x, y, map.w, 3.0, Color::new(color.r, color.g, color.b, a));
                draw_rectangle(map.x, y + h - 3.0, map.w, 3.0, Color::new(color.r, color.g, color.b, a));
                let mut c = *color;
                c.a = a;
                let size = (u * 1.9).min(map.w * 0.9 / (t.len() as f32 * 0.55).max(1.0));
                text_centered(t, map.x + map.w / 2.0 + slide, y + h / 2.0 + size * 0.33, size, c);
            }
            Some(Fx::Notice { text: t, color }) => {
                let a = (self.fx_t * 6.0).min(1.0).min((1.2 - self.fx_t) * 6.0).clamp(0.0, 1.0);
                let w = (text_w(t, u) + u * 2.0).min(map.w);
                let r = Rect::new(map.x + (map.w - w) / 2.0, map.y + map.h * 0.18, w, u * 2.0);
                draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.08, 0.08, 0.16, 0.9 * a));
                draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, Color::new(color.r, color.g, color.b, a));
                let mut c = *color;
                c.a = a;
                text_centered(t, r.x + w / 2.0, r.y + u * 1.35, u, c);
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
                let sub = format!("{}  {}  Lv {}", un.name, un.class.name(), lv.new_level);
                stat_popup(map, u, "LEVEL UP!", &sub, un.pow_label(), lv.gains.to_array(), lv.after.to_array(), self.fx_t);
            }
            Some(Fx::Promote { unit, p }) => {
                let un = &self.b.units[*unit];
                let sub = format!("{}: {} > {}", un.name, p.from.name(), p.to.name());
                stat_popup(map, u, "PROMOTION!", &sub, un.pow_label(), p.gains.to_array(), un.stats.to_array(), self.fx_t);
            }
            Some(Fx::Say { speaker, text: t }) => {
                let look = self.b.units.iter().find(|u| !speaker.is_empty() && u.name == *speaker).map(UnitLook::of);
                dialogue_box(map, u, speaker, t, look, self.fx_t);
            }
            _ => {}
        }
    }

    /// Close-up side view of the current fight. Player units stand on the right.
    fn draw_scene(&self) {
        let Some(sc) = &self.scene else { return };
        let (w, h) = (screen_width(), screen_height());
        let u = ui_unit();
        let t = self.time;
        let indoor = matches!(sc.terrain, Terrain::Floor | Terrain::Pillar | Terrain::Throne | Terrain::Gate | Terrain::Wall);

        // Current strike, if any, and its progress (0..1 over the strike).
        let strike = match self.fx.front() {
            Some(Fx::Strike { by, target, hit, crit, damage, hp_after, .. }) if self.fx_started => Some((*by, *target, *hit, *crit, *damage, *hp_after)),
            _ => None,
        };
        let dur = self.fx_duration().unwrap_or(SCENE_STRIKE).max(0.01);
        let k = (self.fx_t / dur).clamp(0.0, 1.0);
        let impact = SCENE_IMPACT / SCENE_STRIKE;
        let crit_flash = matches!(strike, Some((.., true, _, _))) && strike.is_some_and(|s| s.2) && k > impact && k < impact + 0.18;
        let shake = if crit_flash { ((t * 70.0).sin() * u * 0.4, (t * 53.0).cos() * u * 0.3) } else { (0.0, 0.0) };

        // Backdrop
        if indoor {
            for i in 0..12 {
                let c = 0.10 + i as f32 * 0.012;
                draw_rectangle(0.0, h * i as f32 / 12.0 * 0.62, w, h * 0.62 / 12.0 + 1.0, Color::new(c, c * 0.95, c * 1.2, 1.0));
            }
            for x in 0..8 {
                let px = w * (x as f32 + 0.5) / 8.0;
                draw_rectangle(px - u * 0.8, h * 0.05, u * 1.6, h * 0.57, rgb(70, 70, 86));
                draw_rectangle(px - u * 0.8, h * 0.05, u * 0.4, h * 0.57, rgb(96, 96, 112));
            }
        } else {
            for i in 0..12 {
                let f = i as f32 / 12.0;
                draw_rectangle(0.0, h * f * 0.62, w, h * 0.62 / 12.0 + 1.0, Color::new(0.38 + f * 0.25, 0.58 + f * 0.2, 0.92, 1.0));
            }
            for (cx, cy, r) in [(0.2, 0.15, 0.06), (0.27, 0.13, 0.045), (0.7, 0.2, 0.05), (0.78, 0.18, 0.07)] {
                draw_circle(w * cx + (t * 6.0) % w * 0.02, h * cy, h * r, Color::new(1.0, 1.0, 1.0, 0.8));
            }
            // distant hills
            for x in 0..10 {
                let px = w * x as f32 / 9.0;
                draw_circle(px, h * 0.64, h * 0.12, rgb(84, 140, 90));
            }
        }
        let gt = (h * 0.13).floor();
        let gy = (h * 0.6).floor();
        let cols = (w / gt).ceil() as i32 + 1;
        for row in 0..4 {
            for c in 0..cols {
                gfx::draw_terrain(sc.terrain, c as f32 * gt, gy + row as f32 * gt, gt, c, row, t, false);
            }
        }
        draw_rectangle(0.0, gy, w, h * 0.02, Color::new(0.0, 0.0, 0.0, 0.18));

        // Fighters: player side on the right.
        let (left_id, right_id) = if self.b.units[sc.a].team == Team::Player { (sc.d, sc.a) } else { (sc.a, sc.d) };
        let size = (h * 0.36).floor();
        let base_y = gy - size * 0.92;
        let home = |id: usize| if id == right_id { w * 0.66 } else { w * 0.34 - size };
        let mut xs = [(left_id, home(left_id)), (right_id, home(right_id))];
        let mut alphas = [1.0f32, 1.0];
        let mut projectile: Option<(f32, f32, WeaponKind)> = None;
        if let Some((by, target, hit, _crit, _dmg, hp_after)) = strike {
            let dir = if by == right_id { -1.0 } else { 1.0 };
            let kind = self.b.units[by].weapon().map(|w| w.kind).unwrap_or(WeaponKind::Sword);
            let ranged = matches!(kind, WeaponKind::Bow | WeaponKind::Magic);
            let gap = (home(right_id) - home(left_id)) - size * 0.55;
            for (id, x) in xs.iter_mut() {
                if *id == by && !ranged {
                    let lunge = if k < impact {
                        (k / impact).powf(2.0)
                    } else if k < impact + 0.12 {
                        1.0
                    } else {
                        (1.0 - (k - impact - 0.12) / (1.0 - impact - 0.12)).max(0.0)
                    };
                    *x += dir * gap * lunge;
                }
                if *id == by && ranged && k < impact {
                    *x -= dir * u * 0.5 * (k / impact * PI).sin();
                }
                if *id == target && k > impact {
                    let after = (k - impact) / (1.0 - impact);
                    if hit {
                        *x += dir * (after * 30.0).sin() * u * 0.35 * (1.0 - after);
                    } else {
                        *x += dir * u * 2.5 * (after * PI).sin();
                    }
                }
            }
            if ranged && k > impact * 0.35 && k < impact {
                let p = (k - impact * 0.35) / (impact * 0.65);
                let from = home(by) + size * 0.5;
                let to = home(target) + size * 0.5;
                projectile = Some((from + (to - from) * p, base_y + size * 0.45, kind));
            }
            if hp_after == 0 && k > impact {
                let i = if target == left_id { 0 } else { 1 };
                alphas[i] = (1.0 - (k - impact) / (1.0 - impact) * 1.2).max(0.0);
            }
        }
        for (slot, (id, x)) in xs.iter().enumerate() {
            let un = &self.b.units[*id];
            let mut lk = UnitLook::of(un);
            lk.team = self.vis_team[*id];
            let bob = ((t * 2.5) as i32 + slot as i32) % 2 == 0;
            gfx::draw_unit_facing(lk, x + shake.0, base_y + shake.1, size, false, alphas[slot], bob, *id == right_id);
        }

        // Projectiles and impact effects
        if let Some((px, py, kind)) = projectile {
            match kind {
                WeaponKind::Bow => {
                    let dir = if px > w / 2.0 { 1.0 } else { -1.0 };
                    draw_line(px - dir * u * 2.0, py, px, py, 3.0, rgb(120, 80, 40));
                    draw_triangle(vec2(px + dir * u * 0.6, py), vec2(px, py - u * 0.3), vec2(px, py + u * 0.3), rgb(220, 224, 236));
                }
                _ => {
                    let r = u * (0.6 + 0.4 * (t * 20.0).sin().abs());
                    draw_circle(px, py, r * 1.6, Color::new(1.0, 0.5, 0.1, 0.35));
                    draw_circle(px, py, r, rgb(255, 210, 120));
                }
            }
        }
        if let Some((by, target, hit, crit, dmg, _)) = strike {
            let tx = xs.iter().find(|(id, _)| *id == target).map(|(_, x)| *x).unwrap_or(0.0) + size * 0.5;
            let ty = base_y + size * 0.45;
            if k > impact && k < impact + 0.2 && hit {
                let a = 1.0 - (k - impact) / 0.2;
                let kind = self.b.units[by].weapon().map(|w| w.kind).unwrap_or(WeaponKind::Sword);
                if kind == WeaponKind::Magic {
                    draw_circle(tx, ty, size * 0.45 * (1.0 - a * 0.5), Color::new(1.0, 0.6, 0.2, 0.6 * a));
                } else {
                    for i in 0..5 {
                        let ang = i as f32 * 1.256 + t * 3.0;
                        let r0 = size * 0.1;
                        let r1 = size * (0.25 + 0.2 * (1.0 - a));
                        draw_line(tx + ang.cos() * r0, ty + ang.sin() * r0, tx + ang.cos() * r1, ty + ang.sin() * r1, 4.0, Color::new(1.0, 1.0, 0.85, a));
                    }
                }
            }
            if k > impact {
                let rise = (k - impact) / (1.0 - impact);
                let (label, c) = if !hit {
                    ("MISS".to_string(), DIM)
                } else if dmg == 0 {
                    ("NO DAMAGE".to_string(), DIM)
                } else {
                    (format!("{}", dmg), if crit { ACCENT } else { INK })
                };
                let size_t = if crit && hit { u * 2.6 } else { u * 1.8 };
                text_outlined(&label, tx, base_y - u * 0.5 - rise * u * 2.0, size_t, Color::new(c.r, c.g, c.b, 1.0 - rise * 0.6));
            }
            if crit && hit && k < impact {
                text_outlined("CRITICAL!", w / 2.0, h * 0.18, u * 2.4, Color::new(1.0, 0.85, 0.3, 0.6 + 0.4 * (t * 20.0).sin().abs()));
            }
        }
        if crit_flash {
            let a = 1.0 - (k - impact) / 0.18;
            draw_rectangle(0.0, 0.0, w, h, Color::new(1.0, 1.0, 1.0, 0.65 * a));
        }

        // Stat panels along the bottom
        let ph = h * 0.22;
        let py = h - ph;
        for (slot, id) in [left_id, right_id].into_iter().enumerate() {
            let un = &self.b.units[id];
            let side = if id == sc.a { &sc.f.attacker } else { &sc.f.defender };
            let r = Rect::new(slot as f32 * w / 2.0, py, w / 2.0, ph);
            draw_rectangle(r.x, r.y, r.w, r.h, if self.vis_team[id] == Team::Player { rgb(30, 46, 100) } else { rgb(100, 30, 34) });
            draw_rectangle_lines(r.x + 3.0, r.y + 3.0, r.w - 6.0, r.h - 6.0, 3.0, rgb(220, 200, 140));
            let x = r.x + u;
            text(&un.name, x, r.y + u * 1.6, u * 1.2, INK);
            let wn = un.weapon().map(|w| w.name).unwrap_or("-");
            text_right(wn, r.x + r.w - u, r.y + u * 1.6, u * 0.9, ACCENT);
            let stats = if side.strikes == 0 {
                "HIT --  DMG --  CRT --".to_string()
            } else {
                format!("HIT {}  DMG {}{}  CRT {}", side.hit, side.damage, if side.strikes > 1 { " x2" } else { "" }, side.crit)
            };
            text(&stats, x, r.y + u * 3.0, u * 0.85, DIM);
            let hp = self.vis_hp[id];
            text(&format!("HP {}", hp), x, r.y + ph - u * 1.0, u * 1.1, INK);
            hp_bar(x + u * 4.2, r.y + ph - u * 1.7, r.w - u * 5.5, u * 0.8, hp, un.stats.hp);
        }
        text_right("Tap to skip", w - u * 0.5, u * 1.2, u * 0.7, Color::new(1.0, 1.0, 1.0, 0.6));
    }

    fn over_screen(&mut self, lay: &Layout, input: &mut Input, victory: bool) -> BattleExit {
        let u = lay.u;
        let map = Rect::new(lay.ox, lay.oy, lay.map_w, lay.map_h);
        draw_rectangle(map.x, map.y, map.w, map.h, Color::new(0.0, 0.0, 0.0, 0.45));
        let players: Vec<usize> = (0..self.b.units.len()).filter(|&i| self.b.units[i].team == Team::Player).collect();
        let w = (map.w * 0.7).max(u * 16.0).min(screen_width() - u);
        let h = (u * 8.5 + players.len() as f32 * u * 1.0).min(screen_height() - u);
        let r = Rect::new(map.x + (map.w - w) / 2.0, (map.y + (map.h - h) / 2.0).max(0.0), w, h);
        frame(r);
        let (title, c) = if victory { ("CHAPTER CLEAR", ACCENT) } else { ("DEFEAT", BAD) };
        text_centered(title, r.x + w / 2.0, r.y + u * 2.0, u * 1.8, c);
        let sub = if victory {
            format!("{} - {} turns", self.title, self.b.turn)
        } else {
            let lord = self.b.units.iter().find(|u| u.is_lord).map(|u| u.name.as_str()).unwrap_or("The lord");
            format!("{} has fallen.", lord)
        };
        text_centered(&sub, r.x + w / 2.0, r.y + u * 3.3, u * 0.8, INK);
        let mut y = r.y + u * 4.6;
        for i in players {
            let un = &self.b.units[i];
            text(&format!("{}  Lv {} {}", un.name, un.level, un.class.name()), r.x + u, y, u * 0.75, ALLY);
            let (status, sc) = if un.alive {
                (format!("HP {}/{}", un.hp, un.stats.hp), INK)
            } else if self.b.permadeath {
                ("Fallen - lost forever".to_string(), BAD)
            } else {
                ("Retreated - will return".to_string(), DIM)
            };
            text_right(&status, r.x + w - u, y, u * 0.75, sc);
            y += u * 1.0;
        }
        let bw = (w - u * 3.0) / 2.0;
        let bh = u * 2.1;
        let by = r.y + h - bh - u * 0.8;
        let mut exit = BattleExit::Stay;
        if victory {
            if button(input, Rect::new(r.x + u, by, w - u * 2.0, bh), "Continue", ButtonStyle::Primary, u) {
                exit = BattleExit::Victory;
            }
        } else {
            if button(input, Rect::new(r.x + u, by, bw, bh), "Retry Chapter", ButtonStyle::Primary, u) {
                exit = BattleExit::Retry;
            }
            if button(input, Rect::new(r.x + u * 2.0 + bw, by, bw, bh), "Title", ButtonStyle::Normal, u) {
                exit = BattleExit::Title;
            }
        }
        exit
    }
}

/// Level-up / promotion window.
#[allow(clippy::too_many_arguments)]
fn stat_popup(map: Rect, u: f32, title: &str, sub: &str, pow_label: &str, gains: [i32; 7], after: [i32; 7], t: f32) {
    let w = (map.w * 0.6).max(u * 14.0).min(map.w);
    let h = u * 9.5;
    let r = Rect::new(map.x + (map.w - w) / 2.0, map.y + (map.h - h) / 2.0, w, h);
    frame(r);
    text_centered(title, r.x + w / 2.0, r.y + u * 1.6, u * 1.4, ACCENT);
    text_centered(sub, r.x + w / 2.0, r.y + u * 2.8, u * 0.9, INK);
    let cw = (w - u * 2.0) / 2.0;
    for i in 0..7 {
        let col = (i % 2) as f32;
        let row = (i / 2) as f32;
        let x = r.x + u + col * cw;
        let y = r.y + u * 4.4 + row * u * 1.15;
        let name = if i == 1 { pow_label } else { STAT_NAMES[i] };
        text(name, x, y, u * 0.9, DIM);
        text_right(&after[i].to_string(), x + cw * 0.6, y, u * 0.9, INK);
        if gains[i] > 0 {
            text(&format!("+{}", gains[i]), x + cw * 0.66, y, u * 0.9, GOOD);
        }
    }
    if t > 0.3 {
        text_centered("Tap to continue", r.x + w / 2.0, r.y + h - u * 0.6, u * 0.7, DIM);
    }
}

/// Dialogue box along the bottom of `area` with an optional portrait.
pub fn dialogue_box(area: Rect, u: f32, speaker: &str, body: &str, look: Option<UnitLook>, t: f32) {
    let h = (u * 6.5).min(area.h * 0.5);
    let r = Rect::new(area.x + u * 0.5, area.y + area.h - h - u * 0.5, area.w - u, h);
    frame(r);
    let ps = h - u * 1.2;
    let mut tx = r.x + u * 0.8;
    if let Some(l) = look {
        gfx::draw_portrait(l, r.x + u * 0.6, r.y + u * 0.6, ps);
        tx += ps + u * 0.4;
    }
    let mut y = r.y + u * 1.5;
    if !speaker.is_empty() {
        let c = match look.map(|l| l.team) {
            Some(Team::Enemy) => BAD,
            _ => ALLY,
        };
        text(speaker, tx, y, u * 1.05, c);
        y += u * 1.2;
    }
    let size = u * 0.85;
    let lines = wrap(body, r.x + r.w - tx - u * 0.8, size);
    // Typewriter reveal.
    let total: usize = lines.iter().map(|l| l.len()).sum();
    let mut shown = ((t * 90.0) as usize).min(total);
    for line in lines {
        if shown == 0 {
            break;
        }
        let n = line.len().min(shown);
        let part: String = line.chars().take(n).collect();
        text(&part, tx, y, size, INK);
        shown -= n;
        y += size * 1.25;
    }
    if t > 0.3 {
        let blink = ((t * 3.0) as i32 % 2) == 0;
        if blink {
            text_right("v", r.x + r.w - u * 0.6, r.y + r.h - u * 0.5, u * 0.8, ACCENT);
        }
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
