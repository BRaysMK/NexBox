//! Native WASAPI audio engine with real-time EQ processing.
//!
//! Replaces wujieq.exe with in-process:
//!   WASAPI Loopback capture (FxSound virtual device) -> Biquad EQ -> WASAPI render (physical device)

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use log::{info, warn, error};

use windows::Win32::Media::Audio as wa;
use windows::Win32::System::Com::{
    CoInitializeEx, CoCreateInstance, CoUninitialize, CoTaskMemFree,
    COINIT_MULTITHREADED, CLSCTX_ALL,
};
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, PROPERTYKEY};
use windows::core::{Interface, GUID, PCWSTR};

// ── Constants ──────────────────────────────────────────────────────────

const WAVE_FORMAT_PCM: u16 = 1;
const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

/// {00000001-0000-0010-8000-00aa00389b71} = KSDATAFORMAT_SUBTYPE_PCM
const KSDATAFORMAT_SUBTYPE_PCM: GUID = GUID::from_values(
    0x00000001, 0x0000, 0x0010,
    [0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71],
);

/// {00000003-0000-0010-8000-00aa00389b71} = KSDATAFORMAT_SUBTYPE_IEEE_FLOAT
const KSDATAFORMAT_SUBTYPE_IEEE_FLOAT: GUID = GUID::from_values(
    0x00000003, 0x0000, 0x0010,
    [0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71],
);

/// 10-band EQ standard frequencies
const EQ_FREQS: [f64; 10] = [32.0, 64.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];

// ── Public Types ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EqParams {
    pub bands: Vec<BandParam>,
    pub enabled: bool,
    pub version: u64,
}

#[derive(Debug, Clone)]
pub struct BandParam {
    pub freq: f64,
    pub gain: f64,
}

impl Default for EqParams {
    fn default() -> Self {
        Self {
            bands: EQ_FREQS.iter().map(|&f| BandParam { freq: f, gain: 0.0 }).collect(),
            enabled: true,
            version: 0,
        }
    }
}

// ── Biquad Filter (Direct Form II Transposed) ─────────────────────────

#[derive(Debug, Clone, Copy)]
struct BiquadCoeffs {
    b0: f64, b1: f64, b2: f64,
    a1: f64, a2: f64,
}

struct BiquadFilter {
    coeffs: BiquadCoeffs,
    z1: f64,
    z2: f64,
}

impl BiquadFilter {
    fn new() -> Self {
        Self {
            coeffs: BiquadCoeffs { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0 },
            z1: 0.0, z2: 0.0,
        }
    }

    fn set_peaking(&mut self, sample_rate: f64, freq: f64, gain_db: f64, q: f64) {
        let a = 10.0_f64.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f64::consts::PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);
        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a;
        self.coeffs = BiquadCoeffs { b0: b0/a0, b1: b1/a0, b2: b2/a0, a1: a1/a0, a2: a2/a0 };
    }

    fn set_low_shelf(&mut self, sample_rate: f64, freq: f64, gain_db: f64, q: f64) {
        let a = 10.0_f64.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f64::consts::PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);
        let sq_a = a.sqrt();
        let b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * alpha * sq_a);
        let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * alpha * sq_a);
        let a0 = (a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * alpha * sq_a;
        let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * alpha * sq_a;
        self.coeffs = BiquadCoeffs { b0: b0/a0, b1: b1/a0, b2: b2/a0, a1: a1/a0, a2: a2/a0 };
    }

    fn set_high_shelf(&mut self, sample_rate: f64, freq: f64, gain_db: f64, q: f64) {
        let a = 10.0_f64.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f64::consts::PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);
        let sq_a = a.sqrt();
        let b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * alpha * sq_a);
        let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * alpha * sq_a);
        let a0 = (a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * alpha * sq_a;
        let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * alpha * sq_a;
        self.coeffs = BiquadCoeffs { b0: b0/a0, b1: b1/a0, b2: b2/a0, a1: a1/a0, a2: a2/a0 };
    }

    #[inline(always)]
    fn process(&mut self, x: f64) -> f64 {
        let c = &self.coeffs;
        let y = c.b0 * x + self.z1;
        self.z1 = c.b1 * x - c.a1 * y + self.z2;
        self.z2 = c.b2 * x - c.a2 * y;
        y
    }

    fn reset(&mut self) { self.z1 = 0.0; self.z2 = 0.0; }
}

