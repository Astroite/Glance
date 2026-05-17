# S1-PR5 Scan Orchestration And Relocation

你是负责实现 Glance 的编码 Agent。当前 PR 的目标是把数据库、扫描、身份、配对和 metadata 串成可用的图库扫描流程，并实现缺失文件和手动重定位命令。

## 依赖

本 PR 基于：

- `S1-PR1` 工程骨架
- `S1-PR2` 数据库基础
- `S1-PR3` 扫描、身份、配对
- `S1-PR4` metadata 提取

开始前阅读：

- `Docs/glance_architecture.md` 的 Tauri Command 接口、缺失与重定位机制、首次扫描、增量扫描。
- `Docs/glance_design_document.md` 的 NAS / 缺失文件处理原则。

## 任务

1. 实现图库命令：
   - `library.list()`
   - `library.add(path)`
   - `library.scan(id)`

2. 实现扫描作业：
   - 创建 `scan_jobs`。
   - 记录 `started_at`、`finished_at`、`status`。
   - 扫描期间写入或更新 `photos` / `photo_files`。
   - RAW+JPEG 组合写成一条 `photos` 和多条 `photo_files`。
   - XMP sidecar 写为 `photo_files.role='sidecar'`，并把 Rating / Label 写入 `photos`。

3. 实现变更检测：
   - 同一路径 `mtime` 和 size 未变：更新 `last_seen_at`，保持 `available`。
   - 同一路径 `mtime` 或 size 变化：重新计算 hash。
   - 重新计算后 `hash + size` 未变：更新 mtime。
   - 重新计算后 `hash + size` 已变：旧 `photo_files` 标记为 `missing`，新文件作为新实例入库。

4. 实现缺失状态：
   - 扫描结束后，`last_seen_at < scan.started_at` 的 `photo_files` 统一标记为 `missing`。
   - 不自动判断移动、重复、删除还是离线。
   - 不删除索引。
   - 不删除缩略图缓存。

5. 实现手动重定位：
   - `library.relocate_folder(id, oldPrefix, newPrefix)`
   - `photo.relocate_file(photoFileId, newPath)`
   - 重定位必须重新计算目标路径 `hash + size` 并与原文件身份比对。
   - 身份匹配后才自动恢复 `available`。
   - 身份不匹配时返回明确错误或需要用户确认的状态，不静默替换。

6. 实现扫描事件：
   - `scan:progress`
   - `scan:done`
   - `scan:done` 至少包含 `added`、`updated`、`missing`。

7. 添加测试：
   - 首次扫描写入逻辑照片和文件实例。
   - RAW+JPEG+XMP 写入一张照片。
   - 同路径未变不重复入库。
   - 同路径内容变化标记旧文件 missing。
   - 扫描未见文件标记 missing。
   - 手动重定位身份匹配恢复 available。
   - 手动重定位身份不匹配不会静默覆盖。

## 不做

- 不生成缩略图。
- 不实现前端时间线。
- 不实现筛选 UI。
- 不实现自动移动检测。
- 不实现批量缺失文件助手。
- 不删除任何用户原始文件。

## 验收标准

- 可以通过 Tauri command 添加图库并触发扫描。
- 数据库中正确表达 `photos` 与 `photo_files`。
- 缺失状态符合设计：统一标记 missing，等待用户手动重定位。
- 扫描不会因单个文件 metadata 失败而整体崩溃。
- 测试覆盖关键扫描和重定位行为。

## 建议验证

运行：

```bash
cd src-tauri && cargo test
```
