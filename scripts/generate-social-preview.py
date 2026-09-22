#!/usr/bin/env python3
"""Generate the PureWall GitHub social preview (1200x630) with PIL.

Usage:
    python scripts/generate-social-preview.py [--out .github/social-preview.png]

Design tokens mirror src/styles.css dark theme:
    --bg-app #0B0F14  --bg-sidebar #10161D  --bg-panel #141A21
    --accent #16D6C2  --text-primary #F3F6F8  --text-secondary #A8B0B8
Reproducible: rerun after brand changes.
"""
from __future__ import annotations

import argparse
import math
import os
import sys

from PIL import Image, ImageDraw, ImageFilter, ImageFont

W, H = 1200, 630
ACCENT = (22, 214, 194)        # #16D6C2
ACCENT_DIM = (13, 140, 128)
BG_TOP = (11, 15, 20)          # #0B0F14
BG_BOTTOM = (16, 22, 29)       # #10161D
TEXT_PRIMARY = (243, 246, 248) # #F3F6F8
TEXT_SECONDARY = (168, 176, 184)  # #A8B0B8
TEXT_MUTED = (111, 122, 132)   # #6F7A84

FONT_DIR = r"C:\Windows\Fonts"
if not os.path.isdir(FONT_DIR) and os.path.isdir(r"/mnt/c/Windows/Fonts"):
    FONT_DIR = r"/mnt/c/Windows/Fonts"


def font(size: int, bold: bool = False, light: bool = False) -> ImageFont.FreeTypeFont:
    name = "segoeui.ttf"
    if bold:
        name = "segoeuib.ttf"
    elif light:
        name = "segoeuil.ttf"
    path = os.path.join(FONT_DIR, name)
    if not os.path.exists(path):
        # Fallback chain
        for alt in ("segoeui.ttf", "arialbd.ttf", "arial.ttf"):
            p = os.path.join(FONT_DIR, alt)
            if os.path.exists(p):
                path = p
                break
    return ImageFont.truetype(path, size)


def vertical_gradient(w: int, h: int, top, bottom) -> Image.Image:
    base = Image.new("RGB", (w, h))
    d = ImageDraw.Draw(base)
    for y in range(h):
        t = y / max(1, h - 1)
        color = tuple(int(top[i] + (bottom[i] - top[i]) * t) for i in range(3))
        d.line([(0, y), (w, y)], fill=color)
    return base


def glow(size: int, color, radius_ratio: float = 0.42) -> Image.Image:
    """Soft radial glow layer."""
    layer = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    r = int(size * radius_ratio)
    cx = cy = size // 2
    for i in range(r, 0, -1):
        t = i / r
        alpha = int(60 * (1 - t) * (1 - t))
        rr = int(3 + (1 - t) * r)
        d.ellipse(
            [cx - rr, cy - rr, cx + rr, cy + rr],
            fill=(color[0], color[1], color[2], alpha),
        )
    return layer.filter(ImageFilter.GaussianBlur(60))


def text_width(d: ImageDraw.ImageDraw, text: str, f: ImageFont.FreeTypeFont) -> int:
    bbox = d.textbbox((0, 0), text, font=f)
    return bbox[2] - bbox[0]


def text_height(d: ImageDraw.ImageDraw, text: str, f: ImageFont.FreeTypeFont) -> int:
    bbox = d.textbbox((0, 0), text, font=f)
    return bbox[3] - bbox[1]


def draw_dots_beam(d: ImageDraw.ImageDraw, x: int, y: int, n: int = 3) -> None:
    """Small accent dot trail accenting the logo."""
    for i in range(n):
        dx = x + i * 16
        r = 2.5 if i == 0 else 2.0
        d.ellipse([dx - r, y - r, dx + r, y + r], fill=ACCENT)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=".github/social-preview.png")
    ap.add_argument("--icon", default="src-tauri/icons/icon.png")
    args = ap.parse_args()

    repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    out_path = os.path.join(repo_root, args.out)
    icon_path = os.path.join(repo_root, args.icon)

    canvas = vertical_gradient(W, H, BG_TOP, BG_BOTTOM).convert("RGBA")

    # Accent glows: top-right teal + bottom-left subtle.
    g1 = glow(900, ACCENT).resize((900, 900))
    canvas.paste(g1, (W - 520, -220), g1)
    g2 = glow(700, ACCENT_DIM)
    canvas.paste(g2, (-260, H - 420), g2)

    d = ImageDraw.Draw(canvas)

    # Subtle top hairline accent.
    d.rectangle([0, 0, W, 4], fill=ACCENT)

    # ---- Left column: icon + tagline ----
    icon = Image.open(icon_path).convert("RGBA")
    icon_size = 200
    icon = icon.resize((icon_size, icon_size), Image.LANCZOS)
    icon_x, icon_y = 96, 120

    # Soft backing disc behind the icon.
    disc = Image.new("RGBA", (icon_size + 60, icon_size + 60), (0, 0, 0, 0))
    dd = ImageDraw.Draw(disc)
    dd.ellipse([30, 30, 30 + icon_size, 30 + icon_size], fill=(18, 25, 33, 200))
    disc = disc.filter(ImageFilter.GaussianBlur(12))
    canvas.paste(disc, (icon_x - 30, icon_y - 30), disc)
    canvas.paste(icon, (icon_x, icon_y), icon)

    draw_dots_beam(d, icon_x + icon_size + 18, icon_y + icon_size // 2)

    # ---- Right column: wordmark ----
    tx = icon_x + icon_size + 70
    ty = icon_y + 20

    title_f = font(72, bold=True)
    d.text((tx, ty), "PureWall", font=title_f, fill=TEXT_PRIMARY)

    tag_f = font(30, light=True)
    tag = "A local-first Windows wallpaper library"
    d.text((tx, ty + 100), tag, font=tag_f, fill=TEXT_SECONDARY)

    sub_f = font(22)
    sub = "Browse, organize, and rotate the images you already own."
    d.text((tx, ty + 148), sub, font=sub_f, fill=TEXT_MUTED)

    # ---- Feature chips (bottom) ----
    chips = ["Like-weighted rotation", "Tags & collections", "Multi-display", "Floating widget"]
    chip_y = H - 110
    cx = tx
    for label in chips:
        pad_x, pad_y = 18, 9
        w = text_width(d, label, sub_f) + pad_x * 2
        h = text_height(d, label, sub_f) + pad_y * 2
        d.rounded_rectangle([cx, chip_y, cx + w, chip_y + h], radius=h // 2,
                            outline=(60, 76, 90, 255), width=1)
        d.text((cx + pad_x, chip_y + pad_y - 4), label, font=sub_f, fill=TEXT_SECONDARY)
        cx += w + 14

    # ---- Footer ----
    foot_f = font(18)
    footer = "Tauri 2  ·  Vue 3  ·  Rust  ·  SQLite  ·  Windows 10/11"
    fw = text_width(d, footer, foot_f)
    d.text(((W - fw) // 2, H - 52), footer, font=foot_f, fill=TEXT_MUTED)

    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    canvas.convert("RGB").save(out_path, "PNG")
    print(f"Wrote {out_path} ({W}x{H})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
