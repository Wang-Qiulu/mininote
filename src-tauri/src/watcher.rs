//! 监听笔记目录的外部变更。
//!
//! 应用自己写入的文件会被抑制（见 `AppState::is_self_write`），
//! 避免「保存 → 触发变更 → 重新加载 → 再保存」的回环。

use std::path::PathBuf;
use std::sync::mpsc::channel;

use notify::{RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter, Manager};

pub fn start(app: &AppHandle, dir: PathBuf) {
    let (tx, rx) = channel();

    let mut watcher = match notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    }) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("创建文件监听失败: {e}");
            return;
        }
    };

    if let Err(e) = watcher.watch(&dir, RecursiveMode::NonRecursive) {
        eprintln!("监听目录失败: {e}");
        return;
    }

    let app2 = app.clone();
    std::thread::spawn(move || {
        while let Ok(res) = rx.recv() {
            let Ok(event) = res else { continue };
            let relevant = matches!(
                event.kind,
                notify::EventKind::Create(_) | notify::EventKind::Modify(_)
            );
            if !relevant {
                continue;
            }
            let Some(state) = app2.try_state::<crate::AppState>() else {
                continue;
            };
            for path in event.paths {
                let is_md = path.extension().map(|e| e == "md").unwrap_or(false);
                if !is_md {
                    continue;
                }
                if state.is_self_write(&path) {
                    continue;
                }
                let _ = app2.emit("notes-changed", path.to_string_lossy().to_string());
            }
        }
    });

    // 监听器必须被持有，否则会被回收
    if let Some(state) = app.try_state::<crate::AppState>() {
        if let Ok(mut slot) = state.watcher.lock() {
            *slot = Some(watcher);
        }
    }
}
