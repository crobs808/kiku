#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use kiku_asr::{AsrRuntime, Language, StubAsrRuntime, WhisperAsrRuntime};
use kiku_core::{
    AppController, CaptureSourceState, CoreError, LanguageConfig, LiveTranscriptLine,
    SessionSnapshot, SessionState,
};
use kiku_models::InMemoryModelManager;
use kiku_platform::{CaptureSource, CpalCaptureBackend};
use kiku_privacy::InMemoryPrivacyGuard;
use kiku_settings::InMemorySettingsStore;
use kiku_transcript::SourceIcon;
use serde::Serialize;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};

const MODEL_DOWNLOAD_CANCELLED: &str = "model download cancelled";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ModelAvailability {
    AvailableNow,
    Planned,
}

#[derive(Debug, Clone, Copy)]
struct ModelPackage {
    id: &'static str,
    name: &'static str,
    family: &'static str,
    filename: Option<&'static str>,
    url: Option<&'static str>,
    size: &'static str,
    approx_wer: &'static str,
    latency: &'static str,
    language_focus: &'static str,
    best_for: &'static str,
    notes: &'static str,
    availability: ModelAvailability,
    downloadable: bool,
    recommended: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct ModelOption {
    id: &'static str,
    name: &'static str,
    family: &'static str,
    size: &'static str,
    approx_wer: &'static str,
    latency: &'static str,
    language_focus: &'static str,
    best_for: &'static str,
    notes: &'static str,
    availability: ModelAvailability,
    downloadable: bool,
    recommended: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct ModelInventoryItem {
    id: &'static str,
    name: &'static str,
    family: &'static str,
    size: &'static str,
    approx_wer: &'static str,
    latency: &'static str,
    language_focus: &'static str,
    best_for: &'static str,
    notes: &'static str,
    availability: ModelAvailability,
    downloadable: bool,
    recommended: bool,
    installed: bool,
    active: bool,
}

impl ModelPackage {
    fn install_target(&self) -> Option<(&'static str, &'static str)> {
        self.filename.zip(self.url)
    }

    fn is_installable(&self) -> bool {
        self.downloadable && self.availability == ModelAvailability::AvailableNow
    }
}

const MODEL_PACKAGES: [ModelPackage; 8] = [
    ModelPackage {
        id: "large-v3",
        name: "Whisper Large v3",
        family: "Whisper.cpp",
        filename: Some("ggml-large-v3.bin"),
        url: Some("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin"),
        size: "3.1 GB",
        approx_wer: "~5.0% (best)",
        latency: "Higher latency, highest quality",
        language_focus: "Japanese + multilingual",
        best_for: "Best Japanese->English live meeting accuracy",
        notes: "Default recommendation when quality matters most.",
        availability: ModelAvailability::AvailableNow,
        downloadable: true,
        recommended: true,
    },
    ModelPackage {
        id: "medium",
        name: "Whisper Medium",
        family: "Whisper.cpp",
        filename: Some("ggml-medium.bin"),
        url: Some("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin"),
        size: "1.5 GB",
        approx_wer: "~6.1%",
        latency: "Balanced quality vs speed",
        language_focus: "Japanese + English",
        best_for: "Balanced option for long meetings on most Macs",
        notes: "Good fallback when Large v3 feels too heavy.",
        availability: ModelAvailability::AvailableNow,
        downloadable: true,
        recommended: false,
    },
    ModelPackage {
        id: "small",
        name: "Whisper Small",
        family: "Whisper.cpp",
        filename: Some("ggml-small.bin"),
        url: Some("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin"),
        size: "466 MB",
        approx_wer: "~7.6%",
        latency: "Faster startup and inference",
        language_focus: "English-centric / mixed Japanese",
        best_for: "Speed-first testing and lighter hardware sessions",
        notes: "Useful when responsiveness matters more than top accuracy.",
        availability: ModelAvailability::AvailableNow,
        downloadable: true,
        recommended: false,
    },
    ModelPackage {
        id: "base",
        name: "Whisper Base",
        family: "Whisper.cpp",
        filename: Some("ggml-base.bin"),
        url: Some("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin"),
        size: "142 MB",
        approx_wer: "~10.2%",
        latency: "Fastest / lowest quality",
        language_focus: "Quick setup and demos",
        best_for: "Smoke tests and low-resource prototypes",
        notes: "Expect noticeably weaker Japanese translation accuracy.",
        availability: ModelAvailability::AvailableNow,
        downloadable: true,
        recommended: false,
    },
    ModelPackage {
        id: "kotoba-whisper-v2",
        name: "Kotoba-Whisper v2 (Candidate)",
        family: "Kotoba / Whisper family",
        filename: None,
        url: None,
        size: "~3-4 GB (varies by package)",
        approx_wer: "N/A in current build",
        latency: "Expected medium-high",
        language_focus: "Japanese-focused ASR",
        best_for: "Future Japanese-heavy transcription quality",
        notes: "Planned integration target for stronger Japanese specialization.",
        availability: ModelAvailability::Planned,
        downloadable: false,
        recommended: false,
    },
    ModelPackage {
        id: "reazonspeech-nemo",
        name: "ReazonSpeech NeMo (Candidate)",
        family: "NVIDIA NeMo",
        filename: None,
        url: None,
        size: "~2-3 GB (varies by checkpoint)",
        approx_wer: "N/A in current build",
        latency: "Expected medium",
        language_focus: "Japanese ASR",
        best_for: "Future lower-latency Japanese transcription path",
        notes: "Planned non-Whisper backend option for Japanese meetings.",
        availability: ModelAvailability::Planned,
        downloadable: false,
        recommended: false,
    },
    ModelPackage {
        id: "parakeet-ja",
        name: "Parakeet-TDT JA (Candidate)",
        family: "NVIDIA Parakeet",
        filename: None,
        url: None,
        size: "~2-3 GB (estimated runtime package)",
        approx_wer: "N/A in current build",
        latency: "Expected medium-fast",
        language_focus: "Japanese ASR",
        best_for: "Future real-time Japanese captions with good throughput",
        notes: "Planned evaluation target for speed/quality balance.",
        availability: ModelAvailability::Planned,
        downloadable: false,
        recommended: false,
    },
    ModelPackage {
        id: "seamless-m4t-v2",
        name: "SeamlessM4T v2 (Candidate)",
        family: "Meta Seamless",
        filename: None,
        url: None,
        size: "~5+ GB",
        approx_wer: "Uses non-WER benchmark metrics",
        latency: "High compute",
        language_focus: "Multilingual speech translation",
        best_for: "Future multi-language expansion beyond EN/JA",
        notes: "Planned, with licensing/product review required before shipping.",
        availability: ModelAvailability::Planned,
        downloadable: false,
        recommended: false,
    },
];

struct DesktopState {
    controller: Arc<Mutex<AppController>>,
    model_download: Arc<Mutex<ModelDownloadState>>,
    model_root: PathBuf,
    active_model_id: Arc<Mutex<Option<String>>>,
}

#[derive(Debug, Clone, Serialize)]
struct ModelDownloadProgress {
    in_progress: bool,
    progress: f32,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    installed: bool,
    model_id: Option<String>,
    model_name: Option<String>,
    last_error: Option<String>,
}

#[derive(Debug, Default)]
struct ModelDownloadState {
    in_progress: bool,
    progress: f32,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    installed: bool,
    model_id: Option<String>,
    model_name: Option<String>,
    last_error: Option<String>,
    cancel_requested: bool,
}

impl ModelDownloadState {
    fn snapshot(&self) -> ModelDownloadProgress {
        ModelDownloadProgress {
            in_progress: self.in_progress,
            progress: self.progress,
            downloaded_bytes: self.downloaded_bytes,
            total_bytes: self.total_bytes,
            installed: self.installed,
            model_id: self.model_id.clone(),
            model_name: self.model_name.clone(),
            last_error: self.last_error.clone(),
        }
    }
}

#[tauri::command]
fn get_session_snapshot(state: State<DesktopState>) -> Result<SessionSnapshot, String> {
    with_controller(&state, |controller| Ok(controller.session_snapshot()))
}

#[tauri::command]
fn start_listening(state: State<DesktopState>) -> Result<SessionSnapshot, String> {
    with_controller(&state, |controller| controller.start_listening())
}

#[tauri::command]
fn stop_listening(state: State<DesktopState>) -> Result<SessionSnapshot, String> {
    with_controller(&state, |controller| controller.stop_listening())
}

#[tauri::command]
fn discard_transcript(state: State<DesktopState>) -> Result<SessionSnapshot, String> {
    with_controller(&state, |controller| controller.discard_transcript())
}

#[tauri::command]
fn save_transcript(state: State<DesktopState>) -> Result<String, String> {
    with_controller(&state, |controller| {
        controller.save_transcript().map(|(text, _)| text)
    })
}

#[tauri::command]
fn append_transcript_line(
    state: State<DesktopState>,
    timestamp_ms: u64,
    text: String,
) -> Result<(), String> {
    with_controller(&state, |controller| {
        controller.append_transcript_line(timestamp_ms, SourceIcon::Mic, text);
        Ok(())
    })
}

#[tauri::command]
fn get_source_state(state: State<DesktopState>) -> Result<CaptureSourceState, String> {
    with_controller(&state, |controller| controller.capture_source_state())
}

#[tauri::command]
fn set_mic_enabled(
    state: State<DesktopState>,
    enabled: bool,
) -> Result<CaptureSourceState, String> {
    with_controller(&state, |controller| {
        controller.set_source_enabled(CaptureSource::Mic, enabled)
    })
}

#[tauri::command]
fn set_system_audio_enabled(
    state: State<DesktopState>,
    enabled: bool,
) -> Result<CaptureSourceState, String> {
    with_controller(&state, |controller| {
        controller.set_source_enabled(CaptureSource::SystemAudio, enabled)
    })
}

#[tauri::command]
fn get_audio_level(state: State<DesktopState>) -> Result<f32, String> {
    with_controller(&state, |controller| controller.audio_level())
}

#[tauri::command]
fn get_language_config(state: State<DesktopState>) -> Result<LanguageConfig, String> {
    with_controller(&state, |controller| Ok(controller.language_config()))
}

#[tauri::command]
fn set_language_config(
    state: State<DesktopState>,
    source_language: Language,
    target_language: Language,
) -> Result<LanguageConfig, String> {
    with_controller(&state, |controller| {
        controller.set_language_config(source_language, target_language)
    })
}

#[tauri::command]
fn poll_live_transcript_lines(
    state: State<DesktopState>,
) -> Result<Vec<LiveTranscriptLine>, String> {
    with_controller(&state, |controller| controller.poll_live_transcript_lines())
}

#[tauri::command]
fn get_model_catalog() -> Vec<ModelOption> {
    model_options()
}

#[tauri::command]
fn get_model_inventory(state: State<DesktopState>) -> Result<Vec<ModelInventoryItem>, String> {
    list_model_inventory(&state)
}

#[tauri::command]
fn get_model_download_progress(
    state: State<DesktopState>,
) -> Result<ModelDownloadProgress, String> {
    let download = state
        .model_download
        .lock()
        .map_err(|_| "model download lock poisoned".to_owned())?;
    Ok(download.snapshot())
}

#[tauri::command]
fn set_active_model(
    state: State<DesktopState>,
    model_id: String,
) -> Result<Vec<ModelInventoryItem>, String> {
    ensure_model_mutation_allowed(&state)?;

    let package = find_model_package(&model_id)
        .ok_or_else(|| format!("unknown model option '{model_id}'"))?;
    if !package.is_installable() {
        return Err(format!(
            "{} is a planned model and is not runnable in this build yet.",
            package.name
        ));
    }
    let model_path = model_path_for_package(&state.model_root, package)
        .ok_or_else(|| format!("{} has no install path configured.", package.name))?;
    if !model_path.exists() {
        return Err(format!(
            "{} is not installed yet. Download it first.",
            package.name
        ));
    }

    activate_installed_model(&state.controller, &model_path)?;
    {
        let mut active_id = state
            .active_model_id
            .lock()
            .map_err(|_| "active model lock poisoned".to_owned())?;
        *active_id = Some(package.id.to_owned());
    }

    list_model_inventory(&state)
}

#[tauri::command]
fn delete_model(
    state: State<DesktopState>,
    model_id: String,
) -> Result<Vec<ModelInventoryItem>, String> {
    ensure_model_mutation_allowed(&state)?;

    let package = find_model_package(&model_id)
        .ok_or_else(|| format!("unknown model option '{model_id}'"))?;
    if !package.is_installable() {
        return Err(format!(
            "{} is a planned model entry and cannot be deleted from local runtime storage.",
            package.name
        ));
    }
    let model_path = model_path_for_package(&state.model_root, package)
        .ok_or_else(|| format!("{} has no install path configured.", package.name))?;
    if model_path.exists() {
        std::fs::remove_file(&model_path).map_err(|error| error.to_string())?;
    }

    let was_active = {
        let active_id = state
            .active_model_id
            .lock()
            .map_err(|_| "active model lock poisoned".to_owned())?;
        active_id.as_deref() == Some(package.id)
    };

    if was_active {
        if let Some(next_package) = first_installed_package(&state.model_root) {
            let next_path = model_path_for_package(&state.model_root, next_package)
                .ok_or_else(|| format!("{} has no install path configured.", next_package.name))?;
            activate_installed_model(&state.controller, &next_path)?;
            let mut active_id = state
                .active_model_id
                .lock()
                .map_err(|_| "active model lock poisoned".to_owned())?;
            *active_id = Some(next_package.id.to_owned());
        } else {
            let mut controller = state
                .controller
                .lock()
                .map_err(|_| "controller lock poisoned".to_owned())?;
            controller.set_asr_runtime(Arc::new(StubAsrRuntime::default()));
            controller.mark_model_missing();
            let mut active_id = state
                .active_model_id
                .lock()
                .map_err(|_| "active model lock poisoned".to_owned())?;
            *active_id = None;
        }
    }

    list_model_inventory(&state)
}

#[tauri::command]
fn start_model_download(
    state: State<DesktopState>,
    model_id: Option<String>,
) -> Result<ModelDownloadProgress, String> {
    ensure_model_mutation_allowed(&state)?;

    let selected = match model_id {
        Some(id) => {
            find_model_package(&id).ok_or_else(|| format!("unknown model option '{id}'"))?
        }
        None => &MODEL_PACKAGES[0],
    };
    if !selected.is_installable() {
        return Err(format!(
            "{} is listed as a planned integration and cannot be downloaded in this build yet.",
            selected.name
        ));
    }
    let (selected_filename, selected_url) = selected.install_target().ok_or_else(|| {
        format!(
            "{} is missing download metadata and cannot be installed.",
            selected.name
        )
    })?;

    {
        let mut download = state
            .model_download
            .lock()
            .map_err(|_| "model download lock poisoned".to_owned())?;
        if download.in_progress {
            return Ok(download.snapshot());
        }

        download.in_progress = true;
        download.progress = 0.0;
        download.downloaded_bytes = 0;
        download.total_bytes = None;
        download.installed = false;
        download.model_id = Some(selected.id.to_owned());
        download.model_name = Some(selected.name.to_owned());
        download.last_error = None;
        download.cancel_requested = false;
    }

    if let Err(error) = with_controller(&state, |controller| controller.begin_model_install()) {
        let mut download = state
            .model_download
            .lock()
            .map_err(|_| "model download lock poisoned".to_owned())?;
        download.in_progress = false;
        download.progress = 0.0;
        download.downloaded_bytes = 0;
        download.total_bytes = None;
        download.last_error = Some(error.clone());
        return Err(error);
    }

    let controller = Arc::clone(&state.controller);
    let download_state = Arc::clone(&state.model_download);
    let active_model_id = Arc::clone(&state.active_model_id);
    let model_root = state.model_root.clone();
    let model_path = state.model_root.join(selected_filename);
    let model_url = selected_url.to_owned();
    let selected_model_id = selected.id.to_owned();
    std::thread::spawn(move || {
        let download_result = download_model_file(&model_path, &model_url, &download_state)
            .and_then(|_| activate_installed_model(&controller, &model_path));

        match download_result {
            Ok(()) => {
                if let Ok(mut download) = download_state.lock() {
                    download.in_progress = false;
                    download.progress = 1.0;
                    download.installed = true;
                    download.last_error = None;
                    download.cancel_requested = false;
                }
                if let Ok(mut active) = active_model_id.lock() {
                    *active = Some(selected_model_id);
                }
            }
            Err(error) => {
                let cancelled = error == MODEL_DOWNLOAD_CANCELLED;
                let has_installed_models = first_installed_package(&model_root).is_some();
                if let Ok(mut download) = download_state.lock() {
                    download.in_progress = false;
                    download.progress = if cancelled { 0.0 } else { download.progress };
                    if cancelled {
                        download.downloaded_bytes = 0;
                        download.total_bytes = None;
                    }
                    download.installed = has_installed_models;
                    download.last_error = if cancelled { None } else { Some(error.clone()) };
                    download.cancel_requested = false;
                }

                if let Ok(mut controller) = controller.lock() {
                    if has_installed_models {
                        controller.recover_ready();
                    } else {
                        controller.mark_model_missing();
                    }
                }
            }
        }
    });

    let download = state
        .model_download
        .lock()
        .map_err(|_| "model download lock poisoned".to_owned())?;
    Ok(download.snapshot())
}

#[tauri::command]
fn cancel_model_download(state: State<DesktopState>) -> Result<ModelDownloadProgress, String> {
    let mut download = state
        .model_download
        .lock()
        .map_err(|_| "model download lock poisoned".to_owned())?;
    if download.in_progress {
        download.cancel_requested = true;
        download.last_error = None;
    }
    Ok(download.snapshot())
}

fn download_model_file(
    model_path: &Path,
    model_url: &str,
    download_state: &Arc<Mutex<ModelDownloadState>>,
) -> Result<(), String> {
    let parent = model_path
        .parent()
        .ok_or_else(|| "invalid model path".to_owned())?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;

    let temp_path = model_path.with_extension("bin.part");
    let cleanup_temp = || {
        if temp_path.exists() {
            let _ = std::fs::remove_file(&temp_path);
        }
    };
    cleanup_temp();

    let result = (|| -> Result<(), String> {
        if is_download_cancel_requested(download_state) {
            return Err(MODEL_DOWNLOAD_CANCELLED.to_owned());
        }

        let client = reqwest::blocking::Client::builder()
            .build()
            .map_err(|error| format!("failed to create downloader: {error}"))?;
        let mut response = client
            .get(model_url)
            .send()
            .map_err(|error| format!("model download failed: {error}"))?;

        if !response.status().is_success() {
            return Err(format!(
                "model download failed with status {}",
                response.status()
            ));
        }

        let total_bytes = response.content_length();
        if let Ok(mut download) = download_state.lock() {
            download.total_bytes = total_bytes;
        }

        let mut file = std::fs::File::create(&temp_path)
            .map_err(|error| format!("failed to open model file: {error}"))?;
        let mut downloaded_bytes = 0u64;
        let mut buffer = [0u8; 64 * 1024];

        loop {
            if is_download_cancel_requested(download_state) {
                return Err(MODEL_DOWNLOAD_CANCELLED.to_owned());
            }

            let read_count = response
                .read(&mut buffer)
                .map_err(|error| format!("failed while downloading model: {error}"))?;
            if read_count == 0 {
                break;
            }

            file.write_all(&buffer[..read_count])
                .map_err(|error| format!("failed while writing model file: {error}"))?;
            downloaded_bytes += read_count as u64;

            if let Ok(mut download) = download_state.lock() {
                download.downloaded_bytes = downloaded_bytes;
                download.progress = match total_bytes {
                    Some(total) if total > 0 => {
                        (downloaded_bytes as f32 / total as f32).clamp(0.0, 1.0)
                    }
                    _ => 0.0,
                };
            }
        }

        file.flush()
            .map_err(|error| format!("failed while finalizing model file: {error}"))?;
        std::fs::rename(&temp_path, model_path)
            .map_err(|error| format!("failed to activate model file: {error}"))?;

        Ok(())
    })();

    if result.is_err() {
        cleanup_temp();
    }

    result
}

fn is_download_cancel_requested(download_state: &Arc<Mutex<ModelDownloadState>>) -> bool {
    download_state
        .lock()
        .map(|download| download.cancel_requested)
        .unwrap_or(false)
}

fn activate_installed_model(
    controller: &Arc<Mutex<AppController>>,
    model_path: &Path,
) -> Result<(), String> {
    let runtime = WhisperAsrRuntime::new(model_path).map_err(|error| error.to_string())?;
    let mut controller = controller
        .lock()
        .map_err(|_| "controller lock poisoned".to_owned())?;
    controller.set_asr_runtime(Arc::new(runtime));
    let current_state = controller.session_snapshot().state;
    if current_state != SessionState::DownloadingModel {
        controller
            .begin_model_install()
            .map_err(|error| error.to_string())?;
    }
    controller
        .complete_model_install()
        .map(|_| ())
        .map_err(|error| error.to_string())?;
    std::env::set_var("KIKU_WHISPER_MODEL", model_path);
    Ok(())
}

fn model_options() -> Vec<ModelOption> {
    MODEL_PACKAGES
        .iter()
        .map(|package| ModelOption {
            id: package.id,
            name: package.name,
            family: package.family,
            size: package.size,
            approx_wer: package.approx_wer,
            latency: package.latency,
            language_focus: package.language_focus,
            best_for: package.best_for,
            notes: package.notes,
            availability: package.availability,
            downloadable: package.downloadable,
            recommended: package.recommended,
        })
        .collect()
}

fn find_model_package(model_id: &str) -> Option<&'static ModelPackage> {
    MODEL_PACKAGES.iter().find(|package| package.id == model_id)
}

fn model_path_for_package(model_root: &Path, package: &ModelPackage) -> Option<PathBuf> {
    package.filename.map(|filename| model_root.join(filename))
}

fn first_installed_package(model_root: &Path) -> Option<&'static ModelPackage> {
    MODEL_PACKAGES
        .iter()
        .filter(|package| package.is_installable())
        .find(|package| {
            model_path_for_package(model_root, package)
                .map(|path| path.exists())
                .unwrap_or(false)
        })
}

