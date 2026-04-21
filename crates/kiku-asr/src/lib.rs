use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

const TARGET_SAMPLE_RATE_HZ: u32 = 16_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    English,
    Japanese,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AsrRequest {
    pub source_language: Language,
    pub target_language: Language,
    pub sample_rate_hz: u32,
    pub audio_samples: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AsrOutput {
    pub transcript: String,
    pub confidence: f32,
}

#[derive(Debug, Error)]
pub enum AsrError {
    #[error("asr backend unavailable")]
    BackendUnavailable,
    #[error("asr model unavailable: {0}")]
    ModelUnavailable(String),
    #[error("asr configuration missing: {0}")]
    Configuration(String),
    #[error("asr request failed: {0}")]
    RequestFailed(String),
    #[error("asr response invalid: {0}")]
    InvalidResponse(String),
    #[error("asr inference failed: {0}")]
    InferenceFailed(String),
}

pub type AsrResult<T> = Result<T, AsrError>;

pub trait AsrRuntime: Send + Sync {
    fn infer(&self, request: &AsrRequest) -> AsrResult<AsrOutput>;

    fn uses_network(&self) -> bool {
        false
    }
}

#[derive(Debug, Default)]
pub struct StubAsrRuntime;

impl AsrRuntime for StubAsrRuntime {
    fn infer(&self, request: &AsrRequest) -> AsrResult<AsrOutput> {
        Ok(AsrOutput {
            transcript: format!(
                "stub transcript ({:?} -> {:?}, {} samples @ {} Hz)",
                request.source_language,
                request.target_language,
                request.audio_samples.len(),
                request.sample_rate_hz
            ),
            confidence: 0.25,
        })
    }
}

#[derive(Clone)]
pub struct GoogleCloudAsrRuntime {
    api_key: String,
    client: Client,
}

impl std::fmt::Debug for GoogleCloudAsrRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleCloudAsrRuntime")
            .finish_non_exhaustive()
    }
}

impl GoogleCloudAsrRuntime {
    pub fn new(api_key: impl Into<String>) -> AsrResult<Self> {
        let api_key = api_key.into();
        let trimmed = api_key.trim();
        if trimmed.is_empty() {
            return Err(AsrError::Configuration(
                "KIKU_GOOGLE_SPEECH_API_KEY is empty".to_owned(),
            ));
        }

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|error| AsrError::RequestFailed(error.to_string()))?;

        Ok(Self {
            api_key: trimmed.to_owned(),
            client,
        })
    }
}

impl AsrRuntime for GoogleCloudAsrRuntime {
    fn infer(&self, request: &AsrRequest) -> AsrResult<AsrOutput> {
        if request.audio_samples.is_empty() {
            return Ok(AsrOutput {
                transcript: String::new(),
                confidence: 0.0,
            });
        }

        let pcm = resample_to_target_rate(&request.audio_samples, request.sample_rate_hz);
        if pcm.len() < TARGET_SAMPLE_RATE_HZ as usize / 2 {
            return Ok(AsrOutput {
                transcript: String::new(),
                confidence: 0.0,
            });
        }

        let linear16 = pcm_to_linear16_bytes(&pcm);
        let content = BASE64_STANDARD.encode(linear16);
        let payload = GoogleSpeechRequest {
            config: GoogleSpeechConfig {
                encoding: "LINEAR16",
                sample_rate_hertz: TARGET_SAMPLE_RATE_HZ,
                language_code: google_language_code(request.source_language),
                enable_automatic_punctuation: true,
                model: "latest_long",
            },
            audio: GoogleSpeechAudio { content },
        };
        let url = format!(
            "https://speech.googleapis.com/v1/speech:recognize?key={}",
            self.api_key
        );

        let response = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .map_err(|error| AsrError::RequestFailed(error.to_string()))?;
        let status = response.status();
        let body = response
            .text()
            .map_err(|error| AsrError::RequestFailed(error.to_string()))?;

        if !status.is_success() {
            if let Ok(api_error) = serde_json::from_str::<GoogleSpeechErrorEnvelope>(&body) {
                return Err(AsrError::RequestFailed(format!(
                    "google speech api error {}: {}",
                    status, api_error.error.message
                )));
            }

            return Err(AsrError::RequestFailed(format!(
                "google speech api returned {}: {}",
                status,
                truncate_for_error(&body)
            )));
        }

        let parsed: GoogleSpeechResponse = serde_json::from_str(&body)
            .map_err(|error| AsrError::InvalidResponse(error.to_string()))?;

        let mut transcript_parts = Vec::new();
        let mut confidence = 0.0f32;
        let mut confidence_count = 0usize;

        for result in parsed.results.unwrap_or_default() {
            if let Some(first) = result.alternatives.into_iter().next() {
                let cleaned = first.transcript.trim();
                if !cleaned.is_empty() {
                    transcript_parts.push(cleaned.to_owned());
                }
                if let Some(value) = first.confidence {
                    confidence += value;
                    confidence_count += 1;
                }
            }
        }

        let transcript = transcript_parts.join(" ");
        let avg_confidence = if confidence_count == 0 {
            if transcript.is_empty() {
                0.0
            } else {
                0.65
            }
        } else {
            confidence / confidence_count as f32
        };

        Ok(AsrOutput {
            transcript,
            confidence: avg_confidence,
        })
    }

