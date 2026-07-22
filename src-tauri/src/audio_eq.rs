use serde::{Deserialize, Serialize};
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Manager};

use crate::audio_engine::{AudioEngine, BandParam};

const CREATE_NO_WINDOW: u32 = 0x08000000;
const FXVAD_SERVICE: &str = "FXVAD";

/// 驱动状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverStatus {
    pub installed: bool,
    pub service_exists: bool,
    pub service_running: bool,
    pub device_name: String,
    pub needs_reboot: bool,
}

/// EQ 频段信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqBand {
    pub freq: f64,
    pub gain: f64,
}

/// EQ 预设
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqPreset {
    pub id: String,
    pub name: String,
    pub bands: Vec<EqBand>,
    pub enabled: bool,
}

/// EQ 引擎状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStatus {
    pub running: bool,
    pub pid: Option<u32>,
}

/// 全局 AudioEngine 实例
static EQ_ENGINE: OnceLock<Mutex<Option<AudioEngine>>> = OnceLock::new();

fn eq_engine() -> &'static Mutex<Option<AudioEngine>> {
    EQ_ENGINE.get_or_init(|| Mutex::new(None))
}

/// 获取 fxvad 资源目录
fn get_fxvad_resource_dir(app: &AppHandle) -> Option<PathBuf> {
    // 1. 通过 Tauri resource_dir 查找（生产环境 + 部分开发现境）
    if let Ok(resource_dir) = app.path().resource_dir() {
        let candidates = [
            resource_dir.join("binaries").join("fxvad"),
            resource_dir.join("resources").join("binaries").join("fxvad"),
            resource_dir.join("_up_").join("resources").join("binaries").join("fxvad"),
            resource_dir.join("_up_").join("_up_").join("src-tauri").join("resources").join("binaries").join("fxvad"),
        ];
        for path in &candidates {
            if path.exists() {
                return Some(path.clone());
            }
        }
    }

    // 2. 通过 exe 路径查找（开发环境：exe 在 src-tauri/target/debug/ 下）
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let candidates = [
                parent.join("binaries").join("fxvad"),
                parent.join("resources").join("binaries").join("fxvad"),
                // dev: target/debug -> src-tauri/resources/binaries/fxvad
                parent.join("..").join("..").join("resources").join("binaries").join("fxvad"),
                // dev: target/debug -> NexBox/src-tauri/resources/binaries/fxvad
                parent.join("..").join("..").join("..").join("src-tauri").join("resources").join("binaries").join("fxvad"),
            ];
            for path in &candidates {
                if path.exists() {
                    if let Ok(canon) = path.canonicalize() {
                        return Some(canon);
                    }
                    return Some(path.clone());
                }
            }
        }
    }

    // 3. 编译时路径（开发环境最可靠：CARGO_MANIFEST_DIR = src-tauri 目录）
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dev_path = manifest_dir.join("resources").join("binaries").join("fxvad");
    if dev_path.exists() {
        return Some(dev_path);
    }

    None
}

/// 获取 wujieq 工作目录（%LOCALAPPDATA%/NexBox/EQEngine）
fn get_eq_work_dir() -> PathBuf {
    let mut path = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("NexBox");
    path.push("EQEngine");
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }
    path
}

/// 解析 .fac 预设文件
fn parse_fac_file(content: &str, file_id: &str) -> Option<EqPreset> {
    let mut name = String::new();
    let mut bands: Vec<EqBand> = Vec::new();
    let mut enabled = true;
    let mut in_bands = false;
    let mut current_freq: Option<f64> = None;

    for (line_idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // 第3行是预设名称（索引2）
        if line_idx == 2 {
            name = trimmed.to_string();
            continue;
        }

        // 检测 On/Off Flag
        if trimmed.contains("On/Off Flag") {
            if let Some(val_str) = trimmed.split(':').next() {
                if let Ok(val) = val_str.trim().parse::<i32>() {
                    enabled = val != 0;
                }
            }
            in_bands = true;
            continue;
        }

        if in_bands {
            // 匹配 "   62.5: CF" 格式
            if trimmed.ends_with(": CF") || trimmed.ends_with(":CF") {
                let freq_str = trimmed.split(':').next().unwrap_or("").trim();
                if let Ok(freq) = freq_str.parse::<f64>() {
                    current_freq = Some(freq);
                }
            }
            // 匹配 "   0: Boost/Cut" 格式
            else if trimmed.contains("Boost/Cut") {
                if let Some(freq) = current_freq {
                    let gain_str = trimmed.split(':').next().unwrap_or("").trim();
                    if let Ok(gain) = gain_str.parse::<f64>() {
                        bands.push(EqBand { freq, gain });
                    }
                    current_freq = None;
                }
            }
        }
    }

    if name.is_empty() {
        name = format!("Preset {}", file_id);
    }

    Some(EqPreset {
        id: file_id.to_string(),
        name,
        bands,
        enabled,
    })
}

