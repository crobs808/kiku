use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LanguageCode {
    English,
    Japanese,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LanguagePair {
    pub input: LanguageCode,
    pub output: LanguageCode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WindowSettings {
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub always_on_top: bool,
    pub transparency: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppSettings {
    pub theme_mode: ThemeMode,
    pub font_family: String,
    pub font_size_px: u16,
    pub text_color: String,
    pub visualizer_enabled: bool,
    pub preferred_language_pair: LanguagePair,
    pub streaming_translation_enabled: bool,
    pub mic_enabled_by_default: bool,
    pub system_audio_enabled_by_default: bool,
    pub model_preset: String,
    pub window: WindowSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::System,
            font_family: "IBM Plex Sans".to_owned(),
            font_size_px: 22,
            text_color: "#EAF6FF".to_owned(),
            visualizer_enabled: true,
            preferred_language_pair: LanguagePair {
                input: LanguageCode::Japanese,
                output: LanguageCode::English,
            },
            streaming_translation_enabled: false,
            mic_enabled_by_default: true,
            system_audio_enabled_by_default: false,
            model_preset: "best_accuracy".to_owned(),
            window: WindowSettings {
                width: 1120,
                height: 760,
                x: 80,
                y: 80,
                always_on_top: true,
                transparency: 0.12,
            },
        }
    }
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("settings store lock poisoned")]
    LockPoisoned,
}

pub type SettingsResult<T> = Result<T, SettingsError>;

pub trait SettingsStore: Send + Sync {
    fn load(&self) -> SettingsResult<AppSettings>;
    fn save(&self, settings: &AppSettings) -> SettingsResult<()>;
}

#[derive(Debug, Default)]
pub struct InMemorySettingsStore {
    inner: RwLock<AppSettings>,
}

impl InMemorySettingsStore {
    pub fn new(initial: AppSettings) -> Self {
        Self {
            inner: RwLock::new(initial),
        }
    }
}

impl SettingsStore for InMemorySettingsStore {
    fn load(&self) -> SettingsResult<AppSettings> {
        self.inner
            .read()
            .map(|settings| settings.clone())
            .map_err(|_| SettingsError::LockPoisoned)
    }

    fn save(&self, settings: &AppSettings) -> SettingsResult<()> {
        self.inner
            .write()
            .map(|mut stored| {
                *stored = settings.clone();
            })
            .map_err(|_| SettingsError::LockPoisoned)
    }
}