    fn uses_network(&self) -> bool {
        true
    }
}

#[derive(Clone)]
pub struct WhisperAsrRuntime {
    thread_count: i32,
    context: Arc<Mutex<WhisperContext>>,
}

impl WhisperAsrRuntime {
    pub fn new(model_path: impl AsRef<Path>) -> AsrResult<Self> {
        let resolved = model_path.as_ref();
        if !resolved.exists() {
            return Err(AsrError::ModelUnavailable(format!(
                "model file not found at {}",
                resolved.display()
            )));
        }

        let model = resolved.to_str().ok_or_else(|| {
            AsrError::ModelUnavailable("model path contains invalid UTF-8".to_owned())
        })?;
        let context = WhisperContext::new_with_params(model, WhisperContextParameters::default())
            .map_err(|error| AsrError::InferenceFailed(error.to_string()))?;

        Ok(Self {
            thread_count: 6,
            context: Arc::new(Mutex::new(context)),
        })
    }

    pub fn from_default_model_locations() -> AsrResult<Self> {
        if let Ok(explicit_path) = std::env::var("KIKU_WHISPER_MODEL") {
            return Self::new(explicit_path);
        }

        let candidates = [
            "models/ggml-large-v3.bin",
            "models/ggml-medium.bin",
            "models/ggml-small.bin",
            "models/ggml-base.bin",
            "models/whisper/ggml-large-v3.bin",
            "models/whisper/ggml-medium.bin",
            "models/whisper/ggml-small.bin",
            "models/whisper/ggml-base.bin",
        ];

        if let Some(path) = candidates
            .iter()
            .map(PathBuf::from)
            .find(|path| path.exists())
        {
            return Self::new(path);
        }

        Err(AsrError::ModelUnavailable(format!(
            "no Whisper model found. Set KIKU_WHISPER_MODEL or place one of: {}",
            candidates.join(", ")
        )))
    }
}