/// 检查虚拟声卡驱动状态
#[tauri::command]
pub fn check_virtual_audio_driver() -> Result<DriverStatus, String> {
    let mut service_exists = false;
    let mut service_running = false;

    // 使用 sc query 检查服务
    if let Ok(output) = Command::new("sc")
        .args(["query", FXVAD_SERVICE])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}\n{}", stdout, stderr);

        // sc query 成功 (exit code 0) 才说明服务存在
        // 服务不存在时 exit code 非零，输出包含错误码 1060
        let cmd_success = output.status.success();
        let is_error_1060 = combined.contains("1060")
            && (combined.contains("does not exist")
                || combined.contains("不存在")
                || combined.contains("未安装")
                || combined.contains("FAILED")
                || combined.contains("失败"));

        if cmd_success || (combined.to_lowercase().contains("fxvad") && !is_error_1060) {
            service_exists = true;
            if combined.contains("RUNNING") {
                service_running = true;
            }
        }
    }

    // 检测是否需要重启：服务存在但启动类型已设为"禁用"（说明之前卸载过）
    let mut needs_reboot = false;
    if service_exists {
        if let Ok(output) = Command::new("sc")
            .args(["qc", FXVAD_SERVICE])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains("DISABLED") || stdout.contains("禁用") {
                needs_reboot = true;
            }
        }
    }

    // 使用 pnputil 检查驱动是否已安装
    let mut driver_installed = service_exists;
    if !driver_installed {
        if let Ok(output) = Command::new("pnputil")
            .args(["/enum-drivers"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.to_lowercase().contains("fxsound") || stdout.to_lowercase().contains("fxvad") {
                driver_installed = true;
            }
        }
    }

    Ok(DriverStatus {
        installed: driver_installed,
        service_exists,
        service_running,
        device_name: "FxSound Audio Enhancer".to_string(),
        needs_reboot,
    })
}

