use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureSource {
    Mic,
    SystemAudio,
}

#[derive(Debug, Clone, Copy)]
struct CaptureState {
    mic_enabled: bool,
    system_audio_enabled: bool,
    running: bool,
}

impl Default for CaptureState {
    fn default() -> Self {
        Self {
            mic_enabled: true,
            system_audio_enabled: false,
            running: false,
        }
    }
}

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("capture backend is already running")]
    AlreadyRunning,
    #[error("capture backend is not running")]
    NotRunning,
    #[error("no active audio source is enabled")]
    NoActiveSource,
    #[error("system audio capture is not implemented yet")]
    SystemAudioNotImplemented,
    #[error("default microphone input device is unavailable")]
    MicDeviceUnavailable,
    #[error("default microphone input config is unavailable")]
    MicConfigUnavailable,
    #[error("failed to initialize microphone worker")]
    MicWorkerInitFailed,
    #[error("failed to build microphone stream: {0}")]
    MicStreamBuild(String),
    #[error("failed to start microphone stream: {0}")]
    MicStreamPlay(String),
    #[error("microphone sample format is unsupported")]
    MicUnsupportedSampleFormat,
    #[error("capture backend lock poisoned")]
    LockPoisoned,
}

pub type CaptureResult<T> = Result<T, CaptureError>;

const DEFAULT_SAMPLE_RATE_HZ: u32 = 16_000;
const MIC_RING_BUFFER_SECS: usize = 20;

pub trait CaptureBackend: Send + Sync {
    fn set_source_enabled(&self, source: CaptureSource, enabled: bool) -> CaptureResult<()>;
    fn source_enabled(&self, source: CaptureSource) -> CaptureResult<bool>;
    fn start(&self) -> CaptureResult<()>;
    fn stop(&self) -> CaptureResult<()>;
    fn latest_level(&self) -> CaptureResult<f32>;
    fn mic_sample_rate_hz(&self) -> CaptureResult<u32>;
    fn drain_mic_samples(&self, max_samples: usize) -> CaptureResult<Vec<f32>>;
}

#[derive(Debug, Default)]
pub struct NoopCaptureBackend {
    inner: Mutex<CaptureState>,
}

impl CaptureBackend for NoopCaptureBackend {
    fn set_source_enabled(&self, source: CaptureSource, enabled: bool) -> CaptureResult<()> {
        self.inner
            .lock()
            .map(|mut state| match source {
                CaptureSource::Mic => state.mic_enabled = enabled,
                CaptureSource::SystemAudio => state.system_audio_enabled = enabled,
            })
            .map_err(|_| CaptureError::LockPoisoned)
    }

    fn source_enabled(&self, source: CaptureSource) -> CaptureResult<bool> {
        self.inner
            .lock()
            .map(|state| match source {
                CaptureSource::Mic => state.mic_enabled,
                CaptureSource::SystemAudio => state.system_audio_enabled,
            })
            .map_err(|_| CaptureError::LockPoisoned)
    }

    fn start(&self) -> CaptureResult<()> {
        let mut state = self.inner.lock().map_err(|_| CaptureError::LockPoisoned)?;
        if state.running {
            return Err(CaptureError::AlreadyRunning);
        }
        if !state.mic_enabled && !state.system_audio_enabled {
            return Err(CaptureError::NoActiveSource);
        }

        state.running = true;
        Ok(())
    }

    fn stop(&self) -> CaptureResult<()> {
        let mut state = self.inner.lock().map_err(|_| CaptureError::LockPoisoned)?;
        if !state.running {
            return Err(CaptureError::NotRunning);
        }
        state.running = false;
        Ok(())
    }

    fn latest_level(&self) -> CaptureResult<f32> {
        Ok(0.0)
    }

    fn mic_sample_rate_hz(&self) -> CaptureResult<u32> {
        Ok(DEFAULT_SAMPLE_RATE_HZ)
    }

