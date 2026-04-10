use std::sync::atomic::{AtomicBool, Ordering};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PrivacyError {
    #[error("offline mode is already active")]
    AlreadyOffline,
    #[error("offline mode is not active")]
    NotOffline,
}

pub type PrivacyResult<T> = Result<T, PrivacyError>;

pub trait PrivacyGuard: Send + Sync {
    fn enter_offline_mode(&self) -> PrivacyResult<()>;
    fn exit_offline_mode(&self) -> PrivacyResult<()>;
    fn offline_mode_active(&self) -> bool;
}

#[derive(Debug, Default)]
pub struct InMemoryPrivacyGuard {
    offline_mode: AtomicBool,
}

impl PrivacyGuard for InMemoryPrivacyGuard {
    fn enter_offline_mode(&self) -> PrivacyResult<()> {
        match self
            .offline_mode
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        {
            Ok(_) => Ok(()),
            Err(_) => Err(PrivacyError::AlreadyOffline),
        }
    }

    fn exit_offline_mode(&self) -> PrivacyResult<()> {
        match self
            .offline_mode
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
        {
            Ok(_) => Ok(()),
            Err(_) => Err(PrivacyError::NotOffline),
        }
    }

    fn offline_mode_active(&self) -> bool {
        self.offline_mode.load(Ordering::SeqCst)
    }
}
