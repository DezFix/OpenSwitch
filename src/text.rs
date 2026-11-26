// SPDX-License-Identifier: GPL-3.0-or-later
//! Module "Text": small rule-based fixes of what is typed — a capital letter at the start of a
//! sentence, English "I", a word typed with CapsLock on by accident. No dictionaries needed,
//! nothing leaves the computer. The engine decides when to apply them (see `engine.rs`).

use crate::detect::looks_like_code;
use crate::layout::{KeyMap, KeyStroke};

/// Shortest word for the CapsLock fix (пРИВЕТ -> Привет).
pub const CAPS_FIX_MIN: usize = 3;
/// Characters that end a sentence.
pub const SENTENCE_END: [char; 4] = ['.', '!', '?', '…'];
/// Abbreviations with a dot that do not end a sentence (besides single letters: «г.», «т.»,
/// and dotted ones: «т.е.», «e.g.»).
pub const ABBREVIATIONS: &[&str] = &[
    "др", "пр", "см", "стр", "ул", "руб", "коп", "млн", "млрд", "тыс", "им", "напр", "ст", "обл", "гр", "проф", "акад",
    "св", "кв", "рис", "табл", "вул", "див", "грн", "etc", "vs", "mr", "mrs", "ms", "dr", "jr", "sr", "no", "fig",
    "approx",
];

/// Does a word end a sentence? «дела.», «ура!», «правда?» do; «т.», «т.е.», «ул.», «etc.» don't.
pub fn ends_sentence(word: &str) -> bool {
    let Some(head) = word.strip_suffix('.') else {
        return word.ends_with(['!', '?', '…']);
    };
    if head.ends_with('.') {
        return true; // "..." and "?." count as an end
    }
    let lower = head.to_lowercase();
    let letters = lower.chars().filter(|c| c.is_alphabetic()).count();
    !(letters <= 1 || lower.contains('.') || ABBREVIATIONS.contains(&lower.as_str()))
}

/// English words that are always written with a capital I.
pub const ENGLISH_I: [&str; 5] = ["i", "i'm", "i'll", "i've", "i'd"];

/// Word typed with CapsLock on by accident: Shift held for the first letter only, so it came
/// out as пРИВЕТ. All keys must be letters in `map`.
pub fn caps_accident(keys: &[KeyStroke], map: &KeyMap) -> bool {
    keys.len() >= CAPS_FIX_MIN
        && keys.iter().all(|k| k.caps && map.caps_sensitive(k.vk))
        && keys[0].shift
        && keys[1..].iter().all(|k| !k.shift)
}

/// Should this word get a capital first letter? At the start of a sentence, if it is all
/// small letters (not iPhone, eBay, links or code); English "i", "i'm"... always.
pub fn needs_capital(text: &str, sentence_start: bool, latin: bool) -> bool {
    let Some(first) = text.chars().next() else { return false };
    if !first.is_lowercase() {
        return false;
    }
    if latin && ENGLISH_I.contains(&text) {
        return true;
    }
    sentence_start && !text.chars().any(char::is_uppercase) && !(latin && looks_like_code(text))
}

/// Make the first key type a capital letter. False if it cannot (not a letter key).
pub fn capitalize_keys(keys: &mut [KeyStroke], map: &KeyMap) -> bool {
    let Some(k) = keys.first_mut() else { return false };
    let upper = KeyStroke { vk: k.vk, shift: if map.caps_sensitive(k.vk) { !k.caps } else { true }, caps: k.caps };
    if map.render_key(upper).is_some_and(char::is_uppercase) {
        *k = upper;
        true
    } else {
        false
    }
}

/// The same keys as if CapsLock had been off: capital first letter, the rest small.
pub fn without_caps(keys: &[KeyStroke]) -> Vec<KeyStroke> {
    keys.iter().enumerate().map(|(i, k)| KeyStroke { vk: k.vk, shift: i == 0, caps: false }).collect()
}
