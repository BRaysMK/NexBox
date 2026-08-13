use serde::Serialize;
use std::process::Command;

use crate::optimization;

/// defrag.exe 输出使用系统本地代码页（中文为 GBK），需正确解码避免乱码。
fn decode_console(bytes: &[u8]) -> String {
    let (cow, _, _) = encoding_rs::GBK.decode(bytes);
    cow.into_owned()
}

/// 判断是否为 SSD/固态介质，决定走 TRIM/优化而非传统碎片整理。
fn is_ssd_media(media_type: &str) -> bool {
    let t = media_type.to_lowercase();
    t.contains("ssd")
        || t.contains("solid state")
        || t.contains("nvme")
        || t.contains("flash")
}

#[derive(Serialize)]
pub struct DiskOptimizeResult {
    pub drive_letter: String,
    /// defrag=传统碎片整理(机械盘), retrim=TRIM/优化(固态盘), auto=由系统自动判断
    pub operation: String,
    pub is_ssd: bool,
    pub success: bool,
    pub message: String,
}

/// 调用系统 defrag.exe，比 PowerShell 启动更快、开销更低。
/// 介质区分：HDD 用 /U（传统整理），SSD 用 /L（TRIM/retrim），未知用 /O（系统自动选择）。
fn run_defrag(drive_letter: &str, operation: &str) -> Result<String, String> {
    let target = format!("{}:", drive_letter);
    let mut cmd = Command::new("defrag.exe");
    match operation {
        "retrim" => {
            cmd.args([&target, "/L"]);
        }
        "defrag" => {
            cmd.args([&target, "/U"]);
        }
        _ => {
            cmd.args([&target, "/O"]);
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("执行整理命令失败: {}", e))?;
    let stdout = decode_console(&output.stdout);
    let stderr = decode_console(&output.stderr);

    if output.status.success() {
        // 取输出的末尾几行作为结果反馈
        let lines: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
        let tail = lines
            .iter()
            .rev()
            .take(4)
            .rev()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        Ok(if tail.trim().is_empty() {
            "操作完成".to_string()
        } else {
            tail
        })
    } else {
        Err(if stderr.trim().is_empty() { stdout } else { stderr })
    }
}

/// 对指定盘符执行磁盘整理/优化。
/// - HDD 机械盘：传统碎片整理 (Optimize-Volume -Defrag)
/// - SSD 固态盘：TRIM/优化 (Optimize-Volume -ReTrim)，避免做无意义的碎片整理损耗寿命
/// - 未知介质：交给系统自动判断
#[tauri::command]
pub async fn optimize_disk(
    drive_letter: String,
    media_type: String,
) -> Result<DiskOptimizeResult, String> {
    if !optimization::is_admin() {
        return Err("此操作需要管理员权限，请以管理员身份运行 NexBox 后重试".to_string());
    }

    let letter = drive_letter.trim().trim_end_matches(':').to_string();
    if letter.is_empty() {
        return Err("无效的盘符".to_string());
    }

    let is_ssd = is_ssd_media(&media_type);
    let operation = if is_ssd {
        "retrim"
    } else if media_type.trim().is_empty() {
        "auto"
    } else {
        "defrag"
    };

    let task_letter = letter.clone();
    let task_operation = operation.to_string();
    let task_is_ssd = is_ssd;
    let run_letter = task_letter.clone();
    let run_op = task_operation.clone();

    let run = tauri::async_runtime::spawn_blocking(move || run_defrag(&run_letter, &run_op))
        .await
        .map_err(|e| format!("异步任务失败: {}", e))?;

    match run {
        Ok(msg) => Ok(DiskOptimizeResult {
            drive_letter: task_letter,
            operation: task_operation,
            is_ssd: task_is_ssd,
            success: true,
            message: msg,
        }),
        Err(e) => Err(e),
    }
}