// ── EQ Filter Chain ───────────────────────────────────────────────────

struct EqChain {
    filters: Vec<BiquadFilter>,
    sample_rate: f64,
}

impl EqChain {
    fn new(sample_rate: f64, num_channels: usize) -> Self {
        let filters = (0..(10 * num_channels)).map(|_| BiquadFilter::new()).collect();
        Self { filters, sample_rate }
    }

    fn update(&mut self, bands: &[BandParam], num_channels: usize) {
        let q = 1.41;
        for ch in 0..num_channels {
            for (i, &freq) in EQ_FREQS.iter().enumerate() {
                let idx = ch * 10 + i;
                if idx >= self.filters.len() { break; }
                let gain = bands.get(i).map(|b| b.gain).unwrap_or(0.0);
                if i == 0 { self.filters[idx].set_low_shelf(self.sample_rate, freq, gain, q); }
                else if i == 9 { self.filters[idx].set_high_shelf(self.sample_rate, freq, gain, q); }
                else { self.filters[idx].set_peaking(self.sample_rate, freq, gain, q); }
            }
        }
    }

    fn process_interleaved(&mut self, samples: &mut [f64], num_channels: usize) {
        let num_frames = samples.len() / num_channels;
        for frame in 0..num_frames {
            for ch in 0..num_channels {
                let idx = frame * num_channels + ch;
                let mut s = samples[idx];
                for band in 0..10 {
                    let fidx = ch * 10 + band;
                    if fidx < self.filters.len() { s = self.filters[fidx].process(s); }
                }
                samples[idx] = s;
            }
        }
    }

    fn reset(&mut self) { for f in &mut self.filters { f.reset(); } }
}

// ── Audio Format Handling ─────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum AudioSampleFormat { Pcm16, Pcm24, Pcm32, Float32 }

struct FormatInfo {
    sample_format: AudioSampleFormat,
    sample_rate: u32,
    channels: u16,
    block_align: u16,
}

fn parse_format(wfx: &wa::WAVEFORMATEX) -> FormatInfo {
    let sample_format = if wfx.wFormatTag == WAVE_FORMAT_EXTENSIBLE {
        // WAVEFORMATEXTENSIBLE is packed, read SubFormat via raw pointer
        let ext_ptr = wfx as *const wa::WAVEFORMATEX as *const wa::WAVEFORMATEXTENSIBLE;
        let sub_format = unsafe { std::ptr::addr_of!((*ext_ptr).SubFormat).read_unaligned() };
        if sub_format == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT {
            AudioSampleFormat::Float32
        } else {
            match unsafe { (*ext_ptr).Format.wBitsPerSample } {
                16 => AudioSampleFormat::Pcm16,
                24 => AudioSampleFormat::Pcm24,
                _ => AudioSampleFormat::Pcm32,
            }
        }
    } else if wfx.wFormatTag == WAVE_FORMAT_IEEE_FLOAT {
        AudioSampleFormat::Float32
    } else {
        match wfx.wBitsPerSample {
            16 => AudioSampleFormat::Pcm16,
            24 => AudioSampleFormat::Pcm24,
            _ => AudioSampleFormat::Pcm32,
        }
    };

    FormatInfo {
        sample_format,
        sample_rate: wfx.nSamplesPerSec,
        channels: wfx.nChannels,
        block_align: wfx.nBlockAlign,
    }
}

fn bytes_to_f64(data: &[u8], format: AudioSampleFormat, num_samples: usize) -> Vec<f64> {
    let mut result = Vec::with_capacity(num_samples);
    match format {
        AudioSampleFormat::Float32 => {
            let ptr = data.as_ptr() as *const f32;
            for i in 0..num_samples {
                result.push(unsafe { *ptr.add(i) } as f64);
            }
        }
        AudioSampleFormat::Pcm16 => {
            let ptr = data.as_ptr() as *const i16;
            for i in 0..num_samples {
                result.push(unsafe { *ptr.add(i) } as f64 / 32768.0);
            }
        }
        AudioSampleFormat::Pcm24 => {
            for i in 0..num_samples {
                let off = i * 3;
                if off + 2 >= data.len() { break; }
                let b0 = data[off] as i32;
                let b1 = data[off+1] as i32;
                let b2 = (data[off+2] as i8) as i32;
                result.push(((b2 << 16) | (b1 << 8) | b0) as f64 / 8388608.0);
            }
        }
        AudioSampleFormat::Pcm32 => {
            let ptr = data.as_ptr() as *const i32;
            for i in 0..num_samples {
                result.push(unsafe { *ptr.add(i) } as f64 / 2147483648.0);
            }
        }
    }
    result
}