    fn drain_mic_samples(&self, _max_samples: usize) -> CaptureResult<Vec<f32>> {
        Ok(Vec::new())
    }
}

struct CpalControlState {
    mic_enabled: bool,
    system_audio_enabled: bool,
    running: bool,
    stop_tx: Option<mpsc::Sender<()>>,
    worker_handle: Option<thread::JoinHandle<()>>,
}

impl Default for CpalControlState {
    fn default() -> Self {
        Self {
            mic_enabled: true,
            system_audio_enabled: false,
            running: false,
            stop_tx: None,
            worker_handle: None,
        }
    }
}

pub struct CpalCaptureBackend {
    level_bits: Arc<AtomicU32>,
    sample_rate_hz: Arc<AtomicU32>,
    mic_samples: Arc<Mutex<VecDeque<f32>>>,
    inner: Mutex<CpalControlState>,
}

impl Default for CpalCaptureBackend {
    fn default() -> Self {
        Self {
            level_bits: Arc::new(AtomicU32::new(0.0f32.to_bits())),
            sample_rate_hz: Arc::new(AtomicU32::new(DEFAULT_SAMPLE_RATE_HZ)),
            mic_samples: Arc::new(Mutex::new(VecDeque::new())),
            inner: Mutex::new(CpalControlState::default()),
        }
    }
}

impl CaptureBackend for CpalCaptureBackend {
    fn set_source_enabled(&self, source: CaptureSource, enabled: bool) -> CaptureResult<()> {
        self.inner
            .lock()
            .map(|mut state| match source {
                CaptureSource::Mic => state.mic_enabled = enabled,
                CaptureSource::SystemAudio => state.system_audio_enabled = enabled,
            })
            .map_err(|_| CaptureError::LockPoisoned)
    }

    fn source_enabled(&self, source: CaptureSource) -> CaptureResult<bool> {
        self.inner
            .lock()
            .map(|state| match source {
                CaptureSource::Mic => state.mic_enabled,
                CaptureSource::SystemAudio => state.system_audio_enabled,
            })
            .map_err(|_| CaptureError::LockPoisoned)
    }

    fn start(&self) -> CaptureResult<()> {
        let mut state = self.inner.lock().map_err(|_| CaptureError::LockPoisoned)?;
        if state.running {
            return Err(CaptureError::AlreadyRunning);
        }
        if !state.mic_enabled && !state.system_audio_enabled {
            return Err(CaptureError::NoActiveSource);
        }

        let (started_tx, started_rx) = mpsc::channel::<CaptureResult<()>>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let level_bits = self.level_bits.clone();
        let sample_rate_hz = self.sample_rate_hz.clone();
        let mic_samples = self.mic_samples.clone();

        if let Ok(mut samples) = self.mic_samples.lock() {
            samples.clear();
        }

        let worker_handle = thread::spawn(move || {
            run_mic_capture_worker(level_bits, sample_rate_hz, mic_samples, stop_rx, started_tx)
        });
        let start_result = started_rx
            .recv()
            .map_err(|_| CaptureError::MicWorkerInitFailed)?;
        match start_result {
            Ok(()) => {
                state.stop_tx = Some(stop_tx);
                state.worker_handle = Some(worker_handle);
                state.running = true;
                Ok(())
            }
            Err(error) => {
                let _ = worker_handle.join();
                Err(error)
            }
        }
    }

    fn stop(&self) -> CaptureResult<()> {
        let mut state = self.inner.lock().map_err(|_| CaptureError::LockPoisoned)?;
        if !state.running {
            return Err(CaptureError::NotRunning);
        }

        if let Some(stop_tx) = state.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        if let Some(handle) = state.worker_handle.take() {
            let _ = handle.join();
        }

        state.running = false;
        self.level_bits.store(0.0f32.to_bits(), Ordering::Relaxed);
        self.sample_rate_hz
            .store(DEFAULT_SAMPLE_RATE_HZ, Ordering::Relaxed);
        if let Ok(mut samples) = self.mic_samples.lock() {
            samples.clear();
        }
        Ok(())
    }

