//! 笔记存储层：纯 Markdown 文件 + 极简 frontmatter。
//!
//! 设计要点：
//! - 文件即唯一真相源，任何时候都能用别的编辑器直接改。
//! - frontmatter 只用 `key: value`，手写解析以完全控制写回格式。
//! - 写入走「临时文件 + rename」，保证原子性，崩溃或并发写不会留下半截文件。

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const FM_DELIM: &str = "---";

/// 一条笔记的完整信息。列表接口会省略 `body`，正文单独用 `read_note` 取。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteMeta {
    /// 文件名主干（不含扩展名），全局唯一
    pub id: String,
    pub title: String,
    pub created: String,
    pub updated: String,
    pub pinned: bool,
    /// YYYY-MM-DD
    pub due: Option<String>,
    pub done: bool,
    /// Markdown 正文（不含 frontmatter）
    pub body: String,
    /// 纯文本摘要，用于列表卡片
    pub preview: String,
    pub todo_total: u32,
    pub todo_done: u32,
}

/// 元数据局部更新。`due` 为 `Some("")` 表示清空，`None` 表示不改动。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotePatch {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub pinned: Option<bool>,
    #[serde(default)]
    pub due: Option<String>,
    #[serde(default)]
    pub done: Option<bool>,
}

fn now_iso() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// 把标题压成单行，frontmatter 不允许换行
fn one_line(s: &str) -> String {
    s.lines().next().unwrap_or("").trim().to_string()
}

/// 去掉 Markdown 标记，得到用于摘要的纯文本
pub fn plain_text(markdown: &str) -> String {
    let mut out = String::new();
    for line in markdown.lines() {
        let mut t = line.trim();

        // 引用符号
        if let Some(rest) = t.strip_prefix('>') {
            t = rest.trim_start();
        }
        // 列表符号（只剥一层，避免误伤 *斜体*）
        for marker in ["- ", "* ", "+ "] {
            if let Some(rest) = t.strip_prefix(marker) {
                t = rest;
                break;
            }
        }
        // 任务清单标记
        for marker in ["[ ] ", "[x] ", "[X] "] {
            if let Some(rest) = t.strip_prefix(marker) {
                t = rest;
                break;
            }
        }
        // 标题符号
        t = t.trim_start_matches('#').trim_start();

        if t.is_empty() {
            continue;
        }

        let t = strip_links(&strip_inline_marks(t));
        if t.is_empty() {
            continue;
        }

        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&t);
        if out.chars().count() > 160 {
            break;
        }
    }
    out
}