fn f64_to_bytes(samples: &[f64], format: AudioSampleFormat, output: &mut [u8]) {
    match format {
        AudioSampleFormat::Float32 => {
            let ptr = output.as_mut_ptr() as *mut f32;
            for (i, &s) in samples.iter().enumerate() {
                unsafe { *ptr.add(i) = s as f32; }
            }
        }
        AudioSampleFormat::Pcm16 => {
            let ptr = output.as_mut_ptr() as *mut i16;
            for (i, &s) in samples.iter().enumerate() {
                unsafe { *ptr.add(i) = (s.clamp(-1.0, 1.0) * 32767.0) as i16; }
            }
        }
        AudioSampleFormat::Pcm24 => {
            for (i, &s) in samples.iter().enumerate() {
                let val = (s.clamp(-1.0, 1.0) * 8388607.0) as i32;
                let off = i * 3;
                if off + 2 < output.len() {
                    output[off] = (val & 0xFF) as u8;
                    output[off+1] = ((val >> 8) & 0xFF) as u8;
                    output[off+2] = ((val >> 16) & 0xFF) as u8;
                }
            }
        }
        AudioSampleFormat::Pcm32 => {
            let ptr = output.as_mut_ptr() as *mut i32;
            for (i, &s) in samples.iter().enumerate() {
                unsafe { *ptr.add(i) = (s.clamp(-1.0, 1.0) * 2147483647.0) as i32; }
            }
        }
    }
}

// ── WASAPI Helpers ────────────────────────────────────────────────────

/// PKEY_Device_FriendlyName = {A45C254E-DF1C-4EFD-8020-67D146A850E0}, pid=14
const PKEY_DEVICE_FRIENDLY_NAME: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_values(
        0xA45C254E, 0xDF1C, 0x4EFD,
        [0x80, 0x20, 0x67, 0xD1, 0x46, 0xA8, 0x50, 0xE0],
    ),
    pid: 14,
};

/// Get device ID string
fn get_device_id(device: &wa::IMMDevice) -> Result<String, String> {
    let pwstr = unsafe { device.GetId() }.map_err(|e| format!("GetId failed: {}", e))?;
    let id = unsafe { PCWSTR(pwstr.as_ptr()).to_string() }.map_err(|e| format!("to_string failed: {}", e))?;
    unsafe { CoTaskMemFree(Some(pwstr.as_ptr() as *const _)) };
    Ok(id)
}

/// Get device friendly name via IPropertyStore
fn get_device_name(device: &wa::IMMDevice) -> String {
    unsafe {
        let store: IPropertyStore = match device.OpenPropertyStore(windows::Win32::System::Com::STGM(0)) {
            Ok(s) => s,
            Err(_) => return "<unknown>".to_string(),
        };
        let prop = match store.GetValue(&PKEY_DEVICE_FRIENDLY_NAME) {
            Ok(p) => p,
            Err(_) => return "<no-name>".to_string(),
        };
        prop.to_string()
    }
}

/// List all active render endpoints with names (for diagnostics)
fn list_all_devices(enumerator: &wa::IMMDeviceEnumerator) {
    let collection = match unsafe { enumerator.EnumAudioEndpoints(wa::eRender, wa::DEVICE_STATE_ACTIVE) } {
        Ok(c) => c,
        Err(e) => { warn!("[audio_engine] EnumAudioEndpoints failed: {}", e); return; }
    };
    let count = match unsafe { collection.GetCount() } {
        Ok(c) => c,
        Err(_) => return,
    };
    let default_id = unsafe { enumerator.GetDefaultAudioEndpoint(wa::eRender, wa::eConsole) }
        .map(|d| get_device_id(&d).unwrap_or_default())
        .unwrap_or_default();
    info!("[audio_engine] === All active render devices ({}) ===", count);
    for i in 0..count {
        if let Ok(device) = unsafe { collection.Item(i) } {
            let id = get_device_id(&device).unwrap_or_default();
            let name = get_device_name(&device);
            let is_default = id == default_id;
            info!("[audio_engine]   [{}] name='{}' default={}", i, name, is_default);
        }
    }
}

