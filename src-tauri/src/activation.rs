use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use tauri::{Emitter, Manager};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Serialize, Deserialize)]
pub struct ActivationStatus {
    pub name: Option<String>,
    pub description: Option<String>,
    pub license_status: Option<u32>,
    pub license_status_text: Option<String>,
    pub partial_product_key: Option<String>,
    pub is_activated: bool,
}

impl Default for ActivationStatus {
    fn default() -> Self {
        Self {
            name: None,
            description: None,
            license_status: None,
            license_status_text: None,
            partial_product_key: None,
            is_activated: false,
        }
    }
}

#[derive(Debug, Deserialize)]
struct WmiSoftwareLicensingProduct {
    #[serde(default)]
    Name: Option<String>,
    #[serde(default)]
    Description: Option<String>,
    #[serde(default)]
    LicenseStatus: Option<u32>,
    #[serde(default)]
    PartialProductKey: Option<String>,
}

fn license_status_to_text(status: u32) -> String {
    match status {
        0 => "未授权",
        1 => "已授权",
        2 => "OOB初始宽限期",
        3 => "OOT额外宽限期",
        4 => "通知模式",
        5 => "延长期",
        6 => "延长期已过期",
        _ => "未知",
    }
    .to_string()
}

/// 单次 PowerShell 调用，获取 JSON 结果
fn run_powershell_json<T: for<'de> Deserialize<'de>>(command: &str) -> Result<Vec<T>, String> {
    let mut cmd = Command::new("powershell");
    cmd.args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", command]);

    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd.output().map_err(|e| format!("执行PowerShell失败: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!("PowerShell执行失败: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();

    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let json_array = if trimmed.starts_with('[') {
        trimmed.to_string()
    } else {
        format!("[{}]", trimmed)
    };

    let results: Vec<T> =
        serde_json::from_str(&json_array).map_err(|e| format!("解析JSON失败: {}", e))?;
    Ok(results)
}

/// 一次 PowerShell 调用完成全部激活状态查询（避免多次 spawn）
#[tauri::command]
pub async fn check_windows_activation() -> Result<ActivationStatus, String> {
    // 使用 WQL 过滤条件直接在 WMI 层过滤，比 PowerShell Where-Object 快得多
    let ps_command = r#"
$products = @(Get-CimInstance -Query "SELECT Name, Description, LicenseStatus, PartialProductKey FROM SoftwareLicensingProduct WHERE Name LIKE '%Windows%' AND PartialProductKey IS NOT NULL" -ErrorAction SilentlyContinue)
if ($products.Count -eq 0) {
    $products = @(Get-CimInstance -Query "SELECT Name, Description, LicenseStatus, PartialProductKey FROM SoftwareLicensingProduct WHERE Name LIKE '%Windows%'" -ErrorAction SilentlyContinue)
}
if ($products.Count -eq 0) {
    $products = @(Get-WmiObject -Query "SELECT Name, Description, LicenseStatus, PartialProductKey FROM SoftwareLicensingProduct WHERE Name LIKE '%Windows%' AND PartialProductKey IS NOT NULL" -ErrorAction SilentlyContinue)
}
if ($products.Count -eq 0) {
    $products = @(Get-WmiObject -Query "SELECT Name, Description, LicenseStatus, PartialProductKey FROM SoftwareLicensingProduct WHERE Name LIKE '%Windows%'" -ErrorAction SilentlyContinue)
}
$products | Select-Object -First 1 Name, Description, LicenseStatus, PartialProductKey | ConvertTo-Json -Compress
"#;

    let results: Vec<WmiSoftwareLicensingProduct> = run_powershell_json(ps_command)?;

    if let Some(product) = results.first() {
        let license_status = product.LicenseStatus.unwrap_or(0);
        Ok(ActivationStatus {
            name: product.Name.clone(),
            description: product.Description.clone(),
            license_status: Some(license_status),
            license_status_text: Some(license_status_to_text(license_status)),
            partial_product_key: product.PartialProductKey.clone(),
            is_activated: license_status == 1,
        })
    } else {
        Ok(ActivationStatus::default())
    }
}

/// 查找 MAS 激活脚本，支持 dev/prod 多种路径
fn resolve_script_path(app: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    let script_name = "MAS_AIO_CN_v3.11.cmd";

    // 优先使用文件系统路径（可靠），Tauri 资源目录作为后备

    // 1) exe 相对路径 — 优先，最可靠
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let direct = exe_dir.join(script_name);
            if direct.exists() {
                return Some(direct);
            }
            // exe 在 target/debug 下，向上逐层查找项目根
            for up in &[1, 2, 3, 4] {
                let mut p = exe_dir.to_path_buf();
                for _ in 0..*up {
                    p.push("..");
                }
                let candidate = p.join("mas-cn-main").join("MAS").join(script_name);
                if candidate.exists() {
                    return std::fs::canonicalize(&candidate).ok().or(Some(candidate));
                }
            }
        }
    }

    // 2) 工作目录相对路径
    if let Ok(cwd) = std::env::current_dir() {
        for up in &[0, 1, 2] {
            let mut p = cwd.clone();
            for _ in 0..*up {
                p.push("..");
            }
            let candidate = p.join("mas-cn-main").join("MAS").join(script_name);
            if candidate.exists() {
                return std::fs::canonicalize(&candidate).ok().or(Some(candidate));
            }
        }
    }

    // 3) Tauri 资源目录（production build / dev _up_）
    if let Ok(resource_dir) = app.path().resource_dir() {
        for candidate in &[
            resource_dir.join(script_name),
            resource_dir.join("_up_").join("mas-cn-main").join("MAS").join(script_name),
        ] {
            if candidate.exists() {
                return Some(candidate.clone());
            }
        }
    }

    None
}

