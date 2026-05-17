# S1-PR2 Storage Schema

你是负责实现 Glance 的编码 Agent。当前 PR 的目标是建立本地数据目录、SQLite 连接、WAL 配置和 MVP schema 迁移。

## 依赖

本 PR 基于 `S1-PR1` 的工程骨架。

开始前阅读：

- `Docs/glance_architecture.md` 的数据库 schema、本地存储布局、缺失与重定位机制。
- `Docs/glance_design_document.md` 的 MVP 范围。

## 任务

1. 实现本地数据目录解析：
   - Windows 默认使用 `%APPDATA%/Glance/`。
   - 准备 `index.sqlite`、`thumbs/`、`logs/` 的路径工具。
   - 不要把用户照片目录写入或移动到 AppData。

2. 实现 SQLite 初始化：
   - 创建 SQLite 连接。
   - 启用 WAL 模式。
   - 准备 migration 机制。
   - 数据库访问代码放在 `src-tauri/src/core/db/`。

3. 实现 MVP schema：
   - `libraries`
   - `photos`
   - `photo_files`
   - `thumbnails`
   - `scan_jobs`
   - 必要索引

   必须遵守当前架构文档：
   - `photos` 是逻辑照片。
   - `photo_files` 保存物理文件实例。
   - `content_hash + file_size` 存在 `photo_files` 上，作为物理文件身份。
   - `mtime` 只用于变更检测。
   - `photo_files.status` 至少支持 `available` / `missing`。
   - `photos.display_file_id` 用于指向展示文件。

4. 建立基础 repository / DAO：
   - 插入和查询图库。
   - 插入和查询照片。
   - 插入和更新 `photo_files`。
   - 创建和更新 `scan_jobs`。
   - 插入和查询 `thumbnails` 元数据。

5. 添加测试：
   - 使用临时目录创建测试数据库。
   - 验证迁移可重复运行。
   - 验证 WAL 被启用。
   - 验证核心表和索引存在。
   - 验证 `photo_files` 可以表达 `display` / `raw` / `sidecar` / `duplicate` 和 `available` / `missing`。

## 不做

- 不实现目录扫描。
- 不实现 hash 计算。
- 不实现 EXIF / XMP 读取。
- 不实现缩略图生成。
- 不实现前端页面。
- 不实现自动移动检测。

## 验收标准

- 应用可以在本地数据目录初始化 SQLite 数据库。
- schema 与 `Docs/glance_architecture.md` 的当前设计一致。
- 测试覆盖 migration 和基础 CRUD。
- 不存在把 `mtime` 作为唯一身份字段的实现。

## 建议验证

运行：

```bash
cd src-tauri && cargo test
```
