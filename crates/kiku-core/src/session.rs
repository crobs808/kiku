use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Idle,
    ModelMissing,
    DownloadingModel,
    Ready,
    Listening,
    Stopping,
    PromptingSaveDiscard,
    SavingTranscript,
    Error,
}

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("invalid session transition from {from:?} to {to:?}")]
    InvalidTransition {
        from: SessionState,
        to: SessionState,
    },
}

#[derive(Debug, Clone)]
pub struct SessionMachine {
    state: SessionState,
    last_error: Option<String>,
}

impl Default for SessionMachine {
    fn default() -> Self {
        Self {
            state: SessionState::Idle,
            last_error: None,
        }
    }
}

impl SessionMachine {
    pub fn state(&self) -> SessionState {
        self.state
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn set_model_missing(&mut self) {
        self.state = SessionState::ModelMissing;
        self.last_error = None;
    }

    pub fn set_ready(&mut self) {
        self.state = SessionState::Ready;
        self.last_error = None;
    }

    pub fn begin_model_download(&mut self) -> Result<(), SessionError> {
        self.transition(
            &[SessionState::ModelMissing, SessionState::Ready],
            SessionState::DownloadingModel,
        )
    }

    pub fn finish_model_download(&mut self) -> Result<(), SessionError> {
        self.transition(&[SessionState::DownloadingModel], SessionState::Ready)
    }

    pub fn start_listening(&mut self) -> Result<(), SessionError> {
        self.transition(&[SessionState::Ready], SessionState::Listening)
    }

    pub fn stop_listening(&mut self) -> Result<(), SessionError> {
        self.transition(&[SessionState::Listening], SessionState::Stopping)
    }

    pub fn prompt_save_discard(&mut self) -> Result<(), SessionError> {
        self.transition(
            &[SessionState::Stopping],
            SessionState::PromptingSaveDiscard,
        )
    }

    pub fn begin_save_transcript(&mut self) -> Result<(), SessionError> {
        self.transition(
            &[SessionState::PromptingSaveDiscard],
            SessionState::SavingTranscript,
        )
    }

    pub fn finish_save_transcript(&mut self) -> Result<(), SessionError> {
        self.transition(&[SessionState::SavingTranscript], SessionState::Ready)
    }

    pub fn discard_transcript(&mut self) -> Result<(), SessionError> {
        self.transition(&[SessionState::PromptingSaveDiscard], SessionState::Ready)
    }

    pub fn fail(&mut self, message: impl Into<String>) {
        self.state = SessionState::Error;
        self.last_error = Some(message.into());
    }

    pub fn recover_to_ready(&mut self) {
        self.state = SessionState::Ready;
        self.last_error = None;
    }

    fn transition(
        &mut self,
        allowed: &[SessionState],
        next: SessionState,
    ) -> Result<(), SessionError> {
        if !allowed.contains(&self.state) {
            return Err(SessionError::InvalidTransition {
                from: self.state,
                to: next,
            });
        }

        self.state = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{SessionMachine, SessionState};

    #[test]
    fn allows_nominal_session_lifecycle() {
        let mut machine = SessionMachine::default();

        machine.set_ready();
        machine.start_listening().unwrap();
        machine.stop_listening().unwrap();
        machine.prompt_save_discard().unwrap();
        machine.discard_transcript().unwrap();

        assert_eq!(machine.state(), SessionState::Ready);
    }
}
