//! NVAPI backend for NVIDIA GPU driver settings (DRS API).
//!
//! Links against the official NVAPI SDK library and provides Tauri commands
//! to read/write global 3D settings (VSync, texture quality, AA, FPS limit, etc.).

use serde::Serialize;
use std::process::Command;
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

// ---------------------------------------------------------------------------
// NVAPI DISP (Display Control) structures
// nvapi.h uses #pragma pack(push, 8) — #[repr(C)] on x64 matches this layout.
// Field names match the C SDK header names deliberately.
// ---------------------------------------------------------------------------

#[allow(non_snake_case)]
#[repr(C)]
struct NvResolution {
    width: NvU32,
    height: NvU32,
    colorDepth: NvU32,
}

const _: () = assert!(std::mem::size_of::<NvResolution>() == 12);

#[repr(C)]
struct NvPosition {
    x: NvU32,
    y: NvU32,
}

const _: () = assert!(std::mem::size_of::<NvPosition>() == 8);

#[allow(non_snake_case)]
#[repr(C)]
struct NvDisplayConfigSourceModeInfo {
    resolution: NvResolution,       // NV_RESOLUTION
    colorFormat: NvU32,             // NV_FORMAT (ignored, must be NV_FORMAT_UNKNOWN=0)
    position: NvPosition,           // NV_POSITION
    spanningOrientation: NvU32,     // NV_DISPLAYCONFIG_SPANNING_ORIENTATION (0=none)
    flags: NvU32,                   // bGDIPrimary:1, bSLIFocus:1, reserved:30
}

// sizeof(NV_DISPLAYCONFIG_SOURCE_MODE_INFO_V1) = 12 + 4 + 8 + 4 + 4 = 32
const _: () = assert!(std::mem::size_of::<NvDisplayConfigSourceModeInfo>() == 32);

#[allow(non_snake_case)]
#[repr(C)]
struct NvDisplayConfigPathTargetInfo {
    displayId: NvU32,
    details: *mut std::ffi::c_void, // NV_DISPLAYCONFIG_PATH_ADVANCED_TARGET_INFO* (NULL if not needed)
    targetId: NvU32,                // Windows CCD target ID (non-NVIDIA adapter only, 0 for NVIDIA)
}

// sizeof(NV_DISPLAYCONFIG_PATH_TARGET_INFO_V2) = 4 + pad4 + 8 + 4 = 24 (x64 pack 8)
const _: () = assert!(std::mem::size_of::<NvDisplayConfigPathTargetInfo>() == 24);

#[allow(non_snake_case)]
#[repr(C)]
struct NvDisplayConfigPathInfo {
    version: NvU32,                         // MAKE_NVAPI_VERSION(NV_DISPLAYCONFIG_PATH_INFO_V2, 2)
    sourceId: NvU32,                        // union { sourceId, reserved_sourceId }
    targetInfoCount: NvU32,                 // number of elements in targetInfo array
    targetInfo: *mut NvDisplayConfigPathTargetInfo,
    sourceModeInfo: *mut NvDisplayConfigSourceModeInfo, // may be NULL
    flags: NvU32,                           // IsNonNVIDIAAdapter:1, reserved:31
    pOSAdapterID: *mut std::ffi::c_void,    // LUID pointer for non-NVIDIA adapter
}

// sizeof(NV_DISPLAYCONFIG_PATH_INFO_V2) = 4+4+4+pad4+8+8+4+pad4+8 = 48 (x64 pack 8)
const _: () = assert!(std::mem::size_of::<NvDisplayConfigPathInfo>() == 48);

/// Version constant for NV_DISPLAYCONFIG_PATH_INFO_V2 (MAKE_NVAPI_VERSION(type, 2))
const NV_DISPLAYCONFIG_PATH_INFO_VER: NvU32 =
    (std::mem::size_of::<NvDisplayConfigPathInfo>() as NvU32) | (2u32 << 16);



