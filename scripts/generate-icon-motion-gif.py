from __future__ import annotations

from pathlib import Path
from PIL import Image, ImageDraw, ImageStat

ROOT = Path(__file__).resolve().parents[1]
OUT_GIF = ROOT / "design" / "purewall-icon-motion.gif"
OUT_APNG = ROOT / "design" / "purewall-icon-motion.apng"
OUT_WEBP = ROOT / "design" / "purewall-icon-motion.webp"
OUT_STRIP = ROOT / "design" / "purewall-icon-motion-strip.png"

SCALE = 4
SIZE = 256
CANVAS = SIZE * SCALE
DURATION_MS = 920
FRAME_MS = 40
HOLD_FRAMES = 8

COLORS = {
    "bg_top": (251, 253, 255, 255),
    "bg_bottom": (237, 246, 255, 255),
    "border": (211, 229, 247, 255),
    "back_card": (227, 241, 255, 255),
    "mid_card": (251, 253, 255, 255),
    "mid_stroke": (214, 230, 247, 255),
    "photo_top": (107, 184, 241, 255),
    "photo_mid": (184, 220, 248, 255),
    "photo_bottom": (244, 250, 255, 255),
    "snow": (255, 255, 255, 246),
    "ridge": (74, 111, 145, 158),
    "ink": (74, 111, 145, 255),
    "accent_top": (35, 139, 230, 255),
    "accent_bottom": (9, 111, 200, 255),
}


def s(value: float) -> int:
    return round(value * SCALE)


def ease_out_cubic(t: float) -> float:
    t = max(0.0, min(1.0, t))
    return 1 - (1 - t) ** 3


def smoothstep(t: float) -> float:
    t = max(0.0, min(1.0, t))
    return t * t * (3 - 2 * t)


def between(t: float, start: float, end: float, ease=ease_out_cubic) -> float:
    if t <= start:
        return 0.0
    if t >= end:
        return 1.0
    return ease((t - start) / (end - start))


def gradient(size: tuple[int, int], stops: list[tuple[float, tuple[int, int, int, int]]]) -> Image.Image:
    width, height = size
    img = Image.new("RGBA", size)
    px = img.load()
    stops = sorted(stops, key=lambda item: item[0])
    for y in range(height):
        p = y / max(1, height - 1)
        for idx in range(len(stops) - 1):
            a_pos, a_col = stops[idx]
            b_pos, b_col = stops[idx + 1]
            if a_pos <= p <= b_pos:
                local = 0 if b_pos == a_pos else (p - a_pos) / (b_pos - a_pos)
                col = tuple(round(a_col[i] + (b_col[i] - a_col[i]) * local) for i in range(4))
                break
        else:
            col = stops[-1][1]
        for x in range(width):
            px[x, y] = col
    return img


def lerp_color(a: tuple[int, int, int, int], b: tuple[int, int, int, int], t: float) -> tuple[int, int, int, int]:
    t = max(0.0, min(1.0, t))
    return tuple(round(a[i] + (b[i] - a[i]) * t) for i in range(4))


def vertical_gradient(size: tuple[int, int], top_y: int, bottom_y: int, top: tuple[int, int, int, int], bottom: tuple[int, int, int, int]) -> Image.Image:
    width, height = size
    img = Image.new("RGBA", size)
    draw = ImageDraw.Draw(img)
    span = max(1, bottom_y - top_y)
    for y in range(height):
        col = lerp_color(top, bottom, (y - top_y) / span)
        draw.line((0, y, width, y), fill=col)
    return img


def rounded_mask(size: tuple[int, int], radius: int) -> Image.Image:
    mask = Image.new("L", size, 0)
    draw = ImageDraw.Draw(mask)
    draw.rounded_rectangle((0, 0, size[0] - 1, size[1] - 1), radius=radius, fill=255)
    return mask


def with_opacity(layer: Image.Image, opacity: float) -> Image.Image:
    if opacity >= 0.999:
        return layer
    out = layer.copy()
    alpha = out.getchannel("A").point(lambda value: round(value * max(0.0, min(1.0, opacity))))
    out.putalpha(alpha)
    return out


