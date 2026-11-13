import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { EditorContent, useEditor, type Editor } from "@tiptap/react";
import { BubbleMenu } from "@tiptap/react/menus";
import StarterKit from "@tiptap/starter-kit";
import TaskList from "@tiptap/extension-task-list";
import TaskItem from "@tiptap/extension-task-item";
import Placeholder from "@tiptap/extension-placeholder";
import { Markdown } from "@tiptap/markdown";
import { Bold, Italic, Strikethrough, Code, Heading2, ListChecks, Quote } from "lucide-react";
import { api } from "../lib/api";
import { useAppStore } from "../store/appStore";

/** 自动保存的防抖间隔 */
const SAVE_DEBOUNCE_MS = 400;

type SaveState = "idle" | "saving" | "saved" | "error" | "conflict";

/** 从正文里推导标题：取第一行非空文本，去掉 Markdown 标记 */
export function deriveTitle(body: string): string {
  for (const raw of body.split("\n")) {
    const line = raw.trim();
    if (!line) continue;
    if (line.startsWith("```") || line.startsWith("---")) continue;
    const text = line
      .replace(/^#+\s*/, "")
      .replace(/^[-*+]\s+/, "")
      .replace(/^\[[ xX]\]\s*/, "")
      .trim();
    if (text) return text.slice(0, 40);
  }
  return "无标题";
}

interface Props {
  /** 当前笔记 id；null 表示一条尚未保存的新笔记 */
  noteId: string | null;
}

/**
 * 笔记编辑区：标题 + 所见即所得正文 + 选中浮出的格式工具条。
 *
 * 数据流：TipTap 是正文的唯一真相源，Markdown 字符串由它产出；
 * 磁盘文件是最终的真相源，自动保存把 Markdown 写回 Rust 侧。
 */
export function EditorPane({ noteId }: Props) {
  const loaded = useAppStore((s) => s.loaded);
  const upsertNote = useAppStore((s) => s.upsertNote);
  const setCurrentId = useAppStore((s) => s.setCurrentId);
  const setMarkdownGetter = useAppStore((s) => s.setMarkdownGetter);

  const idRef = useRef<string | null>(null);
  const bodyRef = useRef("");
  const titleRef = useRef("");
  const dirtyRef = useRef(false);
  /** 磁盘上当前的正文。写盘时作为「期望版本」传给后端做冲突检测，
   *  同时用来判断「内容其实没变」——没变就绝不重写文件。 */
  const baselineRef = useRef("");
  /** 程序化写入编辑器时抑制 onUpdate，避免把刚加载的内容当成用户修改 */
  const suppressRef = useRef(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  /** 标题输入框。新建笔记后光标要落在这里。 */
  const titleInputRef = useRef<HTMLInputElement>(null);
  /** 最新的当前笔记 id。落盘时用它判断「用户还在不在这条新笔记上」。 */
  const currentIdRef = useRef<string | null>(noteId);
  currentIdRef.current = noteId;

  const [title, setTitle] = useState("");
  const [saveState, setSaveState] = useState<SaveState>("idle");

  /** 把当前内容落盘 */
  const flush = useCallback(async () => {
    if (!dirtyRef.current) return;
    dirtyRef.current = false;
    const body = bodyRef.current;
    const t = titleRef.current.trim() || deriveTitle(body);
    setSaveState("saving");
    try {
      if (idRef.current) {
        const meta = await api.saveNote(idRef.current, t, body, baselineRef.current);
        upsertNote(meta);
      } else {
        // 空白的新笔记不落盘
        if (!body.trim() && !t.trim()) {
          setSaveState("idle");
          return;
        }
        const meta = await api.createNote(t, body);
        // 期间用户可能已经切到别的笔记去了：只有在还停留在这条新笔记上
        // 时才认领它的 id、才把选中项切过去，否则会抢走用户的导航。
        if (idRef.current === null) idRef.current = meta.id;
        if (currentIdRef.current === null) setCurrentId(meta.id);
        upsertNote(meta);
      }
      baselineRef.current = body;
      setSaveState("saved");
    } catch (e) {
      const message = String(e);
      if (message.includes("外部修改")) {
        // 磁盘上的版本已经变了，绝不覆盖，交给用户决定
        setSaveState("conflict");
        dirtyRef.current = true;
      } else {
        // 保存失败就保持脏状态，下次输入会再试
        dirtyRef.current = true;
        setSaveState("error");
        console.error("保存失败", e);
      }
    }
  }, [upsertNote, setCurrentId]);

  const flushRef = useRef(flush);
  flushRef.current = flush;

  const scheduleSave = useCallback(() => {
    if (timerRef.current) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(() => {
      void flushRef.current();
    }, SAVE_DEBOUNCE_MS);
  }, []);

  const editor = useEditor({
    extensions: [
      StarterKit.configure({
        link: { openOnClick: false },
        heading: { levels: [1, 2, 3] },
      }),
      TaskList,
      TaskItem.configure({ nested: true }),
      Placeholder.configure({ placeholder: "开始写点什么…" }),
      Markdown.configure({ indentation: { style: "space", size: 2 } }),
    ],
    content: "",
    contentType: "markdown",
    editorProps: {
      attributes: { class: "pm", spellcheck: "false" },
    },
    onUpdate: ({ editor: ed }) => {
      if (suppressRef.current) return;
      const md = ed.getMarkdown();
      bodyRef.current = md;
      // 与磁盘上的版本比对：内容其实没变就别写盘。
      // 这比靠时序敏感的 suppress 标志可靠得多——载入时哪怕 onUpdate
      // 延迟触发，也不可能把「没改过」的文件重写一遍。
      if (md.trim() === baselineRef.current.trim()) {
        dirtyRef.current = false;
        return;
      }
      dirtyRef.current = true;
      setSaveState("idle");
      scheduleSave();
    },
  });

  // 把「取当前 Markdown 原文」注册给顶栏的复制按钮。
  // 读的是编辑器此刻的状态而不是磁盘文件，所以刚打完字、以及还没落盘
  // 的新笔记，复制到的都是屏幕上看到的内容。
  useEffect(() => {
    if (!editor) return;
    setMarkdownGetter(() => editor.getMarkdown());
    return () => setMarkdownGetter(null);
  }, [editor, setMarkdownGetter]);

  // 切换笔记时把内容装载进编辑器。
  //
  // 依赖里刻意不放 notes：笔记列表每保存一次就会变，若把它当依赖，
  // 列表一更新就重新装载一次，正在编辑的内容会被冲掉。
  useEffect(() => {
    if (!editor || !loaded) return;

    if (noteId === null) {
      const reset = () => {
        suppressRef.current = true;
        editor.commands.setContent("", { contentType: "markdown" });
        suppressRef.current = false;
        idRef.current = null;
        bodyRef.current = "";
        titleRef.current = "";
        baselineRef.current = "";
        dirtyRef.current = false;
        setTitle("");
        setSaveState("idle");
        // 新建笔记后光标落在标题上，直接就能起标题。
        // 窗口本身还没获得焦点时不要抢——应用常驻托盘，
        // 启动时若把光标抢走会打断用户正在做的事。
        if (document.hasFocus()) {
          titleInputRef.current?.focus();
        }
      };
      // 上一条笔记还有没写完的改动：先写盘再清空，否则内容会被抹掉
      if (dirtyRef.current) {
        void flushRef.current().then(reset);
        return;
      }
      reset();
      return;
    }

    let cancelled = false;
    void api.readNote(noteId).then((full) => {
      if (cancelled || !full) return;
      suppressRef.current = true;
      editor.commands.setContent(full.body, { contentType: "markdown" });
      suppressRef.current = false;
      idRef.current = full.id;
      bodyRef.current = full.body;
      titleRef.current = full.title;
      baselineRef.current = full.body;
      dirtyRef.current = false;
      setTitle(full.title);
      setSaveState("idle");
    });
    return () => {
      cancelled = true;
    };
  }, [editor, noteId, loaded, setTitle, setSaveState]);

  // 离开页面前把未保存的内容冲掉
  useEffect(() => {
    const handler = () => {
      if (timerRef.current) clearTimeout(timerRef.current);
      void flushRef.current();
    };
    window.addEventListener("beforeunload", handler);
    return () => window.removeEventListener("beforeunload", handler);
  }, []);

  // 主动唤起后，把光标放到正文末尾，直接就能打字。
  //
  // Rust 侧现在只在窗口真正拿到 key 之后才发这个事件。但 webview 的
  // document.hasFocus() 可能比窗口的 Focused 事件晚一帧就绪，所以这里
  // 最多等几帧再落光标 —— 没有 document 焦点时的 focus() 只是画了个假光标，
  // 按键会全部打进原来那个全屏 App 里，比没有光标更糟。
  useEffect(() => {
    if (!editor) return;
    let un: (() => void) | undefined;
    void listen("focus-editor", () => {
      let tries = 0;
      const tryFocus = () => {
        if (document.hasFocus()) {
          editor.commands.focus("end");
        } else if (tries++ < 5) {
          requestAnimationFrame(tryFocus);
        }
      };
      requestAnimationFrame(tryFocus);
    }).then((fn) => {
      un = fn;
    });
    return () => un?.();
  }, [editor]);

  // 应用退出前：把未保存的写完，再告诉 Rust 可以退出了
  useEffect(() => {
    let un: (() => void) | undefined;
    void listen("app-quitting", async () => {
      if (timerRef.current) clearTimeout(timerRef.current);
      await flushRef.current();
      await api.quitReady();
    }).then((fn) => {
      un = fn;
    });
    return () => un?.();
  }, []);

  // 「已保存」提示停留一会儿就淡出，不长期占位
  useEffect(() => {
    if (saveState !== "saved") return;
    const t = setTimeout(() => setSaveState("idle"), 1600);
    return () => clearTimeout(t);
  }, [saveState]);

  const onTitleChange = (value: string) => {
    titleRef.current = value;
    setTitle(value);
    dirtyRef.current = true;
    setSaveState("idle");
    scheduleSave();
  };

  return (
    <div className="notePane">
      <div className="notePane__head">
        <input
          className="titleInput"
          ref={titleInputRef}
          value={title}
          placeholder="无标题"
          spellCheck={false}
          onChange={(e) => onTitleChange(e.target.value)}
          onKeyDown={(e) => {
            // 回车跳到正文：标题写完直接往下写，不用去点正文
            if (e.key === "Enter") {
              e.preventDefault();
              editor?.commands.focus();
            }
          }}
        />
        <SaveIndicator state={saveState} />
      </div>
      <div className="editorScroll scroll">
        <EditorContent editor={editor} />
      </div>
      {editor && <BubbleToolbar editor={editor} />}
    </div>
  );
}

function SaveIndicator({ state }: { state: SaveState }) {
  if (state === "idle") return null;
  if (state === "conflict") {
    return <span className="saveDot saveDot--conflict">文件已被外部修改</span>;
  }
  return (
    <span className={`saveDot saveDot--${state}`}>
      {state === "saving" ? "保存中" : state === "saved" ? "已保存" : "保存失败"}
    </span>
  );
}

/** 选中文字时浮出的格式工具条 */
function BubbleToolbar({ editor }: { editor: Editor }) {
  const btn = (
    icon: React.ReactNode,
    active: boolean,
    run: () => void,
    title: string,
  ) => (
    <button
      type="button"
      data-on={active}
      title={title}
      onMouseDown={(e) => e.preventDefault()}
      onClick={run}
    >
      {icon}
    </button>
  );

  return (
    <BubbleMenu editor={editor} options={{ placement: "top", offset: 8 }}>
      <div className="bubble">
        {btn(<Bold size={13} />, editor.isActive("bold"), () => editor.chain().focus().toggleBold().run(), "粗体 ⌘B")}
        {btn(<Italic size={13} />, editor.isActive("italic"), () => editor.chain().focus().toggleItalic().run(), "斜体 ⌘I")}
        {btn(<Strikethrough size={13} />, editor.isActive("strike"), () => editor.chain().focus().toggleStrike().run(), "删除线")}
        {btn(<Code size={13} />, editor.isActive("code"), () => editor.chain().focus().toggleCode().run(), "行内代码")}
        <span className="bubble__sep" />
        {btn(<Heading2 size={13} />, editor.isActive("heading", { level: 2 }), () => editor.chain().focus().toggleHeading({ level: 2 }).run(), "标题")}
        {btn(<ListChecks size={13} />, editor.isActive("taskList"), () => editor.chain().focus().toggleTaskList().run(), "待办清单")}
        {btn(<Quote size={13} />, editor.isActive("blockquote"), () => editor.chain().focus().toggleBlockquote().run(), "引用")}
      </div>
    </BubbleMenu>
  );
}
