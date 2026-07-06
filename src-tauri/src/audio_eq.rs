//! WASAPI 音频均衡器模块
//! 
//! 通过 WASAPI Loopback 捕获系统全局音频输出 → 10段Biquad EQ处理 → 输出至播放设备
//! 
//! 架构：
//! - 全局状态：Mutex<Option<PipelineState>> 管理音频管线生命周期
//! - 后台线程：处理音频数据流，由 Arc<AtomicBool> 控制启停
//! - Biquad滤波器：RBJ Peaking EQ 直接II型转置，10段级联，每声道独立状态
//! - 声道转换：捕获端与渲染端声道数/采样率可能不同，写入前做转换

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use tauri::Emitter;
use windows::core::Interface;
use windows::Win32::Media::Audio::{
    eConsole, eRender, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
    DEVICE_STATE_ACTIVE, IAudioCaptureClient, IAudioClient, IAudioRenderClient,
    IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, CoCreateInstance, CoInitializeEx, CoUninitialize,
    COINIT_MULTITHREADED,
};

// ─── 常量 ───

/// 10段 EQ 中心频率 & Q 值
const EQ_BANDS: [(f32, f32); 10] = [
    (31.0, 1.41), (62.0, 1.41), (125.0, 1.41),
    (250.0, 1.41), (500.0, 1.41), (1000.0, 1.41),
    (2000.0, 1.41), (4000.0, 1.41), (8000.0, 1.41),
    (16000.0, 1.41),
];

/// WASAPI REFERENCE_TIME per millisecond
const REFTIMES_PER_MILLISEC: i64 = 10_000;

/// AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY
const AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY: u32 = 0x1;

// ─── 数据结构 ───

/// 音频设备信息（前端用）
#[derive(serde::Serialize, Clone, Debug)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// EQ 设置（前端 ↔ 后端通信 + 持久化用）
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct EqSettings {
    pub enabled: bool,
    pub bands: [f32; 10],
    pub master_gain: f32,
    pub output_device_id: String,
    pub preset_id: String,
}

impl Default for EqSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            bands: [0.0; 10],
            master_gain: 0.0,
            output_device_id: "default".to_string(),
            preset_id: "flat".to_string(),
        }
    }
}

// ─── 全局状态 ───

struct PipelineState {
    settings: EqSettings,
    is_running: Arc<AtomicBool>,
    thread_handle: Option<thread::JoinHandle<()>>,
    eq_filters: Arc<Mutex<Vec<[BiquadFilter; 10]>>>,
    sample_rate: Arc<AtomicU32>,
}

static PIPELINE: Mutex<Option<PipelineState>> = Mutex::new(None);

// ─── Biquad 滤波器 ───

#[derive(Clone, Debug)]
struct BiquadFilter {
    b0: f32, b1: f32, b2: f32,
    a1: f32, a2: f32,
    z1: f32, z2: f32,
}

impl BiquadFilter {
    fn peaking_eq(freq: f32, q: f32, gain_db: f32, sample_rate: f32) -> Self {
        let a = 10.0_f32.powf(gain_db / 40.0);
        let omega = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let sn = omega.sin();
        let cs = omega.cos();
        let alpha = sn / (2.0 * q);

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cs;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1_n = -2.0 * cs;
        let a2_n = 1.0 - alpha / a;

        Self {
            b0: b0 / a0, b1: b1 / a0, b2: b2 / a0,
            a1: a1_n / a0, a2: a2_n / a0,
            z1: 0.0, z2: 0.0,
        }
    }

    fn update_peaking_eq(&mut self, freq: f32, q: f32, gain_db: f32, sample_rate: f32) {
        let a = 10.0_f32.powf(gain_db / 40.0);
        let omega = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let sn = omega.sin();
        let cs = omega.cos();
        let alpha = sn / (2.0 * q);

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cs;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1_n = -2.0 * cs;
        let a2_n = 1.0 - alpha / a;

        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1_n / a0;
        self.a2 = a2_n / a0;
    }

    #[inline]
    fn process(&mut self, sample: f32) -> f32 {
        let out = self.b0 * sample + self.z1;
        self.z1 = self.b1 * sample - self.a1 * out + self.z2;
        self.z2 = self.b2 * sample - self.a2 * out;
        if self.z1.abs() < 1e-38 { self.z1 = 0.0; }
        if self.z2.abs() < 1e-38 { self.z2 = 0.0; }
        out
    }

    fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

fn create_default_filters(sample_rate: f32) -> [BiquadFilter; 10] {
    std::array::from_fn(|i| {
        BiquadFilter::peaking_eq(EQ_BANDS[i].0, EQ_BANDS[i].1, 0.0, sample_rate)
    })
}

fn create_filters_from_settings(settings: &EqSettings, sample_rate: f32) -> [BiquadFilter; 10] {
    let mut chain = create_default_filters(sample_rate);
    for (i, &gain) in settings.bands.iter().enumerate() {
        if gain.abs() > 0.01 {
            chain[i] = BiquadFilter::peaking_eq(EQ_BANDS[i].0, EQ_BANDS[i].1, gain, sample_rate);
        }
    }
    chain
}

/// 软限幅器：低于 0.9 完全透过，高于 0.9 平滑压缩
#[inline]
fn soft_clip(x: f32) -> f32 {
    const THRESHOLD: f32 = 0.9;
    if x.abs() <= THRESHOLD {
        x
    } else {
        let sign = x.signum();
        let over = x.abs() - THRESHOLD;
        sign * (THRESHOLD + (1.0 - THRESHOLD) * over.tanh())
    }
}

// ─── 声道转换 ───

/// 将交错排列的音频数据从 in_ch 声道转换为 out_ch 声道
/// 支持常见的 stereo↔mono、stereo↔surround 转换
fn convert_channels(input: &[f32], in_ch: usize, out_ch: usize, frames: usize) -> Vec<f32> {
    if in_ch == out_ch || out_ch == 0 || in_ch == 0 {
        return input.to_vec();
    }
    let mut output = vec![0.0_f32; frames * out_ch];
    for frame in 0..frames {
        let in_off = frame * in_ch;
        let out_off = frame * out_ch;
        match (in_ch, out_ch) {
            // mono → stereo
            (1, 2) => {
                output[out_off] = input[in_off];
                output[out_off + 1] = input[in_off];
            }
            // stereo → mono
            (2, 1) => {
                output[out_off] = (input[in_off] + input[in_off + 1]) * 0.5;
            }
            // stereo → multi-channel (复制到前两声道，其余静音)
            (2, _) => {
                output[out_off] = input[in_off];
                output[out_off + 1] = input[in_off + 1];
            }
            // multi → stereo (取前两声道)
            (_, 2) => {
                output[out_off] = input[in_off];
                output[out_off + 1] = if in_ch > 1 { input[in_off + 1] } else { input[in_off] };
            }
            // mono → multi
            (1, _) => {
                for c in 0..out_ch {
                    output[out_off + c] = input[in_off];
                }
            }
            // multi → mono (所有声道平均)
            (_, 1) => {
                let mut sum = 0.0_f32;
                for c in 0..in_ch {
                    sum += input[in_off + c];
                }
                output[out_off] = sum / in_ch as f32;
            }
            // fallback: 复制可重叠的声道
            _ => {
                let copy_ch = in_ch.min(out_ch);
                for c in 0..copy_ch {
                    output[out_off + c] = input[in_off + c];
                }
            }
        }
    }
    output
}

// ─── WASAPI 设备枚举 ───

/// 查找一个不同于默认捕获设备的渲染设备
/// 注意：调用者需确保 COM 已初始化
fn find_alternative_render_device(default_id: &str) -> Option<String> {
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let collection = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE).ok()?;
        let count = collection.GetCount().ok()?;

        for i in 0..count {
            let device = collection.Item(i).ok()?;
            let device: IMMDevice = device.cast().ok()?;
            let id_ptr = device.GetId().ok()?;
            let id = id_ptr.to_string().ok()?;
            if id != default_id {
                return Some(id);
            }
        }
    }
    None
}

/// 获取默认输出设备 ID
fn get_default_device_id() -> Option<String> {
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
        let device: IMMDevice = device.cast().ok()?;
        let id_ptr = device.GetId().ok()?;
        let id = id_ptr.to_string().ok()?;
        Some(id)
    }
}

// ─── WASAPI 初始化 ───

