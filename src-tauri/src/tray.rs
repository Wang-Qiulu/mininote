//! 菜单栏托盘图标。
//!
//! 左键、右键都展开菜单。菜单里只有版本号和「退出」——
//! 小窗的显示／隐藏只由「双击 ⌘」负责，不在这里重复。

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter};

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
    // 版本号是纯展示项，不可点击；内容直接读 Cargo.toml，升版本时不用改这里。
    let version = MenuItem::with_id(
        app,
        "version",
        format!("mininote v{}", env!("CARGO_PKG_VERSION")),
        false,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "退出 mininote", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(app, &[&version, &sep, &quit])?;

    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;

    TrayIconBuilder::with_id("main")
        .icon(icon)
        // 模板图标：系统按菜单栏深浅色自动反相
        .icon_as_template(true)
        .tooltip("mininote")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "quit" => request_quit(app),
            _ => {}
        })
        .build(app)?;

    Ok(())
}
