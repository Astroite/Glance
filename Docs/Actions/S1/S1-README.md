# S1 Actions

S1 是 Glance 的第一个执行阶段，目标是把设计文档中的 MVP 决策落成可迭代的工程基础。本文档夹中的每个 `S1-PRx` 文件都是一份可以直接交给编码 Agent 的提示词，建议按编号顺序执行，每个文件对应一个独立 PR。

执行任何 S1 PR 前，Agent 必须先阅读：

- `Docs/glance_design_document.md`
- `Docs/glance_architecture.md`
- `CLAUDE.md`

S1 阶段必须遵守的核心决策：

- XMP sidecar 的 `Rating` / `Label` 进入 MVP。
- 同目录、同文件名 stem 的 RAW + JPEG 组合视为同一张照片，JPEG 作为展示文件和缩略图来源，RAW 作为关联原片。
- 物理文件身份为 `content_hash + file_size`；`mtime` 只用于变更检测，不参与身份判断。
- Glance 不自动判断文件是移动、重复、删除还是离线；路径不可达或扫描未见时统一标记为 `missing`，显示缓存预览并要求用户手动重定位。
- MVP 为每张照片生成 240 / 480 / 1080 三档缩略图；当缓存缺失且原文件不可用时返回占位图。
- 时间线浏览支持瀑布流和网格两种视图。

## PR 顺序

第一轮（PR1–PR7）已完成，但 S1-REVIEW.md 指出多项阻断性问题与实质 bug。第二轮（PR8–PR15）专门补救并把质量补齐到 MVP 发布门槛。

### 第一轮：基础模块

| PR | 文件 | 状态 | 目标 |
|---|---|---|---|
| S1-PR1 | `S1-PR1-project-scaffold.md` | Done | Tauri + React + Rust 骨架 |
| S1-PR2 | `S1-PR2-storage-schema.md` | Done with deviations | 本地数据目录、SQLite、迁移 |
| S1-PR3 | `S1-PR3-scanner-identity-pairing.md` | Done with bugs | 文件发现、身份、RAW+JPEG/XMP 配对 |
| S1-PR4 | `S1-PR4-metadata-extraction.md` | Done with bugs | EXIF + XMP Rating/Label |
| S1-PR5 | `S1-PR5-scan-orchestration-relocation.md` | Done with bugs | 扫描、缺失状态、手动重定位 |
| S1-PR6 | `S1-PR6-thumbnail-asset-pipeline.md` | Partially done | 三档缩略图 + asset 协议占位图 |
| S1-PR7 | `S1-PR7-timeline-ui.md` | Partially done | 时间线 UI + 瀑布流/网格 + 大图预览 |

### 第二轮：补救与质量

| PR | 文件 | 目标 |
|---|---|---|
| S1-PR8 | `S1-PR8-tauri-ipc-wiring.md` | 注册 Tauri 命令、AppState、asset:// 协议 |
| S1-PR9 | `S1-PR9-thumbnail-integration-and-quality.md` | 缩略图接入扫描、lossy WebP、fast_image_resize、ICC → sRGB |
| S1-PR10 | `S1-PR10-schema-realignment.md` | 把 schema 与架构文档对齐（format 迁到 photo_files 等） |
| S1-PR11 | `S1-PR11-exif-and-scanner-fixes.md` | 修 EXIF 类型匹配、scanner 配对、scan_job 状态、metadata 刷新 |
| S1-PR12 | `S1-PR12-scan-transaction-and-resume.md` | 批量事务 + cursor 续扫 + 取消 / 暂停 |
| S1-PR13 | `S1-PR13-heic-and-raw-decoding.md` | rawler 提 RAW 内嵌 JPEG；libheif-rs（feature gated）HEIC |
| S1-PR14 | `S1-PR14-timeline-ui-rework.md` | Virtuoso group API、接通 Lightbox、消除每张独立 IPC |
| S1-PR15 | `S1-PR15-task-queue.md` | 任务队列 + IO/CPU worker 池 + 优先级 |

第二轮强制要求：每个 PR 都必须能"端到端人工验证"，不能只交单元测试。详见 `S1-REVIEW.md` §9 给下一个 Agent 的提醒。

## 通用交付要求

- PR 只做对应提示词里的事情，不顺手实现后续阶段功能。
- 保持只读图库原则，不修改、不移动、不删除用户原始照片文件。
- 新增行为要有聚焦测试；如果某些能力难以自动测试，需要在 PR 说明中写明人工验证方式。
- 修改设计决策前必须先更新文档并在 PR 说明中解释原因。
- 测试通过 ≠ 功能可用。每个 PR 都要给出 `pnpm tauri dev` 路径下的最小端到端验证步骤。
