mod announcement;
mod auto_start;
mod crosshair;
mod delta_force;
mod display_filter;
mod downloader;
mod game_fps;
mod game_launcher;
mod game_ping;
mod gpu_rename;
mod hardware;
mod heart_rate;
mod hotkey;
mod network_optimize;
mod optimization;
mod overlay_panel;

mod sensor;
mod shader_cache;
mod sponsor;
mod startup_manager;
mod storage_clean;
mod thirdparty_tools;
mod tray;
mod utils;

use tauri::Manager;


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _: () = window.show().unwrap_or(());
                let _: () = window.set_focus().unwrap_or(());
                let _: () = window.unminimize().unwrap_or(());
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
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            } else {
                // Release 模式：日志写入文件，便于排查开机自启失败问题
                // 日志路径：%LOCALAPPDATA%/NexBox/nexbox.log
                let log_dir = dirs::data_local_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join("NexBox");
                let _ = std::fs::create_dir_all(&log_dir);
                let log_path = log_dir.join("nexbox.log");
                if let Ok(log_file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_path)
                {
                    let _ = env_logger::Builder::from_env(
                        env_logger::Env::default().default_filter_or("info"),
                    )
                    .target(env_logger::Target::Pipe(Box::new(log_file)))
                    .try_init();
                }
                log::info!(
                    "NexBox v{} 启动 | exe: {:?} | cwd: {:?}",
                    env!("CARGO_PKG_VERSION"),
                    std::env::current_exe().ok(),
                    std::env::current_dir().ok(),
                );
            }
            sensor::start_sensor_process(app);
            utils::sys_info::check_and_send_statistics(app);

            // 预填显示器信息缓存，确保热键路径也能正确获取设备名
            display_filter::init();

            match tray::init_tray(app.handle()) {
                Ok(_) => log::info!("Tray initialized successfully"),
                Err(e) => log::error!("Failed to initialize tray: {}", e),
            }

            // Register default hotkeys (will be overridden by frontend if user changed them)
            let _ = hotkey::init_overlay(app.handle(), "Shift+F10");
            let _ = hotkey::init_crosshair(app.handle(), "Shift+F9");
            let _ = hotkey::init_filter(app.handle(), "Shift+F8");

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
        announcement::get_announcements,
        announcement::get_important_announcements,
        auto_start::set_nexbox_auto_start,
        auto_start::check_nexbox_auto_start,
        hardware::get_hardware,
        hardware::get_cpu_load,
        hardware::get_gpu_status,
        hardware::get_disk_status,
        hardware::is_nvidia_gpu,
        hardware::get_os_version,
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
        optimization::start_auto_clean,
        optimization::stop_auto_clean,
        optimization::get_auto_clean_config,
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
        optimization::enable_performance_tweaks,
        optimization::disable_performance_tweaks,
        optimization::remove_menu_delay,
        optimization::restore_menu_delay,
        optimization::disable_network_throttling,
        optimization::enable_network_throttling,
        optimization::disable_error_reporting,
        optimization::enable_error_reporting,
        optimization::disable_compatibility_assistant,
        optimization::enable_compatibility_assistant,
        optimization::disable_print_service,
        optimization::enable_print_service,
        optimization::disable_fax_service,
        optimization::enable_fax_service,
        optimization::disable_sticky_keys,
        optimization::enable_sticky_keys,
        optimization::disable_smart_screen,
        optimization::enable_smart_screen,
        optimization::disable_system_restore,
        optimization::enable_system_restore,
        optimization::disable_superfetch,
        optimization::enable_superfetch,
        optimization::disable_hibernate,
        optimization::enable_hibernate,
        optimization::disable_ntfs_timestamp,
        optimization::enable_ntfs_timestamp,
        optimization::disable_telemetry_tasks,
        optimization::enable_telemetry_tasks,
        optimization::disable_media_player_sharing,
        optimization::enable_media_player_sharing,
        optimization::disable_home_group,
        optimization::enable_home_group,
        optimization::disable_smb1,
        optimization::enable_smb1,
        optimization::disable_smb2,
        optimization::enable_smb2,
        optimization::disable_office_telemetry,
        optimization::enable_office_telemetry,
        optimization::disable_firefox_telemetry,
        optimization::enable_firefox_telemetry,
        optimization::disable_chrome_telemetry,
        optimization::enable_chrome_telemetry,
        optimization::disable_nvidia_telemetry,
        optimization::enable_nvidia_telemetry,
        optimization::disable_vs_telemetry,
        optimization::enable_vs_telemetry,
        optimization::batch_enable_tweaks,
        optimization::disable_telemetry_services,
        optimization::enable_telemetry_services,
        optimization::disable_cortana,
        optimization::enable_cortana,
        optimization::disable_news_interests,
        optimization::enable_news_interests,
        optimization::disable_start_menu_ads,
        optimization::enable_start_menu_ads,
        optimization::disable_edge_telemetry,
        optimization::enable_edge_telemetry,
        optimization::disable_edge_discover_bar,
        optimization::enable_edge_discover_bar,
        optimization::optimize_process_count,
        optimization::restore_process_count,
        optimization::disable_store_search_app,
        optimization::enable_store_search_app,
        optimization::disable_store_promotions,
        optimization::enable_store_promotions,
        optimization::disable_store_auto_update,
        optimization::enable_store_auto_update,
        optimization::disable_spotlight_lock,
        optimization::enable_spotlight_lock,
        optimization::disable_my_people,
        optimization::enable_my_people,
        optimization::disable_tpm_check,
        optimization::enable_tpm_check,
        optimization::disable_sensor_services,
        optimization::enable_sensor_services,
        optimization::remove_cast_to_device,
        optimization::add_cast_to_device,
        optimization::disable_vbs,
        optimization::enable_vbs,
        optimization::disable_modern_standby,
        optimization::enable_modern_standby,
        optimization::enable_gaming_mode,
        optimization::disable_gaming_mode,
        optimization::disable_xbox_live,
        optimization::enable_xbox_live,
        optimization::disable_game_bar,
        optimization::enable_game_bar,
        optimization::disable_windows_ink,
        optimization::enable_windows_ink,
        optimization::disable_spelling_typing,
        optimization::enable_spelling_typing,
        optimization::disable_cloud_clipboard,
        optimization::enable_cloud_clipboard,
        optimization::disable_app_launch_tracking,
        optimization::enable_app_launch_tracking,
        optimization::disable_advertising_id,
        optimization::enable_advertising_id,
        optimization::disable_file_system_access,
        optimization::enable_file_system_access,
        optimization::disable_documents_access,
        optimization::enable_documents_access,
        optimization::disable_calendar_access,
        optimization::enable_calendar_access,
        optimization::disable_contacts_access,
        optimization::enable_contacts_access,
        optimization::disable_language_tracking,
        optimization::enable_language_tracking,
        optimization::disable_welcome_experience,
        optimization::enable_welcome_experience,
        optimization::disable_feedback_frequency,
        optimization::enable_feedback_frequency,
        optimization::disable_diagnostic_data,
        optimization::enable_diagnostic_data,
        optimization::disable_writing_habits,
        optimization::enable_writing_habits,
        optimization::disable_ceip,
        optimization::enable_ceip,
        optimization::disable_trk_wks,
        optimization::enable_trk_wks,
        optimization::disable_auto_maintenance,
        optimization::enable_auto_maintenance,
        optimization::enable_large_sys_cache,
        optimization::disable_large_sys_cache,
        optimization::disable_spectre_patch,
        optimization::enable_spectre_patch,
        optimization::disable_auto_debug,
        optimization::enable_auto_debug,
        optimization::disable_crash_dump,
        optimization::enable_crash_dump,
        optimization::disable_audit_log,
        optimization::enable_audit_log,
        optimization::disable_wfp_diag,
        optimization::enable_wfp_diag,
        optimization::disable_address_book_collect,
        optimization::enable_address_book_collect,
        optimization::disable_typing_collection,
        optimization::enable_typing_collection,
        optimization::disable_silent_app_install,
        optimization::enable_silent_app_install,
        optimization::disable_wifi_hotspots,
        optimization::enable_wifi_hotspots,
        optimization::disable_typing_insights,
        optimization::enable_typing_insights,
        optimization::disable_preinstalled_apps,
        optimization::enable_preinstalled_apps,
        optimization::disable_dotnet_telemetry,
        optimization::enable_dotnet_telemetry,
        optimization::disable_pwsh_telemetry,
        optimization::enable_pwsh_telemetry,
        optimization::disable_wifi_sense,
        optimization::enable_wifi_sense,
        optimization::disable_step_recorder,
        optimization::enable_step_recorder,
        optimization::batch_disable_tweaks,
        optimization::check_all_tweak_states,
        optimization::delete_power_plan,
        network_optimize::set_tcp_congestion,
        network_optimize::restore_tcp_congestion,
        network_optimize::set_tcp_chimney_off,
        network_optimize::restore_tcp_chimney,
        network_optimize::set_nagle_optimization,
        network_optimize::restore_nagle_optimization,
        network_optimize::set_adapter_power_saving_off,
        network_optimize::restore_adapter_power_saving,
        network_optimize::set_dns_servers,
        network_optimize::restore_dns_servers,
        network_optimize::check_network_tweak_states,
        network_optimize::batch_network_enable,
        network_optimize::batch_network_disable,
        startup_manager::scan_startup_items,
        startup_manager::delete_startup_item,
        startup_manager::locate_startup_file,
        startup_manager::find_startup_key_in_registry,
        display_filter::get_displays,
        display_filter::set_active_display,
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
        overlay_panel::run_pawnio_setup,

        sensor::get_lhm_cpu_load,
        sensor::get_lhm_cpu_status,
        sensor::get_lhm_gpu_status,
        heart_rate::scan_ble_devices,
        heart_rate::connect_ble_device,
        heart_rate::disconnect_ble_device,
        heart_rate::get_heart_rate_data,
        heart_rate::get_ble_connection_status,
        heart_rate::start_advert_hr_listen,
        heart_rate::stop_advert_hr_listen,

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
        crosshair::get_crosshair_displays,

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
        gpu_rename::get_gpu_info,
        gpu_rename::get_gpu_options,
        gpu_rename::apply_gpu_rename,
        gpu_rename::restore_gpu_name,
            sponsor::get_sponsors,
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
    ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        match event {
            tauri::RunEvent::Exit => {
                sensor::stop_sensor_process(app_handle);
                hardware::cleanup_hardware_cache();
                display_filter::cleanup();
                overlay_panel::cleanup();
                crosshair::cleanup();
                heart_rate::cleanup();
                tray::cleanup();
                hotkey::cleanup(app_handle);
            }
            _ => {}
        }
    });
}