def composite_group(canvas: Image.Image, group: Image.Image, opacity: float = 1.0, scale: float = 1.0, dx: float = 0, dy: float = 0) -> None:
    bbox = group.getbbox()
    if bbox is None or opacity <= 0:
        return
    if abs(scale - 1.0) > 0.001:
        resized = group.resize((round(CANVAS * scale), round(CANVAS * scale)), Image.Resampling.LANCZOS)
        x = round((CANVAS - resized.width) / 2 + dx * SCALE)
        y = round((CANVAS - resized.height) / 2 + dy * SCALE)
        canvas.alpha_composite(with_opacity(resized, opacity), (x, y))
    else:
        canvas.alpha_composite(with_opacity(group, opacity), (round(dx * SCALE), round(dy * SCALE)))


def draw_base() -> Image.Image:
    layer = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    rect = (s(8), s(8), s(248), s(248))
    mask = rounded_mask((s(240), s(240)), s(54))
    fill = gradient((s(240), s(240)), [(0, COLORS["bg_top"]), (1, COLORS["bg_bottom"])])
    layer.paste(fill, (rect[0], rect[1]), mask)
    draw = ImageDraw.Draw(layer)
    draw.rounded_rectangle(rect, radius=s(54), outline=COLORS["border"], width=s(3))
    return layer


def draw_back_card() -> Image.Image:
    layer = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    draw = ImageDraw.Draw(layer)
    draw.rounded_rectangle((s(43), s(48), s(171), s(172)), radius=s(28), fill=COLORS["back_card"])
    return layer


def draw_mid_card() -> Image.Image:
    layer = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    draw = ImageDraw.Draw(layer)
    draw.rounded_rectangle((s(56), s(57), s(184), s(181)), radius=s(28), fill=COLORS["mid_card"], outline=COLORS["mid_stroke"], width=s(3))
    return layer


def draw_wallpaper() -> Image.Image:
    layer = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    rect = (s(70), s(66), s(194), s(190))
    w, h = s(124), s(124)
    mask = rounded_mask((w, h), s(28))
    photo = gradient((w, h), [(0, COLORS["photo_top"]), (.56, COLORS["photo_mid"]), (1, COLORS["photo_bottom"])])
    layer.paste(photo, (rect[0], rect[1]), mask)

    content = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    draw = ImageDraw.Draw(content)
    draw.ellipse((s(90), s(85), s(112), s(107)), fill=COLORS["accent_top"])
    draw.polygon([(s(70), s(144)), (s(103), s(116)), (s(132), s(143)), (s(154), s(124)), (s(194), s(158)), (s(194), s(190)), (s(70), s(190))], fill=COLORS["snow"])
    draw.polygon([(s(70), s(160)), (s(110), s(134)), (s(140), s(158)), (s(160), s(144)), (s(194), s(166)), (s(194), s(190)), (s(70), s(190))], fill=COLORS["ridge"])
    clip = Image.new("L", (CANVAS, CANVAS), 0)
    clip.paste(mask, (rect[0], rect[1]))
    content.putalpha(Image.composite(content.getchannel("A"), Image.new("L", (CANVAS, CANVAS), 0), clip))
    layer.alpha_composite(content)

    draw = ImageDraw.Draw(layer)
    draw.rounded_rectangle(rect, radius=s(28), outline=COLORS["ink"], width=s(6))
    return layer


def draw_chevron() -> Image.Image:
    mask = Image.new("L", (CANVAS, CANVAS), 0)
    draw = ImageDraw.Draw(mask)
    p0 = (s(188), s(111))
    p1 = (s(207), s(128))
    p2 = (s(188), s(145))
    width = s(13)
    radius = width // 2
    draw.line([p0, p1, p2], fill=255, width=width, joint="curve")
    for point in (p0, p1, p2):
        draw.ellipse((point[0] - radius, point[1] - radius, point[0] + radius, point[1] + radius), fill=255)

    fill = vertical_gradient((CANVAS, CANVAS), s(100), s(170), COLORS["accent_top"], COLORS["accent_bottom"])
    layer = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    layer.paste(fill, (0, 0), mask)
    return layer


BASE = draw_base()
BACK = draw_back_card()
MID = draw_mid_card()
WALLPAPER = draw_wallpaper()
CHEVRON = draw_chevron()