/// GBK 字节解码为 String（Windows 上使用系统 API，非 Windows 回退 lossy UTF-8）
fn decode_gbk(bytes: &[u8]) -> String {
    #[cfg(windows)]
    {
        if bytes.is_empty() {
            return String::new();
        }
        // 使用 Windows OEM 代码页（简体中文系统为 936/GBK）
        let wide_count = unsafe {
            windows_sys::Win32::Globalization::MultiByteToWideChar(
                936, // CP_GBK
                0,
                bytes.as_ptr(),
                bytes.len() as i32,
                std::ptr::null_mut(),
                0,
            )
        };
        if wide_count > 0 {
            let mut wide_buf: Vec<u16> = vec![0; wide_count as usize];
            unsafe {
                windows_sys::Win32::Globalization::MultiByteToWideChar(
                    936,
                    0,
                    bytes.as_ptr(),
                    bytes.len() as i32,
                    wide_buf.as_mut_ptr(),
                    wide_count,
                );
            }
            String::from_utf16_lossy(&wide_buf)
        } else {
            String::from_utf8_lossy(bytes).to_string()
        }
    }
    #[cfg(not(windows))]
    {
        String::from_utf8_lossy(bytes).to_string()
    }
}

/// 从脚本进程 stdout 实时读取 GBK 输出并 emit 事件
fn stream_script_output<R: Read + Send + 'static>(
    stdout: R,
    app_handle: &tauri::AppHandle,
) {
    let reader = BufReader::new(stdout);
    for line in reader.split(b'\n') {
        match line {
            Ok(bytes) => {
                // 去掉末尾的 \r
                let trimmed = if bytes.ends_with(b"\r") {
                    &bytes[..bytes.len() - 1]
                } else {
                    &bytes
                };
                let text = decode_gbk(trimmed);
                let _ = app_handle.emit("activation-output", &text);
            }
            Err(e) => {
                let _ = app_handle.emit("activation-output", format!("[读取错误] {}", e));
            }
        }
    }
}

/// 去掉 Windows \\?\ 扩展路径前缀，cmd.exe 不支持此格式
fn strip_verbatim_prefix(path: &std::path::Path) -> String {
    let s = path.to_string_lossy();
    if s.starts_with(r"\\?\") {
        s[4..].to_string()
    } else {
        s.to_string()
    }
}

#[tauri::command]
pub async fn run_windows_activation(
    app_handle: tauri::AppHandle,
    method: String,
) -> Result<String, String> {
    let script_arg = match method.as_str() {
        "hwid" => "/HWID",
        "kms" => "/KMS",
        "tsforge" => "/TSforge",
        _ => return Err(format!("不支持的激活方法: {}", method)),
    };

    let script_path = resolve_script_path(&app_handle)
        .ok_or_else(|| format!("找不到激活脚本 MAS_AIO_CN_v3.11.cmd，请确保脚本已正确部署"))?;

    let script_path_str = strip_verbatim_prefix(&script_path);

    // 直接运行脚本，不加 chcp/call，避免 cmd 解析问题
    // 路径不含空格无需引号，用 /d 禁用 AutoRun
    let command_line = format!("{} {}", script_path_str, script_arg);

    let mut cmd = Command::new("cmd.exe");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.raw_arg(format!("/d /c {}", command_line));
    }
    #[cfg(not(windows))]
    {
        cmd.args(&["/d", "/c", &command_line]);
    }

    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("启动激活脚本失败: {}", e))?;

    // 实时读取 stdout（GBK 编码）
    if let Some(stdout) = child.stdout.take() {
        stream_script_output(stdout, &app_handle);
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("等待激活脚本完成失败: {}", e))?;

    // stderr 也按 GBK 解码
    if !output.stderr.is_empty() {
        for line in output.stderr.split(|&b| b == b'\n') {
            let trimmed = if line.ends_with(b"\r") {
                &line[..line.len() - 1]
            } else {
                line
            };
            let text = decode_gbk(trimmed);
            if !text.trim().is_empty() {
                let _ = app_handle.emit("activation-output", text);
            }
        }
    }

    if output.status.success() {
        let _ = app_handle.emit("activation-output", "--- 激活脚本执行完毕 ---");
        Ok("激活脚本执行完成".to_string())
    } else {
        let exit_code = output.status.code().unwrap_or(-1);
        let _ = app_handle.emit(
            "activation-output",
            format!("--- 脚本退出，退出码: {} ---", exit_code),
        );
        Ok(format!("激活脚本执行完毕（退出码: {}）", exit_code))
    }
}
