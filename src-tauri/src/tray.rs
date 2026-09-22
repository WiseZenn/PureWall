use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Manager,
};

use crate::{execute_playback_action, report_playback_failure, AppState};
pub fn create_tray(app: &mut tauri::App) -> anyhow::Result<()> {
    let next_item = MenuItem::with_id(app, "next", "下一张", true, None::<&str>)?;
    let like_item = MenuItem::with_id(app, "like", "喜欢当前壁纸", true, None::<&str>)?;
    let dislike_item = MenuItem::with_id(app, "dislike", "不喜欢当前壁纸", true, None::<&str>)?;
    let pause_item = MenuItem::with_id(app, "pause", "暂停 / 继续轮播", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let open_item = MenuItem::with_id(app, "open", "打开主界面", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &next_item,
            &like_item,
            &dislike_item,
            &pause_item,
            &separator,
            &open_item,
            &quit_item,
        ],
    )?;

    let _tray = TrayIconBuilder::new()
        .icon(Image::from_bytes(include_bytes!("../icons/icon.png"))?)
        .tooltip("PureWall")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "next" | "like" | "dislike" | "pause" => {
                let action = match event.id().as_ref() {
                    "next" => crate::playback_action::PlaybackAction::Next,
                    "like" => crate::playback_action::PlaybackAction::Like,
                    "dislike" => crate::playback_action::PlaybackAction::Dislike,
                    "pause" => crate::playback_action::PlaybackAction::TogglePause,
                    _ => unreachable!("matched playback tray action"),
                };
                let state = app.state::<AppState>();
                if let Err(message) = execute_playback_action(state.inner(), app, action) {
                    report_playback_failure(app, "Tray", action, message);
                }
            }
            "open" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => {
                crate::shutdown_background_threads(app);
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}
