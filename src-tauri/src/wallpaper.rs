use anyhow::{Context, Result};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct DisplayInfo {
    pub index: u32,
    pub id: String,
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
    pub current_wallpaper: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallpaperPlacement {
    Fill,
    Span,
}

fn canonical_path(path: &str) -> Result<String> {
    let abs_path = Path::new(path)
        .canonicalize()
        .context("Failed to resolve wallpaper path")?;
    Ok(abs_path.to_string_lossy().to_string())
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn set_wallpaper_all(path: &str, placement: WallpaperPlacement) -> Result<()> {
    let path_str = canonical_path(path)?;

    #[cfg(windows)]
    {
        match set_wallpaper_all_com(path_str.clone(), placement) {
            Ok(()) => Ok(()),
            Err(err) if placement == WallpaperPlacement::Fill => {
                set_wallpaper_system_parameters(&path_str)
                    .with_context(|| format!("IDesktopWallpaper failed first: {err}"))
            }
            Err(err) => Err(err),
        }
    }

    #[cfg(not(windows))]
    {
        let _ = placement;
        let _ = path_str;
        anyhow::bail!("Wallpaper setting is only supported on Windows");
    }
}

pub fn set_wallpaper_on_monitor(monitor_id: &str, path: &str) -> Result<()> {
    let path_str = canonical_path(path)?;

    #[cfg(windows)]
    {
        sta_worker().set_monitor(monitor_id.to_string(), path_str)
    }

    #[cfg(not(windows))]
    {
        let _ = monitor_id;
        let _ = path_str;
        anyhow::bail!("Per-monitor wallpaper is only supported on Windows");
    }
}

pub fn get_displays() -> Result<Vec<DisplayInfo>> {
    #[cfg(windows)]
    {
        sta_worker().get_displays()
    }

    #[cfg(not(windows))]
    {
        Ok(Vec::new())
    }
}

#[cfg(windows)]
fn set_wallpaper_all_com(path_str: String, placement: WallpaperPlacement) -> Result<()> {
    sta_worker().set_all(path_str, placement)
}

#[cfg(windows)]
fn create_desktop_wallpaper() -> Result<windows::Win32::UI::Shell::IDesktopWallpaper> {
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
    use windows::Win32::UI::Shell::DesktopWallpaper;

    unsafe {
        CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL)
            .context("Failed to create IDesktopWallpaper")
    }
}

#[cfg(windows)]
fn pwstr_to_string_and_free(value: windows::core::PWSTR) -> Result<String> {
    use windows::Win32::System::Com::CoTaskMemFree;

    if value.is_null() {
        return Ok(String::new());
    }

    let text = unsafe { value.to_string().context("Failed to read COM string")? };
    unsafe {
        CoTaskMemFree(Some(value.as_ptr() as *const _));
    }
    Ok(text)
}

#[cfg(windows)]
enum StaRequest {
    SetAll {
        path: String,
        placement: WallpaperPlacement,
        reply: std::sync::mpsc::Sender<Result<()>>,
    },
    SetMonitor {
        monitor_id: String,
        path: String,
        reply: std::sync::mpsc::Sender<Result<()>>,
    },
    GetDisplays {
        reply: std::sync::mpsc::Sender<Result<Vec<DisplayInfo>>>,
    },
}

#[cfg(windows)]
struct StaWorker {
    sender: std::sync::mpsc::Sender<StaRequest>,
}

#[cfg(windows)]
impl StaWorker {
    fn set_all(&self, path: String, placement: WallpaperPlacement) -> Result<()> {
        let (reply, receiver) = std::sync::mpsc::channel();
        self.sender
            .send(StaRequest::SetAll {
                path,
                placement,
                reply,
            })
            .context("Failed to queue wallpaper COM request")?;
        receiver
            .recv()
            .context("Wallpaper COM worker stopped before replying")?
    }

    fn set_monitor(&self, monitor_id: String, path: String) -> Result<()> {
        let (reply, receiver) = std::sync::mpsc::channel();
        self.sender
            .send(StaRequest::SetMonitor {
                monitor_id,
                path,
                reply,
            })
            .context("Failed to queue per-monitor wallpaper COM request")?;
        receiver
            .recv()
            .context("Wallpaper COM worker stopped before replying")?
    }

    fn get_displays(&self) -> Result<Vec<DisplayInfo>> {
        let (reply, receiver) = std::sync::mpsc::channel();
        self.sender
            .send(StaRequest::GetDisplays { reply })
            .context("Failed to queue display enumeration COM request")?;
        receiver
            .recv()
            .context("Wallpaper COM worker stopped before replying")?
    }
}

#[cfg(windows)]
fn sta_worker() -> &'static StaWorker {
    static WORKER: std::sync::OnceLock<StaWorker> = std::sync::OnceLock::new();
    WORKER.get_or_init(|| {
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            if let Err(error) = run_sta_worker(receiver) {
                eprintln!("PureWall wallpaper COM worker stopped: {error}");
            }
        });
        StaWorker { sender }
    })
}

#[cfg(windows)]
fn retry_transient_e_fail<T, F>(mut operation: F) -> windows::core::Result<T>
where
    F: FnMut() -> windows::core::Result<T>,
{
    let first = operation();
    if matches!(
        &first,
        Err(error) if error.code() == windows::Win32::Foundation::E_FAIL
    ) {
        std::thread::sleep(std::time::Duration::from_millis(80));
        operation()
    } else {
        first
    }
}

