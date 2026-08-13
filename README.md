<p align="center">
  <img src="src-tauri/icons/icon.png" alt="PureWall logo" width="88" />
</p>

<h1 align="center">PureWall</h1>

<p align="center">A local-first Windows wallpaper library — browse, organize, and rotate the images you already own.</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-2ea44f.svg" alt="MIT License"></a>
  <img src="https://img.shields.io/badge/Windows-10%2F11-0078D4.svg" alt="Windows 10 or 11">
  <img src="https://img.shields.io/badge/Tauri-2-24C8DB.svg" alt="Tauri 2">
  <a href="https://github.com/WiseZenn/PureWall/actions/workflows/ci.yml"><img src="https://github.com/WiseZenn/PureWall/actions/workflows/ci.yml/badge.svg" alt="CI workflow"></a>
  <br>
  <sub>Source build · no published binary release yet</sub>
</p>

<p align="center">
  <img src="docs/images/screenshot-library-dark.webp" alt="PureWall library workspace in dark theme" width="49%">
  <img src="docs/images/screenshot-library-light.webp" alt="PureWall library workspace in light theme" width="49%">
</p>

<p align="center"><em>Real Vue render with mocked Tauri synthetic data; not native WebView2 or release evidence.</em></p>

## Why PureWall

PureWall is a quiet, local Windows workbench for the wallpaper files already on your drives. It stores metadata in SQLite while leaving original images at their existing paths.

| | | |
| --- | --- | --- |
| 🗂️ **Local-first library**<br>Import folders or images without copying originals. | 🎲 **Liked-weighted rotation**<br>Random playback gives liked wallpapers extra weight. |
| 🏷️ **Tags & collections**<br>Organize, hide, search, and batch-edit large libraries. | 🖥️ **Multi-display**<br>Use the same image, span one image, or rotate independently. |
| 🪟 **Floating widget**<br>Keep Next, Like, and Dislike close at hand. | 💾 **Backup & restore**<br>Protect PureWall metadata without rewriting source files. |
| ⏸️ **Focus pause**<br>Pause rotation while a full-screen app has focus. | 📈 **Yearly insights**<br>Review playback history and patterns over time. |

Other built-in controls include light/dark themes, Quiet Canvas, optional HKCU-only context-menu and autostart integrations, and an explicit pause control.

## Built for the files you already own

- **References, not imports:** PureWall indexes source paths and keeps originals in place.
- **Metadata stays local:** ratings, tags, collections, settings, caches, and play history use the local SQLite store in `%APPDATA%\com.purewall.app`.
- **Useful at library scale:** pagination, search, hidden items, batch actions, and cached previews keep everyday browsing focused.
- **Windows-aware playback:** wallpaper application understands same, span, and independent display modes.
- **Optional by design:** widget, focus pause, autostart, and context-menu actions can be enabled only when wanted.

PureWall does not include an online wallpaper feed or recommendation service. PureWall-X and shared-core extraction are intentionally deferred; see [ROADMAP.md](ROADMAP.md).

## At a glance

| Area | Choice |
| --- | --- |
| Desktop shell | Tauri 2 + WebView2 |
| Interface | Vue 3 + TypeScript |
| Native layer | Rust + Windows APIs |
| Persistence | SQLite metadata under the local app-data boundary |
| License | MIT |

The maintainable public architecture is documented in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Project status

The local application foundation and maintainability work are implemented. The first public release loop is prepared but still awaits external evidence: a green hosted Windows CI run, signing configuration, an authorized tag, draft-release verification, and disposable-runner installer smoke. This README does not claim those gates have run.

## Safety Promise

PureWall is designed to be conservative around your files and Windows configuration:

- It writes only PureWall-owned entries under **HKCU** for its optional context menu and autostart features.
- It never writes **HKLM** and never changes Windows 11 shell policy or context-menu settings.
- It deletes only explicitly selected registered wallpaper file(s)—a single file or a validated batch—and uses a project-owned Windows Shell Recycle Bin operation after confirmation. It never recursively deletes folders or auto-confirms permanent deletion; an explicit Windows permanent-delete choice is treated as an unknown outcome and metadata is retained.
- It accepts local fixed-volume Windows paths and rejects UNC, mapped/remote, removable/unknown volumes, reparse-backed paths, and other unsafe paths; rejected paths can be removed through Explorer instead. OneDrive/provider paths are rejected when their reparse/provider boundary is observable. Because Windows Shell deletion is path-bound rather than handle-bound, an active same-user process can still replace a path after final validation and during the Shell window; that residual race is not claimed to be eliminated.

