//! Emberfall Tactics — an original retro tactical RPG.
//!
//! `logic` holds the rules (pure Rust, unit-tested); `ui` renders with macroquad.

#[cfg(not(target_os = "android"))]
mod debug_script;
mod logic;
mod ui;

use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: "Emberfall Tactics".to_owned(),
        window_width: 1280,
        window_height: 720,
        high_dpi: true,
        window_resizable: true,
        fullscreen: cfg!(target_os = "android"),
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    // We read touches directly; don't also synthesize mouse clicks from them.
    simulate_mouse_with_touch(false);
    // Show something while the soundtrack is synthesised (a few seconds on the web).
    for _ in 0..2 {
        clear_background(Color::from_rgba(16, 16, 30, 255));
        let msg = "Emberfall - composing music...";
        let size = (screen_height() / 18.0).max(20.0);
        let w = measure_text(msg, None, size as u16, 1.0).width;
        draw_text(msg, (screen_width() - w) / 2.0, screen_height() / 2.0, size, Color::from_rgba(236, 194, 76, 255));
        next_frame().await;
    }
    let profile = ui::storage::load_profile();
    let (music, sfx) = ui::App::music_enabled(&profile);
    ui::audio::init(music, sfx).await;
    let mut app = ui::App::new(profile);
    #[cfg(not(target_os = "android"))]
    let mut script = debug_script::Script::from_env();
    loop {
        #[cfg(not(target_os = "android"))]
        if let Some(s) = script.as_mut() {
            if !s.before() {
                break;
            }
        }
        app.frame();
        #[cfg(not(target_os = "android"))]
        if let Some(s) = script.as_ref() {
            s.after();
        }
        next_frame().await;
    }
}