/// 安装虚拟声卡驱动（需要管理员权限）
#[tauri::command]
pub async fn install_virtual_audio_driver(app: AppHandle) -> Result<String, String> {
    let fxvad_dir = get_fxvad_resource_dir(&app)
        .ok_or_else(|| "无法找到驱动资源目录".to_string())?;

    let inf_path = fxvad_dir.join("fxvad.inf");
    let devcon_path = fxvad_dir.join("fxdevcon64.exe");
    let sys_path = fxvad_dir.join("fxvad.sys");

    if !inf_path.exists() {
        return Err("驱动 INF 文件不存在".to_string());
    }
    if !devcon_path.exists() {
        return Err("devcon 工具不存在".to_string());
    }

    let inf_str = inf_path.to_string_lossy().replace('\'', "''");
    let devcon_str = devcon_path.to_string_lossy().replace('\'', "''");
    let sys_str = sys_path.to_string_lossy().replace('\'', "''");

    // 构建 PowerShell 安装脚本（以管理员权限运行）
    let ps_script = format!(
        r#"
$ErrorActionPreference = 'SilentlyContinue'
$logFile = Join-Path $env:TEMP 'nexbox_eq_install.log'
"=== Install Start ===" | Out-File $logFile -Encoding UTF8

# Step 1: 复制驱动文件到 system32\drivers
"Step 1: Copying fxvad.sys" | Out-File $logFile -Append -Encoding UTF8
$srcSys = '{sys_str}'
$dstSys = 'C:\Windows\System32\drivers\fxvad.sys'
if (-not (Test-Path $dstSys)) {{
    Copy-Item $srcSys $dstSys -Force 2>&1 | Out-File $logFile -Append -Encoding UTF8
    "Copied fxvad.sys" | Out-File $logFile -Append -Encoding UTF8
}} else {{
    "fxvad.sys already exists" | Out-File $logFile -Append -Encoding UTF8
}}

# Step 2: 使用 pnputil 安装驱动包
"Step 2: pnputil /add-driver" | Out-File $logFile -Append -Encoding UTF8
$infPath = '{inf_str}'
$pnputilResult = pnputil /add-driver $infPath /install 2>&1
"pnputil output: $pnputilResult" | Out-File $logFile -Append -Encoding UTF8

# Step 3: 使用 devcon 创建设备实例
"Step 3: devcon install" | Out-File $logFile -Append -Encoding UTF8
$devconExe = '{devcon_str}'
$devconResult = & $devconExe install $infPath 'Root\FXVAD' 2>&1
"devcon output: $devconResult" | Out-File $logFile -Append -Encoding UTF8

Start-Sleep -Seconds 3

# 验证
"Step 4: Verification" | Out-File $logFile -Append -Encoding UTF8
$scQuery = sc.exe query FXVAD 2>&1
"sc query: $scQuery" | Out-File $logFile -Append -Encoding UTF8

if ($scQuery -match 'FXVAD|FxSound') {{
    "RESULT:SUCCESS" | Out-File $logFile -Append -Encoding UTF8
    Write-Output 'SUCCESS'
}} else {{
    # 检查 pnputil 中是否有驱动
    $enumCheck = pnputil /enum-drivers 2>&1
    $found = $enumCheck | Where-Object {{ $_ -match 'fxsound|fxvad' }}
    if ($found) {{
        "RESULT:SUCCESS" | Out-File $logFile -Append -Encoding UTF8
        Write-Output 'SUCCESS'
    }} else {{
        "RESULT:FAILED" | Out-File $logFile -Append -Encoding UTF8
        Write-Output 'FAILED'
    }}
}}
"#,
        sys_str = sys_str,
        inf_str = inf_str,
        devcon_str = devcon_str
    );

    // 写入临时脚本文件
    let temp_dir = std::env::temp_dir();
    let script_path = temp_dir.join("nexbox_eq_install.ps1");
    fs::write(&script_path, &ps_script)
        .map_err(|e| format!("写入安装脚本失败: {}", e))?;

    let script_str = script_path.to_string_lossy().replace('\'', "''");

    log::info!("[install] Script written to {:?}", script_path);

    // 以管理员权限执行脚本
    let ps_command = format!(
        "Start-Process -FilePath 'powershell' -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','\"{}\"' -Verb RunAs -Wait -WindowStyle Hidden",
        script_str
    );

    let output = Command::new("powershell")
        .args(["-WindowStyle", "Hidden", "-NoProfile", "-Command", &ps_command])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("执行安装脚本失败: {}", e))?;

    // 读取日志
    let log_path = temp_dir.join("nexbox_eq_install.log");
    if let Ok(log_content) = fs::read_to_string(&log_path) {
        log::info!("[install] Log:\n{}", log_content);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    log::info!("[install] PowerShell stdout: {}", stdout);
    if !stderr.is_empty() {
        log::warn!("[install] PowerShell stderr: {}", stderr);
    }

    // 清理临时脚本
    let _ = fs::remove_file(&script_path);

    // 检查用户是否取消了 UAC
    if !output.status.success() {
        let stderr_lower = stderr.to_lowercase();
        if stderr_lower.contains("cancel") || stderr_lower.contains("denied") || stderr_lower.contains("取消") {
            return Err("管理员授权被取消，安装未执行".to_string());
        }
    }

    // 验证安装结果
    std::thread::sleep(std::time::Duration::from_secs(1));
    let status = check_virtual_audio_driver()?;
    if status.installed {
        let _ = fs::remove_file(&log_path);
        Ok("虚拟声卡驱动安装成功".to_string())
    } else {
        // 读取日志中的详细信息
        let log_detail = fs::read_to_string(&log_path).unwrap_or_default();
        let log_summary = log_detail
            .lines()
            .filter(|l| l.contains("output:") || l.contains("RESULT:") || l.contains("Failed"))
            .collect::<Vec<_>>()
            .join("\n");
        Err(format!(
            "驱动安装失败。可能需要重启电脑后重试。\n日志摘要:\n{}",
            log_summary
        ))
    }
}

