mod announcement;
mod crosshair;
mod delta_force;
mod display_filter;
mod downloader;
mod fps_tracker;
mod gpu_rename;
mod hardware;
mod music;
mod optimization;
mod overlay_panel;

mod sensor;
mod shader_cache;
mod thirdparty_tools;
mod tray;
mod utils;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.unminimize();
            }
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_os::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            sensor::start_sensor_process(app);

            match tray::init_tray(app.handle()) {
                Ok(_) => log::info!("Tray initialized successfully"),
                Err(e) => log::error!("Failed to initialize tray: {}", e),
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
        announcement::get_announcements,
        announcement::get_important_announcements,
        hardware::get_hardware,
        hardware::get_cpu_load,
        hardware::get_gpu_status,
        hardware::get_disk_status,
        downloader::download_file,
        downloader::open_installer,
        downloader::download_update,
        downloader::install_update,
        downloader::delete_download_file,
        optimization::optimize_memory,
        optimization::get_memory_status,
        optimization::kill_wallpaper_engine,
        optimization::flush_dns,
        optimization::clean_temp_files,
        optimization::optimize_privacy_services,
        optimization::optimize_ace_processes,
        optimization::set_high_performance_power_plan,
        optimization::get_memory_limit_options,
        optimization::get_memory_limit_status,
        optimization::set_memory_limit,
        optimization::restore_memory_limit,
        optimization::get_detailed_memory_status,
        optimization::clean_standby_memory,
        optimization::trim_system_working_set,
        optimization::boost_delta_force_priority,
        optimization::optimize_all_game_processes,
        display_filter::get_filter_settings,
        display_filter::set_filter_settings,
        display_filter::enable_filter,
        display_filter::disable_filter,
        display_filter::toggle_filter,
        display_filter::get_filter_presets,
        display_filter::apply_preset,
        display_filter::get_custom_filter_settings,
        display_filter::save_custom_filter_settings,
        thirdparty_tools::get_thirdparty_tools,
        thirdparty_tools::get_thirdparty_tools_with_status,
        thirdparty_tools::check_tool_installed,
        thirdparty_tools::get_tool_install_path,
        thirdparty_tools::get_tool_download_path,
        thirdparty_tools::run_tool,
        thirdparty_tools::download_tool,
        thirdparty_tools::open_tool_installer,
        overlay_panel::start_overlay_panel,
        overlay_panel::stop_overlay_panel,
        overlay_panel::get_overlay_panel_status,
        overlay_panel::get_overlay_hardware_data,
        overlay_panel::update_overlay_settings,
        fps_tracker::get_debug_info,
        crosshair::toggle_crosshair,
        crosshair::get_crosshair_status,
        crosshair::update_crosshair_settings,

        delta_force::get_delta_passwords,
        delta_force::get_weapon_codes,
        delta_force::get_dlss_model_presets,
        delta_force::apply_dlss_model_preset,
        delta_force::get_delta_maps,
        music::get_music_files,
        gpu_rename::get_gpu_info,
        gpu_rename::get_gpu_options,
        gpu_rename::apply_gpu_rename,
        gpu_rename::restore_gpu_name,
            shader_cache::scan_shader_caches,
            shader_cache::clean_shader_cache,
            utils::sys_info::get_system_locale,
            tray::minimize_to_tray,
            tray::show_window,
            tray::get_close_behavior,
            tray::set_close_behavior,
            tray::get_dont_ask_again,
            tray::set_dont_ask_again,
    ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        match event {
            tauri::RunEvent::ExitRequested { api, .. } => {
                api.prevent_exit();
                sensor::stop_sensor_process(app_handle);
                hardware::cleanup_hardware_cache();
                display_filter::cleanup();
                overlay_panel::cleanup();
                crosshair::cleanup();
                tray::cleanup();
                std::process::exit(0);
            }
            tauri::RunEvent::Exit => {
                sensor::stop_sensor_process(app_handle);
                hardware::cleanup_hardware_cache();
                display_filter::cleanup();
                overlay_panel::cleanup();
                crosshair::cleanup();
                tray::cleanup();
            }
            _ => {}
        }
    });
}
