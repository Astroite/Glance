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

| PR | 文件 | 目标 |
|---|---|---|
| S1-PR1 | `S1-PR1-project-scaffold.md` | 初始化 Tauri + React + Rust 工程骨架 |
| S1-PR2 | `S1-PR2-storage-schema.md` | 建立本地数据目录、SQLite 连接和迁移 schema |
| S1-PR3 | `S1-PR3-scanner-identity-pairing.md` | 实现文件发现、身份计算、RAW+JPEG/XMP 配对核心逻辑 |
| S1-PR4 | `S1-PR4-metadata-extraction.md` | 实现 EXIF 与 XMP Rating/Label 读取 |
| S1-PR5 | `S1-PR5-scan-orchestration-relocation.md` | 串联图库扫描、缺失状态和手动重定位命令 |
| S1-PR6 | `S1-PR6-thumbnail-asset-pipeline.md` | 实现三档缩略图生成和 asset 协议占位图 |
| S1-PR7 | `S1-PR7-timeline-ui.md` | 实现首版时间线 UI、瀑布流/网格切换和大图预览 |

## 通用交付要求

- PR 只做对应提示词里的事情，不顺手实现后续阶段功能。
- 保持只读图库原则，不修改、不移动、不删除用户原始照片文件。
- 新增行为要有聚焦测试；如果某些能力难以自动测试，需要在 PR 说明中写明人工验证方式。
- 修改设计决策前必须先更新文档并在 PR 说明中解释原因。
