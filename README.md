# 小记事本 · mininote

> 本地运行的 macOS 浮窗速记应用。菜单栏常驻，一按快捷键即现即写，内容以纯 Markdown 落盘。

![阶段](https://img.shields.io/badge/阶段-P0%20窗口外壳-d9662b) ![平台](https://img.shields.io/badge/平台-macOS%20arm64%2Fx86-1b1a19) ![许可证](https://img.shields.io/badge/许可证-MIT-1b1a19)

## 它是什么

一个**不到屏幕八分之一**的小浮窗，像一块贴在桌面的实体便签：

- **菜单栏常驻**，没有 Dock 图标，不占任务栏
- **全局快捷键** `⌘⇧Space` 唤出，光标直接落在正文里
- **自动保存**，写完就落盘，不用点保存
- **纯 Markdown 文件**存储，每条笔记一个 `.md`，可以用任何编辑器打开
- **原生 Liquid Glass**（macOS 26），玻璃由系统渲染，不是 CSS 画的

## 目录

```
备忘录/
├── src/                 前端（React 19 + TypeScript + Vite）
│   ├── styles/          设计令牌与全局样式
│   ├── lib/             与 Rust 的接口、时间与分组逻辑
│   ├── store/           zustand 状态
│   └── components/      界面组件
├── src-tauri/           Rust 后端
│   ├── src/notes.rs     存储层：frontmatter 解析、原子写、CRUD
│   ├── src/window.rs    窗口显隐与小窗⇄大窗形变
│   ├── src/tray.rs      菜单栏托盘
│   └── src/watcher.rs   外部文件变更监听
└── scripts/gen-icons.mjs  图标生成（纯 Node，无依赖）
```

## 开发

```bash
npm install
npm run app          # 开发模式（Vite + Tauri）
npm run app:build    # 打包出 .app
```

单独跑 Rust 侧测试：

```bash
cd src-tauri && cargo test
```

## 存储位置

默认 `~/Documents/备忘录/`，每条笔记一个文件：

```markdown
---
title: 周会纪要
created: 2026-11-02T14:20:00
updated: 2026-11-02T15:05:00
pinned: false
due: 2026-11-05
done: false
---

讨论了下季度路线图……

- [x] 拉通数据口径
- [ ] 出设计稿
```

文件是唯一真相源：删掉 App，数据还在；用别的编辑器改，App 会自动感知。

## 设计取舍

- **玻璃交给系统**：Tauri 2.12 原生支持 macOS 26 的 `liquidGlassRegular`，CSS 只叠加描边、噪点与着色。WKWebView 里的 `backdrop-filter` 模糊不到桌面，所以不做。
- **所见即所得存成 Markdown 是有损的**：只承诺粗体 / 斜体 / 删除线 / 行内代码 / 代码块 / 标题 / 列表 / 任务清单 / 引用 / 分隔线这一组往返无损。
- **待办的到期日是笔记级的**：per-item 到期时间需要往纯文本里塞旁注，V1 不做。
- **不做全文搜索与标签**：第一版靠置顶 + 时间线找笔记。

## 路线图

- [x] **P0** 脚手架：窗口外壳、托盘、形变动画
- [x] **P1** 存储层：frontmatter 解析、原子写、外部改动监听
- [x] **P1.5** 生命周期修复、玻璃质感重平衡
- [x] **P2** 所见即所得编辑器（TipTap 3）
- [ ] **P3** 大窗列表：置顶 + 时间分组 + 玻璃卡片 + 到期提醒
- [ ] **P4** 收尾：设置项、命令面板、无障碍与动效打磨

## 许可证

MIT
