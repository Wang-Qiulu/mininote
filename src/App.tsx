import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Check, Copy, Maximize2, Minimize2, SquarePen } from "lucide-react";
import { api } from "./lib/api";
import { pendingTodoCount } from "./lib/time";
import { useAppStore } from "./store/appStore";
import { EditorPane } from "./components/EditorPane";

type CopyState = "idle" | "ok" | "fail";

/**
 * 把文本写进系统剪贴板。
 * webview 的标准接口需要用户手势；万一被拒，退回临时文本框 + execCommand，
 * 这是 WKWebView 上仍然可用的老办法。两条路都不需要新增插件或系统权限。
 */
async function copyText(text: string): Promise<boolean> {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch {
    // 落到下面的兜底
  }
  const ta = document.createElement("textarea");
  ta.value = text;
  ta.setAttribute("readonly", "");
  ta.style.cssText = "position:fixed;top:0;left:-9999px;opacity:0;";
  document.body.appendChild(ta);
  ta.select();
  let ok = false;
  try {
    ok = document.execCommand("copy");
  } catch {
    ok = false;
  }
  ta.remove();
  return ok;
}

export default function App() {
  const mode = useAppStore((s) => s.mode);
  const morphing = useAppStore((s) => s.morphing);
  const notes = useAppStore((s) => s.notes);
  const currentId = useAppStore((s) => s.currentId);
  const setNotes = useAppStore((s) => s.setNotes);
  const setMode = useAppStore((s) => s.setMode);
  const setMorphing = useAppStore((s) => s.setMorphing);
  const setLoaded = useAppStore((s) => s.setLoaded);
  const setCurrentId = useAppStore((s) => s.setCurrentId);

  const [copyState, setCopyState] = useState<CopyState>("idle");
  const copyTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (copyTimerRef.current) clearTimeout(copyTimerRef.current);
    };
  }, []);

  /** 一键复制当前笔记正文的 Markdown 原文（带符号，不含 frontmatter 与笔记标题） */
  const onCopy = useCallback(async () => {
    // 取编辑器此刻的状态，而不是磁盘上的文件：刚打完字、以及还没落盘
    // 的新笔记，复制到的都是屏幕上看到的内容。
    const getter = useAppStore.getState().markdownGetter;
    const ok = getter ? await copyText(getter()) : false;
    setCopyState(ok ? "ok" : "fail");
    if (copyTimerRef.current) clearTimeout(copyTimerRef.current);
    copyTimerRef.current = setTimeout(() => setCopyState("idle"), 1600);
  }, []);

  // Rust 侧驱动形变动画，前端只做内容层淡化
  useEffect(() => {
    let disposed = false;
    const cleanups: Array<() => void> = [];

    void (async () => {
      const [offStart, offEnd, offChanged] = await Promise.all([
        listen<string>("window-mode-start", (e) => {
          // 展开时立刻切到展开布局：这样笔记区在整个动画过程中宽度锁死，
          // 既不会拉伸也不会重排，视觉上完全不动。
          // 收回时保持展开布局到最后一帧，动画结束后再切回小窗布局。
          if (e.payload === "expanded") setMode("expanded");
          setMorphing(true);
        }),
        listen<string>("window-mode-end", (e) => {
          setMode(e.payload === "expanded" ? "expanded" : "mini");
          setMorphing(false);
        }),
        listen<string>("notes-changed", () => {
          void api.listNotes().then((list) => {
            setNotes(list);
            setLoaded(true);
          });
        }),
      ]);
      if (disposed) {
        offStart();
        offEnd();
        offChanged();
        return;
      }
      cleanups.push(offStart, offEnd, offChanged);
    })();

    // 启动时打开最近编辑的那条
    void api.listNotes().then((list) => {
      setNotes(list);
      setLoaded(true);
      setCurrentId(list[0]?.id ?? null);
    });

    return () => {
      disposed = true;
      cleanups.forEach((fn) => fn());
    };
  }, [setNotes, setMode, setMorphing, setLoaded, setCurrentId]);

  const pending = pendingTodoCount(notes);

  return (
    <div className="app" data-mode={mode} data-morphing={morphing}>
      <div className="grain" />

      <header className="titlebar" data-tauri-drag-region="deep">
        <div className="titlebar__spacer" data-tauri-drag-region="deep">
          {mode === "expanded" ? "全部笔记" : ""}
        </div>
        <div className="titlebar__actions">
          {pending > 0 && (
            <span className="badge" title={`${pending} 个待办到期`}>
              {pending}
            </span>
          )}
          <button
            className="iconbtn"
            title="新建笔记"
            onClick={() => setCurrentId(null)}
          >
            <SquarePen size={14} strokeWidth={2} />
          </button>
          <button
            className="iconbtn"
            title={mode === "expanded" ? "收起为小窗" : "展开为全部笔记"}
            onClick={() => void api.setWindowMode(mode === "expanded" ? "mini" : "expanded")}
          >
            {mode === "expanded" ? (
              <Minimize2 size={14} strokeWidth={2} />
            ) : (
              <Maximize2 size={14} strokeWidth={2} />
            )}
          </button>
          <button
            className="iconbtn"
            data-active={copyState === "ok"}
            data-error={copyState === "fail"}
            title={
              copyState === "ok"
                ? "已复制"
                : copyState === "fail"
                  ? "复制失败"
                  : "复制 Markdown 原文"
            }
            onClick={() => void onCopy()}
          >
            {copyState === "ok" ? (
              <Check size={14} strokeWidth={2} />
            ) : (
              <Copy size={14} strokeWidth={2} />
            )}
          </button>
        </div>
      </header>

      <div className="shell">
        <aside className="sidebar">
          <div className="sidebar__inner scroll">
            {notes.length === 0 && <p className="empty">还没有笔记</p>}
            {notes.map((n) => (
              <div
                key={n.id}
                className="row"
                data-current={n.id === currentId}
                onClick={() => setCurrentId(n.id)}
              >
                {n.pinned && <span className="row__pin">置顶</span>}
                <span className="row__title">{n.title || "无标题"}</span>
              </div>
            ))}
          </div>
        </aside>

        <section className="editorPane">
          <EditorPane noteId={currentId} />
        </section>
      </div>
    </div>
  );
}