    fn latest_level(&self) -> CaptureResult<f32> {
        Ok(f32::from_bits(self.level_bits.load(Ordering::Relaxed)).clamp(0.0, 1.0))
    }

    fn mic_sample_rate_hz(&self) -> CaptureResult<u32> {
        Ok(self.sample_rate_hz.load(Ordering::Relaxed).max(1))
    }

    fn drain_mic_samples(&self, max_samples: usize) -> CaptureResult<Vec<f32>> {
        if max_samples == 0 {
            return Ok(Vec::new());
        }

        let mut samples = self
            .mic_samples
            .lock()
            .map_err(|_| CaptureError::LockPoisoned)?;
        if samples.is_empty() {
            return Ok(Vec::new());
        }

        let drain_count = max_samples.min(samples.len());
        if drain_count == samples.len() {
            return Ok(samples.drain(..).collect());
        }

        Ok(samples.drain(..drain_count).collect())
    }
}

fn run_mic_capture_worker(
    level_bits: Arc<AtomicU32>,
    sample_rate_hz: Arc<AtomicU32>,
    mic_samples: Arc<Mutex<VecDeque<f32>>>,
    stop_rx: mpsc::Receiver<()>,
    started_tx: mpsc::Sender<CaptureResult<()>>,
) {
    let stream = match build_mic_stream(level_bits, sample_rate_hz, mic_samples) {
        Ok(stream) => stream,
        Err(error) => {
            let _ = started_tx.send(Err(error));
            return;
        }
    };

    if let Err(error) = stream.play() {
        let _ = started_tx.send(Err(CaptureError::MicStreamPlay(error.to_string())));
        return;
    }
    let _ = started_tx.send(Ok(()));

    loop {
        match stop_rx.recv_timeout(Duration::from_millis(120)) {
            Ok(_) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    drop(stream);
}

fn build_mic_stream(
    level_bits: Arc<AtomicU32>,
    sample_rate_hz: Arc<AtomicU32>,
    mic_samples: Arc<Mutex<VecDeque<f32>>>,
) -> CaptureResult<cpal::Stream> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or(CaptureError::MicDeviceUnavailable)?;
    let config = device
        .default_input_config()
        .map_err(|_| CaptureError::MicConfigUnavailable)?;
    let stream_config: cpal::StreamConfig = config.clone().into();
    let channels = stream_config.channels;
    let sample_rate = stream_config.sample_rate.0.max(1);
    sample_rate_hz.store(sample_rate, Ordering::Relaxed);
    let error_callback = |error| {
        eprintln!("kiku mic stream error: {error}");
    };

    match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let bits = level_bits.clone();
            let samples = mic_samples.clone();
            device
                .build_input_stream(
                    &stream_config,
                    move |input: &[f32], _| {
                        update_level_and_store_from_f32(
                            input,
                            channels,
                            sample_rate,
                            &bits,
                            &samples,
                        )
                    },
                    error_callback,
                    None,
                )
                .map_err(|error| CaptureError::MicStreamBuild(error.to_string()))
        }
        cpal::SampleFormat::I16 => {
            let bits = level_bits.clone();
            let samples = mic_samples.clone();
            device
                .build_input_stream(
                    &stream_config,
                    move |input: &[i16], _| {
                        update_level_and_store_from_i16(
                            input,
                            channels,
                            sample_rate,
                            &bits,
                            &samples,
                        )
                    },
                    error_callback,
                    None,
                )
                .map_err(|error| CaptureError::MicStreamBuild(error.to_string()))
        }
        cpal::SampleFormat::U16 => {
            let bits = level_bits.clone();
            let samples = mic_samples.clone();
            device
                .build_input_stream(
                    &stream_config,
                    move |input: &[u16], _| {
                        update_level_and_store_from_u16(
                            input,
                            channels,
                            sample_rate,
                            &bits,
                            &samples,
                        )
                    },
                    error_callback,
                    None,
                )
                .map_err(|error| CaptureError::MicStreamBuild(error.to_string()))
        }
        _ => Err(CaptureError::MicUnsupportedSampleFormat),
    }
}

