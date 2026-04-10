use crate::{SessionError, SessionMachine, SessionState};
use kiku_asr::{AsrError, AsrOutput, AsrRequest, AsrRuntime, Language};
use kiku_models::{ModelError, ModelManager, ModelPreset};
use kiku_platform::{CaptureBackend, CaptureError, CaptureSource};
use kiku_privacy::{PrivacyError, PrivacyGuard};
use kiku_settings::{
    LanguageCode, LanguagePair as SettingsLanguagePair, SettingsError, SettingsStore,
};
use kiku_transcript::{SourceIcon, TranscriptBuffer};
use kiku_translate::{Language as TranslationLanguage, StubTranslator, Translator};
use serde::Serialize;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;

const MIN_INFER_INTERVAL: Duration = Duration::from_millis(900);
const MIN_AUDIO_WINDOW_SECS: usize = 2;
const MAX_AUDIO_WINDOW_SECS: usize = 4;
const MAX_AUDIO_BUFFER_SECS: usize = 10;
const DRAIN_PER_POLL_SECS: usize = 2;
const SILENCE_RMS_THRESHOLD: f32 = 0.010;
const RETAIN_TAIL_SECS: usize = 2;

#[derive(Debug, Serialize)]
pub struct SessionSnapshot {
    pub state: SessionState,
    pub offline_mode_active: bool,
    pub transcript_line_count: usize,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CaptureSourceState {
    pub mic_enabled: bool,
    pub system_audio_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveTranscriptLine {
    pub timestamp_ms: u64,
    pub source: SourceIcon,
    pub text: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct LanguageConfig {
    pub source_language: Language,
    pub target_language: Language,
}

#[derive(Debug, Error)]
pub enum CoreError {
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(transparent)]
    Settings(#[from] SettingsError),
    #[error(transparent)]
    Models(#[from] ModelError),
    #[error(transparent)]
    Capture(#[from] CaptureError),
    #[error(transparent)]
    Privacy(#[from] PrivacyError),
    #[error(transparent)]
    Asr(#[from] AsrError),
    #[error(transparent)]
    Translation(#[from] kiku_translate::TranslationError),
}

pub struct AppController {
    session: SessionMachine,
    settings: Arc<dyn SettingsStore>,
    models: Arc<dyn ModelManager>,
    asr: Arc<dyn AsrRuntime>,
    capture: Arc<dyn CaptureBackend>,
    privacy: Arc<dyn PrivacyGuard>,
    translator: Arc<dyn Translator>,
    transcript: TranscriptBuffer,
    live_audio_samples: Vec<f32>,
    live_sample_rate_hz: u32,
    source_language: Language,
    target_language: Language,
    listening_started_at: Option<Instant>,
    last_infer_at: Option<Instant>,
    last_emitted_normalized: Option<String>,
    pending_inference: Option<PendingInference>,
}

struct PendingInference {
    receiver: Receiver<Result<AsrOutput, AsrError>>,
}

impl AppController {
    pub fn new(
        settings: Arc<dyn SettingsStore>,
        models: Arc<dyn ModelManager>,
        asr: Arc<dyn AsrRuntime>,
        capture: Arc<dyn CaptureBackend>,
        privacy: Arc<dyn PrivacyGuard>,
    ) -> Self {
        Self {
            session: SessionMachine::default(),
            settings,
            models,
            asr,
            capture,
            privacy,
            translator: Arc::new(StubTranslator::default()),
            transcript: TranscriptBuffer::default(),
            live_audio_samples: Vec::new(),
            live_sample_rate_hz: 16_000,
            source_language: Language::Japanese,
            target_language: Language::English,
            listening_started_at: None,
            last_infer_at: None,
            last_emitted_normalized: None,
            pending_inference: None,
        }
    }

    pub fn boot(&mut self) -> Result<SessionSnapshot, CoreError> {
        let settings = self.settings.load()?;
        self.source_language = language_from_code(settings.preferred_language_pair.input);
        self.target_language = language_from_code(settings.preferred_language_pair.output);
        self.capture
            .set_source_enabled(CaptureSource::Mic, settings.mic_enabled_by_default)?;
        self.capture.set_source_enabled(
            CaptureSource::SystemAudio,
            settings.system_audio_enabled_by_default,
        )?;

        let model_status = self.models.status()?;
        if model_status.installed {
            self.session.set_ready();
        } else {
            self.session.set_model_missing();
        }

        Ok(self.session_snapshot())
    }

    pub fn start_listening(&mut self) -> Result<SessionSnapshot, CoreError> {
        self.privacy.enter_offline_mode()?;
        if let Err(error) = self.capture.start() {
            let _ = self.privacy.exit_offline_mode();
            return Err(error.into());
        }

        if let Err(error) = self.session.start_listening() {
            let _ = self.capture.stop();
            let _ = self.privacy.exit_offline_mode();
            return Err(error.into());
        }

        self.live_audio_samples.clear();
        self.live_sample_rate_hz = self.capture.mic_sample_rate_hz().unwrap_or(16_000).max(1);
        self.listening_started_at = Some(Instant::now());
        self.last_infer_at = None;
        self.last_emitted_normalized = None;
        self.pending_inference = None;

        Ok(self.session_snapshot())
    }

    pub fn begin_model_install(&mut self) -> Result<SessionSnapshot, CoreError> {
        self.session.begin_model_download()?;
        Ok(self.session_snapshot())
    }

    pub fn complete_model_install(&mut self) -> Result<SessionSnapshot, CoreError> {
        self.models.ensure_installed(ModelPreset::BestAccuracy)?;
        self.session.finish_model_download()?;
        Ok(self.session_snapshot())
    }

    pub fn mark_model_missing(&mut self) -> SessionSnapshot {
        self.session.set_model_missing();
        self.session_snapshot()
    }

    pub fn recover_ready(&mut self) -> SessionSnapshot {
        self.session.recover_to_ready();
        self.session_snapshot()
    }

    pub fn set_asr_runtime(&mut self, runtime: Arc<dyn AsrRuntime>) {
        self.asr = runtime;
    }

    pub fn language_config(&self) -> LanguageConfig {
        LanguageConfig {
            source_language: self.source_language,
            target_language: self.target_language,
        }
    }

    pub fn set_language_config(
        &mut self,
        source_language: Language,
        target_language: Language,
    ) -> Result<LanguageConfig, CoreError> {
        validate_language_config(source_language, target_language)?;

        self.source_language = source_language;
        self.target_language = target_language;
        self.last_emitted_normalized = None;

        let mut settings = self.settings.load()?;
        settings.preferred_language_pair = SettingsLanguagePair {
            input: language_to_code(source_language),
            output: language_to_code(target_language),
        };
        self.settings.save(&settings)?;

        Ok(self.language_config())
    }

    pub fn stop_listening(&mut self) -> Result<SessionSnapshot, CoreError> {
        self.session.stop_listening()?;

        if let Err(error) = self.capture.stop() {
            if !matches!(error, CaptureError::NotRunning) {
                self.session.fail(error.to_string());
                return Err(error.into());
            }
        }

        if let Err(error) = self.privacy.exit_offline_mode() {
            self.session.fail(error.to_string());
            return Err(error.into());
        }

        self.session.prompt_save_discard()?;
        self.reset_live_state();

        Ok(self.session_snapshot())
    }

    pub fn append_transcript_line(
        &mut self,
        timestamp_ms: u64,
        source: SourceIcon,
        text: impl Into<String>,
    ) {
        self.transcript.add_line(timestamp_ms, source, text);
    }

    pub fn discard_transcript(&mut self) -> Result<SessionSnapshot, CoreError> {
        self.session.discard_transcript()?;
        self.transcript.clear();
        self.reset_live_state();
        Ok(self.session_snapshot())
    }

    pub fn save_transcript(&mut self) -> Result<(String, SessionSnapshot), CoreError> {
        self.session.begin_save_transcript()?;
        let exported = self.transcript.export_plain_text();
        self.session.finish_save_transcript()?;
        self.transcript.clear();
        self.reset_live_state();

        Ok((exported, self.session_snapshot()))
    }

    pub fn session_snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            state: self.session.state(),
            offline_mode_active: self.privacy.offline_mode_active(),
            transcript_line_count: self.transcript.len(),
            last_error: self.session.last_error().map(ToOwned::to_owned),
        }
    }

    pub fn set_source_enabled(
        &mut self,
        source: CaptureSource,
        enabled: bool,
    ) -> Result<CaptureSourceState, CoreError> {
        self.capture.set_source_enabled(source, enabled)?;
        self.capture_source_state()
    }

    pub fn capture_source_state(&self) -> Result<CaptureSourceState, CoreError> {
        Ok(CaptureSourceState {
            mic_enabled: self.capture.source_enabled(CaptureSource::Mic)?,
            system_audio_enabled: self.capture.source_enabled(CaptureSource::SystemAudio)?,
        })
    }

    pub fn audio_level(&self) -> Result<f32, CoreError> {
        self.capture.latest_level().map_err(Into::into)
    }

    pub fn poll_live_transcript_lines(&mut self) -> Result<Vec<LiveTranscriptLine>, CoreError> {
        if self.session.state() != SessionState::Listening {
            return Ok(Vec::new());
        }

        let mut emitted_lines = Vec::new();

        let sample_rate_hz = self.capture.mic_sample_rate_hz()?.max(1);
        if sample_rate_hz != self.live_sample_rate_hz {
            self.live_sample_rate_hz = sample_rate_hz;
            self.live_audio_samples.clear();
        }

        let max_drain = sample_rate_hz as usize * DRAIN_PER_POLL_SECS;
        let drained = self.capture.drain_mic_samples(max_drain)?;
        if !drained.is_empty() {
            self.live_audio_samples.extend(drained);
        }

        let max_buffer_samples = sample_rate_hz as usize * MAX_AUDIO_BUFFER_SECS;
        if self.live_audio_samples.len() > max_buffer_samples {
            let overflow = self.live_audio_samples.len() - max_buffer_samples;
            self.live_audio_samples.drain(..overflow);
        }

        if let Some(pending) = &self.pending_inference {
            match pending.receiver.try_recv() {
                Ok(result) => {
                    self.pending_inference = None;
                    let output = result?;
                    if let Some(line) = self.process_asr_output(output, sample_rate_hz)? {
                        emitted_lines.push(line);
                    }
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.pending_inference = None;
                    return Err(CoreError::Asr(AsrError::InferenceFailed(
                        "asr worker disconnected".to_owned(),
                    )));
                }
            }
        }

        if self.pending_inference.is_some() {
            return Ok(emitted_lines);
        }

        let min_samples = sample_rate_hz as usize * MIN_AUDIO_WINDOW_SECS;
        if self.live_audio_samples.len() < min_samples {
            return Ok(emitted_lines);
        }

        if let Some(last) = self.last_infer_at {
            if last.elapsed() < MIN_INFER_INTERVAL {
                return Ok(emitted_lines);
            }
        }

        let window_samples =
            (sample_rate_hz as usize * MAX_AUDIO_WINDOW_SECS).min(self.live_audio_samples.len());
        let window_start = self.live_audio_samples.len() - window_samples;
        let inference_window = &self.live_audio_samples[window_start..];

        let rms = rms(inference_window);
        if rms < SILENCE_RMS_THRESHOLD {
            trim_to_tail(
                &mut self.live_audio_samples,
                sample_rate_hz as usize * RETAIN_TAIL_SECS,
            );
            return Ok(emitted_lines);
        }

        let request = AsrRequest {
            source_language: self.source_language,
            target_language: self.target_language,
            sample_rate_hz,
            audio_samples: inference_window.to_vec(),
        };

        let asr = Arc::clone(&self.asr);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(asr.infer(&request));
        });
        self.pending_inference = Some(PendingInference { receiver: rx });
        self.last_infer_at = Some(Instant::now());

        Ok(emitted_lines)
    }

    pub fn fail_session(&mut self, message: impl Into<String>) -> SessionSnapshot {
        self.session.fail(message);
        self.session_snapshot()
    }

    fn reset_live_state(&mut self) {
        self.live_audio_samples.clear();
        self.listening_started_at = None;
        self.last_infer_at = None;
        self.last_emitted_normalized = None;
        self.pending_inference = None;
    }

    fn process_asr_output(
        &mut self,
        output: AsrOutput,
        sample_rate_hz: u32,
    ) -> Result<Option<LiveTranscriptLine>, CoreError> {
        let transcript = single_line(&output.transcript);
        if transcript.is_empty() {
            trim_to_tail(
                &mut self.live_audio_samples,
                sample_rate_hz as usize * RETAIN_TAIL_SECS,
            );
            return Ok(None);
        }

        let rendered = self.translate_transcript(&transcript)?;
        let normalized = normalize_for_dedupe(&rendered);
        if self
            .last_emitted_normalized
            .as_ref()
            .is_some_and(|last| last == &normalized)
        {
            trim_to_tail(
                &mut self.live_audio_samples,
                sample_rate_hz as usize * RETAIN_TAIL_SECS,
            );
            return Ok(None);
        }

        self.last_emitted_normalized = Some(normalized);
        let timestamp_ms = self
            .listening_started_at
            .map(|started| started.elapsed().as_millis() as u64)
            .unwrap_or(0);

        self.transcript
            .add_line(timestamp_ms, SourceIcon::Mic, rendered.clone());
        trim_to_tail(
            &mut self.live_audio_samples,
            sample_rate_hz as usize * RETAIN_TAIL_SECS,
        );

        Ok(Some(LiveTranscriptLine {
            timestamp_ms,
            source: SourceIcon::Mic,
            text: rendered,
        }))
    }

    fn translate_transcript(&self, transcript: &str) -> Result<String, CoreError> {
        if self.source_language == self.target_language {
            return Ok(transcript.to_owned());
        }

        if matches!(self.source_language, Language::Japanese)
            && matches!(self.target_language, Language::English)
        {
            // Whisper already performs direct JA->EN translation in this path.
            return Ok(transcript.to_owned());
        }

        let source = to_translation_language(self.source_language);
        let target = to_translation_language(self.target_language);
        self.translator
            .translate(transcript, source, target)
            .map_err(Into::into)
    }
}

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    let sum = samples
        .iter()
        .fold(0.0f32, |acc, sample| acc + sample * sample);
    (sum / samples.len() as f32).sqrt()
}

fn trim_to_tail(samples: &mut Vec<f32>, keep: usize) {
    if keep == 0 {
        samples.clear();
        return;
    }

    if samples.len() > keep {
        let remove = samples.len() - keep;
        samples.drain(..remove);
    }
}

fn single_line(text: &str) -> String {
    text.replace('\n', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_owned()
}

fn normalize_for_dedupe(text: &str) -> String {
    text.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn validate_language_config(
    _source_language: Language,
    _target_language: Language,
) -> Result<(), CoreError> {
    Ok(())
}

fn language_from_code(code: LanguageCode) -> Language {
    match code {
        LanguageCode::English => Language::English,
        LanguageCode::Japanese => Language::Japanese,
    }
}

fn language_to_code(language: Language) -> LanguageCode {
    match language {
        Language::English => LanguageCode::English,
        Language::Japanese => LanguageCode::Japanese,
    }
}

fn to_translation_language(language: Language) -> TranslationLanguage {
    match language {
        Language::English => TranslationLanguage::English,
        Language::Japanese => TranslationLanguage::Japanese,
    }
}
