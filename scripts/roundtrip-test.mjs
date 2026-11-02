/**
 * Markdown 往返幂等性实测。
 *
 * 判据：parse → serialize → parse → serialize，
 * 第二次 serialize 的结果必须与第一次**逐字节相同**。
 * 若不等，说明每保存一次文件就会被改写一层（对"文件即真相源"是致命的）。
 */
import { MarkdownManager } from "@tiptap/markdown";
import StarterKit from "@tiptap/starter-kit";
import TaskList from "@tiptap/extension-task-list";
import TaskItem from "@tiptap/extension-task-item";

const manager = new MarkdownManager({
  extensions: [StarterKit, TaskList, TaskItem.configure({ nested: true })],
});

const CASES = [
  ["纯段落", "这是一段普通文字。"],
  ["粗体斜体", "**粗体** 与 *斜体* 与 ~~删除线~~"],
  ["下划线字符", "变量名 snake_case_name 和 __init__ 方法"],
  ["行内代码", "调用 `parse_note()` 方法"],
  ["标题", "# 一级标题\n\n## 二级标题\n\n正文"],
  ["任务清单", "- [ ] 未完成的事\n- [x] 已完成的事"],
  ["无序列表", "- 第一项\n- 第二项"],
  ["有序列表", "1. 第一步\n2. 第二步"],
  ["引用", "> 这是一段引用"],
  ["代码块", "```rust\nfn main() {}\n```"],
  ["链接", "看这个 [文档](https://example.com) 就行"],
  ["含方括号", "数组下标 arr[0] 和 arr[1]"],
  ["含波浪号", "路径 ~/Documents/备忘录 和版本 ~1.0"],
  ["混合", "# 标题\n\n一段 **粗体** 文字\n\n- [ ] 任务\n- 普通项\n\n结尾。"],
];

let failed = 0;

for (const [name, input] of CASES) {
  const md1 = manager.serialize(manager.parse(input));
  const md2 = manager.serialize(manager.parse(md1));
  const stable = md1 === md2;
  const lostTask =
    (input.includes("[ ]") || input.includes("[x]")) &&
    !(md2.includes("[ ]") || md2.includes("[x]"));

  const flag = stable && !lostTask ? "✓" : "✗";
  if (!stable || lostTask) failed++;

  console.log(`\n${flag} ${name}`);
  if (!stable || lostTask) {
    console.log(`  输入: ${JSON.stringify(input)}`);
    console.log(`  一次: ${JSON.stringify(md1)}`);
    console.log(`  二次: ${JSON.stringify(md2)}`);
    if (lostTask) console.log("  ⚠️  任务复选框丢失");
    if (!stable) console.log("  ⚠️  往返不幂等：每次保存都会改写文件");
  } else {
    console.log(`  ${JSON.stringify(md1)}`);
  }
}

console.log(
  `\n${"=".repeat(50)}\n${CASES.length - failed}/${CASES.length} 通过` +
    (failed ? `，${failed} 项失败` : "，全部往返幂等 ✅"),
);
process.exit(failed ? 1 : 0);
