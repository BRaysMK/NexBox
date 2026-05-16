use std::process::Child;
use std::sync::Mutex;

use tauri::{App, AppHandle, Manager};

pub struct SensorChild(pub Mutex<Option<Child>>);

pub fn start_sensor_process(app: &App) {
  match spawn_sensor() {
    Ok(Some(child)) => {
      log::info!("已启动传感器子进程 (pid={})", child.id());
      app.manage(SensorChild(Mutex::new(Some(child))));
    }
    Ok(None) => {
      app.manage(SensorChild(Mutex::new(None)));
    }
    Err(e) => {
      log::warn!("启动传感器服务失败: {e}");
      app.manage(SensorChild(Mutex::new(None)));
    }
  }
}

pub fn stop_sensor_process(app: &AppHandle) {
  let Some(state) = app.try_state::<SensorChild>() else {
    return;
  };
  let child = {
    let mut guard = state
      .0
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.take()
  };
  if let Some(mut child) = child {
    log::info!("正在停止传感器子进程 (pid={})", child.id());
    let _ = child.kill();
    let _ = child.wait();
  }
}

fn spawn_sensor() -> std::io::Result<Option<Child>> {
  // C# 传感器后端已移除，直接返回 None
  log::info!("C# 传感器后端已移除，不启动传感器进程。");
  Ok(None)
}