/// Find devices: FxSound = search by name containing "FxSound", physical = matched by name or default
fn find_devices(
    enumerator: &wa::IMMDeviceEnumerator,
    physical_device_name: &str,
) -> Result<(wa::IMMDevice, wa::IMMDevice), String> {
    list_all_devices(enumerator);

    let collection = unsafe {
        enumerator.EnumAudioEndpoints(wa::eRender, wa::DEVICE_STATE_ACTIVE)
    }.map_err(|e| format!("EnumAudioEndpoints failed: {}", e))?;

    let count = unsafe { collection.GetCount() }
        .map_err(|e| format!("GetCount failed: {}", e))?;

    // Get current default device ID (for fallback strategies)
    let default_id = unsafe { enumerator.GetDefaultAudioEndpoint(wa::eRender, wa::eConsole) }
        .map(|d| get_device_id(&d).unwrap_or_default())
        .unwrap_or_default();

    // === Find FxSound device ===
    // Strategy 1: Search by name containing "FxSound" (most reliable)
    let mut fxsound_device: Option<wa::IMMDevice> = None;
    let mut fxsound_id = String::new();
    for i in 0..count {
        let device = unsafe { collection.Item(i) }.map_err(|e| format!("Item failed: {}", e))?;
        let name = get_device_name(&device);
        if name.to_lowercase().contains("fxsound") {
            fxsound_id = get_device_id(&device).unwrap_or_default();
            info!("[audio_engine] FxSound device found by name: '{}'", name);
            fxsound_device = Some(device);
            break;
        }
    }

    // Strategy 2: Fall back to default device if FxSound not found by name
    // (handles race condition where switch hasn't fully propagated yet,
    //  or FxSound was already set as default before the switch)
    if fxsound_device.is_none() {
        let dev = unsafe { enumerator.GetDefaultAudioEndpoint(wa::eRender, wa::eConsole) }
            .map_err(|e| format!("GetDefaultAudioEndpoint failed: {}", e))?;
        let name = get_device_name(&dev);
        fxsound_id = get_device_id(&dev).unwrap_or_default();
        info!("[audio_engine] FxSound (default fallback): name='{}'", name);
        fxsound_device = Some(dev);
    }

    let fxsound_dev = fxsound_device.ok_or_else(|| "FxSound device not found".to_string())?;

    // === Find physical render device ===
    let mut physical_device: Option<wa::IMMDevice> = None;

    // Strategy 1: Match by physical_device_name
    if !physical_device_name.is_empty() {
        let search = physical_device_name.to_lowercase();
        info!("[audio_engine] Searching physical device by name: '{}'", physical_device_name);
        for i in 0..count {
            let device = unsafe { collection.Item(i) }.map_err(|e| format!("Item failed: {}", e))?;
            let id = get_device_id(&device).unwrap_or_default();
            if id == fxsound_id { continue; }
            let name = get_device_name(&device);
            let nl = name.to_lowercase();
            if nl.contains(&search) || search.contains(&nl) {
                physical_device = Some(device);
                info!("[audio_engine] Matched physical device: '{}'", name);
                break;
            }
        }

        // If name looks garbled (contains '?'), try matching by ASCII substrings
        // e.g. "?????? (K02BS)" -> extract "K02BS" and match
        if physical_device.is_none() && search.contains('?') {
            let ascii_parts: Vec<&str> = search
                .split(|c: char| !c.is_ascii_alphanumeric())
                .filter(|s| s.len() >= 3)
                .collect();
            for part in &ascii_parts {
                info!("[audio_engine] Trying ASCII fallback match: '{}'", part);
                for i in 0..count {
                    let device = unsafe { collection.Item(i) }.map_err(|e| format!("Item failed: {}", e))?;
                    let id = get_device_id(&device).unwrap_or_default();
                    if id == fxsound_id { continue; }
                    let name = get_device_name(&device);
                    if name.to_lowercase().contains(part) {
                        physical_device = Some(device);
                        info!("[audio_engine] Matched physical device (ASCII fallback): '{}'", name);
                        break;
                    }
                }
                if physical_device.is_some() { break; }
            }
        }
    }

    // Strategy 2: Use current default device if it's not FxSound
    // (handles race condition: if switch hasn't taken effect, the default is
    //  still the physical speaker, which is exactly what we want)
    if physical_device.is_none() && !default_id.is_empty() && default_id != fxsound_id {
        for i in 0..count {
            let device = unsafe { collection.Item(i) }.map_err(|e| format!("Item failed: {}", e))?;
            let id = get_device_id(&device).unwrap_or_default();
            if id == default_id {
                let name = get_device_name(&device);
                physical_device = Some(device);
                info!("[audio_engine] Using current default as physical device: '{}'", name);
                break;
            }
        }
    }

    // Strategy 3: First non-FxSound device, preferring non-HDMI/DisplayPort
    if physical_device.is_none() {
        info!("[audio_engine] No name match, selecting fallback non-FxSound device");
        // First pass: skip HDMI/DisplayPort audio devices (NVIDIA, AMD, Intel)
        for i in 0..count {
            let device = unsafe { collection.Item(i) }.map_err(|e| format!("Item failed: {}", e))?;
            let id = get_device_id(&device).unwrap_or_default();
            if id == fxsound_id { continue; }
            let name = get_device_name(&device);
            let nl = name.to_lowercase();
            if !nl.contains("nvidia") && !nl.contains("amd") && !nl.contains("intel display")
                && !nl.contains("hdmi") && !nl.contains("displayport") {
                physical_device = Some(device);
                info!("[audio_engine] Fallback physical device (non-HDMI): '{}'", name);
                break;
            }
        }
        // Second pass: accept any non-FxSound device
        if physical_device.is_none() {
            for i in 0..count {
                let device = unsafe { collection.Item(i) }.map_err(|e| format!("Item failed: {}", e))?;
                let id = get_device_id(&device).unwrap_or_default();
                if id != fxsound_id {
                    let name = get_device_name(&device);
                    physical_device = Some(device);
                    info!("[audio_engine] Fallback physical device: '{}'", name);
                    break;
                }
            }
        }
    }

    let physical = physical_device.ok_or_else(|| "Physical audio device not found".to_string())?;
    let phys_name = get_device_name(&physical);
    info!("[audio_engine] Selected physical device: '{}'", phys_name);
    Ok((fxsound_dev, physical))
}

