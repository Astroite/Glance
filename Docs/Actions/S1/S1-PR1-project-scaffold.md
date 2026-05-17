# S1-PR1 Project Scaffold

你是负责实现 Glance 的编码 Agent。当前 PR 的目标是初始化工程骨架，不实现业务功能。

## 必读上下文

开始前阅读：

- `Docs/glance_design_document.md`
- `Docs/glance_architecture.md`
- `CLAUDE.md`

特别注意：Glance 是 Windows 桌面应用，技术栈为 Tauri + Rust + React + TypeScript + Vite。当前仓库还没有源码，本 PR 只负责建立可运行、可测试、可继续扩展的基础结构。

## 任务

1. 初始化前端工程：
   - 使用 React + TypeScript + Vite。
   - 建立 `src/` 目录结构：`views/`、`components/`、`ipc/`、`state/`、`styles/`。
   - 准备基础应用入口和空白主界面。
   - 建立基础样式文件，避免引入复杂 UI 框架。

2. 初始化 Tauri / Rust 工程：
   - 建立 `src-tauri/`。
   - 建立 `src-tauri/src/main.rs`、`commands/`、`core/`、`error.rs`。
   - 在 `core/` 下建立设计文档中的模块目录：`scanner/`、`identity/`、`exif/`、`thumbnail/`、`raw/`、`db/`、`tasks/`。
   - 只放模块骨架、基础类型和最小可编译代码。

3. 建立开发命令：
   - 优先使用 `pnpm`；如果当前环境不可用，可以使用仓库已有约定或最小可行替代，并在 PR 说明中写清楚。
   - 在 `package.json` 中提供开发、构建、前端检查命令。
   - 确保 Rust 侧可以运行 `cargo test`。

4. 更新文档：
   - 更新 `README.md`，加入最小开发启动说明。
   - 如实际命令与 `CLAUDE.md` 中预期不同，同步更新 `CLAUDE.md`。

## 不做

- 不实现图库扫描。
- 不实现 SQLite schema。
- 不实现缩略图生成。
- 不实现 EXIF / XMP 读取。
- 不实现时间线 UI。
- 不添加云服务、HTTP server、后台常驻服务。

## 验收标准

- 仓库包含设计文档中约定的前端和 `src-tauri` 基础目录。
- 前端入口可以构建。
- Rust 工程可以编译并运行空测试。
- Tauri 开发命令能够启动到一个最小窗口或最小 WebView 页面。
- README 中有清晰的本地开发命令。

## 建议验证

运行：

```bash
pnpm install
pnpm build
cd src-tauri && cargo test
```

如果命令因本机依赖未安装而失败，需要在 PR 说明中写明失败原因和下一步处理方式。
