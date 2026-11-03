use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{Emitter, Manager, State};

mod notes;
mod tray;
mod watcher;
mod window;

use notes::{NoteMeta, NotePatch};

/// 全局共享状态。
pub struct AppState {
    /// 笔记存放目录
    pub notes_dir: Mutex<PathBuf>,
    /// 本应用最近写入过的文件与时间戳，用于抑制文件监听的自触发事件
    pub recent_writes: Mutex<HashMap<String, u64>>,
    /// 文件监听器，必须持有引用否则会被回收
    pub watcher: Mutex<Option<notify::RecommendedWatcher>>,
}

impl AppState {
    fn dir(&self) -> PathBuf {
        self.notes_dir.lock().map(|d| d.clone()).unwrap_or_default()
    }

    /// 记录一次由本应用发起的写入
    pub fn mark_written(&self, path: &std::path::Path) {
        if let Ok(mut map) = self.recent_writes.lock() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let key = path.to_string_lossy().to_string();
            map.insert(key, now);
            // 只保留最近 64 条，避免无限增长
            if map.len() > 64 {
                let oldest = map.iter().min_by_key(|(_, v)| **v).map(|(k, _)| k.clone());
                if let Some(k) = oldest {
                    map.remove(&k);
                }
            }
        }
    }

    /// 判断某次文件变更是否是本应用自己写入造成的
    pub fn is_self_write(&self, path: &std::path::Path) -> bool {
        let key = path.to_string_lossy().to_string();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.recent_writes
            .lock()
            .map(|map| map.get(&key).map(|t| now.saturating_sub(*t) < 2).unwrap_or(false))
            .unwrap_or(false)
    }
}

/// 默认笔记目录：`~/Documents/备忘录`
fn default_notes_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("MININOTE_DIR") {
        return PathBuf::from(custom);
    }
    let home = std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."));
    home.join("Documents").join("备忘录")
}

fn ensure_dir(dir: &std::path::Path) {
    let _ = std::fs::create_dir_all(dir);
}

// ---------- 命令 ----------

#[tauri::command]
fn list_notes(state: State<AppState>) -> Vec<NoteMeta> {
    notes::list_notes(&state.dir())
}

#[tauri::command]
fn read_note(id: String, state: State<AppState>) -> Option<NoteMeta> {
    notes::read_note_file(&state.dir().join(format!("{id}.md")))
}

#[tauri::command]
fn create_note(title: String, body: String, state: State<AppState>) -> Result<NoteMeta, String> {
    let dir = state.dir();
    ensure_dir(&dir);
    let meta = notes::create_note(&dir, &title, &body)?;
    state.mark_written(&dir.join(format!("{}.md", meta.id)));
    Ok(meta)
}

#[tauri::command]
fn save_note(
    id: String,
    title: String,
    body: String,
    expected: Option<String>,
    state: State<AppState>,
) -> Result<NoteMeta, String> {
    let dir = state.dir();
    let meta = notes::save_note(&dir, &id, &title, &body, expected.as_deref())?;
    state.mark_written(&dir.join(format!("{}.md", meta.id)));
    Ok(meta)
}

#[tauri::command]
fn patch_note(patch: NotePatch, state: State<AppState>) -> Result<NoteMeta, String> {
    let dir = state.dir();
    let meta = notes::patch_note(&dir, &patch)?;
    state.mark_written(&dir.join(format!("{}.md", meta.id)));
    Ok(meta)
}

#[tauri::command]
fn delete_note(id: String, state: State<AppState>) -> Result<(), String> {
    notes::delete_note(&state.dir(), &id)
}

#[tauri::command]
fn notes_dir(state: State<AppState>) -> String {
    state.dir().to_string_lossy().to_string()
}

/// 在 Finder 中定位某个笔记文件
#[tauri::command]
fn reveal_note(id: String, state: State<AppState>) -> Result<(), String> {
    let path = state.dir().join(format!("{id}.md"));
    if !path.exists() {
        return Err("笔记不存在".into());
    }
    std::process::Command::new("open")
        .arg("-R")
        .arg(&path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("无法打开访达: {e}"))
}

/// 在访达中打开笔记目录
#[tauri::command]
fn open_notes_dir(state: State<AppState>) -> Result<(), String> {
    let dir = state.dir();
    ensure_dir(&dir);
    std::process::Command::new("open")
        .arg(&dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("无法打开访达: {e}"))
}

#[tauri::command]
fn show_window(app: tauri::AppHandle) {
    window::show(&app);
}

/// 前端保存完成后调用，随后进程退出
#[tauri::command]
fn quit_ready() {
    tray::quit_ready();
}

#[tauri::command]
fn hide_window(app: tauri::AppHandle) {
    window::hide(&app);
}

#[tauri::command]
fn toggle_window(app: tauri::AppHandle) {
    window::toggle(&app);
}

/// 切换小窗 / 大窗。`mode` 取 "mini" 或 "expanded"。
#[tauri::command]
fn set_window_mode(mode: String, app: tauri::AppHandle) {
    window::set_mode(&app, &mode);
}

// ---------- 入口 ----------

pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, _shortcut, event| {
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        window::show(app);
                        let _ = app.emit("focus-editor", ());
                    }
                })
                .build(),
        )
        .setup(|app| {
            // 菜单栏常驻：没有 Dock 图标，也没有应用菜单栏
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let dir = default_notes_dir();
            // 首次运行：目录还不存在，视为新装，弹一次窗口做引导
            let first_run = !dir.exists();
            ensure_dir(&dir);
            app.manage(AppState {
                notes_dir: Mutex::new(dir.clone()),
                recent_writes: Mutex::new(HashMap::new()),
                watcher: Mutex::new(None),
            });

            tray::setup(app.handle())?;
            watcher::start(app.handle(), dir);

            // 注册全局快捷键：Cmd + Shift + Space
            use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
            let shortcut = Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::Space);
            if let Err(e) = app.handle().global_shortcut().register(shortcut) {
                eprintln!("注册全局快捷键失败: {e}");
            }

            if first_run || std::env::var("MININOTE_SHOW").is_ok() {
                window::show(app.handle());
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_notes,
            read_note,
            create_note,
            save_note,
            patch_note,
            delete_note,
            notes_dir,
            reveal_note,
            open_notes_dir,
            show_window,
            hide_window,
            toggle_window,
            set_window_mode,
            quit_ready,
        ])
        .build(tauri::generate_context!())
        .expect("构建小记事本失败")
        .run(|_app, event| {
            // 菜单栏应用：关掉窗口不等于退出，托盘图标与全局快捷键必须留着。
            // 只有用户从托盘菜单明确选了「退出」才真正结束进程。
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if !tray::is_quit_requested() {
                    api.prevent_exit();
                }
            }
        });
}
