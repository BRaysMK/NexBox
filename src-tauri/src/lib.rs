mod announcement;
mod audio_engine;
mod audio_eq;
mod auto_start;
mod music_api;
mod cpu_scheduler;
mod crosshair;
mod dattorro;
mod delta_force;
mod spectrum;
mod display_cache;
mod display_filter;
mod downloader;
mod game_fps;
mod game_launcher;
mod game_ping;
mod gpu_rename;
mod hardware;
mod hardware_report;

mod hotkey;
mod music;
mod network_optimize;
#[allow(dead_code, unused_imports)]
mod netease_lyrics;
mod nvapi;
mod optimization;
mod overlay_panel;
mod vertical_overlay;

mod sensor;
mod shader_cache;
mod sponsor;
mod startup_manager;
mod storage_clean;
mod thirdparty_tools;
mod tray;
mod utils;
mod video_bg;
mod wmi_query;
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
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // 初始化音乐 API 和音频代理
            let app_handle_for_music = app.handle().clone();
            music_api::audio_proxy::set_app_handle(app_handle_for_music.clone());
            tauri::async_runtime::spawn(async move {
                music_api::init_cookie_cache(&app_handle_for_music).await;
                match music_api::audio_proxy::start_audio_proxy().await {
                    Ok(port) => log::info!("[MusicAPI] audio proxy started on port {port}"),
                    Err(e) => log::error!("[MusicAPI] failed to start audio proxy: {e}"),
                }
            });

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
            overlay_panel::start_hardware_poller();
            hardware_report::start_recording();

            // 初始化 ACE 自动检测（读取持久化配置并启动后台任务）
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let _ = optimization::init_ace_auto_detect(app_handle).await;
            });

            // 启动时自动应用已保存的 CPU 调度规则
            let app_handle_for_rules = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                cpu_scheduler::apply_all_saved_rules(&app_handle_for_rules).await;
            });

            // Configure widget window (DWM clipping + close intercept)
            if let Some(widget_window) = app.get_webview_window("widget") {
                #[cfg(target_os = "windows")]
                {
                    use windows_sys::Win32::Graphics::Dwm::{
                        DwmSetWindowAttribute,
                        DWMWA_WINDOW_CORNER_PREFERENCE,
                        DWMWA_BORDER_COLOR,
                        DWMWCP_DONOTROUND,
                    };
                    use windows_sys::Win32::UI::WindowsAndMessaging::{
                        SetWindowLongPtrW,
                        GWL_STYLE,
                        WS_CAPTION,
                    };
                    use windows_sys::Win32::Foundation::HWND;

                    if let Ok(hwnd) = widget_window.hwnd() {
                        let hwnd_raw = hwnd.0 as HWND;
                        unsafe {
                            let current_style = windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd_raw, GWL_STYLE);
                            SetWindowLongPtrW(hwnd_raw, GWL_STYLE, current_style & !(WS_CAPTION as isize));

                            let border_color: u32 = 0xFFFFFFFE;
                            let _ = DwmSetWindowAttribute(
                                hwnd_raw,
                                DWMWA_BORDER_COLOR as u32,
                                &border_color as *const _ as *const _,
                                std::mem::size_of::<u32>() as u32,
                            );

                            let corner_preference = DWMWCP_DONOTROUND;
                            let _ = DwmSetWindowAttribute(
                                hwnd_raw,
                                DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                                &corner_preference as *const _ as *const _,
                                std::mem::size_of::<i32>() as u32,
                            );
                        }
                    }
                }

                // Intercept close → hide instead
                let w_clone = widget_window.clone();
                widget_window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = w_clone.hide();
                    }
                });
            }

            // Main window: intercept taskbar Close / Alt+F4 → hide instead of destroy
            if let Some(main_window) = app.get_webview_window("main") {
                let main_clone = main_window.clone();
                main_window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = main_clone.hide();
                    }
                });
            }

            // Tray menu: hide when losing focus (click outside), reset always-on-top
            if let Some(tray_menu) = app.get_webview_window("tray-menu") {
                let menu_clone = tray_menu.clone();
                tray_menu.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let _ = menu_clone.set_always_on_top(false);
                        let _ = menu_clone.hide();
                    }
                });
            }

            // 显示器信息改为前端按需加载 (get_displays 内部已有缓存逻辑)，
            // 不在启动阶段预填，避免阻塞 WebView 加载导致白屏。

            match tray::init_tray(app.handle()) {
                Ok(_) => log::info!("Tray initialized successfully"),
                Err(e) => log::error!("Failed to initialize tray: {}", e),
            }

            // 提前从持久化存储加载悬浮框设置，确保快捷键触发时使用已保存的配置而非默认值
            overlay_panel::try_load_persisted_settings(app.handle());

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
        hardware::get_disk_health_info,
        music::get_music_files,
        // === 音乐播放器 API ===
        music_api::music_search,
        music_api::music_song_url,
        music_api::music_login_qr_key,
        music_api::music_login_qr_create,
        music_api::music_login_qr_check,
        music_api::music_login_status,
        music_api::music_login_cookie,
        music_api::music_logout,
        music_api::music_user_playlist,
        music_api::music_playlist_tracks,
        music_api::music_playlist_tracks_range,
        music_api::music_playlist_info_with_track_ids,
        music_api::music_playlist_detail,
        music_api::music_likelist,
        music_api::music_like,
        music_api::music_playlist_subscribe,
        music_api::music_lyric,
        music_api::music_personalized,
        music_api::music_recommend_songs,
        music_api::music_artist_search,
        music_api::music_artist_songs,
        music_api::music_playlist_search,
        music_api::music_open_login_window,
        music_api::audio_proxy::cmd_get_proxy_port,
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
        optimization::boost_delta_force_affinity_with_mask,
        optimization::limit_ace_priority,
        optimization::restrict_ace_affinity,
        optimization::restrict_ace_affinity_with_mask,
        optimization::set_ace_efficiency_mode,
        optimization::optimize_all_game_processes,
        optimization::set_ace_auto_detect,
        optimization::get_ace_auto_detect_status,
        optimization::init_ace_auto_detect,
        optimization::get_builtin_power_plans,
        optimization::get_system_power_plans,
        optimization::get_active_power_plan,
        optimization::import_power_plan,
        optimization::activate_power_plan,
        optimization::import_and_activate_power_plan,
        optimization::apply_registry_tweak,
        optimization::restore_registry_tweak,
        optimization::batch_apply_registry_tweaks,
        optimization::batch_restore_registry_tweaks,
        optimization::disable_windows_update,
        optimization::enable_windows_update,
        optimization::check_windows_update_state,
        optimization::check_pause_update_state,
        optimization::delete_power_plan,
        optimization::get_peripheral_status,
        optimization::set_peripheral_settings,
        optimization::reset_peripheral_settings,
        optimization::restart_graphics_driver,
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
        display_filter::check_gamma_support,
        display_filter::get_filter_settings,
        display_filter::set_filter_settings,
        display_filter::enable_filter,
        display_filter::disable_filter,
        display_filter::toggle_filter,
        display_filter::get_filter_presets,
        display_filter::apply_preset,
        display_filter::get_custom_filter_settings,
        display_filter::save_custom_filter_settings,
        display_filter::export_custom_filter,
        display_filter::get_user_filter_presets,
        display_filter::save_user_filter_preset,
        display_filter::apply_user_filter_preset,
        display_filter::delete_user_filter_preset,
        display_filter::select_icc_file,
        display_filter::import_icc_profile,
        display_filter::get_icc_presets,
        display_filter::apply_icc_preset,
        display_filter::delete_icc_preset,
        display_filter::export_preset_as_icc,
        display_filter::restore_filter_state,
        // === EQ 调音命令 ===
        audio_eq::check_virtual_audio_driver,
        audio_eq::install_virtual_audio_driver,
        audio_eq::uninstall_virtual_audio_driver,
        audio_eq::start_eq_engine,
        audio_eq::stop_eq_engine,
        audio_eq::get_eq_engine_status,
        audio_eq::get_eq_presets,
        audio_eq::apply_eq_preset,
        audio_eq::import_eq_preset,
        audio_eq::delete_eq_preset,
        audio_eq::save_eq_preset,
        audio_eq::export_fac_file,
        audio_eq::get_audio_levels,
        audio_eq::get_spectrum,
        audio_eq::get_default_audio_device,
        audio_eq::update_eq_bands,
        audio_eq::update_eq_preamp,
        audio_eq::update_eq_effects,
        audio_eq::get_eq_effects,
        thirdparty_tools::get_thirdparty_tools,
        thirdparty_tools::get_tool_install_path,
        thirdparty_tools::get_tool_download_path,
        thirdparty_tools::run_tool,
        thirdparty_tools::download_tool,
        thirdparty_tools::open_tool_installer,
        overlay_panel::start_overlay_panel,
        overlay_panel::stop_overlay_panel,
        overlay_panel::get_overlay_panel_status,
        overlay_panel::set_active_gpu_index,
        overlay_panel::get_overlay_hardware_data,
        overlay_panel::update_overlay_settings,
        overlay_panel::toggle_overlay_panel,
        overlay_panel::set_overlay_drag_mode,
        overlay_panel::get_overlay_current_settings,
        overlay_panel::check_drag_mode_status,
        overlay_panel::reset_overlay_position,
        overlay_panel::run_pawnio_setup,

        vertical_overlay::start_vertical_overlay,
        vertical_overlay::stop_vertical_overlay,
        vertical_overlay::save_vertical_overlay_position,
        vertical_overlay::set_vertical_overlay_click_through,
        vertical_overlay::reset_vertical_overlay_position,
        vertical_overlay::resize_vertical_overlay,

        hardware_report::export_hardware_report,
        hardware_report::get_hardware_recording_status,
        hardware_report::clear_hardware_data,

        sensor::get_lhm_cpu_load,
        sensor::get_lhm_cpu_status,
        sensor::get_lhm_gpu_status,

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
        crosshair::pick_crosshair_image,
        crosshair::get_preset_crosshair_path,
        crosshair::get_crosshair_presets,

        delta_force::get_delta_passwords,
        delta_force::get_weapon_codes,
        delta_force::get_dlss_model_presets,
        delta_force::apply_dlss_model_preset,
        delta_force::get_dlss_preset_status,
        delta_force::get_delta_maps,
        delta_force::toggle_dlss_indicator,
        delta_force::toggle_dlss_lock,
        delta_force::get_dlss_settings_status,
        delta_force::open_platform_window,
        game_launcher::launch_game,
        game_launcher::search_delta_force_launcher,
        game_launcher::get_default_delta_force_game,
        game_launcher::select_exe_file,
        game_launcher::get_file_icon,
        gpu_rename::get_gpu_info,
        gpu_rename::get_gpu_options,
        gpu_rename::apply_gpu_rename,
        gpu_rename::restore_gpu_name,
        video_bg::pick_video_file,
            sponsor::get_sponsors,
        shader_cache::scan_shader_caches,
        shader_cache::clean_shader_cache,
        nvapi::get_nvapi_status,
        nvapi::diagnose_nvapi,
        nvapi::get_nvidia_driver_version,
        nvapi::list_nvidia_settings,
        nvapi::set_nvidia_setting,
        nvapi::reset_nvidia_settings,
        nvapi::list_nvidia_displays,
        nvapi::get_nvidia_display_modes,
        nvapi::set_nvidia_display_resolution,
        nvapi::get_injected_resolutions,
        nvapi::remove_injected_resolution,
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
            tray::exit_app,
            tray::check_update_and_show,
            // === MCTier 命令 ===
            utils::cursor::get_cursor_position,
            utils::cursor::set_desktop_lyrics_click_through,
            utils::lyrics_btn::show_lyrics_unlock_btn,
            utils::lyrics_btn::hide_lyrics_unlock_btn,
                        utils::lyrics_btn::unlock_lyrics,

        // === CPU 核心调度 ===
        cpu_scheduler::get_cpu_topology,
        cpu_scheduler::get_process_list,
        cpu_scheduler::get_process_affinity,
        cpu_scheduler::set_process_affinity,
        cpu_scheduler::restore_process_affinity,
        cpu_scheduler::get_saved_rules,
        cpu_scheduler::save_rule,
        cpu_scheduler::delete_rule,
        cpu_scheduler::apply_rule_by_name,

    ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        match event {
            tauri::RunEvent::ExitRequested { .. } => {
                // 退出流程开始前隐藏所有窗口，避免 WebView2 销毁后闪现原生标题栏
                for label in &["main", "widget", "tray-menu", "desktop-lyrics", "lyrics-unlock-btn", "vertical-overlay"] {
                    if let Some(w) = app_handle.get_webview_window(label) {
                        let _ = w.hide();
                    }
                }
            }
            tauri::RunEvent::Exit => {
                sensor::stop_sensor_process(app_handle);
                hardware::cleanup_hardware_cache();
                overlay_panel::cleanup(); // 先停后台轮询线程(FPS/传感器)，再恢复 Gamma
                display_filter::cleanup();
                vertical_overlay::cleanup(app_handle);
                crosshair::cleanup();
                audio_eq::cleanup();
                tray::cleanup();
                hotkey::cleanup(app_handle);
                nvapi::cleanup();
                hardware_report::stop_recording();
            }
            _ => {}
        }
    });
}
