import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { Maximize2, Minimize2, FolderOpen, SquarePen } from "lucide-react";
import { api } from "./lib/api";
import { pendingTodoCount } from "./lib/time";
import { useAppStore } from "./store/appStore";
import { EditorPane } from "./components/EditorPane";

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
          <button className="iconbtn" title="打开笔记目录" onClick={() => void api.openNotesDir()}>
            <FolderOpen size={14} strokeWidth={2} />
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
