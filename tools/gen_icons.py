#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Regenerates the icon files from assets/openswitch.svg.

Needs rsvg-convert (librsvg) and Pillow:  apt install librsvg2-bin; pip install pillow
Outputs:
  assets/openswitch.ico  - application icon embedded into the exe (16..256 px)
  assets/icon-64.rgba    - raw 64x64 RGBA for the settings window (no image decoder needed)
Run:  python tools/gen_icons.py
"""
import os, subprocess, tempfile
from PIL import Image

ASSETS = os.path.join(os.path.dirname(__file__), '..', 'assets')
SVG = os.path.join(ASSETS, 'openswitch.svg')
SIZES = [16, 20, 24, 32, 40, 48, 64, 128, 256]

def main():
    with tempfile.TemporaryDirectory() as tmp:
        imgs = []
        for s in SIZES:
            png = os.path.join(tmp, f'{s}.png')
            subprocess.run(['rsvg-convert', '-w', str(s), '-h', str(s), SVG, '-o', png], check=True)
            imgs.append(Image.open(png).convert('RGBA'))
        imgs[-1].save(os.path.join(ASSETS, 'openswitch.ico'), format='ICO',
                      sizes=[(s, s) for s in SIZES], append_images=imgs[:-1])
        with open(os.path.join(ASSETS, 'icon-64.rgba'), 'wb') as f:
            f.write(imgs[SIZES.index(64)].tobytes())
    print('ok')

if __name__ == '__main__':
    main()