/// 卸载虚拟声卡驱动（需要管理员权限）
#[tauri::command]
pub async fn uninstall_virtual_audio_driver(app: AppHandle) -> Result<String, String> {
    // 先停止 EQ 引擎
    let _ = stop_eq_engine();

    let fxvad_dir = get_fxvad_resource_dir(&app)
        .ok_or_else(|| "无法找到驱动资源目录".to_string())?;

    let devcon_path = fxvad_dir.join("fxdevcon64.exe");
    let devcon_str = devcon_path.to_string_lossy().replace('\'', "''");

    // 构建精简 PowerShell 卸载脚本（极简：核心就是一条正则找 oem.inf → pnputil 删除）
    let ps_script = format!(
        r#"
$ErrorActionPreference = 'Continue'
$log = "$env:TEMP\nexbox_eq_uninstall.log"
'START' | Out-File $log

# 1. 禁用服务
sc.exe config FXVAD start= disabled *>> $log
# 2. 尝试停止（NOT_STOPPABLE 会失败，忽略）
sc.exe stop FXVAD *>> $log
# 3. devcon 移除设备
$d = '{devcon_str}'
if (Test-Path $d) {{
    & $d remove '@ROOT\FXVAD\*' *>> $log
    & $d remove '=MEDIA' 'ROOT\FXVAD*' *>> $log
}}
# 4. 标记服务删除
$delOut = sc.exe delete FXVAD 2>&1
"SC_DELETE: $delOut" | Out-File $log -Append
$isMarked = $delOut -match '1072|marked|标记'
"MARKED: $isMarked" | Out-File $log -Append

# 5. 删除驱动包 —— 核心：一条正则找 oemXX.inf 的 Published Name
$raw = pnputil /enum-drivers 2>&1 | Out-String
# 匹配 "发布名称: oem6.inf ... 原始名称: fxvad.inf"（中英文均兼容）
$re = '(?:Published Name|发布名称):\s+(oem\d+\.inf).*?\n.*?(?:Original Name|原始名称):\s+fxvad\.inf'
if ($raw -match $re) {{
    $oem = $Matches[1]
    "DELETE_DRIVER: $oem" | Out-File $log -Append
    pnputil /delete-driver $oem /uninstall /force *>> $log
}} else {{
    "NO_FXVAD_OEM" | Out-File $log -Append
}}

# 6. 安排重启后删除 fxvad.sys
$sys = 'C:\Windows\System32\drivers\fxvad.sys'
if (Test-Path $sys) {{
    try {{ Remove-Item $sys -Force -ErrorAction Stop; "FILE_DELETED" | Out-File $log -Append }}
    catch {{
        "FILE_REBOOT" | Out-File $log -Append
        Add-Type -Name MF -Namespace W -EA SilentlyContinue -MemberDef '[DllImport("kernel32.dll")]public static extern bool MoveFileEx(string a,string b,int f);'
        [W.MF]::MoveFileEx($sys, $null, 4) | Out-Null
    }}
}}

# 7. 验证 + 报告
$ck = sc.exe query FXVAD 2>&1
"CHECK: $ck" | Out-File $log -Append
$gone = $ck -match '1060|不存在|未安装'

# 再次检查 pnputil 是否还有残留
$check2 = pnputil /enum-drivers 2>&1 | Out-String
$pnpLeft = $check2 -match 'fxvad|fxsound'
"PNP_LEFT: $pnpLeft" | Out-File $log -Append

if ($gone -and -not $pnpLeft) {{ Write-Output 'SUCCESS' }}
elseif ($pnpLeft -and ($gone -or $isMarked)) {{
    # 驱动包还在，再试一次删除
    if ($check2 -match '(?:Published Name|发布名称):\s+(oem\d+\.inf)[\s\S]*?fxvad') {{
        $oem2 = $Matches[1]
        pnputil /delete-driver $oem2 /uninstall /force *>> $log
        $finalCheck = pnputil /enum-drivers 2>&1 | Out-String
        if ($finalCheck -match 'fxvad|fxsound') {{
            Write-Output 'REBOOT_REQUIRED'
        }} else {{
            Write-Output 'SUCCESS'
        }}
    }} else {{
        Write-Output 'REBOOT_REQUIRED'
    }}
}}
elseif ($isMarked -and ($ck -match 'FXVAD')) {{ Write-Output 'REBOOT_REQUIRED' }}
else {{ Write-Output 'PARTIAL' }}
"#,
        devcon_str = devcon_str
    );

    let temp_dir = std::env::temp_dir();
    let script_path = temp_dir.join("nexbox_eq_uninstall.ps1");
    fs::write(&script_path, &ps_script)
        .map_err(|e| format!("写入卸载脚本失败: {}", e))?;

    let script_str = script_path.to_string_lossy().replace('\'', "''");
    log::info!("[uninstall] Script written to {:?}", script_path);

    // 以管理员权限执行
    let ps_command = format!(
        "Start-Process -FilePath 'powershell' -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','\"{}\"' -Verb RunAs -Wait -WindowStyle Hidden",
        script_str
    );

    let output = Command::new("powershell")
        .args(["-WindowStyle", "Hidden", "-NoProfile", "-Command", &ps_command])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("执行卸载脚本失败: {}", e))?;

    let log_path = temp_dir.join("nexbox_eq_uninstall.log");
    if let Ok(log_content) = fs::read_to_string(&log_path) {
        log::info!("[uninstall] Log:\n{}", log_content);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    log::info!("[uninstall] stdout: {}", stdout);
    if !stderr.is_empty() {
        log::warn!("[uninstall] stderr: {}", stderr);
    }

    let _ = fs::remove_file(&script_path);

    // UAC 取消检测
    if !output.status.success() {
        let se = stderr.to_lowercase();
        if se.contains("cancel") || se.contains("denied") || se.contains("取消") {
            return Err("管理员授权被取消，卸载未执行".to_string());
        }
    }

    // 解析结果
    let out = stdout.trim();
    std::thread::sleep(std::time::Duration::from_secs(1));

    if out.contains("SUCCESS") {
        log::info!("[uninstall] Fully removed");
        let _ = fs::remove_file(&log_path);
        Ok("虚拟声卡驱动已卸载".to_string())
    } else if out.contains("REBOOT_REQUIRED") {
        log::info!("[uninstall] Pending reboot");
        let _ = fs::remove_file(&log_path);
        Ok("驱动已标记卸载，需要重启电脑完成卸载".to_string())
    } else {
        let detail = fs::read_to_string(&log_path).unwrap_or_default();
        let summary: Vec<&str> = detail
            .lines()
            .filter(|l| l.contains("DELETE_DRIVER:") || l.contains("NO_FXVAD_OEM") || l.contains("SC_DELETE:") || l.contains("MARKED:") || l.contains("FILE_") || l.contains("CHECK:"))
            .collect();
        log::error!("[uninstall] Incomplete.\n{}", summary.join("\n"));
        Err(format!(
            "驱动卸载不完整。可能需要重启电脑后重试。\n{}",
            summary.join("\n")
        ))
    }
}

