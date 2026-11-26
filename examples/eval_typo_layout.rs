// SPDX-License-Identifier: GPL-3.0-or-later
//! A typo in a word typed in the wrong layout (`ghdbtn` = «првиет» → «привет»):
//! cargo run --release --example eval_typo_layout   (SWEEP=1 tries several thresholds)
//!
//! 1. Hits: frequent words (5+ letters) of one language with one typical typo, typed in the
//!    layout of the other language. Before: only the detector (then the spelling module fixes
//!    the typo); after: also `spell_other_layout` when the detector kept the word.
//! 2. Harm, words that must not be converted:
//!    - typos typed in the right layout (they belong to the spelling module of that language);
//!    - rare but correct words (ranks 30 000..60 000 against a dictionary of 30 000 words).
use openswitch::detect::Detector;
use openswitch::engine::{spell_other_layout, OTHER_LAYOUT_GAIN};
use openswitch::layout::{KeyMap, KeyStroke, Lang, Language};
use openswitch::spelling::MIN_LEN;

/// Neighbouring key to the right in the same row (for "a neighbouring key" typos).
fn right_neighbour(map: &KeyMap, c: char) -> Option<char> {
    const ROWS: [&[u8]; 3] = [
        &[b'Q', b'W', b'E', b'R', b'T', b'Y', b'U', b'I', b'O', b'P', 0xDB, 0xDD],
        &[b'A', b'S', b'D', b'F', b'G', b'H', b'J', b'K', b'L', 0xBA, 0xDE],
        &[b'Z', b'X', b'C', b'V', b'B', b'N', b'M', 0xBC, 0xBE, 0xBF],
    ];
    let vk = map.key_of_char(c)?;
    let row = ROWS.iter().find(|r| r.contains(&vk))?;
    let i = row.iter().position(|&k| k == vk)?;
    let next = *row.get(i + 1).or(row.get(i.wrapping_sub(1)))?;
    map.render_key(KeyStroke { vk: next, shift: false, caps: false }).filter(|c| c.is_alphabetic())
}

/// (typo, word) pairs: 4 typo kinds for the 5 000 most frequent words of 5+ letters.
fn typos(d: &Detector, lang: Language, layout: Lang) -> Vec<(String, String)> {
    let model = d.model(lang);
    let map = d.layouts.map(layout);
    let words: Vec<&str> = model
        .words()
        .into_iter()
        .filter(|w| w.chars().count() >= MIN_LEN && w.chars().all(char::is_alphabetic))
        .take(5000)
        .collect();
    let mut out = Vec::new();
    for kind in 0..4 {
        for (i, w) in words.iter().enumerate() {
            let c: Vec<char> = w.chars().collect();
            let p = 1 + i % (c.len() - 2);
            let mut t = c.clone();
            match kind {
                0 => t.swap(p, p + 1),
                1 => match right_neighbour(map, c[p]) {
                    Some(x) => t[p] = x,
                    None => continue,
                },
                2 => {
                    t.remove(p);
                }
                _ => t.insert(p, c[p]),
            }
            let typo: String = t.iter().collect();
            if model.rank(&typo).is_none() {
                out.push((typo, w.to_string()));
            }
        }
    }
    out
}

/// The word typed in `typed_in` would be converted by the new step (the detector kept it).
fn converted(d: &Detector, keys: &[KeyStroke], typed_in: Lang, gain: f32) -> Option<String> {
    if d.check(keys, typed_in).switch {
        return None; // the detector's business, as before
    }
    spell_other_layout(d, keys, typed_in, gain).map(|(to, k)| d.layouts.render(&k, to))
}

