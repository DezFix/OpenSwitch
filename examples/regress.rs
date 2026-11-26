// SPDX-License-Identifier: GPL-3.0-or-later
//! Quality check for CI: cargo run --release --example regress
//!
//! A fast version of the evaluations (`eval*.rs`) on the built-in dictionaries only. Prints
//! every number and exits with an error if one is worse than its limit in `LIMITS`. The limits
//! are the measured values with a small margin: a change that makes the detector or the
//! spelling module noticeably worse fails CI; a change that makes them better should lower the
//! limits here (measure again with this example).
use openswitch::detect::Detector;
use openswitch::engine::{spell_other_layout, OTHER_LAYOUT_GAIN};
use openswitch::layout::{en_ru_ua, KeyMap, KeyStroke, Lang, Language, Layouts};
use openswitch::spelling::{correct, MIN_LEN};

/// Metric name, limit, true if bigger is better.
const LIMITS: &[(&str, f64, bool)] = &[
    ("EN+RU: EN words switched (false)", 0.20, false),
    ("EN+RU: RU words switched (false)", 0.20, false),
    ("EN+RU: EN in RU layout detected", 96.5, true),
    ("EN+RU: RU in EN layout detected", 97.5, true),
    ("EN+RU+UA: UK words switched (false)", 0.15, false),
    ("EN+RU+UA: UK in EN layout detected", 97.5, true),
    ("spelling: EN typos fixed", 68.0, true),
    ("spelling: RU typos fixed", 71.5, true),
    ("spelling: UK typos fixed", 74.0, true),
    ("spelling: typos fixed wrong", 0.10, false),
    ("spelling: rare words spoiled", 2.2, false),
    ("wrong layout + typo: fixed", 66.5, true),
    ("wrong layout + typo: own-layout typos moved", 0.05, false),
];

/// Frequent words of a language with 2+ letters, letters only.
fn words(d: &Detector, lang: Language, skip: usize, n: usize, min_len: usize) -> Vec<&'static str> {
    d.model(lang)
        .words()
        .into_iter()
        .skip(skip)
        .filter(|w| w.chars().count() >= min_len && w.chars().all(char::is_alphabetic))
        .take(n)
        .collect()
}

fn pct(x: usize, n: usize) -> f64 {
    x as f64 * 100.0 / n.max(1) as f64
}

/// Share of `list` typed in `typed` that the detector switches.
fn switched(d: &Detector, list: &[&str], layout: Lang, typed: Lang) -> f64 {
    let mut n = 0;
    let mut hit = 0;
    for w in list {
        let Some(keys) = d.layouts.keys_for_text(w, layout) else { continue };
        n += 1;
        if d.check(&keys, typed).switch {
            hit += 1;
        }
    }
    pct(hit, n)
}

/// Neighbouring key to the right in the same row.
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

