/** 相对时间与到期日的显示逻辑 */

function parseLocal(iso: string): Date | null {
  // 「2026-11-02T14:20:00」不带时区，按本地时间解析
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? null : d;
}

function startOfDay(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate());
}

/** 今天 / 昨天 / 本周 / 更早 —— 列表分组用 */
export function groupOf(iso: string): "今天" | "昨天" | "本周" | "更早" {
  const d = parseLocal(iso);
  if (!d) return "更早";
  const now = startOfDay(new Date());
  const that = startOfDay(d);
  const days = Math.round((now.getTime() - that.getTime()) / 86400000);
  if (days <= 0) return "今天";
  if (days === 1) return "昨天";
  if (days < 7) return "本周";
  return "更早";
}

export function groupOrder(group: string): number {
  return ["置顶", "今天", "昨天", "本周", "更早"].indexOf(group);
}

/** 列表卡片上的时间戳 */
export function formatStamp(iso: string): string {
  const d = parseLocal(iso);
  if (!d) return "";
  const g = groupOf(iso);
  if (g === "今天") {
    return d.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", hour12: false });
  }
  if (g === "昨天") return "昨天";
  if (g === "本周") {
    return ["周日", "周一", "周二", "周三", "周四", "周五", "周六"][d.getDay()];
  }
  return `${d.getMonth() + 1}月${d.getDate()}日`;
}

export type DueTone = "overdue" | "today" | "tomorrow" | "later" | "done";

export interface DueInfo {
  tone: DueTone;
  label: string;
}

/** 到期日的中文描述与色调 */
export function dueInfo(due: string | null, done: boolean): DueInfo | null {
  if (!due) return null;
  if (done) return { tone: "done", label: "已完成" };
  const d = new Date(`${due}T00:00:00`);
  if (Number.isNaN(d.getTime())) return null;
  const days = Math.round((startOfDay(d).getTime() - startOfDay(new Date()).getTime()) / 86400000);
  if (days < 0) {
    const n = Math.abs(days);
    return { tone: "overdue", label: n === 1 ? "逾期 1 天" : `逾期 ${n} 天` };
  }
  if (days === 0) return { tone: "today", label: "今天到期" };
  if (days === 1) return { tone: "tomorrow", label: "明天到期" };
  return { tone: "later", label: due.slice(5).replace("-", "/") };
}

/** 待办徽标计数：到期或逾期、且未完成 */
export function pendingTodoCount(notes: { due: string | null; done: boolean }[]): number {
  return notes.filter((n) => {
    if (!n.due || n.done) return false;
    const info = dueInfo(n.due, false);
    return info?.tone === "overdue" || info?.tone === "today";
  }).length;
}