/// 初始化 WASAPI 捕获客户端（Loopback）
/// 返回 (audio_client, capture_client, sample_rate, channels)
unsafe fn init_capture_client(
    enumerator: &IMMDeviceEnumerator,
) -> Result<(IAudioClient, IAudioCaptureClient, u32, u16), String> {
    let device: IMMDevice = enumerator
        .GetDefaultAudioEndpoint(eRender, eConsole)
        .map_err(|e| format!("获取默认渲染设备失败: {:?}", e))?;
    let device: IMMDevice = device.cast().map_err(|e| format!("设备转换失败: {:?}", e))?;

    let audio_client: IAudioClient = device
        .Activate(CLSCTX_ALL, None)
        .map_err(|e| format!("激活音频客户端失败: {:?}", e))?;

    let mix_format_ptr = audio_client
        .GetMixFormat()
        .map_err(|e| format!("获取混音格式失败: {:?}", e))?;
    let mix_format = &*mix_format_ptr;
    let sample_rate = mix_format.nSamplesPerSec;
    let channels = mix_format.nChannels;
    let bits_per_sample = mix_format.wBitsPerSample;
    let format_tag = mix_format.wFormatTag;

    log::info!(
        "Loopback 捕获格式: {} Hz, {} 声道, {} bits, tag=0x{:04X}",
        sample_rate, channels, bits_per_sample, format_tag
    );

    // 验证格式为 float32
    if bits_per_sample != 32 {
        return Err(format!(
            "捕获格式不是 32-bit float (当前 {} bits)，EQ 暂不支持此格式",
            bits_per_sample
        ));
    }

    let buffer_duration = REFTIMES_PER_MILLISEC * 10;
    audio_client
        .Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK,
            buffer_duration,
            0,
            mix_format_ptr as *const _,
            None,
        )
        .map_err(|e| format!("初始化捕获客户端失败: {:?}", e))?;

    let capture_client: IAudioCaptureClient = audio_client
        .GetService()
        .map_err(|e| format!("获取捕获客户端服务失败: {:?}", e))?;

    Ok((audio_client, capture_client, sample_rate, channels))
}

/// 初始化 WASAPI 渲染客户端（输出到指定设备）
/// 返回 (audio_client, render_client, buffer_frame_count, sample_rate, channels)
unsafe fn init_render_client_for_device(
    enumerator: &IMMDeviceEnumerator,
    device_id: &str,
) -> Result<(IAudioClient, IAudioRenderClient, u32, u32, u16), String> {
    let output_device: IMMDevice = if device_id.is_empty() || device_id == "default" {
        enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|e| format!("获取默认输出设备失败: {:?}", e))?
    } else {
        let device_str = windows::core::BSTR::from(device_id);
        enumerator
            .GetDevice(&device_str)
            .map_err(|e| format!("获取指定输出设备失败: {:?}", e))?
    };

    let output_device: IMMDevice = output_device
        .cast()
        .map_err(|e| format!("输出设备转换失败: {:?}", e))?;

    let render_client: IAudioClient = output_device
        .Activate(CLSCTX_ALL, None)
        .map_err(|e| format!("激活渲染客户端失败: {:?}", e))?;

    let mix_format_ptr = render_client
        .GetMixFormat()
        .map_err(|e| format!("获取输出混音格式失败: {:?}", e))?;

    let bits = (*mix_format_ptr).wBitsPerSample;
    let tag = (*mix_format_ptr).wFormatTag;
    let sr = (*mix_format_ptr).nSamplesPerSec;
    let ch = (*mix_format_ptr).nChannels;

    log::info!(
        "渲染混音格式: {} Hz, {} ch, {} bits, tag=0x{:04X}",
        sr, ch, bits, tag
    );

    if bits != 32 {
        return Err(format!(
            "渲染设备格式不是 32-bit float (当前 {} bits), EQ 暂不支持此格式",
            bits
        ));
    }

    let buffer_duration = REFTIMES_PER_MILLISEC * 10;
    render_client
        .Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            0,
            buffer_duration,
            0,
            &*mix_format_ptr as *const _ as *const _,
            None,
        )
        .map_err(|e| format!("初始化渲染客户端失败: {:?}", e))?;

    let render_service: IAudioRenderClient = render_client
        .GetService()
        .map_err(|e| format!("获取渲染客户端服务失败: {:?}", e))?;

    let buffer_frame_count = render_client
        .GetBufferSize()
        .map_err(|e| format!("获取缓冲区大小失败: {:?}", e))?;

    Ok((render_client, render_service, buffer_frame_count, sr, ch))
}

// ─── 音频处理线程 ───

