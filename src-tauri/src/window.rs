//! 窗口的显示/隐藏与「小窗 ⇄ 大窗」形变。
//!
//! 形变动画放在 Rust 侧驱动（而不是前端逐帧调 IPC），
//! 避免每帧一次跨进程往返导致的卡顿；前端只负责内容层的交叉淡化。
//!
//! 三条硬约束：
//! 1. **只改宽度**，高度自始至终不变。
//! 2. **右边缘锚定**：宽度向左生长，笔记本身的位置纹丝不动。
//! 3. **收回时精确还原**到展开前的位置，不做任何居中或钳制。

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager};

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

pub fn show(app: &AppHandle) {
    let Some(w) = webview(app) else {
        eprintln!("[窗口] 找不到 main 窗口");
        return;
    };
    let _ = w.show();
    let _ = w.unminimize();

    // 关键：tao 的 set_focus 有前置守卫（窗口必须已 isVisible），而 show() 是
    // 异步投递到主线程的。若紧挨着调用 set_focus，此刻窗口还不可见，
    // set_focus 会被直接跳过，其中的 activateIgnoringOtherApps:YES 永不执行，
    // 结果就是 Accessory 应用的窗口永远压在别的应用下面。
    // 把聚焦排到主线程队列的 show 之后，FIFO 保证顺序。
    let w2 = w.clone();
    let _ = app.run_on_main_thread(move || {
        let _ = w2.set_focus();
        eprintln!(
            "[窗口] 已请求显示：visible={:?} focused={:?}",
            w2.is_visible(),
            w2.is_focused()
        );
    });
}

pub fn hide(app: &AppHandle) {
    if let Some(w) = webview(app) {
        let _ = w.hide();
    }
}

/// 显示/隐藏切换。返回 true 表示这次是「显示」。
pub fn toggle(app: &AppHandle) -> bool {
    let Some(w) = webview(app) else { return false };
    if w.is_visible().unwrap_or(false) {
        hide(app);
        false
    } else {
        show(app);
        true
    }
}

/// 大窗目标宽度：屏幕的 55%，并夹在合理区间内。高度始终不变。
fn expanded_width(app: &AppHandle, current: f64) -> f64 {
    let mut target = 900.0;
    if let Some(wv) = webview(app) {
        if let Ok(Some(monitor)) = wv.current_monitor() {
            let size = monitor.size();
            let sf = monitor.scale_factor();
            let sw = size.width as f64 / sf;
            target = (sw * 0.55).clamp(560.0, 1000.0);
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

