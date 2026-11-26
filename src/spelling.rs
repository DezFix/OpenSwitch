// SPDX-License-Identifier: GPL-3.0-or-later
//! Module "Spelling": fixes typos with the dictionaries already built into OpenSwitch.
//!
//! No AI and nothing leaves the computer. For a word that is not in the dictionary we look at
//! every word one edit away (a letter missed, an extra letter, a wrong letter, two letters
//! swapped) and fix the word only when one frequent word clearly stands out. Edits that are
//! typical typos (swapped letters, a neighbouring key, a doubled letter, е/и and о/а) count
//! more than others.

use crate::detect::LangModel;
use crate::layout::KeyMap;

/// Shortest word that is checked: short words have too many neighbours.
pub const MIN_LEN: usize = 5;

/// The knobs of the algorithm (the defaults were chosen with `examples/eval_spell.rs`).
#[derive(Clone, Copy, Debug)]
pub struct Tuning {
    /// Only frequent words are offered as a fix.
    pub max_rank: u32,
    /// An edit that is not a typical typo counts as if the word were this many times rarer.
    pub unlikely: u64,
    /// Fix only typical typos (swapped letters, a neighbouring key, a doubled or missed
    /// letter, е/и and о/а), never an arbitrary wrong letter.
    pub likely_only: bool,
    /// How sure we must be that the best fix is the word meant, 0..1. Every candidate gets a
    /// weight (more frequent words and typical typos weigh more); the confidence is the share
    /// of the best one in the total. 0.85 = fixed only when 85 % sure.
    pub min_confidence: f64,
    /// Letters at the end of a word where a different, extra or missing letter is usually an
    /// ending (breakfasts / breakfast, извинений / извинения), not a typo.
    pub ending: usize,
    /// The fix must look this much more like the language than the typed word (character
    /// model, average log-probability per letter): a typo usually breaks the letter patterns.
    pub min_gain: f32,
}

