use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
#[cfg(target_os = "macos")]
use std::fs;
#[cfg(target_os = "macos")]
use std::io::{BufRead, BufReader, ErrorKind, Read};
#[cfg(target_os = "macos")]
use std::os::unix::fs::PermissionsExt;
#[cfg(target_os = "macos")]
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::process::{Child, Command, Stdio};
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SystemAudioPermissionStatus {
    Granted,
    Denied,
    Unsupported,
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
    #[error("no loopback-capable system audio input device is available")]
    SystemAudioDeviceUnavailable,
    #[error("system audio capture requires Screen Recording permission in macOS System Settings")]
    SystemAudioPermissionDenied,
    #[error("failed to initialize capture worker")]
    WorkerInitFailed,
    #[error("default microphone input device is unavailable")]
    MicDeviceUnavailable,
    #[error("default microphone input config is unavailable")]
    MicConfigUnavailable,
    #[error("failed to build microphone stream: {0}")]
    MicStreamBuild(String),
    #[error("failed to start microphone stream: {0}")]
    MicStreamPlay(String),
    #[error("microphone sample format is unsupported")]
    MicUnsupportedSampleFormat,
    #[error("failed to build system audio stream: {0}")]
    SystemAudioStreamBuild(String),
    #[error("failed to start system audio stream: {0}")]
    SystemAudioStreamPlay(String),
    #[error("system audio sample format is unsupported")]
    SystemAudioUnsupportedSampleFormat,
    #[error("failed to prepare macOS system audio helper: {0}")]
    SystemAudioHelperUnavailable(String),
    #[error("failed to launch macOS system audio helper: {0}")]
    SystemAudioHelperLaunch(String),
    #[error("failed to initialize macOS system audio helper: {0}")]
    SystemAudioHelperInit(String),
    #[error("capture backend lock poisoned")]
    LockPoisoned,
}

pub type CaptureResult<T> = Result<T, CaptureError>;

const CAPTURE_SAMPLE_RATE_HZ: u32 = 16_000;
const CAPTURE_RING_BUFFER_SECS: usize = 20;
#[cfg(not(target_os = "macos"))]
const LOOPBACK_STRONG_HINTS: [&str; 8] = [
    "blackhole",
    "loopback",
    "soundflower",
    "vb-cable",
    "virtual",
    "aggregate",
    "system audio",
    "monitor",
];
#[cfg(not(target_os = "macos"))]
const MIC_HINTS: [&str; 4] = ["microphone", "built-in mic", "external mic", "headset mic"];
#[cfg(target_os = "macos")]
const SYSTEM_AUDIO_HELPER_HEADER_LEN: usize = 8;
#[cfg(target_os = "macos")]
const SYSTEM_AUDIO_HELPER_READ_BUF_BYTES: usize = 8192;
#[cfg(target_os = "macos")]
const SYSTEM_AUDIO_HELPER_MAGIC: &[u8; 4] = b"KIKU";
#[cfg(target_os = "macos")]
const EMBEDDED_SYSTEM_AUDIO_HELPER: &[u8] = include_bytes!(env!("KIKU_SYSTEM_AUDIO_HELPER_BINARY"));

#[cfg(target_os = "macos")]
pub fn system_audio_permission_status() -> CaptureResult<SystemAudioPermissionStatus> {
    run_macos_system_audio_helper_control("--permission-status")
}

#[cfg(not(target_os = "macos"))]
pub fn system_audio_permission_status() -> CaptureResult<SystemAudioPermissionStatus> {
    Ok(SystemAudioPermissionStatus::Unsupported)
}

#[cfg(target_os = "macos")]
pub fn request_system_audio_permission() -> CaptureResult<SystemAudioPermissionStatus> {
    run_macos_system_audio_helper_control("--request-permission")
}

#[cfg(not(target_os = "macos"))]
pub fn request_system_audio_permission() -> CaptureResult<SystemAudioPermissionStatus> {
    Ok(SystemAudioPermissionStatus::Unsupported)
}

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
        Ok(CAPTURE_SAMPLE_RATE_HZ)
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
    capture_samples: Arc<Mutex<VecDeque<f32>>>,
    inner: Mutex<CpalControlState>,
}