/// Initialize WASAPI loopback capture
fn init_capture(
    device: &wa::IMMDevice,
) -> Result<(wa::IAudioClient, wa::IAudioCaptureClient, FormatInfo), String> {
    let audio_client: wa::IAudioClient = unsafe {
        device.Activate(CLSCTX_ALL, None)
    }.map_err(|e| format!("Activate capture failed: {}", e))?;

    let mix_format_ptr = unsafe { audio_client.GetMixFormat() }
        .map_err(|e| format!("GetMixFormat failed: {}", e))?;

    let format_info = parse_format(unsafe { &*mix_format_ptr });

    unsafe {
        audio_client.Initialize(
            wa::AUDCLNT_SHAREMODE_SHARED,
            wa::AUDCLNT_STREAMFLAGS_LOOPBACK,
            0, 0,
            mix_format_ptr,
            None,
        )
    }.map_err(|e| format!("Capture Initialize failed: {}", e))?;

    let capture_client: wa::IAudioCaptureClient = unsafe { audio_client.GetService() }
        .map_err(|e| format!("GetService capture failed: {}", e))?;

    unsafe { CoTaskMemFree(Some(mix_format_ptr as *const _)) };

    info!("[audio_engine] Capture: {:?} {}Hz {}ch",
        format_info.sample_format, format_info.sample_rate, format_info.channels);

    Ok((audio_client, capture_client, format_info))
}

