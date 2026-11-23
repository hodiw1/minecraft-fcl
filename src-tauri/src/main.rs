#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;
mod constants;
mod error;
mod events;
mod java;
mod launch;
mod loaders;
mod manifest;
mod minecraft;
mod paths;
mod state;
mod sync;
mod updater;
mod util;
mod version;

fn main() {
    tauri::Builder::default()
        .manage(state::AppState::new())
        .setup(|_app| {
            updater::cleanup_old_executable();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_init_info,
            commands::save_settings,
            commands::bootstrap,
            commands::play
        ])
        .run(tauri::generate_context!())
        .expect("помилка запуску лаунчера");
}
