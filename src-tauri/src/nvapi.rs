//! NVAPI backend for NVIDIA GPU driver settings (DRS API).
//!
//! Links against the official NVAPI SDK library and provides Tauri commands
//! to read/write global 3D settings (VSync, texture quality, AA, FPS limit, etc.).

use serde::Serialize;
use std::sync::Mutex;
use libloading::Library;

// ---------------------------------------------------------------------------
// NVAPI C type aliases
// ---------------------------------------------------------------------------
type NvU32 = u32;
type NvU16 = u16;

#[allow(non_camel_case_types)]
type NvAPI_Status = i32;

const NVAPI_OK: NvAPI_Status = 0;

/// Opaque handle types (NV_DECLARE_HANDLE => opaque pointer)
type NvDRSSessionHandle = *mut std::ffi::c_void;
type NvDRSProfileHandle = *mut std::ffi::c_void;
type NvDisplayHandle = *mut std::ffi::c_void;

const NVAPI_UNICODE_STRING_MAX: usize = 2048;
const NVAPI_SHORT_STRING_MAX: usize = 64;

// ---------------------------------------------------------------------------
// NVIDIA DRS structures (replicated from nvapi.h)
// ---------------------------------------------------------------------------

#[repr(C)]
struct NvdrsSettingV1 {
    version: NvU32,
    setting_name: [NvU16; NVAPI_UNICODE_STRING_MAX],
    setting_id: NvU32,
    setting_type: NvU32,
    setting_location: NvU32,
    is_current_predefined: NvU32,
    is_predefined_valid: NvU32,
    // Anonymous union: max(NvU32, NVDRS_BINARY_SETTING=4+4096, NvAPI_UnicodeString=4096) = 4100
    predefined_union: [u8; 4100],
    // Anonymous union: same layout
    current_union: [u8; 4100],
}

impl NvdrsSettingV1 {
    fn version_const() -> NvU32 {
        (std::mem::size_of::<Self>() as NvU32) | (1u32 << 16)
    }

    fn new() -> Self {
        let mut s: Self = unsafe { std::mem::zeroed() };
        s.version = Self::version_const();
        s
    }

    fn predefined_u32(&self) -> NvU32 {
        unsafe { std::ptr::read_unaligned(self.predefined_union.as_ptr() as *const NvU32) }
    }

    fn current_u32(&self) -> NvU32 {
        unsafe { std::ptr::read_unaligned(self.current_union.as_ptr() as *const NvU32) }
    }

    fn set_current_u32(&mut self, val: NvU32) {
        unsafe {
            std::ptr::write_unaligned(self.current_union.as_mut_ptr() as *mut NvU32, val);
        }
    }
}

// Compile-time verification: sizeof(NVDRS_SETTING_V1) with pack(8)
// = 4 + 4096 + 4 + 4 + 4 + 4 + 4 + 4100 + 4100 = 12320
const _: () = assert!(std::mem::size_of::<NvdrsSettingV1>() == 12320);

#[repr(C)]
struct NvDisplayDriverVersion {
    version: NvU32,
    drv_version: NvU32,
    bld_change_list_num: NvU32,
    sz_build_branch_string: [u8; NVAPI_SHORT_STRING_MAX],
    sz_adapter_string: [u8; NVAPI_SHORT_STRING_MAX],
}

// Compile-time verification: sizeof(NV_DISPLAY_DRIVER_VERSION)
// = 4 + 4 + 4 + 64 + 64 = 140
const _: () = assert!(std::mem::size_of::<NvDisplayDriverVersion>() == 140);



