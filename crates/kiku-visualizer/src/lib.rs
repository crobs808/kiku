use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VisualizerStage {
    Listen,
    Understand,
    Translate,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct VisualizerFrame {
    pub stage: VisualizerStage,
    pub audio_level: f32,
    pub stability: f32,
}

pub trait VisualizerFeed: Send + Sync {
    fn frame(&self) -> VisualizerFrame;
}

#[derive(Debug, Default)]
pub struct StubVisualizer;

impl VisualizerFeed for StubVisualizer {
    fn frame(&self) -> VisualizerFrame {
        VisualizerFrame {
            stage: VisualizerStage::Listen,
            audio_level: 0.0,
            stability: 1.0,
        }
    }
}
