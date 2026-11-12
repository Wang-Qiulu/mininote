//! 窗口的显示/隐藏与「小窗 ⇄ 大窗」形变。
//!
//! 形变动画放在 Rust 侧驱动（而不是前端逐帧调 IPC），
//! 避免每帧一次跨进程往返导致的卡顿；前端只负责内容层的交叉淡化。
//!
//! 三条硬约束：
//! 1. **只改宽度**，高度自始至终不变。
//! 2. **右边缘锚定**：宽度向左生长，笔记本身的位置纹丝不动。
//! 3. **收回时精确还原**到展开前的位置，不做任何居中或钳制。

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager};

/// 是否处于「主动唤起」状态（层级已提升、可跨 Space）。
///
/// 它**不等于**「窗口可见」：启动时窗口就是可见的，但那是被动摆出来，
/// 没有提升层级，也不该算作一次唤起。若拿「是否可见」来切换显隐，
/// 用户切进全屏 Space 后第一次按快捷键会走 hide 分支 —— 窗口明明没显示
/// 在全屏 Space 上，按了却像没反应，得按第二次才浮出来。
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// 本次主动唤起是否要把光标落到编辑器。
/// 启动时的被动显示不该抢焦点，只有主动唤起才要。
static WANT_FOCUS: AtomicBool = AtomicBool::new(false);

pub const MINI_W: f64 = 360.0;
pub const MINI_H: f64 = 300.0;

/// 动画参数：接近临界阻尼的弹簧，末端只有极轻微的回弹。
/// 取值参考了常见的桌面端侧边栏动画参数（stiffness 300~350 / damping 30~35 附近），
/// 按「窗口尺寸变化比 DOM 宽度变化更大」略调硬一点。
const SPRING_STIFFNESS: f64 = 700.0;
const SPRING_DAMPING: f64 = 45.0;
const SPRING_MASS: f64 = 1.0;
/// 动画时长固定。用「误差小于某阈值就提前结束」是不可靠的——
/// 那个窗口只有十几毫秒宽，60fps 很容易整个错过，导致时长在 336ms 和 634ms
/// 之间随机跳变。
const ANIM_DURATION: Duration = Duration::from_millis(320);
/// 目标帧率
const FPS: u64 = 60;

/// 动画代次。每次开始新动画就自增，正在跑的旧动画发现代次变了就自行退出，
/// 避免连续点击时两个动画线程互相拉扯窗口尺寸。
static ANIM_GENERATION: AtomicU32 = AtomicU32::new(0);

/// 展开前的窗口几何。收回时按它精确还原。
#[derive(Clone, Copy, Debug)]
struct Anchor {
    /// 窗口左上角（逻辑像素）
    x: f64,
    y: f64,
    /// 内容区宽度（逻辑像素）。高度自始至终不变，不需要记录。
    inner_w: f64,
}

static SAVED_ANCHOR: Mutex<Option<Anchor>> = Mutex::new(None);

fn webview(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    app.get_webview_window("main")
}

/// 取走一次「落光标」请求。全局只会有一个消费者拿到 true。
///
/// 主线程闭包和 `WindowEvent::Focused` 都会尝试取：谁先到谁发事件，
/// 既保证窗口已经拿到 key，也不会因为「窗口本来就是 key、不触发 Focused」
/// 而漏掉聚焦。
pub fn consume_focus_request() -> bool {
    WANT_FOCUS.swap(false, Ordering::SeqCst)
}

/// 显示窗口。`active` 表示这是一次**主动唤起**（快捷键 / 托盘 / Reopen）：
/// 只有主动唤起才提升层级、跨 Space、并把光标落到编辑器。
/// 启动时传 false，只是把窗口摆出来，保持普通窗口层级。
pub fn show(app: &AppHandle, active: bool) {
    let Some(w) = webview(app) else {
        eprintln!("[窗口] 找不到 main 窗口");
        return;
    };
    // 必须在 set_focus() 抢焦点之前记下前台应用，否则记到的会是 mininote 自己。
    // 启动时的被动显示（active=false）不抢焦点，也不需要记录。
    if active {
        crate::focus::remember();
    }
    ACTIVE.store(active, Ordering::SeqCst);
    WANT_FOCUS.store(active, Ordering::SeqCst);

    let _ = w.show();
    let _ = w.unminimize();

    // 关键：tao 的 set_focus 有前置守卫（窗口必须已 isVisible），而 show() 是
    // 异步投递到主线程的。若紧挨着调用 set_focus，此刻窗口还不可见，
    // set_focus 会被直接跳过，其中的 activateIgnoringOtherApps:YES 永不执行，
    // 结果就是 Accessory 应用的窗口永远压在别的应用下面。
    // 把聚焦排到主线程队列的 show 之后，FIFO 保证顺序。
    //
    // 主动唤起时顺序更关键：先挂上跨 Space / 高层级能力，再 makeKeyAndOrderFront。
    // 反过来的话，窗口会在「还没被允许进入全屏 Space」时被 order 到前面，白做一次。
    let w2 = w.clone();
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        if active {
            crate::overlay::elevate(&w2);
        }
        let _ = w2.set_focus();
        // set_focus 内部是同步的 makeKeyAndOrderFront，此刻窗口已经是 key，
        // 再发聚焦请求就不再是「抢跑」了。
        if consume_focus_request() {
            let _ = app2.emit("focus-editor", ());
        }
        eprintln!(
            "[窗口] 已显示：active={active} visible={:?} focused={:?}",
            w2.is_visible(),
            w2.is_focused()
        );
    });
}