/// `[文字](地址)` → `文字`
fn strip_links(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' {
            if let Some(close) = (i + 1..chars.len()).find(|&j| chars[j] == ']') {
                if close + 1 < chars.len() && chars[close + 1] == '(' {
                    if let Some(paren) = (close + 2..chars.len()).find(|&j| chars[j] == ')') {
                        out.extend(chars[i + 1..close].iter());
                        i = paren + 1;
                        continue;
                    }
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// 去掉行内的强调与代码符号
fn strip_inline_marks(s: &str) -> String {
    s.replace("**", "")
        .replace("~~", "")
        .replace('*', "")
        .replace('`', "")
}

/// 统计正文里的 `- [ ]` / `- [x]` 复选框进度
fn count_todos(body: &str) -> (u32, u32) {
    let mut total = 0;
    let mut done = 0;
    for line in body.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("- [ ]") {
            total += 1;
            let _ = rest;
        } else if let Some(rest) = t.strip_prefix("- [x]") {
            total += 1;
            done += 1;
            let _ = rest;
        }
    }
    (total, done)
}

/// 拆分 frontmatter 与正文
pub fn split_frontmatter(raw: &str) -> (Vec<(String, String)>, String) {
    let mut fm: Vec<(String, String)> = Vec::new();
    let mut lines = raw.lines();

    let first = lines.next();
    match first {
        Some(f) if f.trim() == FM_DELIM => {
            let mut consumed: Vec<&str> = Vec::new();
            let mut closed = false;
            for line in lines.by_ref() {
                consumed.push(line);
                let t = line.trim();
                if t == FM_DELIM {
                    closed = true;
                    break;
                }
                if let Some((k, v)) = t.split_once(':') {
                    let k = k.trim();
                    if k.is_empty() {
                        continue;
                    }
                    fm.push((k.to_string(), v.trim().to_string()));
                }
            }
            if !closed {
                // 没有闭合的 --- ，整份文件按正文处理，避免内容丢失
                let mut body = String::from(FM_DELIM);
                for l in consumed {
                    body.push('\n');
                    body.push_str(l);
                }
                return (Vec::new(), body);
            }
            let body = lines.collect::<Vec<_>>().join("\n");
            (fm, body.trim().to_string())
        }
        Some(first) => {
            // 没有 frontmatter，整份都是正文
            let mut body = first.to_string();
            for l in lines {
                body.push('\n');
                body.push_str(l);
            }
            (fm, body)
        }
        None => (fm, String::new()),
    }
}

/// 由 frontmatter 键值对 + 正文组装出完整笔记
fn assemble(id: String, fm: Vec<(String, String)>, body: String) -> NoteMeta {
    let mut title = String::new();
    let mut created = String::new();
    let mut updated = String::new();
    let mut pinned = false;
    let mut due: Option<String> = None;
    let mut done = false;

    for (k, v) in fm {
        match k.as_str() {
            "title" => title = v,
            "created" => created = v,
            "updated" => updated = v,
            "pinned" => pinned = v == "true",
            "due" => {
                if !v.is_empty() {
                    due = Some(v);
                }
            }
            "done" => done = v == "true",
            _ => {}
        }
    }

    let (todo_total, todo_done) = count_todos(&body);
    let preview = plain_text(&body);

    NoteMeta {
        id,
        title,
        created,
        updated,
        pinned,
        due,
        done,
        preview: preview.chars().take(120).collect(),
        todo_total,
        todo_done,
        body,
    }
}

/// 读单个文件
pub fn read_note_file(path: &Path) -> Option<NoteMeta> {
    let raw = fs::read_to_string(path).ok()?;
    let id = path.file_stem()?.to_string_lossy().to_string();
    let (fm, body) = split_frontmatter(&raw);
    Some(assemble(id, fm, body))
}

/// 列出目录下所有笔记，按更新时间倒序（置顶分组由前端负责）
pub fn list_notes(dir: &Path) -> Vec<NoteMeta> {
    let mut out: Vec<NoteMeta> = match fs::read_dir(dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
            .filter_map(|e| read_note_file(&e.path()))
            .collect(),
        Err(_) => Vec::new(),
    };
    out.sort_by(|a, b| b.updated.cmp(&a.updated));
    out
}

/// 把正文与元数据渲染成最终文件内容
pub fn render(meta: &NoteMeta) -> String {
    let mut out = String::new();
    out.push_str(FM_DELIM);
    out.push('\n');
    out.push_str(&format!("title: {}\n", one_line(&meta.title)));
    out.push_str(&format!("created: {}\n", meta.created));
    out.push_str(&format!("updated: {}\n", meta.updated));
    out.push_str(&format!("pinned: {}\n", meta.pinned));
    if let Some(d) = &meta.due {
        out.push_str(&format!("due: {}\n", d));
    }
    out.push_str(&format!("done: {}\n", meta.done));
    out.push_str(FM_DELIM);
    out.push('\n');
    out.push('\n');
    out.push_str(meta.body.trim());
    out.push('\n');
    out
}

/// 原子写：先写 `.tmp` 再 rename 覆盖
pub fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
    let tmp = path.with_extension("md.tmp");
    fs::write(&tmp, content).map_err(|e| format!("写入临时文件失败: {e}"))?;
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("替换原文件失败: {e}"));
    }
    Ok(())
}

/// 由标题生成文件名主干：保留字母数字（含中文）与 - _，其余替换
pub fn sanitize_stem(title: &str) -> String {
    let mut s: String = title
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if s.trim().is_empty() {
        s = "note".to_string();
    }
    s = s.chars().take(24).collect();
    s
}

/// 生成短随机后缀，避免同名冲突
pub fn short_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    // 再混入进程内计数，保证同毫秒内创建的笔记也不撞
    static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{:04x}", ((nanos ^ (c as u64) ^ (std::process::id() as u64)) & 0xffff))
}

/// 为新笔记分配不冲突的 id
pub fn unique_id(dir: &Path, title: &str) -> String {
    let stem = sanitize_stem(title);
    let mut candidate = format!("{stem}-{}", short_suffix());
    let mut n = 1;
    while dir.join(format!("{candidate}.md")).exists() {
        candidate = format!("{stem}-{}-{n}", short_suffix());
        n += 1;
    }
    candidate
}

pub fn new_meta(id: String, title: String, body: String) -> NoteMeta {
    let ts = now_iso();
    let (todo_total, todo_done) = count_todos(&body);
    NoteMeta {
        id,
        title,
        created: ts.clone(),
        updated: ts,
        pinned: false,
        due: None,
        done: false,
        preview: plain_text(&body).chars().take(120).collect(),
        todo_total,
        todo_done,
        body,
    }
}

