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
            (Language::Japanese, Language::English) => Ok(translate_ja_to_en(text)),
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

fn translate_ja_to_en(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    for &(phrase, translated) in ja_to_en_phrase_dictionary() {
        if trimmed == phrase {
            return translated.to_string();
        }
    }

    let mut rendered = trimmed.to_string();
    for &(phrase, translated) in ja_to_en_phrase_dictionary() {
        if rendered.contains(phrase) {
            rendered = rendered.replace(phrase, translated);
        }
    }

    let rendered = replace_remaining_japanese_spans(&rendered);
    let normalized = rendered.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        "untranslated".to_owned()
    } else {
        normalized
    }
}

fn ja_to_en_phrase_dictionary() -> &'static [(&'static str, &'static str)] {
    &[
        ("おはようございます", "good morning"),
        ("おはよう", "morning"),
        ("こんにちは", "hello"),
        ("こんばんは", "good evening"),
        ("はじめまして", "nice to meet you"),
        ("よろしくお願いします", "thank you in advance"),
        ("ありがとうございます", "thank you"),
        ("ありがとう", "thank you"),
        ("うん", "yeah"),
        ("すみません", "excuse me"),
        ("ごめん", "sorry"),
        ("ごめんなさい", "sorry"),
        ("あれ", "that"),
        ("はい", "yes"),
        ("いいえ", "no"),
        ("いい", "good"),
        ("会議", "meeting"),
        ("プロジェクト", "project"),
        ("タイムライン", "timeline"),
        ("リリース", "release"),
        ("更新", "update"),
        ("確認", "review"),
        ("提案", "proposal"),
        ("チーム", "team"),
        ("みなさん", "everyone"),
        ("予定", "schedule"),
        ("今日", "today"),
        ("明日", "tomorrow"),
        ("重要", "important"),
        ("開始", "start"),
        ("停止", "stop"),
    ]
}

fn replace_remaining_japanese_spans(input: &str) -> String {
    let mut rendered = String::new();
    let mut japanese_span = String::new();

    for ch in input.chars() {
        if is_japanese_script(ch) {
            japanese_span.push(ch);
            continue;
        }

        if !japanese_span.is_empty() {
            rendered.push_str(map_japanese_span(&japanese_span));
            japanese_span.clear();
        }
        rendered.push(ch);
    }

    if !japanese_span.is_empty() {
        rendered.push_str(map_japanese_span(&japanese_span));
    }

    rendered
}

fn map_japanese_span(span: &str) -> &'static str {
    ja_to_en_phrase_dictionary()
        .iter()
        .find(|(phrase, _)| *phrase == span)
        .map_or("untranslated", |(_, translated)| *translated)
}

fn is_japanese_script(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3040..=0x309F // Hiragana
            | 0x30A0..=0x30FF // Katakana
            | 0x31F0..=0x31FF // Katakana Phonetic Extensions
            | 0x4E00..=0x9FFF // CJK Unified Ideographs
            | 0x3400..=0x4DBF // CJK Unified Ideographs Extension A
    )
}
