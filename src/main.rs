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
    let mut app = ui::App::new();
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