/// (typo, word): one typical typo (swap, neighbour, missed, doubled, in turn) per word.
fn typos(d: &Detector, lang: Language, map: &KeyMap, list: &[&str]) -> Vec<(String, String)> {
    let model = d.model(lang);
    let mut out = Vec::new();
    for (i, w) in list.iter().enumerate() {
        let c: Vec<char> = w.chars().collect();
        let p = 1 + i % (c.len() - 2);
        let mut t = c.clone();
        match i % 4 {
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
    out
}

fn main() {
    let mut got: Vec<(&str, f64)> = Vec::new();
    let (en, ru, uk) = (Lang(0), Lang(1), Lang(2));

    // Detector, EN+RU.
    let d = Detector::new();
    let en_words = words(&d, Language::En, 0, 20_000, 1);
    let ru_words = words(&d, Language::Ru, 0, 20_000, 1);
    got.push(("EN+RU: EN words switched (false)", switched(&d, &en_words, en, en)));
    got.push(("EN+RU: RU words switched (false)", switched(&d, &ru_words, ru, ru)));
    got.push(("EN+RU: EN in RU layout detected", switched(&d, &en_words, en, ru)));
    got.push(("EN+RU: RU in EN layout detected", switched(&d, &ru_words, ru, en)));

    // Detector, EN+RU+UA.
    let mut d3 = Detector::new();
    d3.layouts = en_ru_ua();
    let uk_words = words(&d3, Language::Uk, 0, 20_000, 1);
    got.push(("EN+RU+UA: UK words switched (false)", switched(&d3, &uk_words, uk, uk)));
    got.push(("EN+RU+UA: UK in EN layout detected", switched(&d3, &uk_words, uk, en)));

    // Spelling.
    let maps: Layouts = en_ru_ua();
    let (mut wrong, mut total) = (0, 0);
    for (name, lang, layout) in [
        ("spelling: EN typos fixed", Language::En, en),
        ("spelling: RU typos fixed", Language::Ru, ru),
        ("spelling: UK typos fixed", Language::Uk, uk),
    ] {
        let map = maps.map(layout);
        let list = typos(&d3, lang, map, &words(&d3, lang, 0, 2000, MIN_LEN));
        let mut ok = 0;
        for (typo, word) in &list {
            match correct(d3.model(lang), Some(map), typo) {
                Some(f) if f == *word => ok += 1,
                Some(_) => wrong += 1,
                None => {}
            }
        }
        total += list.len();
        got.push((name, pct(ok, list.len())));
    }
    got.push(("spelling: typos fixed wrong", pct(wrong, total)));
    let small = Detector::limited(30_000);
    let (mut spoiled, mut n) = (0, 0);
    for (lang, layout) in [(Language::En, en), (Language::Ru, ru), (Language::Uk, uk)] {
        for w in words(&d3, lang, 30_000, 4000, MIN_LEN) {
            n += 1;
            if correct(small.model(lang), Some(maps.map(layout)), w).is_some() {
                spoiled += 1;
            }
        }
    }
    got.push(("spelling: rare words spoiled", pct(spoiled, n)));

    // A typo and the wrong layout at once (EN+RU).
    let (mut fixed, mut n) = (0, 0);
    for (lang, layout, typed) in [(Language::Ru, ru, en), (Language::En, en, ru)] {
        for (typo, word) in typos(&d, lang, d.layouts.map(layout), &words(&d, lang, 0, 2000, MIN_LEN)) {
            let Some(keys) = d.layouts.keys_for_text(&typo, layout) else { continue };
            n += 1;
            let v = d.check(&keys, typed);
            let result = if v.switch {
                correct(d.model(lang), Some(d.layouts.map(v.to)), &d.layouts.render(&keys, v.to))
            } else {
                spell_other_layout(&d, &keys, typed, OTHER_LAYOUT_GAIN).map(|(to, k)| d.layouts.render(&k, to))
            };
            if result.as_deref() == Some(word.as_str()) {
                fixed += 1;
            }
        }
    }
    got.push(("wrong layout + typo: fixed", pct(fixed, n)));
    let (mut moved, mut n) = (0, 0);
    for (lang, layout) in [(Language::En, en), (Language::Ru, ru)] {
        for (typo, _) in typos(&d, lang, d.layouts.map(layout), &words(&d, lang, 0, 2000, MIN_LEN)) {
            let Some(keys) = d.layouts.keys_for_text(&typo, layout) else { continue };
            n += 1;
            if !d.check(&keys, layout).switch && spell_other_layout(&d, &keys, layout, OTHER_LAYOUT_GAIN).is_some() {
                moved += 1;
            }
        }
    }
    got.push(("wrong layout + typo: own-layout typos moved", pct(moved, n)));

    let mut failed = 0;
    for (name, value) in &got {
        let (_, limit, higher) = LIMITS.iter().find(|l| l.0 == *name).expect("limit");
        let ok = if *higher { value >= limit } else { value <= limit };
        if !ok {
            failed += 1;
        }
        let sign = if *higher { ">=" } else { "<=" };
        println!("{} {name:46} {value:7.2}%   (limit {sign} {limit})", if ok { "ok  " } else { "FAIL" });
    }
    if failed > 0 {
        eprintln!("{failed} metric(s) worse than the limit");
        std::process::exit(1);
    }
}
