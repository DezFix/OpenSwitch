//! Wrong-layout detector.
//!
//! For a typed word we render the key strokes in both layouts and compare:
//! 1. dictionary ranks (word frequency lists for EN and RU),
//! 2. a character trigram language model (average log-probability per char).
//!
//! The thresholds below were tuned on frequency-weighted samples of real words
//! (see `examples/eval.rs`).

use crate::layout::{render, KeyStroke, Lang};
use std::collections::{HashMap, HashSet};

static EN_WORDS: &str = include_str!("../data/en_words.txt");
static RU_WORDS: &str = include_str!("../data/ru_words.txt");
static EN_TRI: &[u8] = include_bytes!("../data/en_tri.bin");
static RU_TRI: &[u8] = include_bytes!("../data/ru_tri.bin");

const EN_ALPHA: &str = " abcdefghijklmnopqrstuvwxyz'";
const RU_ALPHA: &str = " абвгдежзийклмнопрстуфхцчшщъыьэюя";

pub struct LangModel {
    lang: Lang,
    ranks: HashMap<&'static str, u32>,
    alpha: Vec<char>,
    tri: &'static [u8],
}

impl LangModel {
    fn new(lang: Lang) -> Self {
        let (words, alpha, tri) = match lang {
            Lang::En => (EN_WORDS, EN_ALPHA, EN_TRI),
            Lang::Ru => (RU_WORDS, RU_ALPHA, RU_TRI),
        };
        let ranks = words
            .lines()
            .filter(|l| !l.is_empty())
            .enumerate()
            .map(|(i, w)| (w, i as u32 + 1))
            .collect();
        LangModel { lang, ranks, alpha: alpha.chars().collect(), tri }
    }

    fn idx(&self, c: char) -> Option<usize> {
        self.alpha.iter().position(|&a| a == c)
    }

    fn logp(&self, a: usize, b: usize, c: usize) -> f32 {
        let n = self.alpha.len();
        let i = (a * n + b) * n + c;
        i16::from_le_bytes([self.tri[i * 2], self.tri[i * 2 + 1]]) as f32 / 100.0
    }

    /// Normalise a rendered word for this language: lowercase, ё->е, strip edge punctuation.
    /// Returns None if the word contains characters that cannot be inside a word of this language.
    pub fn clean(&self, s: &str) -> Option<String> {
        let mut w: String = s
            .chars()
            .flat_map(|c| c.to_lowercase())
            .map(|c| if c == 'ё' { 'е' } else { c })
            .collect();
        let is_letter = |c: char| c != ' ' && c != '\'' && self.alpha.contains(&c);
        let start = w.find(is_letter)?;
        let end = w.rfind(is_letter)? + w[w.rfind(is_letter)?..].chars().next()?.len_utf8();
        w = w[start..end].to_string();
        for c in w.chars() {
            let ok = c == '-' || (c != ' ' && self.alpha.contains(&c));
            if !ok {
                return None;
            }
        }
        Some(w)
    }

    pub fn rank(&self, w: &str) -> Option<u32> {
        if let Some(r) = self.ranks.get(w) {
            return Some(*r);
        }
        // hyphenated words: rank of the worst part
        if w.contains('-') {
            let mut worst = 0;
            for p in w.split('-').filter(|p| !p.is_empty()) {
                worst = worst.max(*self.ranks.get(p)?);
            }
            if worst > 0 {
                return Some(worst);
            }
        }
        None
    }

    /// Average log-probability per character of a cleaned word.
    pub fn score(&self, w: &str) -> f32 {
        let mut total = 0.0;
        let mut count = 0;
        for part in w.split('-').filter(|p| !p.is_empty()) {
            let (mut a, mut b) = (0usize, 0usize);
            for c in part.chars().chain(std::iter::once(' ')) {
                let ci = self.idx(c).unwrap_or(0);
                total += self.logp(a, b, ci);
                count += 1;
                a = b;
                b = ci;
            }
        }
        if count == 0 { -20.0 } else { total / count as f32 }
    }

    pub fn lang(&self) -> Lang {
        self.lang
    }
}

/// Punctuation at the edge of an English rendering that almost never appears there in real text,
/// but is a Russian letter on the same key (х ъ ё б ю ж э).
fn has_edge_junk(raw: &str, cur: Lang) -> bool {
    if cur != Lang::En {
        return false;
    }
    let first = raw.chars().next();
    let last = raw.chars().last();
    let any_edge = |c: char| matches!(c, '[' | ']' | '{' | '}' | '`' | '~' | '<' | '>');
    let lead_only = |c: char| matches!(c, ',' | '.' | ';' | '\'' | ':' | '"');
    let has_letter = raw.chars().any(|c| c.is_ascii_alphabetic());
    has_letter
        && (first.is_some_and(|c| any_edge(c) || lead_only(c)) || last.is_some_and(any_edge))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    pub switch: bool,
    pub current: String,
    pub alternative: String,
    pub reason: &'static str,
}

pub struct Detector {
    en: LangModel,
    ru: LangModel,
    /// Words (in the layout they were typed) the user never wants converted.
    pub never_switch: HashSet<String>,
    /// Higher = more aggressive auto switching. 1.0 = default.
    pub sensitivity: f32,
    /// Convert one-letter words like "d" -> "в".
    pub single_letters: bool,
}