fn model_id_for_path(path: &Path) -> Option<String> {
    let filename = path.file_name()?.to_str()?;
    find_model_package_by_filename(filename).map(|package| package.id.to_owned())
}

fn find_model_package_by_filename(filename: &str) -> Option<&'static ModelPackage> {
    MODEL_PACKAGES
        .iter()
        .find(|package| package.filename == Some(filename))
}

fn list_model_inventory(state: &State<DesktopState>) -> Result<Vec<ModelInventoryItem>, String> {
    let active_id = state
        .active_model_id
        .lock()
        .map_err(|_| "active model lock poisoned".to_owned())?
        .clone();

    Ok(MODEL_PACKAGES
        .iter()
        .map(|package| ModelInventoryItem {
            id: package.id,
            name: package.name,
            family: package.family,
            size: package.size,
            approx_wer: package.approx_wer,
            latency: package.latency,
            language_focus: package.language_focus,
            best_for: package.best_for,
            notes: package.notes,
            availability: package.availability,
            downloadable: package.downloadable,
            recommended: package.recommended,
            installed: model_path_for_package(&state.model_root, package)
                .map(|path| path.exists())
                .unwrap_or(false),
            active: active_id.as_deref() == Some(package.id),
        })
        .collect())
}

fn ensure_model_mutation_allowed(state: &State<DesktopState>) -> Result<(), String> {
    let snapshot = with_controller(state, |controller| Ok(controller.session_snapshot()))?;
    if matches!(
        snapshot.state,
        SessionState::Listening
            | SessionState::Stopping
            | SessionState::PromptingSaveDiscard
            | SessionState::SavingTranscript
    ) {
        return Err("stop listening before changing models".to_owned());
    }
    Ok(())
}

