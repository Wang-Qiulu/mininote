/**
 * 聚焦测试：TipTap 的 Markdown 序列化对普通字符的转义是否会逐次累积。
 *
 * 背景：用户文件里写的是 `snake_case_name`，编辑器存盘时若转义成
 * `snake\_case\_name`，再读回来再存盘是否会变成 `snake\\\_case\\\_name`？
 * 若会累积，则每保存一次文件就多一层反斜杠 —— 对「文件即真相源」是致命的。
 */
import { MarkdownManager } from "@tiptap/markdown";
import StarterKit from "@tiptap/starter-kit";
import TaskList from "@tiptap/extension-task-list";
import TaskItem from "@tiptap/extension-task-item";

const manager = new MarkdownManager({
  extensions: [StarterKit, TaskList, TaskItem.configure({ nested: true })],
});

const round = (s) => manager.serialize(manager.parse(s));

const INPUTS = [
  ["下划线标识符", "变量名 snake_case_name"],
  ["已转义的下划线", "变量名 snake\\_case\\_name"],
  ["方括号下标", "数组 arr[0] 与 arr[1]"],
  ["已转义的方括号", "数组 arr\\[0\\]"],
  ["波浪号路径", "路径 ~/Documents/备忘录"],
  ["已转义的波浪号", "路径 \\~/Documents/备忘录"],
  ["星号强调", "文字 *强调* 结束"],
  ["已转义的星号", "文字 \\*强调\\* 结束"],
];

console.log("输入".padEnd(18), "| 一次 → 二次 → 三次");
console.log("-".repeat(78));

let compounding = 0;
for (const [name, input] of INPUTS) {
  const r1 = round(input);
  const r2 = round(r1);
  const r3 = round(r2);
  const grows = r2 !== r1 || r3 !== r2;
  if (grows) compounding++;
  console.log(
    `${name.padEnd(16)} | ${JSON.stringify(r1).padEnd(24)} → ${JSON.stringify(r2).padEnd(26)} → ${JSON.stringify(r3)}` +
      (grows ? "   ⚠️ 累积" : "   ✓ 稳定"),
  );
}

console.log("-".repeat(78));
console.log(compounding ? `⚠️ ${compounding}/${INPUTS.length} 项存在累积` : "✅ 全部稳定，无累积");