fn run_audio_pipeline(
    filter_chain: Arc<Mutex<Vec<[BiquadFilter; 10]>>>,
    is_running: Arc<AtomicBool>,
    app: tauri::AppHandle,
    eq_settings: Arc<Mutex<EqSettings>>,
    sample_rate_arc: Arc<AtomicU32>,
) {
    log::info!("音频处理线程启动");

    let capture_sample_rate: u32;
    let capture_channels: u16;

    unsafe {
        let com_hr = CoInitializeEx(None, COINIT_MULTITHREADED);
        if com_hr.is_err() {
            log::error!("线程 COM 初始化失败: {:?}", com_hr);
            return;
        }

        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .expect("创建设备枚举器失败");

        // ── 初始化捕获 ──
        let (capture_client, capture_service, sr, ch) = match init_capture_client(&enumerator) {
            Ok(v) => v,
            Err(e) => {
                log::error!("捕获客户端初始化失败: {}", e);
                CoUninitialize();
                return;
            }
        };
        capture_sample_rate = sr;
        capture_channels = ch;

        if capture_channels == 0 {
            log::error!("声道数为 0，无法启动音频管线");
            let _ = capture_client.Stop();
            CoUninitialize();
            return;
        }

        log::info!("捕获: {}Hz, {}声道", capture_sample_rate, capture_channels);
        sample_rate_arc.store(capture_sample_rate, Ordering::SeqCst);

        // 用实际采样率和声道数重建滤波器组
        {
            let settings = eq_settings.lock().unwrap();
            let mut filters = filter_chain.lock().unwrap();
            *filters = (0..capture_channels as usize)
                .map(|_| create_filters_from_settings(&settings, capture_sample_rate as f32))
                .collect();
            log::info!(
                "滤波器组: {}声道 × 10段 @ {}Hz",
                capture_channels, capture_sample_rate
            );
        }

        // ── 初始化渲染 ──
        let device_id = eq_settings.lock().unwrap().output_device_id.clone();

        let (render_client, render_service, buf_frames, render_sr, render_ch) =
            match init_render_client_for_device(&enumerator, &device_id) {
                Ok(v) => v,
                Err(e) => {
                    log::error!("渲染客户端初始化失败: {}", e);
                    CoUninitialize();
                    return;
                }
            };

        log::info!(
            "渲染: {}Hz, {}声道, {}帧缓冲区",
            render_sr, render_ch, buf_frames
        );

        // ── 格式差异警告 ──
        let need_channel_convert = render_ch != capture_channels;
        let need_resample = render_sr != capture_sample_rate;

        if need_channel_convert {
            log::warn!(
                "声道数不匹配: 捕获{}ch → 渲染{}ch，将自动转换",
                capture_channels, render_ch
            );
        }
        if need_resample {
            log::warn!(
                "采样率不匹配: 捕获{}Hz → 渲染{}Hz，将自动重采样",
                capture_sample_rate, render_sr
            );
        }

        let cap_ch = capture_channels as usize;
        let rnd_ch = render_ch as usize;
        // 采样率差异目前仅记录日志，暂不做重采样（WASAPI 共享模式通常一致）

        // ── 启动音频流 ──
        let _ = render_client.Start();
        let _ = capture_client.Start();

        let mut local_stats_counter: u64 = 0;
        let mut overflow_buf: Vec<f32> = Vec::new();

        while is_running.load(Ordering::SeqCst) {

            // ── Phase 1: 排空溢出缓冲区到渲染 ──
            let overflow_frames = (overflow_buf.len() / cap_ch) as u32;
            if overflow_frames > 0 {
                let padding = render_client.GetCurrentPadding().unwrap_or(0);
                let available = buf_frames.saturating_sub(padding);
                let drain_frames = overflow_frames.min(available);
                if drain_frames > 0 {
                    match render_service.GetBuffer(drain_frames) {
                        Ok(render_ptr) if !render_ptr.is_null() => {
                            let render_data = std::slice::from_raw_parts_mut(
                                render_ptr as *mut f32,
                                drain_frames as usize * rnd_ch,
                            );

                            if need_channel_convert || need_resample {
                                // 需要声道转换：从 overflow_buf 取数据，转换后写入
                                let src_data = &overflow_buf[..drain_frames as usize * cap_ch];
                                let converted = convert_channels(src_data, cap_ch, rnd_ch, drain_frames as usize);
                                let copy_len = converted.len().min(render_data.len());
                                render_data[..copy_len].copy_from_slice(&converted[..copy_len]);
                            } else {
                                // 格式一致，直接复制
                                let copy_samples = drain_frames as usize * cap_ch;
                                render_data[..copy_samples].copy_from_slice(&overflow_buf[..copy_samples]);
                            }

                            let _ = render_service.ReleaseBuffer(drain_frames, 0);
                            overflow_buf.drain(..drain_frames as usize * cap_ch);
                        }
                        Err(e) => log::error!("渲染溢出写出错: {:?}", e),
                        _ => {}
                    }
                }
            }

            // ── Phase 2: 从捕获读取数据 ──
            let mut data_ptr: *mut u8 = std::ptr::null_mut();
            let mut frames_available: u32 = 0;
            let mut flags: u32 = 0;

            let hr = capture_service.GetBuffer(
                &mut data_ptr, &mut frames_available, &mut flags, None, None,
            );

            if hr.is_ok() && frames_available > 0 && !data_ptr.is_null() {
                let frame_samples = frames_available as usize * cap_ch;
                let float_slice = std::slice::from_raw_parts(data_ptr as *const f32, frame_samples);
                overflow_buf.extend_from_slice(float_slice);
                let _ = capture_service.ReleaseBuffer(frames_available);

                let discontinuity = (flags & AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY) != 0;

                // ── Phase 3: EQ 处理（只处理新追加的部分）──
                let new_start = overflow_buf.len().saturating_sub(frame_samples);
                let new_slice = &mut overflow_buf[new_start..];

                let master_gain_db = eq_settings.lock().unwrap().master_gain;
                let master_gain_linear = 10.0_f32.powf(master_gain_db / 20.0);

                {
                    let mut filters = filter_chain.lock().unwrap();

                    if discontinuity {
                        for ch_filters in filters.iter_mut() {
                            for f in ch_filters.iter_mut() {
                                f.reset();
                            }
                        }
                        log::debug!("捕获数据不连续，已重置滤波器");
                    }

                    for frame in 0..frames_available as usize {
                        for ch_idx in 0..cap_ch {
                            let idx = frame * cap_ch + ch_idx;
                            let mut sample = new_slice[idx];
                            if let Some(ch_filters) = filters.get_mut(ch_idx) {
                                for filter in ch_filters.iter_mut() {
                                    sample = filter.process(sample);
                                }
                            }
                            sample *= master_gain_linear;
                            new_slice[idx] = soft_clip(sample);
                        }
                    }
                }

                // ── Phase 4: 尝试写入渲染 ──
                let new_frames = (new_slice.len() / cap_ch) as u32;
                let padding2 = render_client.GetCurrentPadding().unwrap_or(0);
                let available2 = buf_frames.saturating_sub(padding2);
                let write_frames = new_frames.min(available2);

                if write_frames > 0 {
                    match render_service.GetBuffer(write_frames) {
                        Ok(render_ptr) if !render_ptr.is_null() => {
                            let render_data = std::slice::from_raw_parts_mut(
                                render_ptr as *mut f32,
                                write_frames as usize * rnd_ch,
                            );

                            if need_channel_convert || need_resample {
                                let src_data = &overflow_buf[..write_frames as usize * cap_ch];
                                let converted = convert_channels(src_data, cap_ch, rnd_ch, write_frames as usize);
                                let copy_len = converted.len().min(render_data.len());
                                render_data[..copy_len].copy_from_slice(&converted[..copy_len]);
                            } else {
                                let copy_samples = write_frames as usize * cap_ch;
                                render_data[..copy_samples].copy_from_slice(&overflow_buf[..copy_samples]);
                            }

                            let _ = render_service.ReleaseBuffer(write_frames, 0);
                            overflow_buf.drain(..write_frames as usize * cap_ch);
                        }
                        Err(e) => log::error!("渲染写出错: {:?}", e),
                        _ => {}
                    }
                }
            } else {
                // 没有捕获数据 → 排空溢出缓冲区
                let padding = render_client.GetCurrentPadding().unwrap_or(0);
                let available = buf_frames.saturating_sub(padding);
                let overflow_frames2 = (overflow_buf.len() / cap_ch) as u32;
                let drain_frames = overflow_frames2.min(available);
                if drain_frames > 0 {
                    if let Ok(render_ptr) = render_service.GetBuffer(drain_frames) {
                        if !render_ptr.is_null() {
                            let render_data = std::slice::from_raw_parts_mut(
                                render_ptr as *mut f32,
                                drain_frames as usize * rnd_ch,
                            );
                            if need_channel_convert || need_resample {
                                let src_data = &overflow_buf[..drain_frames as usize * cap_ch];
                                let converted = convert_channels(src_data, cap_ch, rnd_ch, drain_frames as usize);
                                let copy_len = converted.len().min(render_data.len());
                                render_data[..copy_len].copy_from_slice(&converted[..copy_len]);
                            } else {
                                let copy_samples = drain_frames as usize * cap_ch;
                                render_data[..copy_samples].copy_from_slice(&overflow_buf[..copy_samples]);
                            }
                            let _ = render_service.ReleaseBuffer(drain_frames, 0);
                            overflow_buf.drain(..drain_frames as usize * cap_ch);
                        }
                    }
                }
                thread::sleep(Duration::from_millis(1));
            }

            // 溢出缓冲区过大时丢弃最旧数据
            let max_overflow = buf_frames as usize * cap_ch * 3;
            if overflow_buf.len() > max_overflow {
                let target = max_overflow * 2 / 3;
                let drop_count = overflow_buf.len() - target;
                // 确保丢弃的是完整帧
                let drop_frames = drop_count / cap_ch;
                let drop_samples = drop_frames * cap_ch;
                overflow_buf.drain(..drop_samples);
                log::warn!("溢出缓冲区过大，丢弃 {} 旧样本", drop_samples);
            }

            local_stats_counter += 1;
            if local_stats_counter % 100 == 0 {
                if let Ok(s) = eq_settings.lock() {
                    let _ = app.emit("eq-stats-update", &s.clone());
                }
            }
        }

        let _ = capture_client.Stop();
        let _ = render_client.Stop();
        CoUninitialize();
    }

    log::info!("音频处理线程已停止");
}

