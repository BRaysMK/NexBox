mod announcement;
mod crosshair;
mod delta_force;
mod display_filter;
mod downloader;
mod game_fps;
mod game_launcher;
mod game_ping;
mod gpu_rename;
mod hardware;
mod hotkey;
mod music;
mod optimization;
mod overlay_panel;

mod sensor;
mod shader_cache;
mod storage_clean;
mod thirdparty_tools;
mod tray;
mod utils;
#[allow(dead_code)]
mod mctier_modules;

use tauri::{Emitter, Manager};
use std::sync::Arc;
use tokio::sync::Mutex;
use mctier_modules::app_core::AppCore;
use mctier_modules::tauri_commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // === MCTier: 初始化 AppCore ===
    let runtime = tokio::runtime::Runtime::new().expect("无法创建 Tokio 运行时");
    let app_core = runtime.block_on(async {
        match AppCore::new().await {
            Ok(core) => {
                log::info!("[MCTier] 应用核心初始化成功");
                if let Err(e) = core.start().await { log::error!("[MCTier] 应用启动失败: {}", e); }
                core
            }
            Err(e) => { log::error!("[MCTier] 应用核心初始化失败: {}", e); panic!("无法初始化应用核心: {}", e); }
        }
    });
    let app_state = AppState { core: Arc::new(Mutex::new(app_core)) };
    // === MCTier 初始化结束 ===

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
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    use tauri_plugin_global_shortcut::ShortcutState;
                    if event.state == ShortcutState::Pressed {
                        if shortcut.id() == hotkey::get_overlay_shortcut_id() {
                            let _ = overlay_panel::toggle_overlay(app);
                        } else if shortcut.id() == hotkey::get_crosshair_shortcut_id() {
                            let _ = crosshair::toggle_crosshair_sync(app);
                        } else if shortcut.id() == hotkey::get_filter_shortcut_id() {
                            let _ = display_filter::toggle_filter_sync(app);
                        }
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(app_state)
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            sensor::start_sensor_process(app);
            utils::sys_info::check_and_send_statistics(app);

            match tray::init_tray(app.handle()) {
                Ok(_) => log::info!("Tray initialized successfully"),
                Err(e) => log::error!("Failed to initialize tray: {}", e),
            }

            // Register default hotkeys (will be overridden by frontend if user changed them)
            let _ = hotkey::init_overlay(app.handle(), "Shift+F10");
            let _ = hotkey::init_crosshair(app.handle(), "Shift+F9");
            let _ = hotkey::init_filter(app.handle(), "Shift+F8");

            // === MCTier: 窗口初始化 ===
            let app_handle = app.handle().clone();
            if let Some(state) = app.try_state::<AppState>() {
                let core = Arc::clone(&state.core);
                
                // 设置应用句柄到 AppCore
                tauri::async_runtime::block_on(async move {
                    core.lock().await.set_app_handle(app_handle).await;
                    log::info!("[MCTier] 应用句柄已设置到 AppCore");
                });
            }
            // === MCTier 初始化结束 ===

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
        optimization::boost_delta_force_affinity,
        optimization::limit_ace_priority,
        optimization::restrict_ace_affinity,
        optimization::set_ace_efficiency_mode,
        optimization::optimize_all_game_processes,
        optimization::get_builtin_power_plans,
        optimization::get_system_power_plans,
        optimization::get_active_power_plan,
        optimization::import_power_plan,
        optimization::activate_power_plan,
        optimization::import_and_activate_power_plan,
        optimization::delete_power_plan,
        display_filter::get_filter_settings,
        display_filter::set_filter_settings,
        display_filter::enable_filter,
        display_filter::disable_filter,
        display_filter::toggle_filter,
        display_filter::get_filter_presets,
        display_filter::apply_preset,
        display_filter::get_custom_filter_settings,
        display_filter::save_custom_filter_settings,
        display_filter::select_icc_file,
        display_filter::import_icc_profile,
        display_filter::get_icc_presets,
        display_filter::apply_icc_preset,
        display_filter::delete_icc_preset,
        thirdparty_tools::get_thirdparty_tools,
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
        overlay_panel::toggle_overlay_panel,
        overlay_panel::get_misans_font_path,
        overlay_panel::set_overlay_drag_mode,
        overlay_panel::get_overlay_current_settings,
        overlay_panel::check_drag_mode_status,
        overlay_panel::reset_overlay_position,
        game_ping::get_current_ping,
        hotkey::get_overlay_hotkey,
        hotkey::set_overlay_hotkey,
        hotkey::get_crosshair_hotkey,
        hotkey::set_crosshair_hotkey,
        hotkey::get_filter_hotkey,
        hotkey::set_filter_hotkey,
        crosshair::toggle_crosshair,
        crosshair::get_crosshair_status,
        crosshair::update_crosshair_settings,

        delta_force::get_delta_passwords,
        delta_force::get_weapon_codes,
        delta_force::get_dlss_model_presets,
        delta_force::apply_dlss_model_preset,
        delta_force::get_dlss_preset_status,
        delta_force::get_delta_maps,
        delta_force::toggle_dlss_indicator,
        delta_force::toggle_dlss_lock,
        delta_force::get_dlss_settings_status,
        game_launcher::launch_game,
        game_launcher::search_delta_force_launcher,
        game_launcher::get_default_delta_force_game,
        game_launcher::select_exe_file,
        music::get_music_files,
        gpu_rename::get_gpu_info,
        gpu_rename::get_gpu_options,
        gpu_rename::apply_gpu_rename,
        gpu_rename::restore_gpu_name,
            shader_cache::scan_shader_caches,
            shader_cache::clean_shader_cache,
            storage_clean::scan_storage_items,
            storage_clean::clean_storage_items,
            storage_clean::empty_recycle_bin_cmd,
            utils::sys_info::get_system_locale,
            tray::minimize_to_tray,
            tray::show_window,
            tray::get_close_behavior,
            tray::set_close_behavior,
            tray::get_dont_ask_again,
            tray::set_dont_ask_again,
            // === MCTier 命令 ===
            mctier_modules::tauri_commands::create_lobby,
            mctier_modules::tauri_commands::join_lobby,
            mctier_modules::tauri_commands::leave_lobby,
            mctier_modules::tauri_commands::force_exit_app,
            mctier_modules::tauri_commands::toggle_mic,
            mctier_modules::tauri_commands::mute_player,
            mctier_modules::tauri_commands::mute_all,
            mctier_modules::tauri_commands::get_config,
            mctier_modules::tauri_commands::update_config,
            mctier_modules::tauri_commands::save_opacity,
            mctier_modules::tauri_commands::get_audio_devices,
            mctier_modules::tauri_commands::get_app_state,
            mctier_modules::tauri_commands::get_current_lobby,
            mctier_modules::tauri_commands::get_players,
            mctier_modules::tauri_commands::get_mic_status,
            mctier_modules::tauri_commands::get_global_mute_status,
            mctier_modules::tauri_commands::is_player_muted,
            mctier_modules::tauri_commands::get_network_status,
            mctier_modules::tauri_commands::get_virtual_ip,
            mctier_modules::tauri_commands::set_always_on_top,
            mctier_modules::tauri_commands::toggle_mini_mode,
            mctier_modules::tauri_commands::set_window_opacity,
            mctier_modules::tauri_commands::send_signaling_message,
            mctier_modules::tauri_commands::broadcast_status_update,
            mctier_modules::tauri_commands::send_heartbeat,
            mctier_modules::tauri_commands::force_stop_easytier,
            mctier_modules::tauri_commands::check_virtual_adapter,
            mctier_modules::tauri_commands::check_firewall_rules,
            mctier_modules::tauri_commands::ping_virtual_ip,
            mctier_modules::tauri_commands::check_udp_port,
            mctier_modules::tauri_commands::save_window_position,
            mctier_modules::tauri_commands::exit_app,
            mctier_modules::tauri_commands::add_player_domain,
            mctier_modules::tauri_commands::remove_player_domain,
            mctier_modules::tauri_commands::get_folder_name,
            mctier_modules::tauri_commands::get_folder_info,
            mctier_modules::tauri_commands::list_directory_files,
            mctier_modules::tauri_commands::read_file_bytes,
            mctier_modules::tauri_commands::write_file_bytes,
            mctier_modules::tauri_commands::select_folder,
            mctier_modules::tauri_commands::select_file,
            mctier_modules::tauri_commands::select_save_location,
            mctier_modules::tauri_commands::save_file,
            mctier_modules::tauri_commands::save_chat_image,
            mctier_modules::tauri_commands::read_file,
            mctier_modules::tauri_commands::delete_file,
            mctier_modules::tauri_commands::extract_zip,
            mctier_modules::tauri_commands::open_file_location,
            mctier_modules::tauri_commands::open_folder,
            mctier_modules::tauri_commands::start_file_server,
            mctier_modules::tauri_commands::stop_file_server,
            mctier_modules::tauri_commands::check_file_server_status,
            mctier_modules::tauri_commands::add_shared_folder,
            mctier_modules::tauri_commands::remove_shared_folder,
            mctier_modules::tauri_commands::get_local_shares,
            mctier_modules::tauri_commands::cleanup_expired_shares,
            mctier_modules::tauri_commands::get_remote_shares,
            mctier_modules::tauri_commands::get_remote_files,
            mctier_modules::tauri_commands::verify_share_password,
            mctier_modules::tauri_commands::get_download_url,
            mctier_modules::tauri_commands::diagnose_file_share_connection,
            mctier_modules::tauri_commands::send_p2p_chat_message,
            mctier_modules::tauri_commands::get_p2p_chat_messages,
            mctier_modules::tauri_commands::clear_p2p_chat_messages,
            mctier_modules::tauri_commands::open_screen_viewer_window,
            mctier_modules::tauri_commands::open_log_folder,
            mctier_modules::tauri_commands::open_log_file,
            mctier_modules::tauri_commands::get_log_file_path,
            mctier_modules::tauri_commands::save_settings,
            mctier_modules::tauri_commands::get_settings,
            mctier_modules::tauri_commands::set_auto_start,
            mctier_modules::tauri_commands::check_auto_start,
            mctier_modules::tauri_commands::reset_config_to_default,
            mctier_modules::tauri_commands::save_voice_volume,
            mctier_modules::tauri_commands::export_config,
            mctier_modules::tauri_commands::import_config,
            mctier_modules::tauri_commands::save_exit_node_advanced_config,
            mctier_modules::tauri_commands::get_exit_node_advanced_config,
            mctier_modules::easytier_advanced_commands::save_global_easytier_advanced_config,
            mctier_modules::easytier_advanced_commands::get_global_easytier_advanced_config,
            mctier_modules::easytier_advanced_commands::save_lobby_easytier_advanced_config,
            mctier_modules::easytier_advanced_commands::get_lobby_easytier_advanced_config,
            mctier_modules::easytier_advanced_commands::clear_lobby_easytier_advanced_config,
    ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        match event {
            tauri::RunEvent::ExitRequested { api, .. } => {
                api.prevent_exit();
                // 通知前端，由前端决定是否需要先退出联机服务再关闭
                // 前端收到事件后执行清理，最后调用 force_exit_app
                let _ = app_handle.emit("lobby-exit-requested", ());
            }
            tauri::RunEvent::Exit => {
                sensor::stop_sensor_process(app_handle);
                hardware::cleanup_hardware_cache();
                display_filter::cleanup();
                overlay_panel::cleanup();
                crosshair::cleanup();
                tray::cleanup();
                hotkey::cleanup(app_handle);
            }
            _ => {}
        }
    });
}
