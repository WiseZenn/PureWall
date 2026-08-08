
# PureWall Living Gallery Fusion

## Design intent

PureWall should feel like a living gallery first and a management tool second. The selected wallpaper defines the atmosphere, while controls appear as quiet, precise instruments placed close to the task.

This direction combines:

- Living Gallery's immersive ambient surface.
- Contact Sheet Studio's distinctive `PURE WALL` wordmark.
- Curatorial Stage's strong now-playing hierarchy and attached command dock.

## Screen structure

1. **Brand title bar**
   - `PURE WALL` is treated as a spaced product wordmark, not a generic app label.
   - Search remains central.
   - `Import folder` and `Add images` are persistent first-level actions.

2. **Quiet navigation**
   - The left navigation is a translucent floating rail, not a full-height opaque column.
   - Collection and system navigation remain visible without competing with the wallpaper.

3. **Now-playing stage**
   - The current wallpaper is the environment behind the interface.
   - The title uses a large, restrained display treatment.
   - Essential metadata is presented as compact mono labels.

4. **Command dock**
   - Next is the strongest action.
   - Keep, Hide, Pause, and interval controls form one attached command surface.
   - The dock should collapse in Quiet Canvas.

5. **Curated collection rows**
   - Replace one endless management grid with named horizontal collections.
   - Rows create an editorial rhythm and make the library feel intentionally curated.
   - A dense contact-sheet view can remain available as a secondary mode.

6. **Slide-over inspector**
   - The inspector opens when a wallpaper is selected and slides away when idle.
   - It owns file details, palette, tags, and secondary actions.
   - It must not permanently reduce the main gallery width.

## Typography

- **Primary family:** Geist Variable
- **Metadata and shortcuts:** Geist Mono or a system monospace fallback
- **Wordmark:** Geist, uppercase, medium weight, generous tracking
- **Display title:** Geist, medium weight, tight tracking
- **UI text:** Geist, regular/medium

The intended feel is similar to a refined developer tool: rational, calm, highly readable, and distinctive through spacing and rhythm rather than decorative type.

## Visual rules

- Wallpaper-derived ambient color is allowed, but text surfaces must preserve contrast.
- Use one restrained accent derived from the active wallpaper; the reference uses muted teal.
- Prefer translucent grouped surfaces and negative space over bordered cards.
- Buttons use 11-16px radii depending on size; content surfaces use 14-22px radii.
- Avoid glow-heavy glassmorphism, strong gradients on every action, and equal visual weight across all regions.

## Interaction notes

- Import actions are always available from the title bar.
- Empty library state should promote `Import folder` as the primary action and `Add images` as secondary.
- Selecting a wallpaper opens the inspector without navigating away.
- Clicking the stage or pressing Escape closes the inspector.
- Quiet Canvas removes navigation, collection rows, and inspector while retaining essential playback controls.
- Respect reduced motion and reduced transparency preferences.

## Deliverables

- `PureWall-Living-Gallery-Fusion.svg`: editable vector mockup suitable for direct Figma import.
- `PureWall-Living-Gallery-Fusion.png`: rendered visual reference.
