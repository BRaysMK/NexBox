mod announcement;
mod audio_engine;
mod audio_eq;
mod auto_start;
mod autoclicker;
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
mod game_filter;
mod game_launcher;
mod game_win_key;
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
mod nvidia_driver_download;
mod optimization;
mod overlay_panel;
mod vertical_overlay;

mod sensor;
mod sensor_monitor;
mod shader_cache;
mod pawnio_driver;
mod sponsor;
mod contributor;
mod startup_manager;
mod steam;
mod speedtest;
mod storage_clean;
mod thirdparty_tools;
mod tray;
mod utils;
mod video_bg;
mod wmi_query;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;

/// 主窗口可见性状态（用于最小化/托盘时暂停动态背景视频）
static MAIN_WINDOW_VISIBLE: AtomicBool = AtomicBool::new(true);

/// 按需创建竖排悬浮框窗口（不常驻，启用时创建、关闭时销毁）。
/// 创建后 visible(false)，前端渲染完成后调用 `vertical_overlay_ready` 命令 show，避免白屏闪烁。
pub fn ensure_vertical_overlay<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Option<tauri::WebviewWindow<R>> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    let label = "vertical-overlay";
    if let Some(win) = app.get_webview_window(label) {
        return Some(win);
    }

    let builder = WebviewWindowBuilder::new(app, label, WebviewUrl::App(label.into()))
        .title("NexBox Vertical Overlay")
        .inner_size(220.0, 400.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .visible(false)
        .maximizable(false)
        .skip_taskbar(true)
        .shadow(false);

    match builder.build() {
        Ok(win) => Some(win),
        Err(e) => {
            log::error!("[Window] 创建 vertical-overlay 失败: {e}");
            None
        }
    }
}

/// 设置当前进程效能模式（EcoQoS，即任务管理器中的"小绿叶"）。
/// 开启后 Windows 会主动限制该进程的 CPU/功耗，适合后台/最小化状态。
#[cfg(windows)]
fn set_efficiency_mode(enable: bool) {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, SetProcessInformation,
        ProcessPowerThrottling,
        PROCESS_POWER_THROTTLING_STATE,
        PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        PROCESS_SET_INFORMATION,
    };
    use windows::Win32::Foundation::GetLastError;

    let pid = std::process::id();
    unsafe {
        match OpenProcess(PROCESS_SET_INFORMATION, false, pid) {
            Ok(handle) => {
                // ControlMask / StateMask 直接用 u32 数值，避免 newtype 隐式转换问题
                // PROCESS_POWER_THROTTLING_EXECUTION_SPEED = 0x1
                const EXECUTION_SPEED: u32 = 0x1;

                let state = PROCESS_POWER_THROTTLING_STATE {
                    Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
                    ControlMask: EXECUTION_SPEED,
                    StateMask: if enable { EXECUTION_SPEED } else { 0 },
                };

                match SetProcessInformation(
                    handle,
                    ProcessPowerThrottling,
                    &state as *const _ as *const std::ffi::c_void,
                    std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
                ) {
                    Ok(_) => log::info!(
                        "[EcoQoS] 效能模式 {} 成功 (pid={})",
                        if enable { "开启 (小绿叶)" } else { "关闭" },
                        pid,
                    ),
                    Err(e) => log::error!(
                        "[EcoQoS] SetProcessInformation 失败: {e} (last_error={})",
                        GetLastError().0,
                    ),
                }

                let _ = CloseHandle(handle);
            }
            Err(e) => {
                log::error!("[EcoQoS] OpenProcess 失败: {e} (pid={})", pid);
            }
        }
    }
}

