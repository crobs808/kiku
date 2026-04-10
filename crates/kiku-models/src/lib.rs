use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelPreset {
    BestAccuracy,
    Balanced,
    ExperimentalJapanese,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelStatus {
    pub installed: bool,
    pub active_preset: ModelPreset,
}

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("model manager lock poisoned")]
    LockPoisoned,
}

pub type ModelResult<T> = Result<T, ModelError>;

pub trait ModelManager: Send + Sync {
    fn status(&self) -> ModelResult<ModelStatus>;
    fn ensure_installed(&self, preset: ModelPreset) -> ModelResult<()>;
    fn set_active_preset(&self, preset: ModelPreset) -> ModelResult<()>;
}

#[derive(Debug)]
pub struct InMemoryModelManager {
    inner: Mutex<ModelStatus>,
}

impl Default for InMemoryModelManager {
    fn default() -> Self {
        Self::new(false)
    }
}

impl InMemoryModelManager {
    pub fn new(installed: bool) -> Self {
        Self {
            inner: Mutex::new(ModelStatus {
                installed,
                active_preset: ModelPreset::BestAccuracy,
            }),
        }
    }
}

impl ModelManager for InMemoryModelManager {
    fn status(&self) -> ModelResult<ModelStatus> {
        self.inner
            .lock()
            .map(|status| *status)
            .map_err(|_| ModelError::LockPoisoned)
    }

    fn ensure_installed(&self, preset: ModelPreset) -> ModelResult<()> {
        self.inner
            .lock()
            .map(|mut status| {
                status.installed = true;
                status.active_preset = preset;
            })
            .map_err(|_| ModelError::LockPoisoned)
    }

    fn set_active_preset(&self, preset: ModelPreset) -> ModelResult<()> {
        self.inner
            .lock()
            .map(|mut status| {
                status.active_preset = preset;
            })
            .map_err(|_| ModelError::LockPoisoned)
    }
}