pub fn hide(app: &AppHandle) {
    // 此刻前台还是不是自己：用户写笔记中途切到别的应用时，收起不该把焦点
    // 硬拽回旧应用。必须在窗口隐藏之前判断，隐藏后系统可能已经改了前台。
    let should_restore_focus = crate::focus::is_self_frontmost();
    ACTIVE.store(false, Ordering::SeqCst);
    WANT_FOCUS.store(false, Ordering::SeqCst);
    if let Some(w) = webview(app) {
        let _ = w.hide();
        // 摘掉跨 Space 能力，别让便签常驻在每个桌面上。
        // AppKit 的窗口属性必须在主线程上改。
        let w2 = w.clone();
        let _ = app.run_on_main_thread(move || {
            crate::overlay::restore(&w2);
            // 排在窗口隐藏与层级还原之后：先摘掉跨 Space 能力，再把前台
            // 交还给对方，否则自己的浮窗可能残留在对方的全屏页面上。
            if should_restore_focus {
                crate::focus::restore();
            } else {
                eprintln!("[焦点] 前台已不是 mininote，跳过归还");
            }
        });
    }
}

/// 显示/隐藏切换：没唤起就主动唤起，已唤起就收起还原。
pub fn toggle(app: &AppHandle) {
    if ACTIVE.load(Ordering::SeqCst) {
        hide(app);
    } else {
        show(app, true);
    }
}

/// 大窗目标宽度：侧栏 360 + 笔记 360 = 720。
///
/// 侧栏用 `flex: 1 1 auto` 吸收多出来的宽度，笔记区锁死 360，
/// 所以窗口多宽侧栏就是多宽——定成 720 侧栏正好也是 360。
fn expanded_width(app: &AppHandle, current: f64) -> f64 {
    let mut target = MINI_W * 2.0;
    if let Some(wv) = webview(app) {
        if let Ok(Some(monitor)) = wv.current_monitor() {
            let size = monitor.size();
            let sf = monitor.scale_factor();
            let sw = size.width as f64 / sf;
            // 留一点余量，别让窗口顶到屏幕边缘
            target = target.min((sw - 80.0).max(MINI_W + 160.0));
        }
    }
    // 比现在更宽才有意义
    target.max(current + 160.0)
}

/// 单位阶跃的弹簧响应。欠阻尼时末端有一点点回弹，像被弹簧拉住。
fn spring(t: f64) -> f64 {
    let wn = (SPRING_STIFFNESS / SPRING_MASS).sqrt();
    let zeta = SPRING_DAMPING / (2.0 * (SPRING_STIFFNESS * SPRING_MASS).sqrt());
    if zeta < 1.0 {
        let wd = wn * (1.0 - zeta * zeta).sqrt();
        1.0 - (-zeta * wn * t).exp() * ((wd * t).cos() + (zeta * wn / wd) * (wd * t).sin())
    } else {
        1.0 - (-wn * t).exp() * (1.0 + wn * t)
    }
}

/// 把窗口放到屏幕可用区域的右上角。
///
/// 用的是 monitor 的 work_area —— 它在 macOS 上是菜单栏以下、程序坞以外的
/// 区域，正好是「贴着菜单栏右下角」该待的地方。
pub fn move_to_top_right(app: &AppHandle) {
    let Some(w) = webview(app) else { return };
    let Ok(Some(monitor)) = w.current_monitor() else { return };

    let sf = monitor.scale_factor();
    let area = monitor.work_area();
    let (ax, ay) = (area.position.x as f64 / sf, area.position.y as f64 / sf);
    let aw = area.size.width as f64 / sf;

    let Ok(size) = w.outer_size() else { return };
    let ww = size.width as f64 / sf;

    /// 离屏幕右边缘和菜单栏下沿的留白
    const MARGIN: f64 = 10.0;

    let x = ax + aw - ww - MARGIN;
    let y = ay + MARGIN;
    let _ = w.set_position(LogicalPosition::new(x, y));
    eprintln!("[窗口] 定位到右上角 {x:.0},{y:.0}（工作区宽 {aw:.0}，窗口宽 {ww:.0}，留白 {MARGIN:.0}）");
}

