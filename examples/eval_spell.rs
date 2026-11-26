// SPDX-License-Identifier: GPL-3.0-or-later
//! Accuracy of the spelling module: cargo run --release --example eval_spell
//!
//! 1. Typos: frequent words (5+ letters) with one typical typo each — two letters swapped,
//!    a neighbouring key, a letter missed, a letter doubled. How many come back right, how many
//!    are "fixed" to another word, how many are left alone.
//! 2. Rare but correct words: words ranked 30 000..60 000 checked against a dictionary of the
//!    30 000 most frequent only — as if they were missing from the dictionary. Every "fix" here
//!    would spoil a correct word.
use openswitch::detect::LangModel;
use openswitch::layout::{ukrainian_map, KeyMap, Lang, Language};
use openswitch::spelling::{correct_with, Tuning, MIN_LEN};

/// Neighbouring key to the right in the same row, in `map` (for "a neighbouring key" typos).
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
    let ks = openswitch::layout::KeyStroke { vk: next, shift: false, caps: false };
    map.render_key(ks).filter(|c| c.is_alphabetic())
}

fn typos(name: &str, model: &LangModel, map: &KeyMap, tu: &Tuning) -> f64 {
    let mut total = 0.0;
    let words: Vec<&str> = model
        .words()
        .into_iter()
        .filter(|w| w.chars().count() >= MIN_LEN && w.chars().all(char::is_alphabetic))
        .take(5000)
        .collect();
    for kind in ["swap", "neighbour", "missed", "doubled"] {
        let (mut n, mut ok, mut wrong, mut left) = (0, 0, 0, 0);
        let mut bad = Vec::new();
        for (i, w) in words.iter().enumerate() {
            let c: Vec<char> = w.chars().collect();
            let p = 1 + i % (c.len() - 2);
            let mut t = c.clone();
            match kind {
                "swap" => t.swap(p, p + 1),
                "neighbour" => match right_neighbour(map, c[p]) {
                    Some(x) => t[p] = x,
                    None => continue,
                },
                "missed" => {
                    t.remove(p);
                }
                _ => t.insert(p, c[p]),
            }
            let typo: String = t.iter().collect();
            if model.rank(&typo).is_some() {
                continue; // the typo is a real word: nothing to fix
            }
            n += 1;
            match correct_with(model, Some(map), &typo, tu) {
                Some(f) if f == *w => ok += 1,
                Some(f) => {
                    wrong += 1;
                    if bad.len() < 8 {
                        bad.push(format!("{typo}->{f}({w})"));
                    }
                }
                None => left += 1,
            }
        }
        let pc = |x: i32| x as f64 * 100.0 / n as f64;
        total += pc(ok) / 4.0;
        if std::env::var("VERBOSE").is_ok() {
            println!(
                "{name} {kind:9} n={n:5} fixed={:5.1}% wrong={:4.1}% left={:5.1}%  {}",
                pc(ok),
                pc(wrong),
                pc(left),
                bad.join(" ")
            );
        } else {
            print!(" {kind}={:.0}/{:.1}", pc(ok), pc(wrong));
        }
    }
    total
}

fn rare(name: &str, full: &LangModel, small: &LangModel, map: &KeyMap, tu: &Tuning) -> f64 {
    let words: Vec<&str> = full
        .words()
        .into_iter()
        .skip(30_000)
        .filter(|w| w.chars().count() >= MIN_LEN && w.chars().all(char::is_alphabetic))
        .collect();
    let mut spoiled = Vec::new();
    for w in &words {
        if let Some(f) = correct_with(small, Some(map), w, tu) {
            spoiled.push(format!("{w}->{f}"));
        }
    }
    let rate = spoiled.len() as f64 * 100.0 / words.len() as f64;
    if std::env::var("VERBOSE").is_ok() {
        println!(
            "{name} rare correct words: n={} spoiled={rate:.2}%  {}",
            words.len(),
            spoiled.iter().take(12).cloned().collect::<Vec<_>>().join(" ")
        );
    }
    rate
}

fn main() {
    let en = KeyMap::builtin(Lang::En);
    let ru = KeyMap::builtin(Lang::Ru);
    let ua = ukrainian_map();
    let d = Tuning::default();
    let mut variants = vec![("default", d)];
    if std::env::var("SWEEP").is_ok() {
        for min_confidence in [0.8, 0.9] {
            variants.push(("", Tuning { min_confidence, ..d }));
        }
    }
    let (men, mru, muk) = (LangModel::new(Language::En), LangModel::new(Language::Ru), LangModel::new(Language::Uk));
    let (sen, sru, suk) = (
        LangModel::limited(Language::En, 30_000),
        LangModel::limited(Language::Ru, 30_000),
        LangModel::limited(Language::Uk, 30_000),
    );
    for (name, tu) in variants {
        println!("{name} {tu:?}");
        print!("  EN");
        let a = typos("EN", &men, &en, &tu);
        print!("\n  RU");
        let b = typos("RU", &mru, &ru, &tu);
        print!("\n  UK");
        let c = typos("UK", &muk, &ua, &tu);
        let (x, y, z) =
            (rare("EN", &men, &sen, &en, &tu), rare("RU", &mru, &sru, &ru, &tu), rare("UK", &muk, &suk, &ua, &tu));
        println!(
            "\n  => typos fixed avg EN {a:.1} RU {b:.1} UK {c:.1} | rare spoiled EN {x:.2}% RU {y:.2}% UK {z:.2}%"
        );
    }
}
