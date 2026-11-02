import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Maximize2, Minimize2, FolderOpen, SquarePen } from "lucide-react";
import { api } from "./lib/api";
import { pendingTodoCount } from "./lib/time";
import { useAppStore } from "./store/appStore";

export default function App() {
  const [dir, setDir] = useState("");
  const mode = useAppStore((s) => s.mode);
  const morphing = useAppStore((s) => s.morphing);
  const notes = useAppStore((s) => s.notes);
  const setNotes = useAppStore((s) => s.setNotes);
  const setMode = useAppStore((s) => s.setMode);
  const setMorphing = useAppStore((s) => s.setMorphing);
  const setLoaded = useAppStore((s) => s.setLoaded);

  // Rust 侧驱动形变动画，前端只做内容层淡化
  useEffect(() => {
    let disposed = false;
    const cleanups: Array<() => void> = [];

    void (async () => {
      const [
        offStart,
        offEnd,
        offChanged,
      ] = await Promise.all([
        listen<string>("window-mode-start", () => setMorphing(true)),
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

    void api.listNotes().then((list) => {
      setNotes(list);
      setLoaded(true);
    });
    void api.notesDir().then(setDir);

    return () => {
      disposed = true;
      cleanups.forEach((fn) => fn());
    };
  }, [setNotes, setMode, setMorphing, setLoaded]);

  const pending = pendingTodoCount(notes);

  return (
    <div className="app" data-mode={mode} data-morphing={morphing}>
      <div className="grain" />

      <header className="titlebar" data-tauri-drag-region>
        <div className="titlebar__spacer" data-tauri-drag-region>
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
            onClick={() => void api.setWindowMode("mini")}
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
            title={dir ? `打开笔记目录：${dir}` : "打开笔记目录"}
            onClick={() => void api.openNotesDir()}
          >
            <FolderOpen size={14} strokeWidth={2} />
          </button>
        </div>
      </header>

      <div className="shell">
        <aside className="sidebar">
          <div className="sidebar__inner scroll">
            {notes.length === 0 && <p className="empty">还没有笔记</p>}
            {notes.map((n) => (
              <div key={n.id} className="row">
                {n.pinned && <span className="row__pin">置顶</span>}
                <span className="row__title">{n.title || "无标题"}</span>
              </div>
            ))}
          </div>
        </aside>

        <section className="editorPane">
          <div className="editorScroll scroll">
            <p className="placeholder">编辑器将在下一阶段接入（TipTap 所见即所得）</p>
          </div>
        </section>
      </div>
    </div>
  );
}