#[cfg(target_os = "windows")]
#[link(name = "nvapi64", kind = "static")]
extern "C" {
    fn NvAPI_Initialize() -> NvAPI_Status;
    fn NvAPI_Unload() -> NvAPI_Status;
    fn NvAPI_GetErrorMessage(status: NvAPI_Status, desc: *mut u8) -> NvAPI_Status;
    fn NvAPI_DRS_CreateSession(session: *mut NvDRSSessionHandle) -> NvAPI_Status;
    fn NvAPI_DRS_DestroySession(session: NvDRSSessionHandle) -> NvAPI_Status;
    fn NvAPI_DRS_LoadSettings(session: NvDRSSessionHandle) -> NvAPI_Status;
    fn NvAPI_DRS_SaveSettings(session: NvDRSSessionHandle) -> NvAPI_Status;
    fn NvAPI_DRS_GetCurrentGlobalProfile(
        session: NvDRSSessionHandle,
        profile: *mut NvDRSProfileHandle,
    ) -> NvAPI_Status;
    fn NvAPI_DRS_GetBaseProfile(
        session: NvDRSSessionHandle,
        profile: *mut NvDRSProfileHandle,
    ) -> NvAPI_Status;
    fn NvAPI_DRS_GetSetting(
        session: NvDRSSessionHandle,
        profile: NvDRSProfileHandle,
        setting_id: NvU32,
        setting: *mut NvdrsSettingV1,
    ) -> NvAPI_Status;
    fn NvAPI_DRS_SetSetting(
        session: NvDRSSessionHandle,
        profile: NvDRSProfileHandle,
        setting: *mut NvdrsSettingV1,
    ) -> NvAPI_Status;
    fn NvAPI_DRS_RestoreProfileDefault(
        session: NvDRSSessionHandle,
        profile: NvDRSProfileHandle,
    ) -> NvAPI_Status;
    fn NvAPI_EnumNvidiaDisplayHandle(index: NvU32, display: *mut NvDisplayHandle) -> NvAPI_Status;
    fn NvAPI_GetDisplayDriverVersion(
        display: NvDisplayHandle,
        version: *mut NvDisplayDriverVersion,
    ) -> NvAPI_Status;
}

// ---------------------------------------------------------------------------
// Global state — Library kept alive, function pointers cached
// ---------------------------------------------------------------------------
struct NvapiState {
    session: NvDRSSessionHandle,
}

unsafe impl Send for NvapiState {}
unsafe impl Sync for NvapiState {}

static NVAPI: Mutex<Option<NvapiState>> = Mutex::new(None);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn utf16_to_string(data: &[u16]) -> String {
    let end = data.iter().position(|&c| c == 0).unwrap_or(data.len());
    String::from_utf16_lossy(&data[..end])
}

fn cstr_to_string(data: &[u8]) -> String {
    let end = data.iter().position(|&c| c == 0).unwrap_or(data.len());
    String::from_utf8_lossy(&data[..end]).to_string()
}

fn nvapi_error_string(status: NvAPI_Status) -> String {
    let mut buf: [u8; NVAPI_SHORT_STRING_MAX] = [0u8; NVAPI_SHORT_STRING_MAX];
    unsafe {
        NvAPI_GetErrorMessage(status, buf.as_mut_ptr());
    }
    cstr_to_string(&buf)
}

fn nvapi_system_dir() -> String {
    std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into())
}

// ---------------------------------------------------------------------------
// DLL loading & session init (using libloading)
// ---------------------------------------------------------------------------