/// 落盘一条新笔记，返回完整元信息
pub fn create_note(dir: &Path, title: &str, body: &str) -> Result<NoteMeta, String> {
    let id = unique_id(dir, title);
    let mut meta = new_meta(id, title.to_string(), body.to_string());
    if meta.title.trim().is_empty() {
        meta.title = "无标题".to_string();
    }
    let path = dir.join(format!("{}.md", meta.id));
    atomic_write(&path, &render(&meta))?;
    Ok(meta)
}

/// 更新正文与标题，保留 pinned / due / done。
///
/// `expected` 是客户端载入时的正文。若磁盘上的正文已经和它不一致，
/// 说明文件在应用之外被改过，此时拒绝覆盖，把冲突交还给调用方。
pub fn save_note(
    dir: &Path,
    id: &str,
    title: &str,
    body: &str,
    expected: Option<&str>,
) -> Result<NoteMeta, String> {
    let path = dir.join(format!("{id}.md"));
    let mut meta = read_note_file(&path).ok_or_else(|| format!("找不到笔记 {id}"))?;

    if let Some(exp) = expected {
        if meta.body.trim() != exp.trim() {
            return Err("文件已被外部修改".to_string());
        }
    }

    meta.title = if title.trim().is_empty() {
        "无标题".to_string()
    } else {
        one_line(title)
    };
    meta.body = body.to_string();
    meta.updated = now_iso();
    let (todo_total, todo_done) = count_todos(&meta.body);
    meta.todo_total = todo_total;
    meta.todo_done = todo_done;
    meta.preview = plain_text(&meta.body).chars().take(120).collect();
    atomic_write(&path, &render(&meta))?;
    Ok(meta)
}

/// 只改元数据，正文原样保留
pub fn patch_note(dir: &Path, patch: &NotePatch) -> Result<NoteMeta, String> {
    let path = dir.join(format!("{}.md", patch.id));
    let mut meta = read_note_file(&path)
        .ok_or_else(|| format!("找不到笔记 {}", patch.id))?;
    if let Some(t) = &patch.title {
        meta.title = one_line(t);
    }
    if let Some(p) = patch.pinned {
        meta.pinned = p;
    }
    if let Some(d) = &patch.due {
        meta.due = if d.trim().is_empty() {
            None
        } else {
            Some(d.trim().to_string())
        };
    }
    if let Some(d) = patch.done {
        meta.done = d;
    }
    meta.updated = now_iso();
    atomic_write(&path, &render(&meta))?;
    Ok(meta)
}