/// 启动 EQ 引擎（原生 WASAPI），并自动切换默认音频设备到 FxSound
#[tauri::command]
pub async fn start_eq_engine(_app: AppHandle) -> Result<String, String> {
    // 检查是否已在运行
    {
        let guard = eq_engine().lock().map_err(|e| format!("锁错误: {}", e))?;
        if let Some(ref engine) = *guard {
            if engine.is_running() {
                return Err("EQ 引擎已在运行".to_string());
            }
        }
    }

    // 检查驱动是否已安装
    let driver_status = check_virtual_audio_driver()?;
    if !driver_status.installed {
        return Err("请先安装虚拟声卡驱动".to_string());
    }

    // 保存当前默认设备，切换到 FxSound 虚拟声卡
    let prev_device = switch_default_audio_to_fxsound()
        .unwrap_or_else(|e| { log::warn!("[eq] Audio switch warning: {}", e); String::new() });
    let physical_device_name = if prev_device.starts_with("PREV:") {
        // Read device name from file written by PowerShell (correct UTF-8 encoding)
        // instead of using stdout which may have GBK/CP936 encoding issues on Chinese Windows
        let prev_file = std::env::temp_dir().join("nexbox_eq_prev_device.txt");
        let name = fs::read_to_string(&prev_file)
            .ok()
            .map(|s| s.trim_start_matches('\u{FEFF}').trim().to_string()) // strip BOM + whitespace
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| prev_device[5..].trim().to_string()); // fallback to stdout
        log::info!("[eq] Previous device name: '{}'", name);
        name
    } else {
        String::new()
    };

    // Wait for default device switch to fully propagate before starting audio engine
    std::thread::sleep(std::time::Duration::from_millis(200));

    // 启动原生音频引擎
    let engine = AudioEngine::start(physical_device_name.clone())?;

    // Store the engine
    {
        let mut guard = eq_engine().lock().map_err(|e| format!("锁错误: {}", e))?;
        *guard = Some(engine);
    }

    // 等待引擎初始化
    std::thread::sleep(std::time::Duration::from_millis(300));

    if prev_device.starts_with("PREV:") {
        Ok(format!("PREV:{}", &prev_device[5..]))
    } else {
        Ok("SWITCHED".to_string())
    }
}

// ===== 音频设备切换功能 =====
// 通过 PowerShell + C# 内联代码调用 Windows COM 接口
// IPolicyConfig::SetDefaultEndpoint 切换默认音频播放设备