/// Flags for NvAPI_DISP_SetDisplayConfig
#[allow(dead_code)]
const NV_DISPLAYCONFIG_VALIDATE_ONLY: NvU32          = 0x00000001;
const NV_DISPLAYCONFIG_SAVE_TO_PERSISTENCE: NvU32    = 0x00000002;
#[allow(dead_code)]
const NV_DISPLAYCONFIG_DRIVER_RELOAD_ALLOWED: NvU32  = 0x00000004;
#[allow(dead_code)]
const NV_DISPLAYCONFIG_FORCE_MODE_ENUMERATION: NvU32 = 0x00000008;
const NV_FORCE_COMMIT_VIDPN: NvU32                   = 0x00000010;

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

    // ── DISP (Display Control) API ──
    /// Retrieve current global display configuration.
    /// Two-pass: first call with pathInfo=NULL to get pathInfoCount,
    /// then allocate pathInfo array and call again.
    fn NvAPI_DISP_GetDisplayConfig(
        pathInfoCount: *mut NvU32,
        pathInfo: *mut NvDisplayConfigPathInfo,
    ) -> NvAPI_Status;

    /// Apply a new global display configuration (resolution, position, etc.).
    fn NvAPI_DISP_SetDisplayConfig(
        pathInfoCount: NvU32,
        pathInfo: *const NvDisplayConfigPathInfo,
        flags: NvU32,
    ) -> NvAPI_Status;

    /// Get displayId from a display name string (ANSI).
    #[allow(dead_code)]
    fn NvAPI_DISP_GetDisplayIdByDisplayName(
        displayName: *const u8,
        displayId: *mut NvU32,
    ) -> NvAPI_Status;

    /// Get the output ID from a display handle.
    #[allow(dead_code)]
    fn NvAPI_GetAssociatedDisplayOutputId(
        hDisplay: NvDisplayHandle,
        pOutputId: *mut NvU32,
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

// === 同步与显示 ===
const SETTING_VSYNCMODE: NvU32 = 0x00A879CF;
const SETTING_FRL_FPS: NvU32 = 0x10835002;
const SETTING_VRR_APP_OVERRIDE: NvU32 = 0x10A879CF;
const SETTING_VRR_MODE: NvU32 = 0x1194F158;
const SETTING_REFRESH_RATE_OVERRIDE: NvU32 = 0x0064B541;
const SETTING_VSYNCTEARCONTROL: NvU32 = 0x005A375C;
const SETTING_OGL_TRIPLE_BUFFER: NvU32 = 0x20FDD1F9;

// === 画质与纹理 ===
const SETTING_QUALITY_ENHANCEMENTS: NvU32 = 0x00CE2691;
const SETTING_ANISO_MODE_LEVEL: NvU32 = 0x101E61A9;
const SETTING_AA_MODE_METHOD: NvU32 = 0x10D773D2;
const SETTING_FXAA_ENABLE: NvU32 = 0x1074C972;
const SETTING_AO_MODE: NvU32 = 0x00667329;
const SETTING_MAXWELL_B_SAMPLE_INTERLEAVE: NvU32 = 0x0098C1AC;
const SETTING_PS_SHADERDISKCACHE: NvU32 = 0x00198FFF;
const SETTING_AA_MODE_SELECTOR: NvU32 = 0x107EFC5B;
const SETTING_AA_MODE_ALPHATOCOVERAGE: NvU32 = 0x10FC2D9C;
const SETTING_PS_TEXFILTER_ANISO_OPTS2: NvU32 = 0x00E73211;
const SETTING_PS_TEXFILTER_NO_NEG_LODBIAS: NvU32 = 0x0019BB68;
const SETTING_AUTO_LODBIASADJUST: NvU32 = 0x00638E8F;
const SETTING_PS_TEXFILTER_BILINEAR_IN_ANISO: NvU32 = 0x0084CD70;
const SETTING_PS_TEXFILTER_DISABLE_TRILIN_SLOPE: NvU32 = 0x002ECAF2;

// === 电源与性能 ===
const SETTING_PREFERRED_PSTATE: NvU32 = 0x1057EB71;
const SETTING_PRERENDERLIMIT: NvU32 = 0x007BA09E;
const SETTING_OGL_THREAD_CONTROL: NvU32 = 0x20C1221E;
const SETTING_VRPRERENDERLIMIT: NvU32 = 0x10111133;
const SETTING_BATTERY_BOOST_APP_FPS: NvU32 = 0x10115C8C;

// === 同步与显示 (追加) ===
const SETTING_VSYNC_BEHAVIOR_FLAGS: NvU32 = 0x10FDEC23;
const SETTING_VSYNCVRRCONTROL: NvU32 = 0x10A879CE;

// === 画质与纹理 (追加) ===
const SETTING_AA_BEHAVIOR_FLAGS: NvU32 = 0x10ECDB82;
const SETTING_AA_MODE_REPLAY: NvU32 = 0x10D48A85;
const SETTING_ANISO_MODE_SELECTOR: NvU32 = 0x10D2BB16;
const SETTING_PREVENT_UI_AF_OVERRIDE: NvU32 = 0x103BCCB5;

// === 指示器与覆盖 ===
const SETTING_VRRFEATUREINDICATOR: NvU32 = 0x1094F157;
const SETTING_VRROVERLAYINDICATOR: NvU32 = 0x1095F16F;
const SETTING_VRRREQUESTSTATE: NvU32 = 0x1094F1F7;
const SETTING_FXAA_INDICATOR_ENABLE: NvU32 = 0x1068FB9C;
const SETTING_PHYSXINDICATOR: NvU32 = 0x1094F16F;
const SETTING_EXPORT_PERF_COUNTERS: NvU32 = 0x108F0841;

// === 电源与性能 (追加) ===
const SETTING_EXTERNAL_QUIET_MODE: NvU32 = 0x10115C8D;

// === 新增常用设置 ===
const SETTING_FXAA_ALLOW: NvU32 = 0x1034CB89;
const SETTING_QUALITY_ENHANCEMENT_SUBSTITUTION: NvU32 = 0x00CE2692;
const SETTING_AO_MODE_ACTIVE: NvU32 = 0x00664339;
const SETTING_VSYNCSMOOTHAFR: NvU32 = 0x101AE763;
const SETTING_LATENCY_INDICATOR_AUTOALIGN: NvU32 = 0x1095F170;
const SETTING_PS_SHADERDISKCACHE_MAX_SIZE: NvU32 = 0x00AC8497;

const TARGET_SETTINGS: &[NvU32] = &[
    // 同步与显示
    SETTING_VSYNCMODE,
    SETTING_FRL_FPS,
    SETTING_VRR_APP_OVERRIDE,
    SETTING_VRR_MODE,
    SETTING_REFRESH_RATE_OVERRIDE,
    SETTING_VSYNCTEARCONTROL,
    SETTING_OGL_TRIPLE_BUFFER,
    // 画质与纹理
    SETTING_QUALITY_ENHANCEMENTS,
    SETTING_ANISO_MODE_LEVEL,
    SETTING_AA_MODE_METHOD,
    SETTING_FXAA_ENABLE,
    SETTING_AO_MODE,
    SETTING_MAXWELL_B_SAMPLE_INTERLEAVE,
    SETTING_PS_SHADERDISKCACHE,
    SETTING_AA_MODE_SELECTOR,
    SETTING_AA_MODE_ALPHATOCOVERAGE,
    SETTING_PS_TEXFILTER_ANISO_OPTS2,
    SETTING_PS_TEXFILTER_NO_NEG_LODBIAS,
    SETTING_AUTO_LODBIASADJUST,
    SETTING_PS_TEXFILTER_BILINEAR_IN_ANISO,
    SETTING_PS_TEXFILTER_DISABLE_TRILIN_SLOPE,
    // 电源与性能
    SETTING_PREFERRED_PSTATE,
    SETTING_PRERENDERLIMIT,
    SETTING_OGL_THREAD_CONTROL,
    SETTING_VRPRERENDERLIMIT,
    SETTING_BATTERY_BOOST_APP_FPS,
    SETTING_EXTERNAL_QUIET_MODE,
    // 同步与显示 (追加)
    SETTING_VSYNC_BEHAVIOR_FLAGS,
    SETTING_VSYNCVRRCONTROL,
    // 画质与纹理 (追加)
    SETTING_AA_BEHAVIOR_FLAGS,
    SETTING_AA_MODE_REPLAY,
    SETTING_ANISO_MODE_SELECTOR,
    SETTING_PREVENT_UI_AF_OVERRIDE,
    // 指示器与覆盖
    SETTING_VRRFEATUREINDICATOR,
    SETTING_VRROVERLAYINDICATOR,
    SETTING_VRRREQUESTSTATE,
    SETTING_FXAA_INDICATOR_ENABLE,
    SETTING_PHYSXINDICATOR,
    SETTING_EXPORT_PERF_COUNTERS,
    // 新增常用
    SETTING_FXAA_ALLOW,
    SETTING_QUALITY_ENHANCEMENT_SUBSTITUTION,
    SETTING_AO_MODE_ACTIVE,
    SETTING_VSYNCSMOOTHAFR,
    SETTING_LATENCY_INDICATOR_AUTOALIGN,
    SETTING_PS_SHADERDISKCACHE_MAX_SIZE,
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
            // ---- 同步与显示 ----
            (
                SETTING_VRR_APP_OVERRIDE,
                "G-Sync",
                vec![
                    SettingOption { value: 0, label: "允许".into() },
                    SettingOption { value: 1, label: "强制关闭".into() },
                    SettingOption { value: 2, label: "禁止".into() },
                    SettingOption { value: 3, label: "ULMB".into() },
                    SettingOption { value: 4, label: "固定刷新".into() },
                ],
            ),
            (
                SETTING_VRR_MODE,
                "G-Sync 全局启用",
                vec![
                    SettingOption { value: 0, label: "关闭".into() },
                    SettingOption { value: 1, label: "仅全屏".into() },
                    SettingOption { value: 2, label: "全屏和窗口".into() },
                ],
            ),
            (
                SETTING_REFRESH_RATE_OVERRIDE,
                "首选刷新率",
                vec![
                    SettingOption { value: 0, label: "应用程序控制".into() },
                    SettingOption { value: 1, label: "最高可用".into() },
                ],
            ),
            (
                SETTING_VSYNCTEARCONTROL,
                "垂直同步撕裂控制",
                vec![
                    SettingOption { value: 0x96861077, label: "禁用".into() },
                    SettingOption { value: 0x99941284, label: "启用".into() },
                ],
            ),
            (
                SETTING_OGL_TRIPLE_BUFFER,
                "OpenGL 三重缓冲",
                vec![
                    SettingOption { value: 0, label: "关闭".into() },
                    SettingOption { value: 1, label: "开启".into() },
                ],
            ),
            // ---- 画质与纹理 ----
            (
                SETTING_AO_MODE,
                "环境光遮蔽",
                vec![
                    SettingOption { value: 0, label: "关闭".into() },
                    SettingOption { value: 1, label: "低".into() },
                    SettingOption { value: 2, label: "中".into() },
                    SettingOption { value: 3, label: "高".into() },
                ],
            ),
            (
                SETTING_MAXWELL_B_SAMPLE_INTERLEAVE,
                "MFAA 多帧采样",
                vec![
                    SettingOption { value: 0, label: "关闭".into() },
                    SettingOption { value: 1, label: "开启".into() },
                ],
            ),
            (
                SETTING_PS_SHADERDISKCACHE,
                "着色器缓存",
                vec![
                    SettingOption { value: 0, label: "关闭".into() },
                    SettingOption { value: 1, label: "开启".into() },
                ],
            ),
            (
                SETTING_AA_MODE_SELECTOR,
                "抗锯齿 - 模式选择",
                vec![
                    SettingOption { value: 0, label: "应用程序控制".into() },
                    SettingOption { value: 1, label: "覆盖应用设置".into() },
                    SettingOption { value: 2, label: "增强应用设置".into() },
                ],
            ),
            (
                SETTING_AA_MODE_ALPHATOCOVERAGE,
                "抗锯齿 - 透明度多重采样",
                vec![
                    SettingOption { value: 0, label: "关闭".into() },
                    SettingOption { value: 4, label: "开启".into() },
                ],
            ),
            (
                SETTING_PS_TEXFILTER_ANISO_OPTS2,
                "纹理过滤 - 各向异性采样优化",
                vec![
                    SettingOption { value: 0, label: "关闭".into() },
                    SettingOption { value: 1, label: "开启".into() },
                ],
            ),
            (
                SETTING_PS_TEXFILTER_NO_NEG_LODBIAS,
                "纹理过滤 - 负 LOD 偏移",
                vec![
                    SettingOption { value: 0, label: "允许".into() },
                    SettingOption { value: 1, label: "拒绝".into() },
                ],
            ),
            (
                SETTING_AUTO_LODBIASADJUST,
                "纹理过滤 - 驱动控制 LOD 偏移",
                vec![
                    SettingOption { value: 0, label: "关闭".into() },
                    SettingOption { value: 1, label: "开启".into() },
                ],
            ),
            (
                SETTING_PS_TEXFILTER_BILINEAR_IN_ANISO,
                "纹理过滤 - 各向异性过滤优化",
                vec![
                    SettingOption { value: 0, label: "关闭".into() },
                    SettingOption { value: 1, label: "开启".into() },
                ],
            ),
            (
                SETTING_PS_TEXFILTER_DISABLE_TRILIN_SLOPE,
                "纹理过滤 - 三线性优化",
                vec![
                    SettingOption { value: 0, label: "关闭".into() },
                    SettingOption { value: 1, label: "开启".into() },
                ],
            ),
            // ---- 电源与性能 ----
            (
                SETTING_PRERENDERLIMIT,
                "最大预渲染帧数",
                vec![
                    SettingOption { value: 0, label: "应用程序控制".into() },
                    SettingOption { value: 1, label: "1".into() },
                    SettingOption { value: 2, label: "2".into() },
                    SettingOption { value: 3, label: "3".into() },
                    SettingOption { value: 4, label: "4".into() },
                ],
            ),
            (
                SETTING_OGL_THREAD_CONTROL,
                "OpenGL 线程优化",
                vec![
                    SettingOption { value: 0, label: "自动".into() },
                    SettingOption { value: 1, label: "启用".into() },
                    SettingOption { value: 2, label: "禁用".into() },
                ],
            ),
            (
                SETTING_VRPRERENDERLIMIT,
                "VR 预渲染帧数",
                vec![
                    SettingOption { value: 0, label: "应用程序控制".into() },
                    SettingOption { value: 1, label: "1".into() },
                ],
            ),
            (
                SETTING_BATTERY_BOOST_APP_FPS,
                "电池加速帧率限制",
                vec![
                    SettingOption { value: 0, label: "关闭".into() },
                    SettingOption { value: 30, label: "30 FPS".into() },
                    SettingOption { value: 60, label: "60 FPS".into() },
                ],
            ),
            // ---- 同步与显示 (追加) ----
            (
                SETTING_VSYNC_BEHAVIOR_FLAGS,
                "垂直同步 - 行为标志",
                vec![
                    SettingOption { value: 0x00000000, label: "默认".into() },
                    SettingOption { value: 0x00000001, label: "忽略交换间隔倍数".into() },
                ],
            ),
            (
                SETTING_VSYNCVRRCONTROL,
                "可变刷新率 (VRR)",
                vec![
                    SettingOption { value: 0x00000000, label: "关闭".into() },
                    SettingOption { value: 0x00000001, label: "开启".into() },
                    SettingOption { value: 0x9f95128e, label: "不支持".into() },
                ],
            ),
            // ---- 画质与纹理 (追加) ----
            (
                SETTING_AA_BEHAVIOR_FLAGS,
                "抗锯齿 - 行为标志",
                vec![
                    SettingOption { value: 0x00000000, label: "无".into() },
                    SettingOption { value: 0x00000001, label: "覆盖视为应用控制".into() },
                    SettingOption { value: 0x00000002, label: "覆盖视为增强".into() },
                    SettingOption { value: 0x00000003, label: "禁用覆盖".into() },
                    SettingOption { value: 0x00000004, label: "增强视为应用控制".into() },
                    SettingOption { value: 0x00000008, label: "增强视为覆盖".into() },
                    SettingOption { value: 0x0000000c, label: "禁用增强".into() },
                ],
            ),
            (
                SETTING_AA_MODE_REPLAY,
                "抗锯齿 - 透明度超级采样",
                vec![
                    SettingOption { value: 0x00000000, label: "关闭".into() },
                    SettingOption { value: 0x00000010, label: "2x 采样".into() },
                    SettingOption { value: 0x00000020, label: "4x 采样".into() },
                    SettingOption { value: 0x00000030, label: "8x 采样".into() },
                ],
            ),
            (
                SETTING_ANISO_MODE_SELECTOR,
                "各向异性过滤模式",
                vec![
                    SettingOption { value: 0x00000000, label: "应用程序控制".into() },
                    SettingOption { value: 0x00000001, label: "用户自定义".into() },
                    SettingOption { value: 0x00000002, label: "条件".into() },
                ],
            ),
            (
                SETTING_PREVENT_UI_AF_OVERRIDE,
                "禁止覆盖各向异性过滤",
                vec![
                    SettingOption { value: 0, label: "关闭".into() },
                    SettingOption { value: 1, label: "开启".into() },
                ],
            ),

            // ---- 指示器与覆盖 ----
            (
                SETTING_VRRFEATUREINDICATOR,
                "G-Sync 功能指示器",
                vec![
                    SettingOption { value: 0x0, label: "关闭".into() },
                    SettingOption { value: 0x1, label: "开启".into() },
                ],
            ),
            (
                SETTING_VRROVERLAYINDICATOR,
                "G-Sync 叠加指示器",
                vec![
                    SettingOption { value: 0x0, label: "关闭".into() },
                    SettingOption { value: 0x1, label: "开启".into() },
                ],
            ),
            (
                SETTING_VRRREQUESTSTATE,
                "G-Sync 请求状态",
                vec![
                    SettingOption { value: 0x0, label: "关闭".into() },
                    SettingOption { value: 0x1, label: "仅全屏".into() },
                    SettingOption { value: 0x2, label: "全屏和窗口".into() },
                ],
            ),
            (
                SETTING_FXAA_INDICATOR_ENABLE,
                "FXAA 指示器",
                vec![
                    SettingOption { value: 0, label: "关闭".into() },
                    SettingOption { value: 1, label: "开启".into() },
                ],
            ),
            (
                SETTING_PHYSXINDICATOR,
                "PhysX 指示器",
                vec![
                    SettingOption { value: 0x34534064, label: "关闭".into() },
                    SettingOption { value: 0x24545582, label: "开启".into() },
                ],
            ),
            (
                SETTING_EXPORT_PERF_COUNTERS,
                "性能计数器导出",
                vec![
                    SettingOption { value: 0x00000000, label: "关闭".into() },
                    SettingOption { value: 0x00000001, label: "开启".into() },
                ],
            ),
            (
                SETTING_EXTERNAL_QUIET_MODE,
                "外部静音模式 (XQM)",
                vec![
                    SettingOption { value: 0x00000000, label: "关闭".into() },
                    SettingOption { value: 0x00000001, label: "开启".into() },
                ],
            ),
            // ---- 新增常用设置 ----
            (
                SETTING_FXAA_ALLOW,
                "FXAA 允许",
                vec![
                    SettingOption { value: 0, label: "不允许".into() },
                    SettingOption { value: 1, label: "允许".into() },
                ],
            ),
            (
                SETTING_QUALITY_ENHANCEMENT_SUBSTITUTION,
                "纹理过滤 - 质量替换",
                vec![
                    SettingOption { value: 0x00000000, label: "无替换".into() },
                    SettingOption { value: 0x00000001, label: "高质量→质量".into() },
                ],
            ),
            (
                SETTING_AO_MODE_ACTIVE,
                "环境光遮蔽激活",
                vec![
                    SettingOption { value: 0, label: "关闭".into() },
                    SettingOption { value: 1, label: "开启".into() },
                ],
            ),
            (
                SETTING_VSYNCSMOOTHAFR,
                "平滑 AFR 行为",
                vec![
                    SettingOption { value: 0x00000000, label: "关闭".into() },
                    SettingOption { value: 0x00000001, label: "开启".into() },
                ],
            ),
            (
                SETTING_LATENCY_INDICATOR_AUTOALIGN,
                "Reflex 指示器自动对齐",
                vec![
                    SettingOption { value: 0x0, label: "关闭".into() },
                    SettingOption { value: 0x1, label: "开启".into() },
                ],
            ),
            (
                SETTING_PS_SHADERDISKCACHE_MAX_SIZE,
                "着色器缓存最大尺寸",
                vec![
                    SettingOption { value: 0x0, label: "无限制".into() },
                    SettingOption { value: 0x1000, label: "4 GB (默认)".into() },
                    SettingOption { value: 0x2000, label: "8 GB".into() },
                    SettingOption { value: 0x4000, label: "16 GB".into() },
                    SettingOption { value: 0x8000, label: "32 GB".into() },
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
                SETTING_FRL_FPS => "限制游戏最大帧率，降低功耗",
                SETTING_VRR_APP_OVERRIDE => "G-Sync 应用级覆盖控制",
                SETTING_VRR_MODE => "G-Sync 全局启用范围",
                SETTING_REFRESH_RATE_OVERRIDE => "选择首选刷新率策略",
                SETTING_VSYNCTEARCONTROL => "控制垂直同步撕裂补偿",
                SETTING_OGL_TRIPLE_BUFFER => "OpenGL 三重缓冲开关",
                SETTING_QUALITY_ENHANCEMENTS => "纹理过滤的全局画质等级",
                SETTING_ANISO_MODE_LEVEL => "增强斜角纹理的清晰度",
                SETTING_AA_MODE_METHOD => "平滑物体边缘锯齿",
                SETTING_FXAA_ENABLE => "快速且低开销的抗锯齿技术",
                SETTING_AO_MODE => "模拟物体间环境光遮蔽阴影",
                SETTING_MAXWELL_B_SAMPLE_INTERLEAVE => "MFAA 多帧抗锯齿技术",
                SETTING_PS_SHADERDISKCACHE => "着色器缓存减少卡顿",
                SETTING_AA_MODE_SELECTOR => "抗锯齿覆盖/增强模式",
                SETTING_AA_MODE_ALPHATOCOVERAGE => "透明纹理的抗锯齿处理",
                SETTING_PS_TEXFILTER_ANISO_OPTS2 => "各向异性过滤采样优化",
                SETTING_PS_TEXFILTER_NO_NEG_LODBIAS => "控制负 LOD 偏移行为",
                SETTING_AUTO_LODBIASADJUST => "驱动自动调节 LOD 偏移",
                SETTING_PS_TEXFILTER_BILINEAR_IN_ANISO => "各向异性过滤时使用双线性",
                SETTING_PS_TEXFILTER_DISABLE_TRILIN_SLOPE => "禁用三线性过滤优化",
                SETTING_PREFERRED_PSTATE => "控制 GPU 性能和功耗策略",
                SETTING_PRERENDERLIMIT => "CPU 提前渲染的帧数",
                SETTING_OGL_THREAD_CONTROL => "OpenGL 多线程优化",
                SETTING_VRPRERENDERLIMIT => "VR 头显预渲染帧数",
                SETTING_BATTERY_BOOST_APP_FPS => "电池供电时帧率限制",
                SETTING_VSYNC_BEHAVIOR_FLAGS => "控制垂直同步行为标志",
                SETTING_VSYNCVRRCONTROL => "可变刷新率开关",
                SETTING_AA_BEHAVIOR_FLAGS => "抗锯齿覆盖/增强行为控制",
                SETTING_AA_MODE_REPLAY => "透明纹理超级采样模式",
                SETTING_ANISO_MODE_SELECTOR => "各向异性过滤控制模式",
                SETTING_PREVENT_UI_AF_OVERRIDE => "禁止程序覆盖各向异性过滤",

                SETTING_VRRFEATUREINDICATOR => "G-Sync 功能状态指示图标",
                SETTING_VRROVERLAYINDICATOR => "G-Sync 刷新率叠加层",
                SETTING_VRRREQUESTSTATE => "G-Sync 请求启用模式",
                SETTING_FXAA_INDICATOR_ENABLE => "FXAA 激活状态指示图标",
                SETTING_PHYSXINDICATOR => "PhysX 状态指示图标",
                SETTING_EXPORT_PERF_COUNTERS => "导出性能计数器数据",
                SETTING_EXTERNAL_QUIET_MODE => "外部静音模式功耗控制",
                SETTING_FXAA_ALLOW => "控制应用是否可使用 FXAA 抗锯齿",
                SETTING_QUALITY_ENHANCEMENT_SUBSTITUTION => "纹理过滤质量降级策略",
                SETTING_AO_MODE_ACTIVE => "环境光遮蔽全局启用开关",
                SETTING_VSYNCSMOOTHAFR => "多 GPU 交替帧渲染优化",
                SETTING_LATENCY_INDICATOR_AUTOALIGN => "NVIDIA Reflex 延迟标记自动对齐",
                SETTING_PS_SHADERDISKCACHE_MAX_SIZE => "着色器缓存磁盘使用上限",
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
// NVAPI DISP Display Control Commands
// ---------------------------------------------------------------------------

#[derive(serde::Serialize, Clone)]
pub struct NvidiaDisplay {
    pub display_id: NvU32,
    pub device_name: String,
    pub monitor_name: String,
    pub is_primary: bool,
    pub current_width: i32,
    pub current_height: i32,
}

#[derive(serde::Serialize, Clone)]
pub struct DisplayMode {
    pub width: u32,
    pub height: u32,
    pub refresh_rate: f64,
    pub is_current: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct InjectedResolution {
    pub width: u32,
    pub height: u32,
}

#[derive(serde::Serialize)]
pub struct SetResolutionResult {
    pub applied: bool,
    pub injected: bool,
}

fn injected_resolutions_path() -> std::path::PathBuf {
    let config_dir = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    config_dir.join("NexBox").join("injected_resolutions.json")
}

fn load_injected_resolutions() -> Vec<InjectedResolution> {
    let path = injected_resolutions_path();
    if !path.exists() {
        return Vec::new();
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_injected_resolution(width: u32, height: u32) -> Result<(), String> {
    let path = injected_resolutions_path();
    let mut list = load_injected_resolutions();
    let entry = InjectedResolution { width, height };
    if !list.contains(&entry) {
        list.push(entry);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
    }
    std::fs::write(
        &path,
        serde_json::to_string(&list).map_err(|e| format!("序列化失败: {}", e))?,
    )
    .map_err(|e| format!("写入失败: {}", e))?;
    Ok(())
}

/// Check if a monitor name is a generic/placeholder (any language variant)
fn is_generic_monitor_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("generic")
        || lower.contains("即插即用")
        || lower.contains("通用")
        || lower.contains("pnp")
        || lower.contains("standard monitor")
        || lower.contains("digital display")
        || lower.contains("analog display")
}

/// UTF-16LE + Base64 编码 PowerShell，绕过系统代码页
fn encode_ps_command(script: &str) -> String {
    let utf16: Vec<u8> = script.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
    let mut out = String::with_capacity((utf16.len() + 2) / 3 * 4);
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    for chunk in utf16.chunks(3) {
        let b = [chunk[0] as u32, *chunk.get(1).unwrap_or(&0) as u32, *chunk.get(2).unwrap_or(&0) as u32];
        let n = (b[0] << 16) | (b[1] << 8) | b[2];
        out.push(CHARS[((n >> 18) & 0x3F) as usize] as char);
        out.push(CHARS[((n >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 { CHARS[((n >> 6) & 0x3F) as usize] } else { b'=' } as char);
        out.push(if chunk.len() > 2 { CHARS[(n & 0x3F) as usize] } else { b'=' } as char);
    }
    out
}

/// 通过 EDID 获取真实显示器型号，返回结果按 GDI 枚举顺序排列
fn query_edid_monitor_names() -> Vec<String> {
    #[allow(non_snake_case)]
    #[derive(serde::Deserialize)]
    struct PsWmiMonitorId { UserFriendlyName: Option<String> }

    let cmd = "ConvertTo-Json -Compress @(Get-CimInstance -Namespace root\\wmi WmiMonitorID | ForEach-Object { $friendly = ''; if ($_.UserFriendlyNameLength -gt 0) { $arr = @($_.UserFriendlyName); $max = [Math]::Min($arr.Count, $_.UserFriendlyNameLength); for ($i = 0; $i -lt $max; $i++) { $c = [char]$arr[$i]; if ($c -eq [char]0) { break } $friendly += $c } }; [PSCustomObject]@{ UserFriendlyName = $friendly.Trim() } })";
    let full = format!("[Console]::OutputEncoding = [Text.Encoding]::UTF8; {}", cmd);
    let encoded = encode_ps_command(&full);

    let output = match Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &encoded])
        .output() {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
            _ => return Vec::new(),
        };

    serde_json::from_str::<Vec<PsWmiMonitorId>>(&output)
        .unwrap_or_default()
        .into_iter()
        .map(|m| m.UserFriendlyName.unwrap_or_default())
        .collect()
}

/// Enumerate NVIDIA displays with real NVAPI displayId from GetDisplayConfig.
#[tauri::command]
pub fn list_nvidia_displays() -> Result<Vec<NvidiaDisplay>, String> {
    #[cfg(not(target_os = "windows"))]
    {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        use std::mem;
        use windows_sys::Win32::Graphics::Gdi::{
            EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
            EnumDisplayDevicesW, DISPLAY_DEVICEW,
        };

        try_init_nvapi()?;

        // ── Step 1: Collect GDI monitor info ──
        struct GdiMonitor {
            device_name: String,
            monitor_name: String,
            is_primary: bool,
            width: i32,
            height: i32,
        }

        struct MonitorData {
            monitors: Vec<GdiMonitor>,
        }

        unsafe extern "system" fn monitor_enum_proc(
            hmonitor: HMONITOR,
            _hdc: HDC,
            _rect: *mut windows_sys::Win32::Foundation::RECT,
            lparam: isize,
        ) -> i32 {
            let data = &mut *(lparam as *mut MonitorData);
            let mut info: MONITORINFOEXW = mem::zeroed();
            info.monitorInfo.cbSize = mem::size_of::<MONITORINFOEXW>() as u32;

            if GetMonitorInfoW(hmonitor, &mut info as *mut _ as *mut _) != 0 {
                let device_name = String::from_utf16_lossy(
                    &info.szDevice[..info
                        .szDevice
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(info.szDevice.len())],
                );
                let is_primary = (info.monitorInfo.dwFlags & 1) != 0;
                let width = info.monitorInfo.rcMonitor.right
                    - info.monitorInfo.rcMonitor.left;
                let height = info.monitorInfo.rcMonitor.bottom
                    - info.monitorInfo.rcMonitor.top;

                let monitor_name = {
                    let wide: Vec<u16> =
                        device_name.encode_utf16().chain(std::iter::once(0)).collect();
                    let mut dd: DISPLAY_DEVICEW = mem::zeroed();
                    dd.cb = mem::size_of::<DISPLAY_DEVICEW>() as u32;
                    if EnumDisplayDevicesW(wide.as_ptr(), 0, &mut dd, 0) != 0 {
                        let len = dd.DeviceString
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(dd.DeviceString.len());
                        if len > 0 {
                            let model = String::from_utf16_lossy(&dd.DeviceString[..len]);
                            let t = model.trim();
                            if !t.is_empty() && !is_generic_monitor_name(t) {
                                t.to_string()
                            } else {
                                device_name.trim_start_matches("\\\\.\\").to_string()
                            }
                        } else {
                            device_name.trim_start_matches("\\\\.\\").to_string()
                        }
                    } else {
                        device_name.trim_start_matches("\\\\.\\").to_string()
                    }
                };

                data.monitors.push(GdiMonitor {
                    device_name,
                    monitor_name,
                    is_primary,
                    width,
                    height,
                });
            }
            1
        }

        let mut data = MonitorData {
            monitors: Vec::new(),
        };
        unsafe {
            EnumDisplayMonitors(
                std::ptr::null_mut(),
                std::ptr::null(),
                Some(monitor_enum_proc),
                &mut data as *mut _ as isize,
            );
        }

        if data.monitors.is_empty() {
            return Err("未检测到任何显示器".to_string());
        }

        // ── Step 2: Get real NVAPI displayIds from GetDisplayConfig ──
        let nvapi_display_ids: Vec<NvU32> = unsafe {
            let mut path_info_count: NvU32 = 0;
            let mut status =
                NvAPI_DISP_GetDisplayConfig(&mut path_info_count, std::ptr::null_mut());
            if status != NVAPI_OK || path_info_count == 0 {
                log::warn!("GetDisplayConfig pass1 failed: status={}", status);
                return Ok(data
                    .monitors
                    .into_iter()
                    .enumerate()
                    .map(|(i, m)| NvidiaDisplay {
                        display_id: i as NvU32,
                        device_name: m.device_name,
                        monitor_name: m.monitor_name,
                        is_primary: m.is_primary,
                        current_width: m.width,
                        current_height: m.height,
                    })
                    .collect());
            }

            // Pass 2: allocate sourceModeInfo
            let mut source_mode_infos: Vec<NvDisplayConfigSourceModeInfo> =
                (0..path_info_count as usize)
                    .map(|_| {
                        let mut sm: NvDisplayConfigSourceModeInfo = mem::zeroed();
                        sm.colorFormat = 0;
                        sm
                    })
                    .collect();

            let mut path_infos: Vec<NvDisplayConfigPathInfo> = (0..path_info_count as usize)
                .map(|i| {
                    let mut p: NvDisplayConfigPathInfo = mem::zeroed();
                    p.version = NV_DISPLAYCONFIG_PATH_INFO_VER;
                    p.sourceModeInfo = &mut source_mode_infos[i];
                    p
                })
                .collect();

            status = NvAPI_DISP_GetDisplayConfig(&mut path_info_count, path_infos.as_mut_ptr());
            if status != NVAPI_OK {
                log::warn!("GetDisplayConfig pass2 failed: status={}", status);
                return Ok(data
                    .monitors
                    .into_iter()
                    .enumerate()
                    .map(|(i, m)| NvidiaDisplay {
                        display_id: i as NvU32,
                        device_name: m.device_name,
                        monitor_name: m.monitor_name,
                        is_primary: m.is_primary,
                        current_width: m.width,
                        current_height: m.height,
                    })
                    .collect());
            }

            // Pass 3: allocate targetInfo
            let mut target_allocations: Vec<Vec<NvDisplayConfigPathTargetInfo>> = Vec::new();
            for path in &mut path_infos {
                let count = path.targetInfoCount as usize;
                if count > 0 {
                    let targets: Vec<NvDisplayConfigPathTargetInfo> =
                        (0..count).map(|_| mem::zeroed()).collect();
                    path.targetInfo = targets.as_ptr() as *mut NvDisplayConfigPathTargetInfo;
                    target_allocations.push(targets);
                } else {
                    path.targetInfo = std::ptr::null_mut();
                }
            }

            status = NvAPI_DISP_GetDisplayConfig(&mut path_info_count, path_infos.as_mut_ptr());
            if status != NVAPI_OK {
                log::warn!("GetDisplayConfig pass3 failed: status={}", status);
                return Ok(data
                    .monitors
                    .into_iter()
                    .enumerate()
                    .map(|(i, m)| NvidiaDisplay {
                        display_id: i as NvU32,
                        device_name: m.device_name,
                        monitor_name: m.monitor_name,
                        is_primary: m.is_primary,
                        current_width: m.width,
                        current_height: m.height,
                    })
                    .collect());
            }

            // Extract displayIds from paths (first targetInfo per path)
            path_infos
                .iter()
                .map(|p| {
                    if p.targetInfoCount > 0 && !p.targetInfo.is_null() {
                        (*p.targetInfo).displayId
                    } else {
                        0
                    }
                })
                .collect()
        };

        log::info!(
            "list_nvidia_displays: {} GDI monitors, {} NVAPI paths, display_ids={:?}",
            data.monitors.len(),
            nvapi_display_ids.len(),
            nvapi_display_ids
        );

        // ── Step 3: Combine GDI info with real NVAPI displayIds ──
        // Match by order: GDI monitor N → NVAPI path N (typically correct for NVIDIA)
        let mut displays: Vec<NvidiaDisplay> = data
            .monitors
            .into_iter()
            .enumerate()
            .map(|(i, m)| {
                let display_id = nvapi_display_ids.get(i).copied().unwrap_or(i as NvU32);
                NvidiaDisplay {
                    display_id,
                    device_name: m.device_name,
                    monitor_name: m.monitor_name,
                    is_primary: m.is_primary,
                    current_width: m.width,
                    current_height: m.height,
                }
            })
            .collect();

        // EDID 回退：如果 EnumDisplayDevicesW 返回通用名称，
        // monitor_name 会退化为 GDI 设备名（如 DISPLAY1）。
        let has_fallback = displays.iter().any(|d| {
            let stripped = d.device_name.trim_start_matches("\\\\.\\");
            d.monitor_name == stripped
        });
        if has_fallback {
            let edid_names = query_edid_monitor_names();
            if !edid_names.is_empty() {
                for (i, d) in displays.iter_mut().enumerate() {
                    let stripped = d.device_name.trim_start_matches("\\\\.\\");
                    if d.monitor_name == stripped {
                        if let Some(name) = edid_names.get(i) {
                            if !name.is_empty() {
                                d.monitor_name = name.clone();
                            }
                        } else if edid_names.len() == 1 && !edid_names[0].is_empty() {
                            d.monitor_name = edid_names[0].clone();
                        }
                    }
                }
            }
        }

        Ok(displays)
    }
}

/// Enumerate all available display modes (resolution + refresh rate) for a given GDI device name.
#[tauri::command]
pub fn get_nvidia_display_modes(device_name: String) -> Result<Vec<DisplayMode>, String> {
    #[cfg(not(target_os = "windows"))]
    {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        use std::mem;
        use windows_sys::Win32::Graphics::Gdi::{EnumDisplaySettingsW, DEVMODEW};

        let wide_name: Vec<u16> =
            device_name.encode_utf16().chain(std::iter::once(0)).collect();
        let mut modes: Vec<DisplayMode> = Vec::new();
        let mut mode_num: u32 = 0;

        loop {
            let mut dev_mode: DEVMODEW = unsafe { mem::zeroed() };
            dev_mode.dmSize = mem::size_of::<DEVMODEW>() as u16;

            let result =
                unsafe { EnumDisplaySettingsW(wide_name.as_ptr(), mode_num, &mut dev_mode) };

            if result == 0 {
                break;
            }

            // Calculate effective refresh rate
            // dmDisplayFrequency may be exact (e.g. 60, 144) or fractional (e.g. 5995 → 59.95)
            let freq_raw = dev_mode.dmDisplayFrequency as f64;
            let refresh_rate = if freq_raw > 200.0 {
                // Assume it's 100x encoded (e.g., 5995 = 59.95 Hz)
                freq_raw / 100.0
            } else {
                freq_raw
            };

            modes.push(DisplayMode {
                width: dev_mode.dmPelsWidth,
                height: dev_mode.dmPelsHeight,
                refresh_rate,
                is_current: false,
            });

            mode_num += 1;
            if mode_num > 300 {
                break;
            }
        }

        if modes.is_empty() {
            return Err(format!("无法枚举显示器 {} 的模式", device_name));
        }

        // Sort: resolution (W*H) desc, then refresh rate desc
        modes.sort_by(|a, b| {
            let area_a = a.width * a.height;
            let area_b = b.width * b.height;
            area_b
                .cmp(&area_a)
                .then(
                    b.refresh_rate
                        .partial_cmp(&a.refresh_rate)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
        });

        // Dedup: same (width, height, approx refresh rate)
        modes.dedup_by(|a, b| {
            a.width == b.width
                && a.height == b.height
                && (a.refresh_rate - b.refresh_rate).abs() < 0.5
        });

        Ok(modes)
    }
}

// ---------------------------------------------------------------------------
// NV_Modes registry injection helpers
// ---------------------------------------------------------------------------

/// Inject a custom resolution into all NVIDIA NV_Modes registry keys
/// under HKLM\SYSTEM\CurrentControlSet\Control\Video\.
///
/// Format: "{width}x{height}x{bpp}-{RRx1000}"
fn inject_nv_modes_registry(device_name: &str, width: u32, height: u32) -> Result<Vec<String>, String> {
    use std::mem;
    use winreg::enums::*;
    use winreg::RegKey;

    // Get current refresh rate via GDI for the NV_Modes entry format
    let refresh_rrx1k: u32 = unsafe {
        let wide: Vec<u16> = device_name.encode_utf16().chain(std::iter::once(0)).collect();
        let mut dm: windows_sys::Win32::Graphics::Gdi::DEVMODEW = mem::zeroed();
        dm.dmSize = mem::size_of_val(&dm) as u16;
        if windows_sys::Win32::Graphics::Gdi::EnumDisplaySettingsW(
            wide.as_ptr(), 0xFFFFFFFF, &mut dm,
        ) != 0
        {
            let freq = dm.dmDisplayFrequency as u32;
            if freq > 200 { freq } else { freq * 1000 }
        } else {
            144000
        }
    };

    let entry = format!("{}x{}x32-{}", width, height, refresh_rrx1k);
    let video_path = r"SYSTEM\CurrentControlSet\Control\Video";
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    let video_key = hklm.open_subkey_with_flags(video_path, KEY_READ)
        .map_err(|e| format!("无法打开注册表路径 {}: {}", video_path, e))?;

    let mut injected: Vec<String> = Vec::new();

    for adapter in video_key.enum_keys().filter_map(|r| r.ok()) {
        let adapter_key = match hklm.open_subkey_with_flags(
            &format!(r"{}\{}", video_path, adapter), KEY_READ,
        ) {
            Ok(k) => k,
            Err(_) => continue,
        };

        for target in adapter_key.enum_keys().filter_map(|r| r.ok()) {
            let target_path = format!(r"{}\{}\{}", video_path, adapter, target);
            let target_key = match hklm.open_subkey_with_flags(
                &target_path, KEY_READ | KEY_WRITE,
            ) {
                Ok(k) => k,
                Err(_) => continue,
            };

            // Check if NV_Modes exists (REG_MULTI_SZ)
            let has_modes = target_key.get_value::<Vec<String>, _>("NV_Modes").is_ok();
            if !has_modes {
                // Try REG_SZ format
                let has_sz = target_key.get_value::<String, _>("NV_Modes").is_ok();
                if !has_sz {
                    continue;
                }
            }

            let mut modes: Vec<String> = target_key.get_value("NV_Modes").unwrap_or_default();
            // Remove stale entry for the same base resolution
            modes.retain(|m| {
                let prefix = format!("{}x{}", width, height);
                !m.starts_with(&prefix)
            });
            // Add our entry if not already present
            if !modes.contains(&entry) {
                modes.push(entry.clone());
            }

            target_key.set_value("NV_Modes", &modes)
                .map_err(|e| format!("写入 NV_Modes 到 {} 失败: {}", target_path, e))?;

            log::info!("已注入分辨率到 NV_Modes: {}  (target={})", entry, target);
            injected.push(target_path);
        }
    }

    if injected.is_empty() {
        // NV_Modes doesn't exist yet — create it under the first suitable NVIDIA target
        for adapter in video_key.enum_keys().filter_map(|r| r.ok()) {
            let adapter_key = match hklm.open_subkey_with_flags(
                &format!(r"{}\{}", video_path, adapter), KEY_READ,
            ) {
                Ok(k) => k,
                Err(_) => continue,
            };

            for target in adapter_key.enum_keys().filter_map(|r| r.ok()) {
                let target_path = format!(r"{}\{}\{}", video_path, adapter, target);
                // Skip known non-target subkeys
                if target == "0000" || target == "Connectivity" || target == "DAL" {
                    continue;
                }
                let target_key = match hklm.create_subkey(&target_path) {
                    Ok((k, _)) => k,
                    Err(_) => continue,
                };
                let modes: Vec<String> = vec![entry.clone()];
                target_key.set_value("NV_Modes", &modes)
                    .map_err(|e| format!("创建 NV_Modes 到 {} 失败: {}", target_path, e))?;
                log::info!("已创建并注入 NV_Modes: {}  (target={})", entry, target);
                injected.push(target_path);
                break;
            }
            if !injected.is_empty() {
                break;
            }
        }
    }

    if injected.is_empty() {
        Err("未找到 NV_Modes 注册表项，且无法创建".to_string())
    } else {
        Ok(injected)
    }
}

/// Re-run NvAPI_DISP_SetDisplayConfig (after NV_Modes injection)
unsafe fn retry_set_display_config(
    display_id: NvU32,
    width: NvU32,
    height: NvU32,
) -> Result<(), String> {
    use std::mem;

    let mut path_info_count: NvU32 = 0;
    let mut s = NvAPI_DISP_GetDisplayConfig(&mut path_info_count, std::ptr::null_mut());
    if s != NVAPI_OK || path_info_count == 0 {
        return Err("retry: GetDisplayConfig 失败".to_string());
    }

    let mut source_mode_infos: Vec<NvDisplayConfigSourceModeInfo> = (0..path_info_count as usize)
        .map(|_| {
            let mut sm: NvDisplayConfigSourceModeInfo = mem::zeroed();
            sm.colorFormat = 0;
            sm
        })
        .collect();

    let mut path_infos: Vec<NvDisplayConfigPathInfo> = (0..path_info_count as usize)
        .map(|i| {
            let mut p: NvDisplayConfigPathInfo = mem::zeroed();
            p.version = NV_DISPLAYCONFIG_PATH_INFO_VER;
            p.sourceModeInfo = &mut source_mode_infos[i];
            p
        })
        .collect();

    s = NvAPI_DISP_GetDisplayConfig(&mut path_info_count, path_infos.as_mut_ptr());
    if s != NVAPI_OK {
        return Err("retry: GetDisplayConfig pass2 失败".to_string());
    }

    // Allocate targetInfo
    let mut target_allocations: Vec<Vec<NvDisplayConfigPathTargetInfo>> = Vec::new();
    for path in &mut path_infos {
        let count = path.targetInfoCount as usize;
        if count > 0 {
            let targets: Vec<NvDisplayConfigPathTargetInfo> =
                (0..count).map(|_| mem::zeroed()).collect();
            path.targetInfo = targets.as_ptr() as *mut NvDisplayConfigPathTargetInfo;
            target_allocations.push(targets);
        } else {
            path.targetInfo = std::ptr::null_mut();
        }
    }

    s = NvAPI_DISP_GetDisplayConfig(&mut path_info_count, path_infos.as_mut_ptr());
    if s != NVAPI_OK {
        return Err("retry: GetDisplayConfig pass3 失败".to_string());
    }

    // Find path for our display_id
    let target_idx = match path_infos.iter().position(|path| {
        if path.targetInfoCount > 0 && !path.targetInfo.is_null() {
            let targets = std::slice::from_raw_parts(path.targetInfo, path.targetInfoCount as usize);
            targets.iter().any(|t| t.displayId == display_id)
        } else {
            false
        }
    }) {
        Some(i) => i,
        None => return Err("retry: 未找到 displayId".to_string()),
    };

    let path = &mut path_infos[target_idx];
    let mode_info = match path.sourceModeInfo.as_mut() {
        Some(m) => m,
        None => return Err("retry: 无 source mode info".to_string()),
    };
    mode_info.resolution.width = width;
    mode_info.resolution.height = height;

    let flags = NV_DISPLAYCONFIG_SAVE_TO_PERSISTENCE | NV_FORCE_COMMIT_VIDPN;
    s = NvAPI_DISP_SetDisplayConfig(path_info_count, path_infos.as_ptr(), flags);
    if s == NVAPI_OK {
        log::info!("retry: SetDisplayConfig 成功设置 {}x{}", width, height);
        Ok(())
    } else {
        Err(format!("retry: SetDisplayConfig 失败: {} (code {})", nvapi_error_string(s), s))
    }
}

/// Get the list of resolutions that have been injected into NV_Modes registry.
#[tauri::command]
pub fn get_injected_resolutions() -> Vec<InjectedResolution> {
    load_injected_resolutions()
}

/// Remove a resolution from the injected resolutions list.
#[tauri::command]
pub fn remove_injected_resolution(width: u32, height: u32) -> Result<(), String> {
    let path = injected_resolutions_path();
    let mut list = load_injected_resolutions();
    let before = list.len();
    list.retain(|r| r.width != width || r.height != height);
    if list.len() == before {
        return Ok(()); // nothing to remove
    }
    std::fs::write(
        &path,
        serde_json::to_string(&list).map_err(|e| format!("序列化失败: {}", e))?,
    )
    .map_err(|e| format!("写入失败: {}", e))?;
    Ok(())
}

/// Set display resolution using NVAPI DISP API.
///
/// Steps (3-tier fallback):
/// 1. Try NvAPI_DISP_SetDisplayConfig (works for EDID-listed modes)
/// 2. Inject into NVIDIA NV_Modes registry + ChangeDisplaySettingsExW (custom/non-EDID modes)
/// 3. Retry SetDisplayConfig after injection
#[tauri::command]
pub fn set_nvidia_display_resolution(
    display_id: NvU32,
    width: NvU32,
    height: NvU32,
    device_name: String,
) -> Result<SetResolutionResult, String> {
    try_init_nvapi()?;

    if width == 0 || height == 0 {
        return Err("分辨率宽高不能为 0".to_string());
    }

    #[cfg(not(target_os = "windows"))]
    {
        return Err("此功能仅支持 Windows 系统".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        use std::mem;
        use windows_sys::Win32::Graphics::Gdi::{
            ChangeDisplaySettingsExW, EnumDisplaySettingsW, DEVMODEW,
        };

        // ── Strategy 1: Try NVAPI SetDisplayConfig (works for EDID-listed modes) ──
        let nvapi_err = 's1: {
            let mut path_info_count: NvU32 = 0;
            let status;
            unsafe {
                // ── Pass 1: Get pathInfoCount ──
                status =
                    NvAPI_DISP_GetDisplayConfig(&mut path_info_count, std::ptr::null_mut());
            }
            if status != NVAPI_OK {
                break 's1 format!(
                    "获取显示配置失败: {} (code {})",
                    nvapi_error_string(status),
                    status
                );
            }
            if path_info_count == 0 {
                break 's1 "没有活动的显示路径".to_string();
            }

            // ── Pass 2: Allocate pathInfo + sourceModeInfo arrays ──
            let mut source_mode_infos: Vec<NvDisplayConfigSourceModeInfo> = (0..path_info_count
                as usize)
                .map(|_| {
                    let mut sm: NvDisplayConfigSourceModeInfo;
                    unsafe { sm = mem::zeroed(); }
                    sm.colorFormat = 0; // NV_FORMAT_UNKNOWN
                    sm
                })
                .collect();

            let mut path_infos: Vec<NvDisplayConfigPathInfo> = (0..path_info_count as usize)
                .map(|i| {
                    let mut p: NvDisplayConfigPathInfo;
                    unsafe { p = mem::zeroed(); }
                    p.version = NV_DISPLAYCONFIG_PATH_INFO_VER;
                    p.sourceModeInfo = &mut source_mode_infos[i];
                    p
                })
                .collect();

            unsafe {
                let s = NvAPI_DISP_GetDisplayConfig(
                    &mut path_info_count,
                    path_infos.as_mut_ptr(),
                );
                if s != NVAPI_OK {
                    break 's1 format!(
                        "获取显示配置失败(Pass 2): {} (code {})",
                        nvapi_error_string(s),
                        s
                    );
                }
            }

            // ── Pass 3: Allocate targetInfo arrays (required to get displayId) ──
            let mut target_allocations: Vec<Vec<NvDisplayConfigPathTargetInfo>> = Vec::new();
            for path in &mut path_infos {
                let count = path.targetInfoCount as usize;
                if count > 0 {
                    let targets: Vec<NvDisplayConfigPathTargetInfo> = (0..count)
                        .map(|_| unsafe { mem::zeroed() })
                        .collect();
                    path.targetInfo = targets.as_ptr() as *mut NvDisplayConfigPathTargetInfo;
                    target_allocations.push(targets);
                } else {
                    path.targetInfo = std::ptr::null_mut();
                }
            }

            unsafe {
                let s = NvAPI_DISP_GetDisplayConfig(
                    &mut path_info_count,
                    path_infos.as_mut_ptr(),
                );
                if s != NVAPI_OK {
                    break 's1 format!(
                        "获取显示配置失败(Pass 3): {} (code {})",
                        nvapi_error_string(s),
                        s
                    );
                }
            }

            // ── Find the path containing our display_id ──
            let available_ids: Vec<NvU32> = path_infos
                .iter()
                .filter(|p| p.targetInfoCount > 0 && !p.targetInfo.is_null())
                .flat_map(|p| {
                    let targets = unsafe {
                        std::slice::from_raw_parts(p.targetInfo, p.targetInfoCount as usize)
                    };
                    targets.iter().map(|t| t.displayId).collect::<Vec<_>>()
                })
                .collect();
            log::info!(
                "NVIDIA display config: path_count={}, available_display_ids={:?}, searching for {}",
                path_info_count,
                available_ids,
                display_id
            );
            let target_path_idx = match path_infos.iter().position(|path| {
                if path.targetInfoCount > 0 && !path.targetInfo.is_null() {
                    let targets = unsafe {
                        std::slice::from_raw_parts(path.targetInfo, path.targetInfoCount as usize)
                    };
                    targets.iter().any(|t| t.displayId == display_id)
                } else {
                    false
                }
            }) {
                Some(idx) => idx,
                None => {
                    break 's1 format!("未找到 displayId {} 的显示路径", display_id);
                }
            };

            // ── Modify sourceModeInfo resolution ──
            let path = &mut path_infos[target_path_idx];
            let mode_info = match unsafe { path.sourceModeInfo.as_mut() } {
                Some(m) => m,
                None => {
                    break 's1 "目标路径没有 source mode info".to_string();
                }
            };
            let old_w = mode_info.resolution.width;
            let old_h = mode_info.resolution.height;

            log::info!(
                "NVIDIA 分辨率变更: displayId={}, {}x{} → {}x{}",
                display_id,
                old_w,
                old_h,
                width,
                height
            );

            mode_info.resolution.width = width;
            mode_info.resolution.height = height;

            // ── Apply via NVAPI ──
            let flags = NV_DISPLAYCONFIG_SAVE_TO_PERSISTENCE | NV_FORCE_COMMIT_VIDPN;
            let s = unsafe {
                NvAPI_DISP_SetDisplayConfig(
                    path_info_count,
                    path_infos.as_ptr(),
                    flags,
                )
            };
            if s == NVAPI_OK {
                log::info!("NVIDIA 分辨率已成功设为 {}x{} (NVAPI)", width, height);
                return Ok(SetResolutionResult { applied: true, injected: false });
            }

            // NVAPI SetDisplayConfig failed — save error for fallback
            format!(
                "{} (code {})",
                nvapi_error_string(s),
                s
            )
        };

        // ── Strategy 2: Inject into NVIDIA NV_Modes registry + ChangeDisplaySettingsExW ──
        log::warn!(
            "NVAPI SetDisplayConfig 失败 ({}), 尝试注入 NV_Modes 注册表",
            nvapi_err
        );

        let reg_inject_err = match inject_nv_modes_registry(&device_name, width, height) {
            Ok(keys) => {
                log::info!("NV_Modes 注册表注入成功: {} 个注册表项", keys.len());
                None
            }
            Err(e) => {
                log::error!("NV_Modes 注册表注入失败: {}", e);
                Some(e)
            }
        };

        unsafe {
            let wide_name: Vec<u16> =
                device_name.encode_utf16().chain(std::iter::once(0)).collect();

            // Get current display mode for dm struct
            const ENUM_CURRENT_SETTINGS: u32 = 0xFFFFFFFF;
            let mut dm: DEVMODEW = mem::zeroed();
            dm.dmSize = mem::size_of::<DEVMODEW>() as u16;
            let _enum_ok =
                EnumDisplaySettingsW(wide_name.as_ptr(), ENUM_CURRENT_SETTINGS, &mut dm);

            // Set target resolution
            const DM_PELSWIDTH: u32 = 0x00080000;
            const DM_PELSHEIGHT: u32 = 0x00100000;
            dm.dmPelsWidth = width;
            dm.dmPelsHeight = height;
            dm.dmFields = DM_PELSWIDTH | DM_PELSHEIGHT;

            let mut cds_result: i32 = -999;

            // Try ChangeDisplaySettingsExW with multiple flag combinations
            const CDS_UPDATEREGISTRY: u32 = 0x00000001;
            const CDS_ENABLE_UNSAFE_MODES: u32 = 0x00000100;
            // Skip CDS_GLOBAL — it requires CDS_SET_PRIMARY and causes BADFLAGS

            let flag_sets: &[u32] = &[
                CDS_UPDATEREGISTRY | CDS_ENABLE_UNSAFE_MODES,
                CDS_UPDATEREGISTRY,
                0,
            ];

            for &flags in flag_sets {
                cds_result = ChangeDisplaySettingsExW(
                    wide_name.as_ptr(),
                    &dm,
                    std::ptr::null_mut(),
                    flags,
                    std::ptr::null_mut(),
                );
                if cds_result == 0 {
                    log::info!(
                        "通过 NV_Modes 注入 + ChangeDisplaySettingsExW(flags={}) 成功设置分辨率 {}x{}",
                        flags, width, height
                    );
                    return Ok(SetResolutionResult { applied: true, injected: false });
                }
                log::warn!("ChangeDisplaySettingsExW(flags={}) 返回 DISP_CHANGE={}", flags, cds_result);
            }

            // ── Strategy 3: Try SetDisplayConfig again (mode might now be in NV_Modes) ──
            log::warn!(
                "ChangeDisplaySettingsExW 失败 (DISP_CHANGE={})，重试 NVAPI SetDisplayConfig",
                cds_result
            );

            // Re-read display config and try SetDisplayConfig again
            let retry_result = retry_set_display_config(display_id, width, height);
            if let Ok(()) = retry_result {
                return Ok(SetResolutionResult { applied: true, injected: false });
            }

            // NV_Modes injection succeeded but mode can't be applied without restart
            if reg_inject_err.is_none() {
                log::info!("NV_Modes 注入成功，需要重启后应用新分辨率");
                if let Err(e) = save_injected_resolution(width, height) {
                    log::warn!("保存注入记录失败: {}", e);
                }
                return Ok(SetResolutionResult { applied: false, injected: true });
            }

            let reg_err_msg = reg_inject_err.unwrap_or_else(|| "注入成功但模式切换失败".to_string());
            Err(format!(
                "所有方案均失败 — NVAPI SetDisplayConfig: {}; NV_Modes注入: {}; ChangeDisplaySettingsExW: DISP_CHANGE={}",
                nvapi_err, reg_err_msg, cds_result
            ))
        }
    }
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
