// SPDX-License-Identifier: GPL-3.0-or-later
//! Accuracy with the Ukrainian dictionary, on a standard Ukrainian layout:
//!   cargo run --release --example eval_uk -- en.txt ru.txt uk.txt
//! Each list: one word per line, sampled by real word frequency (e.g. 30 000 draws from the
//! top 50 000 words of `wordfreq`).
use openswitch::detect::Detector;
use openswitch::layout::{en_ru_ua, ukrainian_map, KeyMap, Lang, Language, Layout, Layouts};

/// English + one Cyrillic layout with the given map and languages.
fn pair(map: KeyMap, languages: Vec<Language>) -> Layouts {
    Layouts::new(vec![
        Layout::new(KeyMap::builtin(Lang::En), vec![Language::En], "EN"),
        Layout::new(map, languages, "CY"),
    ])
}

/// Words of `path` typed in layout `lang`: they must not switch; typed in English (or, from
/// English, in `back`) they must.
fn run(name: &str, d: &Detector, path: &str, lang: Lang, back: Lang) {
    let text = std::fs::read_to_string(path).unwrap();
    let (mut n, mut fp, mut hit) = (0, 0, 0);
    let mut fps = std::collections::HashMap::<String, usize>::new();
    let mut miss = std::collections::HashMap::<String, usize>::new();
    for w in text.lines().filter(|l| !l.is_empty()) {
        let Some(keys) = d.layouts.keys_for_text(w, lang) else { continue };
        n += 1;
        if d.check(&keys, lang).switch {
            fp += 1;
            *fps.entry(w.into()).or_default() += 1;
        }
        let wrong = d.layouts.render(&keys, back);
        let Some(wrong_keys) = d.layouts.keys_for_text(&wrong, back) else { continue };
        let v = d.check(&wrong_keys, back);
        if v.switch && v.to == lang {
            hit += 1
        } else {
            *miss.entry(w.into()).or_default() += 1;
        }
    }
    let top = |m: std::collections::HashMap<String, usize>| {
        let mut v: Vec<_> = m.into_iter().collect();
        v.sort_by_key(|x| std::cmp::Reverse(x.1));
        v.into_iter().take(12).map(|(w, c)| format!("{w}:{c}")).collect::<Vec<_>>().join(" ")
    };
    println!(
        "{name:28} {path:7} n={n:5} false-switch={:.3}% detect={:.2}%",
        fp as f64 * 100.0 / n as f64,
        hit as f64 * 100.0 / n as f64
    );
    println!("    FP: {}", top(fps));
    println!("    MISS: {}", top(miss));
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (en, ru) = (Lang::En, Lang::Ru);
    let base = Detector::new();
    run("EN+RU", &base, &a[0], en, ru);
    run("EN+RU", &base, &a[1], ru, en);
    let mut ua = Detector::new();
    ua.layouts = pair(ukrainian_map(), vec![Language::Uk, Language::Ru]);
    run("EN+UA (UA also Russian)", &ua, &a[0], en, ru);
    run("EN+UA (UA also Russian)", &ua, &a[2], ru, en);
    let mut three = Detector::new();
    three.layouts = en_ru_ua();
    let uk = Lang(2);
    run("EN+RU+UA", &three, &a[0], en, ru);
    run("EN+RU+UA", &three, &a[1], ru, en);
    run("EN+RU+UA", &three, &a[2], uk, en);
}
