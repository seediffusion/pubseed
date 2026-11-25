//! Pubseed: an accessible desktop client for AudioPub.

// No console window behind the app in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod acc;
mod api;
mod config;
mod devalue;
mod player;
mod recorder;
mod speech;
mod stretch;
mod ui;

use wxdragon::appearance::Appearance;

fn main() {
    // Pubseed runs once. Starting it again brings the running copy forward.
    match doorbell::ring("pubseed", &[] as &[&str]) {
        Ok(doorbell::Ring::Delivered) => return,
        Ok(doorbell::Ring::First(door)) => {
            door.listen(|_| wx_utils::post_to_ui(ui::bring_to_front));
        }
        // If it can't tell, starting a second copy is better than none.
        Err(_) => {}
    }
    let cfg = config::Config::load();
    let _ = wxdragon::main(move |_| {
        // Dark mode has to be chosen before any window exists.
        wxdragon::app::set_appearance(match cfg.appearance {
            config::Appearance::System => Appearance::System,
            config::Appearance::Light => Appearance::Light,
            config::Appearance::Dark => Appearance::Dark,
        });
        ui::build(cfg);
    });
}