// ─── Tauri 命令 ───

/// 枚举所有活跃的音频输出设备
#[tauri::command]
pub fn get_audio_devices() -> Result<Vec<AudioDeviceInfo>, String> {
    let mut devices = Vec::new();

    unsafe {
        let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
        if hr.is_err() {
            if devices.is_empty() {
                devices.push(AudioDeviceInfo {
                    id: "default".to_string(),
                    name: "默认音频输出设备".to_string(),
                    is_default: true,
                });
                return Ok(devices);
            }
        }

        let enumerator: IMMDeviceEnumerator =
            match CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) {
                Ok(e) => e,
                Err(_) => {
                    CoUninitialize();
                    devices.push(AudioDeviceInfo {
                        id: "default".to_string(),
                        name: "默认音频输出设备".to_string(),
                        is_default: true,
                    });
                    return Ok(devices);
                }
            };

        let collection = match enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) {
            Ok(c) => c,
            Err(_) => {
                CoUninitialize();
                devices.push(AudioDeviceInfo {
                    id: "default".to_string(),
                    name: "默认音频输出设备".to_string(),
                    is_default: true,
                });
                return Ok(devices);
            }
        };

        let count = match collection.GetCount() {
            Ok(c) => c,
            Err(_) => {
                CoUninitialize();
                devices.push(AudioDeviceInfo {
                    id: "default".to_string(),
                    name: "默认音频输出设备".to_string(),
                    is_default: true,
                });
                return Ok(devices);
            }
        };

        let default_id = get_default_device_id();

        for i in 0..count {
            let device = match collection.Item(i) {
                Ok(d) => d,
                Err(_) => continue,
            };
            let device: IMMDevice = match device.cast::<IMMDevice>() {
                Ok(d) => d,
                Err(_) => continue,
            };
            let id_ptr = match device.GetId() {
                Ok(id) => id,
                Err(_) => continue,
            };
            let id = match id_ptr.to_string() {
                Ok(s) => s,
                Err(_) => continue,
            };
            let is_default = Some(&id) == default_id.as_ref();
            let name = if is_default {
                format!("默认设备 ({})", &id[..id.len().min(20)])
            } else {
                format!("音频设备 ({})", &id[..id.len().min(20)])
            };
            devices.push(AudioDeviceInfo { id, name, is_default });
        }

        CoUninitialize();
    }

    if devices.is_empty() {
        devices.push(AudioDeviceInfo {
            id: "default".to_string(),
            name: "默认音频输出设备".to_string(),
            is_default: true,
        });
    }

    log::info!("枚举到 {} 个音频输出设备", devices.len());
    Ok(devices)
}

