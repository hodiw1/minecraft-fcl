#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod changelog;
mod commands;
mod config;
mod constants;
// CurseForge вмикається прапорцем `constants::ENABLE_CURSEFORGE` (потрібен API-ключ гравця).
#[allow(dead_code)]
mod curseforge;
mod custom;
mod danger;
mod error;
mod events;
mod i18n;
mod import_pack;
mod instance;
mod java;
mod launch;
mod loaders;
mod manifest;
mod minecraft;
mod mirrors;
mod modpack;
mod modrinth;
mod mods;
mod pack_debugger;
mod paths;
mod project;
mod server_status;
mod state;
mod sync;
mod system;
mod updater;
mod util;
mod version;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .manage(state::AppState::new())
        .setup(|app| {
            updater::cleanup_old_executable();
            // Дзеркала: спершу кеш із диска (миттєво, без мережі), потім — свіжий конфіг у фоні.
            if let Ok(root) = paths::Paths::app_root(app.handle()) {
                mirrors::init_from_disk(&root);
                let client = app.state::<state::AppState>().client.clone();
                tauri::async_runtime::spawn(async move {
                    mirrors::refresh(&client, &root).await;
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_init_info,
            commands::onboarding_profile,
            commands::finish_onboarding,
            commands::check_launcher_update,
            commands::sync_all,
            commands::install_launcher_update,
            commands::connect_project,
            commands::leave_project,
            commands::list_projects,
            commands::load_changelog,
            commands::open_github,
            commands::load_project,
            commands::instance_details,
            commands::server_status,
            commands::save_settings,
            commands::mirrors_info,
            commands::apply_graphics_preset,
            commands::pick_folder,
            commands::set_data_dir,
            commands::open_data_folder,
            commands::add_profile,
            commands::remove_profile,
            commands::set_active_profile,
            commands::save_skin,
            commands::pick_skin,
            commands::clear_skin,
            commands::get_skin,
            commands::play,
            commands::stop_instance,
            commands::repair_instance,
            commands::debug_instance,
            commands::pick_zip,
            commands::import_local_pack,
            commands::open_instance_folder,
            commands::list_installed,
            commands::delete_instance,
            commands::instance_disk_usage,
            commands::delete_all_instances,
            commands::crash_report,
            commands::minecraft_versions,
            commands::loader_versions,
            commands::create_custom_instance,
            commands::mod_sources,
            commands::mod_categories,
            commands::search_mods,
            commands::mod_versions,
            commands::list_mods,
            commands::install_mod,
            commands::set_mod_enabled,
            commands::remove_mod,
            commands::open_mods_folder,
            commands::content_status,
            commands::install_shader_mod,
            commands::search_modpacks,
            commands::modpack_versions,
            commands::install_modpack,
            commands::reset_launcher_data,
            commands::uninstall_launcher
        ])
        .run(tauri::generate_context!())
        .expect("помилка запуску лаунчера");
}
