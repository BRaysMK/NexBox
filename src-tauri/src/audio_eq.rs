use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqBand {
    pub frequency: f32,
    pub gain: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqConfig {
    pub enabled: bool,
    pub bands: Vec<EqBand>,
    pub master_gain: f32,
    pub output_device_id: Option<String>,
}

// cpal::Stream is not Send on some platforms (Windows WASAPI).
// We only keep the stream alive in global state, so this wrapper is sufficient.
struct SendStream {
    _stream: cpal::Stream,
}
unsafe impl Send for SendStream {}

struct AudioStreamState {
    input_stream: Option<SendStream>,
    output_stream: Option<SendStream>,
}

lazy_static::lazy_static! {
    static ref EQ_CONFIG: Arc<Mutex<EqConfig>> = Arc::new(Mutex::new(EqConfig {
        enabled: false,
        bands: vec![
            EqBand { frequency: 31.0, gain: 0.0 },
            EqBand { frequency: 62.0, gain: 0.0 },
            EqBand { frequency: 125.0, gain: 0.0 },
            EqBand { frequency: 250.0, gain: 0.0 },
            EqBand { frequency: 500.0, gain: 0.0 },
            EqBand { frequency: 1000.0, gain: 0.0 },
            EqBand { frequency: 2000.0, gain: 0.0 },
            EqBand { frequency: 4000.0, gain: 0.0 },
            EqBand { frequency: 8000.0, gain: 0.0 },
            EqBand { frequency: 16000.0, gain: 0.0 },
        ],
        master_gain: 0.0,
        output_device_id: None,
    }));

    static ref STREAM_STATE: Arc<Mutex<AudioStreamState>> = Arc::new(Mutex::new(AudioStreamState {
        input_stream: None,
        output_stream: None,
    }));
}

#[tauri::command]
pub fn check_virtual_audio_driver() -> bool {
    let host = cpal::default_host();
    if device_list_contains_virtual(host.input_devices()) {
        return true;
    }

    device_list_contains_virtual(host.output_devices())
}

#[tauri::command]
pub fn get_audio_output_devices() -> Vec<AudioDevice> {
    let host = cpal::default_host();
    if let Ok(devices) = host.output_devices() {
        let default_device = host.default_output_device().and_then(|d| d.name().ok());
        
        devices.map(|d| {
            let name = d.name().unwrap_or_default();
            AudioDevice {
                id: name.clone(),
                name: name.clone(),
                is_default: Some(name) == default_device,
            }
        }).collect()
    } else {
        Vec::new()
    }
}

#[tauri::command]
pub fn get_eq_config() -> EqConfig {
    EQ_CONFIG.lock().unwrap().clone()
}

#[tauri::command]
pub fn set_eq_enabled(enabled: bool) {
    {
        let mut config = EQ_CONFIG.lock().unwrap();
        config.enabled = enabled;
    }

    if enabled {
        start_audio_processing();
    } else {
        stop_audio_processing();
    }
}

fn start_audio_processing() {
    let config = EQ_CONFIG.lock().unwrap().clone();
    let host = cpal::default_host();
    
    // 1. 查找虚拟声卡作为输入
    let input_device = host.input_devices().ok().and_then(|mut ds| 
        ds.find(|d| d.name().map(|n| n.contains("FxSound") || n.contains("NexBox")).unwrap_or(false))
    ).or_else(|| host.default_input_device());
    
    // 2. 查找物理声卡作为输出
    let output_device = if let Some(id) = config.output_device_id {
        host.output_devices().ok().and_then(|mut ds| 
            ds.find(|d| d.name().map(|n| n == id).unwrap_or(false))
        )
    } else {
        host.default_output_device()
    };

    if let (Some(_in_dev), Some(_out_dev)) = (input_device, output_device) {
        // let in_config: cpal::StreamConfig = in_dev.default_input_config().unwrap().into();
        // let out_config: cpal::StreamConfig = out_dev.default_output_config().unwrap().into();

        let _err_fn = |err: cpal::StreamError| eprintln!("an error occurred on stream: {}", err);
        
        // TODO: 实现跨设备路由
    }
}

fn stop_audio_processing() {
    let mut state = STREAM_STATE.lock().unwrap();
    state.input_stream = None;
    state.output_stream = None;
}

#[tauri::command]
pub fn set_eq_band(index: usize, gain: f32) {
    let mut config = EQ_CONFIG.lock().unwrap();
    if index < config.bands.len() {
        config.bands[index].gain = gain;
    }
}

#[tauri::command]
pub fn set_master_gain(gain: f32) {
    let mut config = EQ_CONFIG.lock().unwrap();
    config.master_gain = gain;
}

#[tauri::command]
pub fn set_output_device(device_id: String) {
    let enabled = {
        let mut config = EQ_CONFIG.lock().unwrap();
        config.output_device_id = Some(device_id);
        config.enabled
    };

    if enabled {
        stop_audio_processing();
        start_audio_processing();
    }
}

#[tauri::command]
pub fn reset_eq() {
    let mut config = EQ_CONFIG.lock().unwrap();
    for band in &mut config.bands {
        band.gain = 0.0;
    }
    config.master_gain = 0.0;
}

#[tauri::command]
pub fn remove_eq_driver() -> Result<String, String> {
    let driver_assets = resolve_driver_assets()?;

    // 预检：驱动是否已安装
    let is_installed = check_virtual_audio_driver();
    if !is_installed {
        return Ok("虚拟声卡驱动未安装，无需卸载".to_string());
    }

    if !driver_assets.fxdevcon.exists() {
        return Err(format!(
            "未找到驱动安装程序: {}",
            driver_assets.fxdevcon.display()
        ));
    }

    // 第 1 步：fxdevcon remove — 移除 PnP 设备节点
    let _ = run_elevated(
        &driver_assets.fxdevcon,
        &["remove".to_string(), "root\\fxvad".to_string()],
        &driver_assets.driver_dir,
    );

    // 第 2 步：pnputil /enum-drivers → 找到 FXVAD 驱动包 → 删除
    let _ = remove_pnputil_driver_package(&driver_assets);

    Ok("虚拟声卡驱动已卸载，建议重启电脑以完全清除".to_string())
}

/// 使用 pnputil 枚举驱动存储区，找到 FXVAD 驱动包并删除
fn remove_pnputil_driver_package(assets: &DriverAssets) -> Result<(), String> {
    // pnputil /enum-drivers 列出所有第三方驱动包（不需要管理员权限）
    let output = Command::new("pnputil")
        .args(["/enum-drivers"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("启动 pnputil 失败: {}", e))?;

    if !output.status.success() {
        return Err(format!("pnputil /enum-drivers 执行失败"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // 解析输出，找到发布名（Published Name）
    // pnputil 输出格式:
    //   Published Name:    oem0.inf
    //   Driver Package Provider:  FxSound
    //   ...
    let mut oem_names: Vec<String> = Vec::new();
    let mut current_oem = String::new();
    let mut found_match = false;

    for line in stdout.lines() {
        let trimmed = line.trim();

        // 捕获 Published Name
        if let Some(name) = trimmed.strip_prefix("Published Name:") {
            current_oem = name.trim().to_string();
        }

        // 检查是否匹配 FXVAD 或 FxSound
        let lower = trimmed.to_ascii_lowercase();
        if lower.contains("fxvad") || lower.contains("fxsound") || lower.contains("dfx") {
            found_match = true;
        }

        // 遇到空行 = 一个驱动包条目结束
        if trimmed.is_empty() && found_match && !current_oem.is_empty() {
            oem_names.push(current_oem.clone());
            current_oem.clear();
            found_match = false;
        }
    }
    // 处理最后一个条目
    if found_match && !current_oem.is_empty() {
        oem_names.push(current_oem);
    }

    if oem_names.is_empty() {
        // 驱动包没有被发布到存储区，不需要删除
        return Ok(());
    }

    // 逐个删除匹配的驱动包（需要管理员权限）
    for oem in &oem_names {
        let result = run_elevated(
            Path::new("pnputil.exe"),
            &[
                "/delete-driver".to_string(),
                oem.clone(),
                "/force".to_string(),
            ],
            &assets.driver_dir,
        );
        // 忽略"未找到"类错误
        if let Err(ref e) = result {
            if e.contains("1168") || e.contains("退出码: 0") {
                continue;
            }
            return Err(format!("删除驱动包 {} 失败: {}", oem, e));
        }
    }

    Ok(())
}

#[tauri::command]
pub fn install_eq_driver() -> Result<String, String> {
    let driver_assets = resolve_driver_assets()?;

    if !driver_assets.fxdevcon.exists() {
        return Err(format!(
            "未找到驱动安装程序: {}",
            driver_assets.fxdevcon.display()
        ));
    }

    if !driver_assets.inf.exists() {
        return Err(format!(
            "未找到驱动 INF 文件: {}",
            driver_assets.inf.display()
        ));
    }

    // 第 1 步：先尝试移除旧驱动（忽略失败）
    let _ = run_elevated(
        &driver_assets.fxdevcon,
        &["remove".to_string(), "root\\fxvad".to_string()],
        &driver_assets.driver_dir,
    );

    // 第 2 步：尝试用 fxdevcon 安装驱动
    let install_result = run_elevated(
        &driver_assets.fxdevcon,
        &[
            "install".to_string(),
            driver_assets.inf.to_string_lossy().into_owned(),
            "root\\fxvad".to_string(),
        ],
        &driver_assets.driver_dir,
    );

    // 第 3 步：如果 fxdevcon 失败，尝试用 pnputil 作为备选方案
    if install_result.is_err() {
        let pnp_result = try_install_with_pnputil(&driver_assets);
        if let Err(ref pnp_err) = pnp_result {
            return Err(format!(
                "fxdevcon 安装失败，pnputil 也失败:\n- fxdevcon: {:?}\n- pnputil: {}\n\n\
                 建议尝试以下方法:\n\
                 1. 以管理员身份运行本程序再试\n\
                 2. 开启Windows测试签名模式: bcdedit /set testsigning on\n\
                 3. 手动安装驱动: 右键 fxvad.inf -> 安装",
                install_result.unwrap_err(),
                pnp_err
            ));
        }
    }

    // 第 4 步：安装后配置（DfxSetupDrv 可选步骤）
    if driver_assets.dfx_setup.exists() {
        let _ = run_hidden(
            &driver_assets.dfx_setup,
            &["setname".to_string()],
            &driver_assets.apps_dir,
        );
        let _ = run_hidden(
            &driver_assets.dfx_setup,
            &["defaultbuffersize".to_string()],
            &driver_assets.apps_dir,
        );
    }

    Ok("虚拟声卡驱动安装命令已执行，请允许管理员授权并稍候刷新状态".to_string())
}

/// 备选方案: 使用 Windows 内置的 pnputil 安装驱动（更适合现代 Windows 版本）
/// pnputil 也需要管理员权限，所以复用 run_elevated
fn try_install_with_pnputil(assets: &DriverAssets) -> Result<(), String> {
    run_elevated(
        Path::new("pnputil.exe"),
        &[
            "/add-driver".to_string(),
            assets.inf.to_string_lossy().into_owned(),
            "/install".to_string(),
        ],
        &assets.driver_dir,
    )
}

struct DriverAssets {
    fxdevcon: PathBuf,
    inf: PathBuf,
    dfx_setup: PathBuf,
    driver_dir: PathBuf,
    apps_dir: PathBuf,
}

fn device_list_contains_virtual<I>(
    devices: Result<I, cpal::DevicesError>,
) -> bool
where
    I: Iterator<Item = cpal::Device>,
{
    if let Ok(devices) = devices {
        for device in devices {
            if let Ok(name) = device.name() {
                let lower = name.to_ascii_lowercase();
                if lower.contains("fxsound")
                    || lower.contains("fxvad")
                    || lower.contains("nexbox")
                {
                    return true;
                }
            }
        }
    }
    false
}

fn resolve_driver_assets() -> Result<DriverAssets, String> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| "无法定位项目根目录".to_string())?;

    let installer_root = repo_root.join("fxsound-app-main").join("Installer");
    let apps_root = installer_root.join("Apps").join("Version14");
    let drivers_root = installer_root.join("Drivers").join("Version14");

    let arch_dir = match std::env::consts::ARCH {
        "x86_64" => ("win10", "x64", "fxdevcon64.exe"),
        "x86" => ("win10", "x86", "fxdevcon32.exe"),
        "aarch64" => ("win10", "arm64", "fxdevcon64.exe"),
        other => {
            return Err(format!("暂不支持当前架构: {other}"));
        }
    };

    let driver_dir = drivers_root.join(arch_dir.0).join(arch_dir.1);

    Ok(DriverAssets {
        fxdevcon: driver_dir.join(arch_dir.2),
        inf: driver_dir.join("fxvad.inf"),
        dfx_setup: apps_root.join("DfxSetupDrv.exe"),
        driver_dir,
        apps_dir: apps_root,
    })
}

fn run_hidden(exe: &Path, args: &[String], working_dir: &Path) -> Result<(), String> {
    let status = Command::new(exe)
        .args(args)
        .current_dir(working_dir)
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| format!("启动 {} 失败: {}", exe.display(), e))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{} 执行失败，退出码: {:?}",
            exe.display(),
            status.code()
        ))
    }
}

