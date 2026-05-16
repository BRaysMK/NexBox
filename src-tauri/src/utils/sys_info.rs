use tauri_plugin_os::locale;

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
