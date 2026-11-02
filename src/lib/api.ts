import { invoke } from "@tauri-apps/api/core";

/** 与 Rust 侧 `NoteMeta` 对应（serde camelCase） */
export interface NoteMeta {
  id: string;
  title: string;
  created: string;
  updated: string;
  pinned: boolean;
  /** YYYY-MM-DD */
  due: string | null;
  done: boolean;
  body: string;
  preview: string;
  todoTotal: number;
  todoDone: number;
}

/** 元数据局部更新；`due` 传空字符串表示清空 */
export interface NotePatch {
  id: string;
  title?: string;
  pinned?: boolean;
  due?: string;
  done?: boolean;
}

export const api = {
  listNotes: () => invoke<NoteMeta[]>("list_notes"),
  readNote: (id: string) => invoke<NoteMeta | null>("read_note", { id }),
  createNote: (title: string, body: string) => invoke<NoteMeta>("create_note", { title, body }),
  saveNote: (id: string, title: string, body: string) =>
    invoke<NoteMeta>("save_note", { id, title, body }),
  patchNote: (patch: NotePatch) => invoke<NoteMeta>("patch_note", { patch }),
  deleteNote: (id: string) => invoke<void>("delete_note", { id }),
  notesDir: () => invoke<string>("notes_dir"),
  revealNote: (id: string) => invoke<void>("reveal_note", { id }),
  openNotesDir: () =>invoke<void>("open_notes_dir"),
  setWindowMode: (mode: "mini" | "expanded") => invoke<void>("set_window_mode", { mode }),
  quitReady: () => invoke<void>("quit_ready"),
};
