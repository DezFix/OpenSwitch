#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Check UI translations: every string used with t("...") / i18n::f("...") must be in the
Russian table (the unit test checks that the Russian and Ukrainian tables have the same keys).
Also lists table entries nobody uses any more.

    python tools/check_i18n.py      (from the repository root; exit code 1 if something is missing)

Keys used indirectly (through a constant) are listed in INDIRECT.
"""
import glob
import re
import sys

STR = r'"((?:[^"\\]|\\.)*)"'
# Keys passed to t() through constants (HOTKEYS, SENSITIVITY_PRESETS).
INDIRECT = {"Fix the last word", "Fix selected text", "Enable/disable auto switching", "Cautious", "Normal", "Bold"}

src = open('src/i18n.rs', encoding='utf-8').read()
ru = src[src.index('const RU'):src.index('const UK')]
keys = set(re.findall(r'\(\s*' + STR + ',', ru))
used = set(INDIRECT)
for f in glob.glob('src/*.rs'):
    if f.replace('\\', '/').endswith('i18n.rs'):
        continue
    s = open(f, encoding='utf-8').read()
    used |= set(re.findall(r'\bt\(\s*' + STR, s))
    used |= set(re.findall(r'i18n::f\(\s*' + STR, s))

missing = sorted(used - keys)
unused = sorted(keys - used)
out = sys.stdout.buffer
for title, items in (("MISSING in the tables:", missing), ("UNUSED in the tables:", unused)):
    if items:
        out.write((title + "\n" + "".join(f"  {k}\n" for k in items)).encode('utf-8'))
if not missing and not unused:
    out.write(b"translations OK\n")
sys.exit(1 if missing else 0)