/// Find all nvapi64.dll candidates across the system, sorted by filesize (largest first).
/// The real NVIDIA nvapi64.dll is 2-4+ MB; stubs are <1 MB.
fn find_nvapi64_candidates() -> Vec<(String, u64)> {
    let mut files: Vec<(String, u64)> = Vec::new();

    // Helper: add file if it exists
    let mut add_file = |path: &str| {
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.is_file() && meta.len() > 0 {
                files.push((path.to_string(), meta.len()));
            }
        }
    };

    let sys_dir = nvapi_system_dir();
    let is_64bit = std::mem::size_of::<usize>() == 8;

    // Primary: System32
    add_file(&format!("{sys_dir}\\System32\\nvapi64.dll"));
    // Sysnative (bypasses WOW64 redirection for 32-bit processes)
    if !is_64bit {
        add_file(&format!("{sys_dir}\\Sysnative\\nvapi64.dll"));
    }

    // Common NVIDIA program dirs
    let prog_files = std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".into());
    let nv_dir = format!("{prog_files}\\NVIDIA Corporation");
    let common_subdirs = ["NvStreamSrv", "Nvapi64", "Display.NvContainer", "FrameViewSDK"];
    for sub in &common_subdirs {
        add_file(&format!("{nv_dir}\\{sub}\\nvapi64.dll"));
    }

    // Scan NVIDIA Corporation dir one level deep
    if let Ok(entries) = std::fs::read_dir(&nv_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let candidate = p.join("nvapi64.dll");
                if let Some(s) = candidate.to_str() {
                    add_file(s);
                }
            }
        }
    }

    // Search DriverStore for nvapi64.dll
    let driver_store = format!("{sys_dir}\\System32\\DriverStore\\FileRepository");
    if let Ok(entries) = std::fs::read_dir(&driver_store) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            // NVIDIA driver packages: nv_disp*, nv_dispi*, etc.
            if name_str.starts_with("nv_") || name_str.starts_with("nvd") || name_str.starts_with("nvdisps") {
                let candidate = entry.path().join("nvapi64.dll");
                if let Some(s) = candidate.to_str() {
                    add_file(s);
                }
            }
        }
    }

    // Sort by file size descending (biggest = most likely real NVIDIA DLL)
    files.sort_by(|a, b| b.1.cmp(&a.1));
    files
}

fn try_init_nvapi() -> Result<(), String> {
    let mut guard = NVAPI.lock().map_err(|e| format!("Lock error: {e}"))?;
    if guard.is_some() {
        return Ok(());
    }

    let init_status = unsafe { NvAPI_Initialize() };
    if init_status != NVAPI_OK {
        return Err(format!(
            "NvAPI_Initialize 失败: {} (代码 {init_status})",
            nvapi_error_string(init_status)
        ));
    }

    let mut session: NvDRSSessionHandle = std::ptr::null_mut();
    let create_status = unsafe { NvAPI_DRS_CreateSession(&mut session) };
    if create_status != NVAPI_OK {
        unsafe {
            let _ = NvAPI_Unload();
        }
        return Err(format!(
            "NvAPI_DRS_CreateSession 失败: {} (代码 {create_status})",
            nvapi_error_string(create_status)
        ));
    }

    let load_status = unsafe { NvAPI_DRS_LoadSettings(session) };
    if load_status != NVAPI_OK {
        unsafe {
            let _ = NvAPI_DRS_DestroySession(session);
            let _ = NvAPI_Unload();
        }
        return Err(format!(
            "NvAPI_DRS_LoadSettings 失败: {} (代码 {load_status})",
            nvapi_error_string(load_status)
        ));
    }

    let candidates = find_nvapi64_candidates();
    if let Some((path, size)) = candidates.first() {
        log::info!("NVAPI initialized via SDK library; detected runtime candidate: {path} ({size} bytes)");
    } else {
        log::info!("NVAPI initialized via SDK library");
    }
    *guard = Some(NvapiState { session });
    Ok(())
}

fn with_state<T, F: FnOnce(&NvapiState) -> Result<T, String>>(f: F) -> Result<T, String> {
    let guard = NVAPI.lock().map_err(|e| format!("Lock error: {e}"))?;
    match guard.as_ref() {
        Some(state) => f(state),
        None => Err("NVAPI 未初始化".into()),
    }
}

