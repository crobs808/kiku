use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    English,
    Japanese,
}

#[derive(Debug, Error)]
pub enum TranslationError {
    #[error("translator backend unavailable")]
    BackendUnavailable,
}

pub type TranslationResult<T> = Result<T, TranslationError>;

pub trait Translator: Send + Sync {
    fn translate(
        &self,
        text: &str,
        source: Language,
        target: Language,
    ) -> TranslationResult<String>;
}

#[derive(Debug, Default)]
pub struct StubTranslator;

impl Translator for StubTranslator {
    fn translate(
        &self,
        text: &str,
        source: Language,
        target: Language,
    ) -> TranslationResult<String> {
        if source == target {
            return Ok(text.to_owned());
        }

        match (source, target) {
            (Language::English, Language::Japanese) => Ok(translate_en_to_ja(text)),
            (Language::Japanese, Language::English) => Ok(text.to_owned()),
            _ => Ok(text.to_owned()),
        }
    }
}

fn translate_en_to_ja(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let lower = trimmed.to_lowercase();
    for &(phrase, translated) in phrase_dictionary() {
        if lower == phrase {
            return translated.to_string();
        }
    }

    trimmed
        .split_whitespace()
        .map(translate_token)
        .collect::<Vec<_>>()
        .join(" ")
}

fn translate_token(token: &str) -> String {
    let mut start = 0usize;
    let chars: Vec<char> = token.chars().collect();
    let mut end = chars.len();

    while start < chars.len() && !chars[start].is_alphanumeric() {
        start += 1;
    }
    while end > start && !chars[end - 1].is_alphanumeric() {
        end -= 1;
    }

    let prefix: String = chars[..start].iter().collect();
    let core: String = chars[start..end].iter().collect();
    let suffix: String = chars[end..].iter().collect();

    if core.is_empty() {
        return token.to_owned();
    }

    let translated_core = word_dictionary()
        .iter()
        .find(|(word, _)| core.eq_ignore_ascii_case(word))
        .map(|(_, translated)| *translated)
        .unwrap_or(core.as_str());

    format!("{prefix}{translated_core}{suffix}")
}

fn phrase_dictionary() -> &'static [(&'static str, &'static str)] {
    &[
        ("good morning", "おはようございます"),
        ("good afternoon", "こんにちは"),
        ("good evening", "こんばんは"),
        ("thank you", "ありがとうございます"),
        ("thank you very much", "どうもありがとうございます"),
        ("see you", "またね"),
        ("nice to meet you", "はじめまして"),
        ("excuse me", "すみません"),
    ]
}

fn word_dictionary() -> &'static [(&'static str, &'static str)] {
    &[
        ("hello", "こんにちは"),
        ("hi", "こんにちは"),
        ("yes", "はい"),
        ("no", "いいえ"),
        ("please", "お願いします"),
        ("meeting", "会議"),
        ("project", "プロジェクト"),
        ("timeline", "タイムライン"),
        ("release", "リリース"),
        ("update", "更新"),
        ("review", "確認"),
        ("proposal", "提案"),
        ("team", "チーム"),
        ("everyone", "みなさん"),
        ("schedule", "予定"),
        ("today", "今日"),
        ("tomorrow", "明日"),
        ("important", "重要"),
        ("start", "開始"),
        ("stop", "停止"),
    ]
}
