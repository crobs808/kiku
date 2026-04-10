use kiku_platform::CaptureSource;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioFrame {
    pub source: CaptureSource,
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub frame_ms: u16,
    pub data: Vec<f32>,
}

pub trait AudioChunker {
    fn push_frame(&mut self, frame: AudioFrame);
    fn drain_chunk(&mut self) -> Option<Vec<AudioFrame>>;
}

#[derive(Debug, Default)]
pub struct PassthroughChunker {
    pending: Vec<AudioFrame>,
}

impl AudioChunker for PassthroughChunker {
    fn push_frame(&mut self, frame: AudioFrame) {
        self.pending.push(frame);
    }

    fn drain_chunk(&mut self) -> Option<Vec<AudioFrame>> {
        if self.pending.is_empty() {
            return None;
        }
        Some(std::mem::take(&mut self.pending))
    }
}
