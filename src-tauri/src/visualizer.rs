use island_core::error::{AppError, AppResult};
use std::{
    sync::{mpsc, Mutex},
    thread,
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};
use windows::{
    core::GUID,
    Win32::{
        Media::Audio::*,
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
            COINIT_MULTITHREADED,
        },
    },
};

#[derive(Default)]
pub struct Visualizer(Mutex<Option<Worker>>);

struct Worker {
    stop: mpsc::Sender<()>,
    thread: thread::JoinHandle<()>,
}

impl Visualizer {
    pub fn set(&self, app: AppHandle, active: bool) -> AppResult<()> {
        let mut worker = self
            .0
            .lock()
            .map_err(|_| AppError::new("audio_error", "Der Visualizer ist gesperrt."))?;
        if let Some(previous) = worker.take() {
            let _ = previous.stop.send(());
            let _ = previous.thread.join();
        }
        if active {
            let (stop, receiver) = mpsc::channel();
            let thread = thread::Builder::new()
                .name("island-visualizer".into())
                .spawn(move || {
                    while capture(&app, &receiver).is_err() {
                        let _ = app.emit("visualizer-bands", [0.0f32; 5]);
                        if !matches!(
                            receiver.recv_timeout(Duration::from_secs(1)),
                            Err(mpsc::RecvTimeoutError::Timeout)
                        ) {
                            break;
                        }
                    }
                })?;
            *worker = Some(Worker { stop, thread });
        }
        Ok(())
    }
}

#[tauri::command]
pub async fn set_visualizer(app: AppHandle, active: bool) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<crate::state::AppState>();
        let active = active
            && !state
                .shutdown_in_progress
                .load(std::sync::atomic::Ordering::Acquire);
        state.visualizer.set(app.clone(), active)
    })
    .await
    .map_err(|error| AppError::new("audio_error", error.to_string()))?
}

struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

const SAMPLES: usize = 1024;

fn capture(app: &AppHandle, stop: &mpsc::Receiver<()>) -> windows::core::Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
    }
    let _apartment = Apartment;
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
    let device = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)? };
    let client: IAudioClient = unsafe { device.Activate(CLSCTX_ALL, None)? };
    let format = unsafe { client.GetMixFormat()? };
    let spec = unsafe { format.read_unaligned() };
    let tag = if spec.wFormatTag == 0xfffe && spec.cbSize >= 22 {
        let ext = unsafe { format.cast::<WAVEFORMATEXTENSIBLE>().read_unaligned() };
        let subtype = ext.SubFormat;
        if subtype == GUID::from_u128(0x00000003_0000_0010_8000_00aa00389b71) {
            3
        } else if subtype == GUID::from_u128(0x00000001_0000_0010_8000_00aa00389b71) {
            1
        } else {
            0
        }
    } else {
        spec.wFormatTag
    };
    type SampleReader = (usize, fn(&[u8]) -> f64);
    let sample: Option<SampleReader> = match (tag, spec.wBitsPerSample) {
        (3, 32) => Some((4, |bytes: &[u8]| {
            f32::from_le_bytes(bytes[..4].try_into().unwrap()) as f64
        })),
        (1, 16) => Some((2, |bytes: &[u8]| {
            i16::from_le_bytes(bytes[..2].try_into().unwrap()) as f64 / 32768.0
        })),
        (1, 24) => Some((3, |bytes: &[u8]| {
            i32::from_le_bytes([0, bytes[0], bytes[1], bytes[2]]) as f64 / 2_147_483_648.0
        })),
        (1, 32) => Some((4, |bytes: &[u8]| {
            i32::from_le_bytes(bytes[..4].try_into().unwrap()) as f64 / 2_147_483_648.0
        })),
        _ => None,
    };
    let initialized = unsafe {
        client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK,
            1_000_000,
            0,
            format,
            None,
        )
    };
    unsafe {
        CoTaskMemFree(Some(format.cast()));
    }
    initialized?;
    let Some((bytes_per_sample, sample)) = sample else {
        return Ok(());
    };
    let stride = usize::from(spec.nBlockAlign);
    let channels = usize::from(spec.nChannels);
    if channels == 0 || stride < channels * bytes_per_sample || spec.nSamplesPerSec == 0 {
        return Ok(());
    }
    let reader: IAudioCaptureClient = unsafe { client.GetService()? };
    unsafe {
        client.Start()?;
    }
    let mut values = [0.0; SAMPLES];
    let mut cursor = 0;
    let mut last = [0.0; 5];
    while matches!(
        stop.recv_timeout(Duration::from_millis(40)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ) {
        let mut received = false;
        for _ in 0..16 {
            if unsafe { reader.GetNextPacketSize()? } == 0 {
                break;
            }
            let (mut data, mut frames, mut flags) = (std::ptr::null_mut(), 0, 0);
            unsafe {
                reader.GetBuffer(&mut data, &mut frames, &mut flags, None, None)?;
            }
            if !data.is_null() && flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 == 0 {
                let bytes = unsafe { std::slice::from_raw_parts(data, frames as usize * stride) };
                for frame in bytes.chunks_exact(stride) {
                    let mono = frame[..channels * bytes_per_sample]
                        .chunks_exact(bytes_per_sample)
                        .map(sample)
                        .filter(|value| value.is_finite())
                        .sum::<f64>()
                        / channels as f64;
                    values[cursor] = mono;
                    cursor = (cursor + 1) % SAMPLES;
                }
                received = true;
            }
            unsafe {
                reader.ReleaseBuffer(frames)?;
            }
        }
        let next = if received {
            let ordered = std::array::from_fn(|index| values[(cursor + index) % SAMPLES]);
            bands(&ordered, f64::from(spec.nSamplesPerSec))
        } else {
            values.fill(0.0);
            [0.0; 5]
        };
        if next != last {
            let _ = app.emit("visualizer-bands", next);
            last = next;
        }
    }
    unsafe {
        client.Stop()?;
    }
    Ok(())
}