/// 启动 EQ 音频管线
#[tauri::command]
pub async fn start_eq(
    app: tauri::AppHandle,
    settings: EqSettings,
) -> Result<EqSettings, String> {
    if cfg!(not(target_os = "windows")) {
        return Err("EQ 功能仅支持 Windows 系统".to_string());
    }

    // 检查是否已运行
    {
        let pipeline = PIPELINE.lock().map_err(|e| e.to_string())?;
        if pipeline.is_some() {
            return Err("EQ 管线已在运行中".to_string());
        }
    }

    // ── 反馈循环检测与自动设备选择 ──
    // Loopback 捕获默认渲染设备的输出，如果渲染也输出到同一设备，
    // 会形成反馈循环导致信号指数增长（炸音）
    let default_device_id = get_default_device_id().unwrap_or_default();
    let output_is_default = settings.output_device_id.is_empty()
        || settings.output_device_id == "default"
        || settings.output_device_id == default_device_id;

    let active_device_id = if output_is_default && !default_device_id.is_empty() {
        // 尝试自动找一个不同的渲染设备
        let alternative = find_alternative_render_device(&default_device_id);
        match alternative {
            Some(alt_id) => {
                log::info!(
                    "自动选择非捕获设备作为输出: {} (捕获设备: {})",
                    &alt_id[..alt_id.len().min(30)],
                    &default_device_id[..default_device_id.len().min(30)]
                );
                alt_id
            }
            None => {
                return Err(
                    "反馈回路告警！\n\n\
                     EQ 通过 Loopback 捕获系统音频，无法输出到同一设备。\n\
                     您当前只有一个音频输出设备，需要插入第二个设备\n\
                     （如 USB 耳机/音响、HDMI 显示器）后才能启动 EQ。"
                        .to_string(),
                );
            }
        }
    } else {
        settings.output_device_id.clone()
    };

    // 使用选定的设备 ID 继续
    let resolved_settings = EqSettings {
        output_device_id: active_device_id.clone(),
        ..settings
    };

    let is_running = Arc::new(AtomicBool::new(true));
    let is_running_clone = is_running.clone();
    let app_clone = app.clone();

    let filter_chain = Arc::new(Mutex::new(vec![create_default_filters(48000.0); 2]));
    let filter_chain_clone = filter_chain.clone();
    let eq_settings_arc = Arc::new(Mutex::new(resolved_settings.clone()));
    let eq_settings_clone = eq_settings_arc.clone();
    let sample_rate = Arc::new(AtomicU32::new(48000));
    let sample_rate_clone = sample_rate.clone();

    {
        let mut filters = filter_chain.lock().unwrap();
        for channel_filters in filters.iter_mut() {
            *channel_filters = create_filters_from_settings(&resolved_settings, 48000.0);
        }
    }

    let thread_handle = thread::Builder::new()
        .name("eq-audio-pipeline".into())
        .spawn(move || {
            run_audio_pipeline(
                filter_chain_clone,
                is_running_clone,
                app_clone,
                eq_settings_clone,
                sample_rate_clone,
            );
        })
        .map_err(|e| format!("创建处理线程失败: {}", e))?;

    {
        let mut pipeline = PIPELINE.lock().map_err(|e| e.to_string())?;
        *pipeline = Some(PipelineState {
            settings: resolved_settings.clone(),
            is_running: is_running.clone(),
            thread_handle: Some(thread_handle),
            eq_filters: filter_chain,
            sample_rate,
        });
    }

    log::info!("EQ 音频管线启动成功");

    let mut s = resolved_settings;
    s.enabled = true;
    let _ = app.emit("eq-status-changed", &s);

    Ok(s)
}