/// 通知前端主窗口可见性变化。仅在状态切换时发送，避免重复事件刷屏。
/// 同时自动切换系统效能模式：隐藏时开启 EcoQoS 降功耗，恢复时关闭。
pub fn emit_main_visibility<R: tauri::Runtime>(app: &tauri::AppHandle<R>, visible: bool) {
    use tauri::Emitter;
    if MAIN_WINDOW_VISIBLE.swap(visible, Ordering::SeqCst) != visible {
        let _ = app.emit("window-visibility-changed", visible);

        #[cfg(windows)]
        {
            if visible {
                set_efficiency_mode(false);
            } else {
                // 延时 3 秒，避免刚最小化又恢复时反复切换
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    if !MAIN_WINDOW_VISIBLE.load(Ordering::SeqCst) {
                        set_efficiency_mode(true);
                    }
                });
            }
        }
    }
}


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _: () = window.show().unwrap_or(());
                let _: () = window.set_focus().unwrap_or(());
                let _: () = window.unminimize().unwrap_or(());
                emit_main_visibility(app, true);
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
                        // 全部热键总开关关闭时，忽略所有全局热键
                        if !hotkey::is_hotkeys_enabled() {
                            return;
                        }
                        if shortcut.id() == hotkey::get_overlay_shortcut_id() {
                            let _ = overlay_panel::toggle_overlay(app);
                        } else if shortcut.id() == hotkey::get_crosshair_shortcut_id() {
                            let _ = crosshair::toggle_crosshair_sync(app);
                        } else if shortcut.id() == hotkey::get_filter_shortcut_id() {
                            let _ = display_filter::toggle_filter_sync(app);
                        } else if shortcut.id() == hotkey::get_autoclicker_shortcut_id() {
                            let _ = autoclicker::toggle(app);
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

            // 初始化游戏滤镜自动应用（读取持久化配置并启动后台轮询）
            let app_handle_for_game_filter = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let _ = game_filter::init(app_handle_for_game_filter).await;
            });

            // 初始化游戏启动时禁用 Win 键（读取持久化配置并启动后台轮询）
            let app_handle_for_game_win_key = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let _ = game_win_key::init(app_handle_for_game_win_key).await;
            });

            // Main window: intercept taskbar Close / Alt+F4 → hide instead of destroy，
            // 并通知前端窗口可见性变化（最小化/隐藏到托盘时暂停动态背景视频，降低 CPU 占用）
            if let Some(main_window) = app.get_webview_window("main") {
                let main_clone = main_window.clone();
                main_window.on_window_event(move |event| {
                    match event {
                        tauri::WindowEvent::CloseRequested { api, .. } => {
                            api.prevent_close();
                            let _ = main_clone.hide();
                            emit_main_visibility(&main_clone.app_handle(), false);
                        }
                        tauri::WindowEvent::Resized(_) => {
                            let minimized = main_clone.is_minimized().unwrap_or(false);
                            emit_main_visibility(&main_clone.app_handle(), !minimized);
                        }
                        _ => {}
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

            // 启动时直接从持久化配置读取用户设置的快捷键并注册，
            // 不再依赖前端启动后覆盖，避免用户自定义热键重启后失效
            let overlay_hotkey = hotkey::load_saved_hotkey(app.handle(), "overlay-hotkey", "Shift+F10");
            let crosshair_hotkey = hotkey::load_saved_hotkey(app.handle(), "crosshair-hotkey", "Shift+F9");
            let filter_hotkey = hotkey::load_saved_hotkey(app.handle(), "filter-hotkey", "Shift+F8");
            let autoclicker_hotkey = hotkey::load_saved_hotkey(app.handle(), "autoclicker-hotkey", "F8");
            hotkey::set_hotkeys_enabled(hotkey::load_saved_hotkeys_enabled(app.handle()));

            let _ = hotkey::init_overlay(app.handle(), &overlay_hotkey);
            let _ = hotkey::init_crosshair(app.handle(), &crosshair_hotkey);
            let _ = hotkey::init_filter(app.handle(), &filter_hotkey);
            let _ = hotkey::init_autoclicker(app.handle(), &autoclicker_hotkey);

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
        music_api::music_song_comments,
        music_api::music_send_comment,
        music_api::music_personalized,
        music_api::music_recommend_songs,
        music_api::music_recommend_resource,
        music_api::music_artist_search,
        music_api::music_artist_songs,
        music_api::music_artist_detail,
        music_api::music_artist_albums,
        music_api::music_artist_mvs,
        music_api::music_album_detail,
        music_api::music_mv_url,
        music_api::music_playlist_search,
        music_api::music_open_login_window,
        // === 酷狗音乐 API ===
        music_api::kugou_search,
        music_api::kugou_artist_search,
        music_api::kugou_playlist_search,
        music_api::kugou_artist_songs,
        music_api::kugou_song_url,
        music_api::kugou_lyric,
        music_api::kugou_login_status,
        music_api::kugou_login_cookie,
        music_api::kugou_logout,
        music_api::kugou_user_playlists,
        music_api::kugou_playlist_tracks,
        music_api::kugou_playlist_tracks_range,
        music_api::kugou_guess_like,
        music_api::kugou_rank_list,
        music_api::kugou_rank_songs,
        music_api::kugou_like_toggle,
        music_api::kugou_liked_hashes,
        // === QQ 音乐 API ===
        music_api::qq_search,
        music_api::qq_song_url,
        music_api::qq_lyric,
        music_api::qq_login_status,
        music_api::qq_login_cookie,
        music_api::qq_logout,
        music_api::qq_user_playlists,
        music_api::qq_playlist_tracks,
        music_api::qq_playlist_tracks_range,
        music_api::qq_artist_search,
        music_api::qq_artist_songs,
        music_api::qq_playlist_search,
        music_api::qq_rank_list,
        music_api::qq_rank_songs,
        music_api::music_qq_recommend_playlists,
        music_api::qq_liked_hashes,
        music_api::qq_like_toggle,
        // === 多平台管理 ===
        music_api::music_get_login_statuses,
        music_api::music_switch_provider,
        music_api::music_get_playback_source,
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
        optimization::get_laptop_power_lock_status,
        optimization::unlock_laptop_power_plan,
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
        network_optimize::clear_dns_cache,
        network_optimize::check_network_tweak_states,
        network_optimize::batch_network_enable,
        network_optimize::batch_network_disable,
        startup_manager::scan_startup_items,
        startup_manager::disable_startup_item,
        startup_manager::enable_startup_item,
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
        game_filter::get_game_filter_status,
        game_filter::set_game_filter_enabled,
        game_filter::add_custom_game,
        game_filter::remove_custom_game,
        game_win_key::get_game_win_key_status,
        game_win_key::set_game_win_key_enabled,
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
        pawnio_driver::check_pawnio_status,
        pawnio_driver::install_pawnio_driver,

        vertical_overlay::start_vertical_overlay,
        vertical_overlay::stop_vertical_overlay,
        vertical_overlay::vertical_overlay_ready,
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
        sensor::restart_monitor_process,
        sensor_monitor::open_sensor_monitor,
        sensor_monitor::get_all_sensors,

        game_ping::get_current_ping,
        hotkey::get_overlay_hotkey,
        hotkey::set_overlay_hotkey,
        hotkey::get_crosshair_hotkey,
        hotkey::set_crosshair_hotkey,
        hotkey::get_filter_hotkey,
        hotkey::set_filter_hotkey,
        hotkey::get_autoclicker_hotkey,
        hotkey::set_autoclicker_hotkey,
        hotkey::set_hotkeys_enabled_cmd,
        hotkey::get_hotkeys_enabled_cmd,
        autoclicker::autoclicker_start,
        autoclicker::autoclicker_stop,
        autoclicker::autoclicker_toggle,
        autoclicker::autoclicker_update,
        autoclicker::autoclicker_get_status,
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
            contributor::get_contributors,
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
        // === NVIDIA 驱动下载 ===
        nvidia_driver_download::fetch_nvidia_drivers,
        nvidia_driver_download::detect_current_nvidia_gpu,
            storage_clean::scan_storage_items,
            storage_clean::clean_storage_items,
            storage_clean::empty_recycle_bin_cmd,
            utils::sys_info::get_system_locale,
            utils::sys_info::get_system_username,
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

        // === Steam 集成 ===
        steam::get_steam_install_info,
        steam::get_steam_users,
        steam::get_steam_libraries,
        steam::get_steam_games,
        steam::get_steam_all_data,
        steam::launch_steam_client,
        steam::launch_steam_game,
        steam::open_steam_store_page,
        steam::open_game_folder,
        steam::switch_steam_account,
        steam::delete_steam_account,
        steam::uninstall_steam_game,
        steam::format_file_size,
        steam::get_steam_stats,
        steam::get_library_disk_info,
        steam::steam_debug,
        steam::get_steam_user_avatars,

        // === 网络测速 ===
        speedtest::start_speedtest,
        speedtest::stop_speedtest,
        speedtest::is_speedtest_running,

    ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        match event {
            tauri::RunEvent::ExitRequested { .. } => {
                // 退出流程开始前隐藏所有窗口，避免 WebView2 销毁后闪现原生标题栏
                for label in &["main", "tray-menu", "desktop-lyrics", "lyrics-unlock-btn", "vertical-overlay"] {
                    if let Some(w) = app_handle.get_webview_window(label) {
                        let _ = w.hide();
                    }
                }
            }
            tauri::RunEvent::Exit => {
                sensor::stop_sensor_process(app_handle);
                hardware::cleanup_hardware_cache();
                overlay_panel::cleanup(); // 先停后台轮询线程(FPS/传感器)，再恢复 Gamma
                game_win_key::cleanup();
                speedtest::cleanup();
                display_filter::cleanup();
                vertical_overlay::cleanup(app_handle);
                crosshair::cleanup();
                autoclicker::cleanup();
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