fn bands(samples: &[f64; SAMPLES], rate: f64) -> [f32; 5] {
    const LIMITS: [f64; 6] = [25.0, 220.0, 550.0, 1500.0, 4000.0, 16_000.0];
    let mut bins = [(0.0, 0.0); SAMPLES];
    for (index, sample) in samples.iter().enumerate() {
        let reversed = index.reverse_bits() >> (usize::BITS - SAMPLES.ilog2());
        let window = (1.0 - (std::f64::consts::TAU * index as f64 / SAMPLES as f64).cos()) * 0.5;
        bins[reversed].0 = sample * window;
    }
    for stage in 1..=SAMPLES.ilog2() {
        let width = 1 << stage;
        for offset in 0..width / 2 {
            let (sin, cos) = (-std::f64::consts::TAU * offset as f64 / width as f64).sin_cos();
            for first in (offset..SAMPLES).step_by(width) {
                let second = first + width / 2;
                let (a, b) = bins[first];
                let (c, d) = bins[second];
                let (x, y) = (c * cos - d * sin, c * sin + d * cos);
                bins[first] = (a + x, b + y);
                bins[second] = (a - x, b - y);
            }
        }
    }
    let mut result = [0.0f32; 5];
    for (index, &(re, im)) in bins.iter().take(SAMPLES / 2).enumerate() {
        let hz = index as f64 * rate / SAMPLES as f64;
        if let Some(band) = LIMITS
            .windows(2)
            .position(|range| (range[0]..range[1]).contains(&hz))
        {
            let level = (re.hypot(im) * 4.0 / SAMPLES as f64).sqrt().min(1.0) as f32;
            result[band] = result[band].max(level);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn five_bands_distinguish_test_tones_and_silence() {
        assert_eq!(bands(&[0.0; SAMPLES], 48_000.0), [0.0; 5]);
        for (expected, frequency) in [100.0, 375.0, 900.0, 2500.0, 8000.0]
            .into_iter()
            .enumerate()
        {
            let samples = std::array::from_fn(|index| {
                (std::f64::consts::TAU * frequency * index as f64 / 48_000.0).sin()
            });
            let levels = bands(&samples, 48_000.0);
            assert!(levels[expected] > 0.8);
            assert!(levels
                .iter()
                .enumerate()
                .all(|(index, value)| index == expected || *value < 0.3));
        }
    }
}