impl Default for CpalCaptureBackend {
    fn default() -> Self {
        Self {
            level_bits: Arc::new(AtomicU32::new(0.0f32.to_bits())),
            sample_rate_hz: Arc::new(AtomicU32::new(CAPTURE_SAMPLE_RATE_HZ)),
            capture_samples: Arc::new(Mutex::new(VecDeque::new())),
            inner: Mutex::new(CpalControlState::default()),
        }
    }
}

impl CpalCaptureBackend {
    fn start_worker_locked(&self, state: &mut CpalControlState) -> CaptureResult<()> {
        let mic_enabled = state.mic_enabled;
        let system_audio_enabled = state.system_audio_enabled;
        if !mic_enabled && !system_audio_enabled {
            return Err(CaptureError::NoActiveSource);
        }

        let (started_tx, started_rx) = mpsc::channel::<CaptureResult<()>>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let level_bits = self.level_bits.clone();
        let sample_rate_hz = self.sample_rate_hz.clone();
        let capture_samples = self.capture_samples.clone();
        self.level_bits.store(0.0f32.to_bits(), Ordering::Relaxed);
        self.sample_rate_hz
            .store(CAPTURE_SAMPLE_RATE_HZ, Ordering::Relaxed);
        if let Ok(mut samples) = self.capture_samples.lock() {
            samples.clear();
        }

        let worker_handle = thread::spawn(move || {
            run_capture_worker(
                level_bits,
                sample_rate_hz,
                capture_samples,
                stop_rx,
                started_tx,
                mic_enabled,
                system_audio_enabled,
            )
        });
        let start_result = started_rx
            .recv()
            .map_err(|_| CaptureError::WorkerInitFailed)?;
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

    fn stop_worker_locked(state: &mut CpalControlState) {
        if let Some(stop_tx) = state.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        if let Some(handle) = state.worker_handle.take() {
            let _ = handle.join();
        }
        state.running = false;
    }

    fn restart_worker_locked(&self, state: &mut CpalControlState) -> CaptureResult<()> {
        Self::stop_worker_locked(state);
        self.start_worker_locked(state)
    }
}

impl CaptureBackend for CpalCaptureBackend {
    fn set_source_enabled(&self, source: CaptureSource, enabled: bool) -> CaptureResult<()> {
        let mut state = self.inner.lock().map_err(|_| CaptureError::LockPoisoned)?;
        let previous_mic = state.mic_enabled;
        let previous_system = state.system_audio_enabled;

        let changed = match source {
            CaptureSource::Mic => {
                if state.mic_enabled == enabled {
                    false
                } else {
                    state.mic_enabled = enabled;
                    true
                }
            }
            CaptureSource::SystemAudio => {
                if state.system_audio_enabled == enabled {
                    false
                } else {
                    state.system_audio_enabled = enabled;
                    true
                }
            }
        };
        if !changed {
            return Ok(());
        }

        if !state.running {
            return Ok(());
        }
        if !state.mic_enabled && !state.system_audio_enabled {
            state.mic_enabled = previous_mic;
            state.system_audio_enabled = previous_system;
            return Err(CaptureError::NoActiveSource);
        }

        if let Err(error) = self.restart_worker_locked(&mut state) {
            // Restore the previous stream configuration so the live session stays stable.
            state.mic_enabled = previous_mic;
            state.system_audio_enabled = previous_system;
            let _ = self.restart_worker_locked(&mut state);
            return Err(error);
        }

        Ok(())
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
        self.start_worker_locked(&mut state)
    }

