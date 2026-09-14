# Installing PureWall on Windows

PureWall targets 64-bit Windows 10 and Windows 11. It uses Microsoft Edge WebView2, which is present on current Windows installations or can be installed by the Tauri installer bootstrapper.

## Release status

No verified public binary has been published yet. Until a signed release exists, build from source and treat the result as a local unsigned candidate, not as official release evidence.

When the first release is available, download it only from [PureWall Releases](https://github.com/WiseZenn/PureWall/releases). PureWall does not publish a portable executable.

## Choose an installer

| File | Use it when |
| --- | --- |
| `PureWall_*_x64-setup.exe` | You want the normal per-user graphical installer. |
| `PureWall_*_x64_en-US.msi` | You need an MSI for managed or scripted deployment. |

Both formats install the same application. A release is trusted only after its checksum, Authenticode signature, and GitHub provenance match the release notes.

## Verify a release download

Download the installer and `SHA256SUMS.txt` from the same release. In PowerShell, run:

```powershell
Get-FileHash .\PureWall_0.1.0_x64-setup.exe -Algorithm SHA256
Get-Content .\SHA256SUMS.txt
Get-AuthenticodeSignature .\PureWall_0.1.0_x64-setup.exe |
  Format-List Status,StatusMessage,SignerCertificate
gh attestation verify .\PureWall_0.1.0_x64-setup.exe --repo WiseZenn/PureWall
```

The SHA-256 value must match the exact filename. Authenticode must report `Valid` and the signer must match the release notes. Do not bypass an unexpected SmartScreen or signature warning.

## First run

Open PureWall, select **Import Folder** or **Add Images**, and choose files on a local fixed drive. JPG/JPEG, PNG, BMP, and WebP are supported. UNC paths, mapped network drives, and unsafe reparse/provider paths are rejected.

The everyday loop is simple: press **Next** to rotate, then **Like** or **Dislike** the wallpaper currently shown. Liked items receive extra weight during later random rotation.

## Local data and uninstall

PureWall keeps original images where they are. Metadata, settings, history, and preview caches live under `%APPDATA%\com.purewall.app`.

Before uninstalling, disable optional context-menu and autostart integrations in PureWall settings. Then remove PureWall from Windows **Installed apps**. Your original wallpaper files are not part of the installation and are not removed.

Application data may remain after uninstall so a reinstall can recover the library. Back it up before manually removing `%APPDATA%\com.purewall.app`.

## Build from source

Install Node.js `20.19+` or `22.12+`, Rust stable with the MSVC toolchain, Git, and WebView2. Then run:

```powershell
git clone https://github.com/WiseZenn/PureWall.git
cd PureWall
npm ci
npm run test:release-contract
npm run verify:release
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri build
```

Tauri writes the application executable to `src-tauri\target\release\PureWall.exe` and installers below `src-tauri\target\release\bundle\`. Local builds are unsigned unless you own and configure the required signing credentials.

For contributor checks, see [CONTRIBUTING.md](../CONTRIBUTING.md). Maintainers should follow [RELEASING.md](RELEASING.md) before creating or publishing a release.
