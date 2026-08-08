# PureWall Performance and Preview Quality Review

Date: 2026-07-01
Scope: wallpaper preview clarity, thumbnail loading, gallery scroll smoothness, and frontend/backend image handoff.

## Summary

PureWall had two likely causes for the reported blurry preview and occasional stutter:

1. The high-quality preview derivative was limited to 960px wide at JPEG quality 82. That is too small for the main living-stage preview and can look soft when stretched.
2. Startup and several library refresh paths requested all missing thumbnails at once. On large libraries this concentrates image decoding, IPC payload creation, base64 transfer, and reactive `Map` writes into a short burst.

The 2026-07-01 optimization pass moved the app toward visible-range loading and higher-quality derived media without changing registry, uninstall, autostart, system settings, or the SQLite schema.

## Implemented Changes

### Preview Clarity

- Preview derivatives now cap the longest edge at 1440px instead of only capping width at 960px.
- Preview JPEG quality increased from 82 to 88.
- Preview resize continues to use Lanczos3 for higher-quality large surfaces.
- Portrait images are now bounded by height as well as width, preventing tall images from becoming oversized data URLs.

### Thumbnail Quality

- Thumbnail derivatives now cap the longest edge at 384px instead of 256px.
- Thumbnail JPEG quality increased from 60 to 68.
- Thumbnail resize changed from Nearest to CatmullRom to avoid rough photo edges while staying cheaper than Lanczos3.

### Cache Correctness

- Thumbnail and preview cache keys now include a derivative profile string.
- This prevents old 960px previews or 256px thumbnails from being reused after quality settings change.

### Stutter Reduction

- The home screen no longer triggers a full-library thumbnail preload on mount.
- `WallpaperGrid` requests thumbnails only for currently rendered wallpapers.
- Thumbnail requests are chunked into batches of 48 paths.
- Reactive thumbnail-cache writes are chunked into groups of 24 and yield to the next animation frame between chunks.
- Duplicate in-flight thumbnail and preview requests are de-duplicated in the store.

## Verification

- `cargo fmt --check` passed.
- `cargo check` passed with elevation after sandbox target write errors.
- `cargo test` passed with 8 tests, including new thumbnail/preview dimension tests.
- `cargo clippy --all-targets -- -D warnings` passed with elevation after sandbox target write errors.
- `npx vue-tsc --noEmit` passed.
- `npm run build` passed with elevation after the known Vite/Rolldown sandbox `spawn EPERM` failure.
- `git diff --check` passed for the touched performance and documentation files.

## Remaining Work

1. Test with a real large wallpaper library on Windows and record perceived scroll smoothness, startup time, and first-preview latency.
2. Add idle-time background warming for thumbnails beyond the visible range, with a cancellation token when filter/sort/search changes.
3. Add a bounded disk-cache cleanup policy for derived images, probably based on last access time or total cache size.
4. Consider replacing base64 data URLs with safer local asset URLs or a Tauri custom protocol if memory pressure becomes visible in very large libraries.
5. Add a lightweight performance checklist to release testing: cold start, filter switch, rapid selection changes, and import of a large folder.
## 2026-07-01 Startup Stutter Follow-up

Packaged-app testing showed severe startup stutter before other flows could be evaluated. The likely cause was a cold-cache burst created by the previous quality pass: the new cache profile invalidated old thumbnail files, while the gallery still rendered 120 initial cards and could decode 48 thumbnails per IPC batch.

Follow-up fix:

- Initial rendered wallpaper cards reduced to 36.
- Scroll render increments reduced to 36.
- Visible-thumbnail loading delayed by 160ms after the gallery render pass.
- Thumbnail IPC batch size reduced to 12.
- Reactive thumbnail cache write chunks reduced to 6.
- The store yields before the first thumbnail batch and between later batches.
- Startup active-preview loading is delayed by 300ms.
- Thumbnail derivative profile adjusted to 320px, JPEG quality 64, Triangle filtering.

Retest the regenerated NSIS installer first. A successful result is that the main window becomes responsive before thumbnails finish appearing, with thumbnails progressively filling in rather than freezing the whole app.

## 2026-07-01 Phase 1 Architecture Refactor

Phase 1 replaced the highest-cost image handoff path. Thumbnail and preview commands now return generated cache file paths, and the frontend converts those paths with Tauri `convertFileSrc()` so the WebView loads images through the asset protocol instead of receiving Base64 strings over IPC.

Implemented changes:

- Enabled Tauri asset protocol for the PureWall-owned derivative cache only: `$APPDATA/com.purewall.app/thumbnails/**/*`.
- Kept source wallpaper directories out of asset scope to avoid broad local-file exposure.
- Removed direct Rust Base64 encoding from the thumbnail/preview path.
- Added `@vueuse/core` and virtualized gallery rows so DOM size is bounded by visible rows plus overscan instead of total library size.
- Regenerated NSIS/MSI installers after verification.

Expected effect:

- Lower IPC payload size for thumbnail and preview loading.
- Less JS heap pressure because image bytes no longer live as large Base64 strings in reactive state.
- Better browser/WebView image-cache behavior through normal `<img src="asset://...">` loading.
- Smoother large-library scrolling because the gallery no longer mounts every rendered wallpaper card as the user reaches the bottom.