#[cfg(windows)]
fn run_sta_worker(receiver: std::sync::mpsc::Receiver<StaRequest>) -> Result<()> {
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

    struct ComGuard;
    impl Drop for ComGuard {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }

    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .context("Failed to initialize COM apartment")?;
    }
    let _guard = ComGuard;

    while let Ok(request) = receiver.recv() {
        match request {
            StaRequest::SetAll {
                path,
                placement,
                reply,
            } => {
                let _ = reply.send(set_wallpaper_all_com_in_sta(&path, placement));
            }
            StaRequest::SetMonitor {
                monitor_id,
                path,
                reply,
            } => {
                let _ = reply.send(set_wallpaper_on_monitor_in_sta(&monitor_id, &path));
            }
            StaRequest::GetDisplays { reply } => {
                let _ = reply.send(get_displays_in_sta());
            }
        }
    }

    Ok(())
}

#[cfg(windows)]
fn set_wallpaper_all_com_in_sta(path: &str, placement: WallpaperPlacement) -> Result<()> {
    let desktop = create_desktop_wallpaper()?;
    let path_wide = wide_null(path);
    unsafe {
        retry_transient_e_fail(|| {
            desktop.SetPosition(match placement {
                WallpaperPlacement::Fill => windows::Win32::UI::Shell::DWPOS_FILL,
                WallpaperPlacement::Span => windows::Win32::UI::Shell::DWPOS_SPAN,
            })
        })
        .context("IDesktopWallpaper::SetPosition failed")?;
        retry_transient_e_fail(|| {
            desktop.SetWallpaper(
                windows::core::PCWSTR::null(),
                windows::core::PCWSTR::from_raw(path_wide.as_ptr()),
            )
        })
        .with_context(|| format!("IDesktopWallpaper::SetWallpaper failed for file {path}"))?;
    }
    Ok(())
}

#[cfg(windows)]
fn set_wallpaper_on_monitor_in_sta(monitor_id: &str, path: &str) -> Result<()> {
    let desktop = create_desktop_wallpaper()?;
    let monitor_wide = wide_null(monitor_id);
    let path_wide = wide_null(path);
    unsafe {
        retry_transient_e_fail(|| {
            desktop.SetWallpaper(
                windows::core::PCWSTR::from_raw(monitor_wide.as_ptr()),
                windows::core::PCWSTR::from_raw(path_wide.as_ptr()),
            )
        })
        .with_context(|| {
            format!(
                "IDesktopWallpaper::SetWallpaper failed for monitor {monitor_id} and file {path}"
            )
        })?;
    }
    Ok(())
}

#[cfg(windows)]
fn get_displays_in_sta() -> Result<Vec<DisplayInfo>> {
    let desktop = create_desktop_wallpaper()?;
    let count = unsafe {
        retry_transient_e_fail(|| desktop.GetMonitorDevicePathCount())
            .context("IDesktopWallpaper::GetMonitorDevicePathCount failed")?
    };
    let mut displays = Vec::new();

    for index in 0..count {
        let monitor_id = unsafe {
            let raw_id = retry_transient_e_fail(|| desktop.GetMonitorDevicePathAt(index))
                .with_context(|| {
                    format!("IDesktopWallpaper::GetMonitorDevicePathAt failed for index {index}")
                })?;
            pwstr_to_string_and_free(raw_id)?
        };
        let monitor_wide = wide_null(&monitor_id);
        let rect = unsafe {
            retry_transient_e_fail(|| {
                desktop.GetMonitorRECT(windows::core::PCWSTR::from_raw(monitor_wide.as_ptr()))
            })
            .with_context(|| format!("IDesktopWallpaper::GetMonitorRECT failed for {monitor_id}"))?
        };
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width <= 0 || height <= 0 {
            continue;
        }

        let current_wallpaper = unsafe {
            match desktop.GetWallpaper(windows::core::PCWSTR::from_raw(monitor_wide.as_ptr())) {
                Ok(raw_path) => pwstr_to_string_and_free(raw_path).unwrap_or_default(),
                Err(_) => String::new(),
            }
        };

        displays.push(DisplayInfo {
            index,
            id: monitor_id,
            left: rect.left,
            top: rect.top,
            width,
            height,
            current_wallpaper,
        });
    }

    Ok(displays)
}

#[cfg(windows)]
fn set_wallpaper_system_parameters(path_str: &str) -> Result<()> {
    let wide = wide_null(path_str);

    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{
            SystemParametersInfoW, SPIF_SENDCHANGE, SPI_SETDESKWALLPAPER,
        };

        let result = SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            Some(wide.as_ptr() as *mut _),
            SPIF_SENDCHANGE,
        );

        if result.is_err() {
            anyhow::bail!("SystemParametersInfoW failed: {:?}", result);
        }
    }

    Ok(())
}

#[cfg(all(test, windows))]
mod tests {
    use super::retry_transient_e_fail;
    use std::cell::Cell;
    use windows::core::Error;
    use windows::Win32::Foundation::E_FAIL;

    #[test]
    fn transient_e_fail_is_retried_once() {
        let attempts = Cell::new(0);

        let result = retry_transient_e_fail(|| {
            attempts.set(attempts.get() + 1);
            if attempts.get() == 1 {
                Err(Error::from_hresult(E_FAIL))
            } else {
                Ok("recovered")
            }
        });

        assert_eq!(result.expect("second attempt should succeed"), "recovered");
        assert_eq!(attempts.get(), 2);
    }
}
