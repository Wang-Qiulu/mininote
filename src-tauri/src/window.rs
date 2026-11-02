//! 窗口的显示/隐藏与「小窗 ⇄ 大窗」形变。
//!
//! 形变动画放在 Rust 侧驱动（而不是前端逐帧调 IPC），
//! 避免每帧一次跨进程往返导致的卡顿；前端只负责内容层的交叉淡化。

use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager};

pub const MINI_W: f64 = 360.0;
pub const MINI_H: f64 = 300.0;

const ANIM_STEPS: u32 = 24;
const ANIM_STEP_MS: u64 = 12;

fn webview(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    app.get_webview_window("main")
}

pub fn show(app: &AppHandle) {
    if let Some(w) = webview(app) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

pub fn hide(app: &AppHandle) {
    if let Some(w) = webview(app) {
        let _ = w.hide();
    }
}

pub fn toggle(app: &AppHandle) {
    if let Some(w) = webview(app) {
        if w.is_visible().unwrap_or(false) {
            hide(app);
        } else {
            show(app);
        }
    }
}

/// 大窗目标尺寸：屏幕的 55% 宽、70% 高，并受窗口上限约束
fn expanded_size(app: &AppHandle) -> (f64, f64) {
    let mut w = 900.0;
    let mut h = 640.0;
    if let Some(wv) = webview(app) {
        if let Ok(Some(monitor)) = wv.current_monitor() {
            let size = monitor.size();
            let sf = monitor.scale_factor();
            let (sw, sh) = (size.width as f64 / sf, size.height as f64 / sf);
            w = (sw * 0.55).clamp(640.0, 1200.0);
            h = (sh * 0.70).clamp(480.0, 900.0);
        }
    }
    (w, h)
}

/// 切换小窗 / 大窗，过程中发出事件让前端同步做内容层淡化
pub fn set_mode(app: &AppHandle, mode: &str) {
    let Some(w) = webview(app) else { return };

    let (tw, th) = if mode == "expanded" {
        expanded_size(app)
    } else {
        (MINI_W, MINI_H)
    };

    let _ = app.emit("window-mode-start", mode.to_string());

    let sf = w.scale_factor().unwrap_or(1.0);
    let (cw, ch) = w
        .inner_size()
        .map(|s| (s.width as f64 / sf, s.height as f64 / sf))
        .unwrap_or((MINI_W, MINI_H));

    for i in 1..=ANIM_STEPS {
        let t = i as f64 / ANIM_STEPS as f64;
        // ease-out cubic，收尾有轻微减速，像被弹簧拉住
        let e = 1.0 - (1.0 - t).powi(3);
        let x = cw + (tw - cw) * e;
        let y = ch + (th - ch) * e;
        let _ = w.set_size(LogicalSize::new(x, y));
        std::thread::sleep(std::time::Duration::from_millis(ANIM_STEP_MS));
    }

    // 保证放大后的窗口不会超出屏幕
    if let Ok(Some(monitor)) = w.current_monitor() {
        let size = monitor.size();
        let msf = monitor.scale_factor();
        let (sw, sh) = (size.width as f64 / msf, size.height as f64 / msf);
        if let Ok(pos) = w.outer_position() {
            let (px, py) = (pos.x as f64 / msf, pos.y as f64 / msf);
            let nx = px.min((sw - tw).max(0.0));
            let ny = py.min((sh - th).max(0.0));
            if (nx - px).abs() > 0.5 || (ny - py).abs() > 0.5 {
                let _ = w.set_position(LogicalPosition::new(nx, ny));
            }
        }
    }

    let _ = app.emit("window-mode-end", mode.to_string());
}