/// Initialize WASAPI render
fn init_render(
    device: &wa::IMMDevice,
) -> Result<(wa::IAudioClient, wa::IAudioRenderClient, FormatInfo, u32), String> {
    let audio_client: wa::IAudioClient = unsafe {
        device.Activate(CLSCTX_ALL, None)
    }.map_err(|e| format!("Activate render failed: {}", e))?;

    let mix_format_ptr = unsafe { audio_client.GetMixFormat() }
        .map_err(|e| format!("GetMixFormat render failed: {}", e))?;

    let format_info = parse_format(unsafe { &*mix_format_ptr });

    unsafe {
        audio_client.Initialize(
            wa::AUDCLNT_SHAREMODE_SHARED,
            0, 0, 0,
            mix_format_ptr,
            None,
        )
    }.map_err(|e| format!("Render Initialize failed: {}", e))?;

    let render_client: wa::IAudioRenderClient = unsafe { audio_client.GetService() }
        .map_err(|e| format!("GetService render failed: {}", e))?;

    // Check and set audio session volume to prevent silent playback
    match unsafe { audio_client.GetService::<wa::ISimpleAudioVolume>() } {
        Ok(vol) => {
            if let Ok(current) = unsafe { vol.GetMasterVolume() } {
                info!("[audio_engine] Render session volume: {:.3}", current);
                if current < 0.01 {
                    let _ = unsafe { vol.SetMasterVolume(1.0, std::ptr::null()) };
                    info!("[audio_engine] Set render session volume to 1.0");
                }
            }
            let _ = unsafe { vol.SetMute(false, std::ptr::null()) };
        }
        Err(e) => warn!("[audio_engine] Could not get ISimpleAudioVolume: {}", e),
    }

    let buffer_size = unsafe { audio_client.GetBufferSize() }
        .map_err(|e| format!("GetBufferSize failed: {}", e))?;

    unsafe { CoTaskMemFree(Some(mix_format_ptr as *const _)) };

    info!("[audio_engine] Render: {:?} {}Hz {}ch, buf={}",
        format_info.sample_format, format_info.sample_rate, format_info.channels, buffer_size);

    Ok((audio_client, render_client, format_info, buffer_size))
}

// ── Audio Processing Thread ───────────────────────────────────────────