fn update_level_and_store_from_f32(
    samples: &[f32],
    channels: u16,
    sample_rate_hz: u32,
    level_bits: &AtomicU32,
    mic_samples: &Mutex<VecDeque<f32>>,
) {
    let mono = interleaved_to_mono(samples.iter().copied(), channels);
    let rms = rms_normalized(mono.iter().copied());
    smooth_store_level(rms, level_bits);
    push_mic_samples(mono, sample_rate_hz, mic_samples);
}

fn update_level_and_store_from_i16(
    samples: &[i16],
    channels: u16,
    sample_rate_hz: u32,
    level_bits: &AtomicU32,
    mic_samples: &Mutex<VecDeque<f32>>,
) {
    let normalized = interleaved_to_mono(
        samples
            .iter()
            .copied()
            .map(|sample| sample as f32 / i16::MAX as f32),
        channels,
    );
    let rms = rms_normalized(normalized.iter().copied());
    smooth_store_level(rms, level_bits);
    push_mic_samples(normalized, sample_rate_hz, mic_samples);
}

fn update_level_and_store_from_u16(
    samples: &[u16],
    channels: u16,
    sample_rate_hz: u32,
    level_bits: &AtomicU32,
    mic_samples: &Mutex<VecDeque<f32>>,
) {
    let normalized = interleaved_to_mono(
        samples
            .iter()
            .copied()
            .map(|sample| (sample as f32 / u16::MAX as f32) * 2.0 - 1.0),
        channels,
    );
    let rms = rms_normalized(normalized.iter().copied());
    smooth_store_level(rms, level_bits);
    push_mic_samples(normalized, sample_rate_hz, mic_samples);
}

fn interleaved_to_mono(samples: impl Iterator<Item = f32>, channels: u16) -> Vec<f32> {
    let channel_count = channels.max(1) as usize;
    if channel_count == 1 {
        return samples.collect();
    }

    let mut mono = Vec::new();
    let mut frame = Vec::with_capacity(channel_count);
    for sample in samples {
        frame.push(sample);
        if frame.len() == channel_count {
            let sum: f32 = frame.iter().copied().sum();
            mono.push(sum / channel_count as f32);
            frame.clear();
        }
    }

    mono
}

fn push_mic_samples(
    new_samples: Vec<f32>,
    sample_rate_hz: u32,
    mic_samples: &Mutex<VecDeque<f32>>,
) {
    if new_samples.is_empty() {
        return;
    }

    let mut buffer = match mic_samples.lock() {
        Ok(buffer) => buffer,
        Err(_) => return,
    };

    buffer.extend(new_samples);
    let max_samples = sample_rate_hz as usize * MIC_RING_BUFFER_SECS;
    while buffer.len() > max_samples {
        let _ = buffer.pop_front();
    }
}

fn rms_normalized(samples: impl Iterator<Item = f32>) -> f32 {
    let mut sum = 0.0f32;
    let mut count = 0usize;
    for sample in samples {
        sum += sample * sample;
        count += 1;
    }
    if count == 0 {
        return 0.0;
    }

    let rms = (sum / count as f32).sqrt();
    (rms * 1.8).clamp(0.0, 1.0)
}

fn smooth_store_level(raw_level: f32, level_bits: &AtomicU32) {
    let previous = f32::from_bits(level_bits.load(Ordering::Relaxed));
    let smoothed = (previous * 0.8) + (raw_level * 0.2);
    level_bits.store(smoothed.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
}