Retest focus:

1. Cold start with a large real library.
2. First visible thumbnail fill-in time.
3. Scroll through several thousand wallpapers in grid, compact, and list modes.
4. Select wallpapers rapidly and confirm the high-quality preview still appears.
5. Confirm generated thumbnails load in the packaged installer, not only in dev mode.

Remaining architecture work:

- Phase 2: move derivative generation to a fully on-demand/background model and revisit WebP/512px or 720px quality settings.
- Phase 3: re-audit the focus monitor loop; the rotation timer already uses a `Condvar`, but the focus monitor still sleeps every 2 seconds.

## 2026-07-01 Phase 2/3 Follow-up

Implemented after the asset-protocol refactor:

- Cold thumbnail cache misses no longer block `load_thumbnails_batch`.
- The command now returns only existing cache hits and queues missing thumbnails for a single background worker.
- The worker emits `thumbnail-generated` after each derivative is atomically written; the frontend listens and fills the thumbnail cache with an asset URL.
- Thumbnail derivatives now use a 512px longest edge at JPEG quality 76 with CatmullRom sampling, improving visible card clarity while avoiding synchronous UI blocking.
- Focus monitoring no longer wakes every 2 seconds when focus mode is disabled. The monitor blocks on a signal and only uses a 2-second detection timeout while focus mode is enabled.

Why WebP is still pending:

- The currently installed `image` crate supports WebP encoding only through a lossless path.
- The originally requested lossy WebP quality target needs a libwebp-backed encoder dependency, which should be reviewed separately for Windows build compatibility and release footprint before adding it to an open-source app.

Retest focus:

1. Delete or ignore the old thumbnail cache profile and launch with many wallpapers.
2. Confirm the window is interactive before thumbnails finish appearing.
3. Confirm cards progressively fill as the background worker emits generated thumbnails.
4. Toggle focus mode off and confirm idle CPU stays quiet; toggle it on and confirm fullscreen auto-pause still works.

## 2026-07-13 Progressive Preview Pipeline Verification

### Measurement method

- Platform: Windows; optimized Rust release build.
- Test: ignored `preview_performance_tests::report_real_preview_pipeline_timings`.
- Command shape: `PUREWALL_PERF_IMAGES=<path|path|path> cargo test --release report_real_preview_pipeline_timings -- --ignored --nocapture --test-threads=1`.
- Samples were read from the existing PureWall library database. Source files were read-only; generated derivatives used a unique system temporary directory that the test removed after each sample.
- `cold WIC` measures target-sized WIC decode, JPEG q92 encode, and atomic cache write through the production derivative boundary.
- `cache hit` measures production preview cache lookup/touch through the same boundary.
- `forced image` measures full `image::open`, Lanczos3 target resize, JPEG q92 encode, and temporary output write.
- Samples ran sequentially to avoid hiding latency behind concurrent disk/CPU work.

### Results

| Sample | Dimensions | Source | Cold WIC | Cache hit | Forced `image` | WIC preview | Fallback preview |
|---|---:|---:|---:|---:|---:|---:|---:|
| Normal JPEG | 6880×3607 (24.8MP) | 15.7MiB | 200.5ms | 0.244ms | 506.8ms | 279.2KiB | 302.8KiB |
| Very large JPEG | 15433×7291 (112.5MP) | 86.0MiB | 854.0ms | 0.284ms | 3154.3ms | 343.6KiB | 372.5KiB |
| Very large JPEG | 11648×7765 (90.4MP) | 85.4MiB | 1155.3ms | 0.580ms | 2639.5ms | 417.1KiB | 496.8KiB |

### Interpretation

- WIC reduced measured cold derivative latency by about 2.5× for 24.8MP, 3.7× for 112.5MP, and 2.3× for 90.4MP versus the forced full-decode fallback on this machine.
- Cache-hit lookup remained below 0.6ms for all three samples, supporting the startup strategy of restoring the persisted current preview before gallery pagination.
- The 112.5MP sample completed under one second through WIC instead of more than three seconds through the forced fallback. The 90.4MP sample was just over one second through WIC, so the thumbnail/stale fallback remains necessary on a first-ever cold image.
- WIC and `image` preview byte sizes differ because their scaling paths are not pixel-identical. Both enforce the same 1440px longest edge and JPEG q92 contract.

### Verified behavior

- A current-format cache hit returns without decode.
- A cold current wallpaper returns its thumbnail or compatible stale preview immediately and queues the current 1440px derivative.
- WIC failures automatically fall back to `image`; the user-facing preview retains its thumbnail on failure.
- Thumbnail and preview needs for the same source merge into one decode request.
- One worker lane remains reserved for direct preview work; two likely-next previews are bounded to general workers and are evicted by new active intent.
- Browser presentation switches from fallback to preview only after the matching `Image.load` callback.

### Remaining measurement work

- Record peak process working set and GPU/WebView memory during cold 100MP selection before defining a formal memory budget.
- Run rendered Tauri/WebView QA for cold start, rapid selection, quiet canvas, and inspector transitions using the user's packaged build.
- Repeat on at least one lower-spec Windows machine; these timings are evidence for relative improvement, not universal latency guarantees.