/// COM 接口定义（C# 内联，用于 PowerShell 调用）
const AUDIO_COM_DEFS: &str = r#"
[ComImport,Guid("870AF99C-171D-4F9E-AF0D-E63DF40C2BC9")] public class _PC {}
[ComImport,Guid("F8679F50-850A-41CF-9C72-430F290290C8"),InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IPolicyConfig {
    [PreserveSig] int a();[PreserveSig] int b();[PreserveSig] int c();[PreserveSig] int d();[PreserveSig] int e();[PreserveSig] int f();[PreserveSig] int g();[PreserveSig] int h();[PreserveSig] int i();
    [PreserveSig] int SetDefaultEndpoint([MarshalAs(UnmanagedType.LPWStr)] string id, int role);
    [PreserveSig] int j();
}
[ComImport,Guid("BCDE0395-E52F-467C-8E3D-C4579291692E")] public class _MMDE {}
[ComImport,Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"),InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IMMDeviceEnumerator {
    int EnumAudioEndpoints(int df, int sm, out IntPtr p);
    int GetDefaultAudioEndpoint(int df, int r, out IntPtr p);
}
[ComImport,Guid("D666063F-1587-4E43-81F1-B948E807363F"),InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IMMDevice {
    int a(); int OpenPropertyStore(int s, out IntPtr p);
    int GetId([MarshalAs(UnmanagedType.LPWStr)] out string id);
}
[ComImport,Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99"),InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IPropertyStore {
    int a(); int b(); int GetValue(ref PROPERTYKEY k, out PROPVARIANT v);
}
[ComImport,Guid("0BD7A1BE-7A1A-44DB-8397-CC5392387B5E"),InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IMMDeviceCollection { int GetCount(out uint c); int Item(uint i, out IntPtr p); }
[StructLayout(LayoutKind.Sequential)] public struct PROPERTYKEY { public Guid fmtid; public uint pid; }
[StructLayout(LayoutKind.Explicit)] public struct PROPVARIANT { [FieldOffset(0)] public ushort vt; [FieldOffset(8)] public IntPtr ptrVal; }
"#;

/// 切换 Windows 默认音频播放设备到 FxSound Audio Enhancer
/// 返回格式 "PREV:设备名"，包含切换前的默认设备名
fn switch_default_audio_to_fxsound() -> Result<String, String> {
    // C# 代码：GetCur() 获取当前默认设备名，SetDef(name) 按名称匹配并切换
    let cs_code = r#"
public static string GetCur() {
    IMMDeviceEnumerator imm = (IMMDeviceEnumerator)new _MMDE();
    IntPtr pD; imm.GetDefaultAudioEndpoint(0,0,out pD);
    IMMDevice dev = (IMMDevice)Marshal.GetObjectForIUnknown(pD);
    IntPtr pS; dev.OpenPropertyStore(0,out pS);
    IPropertyStore s = (IPropertyStore)Marshal.GetObjectForIUnknown(pS);
    PROPERTYKEY k = new PROPERTYKEY();
    k.fmtid = Guid.Parse("{A45C254E-DF1C-4EFD-8020-67D146A850E0}");
    k.pid = 14;
    PROPVARIANT v; s.GetValue(ref k, out v);
    return Marshal.PtrToStringUni(v.ptrVal) ?? "";
}
public static string SetDef(string name) {
    IMMDeviceEnumerator imm = (IMMDeviceEnumerator)new _MMDE();
    IntPtr pC; imm.EnumAudioEndpoints(0,1,out pC);
    IMMDeviceCollection col = (IMMDeviceCollection)Marshal.GetObjectForIUnknown(pC);
    uint n; col.GetCount(out n);
    for (uint i=0;i<n;i++) {
        IntPtr pD; col.Item(i,out pD);
        IMMDevice dev = (IMMDevice)Marshal.GetObjectForIUnknown(pD);
        IntPtr pS; dev.OpenPropertyStore(0,out pS);
        IPropertyStore s = (IPropertyStore)Marshal.GetObjectForIUnknown(pS);
        PROPERTYKEY k = new PROPERTYKEY();
        k.fmtid = Guid.Parse("{A45C254E-DF1C-4EFD-8020-67D146A850E0}");
        k.pid = 14;
        PROPVARIANT v; s.GetValue(ref k, out v);
        string nm = Marshal.PtrToStringUni(v.ptrVal) ?? "";
        if (nm.IndexOf(name,StringComparison.OrdinalIgnoreCase)>=0) {
            string did; dev.GetId(out did);
            object cfg = Activator.CreateInstance(Type.GetTypeFromCLSID(Guid.Parse("{870AF99C-171D-4F9E-AF0D-E63DF40C2BC9}")));
            ((IPolicyConfig)cfg).SetDefaultEndpoint(did,0);
            return nm;
        }
    }
    return null;
}
"#;

    // 构建 PowerShell 脚本
    let ps_script = format!(
        "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8\r\
        $ErrorActionPreference = 'Stop'\r\
        $src = @'\r\
        using System;\r\
        using System.Runtime.InteropServices;\r\
        {com_defs}\r\
        public static class AudioSwitch {{\r\
        {cs_code}\r\
        }}\r\
        '@\r\
        \r\
        $type = Add-Type -TypeDefinition $src -PassThru -ErrorAction Stop\r\
        $prev = [AudioSwitch]::GetCur()\r\
        $prevFile = Join-Path $env:TEMP 'nexbox_eq_prev_device.txt'\r\
        [System.IO.File]::WriteAllText($prevFile, $prev, [System.Text.UTF8Encoding]::new($false))\r\
        $ok = [AudioSwitch]::SetDef('FxSound')\r\
        Start-Sleep -Milliseconds 300\r\
        if ($ok) {{ Write-Output \"PREV:$prev\" }} else {{ Write-Output \"NOT_FOUND\" }}\r",
        com_defs = AUDIO_COM_DEFS,
        cs_code = cs_code,
    );

    let temp_dir = std::env::temp_dir();
    let script_path = temp_dir.join("nexbox_audio_switch.ps1");
    // 写入 UTF-8 with BOM（PowerShell 需要 BOM 才能正确识别 UTF-8）
    let mut bom = vec![0xEF, 0xBB, 0xBF];
    bom.extend_from_slice(ps_script.as_bytes());
    fs::write(&script_path, &bom).map_err(|e| format!("写入脚本失败: {}", e))?;

    let output = Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script_path)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("执行失败: {}", e))?;

    let _ = fs::remove_file(&script_path);
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    log::info!("[audio_switch] stdout: {}", stdout);
    if !stderr.is_empty() {
        log::warn!("[audio_switch] stderr: {}", stderr);
    }

    if stdout.starts_with("PREV:") {
        Ok(stdout)
    } else {
        Err(format!(
            "找不到 FxSound Audio Enhancer 设备，请确认驱动已安装并在声音设置中可见。\nstderr: {}",
            stderr
        ))
    }
}

/// 恢复 Windows 默认音频设备
fn restore_default_audio_device(device_name: &str) {
    let escaped = device_name.replace('\'', "''");
    let cs_code = r#"
public static string Restore(string name) {
    IMMDeviceEnumerator imm = (IMMDeviceEnumerator)new _MMDE();
    IntPtr pC; imm.EnumAudioEndpoints(0,1,out pC);
    IMMDeviceCollection col = (IMMDeviceCollection)Marshal.GetObjectForIUnknown(pC);
    uint n; col.GetCount(out n);
    for (uint i=0;i<n;i++) {
        IntPtr pD; col.Item(i,out pD);
        IMMDevice dev = (IMMDevice)Marshal.GetObjectForIUnknown(pD);
        IntPtr pS; dev.OpenPropertyStore(0,out pS);
        IPropertyStore s = (IPropertyStore)Marshal.GetObjectForIUnknown(pS);
        PROPERTYKEY k = new PROPERTYKEY();
        k.fmtid = Guid.Parse("{A45C254E-DF1C-4EFD-8020-67D146A850E0}");
        k.pid = 14;
        PROPVARIANT v; s.GetValue(ref k, out v);
        string nm = Marshal.PtrToStringUni(v.ptrVal) ?? "";
        if (nm.IndexOf(name,StringComparison.OrdinalIgnoreCase)>=0) {
            string did; dev.GetId(out did);
            object cfg = Activator.CreateInstance(Type.GetTypeFromCLSID(Guid.Parse("{870AF99C-171D-4F9E-AF0D-E63DF40C2BC9}")));
            ((IPolicyConfig)cfg).SetDefaultEndpoint(did,0);
            return "OK";
        }
    }
    return null;
}
"#;

    let ps_script = format!(
        "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8\r\
        $ErrorActionPreference = 'Stop'\r\
        $src = @'\r\
        using System;\r\
        using System.Runtime.InteropServices;\r\
        {com_defs}\r\
        public static class AudioRestore {{\r\
        {cs_code}\r\
        }}\r\
        '@\r\
        Add-Type -TypeDefinition $src -ErrorAction SilentlyContinue | Out-Null\r\
        if ([AudioRestore]::Restore('{escaped}')) {{ }} else {{ }}\r",
        com_defs = AUDIO_COM_DEFS,
        cs_code = cs_code,
        escaped = escaped,
    );

    let temp_dir = std::env::temp_dir();
    let script_path = temp_dir.join("nexbox_audio_restore.ps1");
    let mut bom = vec![0xEF, 0xBB, 0xBF];
    bom.extend_from_slice(ps_script.as_bytes());
    let _ = fs::write(&script_path, &bom);

    let output = Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script_path)
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    let _ = fs::remove_file(&script_path);
    match output {
        Ok(o) => {
            let s = String::from_utf8_lossy(&o.stdout);
            if !s.trim().is_empty() {
                log::info!("[audio_restore] stdout: {}", s.trim());
            }
        }
        Err(e) => log::warn!("[audio_restore] failed: {}", e),
    }
}