/// 停止 EQ 音频管线
#[tauri::command]
pub async fn stop_eq(app: tauri::AppHandle) -> Result<(), String> {
    let mut pipeline = PIPELINE.lock().map_err(|e| e.to_string())?;

    match pipeline.take() {
        Some(state) => {
            log::info!("正在停止 EQ 音频管线...");
            state.is_running.store(false, Ordering::SeqCst);

            if let Some(handle) = state.thread_handle {
                let _ = handle.join();
            }

            let mut settings = state.settings;
            settings.enabled = false;
            let _ = app.emit("eq-status-changed", &settings);

            log::info!("EQ 音频管线已停止");
            Ok(())
        }
        None => Err("EQ 管线未在运行".to_string()),
    }
}

/// 获取当前 EQ 状态
#[tauri::command]
pub fn get_eq_status() -> Result<EqSettings, String> {
    let pipeline = PIPELINE.lock().map_err(|e| e.to_string())?;

    match pipeline.as_ref() {
        Some(state) => {
            let mut s = state.settings.clone();
            s.enabled = state.is_running.load(Ordering::SeqCst);
            Ok(s)
        }
        None => Ok(EqSettings::default()),
    }
}

/// 更新 EQ 设置
#[tauri::command]
pub fn update_eq_settings(settings: EqSettings) -> Result<(), String> {
    let mut pipeline = PIPELINE.lock().map_err(|e| e.to_string())?;

    match pipeline.as_mut() {
        Some(state) => {
            state.settings = settings.clone();

            let sample_rate = state.sample_rate.load(Ordering::SeqCst) as f32;
            let mut filters = state.eq_filters.lock().map_err(|e| e.to_string())?;

            for channel_filters in filters.iter_mut() {
                for i in 0..10 {
                    let gain = settings.bands[i].clamp(-12.0, 12.0);
                    channel_filters[i].update_peaking_eq(
                        EQ_BANDS[i].0, EQ_BANDS[i].1, gain, sample_rate,
                    );
                }
            }

            log::info!(
                "EQ 设置已更新 (采样率: {}Hz, 声道数: {})",
                sample_rate as u32,
                filters.len()
            );
            Ok(())
        }
        None => Err("EQ 管线未启动，无法更新设置".to_string()),
    }
}

// ─── 生命周期 ───

