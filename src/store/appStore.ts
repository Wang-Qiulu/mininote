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

  setNotes: (notes: NoteMeta[]) => void;
  upsertNote: (note: NoteMeta) => void;
  removeNote: (id: string) => void;
  setCurrentId: (id: string | null) => void;
  setMode: (mode: WindowMode) => void;
  setMorphing: (morphing: boolean) => void;
  setLoaded: (loaded: boolean) => void;
}

export const useAppStore = create<AppState>((set) => ({
  notes: [],
  currentId: null,
  mode: "mini",
  morphing: false,
  loaded: false,

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
}));
