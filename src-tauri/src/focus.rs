//! 唤起前记住前台应用，收起时把焦点还回去。
//!
//! 双击 ⌘ 唤起时 `set_focus()` 会把焦点抢到小窗（必须，否则打不了字），
//! 但只把窗口隐藏起来，macOS 不会自动把前台交还给原来的应用 —— 对没有
//! Dock 图标的 Accessory 应用尤其明显，收起后整个屏幕处于失焦状态。
//!
//! 这里用 NSWorkspace / NSRunningApplication 记录并激活，都是普通 AppKit
//! 接口，不需要辅助功能权限（双击 ⌘ 的事件监视器才需要）。

#[cfg(target_os = "macos")]
mod imp {
    use std::sync::Mutex;

    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSWorkspace};

    /// 唤起前的前台应用。只存 pid：跨线程持有 AppKit 对象既没必要也不安全。
    static PREVIOUS_PID: Mutex<Option<i32>> = Mutex::new(None);

    fn my_pid() -> i32 {
        std::process::id() as i32
    }

    fn frontmost_pid() -> Option<i32> {
        NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .map(|app| app.processIdentifier())
    }

    /// 在 `set_focus()` 抢焦点**之前**调用。
    ///
    /// 前台若已是 mininote 自己（窗口本来就可见、或重复唤起），保留上一次
    /// 的记录，别把「要还给谁」清掉。
    pub fn remember() {
        match frontmost_pid() {
            Some(pid) if pid != my_pid() => {
                *PREVIOUS_PID.lock().unwrap() = Some(pid);
                eprintln!("[焦点] 记住前台应用 pid={pid}");
            }
            Some(_) => eprintln!("[焦点] 前台已是 mininote，保留上次记录"),
            None => eprintln!("[焦点] 读不到前台应用，保留上次记录"),
        }
    }

    /// 此刻前台是否仍是 mininote。
    /// 用户写笔记中途切到别的应用时，收起不该把焦点硬拽回旧应用。
    pub fn is_self_frontmost() -> bool {
        frontmost_pid() == Some(my_pid())
    }

    /// 隐藏窗口、还原浮窗层级之后调用：把焦点还给记下的应用。
    pub fn restore() {
        let Some(pid) = PREVIOUS_PID.lock().unwrap().take() else {
            eprintln!("[焦点] 没有记录，跳过归还");
            return;
        };
        let Some(prev) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) else {
            eprintln!("[焦点] pid={pid} 已不存在，跳过归还");
            return;
        };
        if prev.isTerminated() {
            eprintln!("[焦点] pid={pid} 已退出，跳过归还");
            return;
        }
        // macOS 14 起 ActivateIgnoringOtherApps 已废弃且无效，用空选项即可。
        let ok = prev.activateWithOptions(NSApplicationActivationOptions::empty());
        eprintln!("[焦点] 归还 pid={pid} 结果={ok}");
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn remember() {}
    pub fn is_self_frontmost() -> bool {
        false
    }
    pub fn restore() {}
}

pub use imp::{is_self_frontmost, remember, restore};
