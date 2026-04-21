use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceIcon {
    Mic,
    SystemAudio,
    Mixed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TranscriptLine {
    pub timestamp_ms: u64,
    pub source: SourceIcon,
    pub text: String,
}

#[derive(Debug, Default, Clone)]
pub struct TranscriptBuffer {
    lines: Vec<TranscriptLine>,
}

impl TranscriptBuffer {
    pub fn add_line(&mut self, timestamp_ms: u64, source: SourceIcon, text: impl Into<String>) {
        self.lines.push(TranscriptLine {
            timestamp_ms,
            source,
            text: text.into(),
        });
    }

    pub fn replace_last_line(
        &mut self,
        timestamp_ms: u64,
        source: SourceIcon,
        text: impl Into<String>,
    ) -> bool {
        if let Some(last) = self.lines.last_mut() {
            last.timestamp_ms = timestamp_ms;
            last.source = source;
            last.text = text.into();
            true
        } else {
            false
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn export_plain_text(&self) -> String {
        self.lines
            .iter()
            .map(|line| {
                format!(
                    "[{}] [{}] {}",
                    format_timestamp(line.timestamp_ms),
                    line.source.as_str(),
                    line.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl SourceIcon {
    fn as_str(&self) -> &'static str {
        match self {
            SourceIcon::Mic => "mic",
            SourceIcon::SystemAudio => "sys",
            SourceIcon::Mixed => "mix",
        }
    }
}

fn format_timestamp(ms: u64) -> String {
    let total_seconds = ms / 1000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

#[cfg(test)]
mod tests {
    use super::{SourceIcon, TranscriptBuffer};

    #[test]
    fn exports_plain_text_lines() {
        let mut buffer = TranscriptBuffer::default();
        buffer.add_line(3_100, SourceIcon::Mic, "Good morning everyone.");

        let exported = buffer.export_plain_text();
        assert_eq!(exported, "[00:00:03] [mic] Good morning everyone.");
    }
}
