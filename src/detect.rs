// SPDX-License-Identifier: GPL-3.0-or-later
//! Wrong-layout detector.
//!
//! For a typed word we render the key strokes in both layouts and compare:
//! 1. dictionary ranks (word frequency lists for EN and RU),
//! 2. a character trigram language model (average log-probability per char).
//!
//! The thresholds below were tuned on frequency-weighted samples of real words
//! (see `examples/eval.rs`).

use crate::layout::{KeyStroke, Lang, Layouts};
use std::collections::{HashMap, HashSet};

static EN_WORDS: &str = include_str!("../data/en_words.txt");
static RU_WORDS: &str = include_str!("../data/ru_words.txt");
static UK_WORDS: &str = include_str!("../data/uk_words.txt");
static EN_TRI: &[u8] = include_bytes!("../data/en_tri.bin");
static RU_TRI: &[u8] = include_bytes!("../data/ru_tri.bin");
static UK_TRI: &[u8] = include_bytes!("../data/uk_tri.bin");

const EN_ALPHA: &str = " abcdefghijklmnopqrstuvwxyz'";
const RU_ALPHA: &str = " абвгдежзийклмнопрстуфхцчшщъыьэюя";
const UK_ALPHA: &str = " абвгґдеєжзиіїйклмнопрстуфхцчшщьюя'";

/// A language with its own dictionary and character model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    En,
    Ru,
    Uk,
}

pub struct LangModel {
    model: Model,
    ranks: HashMap<&'static str, u32>,
    alpha: Vec<char>,
    tri: &'static [u8],
}

impl LangModel {
    pub fn new(model: Model) -> Self {
        let (words, alpha, tri) = match model {
            Model::En => (EN_WORDS, EN_ALPHA, EN_TRI),
            Model::Ru => (RU_WORDS, RU_ALPHA, RU_TRI),
            Model::Uk => (UK_WORDS, UK_ALPHA, UK_TRI),
        };
        let ranks = words.lines().filter(|l| !l.is_empty()).enumerate().map(|(i, w)| (w, i as u32 + 1)).collect();
        LangModel { model, ranks, alpha: alpha.chars().collect(), tri }
    }

    fn idx(&self, c: char) -> Option<usize> {
        self.alpha.iter().position(|&a| a == c)
    }

    fn logp(&self, a: usize, b: usize, c: usize) -> f32 {
        let n = self.alpha.len();
        let i = (a * n + b) * n + c;
        i16::from_le_bytes([self.tri[i * 2], self.tri[i * 2 + 1]]) as f32 / 100.0
    }

    /// Normalise a rendered word for this language: lowercase, ё->е (Russian), ’->' (Ukrainian),
    /// strip edge punctuation. Returns None if the word contains characters that cannot be
    /// inside a word of this language.
    pub fn clean(&self, s: &str) -> Option<String> {
        let ru = self.model == Model::Ru;
        let mut w: String = s
            .chars()
            .flat_map(|c| c.to_lowercase())
            .map(|c| match c {
                'ё' if ru => 'е',
                '’' | 'ʼ' => '\'',
                c => c,
            })
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
        if count == 0 {
            -20.0
        } else {
            total / count as f32
        }
    }

