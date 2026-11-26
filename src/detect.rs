// SPDX-License-Identifier: GPL-3.0-or-later
//! Wrong-layout detector.
//!
//! For a typed word we render the key strokes in the current layout and in every other one,
//! and compare each pair:
//! 1. dictionary ranks (word frequency lists for EN, RU and UK),
//! 2. a character trigram language model (average log-probability per char).
//!
//! Of the layouts that win against the current one, the most convincing is chosen.
//!
//! The thresholds below were tuned on frequency-weighted samples of real words
//! (see `examples/eval.rs`).

use crate::layout::{KeyStroke, Lang, Language, Layouts};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

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
pub use crate::layout::Language as Model;

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
fn has_edge_junk(raw: &str, cur: Language) -> bool {
    if !cur.is_latin() {
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
    /// The layout to switch to (the current one if `switch` is false).
    pub to: Lang,
    pub current: String,
    pub alternative: String,
    pub reason: &'static str,
    /// Dictionary rank and model score of the alternative (to choose between layouts).
    rank: Option<u32>,
    score: f32,
}

impl Verdict {
    fn keep(cur: Lang, current: String, reason: &'static str) -> Verdict {
        Verdict { switch: false, to: cur, alternative: current.clone(), current, reason, rank: None, score: -20.0 }
    }
}

pub struct Detector {
    /// Language models, loaded the first time a layout needs them.
    models: [OnceLock<LangModel>; 3],
    /// Words (in the layout they were typed) the user never wants converted.
    pub never_switch: HashSet<String>,
    /// Words (in the layout they were typed) that are always converted, with the code of the
    /// layout to convert to if known: learned from manual fixes. `never_switch` wins.
    pub always_switch: HashMap<String, Option<String>>,
    /// Higher = more aggressive auto switching. 1.0 = default.
    pub sensitivity: f32,
    /// Convert one-letter words like "d" -> "в".
    pub single_letters: bool,
    /// Leave links, file names and camelCase identifiers typed in English alone.
    pub skip_code: bool,
    /// The layouts taking part, with their key maps and languages.
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
            models: Default::default(),
            never_switch: HashSet::new(),
            always_switch: HashMap::new(),
            sensitivity: 1.0,
            single_letters: true,
            skip_code: true,
            layouts: Layouts::default(),
        }
    }

    /// The dictionary and character model of a language (loaded on first use).
    pub fn model(&self, lang: Language) -> &LangModel {
        let i = match lang {
            Language::En => 0,
            Language::Ru => 1,
            Language::Uk => 2,
        };
        self.models[i].get_or_init(|| LangModel::new(lang))
    }

    /// Decide whether `keys`, typed while layout `cur` was active, were meant for another
    /// layout, and for which one.
    ///
    /// Each other layout is compared with the current one. A layout may stand for several
    /// languages (Ukrainian also counts as Russian when no Russian layout is installed): the
    /// word is kept if it is plausible in any language of the current layout, and switched if
    /// it is a word in any language of the other layout. Layouts that share a language with
    /// the current one (two English layouts, say) are not compared.
    pub fn check(&self, keys: &[KeyStroke], cur: Lang) -> Verdict {
        self.check_careful(keys, cur, 1.0)
    }

    /// Like `check`, with the sensitivity multiplied by `care` (< 1 = switch less readily,
    /// e.g. right after the user switched the layout on purpose).
    pub fn check_careful(&self, keys: &[KeyStroke], cur: Lang, care: f32) -> Verdict {
        let raw_cur = self.layouts.render(keys, cur);
        let key = raw_cur.to_lowercase();
        if self.never_switch.contains(&key) {
            return Verdict::keep(cur, raw_cur, "user exception");
        }
        if let Some(code) = self.always_switch.get(&key) {
            let by_code = self.layouts.all().find(|&l| l != cur && code.as_deref() == Some(self.layouts.code(l)));
            if let Some(to) = by_code.or_else(|| self.ranked_alternatives(keys, cur).first().copied()) {
                let mut v = Verdict::keep(cur, raw_cur, "learned from a manual fix");
                v.switch = true;
                v.to = to;
                v.alternative = self.layouts.render(keys, to);
                return v;
            }
        }
        let mut best: Option<Verdict> = None;
        let mut kept: Option<Verdict> = None;
        let cur_langs = self.layouts.languages(cur);
        for alt in self.layouts.all() {
            let Some(layout) = self.layouts.get(alt) else { continue };
            if alt == cur || !layout.target || layout.languages.iter().any(|l| cur_langs.contains(l)) {
                continue;
            }
            let v = self.check_pair(keys, cur, alt, care);
            if !v.switch {
                kept.get_or_insert(v);
            } else if best.as_ref().is_none_or(|b| self.better(&v, b)) {
                best = Some(v);
            }
        }
        best.or(kept).unwrap_or_else(|| Verdict::keep(cur, raw_cur, "no other layout"))
    }

    /// Is switching to `a` more convincing than to `b`? The same text in two layouts (Russian
    /// and Ukrainian «так») goes to the one used last; otherwise (and for different texts) a
    /// known word beats an unknown one, a more frequent word a rarer one, then the character
    /// model decides.
    fn better(&self, a: &Verdict, b: &Verdict) -> bool {
        if a.alternative.to_lowercase() == b.alternative.to_lowercase() {
            let recent = |l: Lang| self.layouts.recency.get(l.index()).copied().unwrap_or(0);
            if recent(a.to) != recent(b.to) {
                return recent(a.to) > recent(b.to);
            }
        }
        match (a.rank, b.rank) {
            (Some(x), Some(y)) => x < y,
            (Some(_), None) => true,
            (None, Some(_)) => false,
            (None, None) => a.score > b.score,
        }
    }

    /// One pair of layouts: switch if, for every language of the current layout, some
    /// language of `alt` wins.
    fn check_pair(&self, keys: &[KeyStroke], cur: Lang, alt: Lang, care: f32) -> Verdict {
        let mut switch: Option<Verdict> = None;
        for &mc in self.layouts.languages(cur) {
            let mut found: Option<Verdict> = None;
            let mut keep: Option<Verdict> = None;
            for &ma in self.layouts.languages(alt) {
                let v = self.check_with(keys, cur, mc, alt, ma, care);
                if !v.switch {
                    keep.get_or_insert(v);
                } else if found.as_ref().is_none_or(|f| self.better(&v, f)) {
                    found = Some(v);
                }
            }
            match found {
                Some(v) => {
                    if switch.as_ref().is_none_or(|s| self.better(&v, s)) {
                        switch = Some(v);
                    }
                }
                None => return keep.unwrap_or_else(|| Verdict::keep(cur, self.layouts.render(keys, cur), "")),
            }
        }
        switch.unwrap_or_else(|| Verdict::keep(cur, self.layouts.render(keys, cur), "no language"))
    }

    /// The other layouts in order of how plausible the word is in them (for manual fixes):
    /// known words first, by frequency, then by the character model. Layouts that give the
    /// same text as the current one are left out.
    pub fn ranked_alternatives(&self, keys: &[KeyStroke], cur: Lang) -> Vec<Lang> {
        let raw_cur = self.layouts.render(keys, cur).to_lowercase();
        let mut out: Vec<(Lang, Option<u32>, f32, String)> = Vec::new();
        for alt in self.layouts.all().filter(|&l| l != cur) {
            let raw = self.layouts.render(keys, alt);
            if raw.to_lowercase() == raw_cur {
                continue;
            }
            let mut rank: Option<u32> = None;
            let mut score = -20.0f32;
            for &lang in self.layouts.languages(alt) {
                let m = self.model(lang);
                if let Some(w) = m.clean(&raw) {
                    if let Some(r) = m.rank(&w) {
                        rank = Some(rank.map_or(r, |x| x.min(r)));
                    }
                    score = score.max(m.score(&w));
                }
            }
            out.push((alt, rank, score, raw.to_lowercase()));
        }
        let pos = |l: Lang| self.layouts.recency.get(l.index()).copied().unwrap_or(0);
        out.sort_by(|a, b| {
            let by_rank = match (a.1, b.1) {
                (Some(x), Some(y)) => x.cmp(&y),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => b.2.total_cmp(&a.2),
            };
            if a.3 == b.3 {
                pos(b.0).cmp(&pos(a.0))
            } else {
                by_rank
            }
        });
        // Same text in two layouts (Russian and Ukrainian «руку»): keep the first only.
        let mut seen = HashSet::new();
        out.into_iter().filter(|x| seen.insert(x.3.clone())).map(|x| x.0).collect()
    }

    fn check_with(
        &self,
        keys: &[KeyStroke],
        cur: Lang,
        cur_lang: Language,
        alt: Lang,
        alt_lang: Language,
        care: f32,
    ) -> Verdict {
        let raw_cur = self.layouts.render(keys, cur);
        let raw_alt = self.layouts.render(keys, alt);
        let mut v = Verdict::keep(cur, raw_cur.clone(), "");
        v.alternative = raw_alt.clone();
        v.to = alt;

        if raw_alt.to_lowercase() == raw_cur.to_lowercase() {
            v.reason = "same text";
            return v;
        }
        if self.skip_code && cur_lang.is_latin() && looks_like_code(&raw_cur) {
            v.reason = "link or code";
            return v;
        }
        let mc = self.model(cur_lang);
        let ma = self.model(alt_lang);
        let Some(wa) = ma.clean(&raw_alt) else {
            v.reason = "alt not a word";
            return v;
        };
        let alen = wa.chars().filter(|c| *c != '-').count();
        if alen == 1 && keys.len() == 1 && self.single_letters {
            // One-letter words: "d" -> "в", "z" -> "я", "ш" -> "i" ... Decided by the letter it
            // would become, so it also works with customised layouts.
            let hit = match (cur_lang, alt_lang) {
                (Language::En, Language::Uk) => {
                    matches!(wa.as_str(), "в" | "і" | "с" | "я" | "о" | "у" | "а" | "з" | "й")
                }
                (Language::En, Language::Ru) => matches!(wa.as_str(), "в" | "и" | "с" | "я" | "о" | "у" | "а" | "к"),
                (_, Language::En) => matches!(wa.as_str(), "i" | "a"),
                // Russian layout, Ukrainian meant: «ы» is «і» on the same key
                (Language::Ru, Language::Uk) => wa == "і",
                _ => false,
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
        v.rank = ra;
        v.score = sa;
        let wc = if has_edge_junk(&raw_cur, cur_lang) { None } else { mc.clean(&raw_cur) };
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

    #[test]
    fn three_layouts() {
        let mut d = Detector::new();
        d.layouts = crate::layout::en_ru_ua();
        let (en, ru, ua) = (Lang::En, Lang::Ru, Lang(2));
        let check = |text: &str, typed: Lang| {
            let keys = d.layouts.keys_for_text(text, typed).unwrap();
            let v = d.check(&keys, typed);
            v.switch.then_some((v.to, v.alternative))
        };
        // English keys: Russian or Ukrainian, whichever is a word
        assert_eq!(check("ghbdtn", en), Some((ru, "привет".into())));
        assert_eq!(check("ghbdsn", en), Some((ua, "привіт".into())));
        assert_eq!(check("lzre.", en), Some((ua, "дякую".into())));
        // Russian layout, Ukrainian meant, and the other way round
        assert_eq!(check("привыт", ru), Some((ua, "привіт".into())));
        assert_eq!(check("місль", ua), Some((ru, "мысль".into())));
        // English typed in either Cyrillic layout
        assert_eq!(check("руддщ", ru), Some((en, "hello".into())));
        assert_eq!(check("руддщ", ua), Some((en, "hello".into())));
        // Real words stay where they are
        for (w, l) in [("привет", ru), ("мысль", ru), ("как", ru), ("привіт", ua), ("дякую", ua), ("також", ua)]
        {
            assert_eq!(check(w, l), None, "{w}");
        }
        for w in ["hello", "the", "github"] {
            assert_eq!(check(w, en), None, "{w}");
        }
    }

    #[test]
    fn same_text_goes_to_the_recent_layout() {
        let mut d = Detector::new();
        d.layouts = crate::layout::en_ru_ua();
        let keys = d.layouts.keys_for_text("ghbdtn", Lang::En).unwrap();
        assert_eq!(d.check(&keys, Lang::En).to, Lang::Ru);
        d.layouts.touch(Lang(2), 100);
        assert_eq!(d.check(&keys, Lang::En).to, Lang(2));
    }

    #[test]
    fn disabled_target() {
        let mut d = Detector::new();
        d.layouts = crate::layout::en_ru_ua();
        d.layouts.list[2].target = false;
        let keys = d.layouts.keys_for_text("ghbdsn", Lang::En).unwrap();
        assert!(!d.check(&keys, Lang::En).switch || d.check(&keys, Lang::En).to != Lang(2));
    }

    fn ukrainian() -> Detector {
        use crate::layout::KeyMap;
        let mut d = Detector::new();
        let mut ru = KeyMap::builtin(Lang::Ru);
        ru.set(b'S', Some('і'), Some('І'), true);
        ru.set(0xDD, Some('ї'), Some('Ї'), true);
        ru.set(0xDE, Some('є'), Some('Є'), true);
        ru.set(0xC0, Some('\''), Some('₴'), false);
        d.layouts = crate::layout::Layouts::new(vec![
            crate::layout::Layout::new(KeyMap::builtin(Lang::En), vec![crate::layout::Language::En], "EN"),
            crate::layout::Layout::new(ru, vec![crate::layout::Language::Uk, crate::layout::Language::Ru], "UA"),
        ]);
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