/// 切换小窗 / 大窗。
///
/// - 只改宽度，高度锁死
/// - 展开时右边缘锚定：宽度向左生长，笔记原地不动
/// - 收回时按展开前记录的位置精确还原
pub fn set_mode(app: &AppHandle, mode: &str) {
    let Some(w) = webview(app) else { return };

    // 新动画开始，老动画会让位
    let generation = ANIM_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;

    let sf = w.scale_factor().unwrap_or(1.0);
    let (x0, y0) = w
        .outer_position()
        .map(|p| (p.x as f64 / sf, p.y as f64 / sf))
        .unwrap_or((0.0, 0.0));
    let (iw0, ih0) = w
        .inner_size()
        .map(|s| (s.width as f64 / sf, s.height as f64 / sf))
        .unwrap_or((MINI_W, MINI_H));

    let expanding = mode == "expanded";

    let (end_w, end_x, end_y) = if expanding {
        // 记下展开前的几何，收回时原样还原
        {
            let mut saved = SAVED_ANCHOR.lock().unwrap();
            if saved.is_none() {
                *saved = Some(Anchor {
                    x: x0,
                    y: y0,
                    inner_w: iw0,
                });
            }
        }
        let tw = expanded_width(app, iw0);
        // 右边缘锚定：宽度往左长，左上角相应左移
        let mut tx = x0 + iw0 - tw;
        // 长到屏幕外就改为左边缘锚定，往右长
        if let Some(left) = screen_left(app) {
            if tx < left {
                tx = x0;
            }
        }
        (tw, tx, y0)
    } else {
        // 收回：回到展开前记录的位置与宽度
        let saved = *SAVED_ANCHOR.lock().unwrap();
        match saved {
            Some(a) => (a.inner_w, a.x, a.y),
            None => (MINI_W, x0, y0),
        }
    };

    let _ = app.emit("window-mode-start", mode.to_string());

    // 固定 60fps 的步进，用真实时间算弹簧值，掉帧时自动追赶
    let frame = Duration::from_nanos(1_000_000_000 / FPS);
    let start = Instant::now();
    let mut next = start;
    let mut last_w = iw0;
    let mut last_x = x0;
    let mut ticks: u32 = 0;
    let mut applied: u32 = 0;
    let mut max_gap = Duration::ZERO;

    loop {
        // 被新的动画取代了：直接退出，最终状态交给新动画
        if ANIM_GENERATION.load(Ordering::SeqCst) != generation {
            return;
        }
        let elapsed = start.elapsed();
        if elapsed >= ANIM_DURATION {
            break;
        }
        let p = spring(elapsed.as_secs_f64()).clamp(0.0, 1.05);

        let nw = iw0 + (end_w - iw0) * p;
        let nx = x0 + (end_x - x0) * p;
        let ny = y0 + (end_y - y0) * p;

        // 变化不足半像素就不下发，省掉无谓的窗口服务器调用
        if (nw - last_w).abs() >= 0.5 || (nx - last_x).abs() >= 0.5 {
            // 高度自始至终不变
            let _ = w.set_size(LogicalSize::new(nw, ih0));
            // 同时下发位置，右边缘才会严格锚住
            let _ = w.set_position(LogicalPosition::new(nx, ny));
            last_w = nw;
            last_x = nx;
            applied += 1;
        }
        ticks += 1;

        next += frame;
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else {
            // 落后了就跳过，不堆积
            let gap = now.duration_since(next);
            if gap > max_gap {
                max_gap = gap;
            }
            next = now;
        }
    }

    // 收尾：精确落到目标值
    let _ = w.set_size(LogicalSize::new(end_w, ih0));
    let _ = w.set_position(LogicalPosition::new(end_x, end_y));
    if !expanding {
        let _ = SAVED_ANCHOR.lock().unwrap().take();
    }

    let total = start.elapsed();
    eprintln!(
        "[形变] {mode}: 宽 {iw0:.0}→{end_w:.0}（高不变 {ih0:.0}），位置 {x0:.0},{y0:.0}→{end_x:.0},{end_y:.0}，\
         {ticks} 帧 / {}ms（{:.0}fps，实际下发 {applied} 次，最大落后 {:?}）",
        total.as_millis(),
        ticks as f64 / total.as_secs_f64(),
        max_gap
    );

    let _ = app.emit("window-mode-end", mode.to_string());
}

/// 屏幕左边界（逻辑像素），用于判断向左扩展会不会跑出屏幕
fn screen_left(app: &AppHandle) -> Option<f64> {
    let wv = webview(app)?;
    let monitor = wv.current_monitor().ok()??;
    let sf = monitor.scale_factor();
    Some(monitor.position().x as f64 / sf)
}