fn with_controller<T>(
    state: &State<DesktopState>,
    op: impl FnOnce(&mut AppController) -> Result<T, CoreError>,
) -> Result<T, String> {
    let mut controller = state
        .controller
        .lock()
        .map_err(|_| "controller lock poisoned".to_owned())?;

    op(&mut controller).map_err(|error| error.to_string())
}

fn build_controller() -> AppController {
    let settings = Arc::new(InMemorySettingsStore::default());
    let (asr, model_installed): (Arc<dyn AsrRuntime>, bool) =
        match WhisperAsrRuntime::from_default_model_locations() {
            Ok(runtime) => (Arc::new(runtime), true),
            Err(error) => {
                eprintln!("kiku asr runtime fallback: {error}");
                (Arc::new(StubAsrRuntime::default()), false)
            }
        };
    let models = Arc::new(InMemoryModelManager::new(model_installed));
    let capture = Arc::new(CpalCaptureBackend::default());
    let privacy = Arc::new(InMemoryPrivacyGuard::default());

    let mut controller = AppController::new(settings, models, asr, capture, privacy);
    if let Err(error) = controller.boot() {
        controller.fail_session(error.to_string());
    }

    controller
}

fn resolve_model_path(app: &tauri::AppHandle) -> PathBuf {
    if let Ok(explicit) = std::env::var("KIKU_WHISPER_MODEL") {
        return PathBuf::from(explicit);
    }

    let mut preferred_candidates = MODEL_PACKAGES
        .iter()
        .filter(|package| package.is_installable())
        .filter_map(|package| package.filename)
        .flat_map(|filename| {
            [
                PathBuf::from(format!("models/{filename}")),
                PathBuf::from(format!("models/whisper/{filename}")),
            ]
        });
    if let Some(existing) = preferred_candidates.find(|path| path.exists()) {
        return existing;
    }

    let default_filename = MODEL_PACKAGES
        .iter()
        .find(|package| package.recommended && package.is_installable())
        .and_then(|package| package.filename)
        .unwrap_or("ggml-large-v3.bin");

    let fallback_path = if let Ok(data_dir) = app.path().app_data_dir() {
        data_dir.join("models").join(default_filename)
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("models")
            .join(default_filename)
    };
    std::env::set_var("KIKU_WHISPER_MODEL", &fallback_path);
    fallback_path
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_session_snapshot,
            start_listening,
            stop_listening,
            discard_transcript,
            save_transcript,
            append_transcript_line,
            get_source_state,
            set_mic_enabled,
            set_system_audio_enabled,
            get_audio_level,
            get_language_config,
            set_language_config,
            poll_live_transcript_lines,
            get_model_catalog,
            get_model_inventory,
            get_model_download_progress,
            start_model_download,
            cancel_model_download,
            set_active_model,
            delete_model
        ])
        .setup(|app| {
            let model_path = resolve_model_path(app.handle());
            std::env::set_var("KIKU_WHISPER_MODEL", &model_path);
            let model_root = model_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("models"));
            let initial_active_model_id = if model_path.exists() {
                model_id_for_path(&model_path)
            } else {
                None
            };

            let controller = build_controller();
            let is_model_installed =
                matches!(controller.session_snapshot().state, SessionState::Ready);

            app.manage(DesktopState {
                controller: Arc::new(Mutex::new(controller)),
                model_download: Arc::new(Mutex::new(ModelDownloadState {
                    installed: is_model_installed,
                    model_id: initial_active_model_id.clone(),
                    model_name: initial_active_model_id
                        .as_deref()
                        .and_then(find_model_package)
                        .map(|package| package.name.to_owned()),
                    ..Default::default()
                })),
                model_root,
                active_model_id: Arc::new(Mutex::new(initial_active_model_id)),
            });

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_always_on_top(true);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running kiku desktop application");
}