pub fn cleanup() {
    if let Ok(mut pipeline) = PIPELINE.lock() {
        if let Some(state) = pipeline.take() {
            log::info!("正在清理 EQ 音频管线资源...");
            state.is_running.store(false, Ordering::SeqCst);
            if let Some(handle) = state.thread_handle {
                let _ = handle.join();
            }
            log::info!("EQ 音频管线资源已清理");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_biquad_peaking_flat() {
        let mut filter = BiquadFilter::peaking_eq(1000.0, 1.41, 0.0, 48000.0);
        let input: Vec<f32> = (0..100).map(|_| 1.0).collect();
        let output: Vec<f32> = input.iter().map(|&s| filter.process(s)).collect();
        let avg = output[20..].iter().sum::<f32>() / 80.0;
        assert!((avg - 1.0).abs() < 0.01, "0dB 增益应为直通，实际均值: {}", avg);
    }

    #[test]
    fn test_biquad_gain_boost() {
        let mut filter = BiquadFilter::peaking_eq(1000.0, 1.41, 6.0, 48000.0);
        let mut input = vec![0.0_f32; 48000];
        for (i, sample) in input.iter_mut().enumerate() {
            let t = i as f32 / 48000.0;
            *sample = (2.0 * std::f32::consts::PI * 1000.0 * t).sin();
        }
        let mut output = vec![0.0_f32; 48000];
        for (i, &s) in input.iter().enumerate() {
            output[i] = filter.process(s);
        }
        let rms_in = (input[1000..].iter().map(|s| s * s).sum::<f32>() / 47000.0).sqrt();
        let rms_out = (output[1000..].iter().map(|s| s * s).sum::<f32>() / 47000.0).sqrt();
        let gain_db = 20.0 * (rms_out / rms_in).log10();
        assert!((gain_db - 6.0).abs() < 1.5, "期望 6dB 增益，实际: {:.2}dB", gain_db);
    }

    #[test]
    fn test_filter_reset() {
        let mut filter = BiquadFilter::peaking_eq(1000.0, 1.41, 12.0, 48000.0);
        for _ in 0..1000 {
            filter.process(1.0);
        }
        filter.reset();
        assert_eq!(filter.z1, 0.0);
        assert_eq!(filter.z2, 0.0);
    }

    #[test]
    fn test_update_preserves_state() {
        let mut filter = BiquadFilter::peaking_eq(1000.0, 1.41, 6.0, 48000.0);
        for _ in 0..1000 {
            filter.process(0.5);
        }
        let z1_before = filter.z1;
        let z2_before = filter.z2;
        assert!(z1_before.abs() > 0.001);
        filter.update_peaking_eq(2000.0, 1.41, 3.0, 48000.0);
        assert!((filter.z1 - z1_before).abs() < 1e-10);
        assert!((filter.z2 - z2_before).abs() < 1e-10);
    }

    #[test]
    fn test_soft_clip_transparent() {
        for x in [-0.5, -0.1, 0.0, 0.1, 0.5, 0.89].iter() {
            let y = soft_clip(*x);
            assert!((y - x).abs() < 1e-7, "soft_clip({}) 应透过得 {}", x, y);
        }
    }

    #[test]
    fn test_soft_clip_limits() {
        for x in [-10.0, -5.0, -2.0, 2.0, 5.0, 10.0].iter() {
            let y = soft_clip(*x);
            assert!(y.abs() <= 1.0, "soft_clip({}) = {} 超出 [-1,1]", x, y);
        }
    }

    #[test]
    fn test_per_channel_independence() {
        let sr = 48000.0_f32;
        let mut left_chain = create_default_filters(sr);
        let mut right_chain = create_default_filters(sr);
        for _ in 0..1000 {
            let mut s = 0.8_f32;
            for f in left_chain.iter_mut() {
                s = f.process(s);
            }
        }
        for _ in 0..1000 {
            let mut s = 0.0_f32;
            for f in right_chain.iter_mut() {
                s = f.process(s);
            }
        }
        for (i, f) in right_chain.iter().enumerate() {
            assert!(f.z1.abs() < 0.01, "右声道滤波器[{}] z1 应接近 0", i);
        }
    }

    #[test]
    fn test_convert_stereo_to_mono() {
        // 2 frames, 2 channels → 1 channel
        let input = vec![0.1, 0.2, 0.3, 0.4]; // [L0,R0, L1,R1]
        let output = convert_channels(&input, 2, 1, 2);
        assert!((output[0] - 0.15).abs() < 1e-6, "frame0: {}", output[0]);
        assert!((output[1] - 0.35).abs() < 1e-6, "frame1: {}", output[1]);
    }

    #[test]
    fn test_convert_mono_to_stereo() {
        let input = vec![0.5, 0.7]; // 2 frames, 1 channel
        let output = convert_channels(&input, 1, 2, 2);
        assert_eq!(output, vec![0.5, 0.5, 0.7, 0.7]);
    }

    #[test]
    fn test_convert_same_channels() {
        let input = vec![0.1, 0.2, 0.3, 0.4];
        let output = convert_channels(&input, 2, 2, 2);
        assert_eq!(output, input);
    }

    #[test]
    fn test_convert_stereo_to_surround() {
        // 1 frame, stereo → 5.1 (6ch)
        let input = vec![0.1, 0.2];
        let output = convert_channels(&input, 2, 6, 1);
        assert_eq!(output.len(), 6);
        assert_eq!(output[0], 0.1); // L
        assert_eq!(output[1], 0.2); // R
        assert_eq!(output[2], 0.0); // silence
    }
}
