# S1-PR8 Tauri IPC Wiring And Asset Protocol

你是负责实现 Glance 的编码 Agent。当前 PR 的目标是把后端模块接到 Tauri command 层、注册 `asset://thumb` 自定义协议、托管全局 DB 连接，让前端首次能够通过 IPC 调到 Rust 核心。本 PR 是 S1 阶段最高优先级的修复，不完成则后续任何 PR 都无法人工验证。

## 依赖

本 PR 基于：

- `S1-PR1`–`S1-PR7` 所有模块
- 评审结论 `S1-REVIEW.md` §3.1、§3.2

开始前阅读：

- `Docs/glance_architecture.md` §4 Tauri Command 接口、§7 本地存储布局
- `Docs/Actions/S1/S1-REVIEW.md` §3 阻断性问题
- `src-tauri/src/lib.rs`、`src-tauri/src/commands/mod.rs`
- `src/ipc/*.ts` 前端调用契约

## 任务

1. 引入应用状态：
   - 定义 `AppState { db: Arc<Mutex<rusqlite::Connection>> }`。
   - 在 `tauri::Builder::setup` 里：
     - 调用 `core::db::init_app_data_dir()`
     - 创建 DB connection（WAL + foreign_keys）
     - 运行 migrations
     - 通过 `app.manage(AppState { .. })` 注入
   - 命令函数通过 `tauri::State<'_, AppState>` 拿到 connection。

2. 注册以下 command（保持与前端 `src/ipc/*.ts` 签名一致，必要时同时改前端）：
   - `library_list` → `Vec<Library>`
   - `library_add(path: String)` → `Library`
   - `library_scan(id: i64)` → `ScanJob`
   - `library_relocate_folder(library_id: i64, old_prefix: String, new_prefix: String)` → `i64`
   - `timeline_query(library_id: i64, cursor: Option<String>, limit: Option<i64>)` → `TimelinePage`
   - `photo_detail(id: i64)` → `PhotoDetail`
   - `photo_relocate_file(photo_file_id: i64, new_path: String)` → `()`
   - `thumbnail_url(photo_id: i64, tier: i64)` → `String`（返回 `asset://thumb/{photo_id}/{tier}`）
   - 在 `tauri::generate_handler![]` 中声明全部。

3. 实现 `timeline_query`：
   - 查询 `photos WHERE library_id=? AND (cursor 解析后 taken_at < ?) ORDER BY taken_at DESC LIMIT N`。
   - 默认 `limit=200`。
   - 游标格式建议 `{taken_at_unix}:{photo_id}`，避免 taken_at 重复导致跳页。
   - 返回 `PhotoSummary` 至少包含 `id, taken_at, content_hash, orientation, width, height, is_missing, thumbnail_url`。
   - `is_missing` 由"所有 `display` 角色 `photo_files.status='missing'`"推导。
   - `thumbnail_url` 字段已包含 `asset://thumb/{id}/240` 字符串；前端不再单独调用 `thumbnail_url` 命令。
   - 旧的 `thumbnail_url` command 保留作为兜底获取其他 tier（如 1080）的入口。

4. 实现 `photo_detail`：
   - 查 `photos` + 关联 `photo_files`，返回 `PhotoDetail`（含 `files: Vec<PhotoFileInfo>`，每条带 path/role/status）。

5. 注册 `asset://` 自定义协议：
   - 在 `Builder` 上调用 `register_uri_scheme_protocol("asset", handler)`。
   - Handler 解析 URL 为 `thumb/{photoId}/{tier}`：
     - 查 `thumbnails` 拿到 `source_hash`
     - 推导磁盘路径 `thumbs/{tier}/{hash[0:2]}/{hash}.webp`
     - 命中：返回 bytes + `Content-Type: image/webp`
     - 未命中且原文件可用：先返回占位图，并在后台排队补齐（本 PR 可以做同步生成兜底）
     - 未命中且原文件 missing：返回 `thumbnail::generate_placeholder(tier)`
   - 不启动 localhost HTTP server，不使用 base64。

6. 错误处理：
   - 命令统一返回 `Result<T, String>`，复用 `crate::error::Error` 转字符串。
   - 文件路径不存在 / DB 错误 / identity 不匹配 等都用清晰错误消息。
   - 任何命令不能 panic。

7. 同步前端契约：
   - `src/ipc/timeline.ts` 的 `PhotoSummary` 增加 `thumbnail_url: string` 字段。
   - 不再从 `ThumbnailItem` 调 `getThumbnailUrl`；直接读 `photo.thumbnail_url`。
   - 若有签名调整，同时更新 `src/state/*.ts`。

8. 添加测试：
   - 单元：每个命令的 happy path 用 `mock` Tauri State 或直接抽出"业务函数"做单测。
   - 集成：写一个 Rust 端的集成测试，模拟"create lib → scan → query timeline"，验证返回 `PhotoSummary` 字段齐全（即便此时缩略图未必存在，至少 URL 字符串可拿到）。

## 不做

- 不实现任务队列（PR15）。
- 不修缩略图编码 / 性能问题（PR9）。
- 不动 schema（PR10）。
- 不实现前端时间线重写（PR14）。
- 不实现 HEIC / RAW 解码（PR13）。
- 不实现 EXIF 类型修正（PR11）；本 PR 接受已知 metadata 字段会有 None。

## 验收标准

- `pnpm tauri dev` 启动后：
  1. 在 LibrarySetup 输入一个含 JPEG 的本地目录，点击 Add，目录被加入图库列表。
  2. 点击 Scan，扫描结束后 `scan:done` 事件触发（或同步返回）。
  3. 切回 Timeline 视图，能拿到至少一行 `PhotoSummary`（即便此时 thumbnail 文件还没生成，URL 字段已包含 asset 协议路径）。
  4. `asset://thumb/{id}/240` 命中时返回 WebP，未命中时返回占位图，不报 404。
- `cargo test` 全部通过，新增的集成测试覆盖 IPC 流程。
- 前端 `src/ipc/*.ts` 调用全部能命中实际 command，浏览器 console 无 "command not found" 报错。

## 建议验证

```bash
cd src-tauri && cargo test
pnpm build
pnpm tauri dev    # 人工验证：加目录 → 扫描 → 时间线查询不报错
```

人工验证时打开 webview devtools，确认 IPC 调用全部返回成功，无 "command not found"。