    pub fn model(&self) -> Model {
        self.model
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
    has_letter && (first.is_some_and(|c| any_edge(c) || lead_only(c)) || last.is_some_and(any_edge))
}

/// File extensions and top-level domains: `name.ext` / `site.tld` typed in the English layout
/// is a file or a link, not Russian text with «ю» in it.
const CODE_SUFFIXES: &[&str] = &[
    "com", "net", "org", "ru", "ua", "by", "kz", "io", "dev", "app", "me", "co", "uk", "de", "eu", "info", "gov",
    "edu", "rs", "py", "js", "ts", "md", "txt", "exe", "dll", "html", "htm", "css", "json", "xml", "yml", "yaml",
    "toml", "ini", "cfg", "log", "pdf", "doc", "docx", "xls", "xlsx", "png", "jpg", "jpeg", "gif", "svg", "zip", "rar",
    "mp3", "mp4", "csv", "sh", "bat", "cpp", "go", "java", "php", "sql",
];

/// Does a word typed in the English layout look like a link, an e-mail/file name or a code
/// identifier? Those are left alone (`skip_code`).
pub fn looks_like_code(raw: &str) -> bool {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("://") || lower.starts_with("www.") {
        return true;
    }
    // name.ext / site.tld (at least two characters before the dot)
    if let Some((head, ext)) = lower.rsplit_once('.') {
        let head_ok = head.chars().filter(|c| c.is_ascii_alphanumeric()).count() >= 2
            && head.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '/'));
        if head_ok && CODE_SUFFIXES.contains(&ext) {
            return true;
        }
    }
    // camelCase: a capital after a small letter, followed by more small letters (getValue).
    // "gHBDTN" (CapsLock by accident) has no small letters after the capitals.
    let c: Vec<char> = raw.chars().collect();
    (1..c.len()).any(|i| {
        c[i - 1].is_ascii_lowercase() && c[i].is_ascii_uppercase() && c[i + 1..].iter().any(char::is_ascii_lowercase)
    })
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
    /// Ukrainian, loaded only when a Ukrainian layout is in use (see `set_ukrainian`).
    uk: Option<LangModel>,
    /// Words (in the layout they were typed) the user never wants converted.
    pub never_switch: HashSet<String>,
    /// Higher = more aggressive auto switching. 1.0 = default.
    pub sensitivity: f32,
    /// Convert one-letter words like "d" -> "в".
    pub single_letters: bool,
    /// Leave links, file names and camelCase identifiers typed in English alone.
    pub skip_code: bool,
    /// Key -> character maps of the two layouts.
    pub layouts: Layouts,
}

impl Default for Detector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector {
    pub fn new() -> Self {
        Detector {
            en: LangModel::new(Model::En),
            ru: LangModel::new(Model::Ru),
            uk: None,
            never_switch: HashSet::new(),
            sensitivity: 1.0,
            single_letters: true,
            skip_code: true,
            layouts: Layouts::default(),
        }
    }

    /// The model for a side of the pair (the Cyrillic side is Russian here).
    pub fn model(&self, lang: Lang) -> &LangModel {
        match lang {
            Lang::En => &self.en,
            Lang::Ru => &self.ru,
        }
    }

    /// Also treat the Cyrillic side as Ukrainian (load the Ukrainian dictionary).
    pub fn set_ukrainian(&mut self, on: bool) {
        match (on, self.uk.is_some()) {
            (true, false) => self.uk = Some(LangModel::new(Model::Uk)),
            (false, true) => self.uk = None,
            _ => {}
        }
    }

    pub fn ukrainian(&self) -> bool {
        self.uk.is_some()
    }

    /// Decide whether `keys`, typed while layout `cur` was active, were meant for the other layout.
    ///
    /// With Ukrainian enabled, the Cyrillic side is checked as Russian and as Ukrainian:
    /// a Cyrillic word is kept if it is plausible in either language, and English typing is
    /// converted if the result is a word in either language.
    pub fn check(&self, keys: &[KeyStroke], cur: Lang) -> Verdict {
        self.check_careful(keys, cur, 1.0)
    }

    /// Like `check`, with the sensitivity multiplied by `care` (< 1 = switch less readily,
    /// e.g. right after the user switched the layout on purpose).
    pub fn check_careful(&self, keys: &[KeyStroke], cur: Lang, care: f32) -> Verdict {
        let ru = self.check_with(keys, cur, &self.ru, care);
        let Some(uk) = &self.uk else { return ru };
        let uk = self.check_with(keys, cur, uk, care);
        match cur {
            Lang::Ru if ru.switch && !uk.switch => uk,
            Lang::Ru => ru,
            Lang::En if !ru.switch && uk.switch => uk,
            Lang::En => ru,
        }
    }