fn get_global_profile(state: &NvapiState) -> Result<NvDRSProfileHandle, String> {
    let mut profile: NvDRSProfileHandle = std::ptr::null_mut();
    let current_status = unsafe { NvAPI_DRS_GetCurrentGlobalProfile(state.session, &mut profile) };
    if current_status == NVAPI_OK {
        return Ok(profile);
    }

    let base_status = unsafe { NvAPI_DRS_GetBaseProfile(state.session, &mut profile) };
    if base_status == NVAPI_OK {
        return Ok(profile);
    }

    Err(format!(
        "获取全局配置文件失败: 当前全局配置={} (代码 {current_status})，基础配置={} (代码 {base_status})",
        nvapi_error_string(current_status),
        nvapi_error_string(base_status)
    ))
}

// ---------------------------------------------------------------------------
// Setting options — hardcoded from NvApiDriverSettings.h
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize)]
pub struct SettingOption {
    pub value: NvU32,
    pub label: String,
}

#[derive(Clone, Serialize)]
pub struct NvidiaSetting {
    pub id: NvU32,
    pub name: String,
    pub description: String,
    pub current_value: NvU32,
    pub default_value: NvU32,
    pub options: Vec<SettingOption>,
}

const SETTING_VSYNCMODE: NvU32 = 0x00A879CF;
const SETTING_QUALITY_ENHANCEMENTS: NvU32 = 0x00CE2691;
const SETTING_ANISO_MODE_LEVEL: NvU32 = 0x101E61A9;
const SETTING_AA_MODE_METHOD: NvU32 = 0x10D773D2;
const SETTING_FRL_FPS: NvU32 = 0x10835002;
const SETTING_PREFERRED_PSTATE: NvU32 = 0x1057EB71;
const SETTING_FXAA_ENABLE: NvU32 = 0x1074C972;

const TARGET_SETTINGS: &[NvU32] = &[
    SETTING_VSYNCMODE,
    SETTING_QUALITY_ENHANCEMENTS,
    SETTING_ANISO_MODE_LEVEL,
    SETTING_AA_MODE_METHOD,
    SETTING_FRL_FPS,
    SETTING_PREFERRED_PSTATE,
    SETTING_FXAA_ENABLE,
];