pub fn delete_note(dir: &Path, id: &str) -> Result<(), String> {
    let path = dir.join(format!("{id}.md"));
    fs::remove_file(&path).map_err(|e| format!("删除失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_roundtrip() {
        let raw = "---\ntitle: 周会纪要\ncreated: 2026-11-02T14:20:00\nupdated: 2026-11-02T15:00:00\npinned: true\ndue: 2026-11-05\ndone: false\n---\n\n正文第一行\n第二行\n";
        let (fm, body) = split_frontmatter(raw);
        let meta = assemble("x".into(), fm, body);
        assert_eq!(meta.title, "周会纪要");
        assert!(meta.pinned);
        assert_eq!(meta.due.as_deref(), Some("2026-11-05"));
        assert!(!meta.done);
        assert_eq!(meta.body, "正文第一行\n第二行");

        // 再渲染回去，字段应完全一致
        let rendered = render(&meta);
        let (fm2, body2) = split_frontmatter(&rendered);
        let meta2 = assemble("x".into(), fm2, body2);
        assert_eq!(meta2.title, meta.title);
        assert_eq!(meta2.pinned, meta.pinned);
        assert_eq!(meta2.due, meta.due);
        assert_eq!(meta2.done, meta.done);
        assert_eq!(meta2.body, meta.body);
    }

    #[test]
    fn title_with_colon_survives() {
        let raw = "---\ntitle: 周会: 讨论路线图\n---\n\n内容\n";
        let (fm, body) = split_frontmatter(raw);
        let meta = assemble("x".into(), fm, body);
        assert_eq!(meta.title, "周会: 讨论路线图");
    }

    #[test]
    fn no_frontmatter_is_all_body() {
        let (fm, body) = split_frontmatter("第一行\n第二行");
        assert!(fm.is_empty());
        assert_eq!(body, "第一行\n第二行");
    }

    #[test]
    fn unclosed_frontmatter_keeps_everything() {
        let (fm, body) = split_frontmatter("---\ntitle: 只有开头\n正文");
        assert!(fm.is_empty());
        assert_eq!(body, "---\ntitle: 只有开头\n正文");
    }

    #[test]
    fn counts_checkboxes() {
        let (total, done) = count_todos("- [x] a\n- [ ] b\n- [ ] c\n普通行");
        assert_eq!(total, 3);
        assert_eq!(done, 1);
    }

    #[test]
    fn plain_text_strips_markup() {
        let s = plain_text("# 标题\n\n- [ ] 待办一\n- [x] 待办二\n\n正文 **粗体**");
        assert!(s.contains("标题"));
        assert!(s.contains("待办一"));
        assert!(!s.contains("[ ]"));
        assert!(!s.contains("**"));
    }

    #[test]
    fn sanitize_keeps_cjk_and_drops_unsafe() {
        assert_eq!(sanitize_stem("周会: 讨论/路线图"), "周会讨论路线图");
        assert_eq!(sanitize_stem("   "), "note");
    }

    /// 存储层端到端：真实文件系统上的 新建 → 读回 → 改正文 → 改元数据 → 列表 → 删除
    #[test]
    fn storage_roundtrip_on_real_filesystem() {
        let dir = std::env::temp_dir().join(format!("mininote-test-{}", short_suffix()));
        std::fs::create_dir_all(&dir).unwrap();

        // 新建
        let meta = create_note(&dir, "周会纪要", "第一段\n\n- [ ] 待办\n").unwrap();
        assert_eq!(meta.title, "周会纪要");
        assert_eq!(meta.todo_total, 1);
        assert_eq!(meta.todo_done, 0);
        assert!(meta.due.is_none());

        // 文件确实落盘，且是合法的 markdown + frontmatter
        let path = dir.join(format!("{}.md", meta.id));
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.starts_with("---\n"), "frontmatter 开头: {raw:?}");
        assert!(raw.contains("title: 周会纪要"));
        assert!(raw.contains("- [ ] 待办"));

        // 读回来
        let back = read_note_file(&path).unwrap();
        assert_eq!(back.title, "周会纪要");
        assert_eq!(back.body, "第一段\n\n- [ ] 待办");
        assert_eq!(back.todo_total, 1);

        // 改正文
        let saved = save_note(&dir, &meta.id, "周会纪要", "改过的正文\n\n- [x] 做好了", None).unwrap();
        assert_eq!(saved.todo_done, 1);
        assert_eq!(saved.todo_total, 1);
        assert!(saved.updated.as_str() >= meta.updated.as_str());

        // 磁盘版本对不上时必须拒绝覆盖，绝不静默吞掉外部修改
        let conflict = save_note(
            &dir,
            &meta.id,
            "周会纪要",
            "会覆盖掉别人改的内容",
            Some("客户端载入时的旧正文"),
        );
        assert!(conflict.is_err(), "应当报告冲突而不是覆盖");
        let still = read_note_file(&path).unwrap();
        assert_eq!(still.body, "改过的正文\n\n- [x] 做好了", "冲突时磁盘内容不能被改");

        // 版本一致时正常写入
        let ok = save_note(
            &dir,
            &meta.id,
            "周会纪要",
            "改过的正文\n\n- [x] 做好了",
            Some("改过的正文\n\n- [x] 做好了"),
        )
        .unwrap();
        assert_eq!(ok.body, "改过的正文\n\n- [x] 做好了");

        // 改元数据不应动正文
        let patched = patch_note(
            &dir,
            &NotePatch {
                id: meta.id.clone(),
                title: None,
                pinned: Some(true),
                due: Some("2026-11-05".into()),
                done: None,
            },
        )
        .unwrap();
        assert!(patched.pinned);
        assert_eq!(patched.due.as_deref(), Some("2026-11-05"));
        assert_eq!(patched.body, "改过的正文\n\n- [x] 做好了");

        // 清空到期日
        let cleared = patch_note(
            &dir,
            &NotePatch {
                id: meta.id.clone(),
                title: None,
                pinned: None,
                due: Some(String::new()),
                done: None,
            },
        )
        .unwrap();
        assert!(cleared.due.is_none());

        // 列表
        let list = list_notes(&dir);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, meta.id);

        // 摘要取自正文
        assert!(list[0].preview.contains("改过的正文"));

        // 删除
        delete_note(&dir, &meta.id).unwrap();
        assert!(list_notes(&dir).is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn empty_title_falls_back() {
        let dir = std::env::temp_dir().join(format!("mininote-test-{}", short_suffix()));
        std::fs::create_dir_all(&dir).unwrap();
        let meta = create_note(&dir, "   ", "正文").unwrap();
        assert_eq!(meta.title, "无标题");
        std::fs::remove_dir_all(&dir).ok();
    }
}
