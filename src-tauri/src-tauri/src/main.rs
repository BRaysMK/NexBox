// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
  // 开机自启修复：Windows 通过注册表 Run 键启动程序时工作目录是 System32，
  // 这会导致 Tauri 加载资源和依赖失败。主动切换到 exe 所在目录。
  if let Ok(exe_path) = std::env::current_exe() {
    if let Some(exe_dir) = exe_path.parent() {
      let _ = std::env::set_current_dir(exe_dir);
    }
  }

  nexbox_lib::run();
}