/// 获取当前默认音频输出设备名称
#[tauri::command]
pub fn get_default_audio_device() -> Result<String, String> {
    let cs_code = r#"
public static string Get() {
    IMMDeviceEnumerator imm = (IMMDeviceEnumerator)new _MMDE();
    IntPtr pD; imm.GetDefaultAudioEndpoint(0,0,out pD);
    IMMDevice dev = (IMMDevice)Marshal.GetObjectForIUnknown(pD);
    IntPtr pS; dev.OpenPropertyStore(0,out pS);
    IPropertyStore s = (IPropertyStore)Marshal.GetObjectForIUnknown(pS);
    PROPERTYKEY k = new PROPERTYKEY();
    k.fmtid = Guid.Parse("{A45C254E-DF1C-4EFD-8020-67D146A850E0}");
    k.pid = 14;
    PROPVARIANT v; s.GetValue(ref k, out v);
    return Marshal.PtrToStringUni(v.ptrVal) ?? "";
}
"#;

    let ps_script = format!(
        "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8\r\
        $ErrorActionPreference = 'Stop'\r\
        $src = @'\r\
        using System;\r\
        using System.Runtime.InteropServices;\r\
        {com_defs}\r\
        public static class AudioQ {{\r\
        {cs_code}\r\
        }}\r\
        '@\r\
        Add-Type -TypeDefinition $src -ErrorAction Stop | Out-Null\r\
        Write-Output ([AudioQ]::Get())\r",
        com_defs = AUDIO_COM_DEFS,
        cs_code = cs_code,
    );

    let temp_dir = std::env::temp_dir();
    let script_path = temp_dir.join("nexbox_audio_query.ps1");
    let mut bom = vec![0xEF, 0xBB, 0xBF];
    bom.extend_from_slice(ps_script.as_bytes());
    fs::write(&script_path, &bom).map_err(|e| format!("{}", e))?;

    let output = Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script_path)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("{}", e))?;

    let _ = fs::remove_file(&script_path);
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// 停止 EQ 引擎
#[tauri::command]
pub fn stop_eq_engine() -> Result<(), String> {
    // 停止音频引擎
    {
        let mut guard = eq_engine().lock().map_err(|e| format!("锁错误: {}", e))?;
        if let Some(mut engine) = guard.take() {
            engine.stop();
            log::info!("[eq] Audio engine stopped");
        }
    }

    // 恢复原始默认音频设备
    let prev_file = std::env::temp_dir().join("nexbox_eq_prev_device.txt");
    if let Ok(prev_name) = fs::read_to_string(&prev_file) {
        let prev_name = prev_name.trim().to_string();
        if !prev_name.is_empty() && !prev_name.to_lowercase().contains("fxsound") {
            log::info!("[eq] Restoring audio device to: {}", prev_name);
            restore_default_audio_device(&prev_name);
        }
        let _ = fs::remove_file(&prev_file);
    }

    Ok(())
}

