#!/usr/bin/env python3
"""Generates OpenSwitch language data from the `wordfreq` package (pip install wordfreq).

Outputs into data/:
  en_words.txt / ru_words.txt  - dictionary, one word per line, most frequent first
  en_tri.bin  / ru_tri.bin     - dense table of i16 log-probabilities (x100) of
                                 P(c3 | c1 c2) over the language alphabet + word boundary
Run:  python tools/gen_data.py
"""
import math, os, re, struct, sys
import wordfreq

OUT = os.path.join(os.path.dirname(__file__), '..', 'data')
DICT_SIZE = int(os.environ.get('DICT_SIZE', 60000))
MODEL_WORDS = 150000

LANGS = {
    # boundary symbol is index 0
    'en': ' ' + "abcdefghijklmnopqrstuvwxyz'",
    'ru': ' ' + 'абвгдежзийклмнопрстуфхцчшщъыьэюя',
}
WORD_RE = {
    'en': re.compile(r"^[a-z]+(?:'[a-z]+)?$"),
    'ru': re.compile(r'^[а-я]+$'),
}

def norm(lang, w):
    w = w.lower()
    if lang == 'ru':
        w = w.replace('ё', 'е')
    else:
        w = w.replace('’', "'")
    return w

def main():
    os.makedirs(OUT, exist_ok=True)
    for lang, alpha in LANGS.items():
        rx = WORD_RE[lang]
        words, seen = [], set()
        for w in wordfreq.iter_wordlist(lang, wordlist='large'):
            w = norm(lang, w)
            if w in seen or not rx.match(w):
                continue
            seen.add(w)
            words.append(w)
            if len(words) >= MODEL_WORDS:
                break
        with open(os.path.join(OUT, f'{lang}_words.txt'), 'w', encoding='utf-8', newline='\n') as f:
            f.write('\n'.join(words[:DICT_SIZE]) + '\n')

        n = len(alpha)
        idx = {c: i for i, c in enumerate(alpha)}
        uni = [0.0] * n
        bi = [[0.0] * n for _ in range(n)]
        tri = {}
        for w in words:
            weight = max(wordfreq.zipf_frequency(w, lang, wordlist='large'), 0.5)
            s = [0, 0] + [idx[c] for c in w] + [0]
            for i in range(2, len(s)):
                a, b, c = s[i - 2], s[i - 1], s[i]
                uni[c] += weight
                bi[b][c] += weight
                tri[(a, b, c)] = tri.get((a, b, c), 0.0) + weight
        tot_uni = sum(uni)
        bi_tot = [sum(r) for r in bi]
        tri_tot = {}
        for (a, b, c), v in tri.items():
            tri_tot[(a, b)] = tri_tot.get((a, b), 0.0) + v
        out = bytearray()
        for a in range(n):
            for b in range(n):
                tt = tri_tot.get((a, b), 0.0)
                for c in range(n):
                    pu = uni[c] / tot_uni
                    pb = bi[b][c] / bi_tot[b] if bi_tot[b] else 0.0
                    pt = tri.get((a, b, c), 0.0) / tt if tt else 0.0
                    p = 0.70 * pt + 0.25 * pb + 0.04 * pu + 0.01 / n
                    out += struct.pack('<h', max(-32000, int(round(math.log(p) * 100))))
        with open(os.path.join(OUT, f'{lang}_tri.bin'), 'wb') as f:
            f.write(out)
        print(lang, 'words', len(words), 'alphabet', n, 'table bytes', len(out))

if __name__ == '__main__':
    main()
