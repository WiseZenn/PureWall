use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Serialize)]
pub struct FocusModeStatus {
    pub enabled: bool,
    pub fullscreen_detected: bool,
    pub auto_paused: bool,
}

pub struct FocusMode {
    enabled: AtomicBool,
    fullscreen_detected: AtomicBool,
    auto_paused: AtomicBool,
    auto_pause_suppressed: AtomicBool,
}

impl FocusMode {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
            fullscreen_detected: AtomicBool::new(false),
            auto_paused: AtomicBool::new(false),
            auto_pause_suppressed: AtomicBool::new(false),
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_fullscreen_detected(&self, detected: bool) {
        self.fullscreen_detected.store(detected, Ordering::Relaxed);
    }

    pub fn fullscreen_detected(&self) -> bool {
        self.fullscreen_detected.load(Ordering::Relaxed)
    }

    pub fn set_auto_paused(&self, paused: bool) {
        self.auto_paused.store(paused, Ordering::Relaxed);
    }

    pub fn auto_paused(&self) -> bool {
        self.auto_paused.load(Ordering::Relaxed)
    }

    pub fn set_auto_pause_suppressed(&self, suppressed: bool) {
        self.auto_pause_suppressed
            .store(suppressed, Ordering::Relaxed);
    }

    pub fn auto_pause_suppressed(&self) -> bool {
        self.auto_pause_suppressed.load(Ordering::Relaxed)
    }

    pub fn snapshot(&self) -> FocusModeStatus {
        FocusModeStatus {
            enabled: self.enabled.load(Ordering::Relaxed),
            fullscreen_detected: self.fullscreen_detected.load(Ordering::Relaxed),
            auto_paused: self.auto_paused.load(Ordering::Relaxed),
        }
    }
}

#[cfg(windows)]
pub fn is_fullscreen_foreground() -> bool {
    use std::mem::size_of;
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowRect, IsWindowVisible,
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() || !IsWindowVisible(hwnd).as_bool() {
            return false;
        }

        let mut window_rect = RECT::default();
        if GetWindowRect(hwnd, &mut window_rect).is_err() {
            return false;
        }

        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if monitor.0.is_null() {
            return false;
        }

        let mut monitor_info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut monitor_info).as_bool() {
            return false;
        }

        covers_rect(window_rect, monitor_info.rcMonitor)
    }
}

#[cfg(not(windows))]
pub fn is_fullscreen_foreground() -> bool {
    false
}

#[cfg(windows)]
fn covers_rect(
    window: windows::Win32::Foundation::RECT,
    monitor: windows::Win32::Foundation::RECT,
) -> bool {
    const TOLERANCE: i32 = 2;

    window.left <= monitor.left + TOLERANCE
        && window.top <= monitor.top + TOLERANCE
        && window.right >= monitor.right - TOLERANCE
        && window.bottom >= monitor.bottom - TOLERANCE
}
