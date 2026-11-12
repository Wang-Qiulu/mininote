//! 「双击 ⌘」唤起 / 收起小窗。
//!
//! 修饰键的双击没法表达成「修饰键 + 主键」的组合，所以用 AppKit 的事件监视器
//! 自己判定。这是项目唯一的快捷键。
//!
//! ⌘ 是日常使用最频繁的修饰键（⌘C / ⌘V / ⌘Tab…），误触风险最高，判定因此很严：
//!
//! 1. 必须是两次完整的「按下 → 抬起 → 按下」，不是一直按住；
//! 2. 两次按下的间隔在 300ms 内；
//! 3. 中间不能夹任何普通按键，也不能同时按 Shift / Option / Control ——
//!    这样 `⌘C` 接 `⌘V` 这类连续组合都不会误触发。
//!
//! global monitor 收其他 App 的事件，local monitor 收本应用自己的事件：小窗唤起后
//! 本应用会成为前台，没有 local monitor 就再也收不回来了。两者都要求辅助功能权限
//! （与项目已有的权限检查是同一套机制）。

#[cfg(target_os = "macos")]
mod imp {
    use std::ptr::NonNull;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use block2::RcBlock;
    use objc2_app_kit::{NSEvent, NSEventMask, NSEventModifierFlags, NSEventType};
    use tauri::AppHandle;

    /// 两次按下之间的最大间隔。太短难按，太长容易和连续的 ⌘ 组合键撞上。
    const WINDOW: Duration = Duration::from_millis(300);

    struct State {
        /// 当前 Command 是否按下
        cmd_down: bool,
        /// 已完成第一次「干净」的 Command 按下，正在等第二次
        armed: bool,
        last_press: Option<Instant>,
    }

    static STATE: Mutex<State> = Mutex::new(State {
        cmd_down: false,
        armed: false,
        last_press: None,
    });

    fn handle(ev: NonNull<NSEvent>, app: &AppHandle) {
        // SAFETY: AppKit 保证回调期间 event 有效，这里只读类型与修饰键。
        let event = unsafe { ev.as_ref() };
        let ty = event.r#type();

        if ty.0 == NSEventType::FlagsChanged.0 {
            let flags = event.modifierFlags();
            let cmd = flags.contains(NSEventModifierFlags::Command);
            let other_mods = flags.intersects(
                NSEventModifierFlags::Shift
                    | NSEventModifierFlags::Control
                    | NSEventModifierFlags::Option,
            );

            let hit = {
                let mut st = STATE.lock().unwrap();
                let now = Instant::now();
                let mut hit = false;

                if cmd && !st.cmd_down {
                    // Command 刚被按下
                    if other_mods {
                        // 一上来就是组合键（⌘⇧…）
                        st.armed = false;
                    } else if st.armed
                        && st.last_press.is_some_and(|t| now.duration_since(t) <= WINDOW)
                    {
                        // 第二次干净按下且在窗口内 —— 双击成立
                        st.armed = false;
                        st.last_press = None;
                        hit = true;
                    } else {
                        st.last_press = Some(now);
                        st.armed = true;
                    }
                } else if other_mods {
                    // 按住 ⌘ 时又压了别的修饰键，视为组合键
                    st.armed = false;
                }

                // 无论走哪个分支都要更新：global 与 local 监视器可能收到同一个
                // 事件，漏了这行就会把一次按下算成两次。
                st.cmd_down = cmd;
                hit
            };

            if hit {
                eprintln!("[双击⌘] 命中，切换窗口");
                crate::window::toggle(app);
            }
        } else if ty.0 == NSEventType::KeyDown.0 {
            // 两次 ⌘ 之间夹了任何普通键（含 ⌘C / ⌘V）就不算双击
            STATE.lock().unwrap().armed = false;
        }
    }

    pub fn setup(app: &AppHandle) {
        let mask = NSEventMask::FlagsChanged | NSEventMask::KeyDown;

        let app_global = app.clone();
        let global = RcBlock::new(move |ev: NonNull<NSEvent>| handle(ev, &app_global));
        let token_global = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &global);

        let app_local = app.clone();
        let local = RcBlock::new(move |ev: NonNull<NSEvent>| -> *mut NSEvent {
            handle(ev, &app_local);
            ev.as_ptr()
        });
        // SAFETY: 原样返回事件，不吞、不改写。
        let token_local =
            unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &local) };

        if token_global.is_some() && token_local.is_some() {
            eprintln!("[双击⌘] 监视器已注册（判定窗口 300ms；需要辅助功能权限）");
        } else {
            eprintln!("[双击⌘] 监视器注册失败");
        }

        // 监视器必须活到进程结束：把 token 和 block 都交给进程持有，
        // 别让 Rust 侧作用域结束就把它们 drop 掉。
        std::mem::forget((token_global, token_local, global, local));
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn setup(_: &tauri::AppHandle) {}
}

pub use imp::setup;