    fn stop(&self) -> CaptureResult<()> {
        let mut state = self.inner.lock().map_err(|_| CaptureError::LockPoisoned)?;
        if !state.running {
            return Err(CaptureError::NotRunning);
        }
        Self::stop_worker_locked(&mut state);

        self.level_bits.store(0.0f32.to_bits(), Ordering::Relaxed);
        self.sample_rate_hz
            .store(CAPTURE_SAMPLE_RATE_HZ, Ordering::Relaxed);
        if let Ok(mut samples) = self.capture_samples.lock() {
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
            .capture_samples
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

fn run_capture_worker(
    level_bits: Arc<AtomicU32>,
    sample_rate_hz: Arc<AtomicU32>,
    capture_samples: Arc<Mutex<VecDeque<f32>>>,
    stop_rx: mpsc::Receiver<()>,
    started_tx: mpsc::Sender<CaptureResult<()>>,
    mic_enabled: bool,
    system_audio_enabled: bool,
) {
    sample_rate_hz.store(CAPTURE_SAMPLE_RATE_HZ, Ordering::Relaxed);

    let mut streams = Vec::new();
    #[cfg(target_os = "macos")]
    let mut system_audio_helper_session: Option<MacOsSystemAudioHelperSession> = None;

    if mic_enabled {
        let stream = match build_mic_stream(level_bits.clone(), capture_samples.clone()) {
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
        streams.push(stream);
    }

    if system_audio_enabled {
        #[cfg(target_os = "macos")]
        {
            match start_macos_system_audio_helper(level_bits.clone(), capture_samples.clone()) {
                Ok(session) => {
                    system_audio_helper_session = Some(session);
                }
                Err(error) => {
                    let _ = started_tx.send(Err(error));
                    return;
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            let stream = match build_system_stream(level_bits.clone(), capture_samples.clone()) {
                Ok(stream) => stream,
                Err(error) => {
                    let _ = started_tx.send(Err(error));
                    return;
                }
            };
            if let Err(error) = stream.play() {
                let _ =
                    started_tx.send(Err(CaptureError::SystemAudioStreamPlay(error.to_string())));
                return;
            }
            streams.push(stream);
        }
    }

    #[cfg(target_os = "macos")]
    let no_capture_session = streams.is_empty() && system_audio_helper_session.is_none();
    #[cfg(not(target_os = "macos"))]
    let no_capture_session = streams.is_empty();

    if no_capture_session {
        let _ = started_tx.send(Err(CaptureError::NoActiveSource));
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

    drop(streams);
    #[cfg(target_os = "macos")]
    drop(system_audio_helper_session);
}

#[cfg(target_os = "macos")]
struct MacOsSystemAudioHelperSession {
    child: Child,
    reader_handle: Option<thread::JoinHandle<()>>,
    stderr_handle: Option<thread::JoinHandle<()>>,
}

#[cfg(target_os = "macos")]
impl Drop for MacOsSystemAudioHelperSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();

        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.stderr_handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(target_os = "macos")]
fn start_macos_system_audio_helper(
    level_bits: Arc<AtomicU32>,
    capture_samples: Arc<Mutex<VecDeque<f32>>>,
) -> CaptureResult<MacOsSystemAudioHelperSession> {
    let helper_path = prepare_macos_system_audio_helper()?;
    let mut child = Command::new(&helper_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| CaptureError::SystemAudioHelperLaunch(error.to_string()))?;

    let stdout = child.stdout.take().ok_or_else(|| {
        CaptureError::SystemAudioHelperInit("helper stdout is unavailable".to_owned())
    })?;
    let stderr = child.stderr.take();

    let stderr_log = Arc::new(Mutex::new(String::new()));
    let stderr_log_for_thread = stderr_log.clone();
    let stderr_handle = stderr.map(move |stderr_pipe| {
        thread::spawn(move || {
            let mut reader = BufReader::new(stderr_pipe);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            eprintln!("kiku system helper: {trimmed}");
                        }
                        if let Ok(mut log) = stderr_log_for_thread.lock() {
                            log.push_str(trimmed);
                            log.push('\n');
                        }
                    }
                    Err(_) => break,
                }
            }
        })
    });

    let (ready_tx, ready_rx) = mpsc::channel::<CaptureResult<u32>>();
    let reader_handle = thread::spawn(move || {
        read_macos_system_audio_helper_stream(stdout, ready_tx, level_bits, capture_samples)
    });

    match ready_rx.recv_timeout(Duration::from_secs(8)) {
        Ok(Ok(_input_sample_rate_hz)) => Ok(MacOsSystemAudioHelperSession {
            child,
            reader_handle: Some(reader_handle),
            stderr_handle,
        }),
        Ok(Err(error)) => {
            let session = MacOsSystemAudioHelperSession {
                child,
                reader_handle: Some(reader_handle),
                stderr_handle,
            };
            let stderr_summary = stderr_log
                .lock()
                .map(|log| log.trim().to_owned())
                .unwrap_or_default();
            let classified = classify_macos_helper_error(error, &stderr_summary);
            drop(session);
            Err(classified)
        }
        Err(_) => {
            let session = MacOsSystemAudioHelperSession {
                child,
                reader_handle: Some(reader_handle),
                stderr_handle,
            };
            let stderr_summary = stderr_log
                .lock()
                .map(|log| log.trim().to_owned())
                .unwrap_or_default();
            let timeout_message = if stderr_summary.is_empty() {
                "timed out waiting for helper readiness".to_owned()
            } else {
                format!("timed out waiting for helper readiness ({stderr_summary})")
            };
            let classified = classify_macos_helper_error(
                CaptureError::SystemAudioHelperInit(timeout_message),
                &stderr_summary,
            );
            drop(session);
            Err(classified)
        }
    }
}

#[cfg(target_os = "macos")]
fn run_macos_system_audio_helper_control(arg: &str) -> CaptureResult<SystemAudioPermissionStatus> {
    let helper_path = prepare_macos_system_audio_helper()?;
    let output = Command::new(&helper_path)
        .arg(arg)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| CaptureError::SystemAudioHelperLaunch(error.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_ascii_lowercase();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if !output.status.success() {
        let error = CaptureError::SystemAudioHelperInit(if stderr.is_empty() {
            "helper control command failed".to_owned()
        } else {
            stderr.clone()
        });
        return Err(classify_macos_helper_error(error, &stderr));
    }

    parse_permission_status(&stdout).ok_or_else(|| {
        CaptureError::SystemAudioHelperInit(format!(
            "helper control returned unexpected output for {arg}: '{stdout}'"
        ))
    })
}

#[cfg(target_os = "macos")]
fn parse_permission_status(raw: &str) -> Option<SystemAudioPermissionStatus> {
    match raw.trim() {
        "granted" => Some(SystemAudioPermissionStatus::Granted),
        "denied" => Some(SystemAudioPermissionStatus::Denied),
        "unsupported" => Some(SystemAudioPermissionStatus::Unsupported),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn read_macos_system_audio_helper_stream(
    mut stdout: impl Read,
    ready_tx: mpsc::Sender<CaptureResult<u32>>,
    level_bits: Arc<AtomicU32>,
    capture_samples: Arc<Mutex<VecDeque<f32>>>,
) {
    let mut header = [0u8; SYSTEM_AUDIO_HELPER_HEADER_LEN];
    if let Err(error) = stdout.read_exact(&mut header) {
        let _ = ready_tx.send(Err(CaptureError::SystemAudioHelperInit(format!(
            "failed to read helper header: {error}"
        ))));
        return;
    }

    if &header[0..4] != SYSTEM_AUDIO_HELPER_MAGIC {
        let _ = ready_tx.send(Err(CaptureError::SystemAudioHelperInit(
            "invalid helper header magic".to_owned(),
        )));
        return;
    }
    let input_sample_rate_hz =
        u32::from_le_bytes([header[4], header[5], header[6], header[7]]).max(1);
    let _ = ready_tx.send(Ok(input_sample_rate_hz));

    let mut read_buffer = [0u8; SYSTEM_AUDIO_HELPER_READ_BUF_BYTES];
    let mut pending_bytes = Vec::<u8>::new();

    loop {
        match stdout.read(&mut read_buffer) {
            Ok(0) => break,
            Ok(read_len) => {
                pending_bytes.extend_from_slice(&read_buffer[..read_len]);
                let complete_bytes = pending_bytes.len() - (pending_bytes.len() % 4);
                if complete_bytes == 0 {
                    continue;
                }

                let mut samples = Vec::with_capacity(complete_bytes / 4);
                for chunk in pending_bytes[..complete_bytes].chunks_exact(4) {
                    samples.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
                }
                pending_bytes.drain(..complete_bytes);

                if !samples.is_empty() {
                    ingest_capture_samples(
                        samples,
                        input_sample_rate_hz,
                        &level_bits,
                        &capture_samples,
                    );
                }
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => {
                eprintln!("kiku system helper read error: {error}");
                break;
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn classify_macos_helper_error(base_error: CaptureError, stderr_log: &str) -> CaptureError {
    let normalized = stderr_log.to_ascii_lowercase();
    if normalized.contains("screen recording permission denied")
        || normalized.contains("permission denied")
    {
        return CaptureError::SystemAudioPermissionDenied;
    }

    match base_error {
        CaptureError::SystemAudioHelperInit(message) => {
            if stderr_log.trim().is_empty() {
                CaptureError::SystemAudioHelperInit(message)
            } else {
                CaptureError::SystemAudioHelperInit(format!("{message}: {}", stderr_log.trim()))
            }
        }
        other => other,
    }
}

#[cfg(target_os = "macos")]
fn prepare_macos_system_audio_helper() -> CaptureResult<PathBuf> {
    let helper_dir = std::env::temp_dir()
        .join("kiku")
        .join("system-audio-helper");
    fs::create_dir_all(&helper_dir).map_err(|error| {
        CaptureError::SystemAudioHelperUnavailable(format!(
            "failed to create helper directory {}: {error}",
            helper_dir.display()
        ))
    })?;

    // Use a stable helper path so macOS permission grants can persist across
    // normal dev rebuilds instead of appearing as a brand-new binary each run.
    let helper_path = helper_dir.join("kiku-system-audio-helper");
    let mut write_binary = true;
    if let Ok(existing) = fs::read(&helper_path) {
        if existing == EMBEDDED_SYSTEM_AUDIO_HELPER {
            write_binary = false;
        }
    }

    if write_binary {
        fs::write(&helper_path, EMBEDDED_SYSTEM_AUDIO_HELPER).map_err(|error| {
            CaptureError::SystemAudioHelperUnavailable(format!(
                "failed to write helper binary {}: {error}",
                helper_path.display()
            ))
        })?;
    }

    let perms = fs::Permissions::from_mode(0o755);
    fs::set_permissions(&helper_path, perms).map_err(|error| {
        CaptureError::SystemAudioHelperUnavailable(format!(
            "failed to set helper permissions {}: {error}",
            helper_path.display()
        ))
    })?;

    Ok(helper_path)
}

fn build_mic_stream(
    level_bits: Arc<AtomicU32>,
    capture_samples: Arc<Mutex<VecDeque<f32>>>,
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
    let input_sample_rate_hz = stream_config.sample_rate.0.max(1);
    let error_callback = |error| {
        eprintln!("kiku mic stream error: {error}");
    };

    match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let bits = level_bits.clone();
            let samples = capture_samples.clone();
            device
                .build_input_stream(
                    &stream_config,
                    move |input: &[f32], _| {
                        update_level_and_store_from_f32(
                            input,
                            channels,
                            input_sample_rate_hz,
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
            let samples = capture_samples.clone();
            device
                .build_input_stream(
                    &stream_config,
                    move |input: &[i16], _| {
                        update_level_and_store_from_i16(
                            input,
                            channels,
                            input_sample_rate_hz,
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
            let samples = capture_samples.clone();
            device
                .build_input_stream(
                    &stream_config,
                    move |input: &[u16], _| {
                        update_level_and_store_from_u16(
                            input,
                            channels,
                            input_sample_rate_hz,
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

#[cfg(not(target_os = "macos"))]
fn build_system_stream(
    level_bits: Arc<AtomicU32>,
    capture_samples: Arc<Mutex<VecDeque<f32>>>,
) -> CaptureResult<cpal::Stream> {
    let host = cpal::default_host();
    let (device, config) = select_system_input_device(&host)?;
    let stream_config: cpal::StreamConfig = config.clone().into();
    let channels = stream_config.channels;
    let input_sample_rate_hz = stream_config.sample_rate.0.max(1);
    let error_callback = |error| {
        eprintln!("kiku system stream error: {error}");
    };

    match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let bits = level_bits.clone();
            let samples = capture_samples.clone();
            device
                .build_input_stream(
                    &stream_config,
                    move |input: &[f32], _| {
                        update_level_and_store_from_f32(
                            input,
                            channels,
                            input_sample_rate_hz,
                            &bits,
                            &samples,
                        )
                    },
                    error_callback,
                    None,
                )
                .map_err(|error| CaptureError::SystemAudioStreamBuild(error.to_string()))
        }
        cpal::SampleFormat::I16 => {
            let bits = level_bits.clone();
            let samples = capture_samples.clone();
            device
                .build_input_stream(
                    &stream_config,
                    move |input: &[i16], _| {
                        update_level_and_store_from_i16(
                            input,
                            channels,
                            input_sample_rate_hz,
                            &bits,
                            &samples,
                        )
                    },
                    error_callback,
                    None,
                )
                .map_err(|error| CaptureError::SystemAudioStreamBuild(error.to_string()))
        }
        cpal::SampleFormat::U16 => {
            let bits = level_bits.clone();
            let samples = capture_samples.clone();
            device
                .build_input_stream(
                    &stream_config,
                    move |input: &[u16], _| {
                        update_level_and_store_from_u16(
                            input,
                            channels,
                            input_sample_rate_hz,
                            &bits,
                            &samples,
                        )
                    },
                    error_callback,
                    None,
                )
                .map_err(|error| CaptureError::SystemAudioStreamBuild(error.to_string()))
        }
        _ => Err(CaptureError::SystemAudioUnsupportedSampleFormat),
    }
}

#[cfg(not(target_os = "macos"))]
fn select_system_input_device(
    host: &cpal::Host,
) -> CaptureResult<(cpal::Device, cpal::SupportedStreamConfig)> {
    if let Some(default_output) = host.default_output_device() {
        if let Ok(config) = default_output.default_input_config() {
            return Ok((default_output, config));
        }
        if let Ok(output_name) = default_output.name() {
            if let Some(candidate) = find_input_device_by_name(host, &output_name) {
                return Ok(candidate);
            }
        }
    }

    let mut best_candidate: Option<(i32, cpal::Device, cpal::SupportedStreamConfig)> = None;
    let devices = host
        .input_devices()
        .map_err(|_| CaptureError::SystemAudioDeviceUnavailable)?;
    for device in devices {
        let name = match device.name() {
            Ok(name) => name,
            Err(_) => continue,
        };
        let score = score_system_loopback_name(&name);
        if score <= 0 {
            continue;
        }
        let config = match device.default_input_config() {
            Ok(config) => config,
            Err(_) => continue,
        };

        if best_candidate
            .as_ref()
            .map(|(best_score, _, _)| score > *best_score)
            .unwrap_or(true)
        {
            best_candidate = Some((score, device, config));
        }
    }

    if let Some((_, device, config)) = best_candidate {
        return Ok((device, config));
    }

    Err(CaptureError::SystemAudioDeviceUnavailable)
}

#[cfg(not(target_os = "macos"))]
fn find_input_device_by_name(
    host: &cpal::Host,
    target_name: &str,
) -> Option<(cpal::Device, cpal::SupportedStreamConfig)> {
    let target = target_name.trim().to_ascii_lowercase();
    let devices = host.input_devices().ok()?;
    for device in devices {
        let name = match device.name() {
            Ok(name) => name,
            Err(_) => continue,
        };
        if name.trim().to_ascii_lowercase() != target {
            continue;
        }
        if let Ok(config) = device.default_input_config() {
            return Some((device, config));
        }
    }
    None
}

#[cfg(not(target_os = "macos"))]
fn score_system_loopback_name(name: &str) -> i32 {
    let normalized = name.to_ascii_lowercase();
    let mut score = 0;
    for hint in LOOPBACK_STRONG_HINTS {
        if normalized.contains(hint) {
            score += 4;
        }
    }
    for hint in MIC_HINTS {
        if normalized.contains(hint) {
            score -= 5;
        }
    }
    score
}

fn update_level_and_store_from_f32(
    samples: &[f32],
    channels: u16,
    input_sample_rate_hz: u32,
    level_bits: &AtomicU32,
    capture_samples: &Mutex<VecDeque<f32>>,
) {
    let mono = interleaved_to_mono(samples.iter().copied(), channels);
    ingest_capture_samples(mono, input_sample_rate_hz, level_bits, capture_samples);
}

fn update_level_and_store_from_i16(
    samples: &[i16],
    channels: u16,
    input_sample_rate_hz: u32,
    level_bits: &AtomicU32,
    capture_samples: &Mutex<VecDeque<f32>>,
) {
    let normalized = interleaved_to_mono(
        samples
            .iter()
            .copied()
            .map(|sample| sample as f32 / i16::MAX as f32),
        channels,
    );
    ingest_capture_samples(
        normalized,
        input_sample_rate_hz,
        level_bits,
        capture_samples,
    );
}

fn update_level_and_store_from_u16(
    samples: &[u16],
    channels: u16,
    input_sample_rate_hz: u32,
    level_bits: &AtomicU32,
    capture_samples: &Mutex<VecDeque<f32>>,
) {
    let normalized = interleaved_to_mono(
        samples
            .iter()
            .copied()
            .map(|sample| (sample as f32 / u16::MAX as f32) * 2.0 - 1.0),
        channels,
    );
    ingest_capture_samples(
        normalized,
        input_sample_rate_hz,
        level_bits,
        capture_samples,
    );
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

fn ingest_capture_samples(
    mono_samples: Vec<f32>,
    input_sample_rate_hz: u32,
    level_bits: &AtomicU32,
    capture_samples: &Mutex<VecDeque<f32>>,
) {
    if mono_samples.is_empty() {
        return;
    }

    let rms = rms_normalized(mono_samples.iter().copied());
    smooth_store_level(rms, level_bits);
    push_capture_samples(mono_samples, input_sample_rate_hz, capture_samples);
}

fn push_capture_samples(
    new_samples: Vec<f32>,
    input_sample_rate_hz: u32,
    capture_samples: &Mutex<VecDeque<f32>>,
) {
    if new_samples.is_empty() {
        return;
    }

    let mut buffer = match capture_samples.lock() {
        Ok(buffer) => buffer,
        Err(_) => return,
    };

    let resampled = if input_sample_rate_hz == CAPTURE_SAMPLE_RATE_HZ {
        new_samples
    } else {
        resample_linear(&new_samples, input_sample_rate_hz, CAPTURE_SAMPLE_RATE_HZ)
    };

    if resampled.is_empty() {
        return;
    }

    buffer.extend(resampled);
    let max_samples = CAPTURE_SAMPLE_RATE_HZ as usize * CAPTURE_RING_BUFFER_SECS;
    while buffer.len() > max_samples {
        let _ = buffer.pop_front();
    }
}

fn resample_linear(
    samples: &[f32],
    input_sample_rate_hz: u32,
    target_sample_rate_hz: u32,
) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }
    if input_sample_rate_hz == target_sample_rate_hz {
        return samples.to_vec();
    }

    let input_rate = input_sample_rate_hz.max(1) as f64;
    let target_rate = target_sample_rate_hz.max(1) as f64;
    let output_len = ((samples.len() as f64) * target_rate / input_rate).round() as usize;
    if output_len == 0 {
        return Vec::new();
    }
    if samples.len() == 1 {
        return vec![samples[0]; output_len];
    }

    let step = input_rate / target_rate;
    let mut out = Vec::with_capacity(output_len);
    for out_idx in 0..output_len {
        let src_pos = out_idx as f64 * step;
        let src_floor = src_pos.floor() as usize;
        let next_idx = (src_floor + 1).min(samples.len() - 1);
        let frac = (src_pos - src_floor as f64) as f32;
        let a = samples[src_floor.min(samples.len() - 1)];
        let b = samples[next_idx];
        out.push(a + (b - a) * frac);
    }
    out
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
