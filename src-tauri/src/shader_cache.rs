use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderCacheDir {
    pub name: String,
    pub path: String,
    pub exists: bool,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VendorScanResult {
    pub vendor: String,
    pub dirs: Vec<ShaderCacheDir>,
    pub total_dirs: u64,
    pub total_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub nvidia: VendorScanResult,
    pub amd: VendorScanResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanResult {
    pub success: bool,
    pub message: String,
    pub freed_bytes: u64,
}

fn get_dir_size(path: &std::path::Path) -> u64 {
    let mut total = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += get_dir_size(&p);
            } else if let Ok(metadata) = fs::metadata(&p) {
                total += metadata.len();
            }
        }
    }
    total
}

fn scan_cache_dir(name: &str, path: PathBuf) -> ShaderCacheDir {
    let exists = path.exists() && path.is_dir();
    let size_bytes = if exists { get_dir_size(&path) } else { 0 };
    ShaderCacheDir {
        name: name.to_string(),
        path: path.to_string_lossy().to_string(),
        exists,
        size_bytes,
    }
}

fn get_local_app_data() -> Option<PathBuf> {
    dirs::data_local_dir()
}

fn get_local_low_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join("AppData").join("LocalLow"))
}

fn get_nvidia_dirs(base: &PathBuf) -> Vec<ShaderCacheDir> {
    vec![
        scan_cache_dir("NVIDIA DXCache", base.join("NVIDIA").join("DXCache")),
        scan_cache_dir("NVIDIA GLCache", base.join("NVIDIA").join("GLCache")),
        scan_cache_dir(
            "NVIDIA Corporation NV_Cache",
            base.join("NVIDIA Corporation").join("NV_Cache"),
        ),
    ]
}

fn get_amd_dirs(local: &PathBuf, local_low: &PathBuf) -> Vec<ShaderCacheDir> {
    vec![
        scan_cache_dir("AMD DxCache", local.join("AMD").join("DxCache")),
        scan_cache_dir("AMD GLCache", local.join("AMD").join("GLCache")),
        scan_cache_dir("AMD VkCache", local.join("AMD").join("VkCache")),
        scan_cache_dir("AMD DxcCache", local.join("AMD").join("DxcCache")),
        scan_cache_dir("AMD LocalLow DxCache", local_low.join("AMD").join("DxCache")),
        scan_cache_dir("AMD LocalLow GLCache", local_low.join("AMD").join("GLCache")),
        scan_cache_dir("AMD LocalLow VkCache", local_low.join("AMD").join("VkCache")),
    ]
}

fn build_vendor_result(vendor: &str, dirs: Vec<ShaderCacheDir>) -> VendorScanResult {
    let total_dirs = dirs.iter().filter(|d| d.exists).count() as u64;
    let total_size = dirs.iter().map(|d| d.size_bytes).sum();
    VendorScanResult {
        vendor: vendor.to_string(),
        dirs,
        total_dirs,
        total_size,
    }
}

#[tauri::command]
pub async fn scan_shader_caches() -> Result<ScanResult, String> {
    let local_app_data = get_local_app_data().ok_or("无法获取 LocalAppData 目录")?;
    let local_low = get_local_low_dir().ok_or("无法获取 LocalLow 目录")?;

    let nvidia_dirs = get_nvidia_dirs(&local_app_data);
    let amd_dirs = get_amd_dirs(&local_app_data, &local_low);

    Ok(ScanResult {
        nvidia: build_vendor_result("nvidia", nvidia_dirs),
        amd: build_vendor_result("amd", amd_dirs),
    })
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

#[tauri::command]
pub async fn clean_shader_cache(vendor: String) -> Result<CleanResult, String> {
    let local_app_data = get_local_app_data().ok_or("无法获取 LocalAppData 目录")?;
    let local_low = get_local_low_dir().ok_or("无法获取 LocalLow 目录")?;

    let target_dirs: Vec<PathBuf> = match vendor.as_str() {
        "nvidia" => vec![
            local_app_data.join("NVIDIA").join("DXCache"),
            local_app_data.join("NVIDIA").join("GLCache"),
            local_app_data.join("NVIDIA Corporation").join("NV_Cache"),
        ],
        "amd" => vec![
            local_app_data.join("AMD").join("DxCache"),
            local_app_data.join("AMD").join("GLCache"),
            local_app_data.join("AMD").join("VkCache"),
            local_app_data.join("AMD").join("DxcCache"),
            local_low.join("AMD").join("DxCache"),
            local_low.join("AMD").join("GLCache"),
            local_low.join("AMD").join("VkCache"),
        ],
        _ => return Err(format!("不支持的显卡厂商: {}", vendor)),
    };

    let mut freed_bytes: u64 = 0;
    let mut cleaned_count = 0;

    for dir_path in target_dirs {
        if !dir_path.exists() {
            continue;
        }

        let before_size = get_dir_size(&dir_path);

        match fs::remove_dir_all(&dir_path) {
            Ok(()) => {
                freed_bytes += before_size;
                cleaned_count += 1;
            }
            Err(e) => {
                eprintln!(
                    "Failed to remove directory {:?}: {}",
                    dir_path, e
                );
            }
        }
    }

    if cleaned_count == 0 && freed_bytes == 0 {
        Ok(CleanResult {
            success: true,
            message: "没有找到需要清理的缓存目录".to_string(),
            freed_bytes: 0,
        })
    } else {
        Ok(CleanResult {
            success: true,
            message: format!(
                "清理完成，已清理 {} 个目录，释放 {}",
                cleaned_count,
                format_size(freed_bytes)
            ),
            freed_bytes,
        })
    }
}