    fn check_with(&self, keys: &[KeyStroke], cur: Lang, cyr: &LangModel, care: f32) -> Verdict {
        let model = |l: Lang| match l {
            Lang::En => &self.en,
            Lang::Ru => cyr,
        };
        let alt = cur.other();
        let raw_cur = self.layouts.render(keys, cur);
        let raw_alt = self.layouts.render(keys, alt);
        let mut v = Verdict { switch: false, current: raw_cur.clone(), alternative: raw_alt.clone(), reason: "" };

        let key = raw_cur.to_lowercase();
        if self.never_switch.contains(&key) {
            v.reason = "user exception";
            return v;
        }
        if self.skip_code && cur == Lang::En && looks_like_code(&raw_cur) {
            v.reason = "link or code";
            return v;
        }
        let mc = model(cur);
        let ma = model(alt);
        let Some(wa) = ma.clean(&raw_alt) else {
            v.reason = "alt not a word";
            return v;
        };
        let alen = wa.chars().filter(|c| *c != '-').count();
        if alen == 1 && keys.len() == 1 && self.single_letters {
            // One-letter words: "d" -> "в", "z" -> "я", "ш" -> "i" ... Decided by the letter it
            // would become, so it also works with customised layouts.
            let hit = match (cur, cyr.model) {
                (Lang::En, Model::Uk) => matches!(wa.as_str(), "в" | "і" | "с" | "я" | "о" | "у" | "а" | "з" | "й"),
                (Lang::En, _) => matches!(wa.as_str(), "в" | "и" | "с" | "я" | "о" | "у" | "а" | "к"),
                (Lang::Ru, _) => matches!(wa.as_str(), "i" | "a"),
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
        let s = self.sensitivity * care;

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
                        if a <= 300 && rc.is_none_or(|r| r > 50 * a) {
                            0.0
                        } else {
                            f32::INFINITY
                        }
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

    #[test]
    fn links_and_code() {
        let d = Detector::new();
        for w in ["github.com", "https://example", "www.google", "readme.md", "main.rs", "getValue", "isEmpty"] {
            assert!(looks_like_code(w), "{w}");
            assert!(!sw(&d, w, Lang::En), "{w}");
        }
        // Russian words with «ю», «б», «ж» typed in English are still fixed
        for w in ["k.,jdm", "c.lf", "rk.d", "gk.c", "ghbdtn", "gHBDTN", "Ghbdtn"] {
            assert!(!looks_like_code(w), "{w}");
        }
        assert!(sw(&d, "c.lf", Lang::En));
    }

    #[test]
    fn code_rule_spares_russian_words() {
        // No common Russian word may look like code when typed in the English layout.
        let d = Detector::new();
        let hits: Vec<&str> = RU_WORDS
            .lines()
            .take(30_000)
            .filter(|w| {
                let keys = keys_for_text(w, Lang::Ru);
                keys.is_some_and(|k| looks_like_code(&d.layouts.render(&k, Lang::En)))
            })
            .collect();
        assert!(hits.is_empty(), "{hits:?}");
        let uk = ukrainian();
        let hits: Vec<&str> = UK_WORDS
            .lines()
            .take(30_000)
            .filter(|w| {
                let keys = uk.layouts.keys_for_text(w, Lang::Ru);
                keys.is_some_and(|k| looks_like_code(&uk.layouts.render(&k, Lang::En)))
            })
            .collect();
        assert!(hits.is_empty(), "{hits:?}");
    }

    #[test]
    fn careful_check() {
        let d = Detector::new();
        let keys = keys_for_text("ghbdtn", Lang::En).unwrap();
        // a clear case is fixed even when careful
        assert!(d.check_careful(&keys, Lang::En, 0.75).switch);
    }

    fn ukrainian() -> Detector {
        use crate::layout::KeyMap;
        let mut d = Detector::new();
        d.set_ukrainian(true);
        let mut ru = KeyMap::builtin(Lang::Ru);
        ru.set(b'S', Some('і'), Some('І'), true);
        ru.set(0xDD, Some('ї'), Some('Ї'), true);
        ru.set(0xDE, Some('є'), Some('Є'), true);
        ru.set(0xC0, Some('\''), Some('₴'), false);
        d.layouts = Layouts { en: KeyMap::builtin(Lang::En), ru };
        d
    }

    fn sw_l(d: &Detector, text: &str, lang: Lang) -> bool {
        d.check(&d.layouts.keys_for_text(text, lang).unwrap(), lang).switch
    }

    #[test]
    fn ukrainian_layout() {
        let d = ukrainian();
        // English typed in the Ukrainian layout -> English
        for w in ["дщму", "руддщ", "цщкдв", "еуіе"] {
            assert!(sw_l(&d, w, Lang::Ru), "{w}");
        }
        // Ukrainian typed in the English layout -> Ukrainian
        for w in ["ghbdsn", "vscnj", "ldsxf", "s"] {
            assert!(sw_l(&d, w, Lang::En), "{w}");
        }
        // Real Ukrainian and Russian words stay as they are
        for w in ["від", "рік", "між", "привіт", "їжак", "інтерв'ю", "також", "привет", "как"]
        {
            assert!(!sw_l(&d, w, Lang::Ru), "{w}");
        }
        for w in ["hello", "the", "github", "love"] {
            assert!(!sw_l(&d, w, Lang::En), "{w}");
        }
    }
}
