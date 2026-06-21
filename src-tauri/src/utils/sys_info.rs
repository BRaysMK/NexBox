use serde_json::json;
use tauri_plugin_os::locale;
use tauri_plugin_store::StoreExt;

async fn send_statistics(version: String, os: String) {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    match client
        .post("https://mc.sjtu.cn/api-sjmcl/statistics")
        .json(&json!({
            "version": format!("box-{}", version),
            "os": os,
        }))
        .send()
        .await
    {
        Ok(resp) => log::info!("Statistics sent, status: {}", resp.status()),
        Err(e) => log::error!("Failed to send statistics: {}", e),
    }
}

pub fn check_and_send_statistics(app: &tauri::App) {
    let store = match app.store("settings.json") {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to open settings store: {}", e);
            return;
        }
    };

    if store.has("box-statistics-sent") {
        log::info!("Statistics already sent, skipping");
        return;
    }

    let version = env!("CARGO_PKG_VERSION").to_string();
    let os = std::env::consts::OS.to_string();
    log::info!("Sending statistics: version={}, os={}", version, os);

    let app_handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        send_statistics(version, os).await;

        if let Ok(store) = app_handle.store("settings.json") {
            store.set("box-statistics-sent", json!(true));
            let _ = store.save();
            log::info!("Statistics flag saved");
        }
    });
}

pub fn get_mapped_locale() -> String {
    let locale = locale().unwrap_or_else(|| "en".to_string());
    let matched_locale;

    #[cfg(target_os = "macos")]
    {
        let language_map = [
            ("fr", vec!["fr"]),
            ("ja", vec!["ja"]),
            ("zh-Hans", vec!["zh-Hans", "wuu-Hans", "yue-Hans"]),
            ("zh-Hant", vec!["zh-Hant", "yue-Hant"]),
        ];

        matched_locale = language_map
            .iter()
            .find(|(_, locales)| locales.iter().any(|l| locale.starts_with(l)))
            .map(|(mapped, _)| mapped.to_string());
    }

    #[cfg(not(target_os = "macos"))]
    {
        let language_map = [
            ("fr", vec!["fr"]),
            ("ja", vec!["ja"]),
            ("zh-Hans", vec!["zh-CN", "zh-SG"]),
            ("zh-Hant", vec!["zh-TW", "zh-HK", "zh-MO"]),
        ];

        matched_locale = language_map
            .iter()
            .find(|(_, locales)| locales.iter().any(|l| locale.starts_with(l)))
            .map(|(mapped, _)| mapped.to_string());
    }

    matched_locale.unwrap_or_else(|| "en".to_string())
}

#[tauri::command]
pub fn get_system_locale() -> String {
    get_mapped_locale()
}