def frame_at(ms: int) -> Image.Image:
    t = ms / DURATION_MS
    canvas = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    base = between(t, 0.0, 0.18, smoothstep)
    composite_group(canvas, BASE, opacity=base, scale=0.93 + 0.07 * base)
    back = between(t, 0.10, 0.48)
    composite_group(canvas, BACK, opacity=back, dy=16 * (1 - back))
    mid = between(t, 0.18, 0.58)
    composite_group(canvas, MID, opacity=mid, dx=-6 * (1 - mid), dy=14 * (1 - mid))
    current = between(t, 0.28, 0.72)
    overshoot = 1.5 * (1 - abs(current - .86) / .86) if current > .72 else 0
    composite_group(canvas, WALLPAPER, opacity=current, dx=-20 * (1 - current) + max(0, overshoot))
    chev = between(t, 0.58, 0.92)
    composite_group(canvas, CHEVRON, opacity=chev, dx=-14 * (1 - chev) + 2 * max(0, 1 - abs(chev - .75) / .75))
    return canvas.resize((SIZE, SIZE), Image.Resampling.LANCZOS)


def flatten_for_gif(frame: Image.Image) -> Image.Image:
    matte = Image.new("RGBA", frame.size, (255, 255, 255, 255))
    matte.alpha_composite(frame.convert("RGBA"))
    return matte.convert("RGB")


def build_global_palette(frames: list[Image.Image]) -> Image.Image:
    sheet = Image.new("RGB", (SIZE * len(frames), SIZE), (255, 255, 255))
    for idx, frame in enumerate(frames):
        sheet.paste(flatten_for_gif(frame), (idx * SIZE, 0))
    return sheet.quantize(colors=256, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.FLOYDSTEINBERG)


def quantize_for_gif(frames: list[Image.Image]) -> list[Image.Image]:
    palette = build_global_palette(frames)
    return [flatten_for_gif(frame).quantize(palette=palette, dither=Image.Dither.FLOYDSTEINBERG) for frame in frames]


def sky_unique_colors(frame: Image.Image) -> int:
    crop = frame.convert("RGBA").crop((78, 74, 186, 128))
    raw = crop.tobytes()
    colors = set()
    for idx in range(0, len(raw), 4):
        if raw[idx + 3] > 180:
            colors.add((raw[idx], raw[idx + 1], raw[idx + 2]))
    return len(colors)


def sky_luma_std(frame: Image.Image) -> float:
    crop = frame.convert("RGBA").crop((78, 74, 186, 128)).convert("L")
    return round(ImageStat.Stat(crop).stddev[0], 3)


def save_outputs(frames: list[Image.Image]) -> None:
    OUT_GIF.parent.mkdir(parents=True, exist_ok=True)
    gif_frames = quantize_for_gif(frames)
    gif_frames[0].save(OUT_GIF, save_all=True, append_images=gif_frames[1:], duration=FRAME_MS, loop=0, disposal=2, optimize=False)
    frames[0].save(OUT_APNG, save_all=True, append_images=frames[1:], duration=FRAME_MS, loop=0, disposal=2)
    frames[0].save(OUT_WEBP, save_all=True, append_images=frames[1:], duration=FRAME_MS, loop=0, lossless=True, quality=100, method=6)


def save_strip() -> None:
    strip_samples = [0, 160, 360, 560, 740, 920]
    strip_frames = [frame_at(ms) for ms in strip_samples]
    gap = 10
    strip = Image.new("RGBA", (SIZE * len(strip_frames) + gap * (len(strip_frames) - 1), SIZE), (255, 255, 255, 0))
    x = 0
    for frame in strip_frames:
        strip.alpha_composite(frame, (x, 0))
        x += SIZE + gap
    strip.save(OUT_STRIP)


def main() -> None:
    frames = [frame_at(ms) for ms in range(0, DURATION_MS + 1, FRAME_MS)]
    frames.extend([frames[-1]] * HOLD_FRAMES)
    save_outputs(frames)
    save_strip()
    final_rgba = frames[min(len(frames) - 1, DURATION_MS // FRAME_MS)]
    with Image.open(OUT_GIF) as gif:
        gif.seek(min(gif.n_frames - 1, DURATION_MS // FRAME_MS))
        gif_final = gif.convert("RGBA")
    print(f"Generated {OUT_GIF}")
    print(f"Generated {OUT_APNG}")
    print(f"Generated {OUT_WEBP}")
    print(f"Generated {OUT_STRIP}")
    print(f"Sky unique colors: RGBA/APNG source={sky_unique_colors(final_rgba)}, GIF={sky_unique_colors(gif_final)}")
    print(f"Sky luma stddev: RGBA/APNG source={sky_luma_std(final_rgba)}, GIF={sky_luma_std(gif_final)}")


if __name__ == "__main__":
    main()