impl AsrRuntime for WhisperAsrRuntime {
    fn infer(&self, request: &AsrRequest) -> AsrResult<AsrOutput> {
        if request.audio_samples.is_empty() {
            return Ok(AsrOutput {
                transcript: String::new(),
                confidence: 0.0,
            });
        }

        let pcm = resample_to_target_rate(&request.audio_samples, request.sample_rate_hz);
        if pcm.len() < TARGET_SAMPLE_RATE_HZ as usize / 2 {
            return Ok(AsrOutput {
                transcript: String::new(),
                confidence: 0.0,
            });
        }

        let context = self.context.lock().map_err(|_| {
            AsrError::InferenceFailed("asr runtime context lock poisoned".to_owned())
        })?;
        let mut state = context
            .create_state()
            .map_err(|error| AsrError::InferenceFailed(error.to_string()))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 2 });
        params.set_n_threads(self.thread_count);
        params.set_translate(
            matches!(request.source_language, Language::Japanese)
                && matches!(request.target_language, Language::English),
        );
        // Use Whisper language auto-detection to avoid hard failures when the selected
        // source language does not exactly match the incoming audio.
        params.set_language(None);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);

        state
            .full(params, &pcm)
            .map_err(|error| AsrError::InferenceFailed(error.to_string()))?;

        let segment_count = state
            .full_n_segments()
            .map_err(|error| AsrError::InferenceFailed(error.to_string()))?;
        let mut combined = String::new();
        for segment_idx in 0..segment_count {
            let segment_text = state
                .full_get_segment_text(segment_idx)
                .map_err(|error| AsrError::InferenceFailed(error.to_string()))?;
            if !combined.is_empty() {
                combined.push(' ');
            }
            combined.push_str(segment_text.trim());
        }

        Ok(AsrOutput {
            transcript: combined.trim().to_owned(),
            confidence: if combined.trim().is_empty() {
                0.0
            } else {
                0.82
            },
        })
    }
}

#[derive(Debug, Serialize)]
struct GoogleSpeechRequest {
    config: GoogleSpeechConfig<'static>,
    audio: GoogleSpeechAudio,
}

#[derive(Debug, Serialize)]
struct GoogleSpeechConfig<'a> {
    encoding: &'a str,
    #[serde(rename = "sampleRateHertz")]
    sample_rate_hertz: u32,
    #[serde(rename = "languageCode")]
    language_code: &'a str,
    #[serde(rename = "enableAutomaticPunctuation")]
    enable_automatic_punctuation: bool,
    model: &'a str,
}

#[derive(Debug, Serialize)]
struct GoogleSpeechAudio {
    content: String,
}

#[derive(Debug, Deserialize)]
struct GoogleSpeechResponse {
    results: Option<Vec<GoogleSpeechResult>>,
}

#[derive(Debug, Deserialize)]
struct GoogleSpeechResult {
    alternatives: Vec<GoogleSpeechAlternative>,
}

#[derive(Debug, Deserialize)]
struct GoogleSpeechAlternative {
    transcript: String,
    confidence: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct GoogleSpeechErrorEnvelope {
    error: GoogleSpeechError,
}

#[derive(Debug, Deserialize)]
struct GoogleSpeechError {
    message: String,
}

fn google_language_code(language: Language) -> &'static str {
    match language {
        Language::Japanese => "ja-JP",
        Language::English => "en-US",
    }
}

fn pcm_to_linear16_bytes(samples: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let scaled = if clamped <= -1.0 {
            i16::MIN
        } else {
            (clamped * i16::MAX as f32).round() as i16
        };
        bytes.extend_from_slice(&scaled.to_le_bytes());
    }
    bytes
}

fn truncate_for_error(message: &str) -> String {
    const MAX_LEN: usize = 240;
    let normalized = message.trim().replace('\n', " ");
    if normalized.len() <= MAX_LEN {
        normalized
    } else {
        format!("{}...", &normalized[..MAX_LEN])
    }
}

fn resample_to_target_rate(input: &[f32], source_rate_hz: u32) -> Vec<f32> {
    let src_rate = source_rate_hz.max(1);
    if src_rate == TARGET_SAMPLE_RATE_HZ {
        return input.to_vec();
    }

    if input.len() < 2 {
        return input.to_vec();
    }

    let ratio = TARGET_SAMPLE_RATE_HZ as f64 / src_rate as f64;
    let target_len = ((input.len() as f64) * ratio).round().max(1.0) as usize;
    let mut output = Vec::with_capacity(target_len);

    for idx in 0..target_len {
        let source_pos = idx as f64 / ratio;
        let left = source_pos.floor() as usize;
        let right = (left + 1).min(input.len() - 1);
        let frac = (source_pos - left as f64) as f32;
        let value = input[left] * (1.0 - frac) + input[right] * frac;
        output.push(value);
    }

    output
}
