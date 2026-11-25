#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;
mod constants;
mod error;
mod events;
mod i18n;
mod instance;
mod java;
mod launch;
mod loaders;
mod manifest;
mod minecraft;
mod paths;
mod project;
mod server_status;
mod state;
mod sync;
mod system;
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
            commands::check_launcher_update,
            commands::connect_project,
            commands::leave_project,
            commands::load_project,
            commands::instance_details,
            commands::server_status,
            commands::save_settings,
            commands::apply_graphics_preset,
            commands::pick_folder,
            commands::set_data_dir,
            commands::open_data_folder,
            commands::add_profile,
            commands::remove_profile,
            commands::set_active_profile,
            commands::save_skin,
            commands::clear_skin,
            commands::get_skin,
            commands::play,
            commands::repair_instance,
            commands::open_instance_folder,
            commands::list_installed,
            commands::delete_instance,
            commands::delete_all_instances,
            commands::crash_report
        ])
        .run(tauri::generate_context!())
        .expect("помилка запуску лаунчера");
}