fn main() {
    let d = Detector::new();
    let limited = Detector::limited(30_000);
    let (en, ru) = (Lang(0), Lang(1));
    let mut gains = vec![OTHER_LAYOUT_GAIN];
    if std::env::var("SWEEP").is_ok() {
        gains = vec![-10.0, 0.0, 0.5, 1.0, 1.5, 2.0];
    }
    let verbose = std::env::var("VERBOSE").is_ok();
    let sets = [("RU word typed in EN", Language::Ru, ru, en), ("EN word typed in RU", Language::En, en, ru)];
    let pc = |x: usize, n: usize| x as f64 * 100.0 / n.max(1) as f64;

    // The test words do not depend on the threshold.
    let hit_sets: Vec<_> = sets.iter().map(|&(name, lang, layout, _)| (name, typos(&d, lang, layout))).collect();
    let own = [("EN typos typed in EN", Language::En, en), ("RU typos typed in RU", Language::Ru, ru)];
    let own_sets: Vec<_> = own.iter().map(|&(name, lang, layout)| (name, typos(&d, lang, layout), layout)).collect();

    for gain in gains {
        println!("gain {gain}");
        for (i, (name, list)) in hit_sets.iter().enumerate() {
            let (_, _, layout, typed_in) = sets[i];
            let (mut before, mut after, mut wrong) = (0, 0, 0);
            let mut bad = Vec::new();
            for (typo, word) in list {
                let Some(keys) = d.layouts.keys_for_text(typo, layout) else { continue };
                let v = d.check(&keys, typed_in);
                if v.switch {
                    // As before: the detector converts, then the spelling module may fix it.
                    let shown = d.layouts.render(&keys, v.to);
                    let fixed = openswitch::spelling::correct(
                        d.model(d.layouts.languages(v.to)[0]),
                        Some(d.layouts.map(v.to)),
                        &shown,
                    );
                    if fixed.as_deref() == Some(word.as_str()) {
                        before += 1;
                        after += 1;
                    }
                } else if let Some((to, k)) = spell_other_layout(&d, &keys, typed_in, gain) {
                    let got = d.layouts.render(&k, to);
                    if got == *word {
                        after += 1;
                    } else {
                        wrong += 1;
                        if bad.len() < 10 {
                            bad.push(format!("{typo}->{got}({word})"));
                        }
                    }
                }
            }
            let n = list.len();
            println!(
                "  {name}: n={n} fixed before={:.1}% after={:.1}% wrong={:.2}% {}",
                pc(before, n),
                pc(after, n),
                pc(wrong, n),
                if verbose { bad.join(" ") } else { String::new() }
            );
        }
        for (name, list, layout) in &own_sets {
            let mut spoiled = Vec::new();
            for (typo, _) in list {
                let Some(keys) = d.layouts.keys_for_text(typo, *layout) else { continue };
                if let Some(got) = converted(&d, &keys, *layout, gain) {
                    spoiled.push(format!("{typo}->{got}"));
                }
            }
            println!(
                "  {name}: n={} converted={:.2}% {}",
                list.len(),
                pc(spoiled.len(), list.len()),
                if verbose { spoiled.iter().take(10).cloned().collect::<Vec<_>>().join(" ") } else { String::new() }
            );
        }
        for (name, lang, layout) in
            [("rare EN words typed in EN", Language::En, en), ("rare RU words typed in RU", Language::Ru, ru)]
        {
            let words: Vec<&str> = d
                .model(lang)
                .words()
                .into_iter()
                .skip(30_000)
                .filter(|w| w.chars().count() >= MIN_LEN && w.chars().all(char::is_alphabetic))
                .collect();
            let mut spoiled = Vec::new();
            for w in &words {
                let Some(keys) = limited.layouts.keys_for_text(w, layout) else { continue };
                if let Some(got) = converted(&limited, &keys, layout, gain) {
                    spoiled.push(format!("{w}->{got}"));
                }
            }
            println!(
                "  {name}: n={} converted={:.2}% {}",
                words.len(),
                pc(spoiled.len(), words.len()),
                if verbose { spoiled.iter().take(10).cloned().collect::<Vec<_>>().join(" ") } else { String::new() }
            );
        }
    }
}
