//! 菜单栏托盘图标。
//!
//! 左侧点击显示/隐藏小窗（菜单栏项目的常见行为），右侧点击弹出菜单。

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

/// 前端确认「已保存完，可以退了」的信号
static QUIT_READY: AtomicBool = AtomicBool::new(false);

/// 用户主动点了「退出」。用于区分「关闭最后一个窗口」与「真正退出」——
/// 前者必须阻止退出（菜单栏应用的托盘和快捷键得留着），后者不能拦。
static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);

pub fn quit_ready() {
    QUIT_READY.store(true, Ordering::SeqCst);
}

/// 是否是用户主动要求退出
pub fn is_quit_requested() -> bool {
    QUIT_REQUESTED.load(Ordering::SeqCst)
}

/// 通知前端保存并退出；等待前端确认（最多 1.2 秒）后退出
fn request_quit(app: &AppHandle) {
    QUIT_REQUESTED.store(true, Ordering::SeqCst);
    QUIT_READY.store(false, Ordering::SeqCst);
    let _ = app.emit("app-quitting", ());

    let handle = app.clone();
    std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_millis(1200);
        while std::time::Instant::now() < deadline {
            if QUIT_READY.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        handle.exit(0);
    });
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "显示 / 隐藏小窗", true, None::<&str>)?;
    let open_dir = MenuItem::with_id(app, "open_dir", "打开笔记目录", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(app, &[&toggle, &sep1, &open_dir, &sep1, &quit])?;

    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;

    TrayIconBuilder::with_id("main")
        .icon(icon)
        // 模板图标：系统按菜单栏深浅色自动反相
        .icon_as_template(true)
        .tooltip("小记事本")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "toggle" => {
                crate::window::toggle(app);
            }
            "open_dir" => {
                let dir = app
                    .state::<crate::AppState>()
                    .notes_dir
                    .lock()
                    .ok()
                    .map(|d| d.clone());
                if let Some(dir) = dir {
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = std::process::Command::new("open").arg(&dir).spawn();
                }
            }
            "quit" => request_quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                crate::window::show(tray.app_handle(), true);
            }
        })
        .build(app)?;

    Ok(())
}