impl Default for Detector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector {
    pub fn new() -> Self {
        Detector {
            en: LangModel::new(Lang::En),
            ru: LangModel::new(Lang::Ru),
            never_switch: HashSet::new(),
            sensitivity: 1.0,
            single_letters: true,
        }
    }

    pub fn model(&self, lang: Lang) -> &LangModel {
        match lang {
            Lang::En => &self.en,
            Lang::Ru => &self.ru,
        }
    }

    /// Decide whether `keys`, typed while layout `cur` was active, were meant for the other layout.
    pub fn check(&self, keys: &[KeyStroke], cur: Lang) -> Verdict {
        let alt = cur.other();
        let raw_cur = render(keys, cur);
        let raw_alt = render(keys, alt);
        let mut v = Verdict { switch: false, current: raw_cur.clone(), alternative: raw_alt.clone(), reason: "" };

        let key = raw_cur.to_lowercase();
        if self.never_switch.contains(&key) {
            v.reason = "user exception";
            return v;
        }
        let mc = self.model(cur);
        let ma = self.model(alt);
        let Some(wa) = ma.clean(&raw_alt) else {
            v.reason = "alt not a word";
            return v;
        };
        let alen = wa.chars().filter(|c| *c != '-').count();
        if alen == 1 && keys.len() == 1 && self.single_letters {
            // One-letter words: "d" -> "в", "z" -> "я", "ш" -> "i" ...
            let c = raw_cur.to_lowercase();
            let hit = match cur {
                Lang::En => matches!(c.as_str(), "d" | "b" | "c" | "z" | "j" | "e" | "f" | "r"),
                Lang::Ru => matches!(c.as_str(), "ш" | "ф"),
            };
            v.switch = hit;
            v.reason = "single letter";
            return v;
        }
        if alen < 2 {
            v.reason = "too short";
            return v;
        }
        // Abbreviations typed in caps (e.g. "USA", "ФСБ") are left alone.
        let letters: Vec<char> = raw_cur.chars().filter(|c| c.is_alphabetic()).collect();
        if letters.len() >= 2 && letters.len() <= 5 && letters.iter().all(|c| c.is_uppercase()) {
            v.reason = "abbreviation";
            return v;
        }

        let ra = ma.rank(&wa);
        let sa = ma.score(&wa);
        let wc = if has_edge_junk(&raw_cur, cur) { None } else { mc.clean(&raw_cur) };
        let s = self.sensitivity;

        let Some(wc) = wc else {
            // Current rendering contains punctuation inside the word (e.g. "j;bl'v").
            if ra.is_some() || sa > -3.2 / s {
                v.switch = true;
                v.reason = "current has junk inside";
            } else {
                v.reason = "both look bad";
            }
            return v;
        };
        let rc = mc.rank(&wc);
        let sc = mc.score(&wc);
        let clen = wc.chars().count();

        // Rank thresholds depend on word length: short words collide a lot between languages.
        let keep_rank = |len: usize| -> u32 {
            match len {
                0..=2 => 400,
                3 => 6_000,
                4 => 25_000,
                _ => u32::MAX,
            }
        };
        if let Some(r) = rc {
            if r <= keep_rank(clen) {
                // Known word in the current language. Switch only if the alternative is a
                // dramatically more common word and the current one is rare and short.
                if let Some(a) = ra {
                    if clen <= 3 && a <= 300 && r > 50 * a && r > 2_000 {
                        v.switch = true;
                        v.reason = "alt much more frequent";
                        return v;
                    }
                }
                v.reason = "current is a known word";
                return v;
            }
        }

        let d = sa - sc;
        match ra {
            Some(a) => {
                let need = match alen {
                    2 => {
                        if a <= 300 && rc.is_none_or(|r| r > 50 * a) { 0.0 } else { f32::INFINITY }
                    }
                    3 => 0.4,
                    _ => 0.0,
                };
                if d * s > need || (alen >= 4 && rc.is_none() && d > -0.3) {
                    v.switch = true;
                    v.reason = "alt is a known word";
                } else {
                    v.reason = "alt known but not convincing";
                }
            }
            None => {
                if alen >= 4 && rc.is_none() && d * s > 1.6 && sc < -2.7 {
                    v.switch = true;
                    v.reason = "language model";
                } else {
                    v.reason = "not convincing";
                }
            }
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::keys_for_text;

    fn sw(d: &Detector, text: &str, lang: Lang) -> bool {
        d.check(&keys_for_text(text, lang).unwrap(), lang).switch
    }

    #[test]
    fn basic() {
        let d = Detector::new();
        for w in ["ghbdtn", "rfr", "ltkf", "yt", "z", "cgfcb,j", "j;blfybt", "Ghbdtn?", ",sk", ";t", "b["] {
            let expect = true;
            assert_eq!(sw(&d, w, Lang::En), expect, "{w}");
        }
        for w in ["руддщ", "цщкдв", "ерфтл", "нщг", "ыщккн"] {
            assert!(sw(&d, w, Lang::Ru), "{w}");
        }
        for w in ["hello", "world", "the", "I", "rust", "is", "Microsoft", "USA", "github"] {
            assert!(!sw(&d, w, Lang::En), "{w}");
        }
        for w in ["привет", "как", "дела", "не", "я", "сеть", "программа", "ФСБ"] {
            assert!(!sw(&d, w, Lang::Ru), "{w}");
        }
    }
}
