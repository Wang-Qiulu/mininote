//! 让小窗能浮在 macOS 原生全屏 Space 之上。
//!
//! 原生全屏会把 App 挪进独立 Space，而窗口默认的 collectionBehavior 只属于
//! 创建它的那个 Space —— 此时窗口不是「被压在下面」，而是**根本不会被合成到
//! 全屏 Space 上**。要显示出来必须同时做两件事：
//!
//! 1. 提高 `windowLevel`（普通窗口是 0，压不住全屏窗口）
//! 2. `collectionBehavior` 带上 `CanJoinAllSpaces` + `FullScreenAuxiliary`
//!
//! 只调 Tauri 的 `set_always_on_top(true)` 只做了第 1 件，进不了全屏 Space。
//!
//! 按「写完就走」的调性，这两个能力**只在窗口被唤起时挂上、隐藏时摘掉**：
//! 常驻所有 Space 会让一块便签一直挂在每个桌面上，太吵。

use tauri::WebviewWindow;

#[cfg(target_os = "macos")]
mod imp {
    use super::WebviewWindow;
    use objc2_app_kit::{
        NSNormalWindowLevel, NSStatusWindowLevel, NSWindow, NSWindowCollectionBehavior,
        NSWindowLevel,
    };

    /// 默认用状态栏层级。全屏窗口本身在 `NSMainMenuWindowLevel` 附近，
    /// `NSFloatingWindowLevel`(3) 在部分全屏 App 上压不住。
    ///
    /// 实测若还压不住，可用环境变量 `MININOTE_FLOAT_LEVEL` 调高（101 / 1000）；
    /// 若发现它盖住了自己的托盘菜单或系统下拉菜单，可往低调（3）。
    /// 不必改代码重新编译。
    fn overlay_level() -> NSWindowLevel {
        std::env::var("MININOTE_FLOAT_LEVEL")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(NSStatusWindowLevel)
    }

    /// 取到底层的 `NSWindow*`。
    ///
    /// `macos-private-api` feature 已开，`ns_window()` 返回 `*mut c_void`，
    /// 实际就是 NSWindow*。窗口由 Tauri 全程持有，调用期间一定存活。
    /// 所有调用都发生在主线程（见 `window.rs` 的 `run_on_main_thread`）。
    fn ns_window(w: &WebviewWindow) -> Option<&NSWindow> {
        let ptr = w.ns_window().ok()?;
        if ptr.is_null() {
            return None;
        }
        // SAFETY: Tauri 保证该指针是当前窗口的 NSWindow，且窗口在 w 的生命周期内存活。
        Some(unsafe { &*ptr.cast::<NSWindow>() })
    }

    /// 唤起时提升：允许进入所有 Space（含原生全屏 Space），并压在其他窗口之上。
    pub fn elevate(w: &WebviewWindow) {
        let Some(win) = ns_window(w) else {
            eprintln!("[浮窗] 拿不到 NSWindow，跳过提升");
            return;
        };
        let behavior = NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle;
        let level = overlay_level();
        // 顺序：先改归属，再改层级，最后才由 set_focus 把窗口 order 到前面。
        win.setCollectionBehavior(behavior);
        win.setLevel(level);
        eprintln!("[浮窗] 已提升：level={level} behavior={behavior:?}");
    }

    /// 隐藏时还原：窗口回到普通层级、只属于当前 Space。
    pub fn restore(w: &WebviewWindow) {
        let Some(win) = ns_window(w) else { return };
        win.setCollectionBehavior(NSWindowCollectionBehavior::Default);
        win.setLevel(NSNormalWindowLevel);
        eprintln!("[浮窗] 已还原：level=0 behavior=Default");
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::WebviewWindow;
    pub fn elevate(_: &WebviewWindow) {}
    pub fn restore(_: &WebviewWindow) {}
}

pub use imp::{elevate, restore};
