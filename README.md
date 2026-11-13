# mininote

> 一块贴在桌面右上角的玻璃便签。双击 ⌘ 一按即现即写，写完就走。

![平台](https://img.shields.io/badge/平台-macOS%2013%2B-1b1a19) ![版本](https://img.shields.io/badge/版本-v0.4.0-d9662b) ![许可证](https://img.shields.io/badge/许可证-MIT-d9662b) ![状态](https://img.shields.io/badge/状态-早期%20版本-8a8a8a)

![小窗：360×300](docs/screenshot-mini.png)

![展开态：笔记列表 + 笔记](docs/screenshot-expanded.png)

---

## 特色

**轻到几乎不存在。** 菜单栏常驻，没有 Dock 图标，不占 ⌘Tab。小窗只有 360×300，不到 14 寸屏的八分之一，安静地蹲在屏幕右上角贴着菜单栏。

**一按即现即写。** 双击 `⌘` 唤起，**同一个动作开、关**。这是唯一的快捷键。新建笔记时光标直接落在标题上，写完标题按回车就进正文 —— 不用鼠标。

**写完就走。** 400ms 防抖自动保存，落盘是原子的（先写临时文件再改名），写到一半断电也不会留下半个文件。

**数据是自己的。** 每条笔记就是一个纯 Markdown 文件，带 YAML frontmatter，躺在 `~/Documents/备忘录/`。删掉 App 笔记还在；用别的编辑器改了文件，App 会自动感知。

**玻璃是系统给的。** macOS 的 Liquid Glass 原生材质，不是 CSS 画出来的假玻璃。

**安静。** 320ms 的窗口形变跟手不喧哗；跟随系统的亮/暗主题；尊重「减弱动画」「减弱透明度」的系统偏好。

---

## 已实现的功能

### 窗口与外壳

- 菜单栏托盘常驻（**无 Dock 图标**）；左键或右键点图标都能展开菜单，菜单里只有当前版本号和「退出」
- 主快捷键 **双击 `⌘`** —— 同一个动作开/关，也是项目唯一的快捷键。判定很严：两次完整的按下—抬起、间隔 300ms 内、中间不能夹任何其他按键，所以 `⌘C` 接 `⌘V` 不会误触
- 收起小窗后焦点自动回到唤起前正在用的那个应用，不会让屏幕失焦
- 双击 `⌘` 的事件监听依赖「辅助功能」权限
- 小窗 360×300，默认停在**屏幕右上角**（右侧留 10pt，正好在菜单栏下方）
- 一键展开成 720×300 的「笔记列表 + 笔记」双栏视图：宽度**向左生长**、右边缘锚定，笔记本身纹丝不动；收回时精确还原到原位
- 关掉窗口只是隐藏，进程、托盘图标和快捷键都还在
- **启动就把小窗显示出来**；应用已在运行时再双击一次 `.app`，窗口也会被叫出来
- **能浮在原生全屏之上**：别的 App 进了 macOS 全屏（独立 Space），双击 `⌘` 照样把小窗叫到它上面并直接落光标开写。只在唤起期间临时提升层级、收起后立刻还原，不常驻在每个桌面

### 编辑

- 所见即所得编辑器：粗体 / 斜体 / 删除线 / 行内代码 / 代码块 / 标题 1–3（三级字号逐级区分）/ 有序无序列表 / 任务清单 / 引用 / 分隔线 / 链接
- 选中文字时浮出格式工具条
- 顶栏一键复制当前笔记的 Markdown 原文：`#` `**` `- [ ]` 等符号原样保留，不用选中文字，也不会带出排版产生的多余空格；不含 frontmatter 和笔记标题
- 标题独立于正文（存在 frontmatter 里）；新建笔记后光标落在标题，回车进正文
- 任务清单的 `- [ ]` / `- [x]` 存成 Markdown 再读回来，一字不差

### 保存与数据

- 自动保存（400ms 防抖）；**没编辑过的文件绝不重写**
- 原子写：先写 `.tmp` 再 `rename` 覆盖
- 外部改动监听：用别的编辑器改了 `.md`，App 自动刷新
- 冲突保护：磁盘上的版本和载入时对不上就**拒绝覆盖**，顶部提示「文件已被外部修改」—— 绝不默默吃掉你打的字

### 界面

- 亮/暗主题跟随系统
- 尊重 `prefers-reduced-motion` 与 `prefers-reduced-transparency`（系统开了「减弱透明度」就降级成实色面板）
- 展开态侧栏列出全部笔记，当前笔记高亮

---

## 安装

### 下载现成的（推荐）

1. 打开 [Releases](https://github.com/Wang-Qiulu/mininote/releases) 页面，下载 `mininote-v0.1.0-macos.zip`（里面是「mininote.app」）
2. 解压，把 **mininote.app** 拖进「应用程序」文件夹
3. 第一次打开：右键点它 → 选「打开」→ 再点「打开」
   （没做开发者签名，Gatekeeper 会拦一下。命令行放行：`xattr -dr com.apple.quarantine "/Applications/mininote.app"`）
4. 启动后小窗会**直接出现在屏幕右上角**。之后它就常驻菜单栏了：

   ![菜单栏里的图标](docs/menubar-icon.png)

   这个图标就是入口 —— 左键或右键点它都能展开菜单，里面显示当前版本号和「退出」。小窗的显示／隐藏只用双击 `⌘`，不走菜单。

   **没有 Dock 图标是正常的**：这是菜单栏应用，不占 ⌘Tab、不抢任务栏，图标挤在电量、Wi‑Fi 那一堆状态图标里。
5. 随时**双击 `⌘`** 唤出／收起小窗（同一个动作开、关）。这是唯一的快捷键。
6. 双击 `⌘` 没反应：系统设置 → 隐私与安全性 → 辅助功能 → 加上「mininote」。
   双击修饰键的事件监听依赖这个权限；不给的话系统会**静默丢弃**事件，不报错也不弹窗。

**要求：macOS 13 或更高。** macOS 26 及以上会自动换上 Liquid Glass 材质，低版本也有优雅降级。

### 从源码构建

需要 macOS 13+、Node.js 20+、Rust 1.77+、Xcode Command Line Tools。

```bash
git clone https://github.com/Wang-Qiulu/mininote.git
cd mininote
npm install

npm run app          # 开发模式（Vite 热更新 + Rust，改代码立刻见效）
npm run app:build    # 打包出 .app
```

产物在 `src-tauri/target/release/bundle/macos/mininote.app`。

---

## 笔记存在哪

默认 `~/Documents/备忘录/`，每条笔记一个文件：

```markdown
---
title: 周会纪要
created: 2026-11-02T14:20:00
updated: 2026-11-03T15:05:00
---

讨论了下季度计划……

- [x] 拉通数据口径
- [ ] 出设计稿
```

文件是唯一真相源：删掉 App，数据还在；用别的编辑器改，App 会自动刷新，并且不会用旧内容覆盖你的新内容。

---

<!-- 下面开始是技术内容 -->

---

## 技术

### 技术栈

| 层 | 选型 | 为什么 |
| --- | --- | --- |
| 外壳 | Tauri 2.12（Rust） | 体积小（.app 只有 3.9 MiB）、能直接用系统的窗口材质和事件监听 |
| 前端 | React 19 + TypeScript 5.9 + Vite 8 | — |
| 编辑器 | TipTap 3.32 + `@tiptap/markdown` | 所见即所得，且能原样吐 Markdown |
| 状态 | zustand | 够用，体积小 |
| 文件监听 | notify 8 | — |

### 目录结构

```text
mininote/
├── src/                       前端
│   ├── App.tsx                外壳：模式切换、事件监听
│   ├── components/EditorPane.tsx  所见即所得编辑器、标题、自动保存
│   ├── store/appStore.ts      zustand
│   ├── lib/{api,time}.ts      IPC 封装、时间工具
│   └── styles/{tokens,global}.css  设计令牌与玻璃样式
├── src-tauri/src/
│   ├── notes.rs               存储层：frontmatter 解析、原子写、CRUD（9 个单测）
│   ├── window.rs              显隐、定位、形变动画
│   ├── lib.rs                 生命周期、命令、托盘接线
│   ├── tray.rs                菜单栏图标与菜单
│   └── watcher.rs             外部文件变更监听
├── docs/
│   ├── VERSIONS.md            版本计划与实现记录
│   └── *.png                  README 用的截图
├── scripts/
│   ├── gen-icons.mjs          图标生成（纯 Node，无依赖）
│   ├── roundtrip-test.mjs     Markdown 往返幂等测试（14 个用例）
│   ├── escape-accumulation-test.mjs  转义累积测试（8 个用例）
│   └── shot-window.py         只截本应用窗口的截图脚本
└── ...
```

### 几个关键实现

**玻璃交给系统。** Tauri 2.12 原生支持 macOS 26 的 `liquidGlassRegular`，CSS 只在系统材质之上叠加渐变描边、内高光和噪点。WKWebView 的 `backdrop-filter` **模糊不到桌面**（backdrop 只含自身绘制内容），所以不做那种假折射。

**动画在 Rust 侧驱动。** Tauri 的**同步命令跑在主线程上**，320ms 的动画会把窗口合成堵死，前端只会看到卡顿。所以 `set_window_mode` 立刻返回，动画派到独立线程，用真实时间算弹簧值、固定 60fps 步进。

**右边缘锚定。** `tao` 的 `setContentSize` 锚的是 NSWindow 的**左下角**，不是左上角 —— 直接用会让窗口飘。所以每帧同时下发尺寸和位置，收回时按展开前记录的锚点精确还原。

**存 Markdown 是有损的，只承诺一部分无损。** 粗体 / 斜体 / 删除线 / 行内代码 / 代码块 / 标题 / 列表 / 任务清单 / 引用 / 分隔线这一组往返无损；普通文本里的 `_` `[]` `~` 会在**第一次保存**时被转义（语义一致，且只变一次不再累积）。为此写了两个脚本实测：14 个往返用例 + 8 个转义累积用例，全部幂等。

**未编辑过的文件绝不重写。** 判断依据是**和磁盘上的内容比对**，不是时序敏感的抑制标志位 —— 载入时哪怕 `onUpdate` 延迟触发，也不可能把没改过的文件重写一遍。

**写盘带冲突检测。** 落盘时把载入时的正文作为「期望版本」一起传给后端；磁盘上的内容若已经和它不一致，后端直接拒绝写入，前端显示冲突提示。

**托盘应用的坑。** 打包后的 `.app` 里 Tauri 的 bundler 不会写 `LSUIElement`（[tauri#15005](https://github.com/tauri-apps/tauri/issues/15005)），托盘应用会重新冒出 Dock 图标 —— 所以自带 `Info.plist` 显式声明，和运行时的 `set_activation_policy(Accessory)` 互为保险。

**浮进全屏 Space。** 原生全屏会把 App 挪进独立 Space，默认 `collectionBehavior` 的窗口**根本不会被合成到那个 Space 上** —— 不是被压在下面。所以唤起时给 `NSWindow` 同时挂上 `windowLevel = NSStatusWindowLevel` 和 `CanJoinAllSpaces | FullScreenAuxiliary | Stationary | IgnoresCycle`，收起时摘掉；只调 `set_always_on_top(true)` 不管用，它只改 level。层级在某些全屏 App 上若还不够，可用环境变量 `MININOTE_FLOAT_LEVEL`（如 `101`）覆盖实测，不必重新编译。切换显隐也改成判断「是否处于主动唤起」而不是「窗口是否可见」：启动时窗口本就可见，若按可见来切，进全屏 Space 后第一次按键会走 hide 分支，得按两次才浮出来。聚焦则等窗口真正成为 key 之后才发出，避免「光标画出来了、按键却打进全屏页面」。

**双击 ⌘ 用事件监视器判定。** 修饰键的双击没法表达成「修饰键 + 主键」的组合，所以用 AppKit 的 `NSEvent` 事件监视器自己判定：global monitor 收别的 App 的事件，local monitor 收本应用的 —— 窗口唤起后本应用会成为前台，缺了 local 就再也收不回来。状态机要求两次完整的按下—抬起、间隔 300ms 内、中间不夹任何普通按键或额外修饰键，因此连续的 `⌘C`、`⌘V` 都不会误触发。两套监视器同样需要辅助功能权限。项目没有注册任何其他快捷键。

### 开发与测试

```bash
npm run build              # 类型检查 + 打包
npm run app                # 开发模式
npm run app:build          # 打包 .app

cd src-tauri && cargo test # Rust 单元测试（9 个）
npm run test:roundtrip     # Markdown 往返幂等
npm run test:escape        # 转义不累积
```

### 版本记录

版本计划、每次改动对应的版本与功能，统一记在 [docs/VERSIONS.md](docs/VERSIONS.md)。README 只描述当前已经实现的功能。

## 许可证

[MIT](LICENSE)
