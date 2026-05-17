# S1-PR10 Schema Realignment

你是负责实现 Glance 的编码 Agent。当前 PR 的目标是把数据库 schema 与 `Docs/glance_architecture.md` §3 对齐，把 `format` 从 `photos` 迁到 `photo_files`，并补齐 `library_id`、`missing_since`、复合索引。本阶段产品未发布，**允许直接重写 `001_initial_schema.sql`，不保留任何兼容性**。

## 依赖

本 PR 基于：

- `S1-PR2` 数据库基础
- `S1-PR5` 扫描编排（会一起改）
- `S1-PR8` IPC 接通后能验证流程
- 评审结论 `S1-REVIEW.md` B12、B13

开始前阅读：

- `Docs/glance_architecture.md` §3 数据库 Schema 全文
- `src-tauri/src/core/db/migrations/001_initial_schema.sql`
- `src-tauri/src/core/db/dao.rs`
- `src-tauri/src/core/scanner/orchestrator.rs`

## 任务

1. 重写 `001_initial_schema.sql`，使其与架构文档完全一致：
   - `photos`：移除 `format` 列。
   - `photo_files`：
     - 增加 `library_id INTEGER NOT NULL REFERENCES libraries(id)`
     - 增加 `format TEXT NOT NULL`
     - 增加 `missing_since INTEGER`（可空，转 missing 时写）
     - 把 `UNIQUE(path)` 改成 `UNIQUE(library_id, path)`
   - `thumbnails`、`scan_jobs`、`libraries` 与现状保持一致或补齐架构文档列出的字段。
   - 索引：
     - `idx_photos_timeline(library_id, taken_at DESC)`
     - `idx_photo_files_photo(photo_id)`
     - `idx_photo_files_path_prefix(path)`
     - `idx_photo_files_identity(library_id, content_hash, file_size)`
     - `idx_photo_files_status(library_id, status)`

2. 处理已有开发数据：
   - 不写"alter / migrate-data"分支。
   - 在本 PR 说明里告知开发者运行前清空 `%APPDATA%/Glance/index.sqlite*`（或加一个一次性 CLI 帮助命令也可，但不强制）。

3. 更新 Rust 类型：
   - `dao::Photo`：移除 `format` 字段。
   - `dao::PhotoFile`：增加 `library_id`、`format`、`missing_since`。
   - 所有 `insert_*`、`get_*`、`find_*` 函数签名和 SQL 同步更新。
   - 状态变更函数：把 `update_photo_file_status(id, "missing")` 改成同时写 `missing_since = now()`；从 missing 恢复到 available 时清空 `missing_since`。

4. 更新扫描编排：
   - `create_new_photo` 不再写 `photos.format`。
   - 每个 `photo_files` 写入时按文件扩展名（或 EXIF）决定 `format`：JPEG/PNG/HEIC/ARW/CR2/.../XMP。
   - 重定位、missing 标记、变更检测路径中所有用到 `photo_files` 的位置同步加 `library_id` 参数。
   - `find_photo_files_by_identity` 改为 `find_photo_files_by_identity(library_id, hash, size)`。

5. 更新前端契约：
   - `src/ipc/timeline.ts`：从 `PhotoDetail` 移除 `format`，把 `format` 加到 `PhotoFileInfo`。
   - 任何引用 `photo.format` 的视图改为从 display 文件的 `format` 读。

6. 测试：
   - DAO 单测全部对齐新签名。
   - 扫描测试：JPEG+ARW 配对后两个 `photo_files` 行 format 分别为 `jpeg` 和 `arw`。
   - 扫描测试：同一 hash 出现在两个不同 library 时不冲突（验证 `UNIQUE(library_id, path)` 行为，可建两个 library 走测试）。
   - 标记 missing 时 `missing_since` 写入；恢复 available 时清空。

## 不做

- 不实现多图库 UI（仍是 0.2）。
- 不引入数据迁移工具。
- 不修 EXIF 数值类型问题（PR11）。
- 不改缩略图流程（PR9）。

## 验收标准

- `001_initial_schema.sql` 与架构文档 §3 文本可逐行对应。
- 删除本地 DB 后重新 `pnpm tauri dev` 启动 → 扫描 → 时间线 → 详情，全链路无 SQL 错误。
- JPEG+ARW 配对在 DB 中可观测：1 行 `photos`、2+ 行 `photo_files`，format 分别记录。
- 标记为 missing 的 photo_files `missing_since` 不为空。
- `cargo test` 通过。

## 建议验证

```bash
# 清空开发数据库
rm "$APPDATA/Glance/index.sqlite"*  # Windows PowerShell 类似命令

cd src-tauri && cargo test
pnpm tauri dev
```

用 `sqlite3 index.sqlite '.schema photo_files'` 检查表结构。
