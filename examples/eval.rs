//! Accuracy evaluation: cargo run --release --example eval -- <words_en> <words_ru> [rare_en] [rare_ru]
//! For each list, words typed in the correct layout must NOT switch (false positives),
//! and the same words typed in the wrong layout MUST switch (detection rate).
use openswitch::detect::Detector;
use openswitch::layout::{keys_for_text, Lang};

fn run(d: &Detector, path: &str, lang: Lang, show: usize) {
    let text = std::fs::read_to_string(path).expect("read list");
    let (mut n, mut fp, mut hit) = (0, 0, 0);
    let mut fps = std::collections::HashMap::<String, usize>::new();
    let mut misses = std::collections::HashMap::<String, usize>::new();
    for w in text.lines().filter(|l| !l.is_empty()) {
        let Some(keys) = keys_for_text(w, lang) else { continue };
        n += 1;
        if d.check(&keys, lang).switch {
            fp += 1;
            *fps.entry(w.to_string()).or_default() += 1;
        }
        let other = if lang == Lang::En { Lang::Ru } else { Lang::En };
        if d.check(&keys, other).switch {
            hit += 1;
        } else {
            *misses.entry(w.to_string()).or_default() += 1;
        }
    }
    println!(
        "{path}: n={n} false-switch={:.3}% detect={:.2}%",
        fp as f64 * 100.0 / n as f64,
        hit as f64 * 100.0 / n as f64
    );
    let top = |m: std::collections::HashMap<String, usize>| {
        let mut v: Vec<_> = m.into_iter().collect();
        v.sort_by_key(|x| std::cmp::Reverse(x.1));
        v.into_iter().take(show).map(|(w, c)| format!("{w}:{c}")).collect::<Vec<_>>().join(" ")
    };
    println!("  FP: {}", top(fps));
    println!("  MISS: {}", top(misses));
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let d = Detector::new();
    let langs = [Lang::En, Lang::Ru, Lang::En, Lang::Ru];
    for (p, l) in a.iter().zip(langs) {
        run(&d, p, l, 25);
    }
}