/// 获取 EQ 引擎运行状态
#[tauri::command]
pub fn get_eq_engine_status() -> Result<EngineStatus, String> {
    let guard = eq_engine().lock().map_err(|e| format!("锁错误: {}", e))?;
    if let Some(ref engine) = *guard {
        if engine.is_running() {
            return Ok(EngineStatus {
                running: true,
                pid: None,
            });
        }
    }

    Ok(EngineStatus {
        running: false,
        pid: None,
    })
}

/// 实时更新 EQ 频段增益（无需重启引擎）
#[tauri::command]
pub fn update_eq_bands(bands: Vec<(f64, f64)>) -> Result<(), String> {
    let guard = eq_engine().lock().map_err(|e| format!("锁错误: {}", e))?;
    if let Some(ref engine) = *guard {
        let band_params: Vec<BandParam> = bands
            .iter()
            .map(|(freq, gain)| BandParam { freq: *freq, gain: *gain })
            .collect();
        engine.update_bands(band_params);
        log::info!("[eq] Updated {} bands", bands.len());
    }
    Ok(())
}

/// 获取所有内置 EQ 预设
#[tauri::command]
pub fn get_eq_presets(app: AppHandle) -> Result<Vec<EqPreset>, String> {
    let fxvad_dir = get_fxvad_resource_dir(&app)
        .ok_or_else(|| "无法找到资源目录".to_string())?;

    let presets_dir = fxvad_dir.join("presets");
    if !presets_dir.exists() {
        return Ok(Vec::new());
    }

    let mut presets: Vec<EqPreset> = Vec::new();

    let mut entries: Vec<_> = fs::read_dir(&presets_dir)
        .map_err(|e| format!("读取预设目录失败: {}", e))?
        .filter_map(|e| e.ok())
        .collect();

    // 按文件名排序
    entries.sort_by_key(|e| {
        e.file_name()
            .to_string_lossy()
            .trim_end_matches(".fac")
            .parse::<i32>()
            .unwrap_or(999)
    });

    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("fac") {
            continue;
        }

        let file_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        if let Ok(content) = fs::read_to_string(&path) {
            if let Some(preset) = parse_fac_file(&content, &file_id) {
                presets.push(preset);
            }
        }
    }

    Ok(presets)
}

/// 应用 EQ 预设（更新引擎中的频段参数）
#[tauri::command]
pub fn apply_eq_preset(app: AppHandle, preset_id: String) -> Result<(), String> {
    let fxvad_dir = get_fxvad_resource_dir(&app)
        .ok_or_else(|| "无法找到资源目录".to_string())?;

    let preset_file = fxvad_dir.join("presets").join(format!("{}.fac", preset_id));
    if !preset_file.exists() {
        return Err(format!("预设 {} 不存在", preset_id));
    }

    let content = fs::read_to_string(&preset_file)
        .map_err(|e| format!("读取预设失败: {}", e))?;
    let preset = parse_fac_file(&content, &preset_id);

    if let Some(p) = &preset {
        // 更新引擎中的频段
        let guard = eq_engine().lock().map_err(|e| format!("锁错误: {}", e))?;
        if let Some(ref engine) = *guard {
            let band_params: Vec<BandParam> = p.bands
                .iter()
                .map(|b| BandParam { freq: b.freq, gain: b.gain })
                .collect();
            engine.update_bands(band_params);
            log::info!("[eq] Applied preset '{}' with {} bands", p.name, p.bands.len());
        }
    }

    Ok(())
}

/// 清理 EQ 相关资源（应用退出时调用）
pub fn cleanup() {
    let _ = stop_eq_engine();
}