fn audio_thread(
    physical_device_name: String,
    params: Arc<RwLock<EqParams>>,
    stop_flag: Arc<AtomicBool>,
) {
    info!("[audio_engine] Thread starting");

    let co_init = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    if co_init.is_err() {
        error!("[audio_engine] CoInitializeEx failed: {:?}", co_init);
        return;
    }

    let enumerator: wa::IMMDeviceEnumerator = match unsafe {
        CoCreateInstance(&wa::MMDeviceEnumerator, None, CLSCTX_ALL)
    } {
        Ok(e) => e,
        Err(e) => {
            error!("[audio_engine] CoCreateInstance failed: {}", e);
            unsafe { CoUninitialize(); }
            return;
        }
    };

    let (fxsound_dev, physical_dev) = match find_devices(&enumerator, &physical_device_name) {
        Ok(d) => d,
        Err(e) => {
            error!("[audio_engine] {}", e);
            unsafe { CoUninitialize(); }
            return;
        }
    };

    let (capture_client_obj, capture_client, capture_fmt) = match init_capture(&fxsound_dev) {
        Ok(c) => c,
        Err(e) => {
            error!("[audio_engine] {}", e);
            unsafe { CoUninitialize(); }
            return;
        }
    };

    let (render_client_obj, render_client, render_fmt, render_buf_size) = match init_render(&physical_dev) {
        Ok(r) => r,
        Err(e) => {
            error!("[audio_engine] {}", e);
            unsafe { CoUninitialize(); }
            return;
        }
    };

    if capture_fmt.sample_rate != render_fmt.sample_rate {
        warn!("[audio_engine] Sample rate mismatch: {} vs {}Hz",
            capture_fmt.sample_rate, render_fmt.sample_rate);
    }

    let channels = capture_fmt.channels.min(render_fmt.channels) as usize;
    if capture_fmt.channels != render_fmt.channels {
        warn!("[audio_engine] Channel mismatch: {} vs {}ch, using {}",
            capture_fmt.channels, render_fmt.channels, channels);
    }

    let mut eq_chain = EqChain::new(capture_fmt.sample_rate as f64, channels);
    let mut last_version = u64::MAX;

    // Pre-fill render buffer with silence before starting to prevent initial underrun
    let render_bpf = render_fmt.block_align as usize;
    let pre_fill_frames = render_buf_size;
    let pre_fill_bytes = pre_fill_frames as usize * render_bpf;
    if let Ok(render_ptr) = (unsafe { render_client.GetBuffer(pre_fill_frames) }) {
        unsafe { std::ptr::write_bytes(render_ptr, 0, pre_fill_bytes); }
        let _ = unsafe { render_client.ReleaseBuffer(pre_fill_frames, 0) };
        info!("[audio_engine] Pre-filled render buffer with {} frames of silence", pre_fill_frames);
    }

    // Start capture first, then render
    if let Err(e) = unsafe { capture_client_obj.Start() } {
        error!("[audio_engine] Capture Start failed: {}", e);
        unsafe { CoUninitialize(); }
        return;
    }
    if let Err(e) = unsafe { render_client_obj.Start() } {
        error!("[audio_engine] Render Start failed: {}", e);
        unsafe { CoUninitialize(); }
        return;
    }

    info!("[audio_engine] Pipeline started: Loopback(FxSound) -> EQ -> Render");

    let mut sample_buffer: Vec<f64> = Vec::with_capacity(8192);

    // Stats for diagnostics
    let mut total_captured: u64 = 0;
    let mut total_rendered: u64 = 0;
    let mut silence_packets: u64 = 0;
    let mut audio_packets: u64 = 0;
    let mut last_stats_time = std::time::Instant::now();
    let mut first_audio_logged = false;
    let mut max_capture_amp: f64 = 0.0;
    let mut max_render_amp: f64 = 0.0;

    while !stop_flag.load(Ordering::SeqCst) {
        // Update EQ if params changed
        {
            let p = params.read().unwrap();
            if p.version != last_version {
                eq_chain.update(&p.bands, channels);
                last_version = p.version;
                info!("[audio_engine] EQ updated (v{})", last_version);
            }
        }

        // Read capture data
        let packet_size = match unsafe { capture_client.GetNextPacketSize() } {
            Ok(s) => s,
            Err(e) => {
                warn!("[audio_engine] GetNextPacketSize: {}", e);
                thread::sleep(Duration::from_millis(2));
                continue;
            }
        };

        let mut current_packet = packet_size;
        while current_packet > 0 {
            let mut data_ptr: *mut u8 = std::ptr::null_mut();
            let mut num_frames: u32 = 0;
            let mut flags: u32 = 0;

            match unsafe {
                capture_client.GetBuffer(
                    &mut data_ptr,
                    &mut num_frames,
                    &mut flags,
                    None,
                    None,
                )
            } {
                Ok(_) => {}
                Err(e) => { warn!("[audio_engine] GetBuffer: {}", e); break; }
            }

            let is_silent = flags & (wa::AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0;
            let num_samples = num_frames as usize * channels;

            if is_silent {
                silence_packets += 1;
                // Push silence
                for _ in 0..num_samples { sample_buffer.push(0.0); }
            } else {
                audio_packets += 1;
                if !first_audio_logged {
                    first_audio_logged = true;
                    info!("[audio_engine] *** First non-silent audio captured: {} frames ***", num_frames);
                }
                let cap_ch = capture_fmt.channels as usize;
                let byte_count = num_frames as usize * capture_fmt.block_align as usize;
                let raw = unsafe { std::slice::from_raw_parts(data_ptr, byte_count) };
                let samples = bytes_to_f64(raw, capture_fmt.sample_format, num_frames as usize * cap_ch);

                if cap_ch == channels {
                    sample_buffer.extend_from_slice(&samples);
                } else {
                    for frame in 0..num_frames as usize {
                        for ch in 0..channels {
                            let src = frame * cap_ch + ch;
                            if src < samples.len() { sample_buffer.push(samples[src]); }
                        }
                    }
                }
            }

            total_captured += num_frames as u64;
            let _ = unsafe { capture_client.ReleaseBuffer(num_frames) };
            match unsafe { capture_client.GetNextPacketSize() } {
                Ok(s) => current_packet = s,
                Err(_) => break,
            }
        }

        // Prevent unbounded buffer growth (max 2x render buffer)
        let max_samples = (render_buf_size as usize * 2) * channels;
        if sample_buffer.len() > max_samples {
            let drain_count = sample_buffer.len() - max_samples;
            sample_buffer.drain(..drain_count);
        }

        // Write to render buffer - write PARTIAL data (key fix!)
        let padding = match unsafe { render_client_obj.GetCurrentPadding() } {
            Ok(p) => p,
            Err(_) => { thread::sleep(Duration::from_millis(1)); continue; }
        };

        let available = render_buf_size.saturating_sub(padding);
        if available == 0 {
            thread::sleep(Duration::from_millis(1));
            continue;
        }

        // KEY FIX: Only write as many frames as we actually have data for
        let buffered_frames = sample_buffer.len() / channels;
        let frames_to_write = buffered_frames.min(available as usize);

        if frames_to_write == 0 {
            thread::sleep(Duration::from_millis(1));
            continue;
        }

        let needed_samples = frames_to_write * channels;
        let mut process_buf: Vec<f64> = sample_buffer.drain(..needed_samples).collect();

        // Log max amplitude of captured audio (before EQ)
        let max_pre = process_buf.iter().fold(0.0f64, |m, &s| m.max(s.abs()));

        // Apply EQ
        {
            let p = params.read().unwrap();
            if p.enabled {
                eq_chain.process_interleaved(&mut process_buf, channels);
            }
        }

        // Log max amplitude after EQ
        let max_post = process_buf.iter().fold(0.0f64, |m, &s| m.max(s.abs()));
        max_capture_amp = max_capture_amp.max(max_pre);
        max_render_amp = max_render_amp.max(max_post);

        // Convert to render format
        let render_ch = render_fmt.channels as usize;
        let render_byte_count = frames_to_write * render_bpf;
        let mut render_data = vec![0u8; render_byte_count];

        if render_ch == channels {
            f64_to_bytes(&process_buf, render_fmt.sample_format, &mut render_data);
        } else {
            let mut expanded = Vec::with_capacity(frames_to_write * render_ch);
            for frame in 0..frames_to_write {
                for ch in 0..render_ch {
                    let src_ch = ch.min(channels - 1);
                    expanded.push(process_buf[frame * channels + src_ch]);
                }
            }
            f64_to_bytes(&expanded, render_fmt.sample_format, &mut render_data);
        }

        // Write to render buffer (write only frames_to_write, not available)
        match unsafe { render_client.GetBuffer(frames_to_write as u32) } {
            Ok(render_ptr) => {
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        render_data.as_ptr(), render_ptr, render_byte_count,
                    );
                }
                let _ = unsafe { render_client.ReleaseBuffer(frames_to_write as u32, 0) };
                total_rendered += frames_to_write as u64;
            }
            Err(e) => warn!("[audio_engine] Render GetBuffer: {}", e),
        }

        // Periodic stats logging (every 5 seconds)
        if last_stats_time.elapsed() >= Duration::from_secs(5) {
            info!(
                "[audio_engine] Stats: captured={} frames, rendered={} frames, silent_pkts={}, audio_pkts={}, buf={} samples, max_capture_amp={:.6}, max_render_amp={:.6}",
                total_captured, total_rendered, silence_packets, audio_packets, sample_buffer.len(), max_capture_amp, max_render_amp
            );
            total_captured = 0;
            total_rendered = 0;
            silence_packets = 0;
            audio_packets = 0;
            max_capture_amp = 0.0;
            max_render_amp = 0.0;
            last_stats_time = std::time::Instant::now();
        }
    }

    // Cleanup
    let _ = unsafe { capture_client_obj.Stop() };
    let _ = unsafe { render_client_obj.Stop() };
    eq_chain.reset();
    info!("[audio_engine] Thread stopped");
    unsafe { CoUninitialize(); }
}