## Quick start

### Install a release

When a verified build is published, install it from [PureWall Releases](https://github.com/WiseZenn/PureWall/releases). There is no published binary release yet; do not treat a local build as release evidence.

### Build from source

Requirements: Windows 10 or 11, Node.js `20.19+` or `22.12+`, Rust stable with the MSVC toolchain, WebView2, and Git.

```powershell
git clone https://github.com/WiseZenn/PureWall.git
cd PureWall
npm ci
npx vue-tsc --noEmit
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri build
```

Launch a development instance with `npm run tauri dev`. Once PureWall opens, choose **Import Folder** or **Import Images**, then select a local folder or files. Supported formats are JPG/JPEG, PNG, BMP, and WebP. PureWall keeps the originals where they are and stores metadata under `%APPDATA%\com.purewall.app`.

For maintainer release checks, signing, draft review, and installer smoke boundaries, see [docs/RELEASING.md](docs/RELEASING.md).

## Screenshots

<p align="center">
  <img src="docs/images/screenshot-inspector-dark.webp" alt="PureWall wallpaper inspector in dark theme" width="49%">
  <img src="docs/images/screenshot-inspector-light.webp" alt="PureWall wallpaper inspector in light theme" width="49%">
</p>
<p align="center">
  <img src="docs/images/screenshot-displays-dark.webp" alt="PureWall display settings in dark theme" width="49%">
  <img src="docs/images/screenshot-displays-light.webp" alt="PureWall display settings in light theme" width="49%">
</p>

- **Inspector** — selected-wallpaper metadata, tags, actions, and playback controls.
- **Displays** — same, span, or independent display modes.

These are real Vue renders with mocked Tauri synthetic data, not native WebView2 or release evidence. Regenerate them with `npm run qa:screenshots`; set `PW_PLAYWRIGHT_PATH` or `PW_CHROME_PATH` when using a separately installed Windows QA toolchain. On Windows, the Chrome fallback is `C:\Program Files\Google\Chrome\Application\chrome.exe`.

## Brand mark

<p align="center">
  <img src="design/purewall-icon-motion.webp" alt="PureWall animated icon mark" width="220">
</p>

The brand mark is a wallpaper card stack with a next arrow—the small visual cue behind PureWall's rotation loop.

## FAQ

### Is PureWall local-only?

Yes. Original images stay at their existing paths. SQLite metadata, settings, caches, and history live under `%APPDATA%\com.purewall.app`; ordinary use does not require an account or cloud library.

### Which image formats and paths are supported?

JPG/JPEG, PNG, BMP, and WebP on local Windows drives. UNC paths and mapped network drives are rejected.

### Does deleting an item permanently delete the file?

No automatic permanent deletion. After confirmation and the pending Undo window, PureWall sends only explicitly selected registered wallpaper file(s)—a single file or a validated batch on a local fixed volume—to a project-owned Windows Shell operation configured for recycling. PureWall rejects observed reparse/provider boundaries, requires observed Recycle Bin evidence before removing metadata, never auto-confirms a permanent-delete prompt, and treats an explicit Windows permanent-delete choice or missing evidence as unknown with metadata retained. Because the Windows Shell operation is path-bound rather than handle-bound, an active same-user process can still replace a path after final validation and during the Shell window; this residual race cannot be eliminated by PureWall.

### Why might Windows SmartScreen warn me?

Source builds and unsigned binaries can trigger reputation warnings. Do not bypass an unexpected warning. For a release artifact, check the release notes, checksum, signer, and provenance when those controls are provided.

### Is the updater ready?

The Tauri updater path is implemented and unit-tested at the state-machine and UI level (pending-update handling, retry after failed download). Signature acceptance/rejection is enforced by the Tauri updater plugin and its mandatory signature checks, but no repository test exercises real cryptographic signatures, and the path has not been exercised against a real signed release. Until a release documents that evidence, prefer downloading a newer release from the matching Releases page and verifying it independently.

## Contributing, support, and security

- [Contributing guide](CONTRIBUTING.md) — development setup, checks, and pull requests.
- [Architecture](docs/ARCHITECTURE.md) — public contributor-facing system overview.
- [Support](SUPPORT.md) — discussions and reproducible bug reports.
- [Security policy](SECURITY.md) — private reporting and safety boundaries.
- [Releases](https://github.com/WiseZenn/PureWall/releases) — published builds when available.

## License

PureWall is released under the [MIT License](LICENSE). See the license text for terms.