type OptsEntry = (NvU32, &'static str, Vec<SettingOption>);

fn settings_options() -> &'static [OptsEntry] {
    use std::sync::OnceLock;
    static OPTS: OnceLock<Vec<OptsEntry>> = OnceLock::new();
    OPTS.get_or_init(|| {
        vec![
            (
                SETTING_VSYNCMODE,
                "垂直同步",
                vec![
                    SettingOption { value: 0x60925292, label: "使用 3D 应用程序设置".into() },
                    SettingOption { value: 0x08416747, label: "关".into() },
                    SettingOption { value: 0x47814940, label: "开".into() },
                    SettingOption { value: 0x32610244, label: "半刷新率".into() },
                    SettingOption { value: 0x71271021, label: "1/3 刷新率".into() },
                    SettingOption { value: 0x13245256, label: "1/4 刷新率".into() },
                ],
            ),
            (
                SETTING_QUALITY_ENHANCEMENTS,
                "纹理过滤 - 质量",
                vec![
                    SettingOption { value: 0xFFFFFFF6, label: "高质量".into() },
                    SettingOption { value: 0x00000000, label: "质量".into() },
                    SettingOption { value: 0x0000000A, label: "性能".into() },
                    SettingOption { value: 0x00000014, label: "高性能".into() },
                ],
            ),
            (
                SETTING_ANISO_MODE_LEVEL,
                "各向异性过滤",
                vec![
                    SettingOption { value: 0x00000000, label: "关闭".into() },
                    SettingOption { value: 0x00000001, label: "应用程序控制".into() },
                    SettingOption { value: 0x00000002, label: "2x".into() },
                    SettingOption { value: 0x00000004, label: "4x".into() },
                    SettingOption { value: 0x00000008, label: "8x".into() },
                    SettingOption { value: 0x00000010, label: "16x".into() },
                ],
            ),
            (
                SETTING_AA_MODE_METHOD,
                "抗锯齿 - 模式",
                vec![
                    SettingOption { value: 0x00000000, label: "关闭".into() },
                    SettingOption { value: 0x00000001, label: "2x (超级采样)".into() },
                    SettingOption { value: 0x0000000E, label: "2x (多重采样)".into() },
                    SettingOption { value: 0x00000010, label: "4x (多重采样)".into() },
                    SettingOption { value: 0x00000012, label: "4x (多重采样 高斯)".into() },
                    SettingOption { value: 0x0000001B, label: "4x (多重采样 Gamma)".into() },
                    SettingOption { value: 0x00000025, label: "8x (多重采样)".into() },
                    SettingOption { value: 0x00000020, label: "2x (SSGSSAA)".into() },
                    SettingOption { value: 0x00000022, label: "4x (SSGSSAA)".into() },
                ],
            ),
            (
                SETTING_FRL_FPS,
                "最大帧速率",
                vec![
                    SettingOption { value: 0, label: "关闭".into() },
                    SettingOption { value: 30, label: "30 FPS".into() },
                    SettingOption { value: 60, label: "60 FPS".into() },
                    SettingOption { value: 120, label: "120 FPS".into() },
                    SettingOption { value: 144, label: "144 FPS".into() },
                    SettingOption { value: 165, label: "165 FPS".into() },
                    SettingOption { value: 240, label: "240 FPS".into() },
                ],
            ),
            (
                SETTING_PREFERRED_PSTATE,
                "电源管理模式",
                vec![
                    SettingOption { value: 0x00000000, label: "自适应".into() },
                    SettingOption { value: 0x00000001, label: "最高性能优先".into() },
                    SettingOption { value: 0x00000002, label: "由驱动程序控制".into() },
                    SettingOption { value: 0x00000003, label: "一致性能".into() },
                    SettingOption { value: 0x00000004, label: "最低功耗".into() },
                    SettingOption { value: 0x00000005, label: "最佳功率".into() },
                ],
            ),
            (
                SETTING_FXAA_ENABLE,
                "FXAA 快速近似抗锯齿",
                vec![
                    SettingOption { value: 0, label: "关闭".into() },
                    SettingOption { value: 1, label: "开启".into() },
                ],
            ),
        ]
    })
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct NvapiStatus {
    pub available: bool,
    pub error: Option<String>,
}

#[tauri::command]
pub fn get_nvapi_status() -> NvapiStatus {
    match try_init_nvapi() {
        Ok(()) => NvapiStatus {
            available: true,
            error: None,
        },
        Err(e) => NvapiStatus {
            available: false,
            error: Some(e),
        },
    }
}

#[derive(Serialize)]
pub struct NvapiDiagnostic {
    pub nvapi64_exists: bool,
    pub nvapi64_size: u64,
    pub nvapi64_path: String,
    pub tried_paths: Vec<String>,
    pub dll_loaded: bool,
    pub dll_load_error: Option<String>,
    pub loaded_module_path: Option<String>,
    pub has_initialize: bool,
    pub has_drs_create: bool,
    pub initialize_status: i32,
    pub initialize_error: Option<String>,
    pub conclusion: String,
    pub suggestion: String,
    pub exports: Vec<String>,
    pub is_64bit_process: bool,
}

/// Check a candidate nvapi64.dll.
/// Since static linking is used, this simply checks if the file exists,
/// reports its size, and attempts a static-link NVAPI call to verify.
fn try_load_and_check(path: &str) -> Result<(String, u64, Vec<String>, bool, bool, i32, Option<String>), String> {
    let meta = std::fs::metadata(path).map_err(|_| "文件不存在".to_string())?;
    let size = meta.len();

    let mut exports = Vec::new();

    // Try to load with libloading to see if it's a valid DLL at all
    let _library = match unsafe { Library::new(path) } {
        Ok(lib) => {
            exports.push("DLL 可加载".into());
            lib
        }
        Err(e) => {
            return Err(format!("DLL 加载失败: {e}"));
        }
    };

    // DRS availability: check file size — real nvapi64.dll >= ~2 MB
    let has_drs = size >= 2_000_000;

    // We can't easily test NvAPI_Initialize from here since it's statically linked
    // (the import would resolve to whatever DLL the linker found).
    // Report the file info for diagnostic purposes.
    exports.push(format!("文件大小: {} KB ({} bytes)", size / 1024, size));
    exports.push(if has_drs {
        "大小 >= 2MB，包含 DRS 接口可能性高".into()
    } else {
        "大小 < 2MB，可能为精简存根".into()
    });

    Ok((path.to_string(), size, exports, true, has_drs, 0, None))
}

#[tauri::command]
pub fn diagnose_nvapi() -> NvapiDiagnostic {
    let is_64bit = std::mem::size_of::<usize>() == 8;
    let bitness = if is_64bit { "64位" } else { "32位" };

    let all_candidates = find_nvapi64_candidates();

    let mut diag = NvapiDiagnostic {
        nvapi64_exists: !all_candidates.is_empty(),
        nvapi64_size: all_candidates.first().map(|(_, s)| *s).unwrap_or(0),
        nvapi64_path: all_candidates.first().map(|(p, _)| p.clone()).unwrap_or_default(),
        tried_paths: Vec::new(),
        dll_loaded: false,
        dll_load_error: None,
        loaded_module_path: None,
        has_initialize: false,
        has_drs_create: false,
        initialize_status: 0,
        initialize_error: None,
        conclusion: String::new(),
        suggestion: String::new(),
        exports: Vec::new(),
        is_64bit_process: is_64bit,
    };

    // Build summary of all found files
    let mut file_summary = String::new();
    for (path, size) in &all_candidates {
        file_summary.push_str(&format!("• {} ({} KB)\n", path, size / 1024));
    }

    // Try each candidate (already sorted largest first)
    for (path, _size) in &all_candidates {
        diag.tried_paths.push(path.clone());

        match try_load_and_check(path) {
            Ok((loaded_path, file_size, exports, has_init, has_drs, init_status, init_error)) => {
                diag.dll_loaded = true;
                diag.loaded_module_path = Some(loaded_path.clone());
                diag.nvapi64_size = file_size;
                diag.nvapi64_exists = true;
                diag.has_initialize = has_init;
                diag.has_drs_create = has_drs;
                diag.initialize_status = init_status;
                diag.initialize_error = init_error;
                diag.exports = exports;

                if has_init && init_status == NVAPI_OK {
                    break;
                }
            }
            Err(e) => {
                diag.dll_load_error = Some(format!("{path}: {e}"));
            }
        }
    }

    // Build conclusion
    if diag.has_initialize && diag.has_drs_create && diag.initialize_status == NVAPI_OK {
        diag.conclusion = format!(
            "NVAPI 完整可用，从 {} 加载（Init+DRS均就绪）",
            diag.loaded_module_path.as_deref().unwrap_or("N/A")
        );
        diag.suggestion = "无需额外操作。".into();
    } else if diag.has_initialize && diag.initialize_status == NVAPI_OK && !diag.has_drs_create {
        diag.conclusion = format!(
            "初始化成功但缺少 DRS 设置功能！\nDLL: {} ({}KB)\n此 nvapi64.dll 为精简存根，仅含 Init/Unload 序号导出，缺少 DRS 设置接口。\n无法调节纹理质量、垂直同步等 3D 设置。",
            diag.loaded_module_path.as_deref().unwrap_or("N/A"),
            diag.nvapi64_size / 1024,
        );
        diag.suggestion = "请使用 DDU 完全卸载当前驱动，从 nvidia.com 下载最新驱动后选择「自定义安装 → 清洁安装」。".into();
    } else if diag.has_initialize && diag.initialize_status != NVAPI_OK {
        let err_msg = diag.initialize_error.as_deref().unwrap_or("(未知)");
        diag.conclusion = format!(
            "NvAPI_Initialize 返回错误: {} (代码 {})",
            err_msg, diag.initialize_status
        );
        diag.suggestion = "请确认当前显示器连接到 NVIDIA GPU。".into();
    } else if all_candidates.is_empty() {
        diag.conclusion = "系统中未找到任何 nvapi64.dll。可能未安装 NVIDIA 显卡驱动。".into();
        diag.suggestion = "请访问 nvidia.com/drivers 下载安装驱动。".into();
    } else if !diag.dll_loaded {
        let load_err = diag.dll_load_error.as_deref().unwrap_or("(未知)");
        diag.conclusion = format!(
            "找到 {} 个 nvapi64.dll 但全部加载失败: {load_err}\n文件列表:\n{file_summary}",
            all_candidates.len()
        );
        diag.suggestion = "请从 NVIDIA 官网安装最新驱动，选择「清洁安装」。".into();
    } else {
        diag.conclusion = format!(
            "未找到有效 NVAPI! 进程:{bitness} | 找到{}个文件 | 当前加载: {} ({}KB, {}导出)\n文件列表:\n{file_summary}",
            all_candidates.len(),
            diag.loaded_module_path.as_deref().unwrap_or("N/A"),
            diag.nvapi64_size / 1024,
            diag.exports.len(),
        );
        if diag.nvapi64_size < 1_000_000 {
            diag.suggestion = "⚠ 所有 nvapi64.dll 均小于 1 MB，不是 NVIDIA 官方版本！请从 nvidia.com 下载最新驱动，安装时选择「自定义安装 → 清洁安装」。".into();
        } else {
            diag.suggestion = "文件存在但无 NVAPI 导出。尝试 DDU 完全卸载后重装 NVIDIA 驱动。".into();
        }
    }

    diag
}

#[derive(Serialize)]
pub struct NvidiaDriverInfo {
    pub version: u32,
    pub branch: String,
    pub gpu_name: String,
}

#[tauri::command]
pub fn get_nvidia_driver_version() -> Result<NvidiaDriverInfo, String> {
    try_init_nvapi()?;
    let mut display: NvDisplayHandle = std::ptr::null_mut();
    let status = unsafe { NvAPI_EnumNvidiaDisplayHandle(0, &mut display) };
    if status != NVAPI_OK {
        return Ok(NvidiaDriverInfo { version: 0, branch: "Unknown".into(), gpu_name: "NVIDIA GPU".into() });
    }

    let mut ver_info: NvDisplayDriverVersion = unsafe { std::mem::zeroed() };
    ver_info.version = (std::mem::size_of::<NvDisplayDriverVersion>() as NvU32) | (1u32 << 16);
    let status = unsafe { NvAPI_GetDisplayDriverVersion(display, &mut ver_info) };
    if status != NVAPI_OK {
        return Ok(NvidiaDriverInfo { version: 0, branch: "Unknown".into(), gpu_name: "NVIDIA GPU".into() });
    }

    let branch = cstr_to_string(&ver_info.sz_build_branch_string);
    let gpu_name = cstr_to_string(&ver_info.sz_adapter_string).trim().to_string();
    Ok(NvidiaDriverInfo { version: ver_info.drv_version, branch, gpu_name })
}

#[tauri::command]
pub fn list_nvidia_settings() -> Result<Vec<NvidiaSetting>, String> {
    try_init_nvapi()?;
    with_state(|state| {
        let profile = get_global_profile(state)?;
        let opts_list = settings_options();

        let mut results = Vec::new();

        for &target_id in TARGET_SETTINGS {
            let mut setting = NvdrsSettingV1::new();
            setting.setting_id = target_id;

            let status =
                unsafe { NvAPI_DRS_GetSetting(state.session, profile, target_id, &mut setting) };
            if status != NVAPI_OK {
                log::warn!(
                    "NvAPI_DRS_GetSetting for 0x{target_id:08X} failed: {}",
                    nvapi_error_string(status)
                );
                continue;
            }

            let name = utf16_to_string(&setting.setting_name);
            let current = setting.current_u32();
            let default = setting.predefined_u32();

            let options = opts_list
                .iter()
                .find(|(id, _, _)| *id == target_id)
                .map(|(_, _, opts)| opts.clone())
                .unwrap_or_else(|| {
                    vec![SettingOption {
                        value: current,
                        label: format!("{current}"),
                    }]
                });

            let description = match target_id {
                SETTING_VSYNCMODE => "控制画面撕裂与帧同步",
                SETTING_QUALITY_ENHANCEMENTS => "纹理过滤的全局画质等级",
                SETTING_ANISO_MODE_LEVEL => "增强斜角纹理的清晰度",
                SETTING_AA_MODE_METHOD => "平滑物体边缘锯齿",
                SETTING_FRL_FPS => "限制游戏最大帧率，降低功耗",
                SETTING_PREFERRED_PSTATE => "控制 GPU 性能和功耗策略",
                SETTING_FXAA_ENABLE => "快速且低开销的抗锯齿技术",
                _ => "",
            };

            results.push(NvidiaSetting {
                id: target_id,
                name: opts_list
                    .iter()
                    .find(|(id, _, _)| *id == target_id)
                    .map(|(_, n, _)| n.to_string())
                    .unwrap_or(name),
                description: description.to_string(),
                current_value: current,
                default_value: default,
                options,
            });
        }

        Ok(results)
    })
}

#[tauri::command]
pub fn set_nvidia_setting(setting_id: NvU32, value: NvU32) -> Result<(), String> {
    try_init_nvapi()?;
    with_state(|state| {
        let profile = get_global_profile(state)?;

        let mut setting = NvdrsSettingV1::new();
        setting.setting_id = setting_id;
        setting.set_current_u32(value);

        let status =
            unsafe { NvAPI_DRS_SetSetting(state.session, profile, &mut setting) };
        if status != NVAPI_OK {
            return Err(format!(
                "设置 0x{setting_id:08X} 失败: {}",
                nvapi_error_string(status)
            ));
        }

        let status = unsafe { NvAPI_DRS_SaveSettings(state.session) };
        if status != NVAPI_OK {
            return Err(format!(
                "保存设置失败: {}",
                nvapi_error_string(status)
            ));
        }

        log::info!("NVIDIA 设置 0x{setting_id:08X} 已更新为 {value}");
        Ok(())
    })
}

#[tauri::command]
pub fn reset_nvidia_settings() -> Result<(), String> {
    try_init_nvapi()?;
    with_state(|state| {
        let profile = get_global_profile(state)?;

        let status = unsafe { NvAPI_DRS_RestoreProfileDefault(state.session, profile) };
        // -151 = NVAPI_PROFILE_REMOVED (success case)
        if status != NVAPI_OK && status != -151 {
            return Err(format!(
                "恢复默认设置失败: {}",
                nvapi_error_string(status)
            ));
        }

        // Profile might be invalidated; get a fresh handle and save
        let _new_profile = get_global_profile(state)?;
        let status = unsafe { NvAPI_DRS_SaveSettings(state.session) };
        if status != NVAPI_OK {
            return Err(format!(
                "保存恢复后的设置失败: {}",
                nvapi_error_string(status)
            ));
        }

        log::info!("NVIDIA 全局设置已恢复默认值");
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Cleanup
// ---------------------------------------------------------------------------

pub fn cleanup() {
    if let Ok(mut guard) = NVAPI.lock() {
        if let Some(state) = guard.take() {
            unsafe {
                NvAPI_DRS_DestroySession(state.session);
                NvAPI_Unload();
            }
            log::info!("NVAPI session cleaned up");
        }
    }
}
