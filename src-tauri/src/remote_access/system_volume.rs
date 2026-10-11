//! PC 系统主音量：IAudioEndpointVolume 读写（手机端音量弹窗用）。

use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{eConsole, eRender, MMDeviceEnumerator, IMMDeviceEnumerator};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED};

fn with_endpoint_volume<T>(f: impl FnOnce(&IAudioEndpointVolume) -> Result<T, String>) -> Result<T, String> {
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    // 只把 WASAPI 端点激活包进 unsafe：f 里对端点音量的调用保持安全上下文，
    // 避免调用方被迫写嵌套 unsafe
    let endpoint: Result<IAudioEndpointVolume, String> = (|| unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| format!("CoCreateInstance 失败: {e}"))?;
        let device = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|e| format!("取默认渲染设备失败: {e}"))?;
        device
            .Activate(CLSCTX_ALL, None)
            .map_err(|e| format!("Activate 端点音量失败: {e}"))
    })();
    let out = match endpoint {
        Ok(ep) => f(&ep),
        Err(e) => Err(e),
    };
    unsafe { CoUninitialize() };
    out
}

/// 设置 PC 系统主音量（0..100）
pub fn set_pc_volume(percent: f32) -> Result<(), String> {
    let v = (percent / 100.0).clamp(0.0, 1.0);
    with_endpoint_volume(|ep| {
        unsafe { ep.SetMasterVolumeLevelScalar(v, std::ptr::null()) }.map_err(|e| format!("设置音量失败: {e}"))
    })
}

/// 读取 PC 系统主音量（0..100）
pub fn get_pc_volume() -> Result<f32, String> {
    with_endpoint_volume(|ep| {
        let v = unsafe { ep.GetMasterVolumeLevelScalar() }.map_err(|e| format!("读音量失败: {e}"))?;
        Ok((v * 100.0).clamp(0.0, 100.0))
    })
}
