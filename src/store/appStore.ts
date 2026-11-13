import { create } from "zustand";
import type { NoteMeta } from "../lib/api";

export type WindowMode = "mini" | "expanded";

interface AppState {
  notes: NoteMeta[];
  currentId: string | null;
  mode: WindowMode;
  /** 形变过程中为 true：内容淡出，避免尺寸变化时的重排跳变 */
  morphing: boolean;
  loaded: boolean;
  /** 读取编辑器此刻的 Markdown 原文。顶栏的复制按钮不在编辑器组件树里，
   *  由 EditorPane 把「取原文」注册进来。用 getState() 读、不订阅，
   *  就不会因为每敲一个字而重渲染顶栏。 */
  markdownGetter: (() => string) | null;

  setNotes: (notes: NoteMeta[]) => void;
  upsertNote: (note: NoteMeta) => void;
  removeNote: (id: string) => void;
  setCurrentId: (id: string | null) => void;
  setMode: (mode: WindowMode) => void;
  setMorphing: (morphing: boolean) => void;
  setLoaded: (loaded: boolean) => void;
  setMarkdownGetter: (getter: (() => string) | null) => void;
}

export const useAppStore = create<AppState>((set) => ({
  notes: [],
  currentId: null,
  mode: "mini",
  morphing: false,
  loaded: false,
  markdownGetter: null,

  setNotes: (notes) => set({ notes }),
  upsertNote: (note) =>
    set((s) => {
      const idx = s.notes.findIndex((n) => n.id === note.id);
      const notes = idx >= 0 ? s.notes.map((n) => (n.id === note.id ? note : n)) : [note, ...s.notes];
      notes.sort((a, b) => b.updated.localeCompare(a.updated));
      return { notes };
    }),
  removeNote: (id) => set((s) => ({ notes: s.notes.filter((n) => n.id !== id) })),
  setCurrentId: (currentId) => set({ currentId }),
  setMode: (mode) => set({ mode }),
  setMorphing: (morphing) => set({ morphing }),
  setLoaded: (loaded) => set({ loaded }),
  setMarkdownGetter: (markdownGetter) => set({ markdownGetter }),
}));
