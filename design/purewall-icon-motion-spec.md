# PureWall Icon Motion Spec

## Personality

- Words: calm, selective, effortless.
- Preset: Trustworthy / Professional with a light gallery-app softness.
- Duration: 920ms reveal.
- Principles: Staging, Slow In/Slow Out, Timing, Follow Through, Appeal.

## Part Inventory

- `#base-plate`: stable app container.
- `#back-card`: older wallpaper in the library stack.
- `#mid-card`: queued wallpaper card.
- `#wallpaper-card`: current wallpaper thumbnail.
- `#wallpaper-sun`, `#wallpaper-snow`, `#wallpaper-ridge`: simplified wallpaper content.
- `#next-chevron`: PureWall's core action, switching to the next wallpaper.

## Timeline

| Time | Part | Action |
|---:|---|---|
| 0-160ms | base plate | Softly appears as the stable app container. |
| 120-440ms | back and mid cards | Stagger upward into a visible library stack. |
| 260-660ms | current wallpaper card | Slides into focus and settles on top. |
| 560-920ms | next chevron | Enters last, confirming the wallpaper switching action. |

## Notes

This version intentionally favors recognizability over abstraction, with a softer wallpaper-sky palette: the mountain sky is lighter and less saturated, the large outer border recedes, and the front card outline is a thinner neutral blue-gray. The stack says "wallpaper library"; the front scenic card says "desktop wallpaper"; the blue chevron remains clearer than the scenery so "next/switch/manage" still reads. The motion preview remains a lightweight design artifact with Pixel2Motion-style QA hooks rather than a full pixel2motion QA package. GIF output is kept as a compatibility preview with a shared palette and dithering; APNG and WebP are the preferred high-quality motion previews for smooth sky gradients.