impl Default for Tuning {
    fn default() -> Self {
        Tuning { max_rank: 30_000, unlikely: 6, likely_only: true, min_confidence: 0.85, ending: 2, min_gain: 0.0 }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Edit {
    Swap,
    Extra { doubled: bool },
    Wrong,
    Missed,
}

/// Physical key rows (virtual keys), to know which keys are neighbours.
const ROWS: [&[u8]; 3] = [
    &[b'Q', b'W', b'E', b'R', b'T', b'Y', b'U', b'I', b'O', b'P', 0xDB, 0xDD],
    &[b'A', b'S', b'D', b'F', b'G', b'H', b'J', b'K', b'L', 0xBA, 0xDE],
    &[b'Z', b'X', b'C', b'V', b'B', b'N', b'M', 0xBC, 0xBE, 0xBF],
];

fn position(vk: u8) -> Option<(i32, i32)> {
    ROWS.iter().enumerate().find_map(|(r, row)| row.iter().position(|&k| k == vk).map(|c| (r as i32, c as i32)))
}

/// Are two letters on neighbouring keys in this layout?
fn neighbours(map: Option<&KeyMap>, a: char, b: char) -> bool {
    let Some(map) = map else { return false };
    let (Some(pa), Some(pb)) = (map.key_of_char(a).and_then(position), map.key_of_char(b).and_then(position)) else {
        return false;
    };
    let (dr, dc) = (pa.0 - pb.0, pa.1 - pb.1);
    (dr == 0 && dc.abs() == 1) || (dr.abs() == 1 && (dc == 0 || dc == -dr))
}

/// Letters people mix up when they don't know the spelling (not a slip of the finger).
fn confusable(a: char, b: char) -> bool {
    let pair = |x, y| (a == x && b == y) || (a == y && b == x);
    pair('е', 'и') || pair('о', 'а') || pair('е', 'я') || pair('и', 'ы') || pair('е', 'є') || pair('и', 'і')
}

/// How sure we are of the first of `costs` (sorted, smaller = better): its weight 1/cost as a
/// share of all weights. One candidate = 1.0; two equal ones = 0.5.
pub fn confidence(costs: &[u64]) -> f64 {
    let weight = |c: u64| 1.0 / c.max(1) as f64;
    let total: f64 = costs.iter().map(|&c| weight(c)).sum();
    costs.first().map_or(0.0, |&c| weight(c) / total)
}

/// The fix for `typed` (a word as typed, in the language of `model`), if there is a clear one.
/// `map` is the layout it was typed in (for neighbouring keys); the capital first letter is
/// kept. Words with capitals inside, digits or other signs are left alone.
pub fn correct(model: &LangModel, map: Option<&KeyMap>, typed: &str) -> Option<String> {
    correct_with(model, map, typed, &Tuning::default())
}

/// `correct` with other knobs (for the accuracy evaluation).
pub fn correct_with(model: &LangModel, map: Option<&KeyMap>, typed: &str, t: &Tuning) -> Option<String> {
    let chars: Vec<char> = typed.chars().collect();
    if chars.len() < MIN_LEN || !chars.iter().all(|c| c.is_alphabetic()) {
        return None;
    }
    let capital = chars[0].is_uppercase();
    if chars[1..].iter().any(|c| c.is_uppercase()) {
        return None; // ПРИВЕТ, iPhone, McDonald
    }
    let word = model.clean(typed)?;
    if model.rank(&word).is_some() {
        return None; // a known word
    }
    let w: Vec<char> = word.chars().collect();
    let letters: Vec<char> = model.letters().collect();

    // Every dictionary word one edit away: (cost, word, edit, position, typical typo?).
    let mut found: Vec<(u64, String, Edit, usize, bool)> = Vec::new();
    let mut offer = |cand: Vec<char>, edit: Edit, pos: usize, likely: bool| {
        let cand: String = cand.into_iter().collect();
        let Some(rank) = model.rank(&cand).filter(|r| *r <= t.max_rank) else { return };
        let cost = rank as u64 * if likely { 1 } else { t.unlikely };
        match found.iter_mut().find(|e| e.1 == cand) {
            Some(e) if cost < e.0 => *e = (cost, cand, edit, pos, likely),
            Some(_) => {}
            None => found.push((cost, cand, edit, pos, likely)),
        }
    };
    for i in 0..w.len() {
        let mut v = w.clone();
        let removed = v.remove(i);
        let doubled = (i > 0 && w[i - 1] == removed) || w.get(i + 1) == Some(&removed);
        let slip =
            (i > 0 && neighbours(map, w[i - 1], removed)) || w.get(i + 1).is_some_and(|&n| neighbours(map, n, removed));
        offer(v, Edit::Extra { doubled }, i, doubled || slip);
        if i + 1 < w.len() && w[i] != w[i + 1] {
            let mut v = w.clone();
            v.swap(i, i + 1);
            offer(v, Edit::Swap, i, true);
        }
        for &c in &letters {
            if c != w[i] {
                let mut v = w.clone();
                v[i] = c;
                offer(v, Edit::Wrong, i, neighbours(map, w[i], c) || confusable(w[i], c));
            }
        }
    }
    for i in 0..=w.len() {
        for &c in &letters {
            let mut v = w.clone();
            v.insert(i, c);
            offer(v, Edit::Missed, i, true);
        }
    }
    found.sort();
    let (cost, fix, edit, pos, likely) = found.first()?.clone();
    if confidence(&found.iter().map(|f| f.0).collect::<Vec<_>>()) < t.min_confidence {
        return None; // not sure enough which word was meant: better do nothing
    }
    // The vetoes never pick another fix: when the best one is doubtful, nothing is changed.
    if t.likely_only && !likely {
        return None;
    }
    let in_ending = pos + t.ending >= w.len();
    if in_ending && !matches!(edit, Edit::Swap | Edit::Extra { doubled: true }) {
        return None; // probably another form of the word
    }
    if model.score(&fix) - model.score(&word) < t.min_gain {
        return None; // the typed word looks no less like the language
    }
    Some(if capital {
        let mut c = fix.chars();
        c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
    } else {
        fix
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::Detector;
    use crate::layout::{Lang, Language};

    #[test]
    fn typos() {
        let d = Detector::new();
        let ru = d.model(Language::Ru);
        let map = KeyMap::builtin(Lang::Ru);
        let fix = |w: &str| correct(ru, Some(&map), w);
        assert_eq!(fix("првиет").as_deref(), Some("привет")); // swapped letters
        assert_eq!(fix("Сегоддня").as_deref(), Some("Сегодня")); // doubled letter, capital kept
        assert_eq!(fix("хорошл").as_deref(), None); // a different ending: left alone
        assert_eq!(fix("спаисбо").as_deref(), Some("спасибо"));
        assert_eq!(fix("нкесколько").as_deref(), Some("несколько")); // neighbouring key
                                                                     // known words, short words, capitals inside, digits: left alone
        for w in ["привет", "сегодня", "кот", "ПРИВЕТ", "пРивет", "привет2", "ксенофонт"]
        {
            assert_eq!(fix(w), None, "{w}");
        }
        let en = d.model(Language::En);
        let enmap = KeyMap::builtin(Lang::En);
        assert_eq!(correct(en, Some(&enmap), "teh").as_deref(), None); // too short
        assert_eq!(correct(en, Some(&enmap), "freind").as_deref(), Some("friend"));
        assert_eq!(correct(en, Some(&enmap), "Thnaks").as_deref(), Some("Thanks"));
        // another form of a word is not a typo
        assert_eq!(correct(en, Some(&enmap), "breakfastz").as_deref(), None);
    }

    #[test]
    fn confidence_share() {
        assert_eq!(confidence(&[100]), 1.0);
        assert_eq!(confidence(&[100, 100]), 0.5);
        assert!((confidence(&[100, 900]) - 0.9).abs() < 1e-9);
        assert_eq!(confidence(&[]), 0.0);
    }

    #[test]
    fn neighbour_keys() {
        let map = KeyMap::builtin(Lang::Ru);
        assert!(neighbours(Some(&map), 'и', 'т')); // B, N
        assert!(neighbours(Some(&map), 'п', 'р')); // G, H
        assert!(neighbours(Some(&map), 'е', 'п')); // T, G (row below)
        assert!(!neighbours(Some(&map), 'й', 'ю'));
    }
}
