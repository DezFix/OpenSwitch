// SPDX-License-Identifier: GPL-3.0-or-later
//! Accuracy with the Ukrainian dictionary, on a standard Ukrainian layout:
//!   cargo run --release --example eval_uk -- en.txt ru.txt uk.txt
//! Each list: one word per line, sampled by real word frequency (e.g. 30 000 draws from the
//! top 50 000 words of `wordfreq`).
use openswitch::detect::Detector;
use openswitch::layout::{KeyMap, Lang, Layouts};

fn ua_layouts() -> Layouts {
    let mut ru = KeyMap::builtin(Lang::Ru);
    ru.set(b'S', Some('і'), Some('І'), true);
    ru.set(0xDD, Some('ї'), Some('Ї'), true);
    ru.set(0xDE, Some('є'), Some('Є'), true);
    ru.set(0xC0, Some('\''), Some('₴'), false);
    Layouts { en: KeyMap::builtin(Lang::En), ru }
}

fn run(name: &str, d: &Detector, path: &str, lang: Lang) {
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
        if d.check(&keys, lang.other()).switch {
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
    let base = Detector::new();
    run("RU only, ЙЦУКЕН", &base, &a[0], Lang::En);
    run("RU only, ЙЦУКЕН", &base, &a[1], Lang::Ru);
    let mut uk = Detector::new();
    uk.set_ukrainian(true);
    run("RU+UK, ЙЦУКЕН", &uk, &a[0], Lang::En);
    run("RU+UK, ЙЦУКЕН", &uk, &a[1], Lang::Ru);
    uk.layouts = ua_layouts();
    run("RU+UK, Ukrainian layout", &uk, &a[0], Lang::En);
    run("RU+UK, Ukrainian layout", &uk, &a[2], Lang::Ru);
    let mut ruua = Detector::new();
    ruua.layouts = ua_layouts();
    run("RU only, Ukrainian layout", &ruua, &a[2], Lang::Ru);
}