fn run_elevated(exe: &Path, args: &[String], working_dir: &Path) -> Result<(), String> {
    let exe_str = exe.to_string_lossy();
    let wd_str = working_dir.to_string_lossy();

    // PowerShell 脚本：
    // 1. 用 -Verb RunAs 提权运行目标程序（显示 UAC 窗口）
    // 2. 捕获执行结果（区分"用户取消UAC"和"工具执行失败"）
    // 3. 通过 stdout 输出 JSON 格式的结果信息
    let script = format!(
        "$ErrorActionPreference='Stop'; \
         $r=@{{code=-1;msg=''}}; \
         try{{ \
             $p=Start-Process -FilePath '{}' -WorkingDirectory '{}' -ArgumentList @({}) -Verb RunAs -Wait -PassThru -WindowStyle Normal -ErrorAction Stop; \
             $r.code=$p.ExitCode; \
         }}catch{{ \
             $r.code=-2; \
             $r.msg=$_.Exception.Message; \
         }}; \
         Write-Output (ConvertTo-Json $r -Compress)",
        exe_str.replace('\'', "''"),
        wd_str.replace('\'', "''"),
        args.iter()
            .map(|a| format!("'{}'", a.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(", "),
    );

    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        // 不使用 CREATE_NO_WINDOW，确保 UAC 授权窗口能正常弹出
        .creation_flags(0x00000000)
        .output()
        .map_err(|e| format!("提权执行失败: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // 尝试解析 PowerShell 返回的 JSON 结果
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(stdout.trim()) {
        let code = json["code"].as_i64().unwrap_or(-1);
        let msg = json["msg"].as_str().unwrap_or("");

        match code {
            0 => return Ok(()),
            -2 => {
                // UAC 被取消
                return Err(format!(
                    "管理员授权被取消: {}",
                    if msg.is_empty() { "用户未允许管理员授权" } else { msg }
                ));
            }
            _ => {
                // 工具执行失败
                return Err(format!(
                    "驱动安装程序返回错误 (退出码: {})",
                    code
                ));
            }
        }
    }

    // 如果 JSON 解析失败（例如 PowerShell 本身出错），使用备用的错误信息
    if !output.status.success() {
        let detail = match output.status.code() {
            Some(-1) => "可能是用户取消了管理员授权，或驱动安装程序执行失败".to_string(),
            Some(code) => format!("退出码: {}", code),
            None => "未获取到退出码".to_string(),
        };
        let err_msg = if !stderr.trim().is_empty() {
            format!("{} [PowerShell: {}]", detail, stderr.trim())
        } else {
            detail
        };
        return Err(format!("驱动安装命令执行失败: {}", err_msg));
    }

    Ok(())
}