// ── AudioEngine (public API) ──────────────────────────────────────────

pub struct AudioEngine {
    stop_flag: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    params: Arc<RwLock<EqParams>>,
}

impl AudioEngine {
    pub fn start(physical_device_name: String) -> Result<Self, String> {
        let params = Arc::new(RwLock::new(EqParams::default()));
        let stop_flag = Arc::new(AtomicBool::new(false));
        let p_clone = params.clone();
        let sf_clone = stop_flag.clone();
        let dev_name = physical_device_name;

        let thread = thread::Builder::new()
            .name("eq-audio-engine".to_string())
            .spawn(move || { audio_thread(dev_name, p_clone, sf_clone); })
            .map_err(|e| format!("Failed to spawn audio thread: {}", e))?;

        thread::sleep(Duration::from_millis(100));
        Ok(Self { stop_flag, thread: Some(thread), params })
    }

    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        info!("[audio_engine] Engine stopped");
    }

    pub fn is_running(&self) -> bool {
        if self.stop_flag.load(Ordering::SeqCst) { return false; }
        match &self.thread {
            Some(t) => !t.is_finished(),
            None => false,
        }
    }

    pub fn update_bands(&self, bands: Vec<BandParam>) {
        if let Ok(mut p) = self.params.write() {
            p.bands = bands;
            p.version = p.version.wrapping_add(1);
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        if let Ok(mut p) = self.params.write() {
            p.enabled = enabled;
            p.version = p.version.wrapping_add(1);
        }
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) { self.stop(); }
}
