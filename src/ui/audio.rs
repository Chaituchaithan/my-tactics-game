//! Chiptune music and sound effects, synthesised in code at startup (no
//! audio files). Tracks are written in a tiny note language:
//! `C4` = note, `C#4`/`Db4` = sharps/flats, `-` = rest, `C4:2` = two steps long.

use std::cell::RefCell;
use std::collections::HashMap;

use macroquad::audio::{load_sound_from_bytes, play_sound, stop_sound, PlaySoundParams, Sound};

const RATE: u32 = 22_050;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Track {
    Title,
    Story,
    Player,
    Enemy,
    Boss,
    Victory,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Sfx {
    Select,
    Back,
    Hit,
    Crit,
    Miss,
    Heal,
    LevelUp,
    Death,
    Chest,
    Door,
    Steal,
    Achievement,
}

#[derive(Clone, Copy)]
enum Wave {
    Square(f32),
    Triangle,
    Noise,
}

fn note_freq(tok: &str) -> Option<f32> {
    let b = tok.as_bytes();
    let base = match b.first()? {
        b'C' => 0,
        b'D' => 2,
        b'E' => 4,
        b'F' => 5,
        b'G' => 7,
        b'A' => 9,
        b'B' => 11,
        _ => return None,
    };
    let (acc, rest) = match b.get(1) {
        Some(b'#') => (1, &tok[2..]),
        Some(b'b') => (-1, &tok[2..]),
        _ => (0, &tok[1..]),
    };
    let octave: i32 = rest.parse().ok()?;
    let midi = 12 * (octave + 1) + base + acc;
    Some(440.0 * 2f32.powf((midi - 69) as f32 / 12.0))
}

struct Synth {
    buf: Vec<f32>,
    noise: u32,
}

impl Synth {
    fn new(len: usize) -> Synth {
        Synth { buf: vec![0.0; len], noise: 0x1234_5678 }
    }

    fn rand(&mut self) -> f32 {
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        (self.noise as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// Adds a note with a short attack/decay envelope. `slide` bends pitch per second.
    #[allow(clippy::too_many_arguments)]
    fn note(&mut self, start: f32, dur: f32, freq: f32, wave: Wave, vol: f32, slide: f32, decay: f32) {
        let s0 = (start * RATE as f32) as usize;
        let n = (dur * RATE as f32) as usize;
        let mut phase = 0.0f32;
        for i in 0..n {
            let idx = s0 + i;
            if idx >= self.buf.len() {
                break;
            }
            let t = i as f32 / RATE as f32;
            let f = (freq + slide * t).max(20.0);
            phase = (phase + f / RATE as f32).fract();
            let v = match wave {
                Wave::Square(duty) => {
                    if phase < duty {
                        1.0
                    } else {
                        -1.0
                    }
                }
                Wave::Triangle => 4.0 * (phase - 0.5).abs() - 1.0,
                Wave::Noise => self.rand(),
            };
            let attack = (t / 0.005).min(1.0);
            let release = ((dur - t) / 0.02).clamp(0.0, 1.0);
            let env = attack * release * (-t * decay).exp();
            self.buf[idx] += v * vol * env;
        }
    }

    /// Plays a note string on one channel.
    #[allow(clippy::too_many_arguments)]
    fn line(&mut self, notes: &str, start: f32, step: f32, wave: Wave, vol: f32, decay: f32, transpose: f32) -> f32 {
        let mut t = start;
        for tok in notes.split_whitespace() {
            let (n, len) = match tok.split_once(':') {
                Some((n, l)) => (n, l.parse::<f32>().unwrap_or(1.0)),
                None => (tok, 1.0),
            };
            if let Some(f) = note_freq(n) {
                self.note(t, step * len * 0.95, f * transpose, wave, vol, 0.0, decay);
            }
            t += step * len;
        }
        t
    }

    fn drums(&mut self, pattern: &str, start: f32, step: f32, bars: usize, vol: f32) {
        let mut t = start;
        for _ in 0..bars {
            for c in pattern.chars() {
                match c {
                    'k' => self.note(t, 0.12, 110.0, Wave::Triangle, vol * 1.4, -500.0, 18.0),
                    's' => self.note(t, 0.1, 0.0, Wave::Noise, vol * 0.7, 0.0, 30.0),
                    'h' => self.note(t, 0.03, 0.0, Wave::Noise, vol * 0.3, 0.0, 80.0),
                    _ => {}
                }
                t += step;
            }
        }
    }

    fn wav(&self) -> Vec<u8> {
        let peak = self.buf.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1.0);
        let data: Vec<i16> = self.buf.iter().map(|v| ((v / peak) * 0.9 * i16::MAX as f32) as i16).collect();
        let mut out = Vec::with_capacity(44 + data.len() * 2);
        let len = (data.len() * 2) as u32;
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&RATE.to_le_bytes());
        out.extend_from_slice(&(RATE * 2).to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&len.to_le_bytes());
        for s in data {
            out.extend_from_slice(&s.to_le_bytes());
        }
        out
    }
}

/// A looping piece: melody, bass and drums repeated over `bars` of 8 steps.
struct Piece {
    bpm: f32,
    melody: &'static str,
    harmony: &'static str,
    bass: &'static str,
    drums: &'static str,
    bars: usize,
}

fn render(p: &Piece) -> Vec<u8> {
    let step = 60.0 / p.bpm / 2.0; // eighth notes
    let len = step * 8.0 * p.bars as f32;
    let mut s = Synth::new((len * RATE as f32) as usize + 10);
    s.line(p.melody, 0.0, step, Wave::Square(0.25), 0.22, 2.5, 1.0);
    s.line(p.harmony, 0.0, step, Wave::Square(0.5), 0.10, 3.0, 1.0);
    s.line(p.bass, 0.0, step, Wave::Triangle, 0.38, 1.2, 1.0);
    if !p.drums.is_empty() {
        s.drums(p.drums, 0.0, step, p.bars, 0.25);
    }
    s.wav()
}

fn track(t: Track) -> Piece {
    match t {
        // Heroic theme in D minor -> F major.
        Track::Title => Piece {
            bpm: 92.0,
            melody: "D5:2 A4 D5 E5:2 F5:2 E5:2 D5 C5 D5:4 A4:2 C5:2 F5:2 E5 D5 C5:2 A4:2 G4:4 \
                     Bb4:2 C5 D5 F5:2 E5:2 D5:2 C5 Bb4 A4:4 G4 A4 Bb4 C5 D5:2 C#5:2 D5:8",
            harmony: "F4:4 G4:4 A4:4 F4:4 A4:4 G4:4 E4:4 E4:4 F4:4 G4:4 F4:4 E4:4 D4:4 E4:4 F4:8",
            bass: "D3:4 D3:4 Bb2:4 Bb2:4 F2:4 F2:4 C3:4 C3:4 G2:4 G2:4 F2:4 F2:4 G2:4 A2:4 D3:8",
            drums: "k-h-s-h-",
            bars: 8,
        },
        // Calm story theme.
        Track::Story => Piece {
            bpm: 76.0,
            melody: "E5:3 D5 C5:2 G4:2 A4:3 B4 C5:4 D5:3 C5 B4:2 G4:2 E4:6 - - \
                     E5:3 F5 G5:2 E5:2 D5:3 C5 A4:4 B4:3 C5 D5:2 B4:2 C5:8",
            harmony: "C4:8 F4:8 G4:8 C4:8 C4:8 F4:8 G4:8 C4:8",
            bass: "C3:8 F2:8 G2:8 A2:8 C3:8 D3:8 G2:8 C3:8",
            drums: "",
            bars: 8,
        },
        // Player phase march.
        Track::Player => Piece {
            bpm: 120.0,
            melody: "G4 G4 C5 D5 E5:2 D5 C5 D5:2 B4 G4 A4:2 B4:2 C5 C5 E5 F5 G5:2 F5 E5 D5:4 - - D5 E5 \
                     F5:2 E5 D5 E5:2 C5 A4 B4:2 G4 B4 D5:2 F5 E5 E5:2 D5 C5 B4:2 G4:2 C5:6 - -",
            harmony: "E4:4 F4:4 D4:4 G4:4 E4:4 A4:4 F4:4 G4:4 A4:4 C4:4 B3:4 D4:4 C4:4 G4:4 E4:8",
            bass: "C3 - G2 - C3 - G2 - F2 - C3 - G2 - D3 - C3 - G2 - A2 - E2 - F2 - C3 - G2 - B2 - \
                   F2 - C3 - A2 - E2 - G2 - D3 - G2 - B2 - C3 - G2 - F2 - G2 - C3 - G2 - C3:2",
            drums: "k-h-s-hk",
            bars: 8,
        },
        // Tense enemy phase in A minor.
        Track::Enemy => Piece {
            bpm: 112.0,
            melody: "A4 - A4 C5 B4 - G#4 - A4:2 E4:2 - - - - F4 - F4 A4 G#4 - E4 - B4:4 - - - - \
                     A4 - C5 E5 D5 - B4 - C5:2 A4:2 - - - - F4 G#4 B4 D5 C5 B4 G#4 E4 A4:8",
            harmony: "",
            bass: "A2 A2 A3 A2 A2 A2 A3 A2 A2 A2 A3 A2 A2 A2 A3 A2 F2 F2 F3 F2 E2 E2 E3 E2 E2 E2 E3 E2 E2 E2 E3 E2 \
                   A2 A2 A3 A2 G2 G2 G3 G2 F2 F2 F3 F2 F2 F2 F3 F2 D2 D2 D3 D2 E2 E2 E3 E2 A2 A2 A3 A2 A2:4",
            drums: "k-s-k-s-",
            bars: 8,
        },
        // Driving boss theme.
        Track::Boss => Piece {
            bpm: 140.0,
            melody: "E5 E5 D5 E5 G5:2 F#5 E5 D5:2 B4:2 E5:4 E5 E5 D5 E5 A5:2 G5 F#5 G5:4 - - F#5 E5 \
                     D5:2 E5 F#5 G5:2 F#5 E5 D5:2 B4 A4 B4:4 C5:2 D5:2 E5:2 F#5:2 G5:2 F#5:2 E5:4",
            harmony: "B4:4 B4:4 G4:4 B4:4 C5:4 D5:4 B4:4 A4:4 A4:4 B4:4 F#4:4 G4:4 A4:4 B4:4 G4:4 B4:4",
            bass: "E2 E3 E2 E3 E2 E3 E2 E3 G2 G3 G2 G3 B2 B3 B2 B3 C3 C4 C3 C4 D3 D4 D3 D4 E2 E3 E2 E3 E2 E3 E2 E3 \
                   B2 B3 B2 B3 B2 B3 B2 B3 G2 G3 G2 G3 A2 A3 A2 A3 C3 C4 C3 C4 D3 D4 D3 D4 E2 E3 E2 E3 E2 E3 E2 E3",
            drums: "kshskshs",
            bars: 8,
        },
        // Short victory fanfare (not looped).
        Track::Victory => Piece {
            bpm: 132.0,
            melody: "C5 E5 G5 C6:3 G5 A5 B5:2 C6:6 - -",
            harmony: "G4 C5 E5 G5:3 E5 F5 G5:2 E5:6 - -",
            bass: "C3:4 G2:2 F2 G2 C3:6 - -",
            drums: "k-s-k-s-",
            bars: 2,
        },
    }
}

fn effect(e: Sfx) -> Vec<u8> {
    let mut s = Synth::new((RATE as f32 * 0.9) as usize);
    match e {
        Sfx::Select => s.note(0.0, 0.06, 880.0, Wave::Square(0.5), 0.25, 400.0, 20.0),
        Sfx::Back => s.note(0.0, 0.07, 520.0, Wave::Square(0.5), 0.25, -800.0, 20.0),
        Sfx::Hit => {
            s.note(0.0, 0.12, 0.0, Wave::Noise, 0.6, 0.0, 25.0);
            s.note(0.0, 0.15, 180.0, Wave::Square(0.5), 0.35, -600.0, 15.0);
        }
        Sfx::Crit => {
            s.note(0.0, 0.08, 1200.0, Wave::Square(0.25), 0.3, 3000.0, 5.0);
            s.note(0.07, 0.25, 0.0, Wave::Noise, 0.8, 0.0, 10.0);
            s.note(0.07, 0.3, 140.0, Wave::Square(0.5), 0.45, -300.0, 8.0);
        }
        Sfx::Miss => s.note(0.0, 0.18, 0.0, Wave::Noise, 0.25, 0.0, 12.0),
        Sfx::Heal => {
            for (i, f) in [523.0, 659.0, 784.0, 1046.0].iter().enumerate() {
                s.note(i as f32 * 0.07, 0.2, *f, Wave::Triangle, 0.4, 0.0, 6.0);
            }
        }
        Sfx::LevelUp => {
            for (i, f) in [392.0, 523.0, 659.0, 784.0, 1046.0].iter().enumerate() {
                s.note(i as f32 * 0.08, 0.25, *f, Wave::Square(0.25), 0.3, 0.0, 4.0);
            }
            s.note(0.4, 0.4, 1046.0, Wave::Triangle, 0.3, 0.0, 3.0);
        }
        Sfx::Death => {
            s.note(0.0, 0.6, 400.0, Wave::Square(0.5), 0.3, -500.0, 3.0);
            s.note(0.0, 0.5, 0.0, Wave::Noise, 0.2, 0.0, 6.0);
        }
        Sfx::Chest => {
            s.note(0.0, 0.05, 200.0, Wave::Noise, 0.3, 0.0, 30.0);
            for (i, f) in [784.0, 988.0, 1175.0, 1568.0].iter().enumerate() {
                s.note(0.08 + i as f32 * 0.06, 0.3, *f, Wave::Triangle, 0.35, 0.0, 5.0);
            }
        }
        Sfx::Door => {
            s.note(0.0, 0.05, 0.0, Wave::Noise, 0.4, 0.0, 40.0);
            s.note(0.06, 0.35, 90.0, Wave::Square(0.5), 0.3, -40.0, 5.0);
        }
        Sfx::Steal => {
            s.note(0.0, 0.08, 600.0, Wave::Square(0.25), 0.25, 2000.0, 10.0);
            s.note(0.1, 0.08, 900.0, Wave::Square(0.25), 0.25, 2000.0, 10.0);
        }
        Sfx::Achievement => {
            for (i, f) in [659.0, 784.0, 988.0, 1318.0].iter().enumerate() {
                s.note(i as f32 * 0.1, 0.35, *f, Wave::Square(0.5), 0.25, 0.0, 3.0);
            }
        }
    }
    s.wav()
}

#[derive(Default)]
struct Bank {
    tracks: HashMap<Track, Sound>,
    sfx: HashMap<Sfx, Sound>,
    current: Option<Track>,
    music_on: bool,
    sfx_on: bool,
}

thread_local! {
    static BANK: RefCell<Bank> = RefCell::new(Bank::default());
}

/// Synthesises and loads every sound. Call once at startup.
pub async fn init(music_on: bool, sfx_on: bool) {
    let mut tracks = HashMap::new();
    for t in [Track::Title, Track::Story, Track::Player, Track::Enemy, Track::Boss, Track::Victory] {
        if let Ok(s) = load_sound_from_bytes(&render(&track(t))).await {
            tracks.insert(t, s);
        }
    }
    let mut sfx = HashMap::new();
    for e in [Sfx::Select, Sfx::Back, Sfx::Hit, Sfx::Crit, Sfx::Miss, Sfx::Heal, Sfx::LevelUp, Sfx::Death, Sfx::Chest, Sfx::Door, Sfx::Steal, Sfx::Achievement] {
        if let Ok(s) = load_sound_from_bytes(&effect(e)).await {
            sfx.insert(e, s);
        }
    }
    BANK.with(|b| {
        let mut b = b.borrow_mut();
        b.tracks = tracks;
        b.sfx = sfx;
        b.music_on = music_on;
        b.sfx_on = sfx_on;
    });
}

pub fn sfx(e: Sfx) {
    BANK.with(|b| {
        let b = b.borrow();
        if b.sfx_on {
            if let Some(s) = b.sfx.get(&e) {
                play_sound(s, PlaySoundParams { looped: false, volume: 0.6 });
            }
        }
    });
}

/// Switches background music (no-op if already playing `t`).
pub fn music(t: Track) {
    BANK.with(|b| {
        let mut b = b.borrow_mut();
        if b.current == Some(t) {
            return;
        }
        if let Some(cur) = b.current.and_then(|c| b.tracks.get(&c)) {
            stop_sound(cur);
        }
        b.current = Some(t);
        if b.music_on {
            if let Some(s) = b.tracks.get(&t) {
                play_sound(s, PlaySoundParams { looped: t != Track::Victory, volume: 0.45 });
            }
        }
    });
}

pub fn set_enabled(music_on: bool, sfx_on: bool) {
    BANK.with(|b| {
        let mut b = b.borrow_mut();
        let was = b.music_on;
        b.music_on = music_on;
        b.sfx_on = sfx_on;
        if let Some(t) = b.current {
            if let Some(s) = b.tracks.get(&t) {
                if was && !music_on {
                    stop_sound(s);
                } else if !was && music_on {
                    play_sound(s, PlaySoundParams { looped: t != Track::Victory, volume: 0.45 });
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_parse() {
        assert!((note_freq("A4").unwrap() - 440.0).abs() < 0.01);
        assert!((note_freq("A5").unwrap() - 880.0).abs() < 0.01);
        assert!((note_freq("C#4").unwrap() - note_freq("Db4").unwrap()).abs() < 0.01);
        assert!(note_freq("-").is_none());
    }

    #[test]
    fn everything_renders_valid_wav() {
        for t in [Track::Title, Track::Story, Track::Player, Track::Enemy, Track::Boss, Track::Victory] {
            let w = render(&track(t));
            assert_eq!(&w[0..4], b"RIFF");
            assert!(w.len() > 44 + RATE as usize, "{t:?} too short");
        }
        for e in [Sfx::Hit, Sfx::Crit, Sfx::LevelUp, Sfx::Achievement] {
            assert_eq!(&effect(e)[8..12], b"WAVE");
        }
    }
}
