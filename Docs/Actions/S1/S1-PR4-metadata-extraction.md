# S1-PR4 Metadata Extraction

你是负责实现 Glance 的编码 Agent。当前 PR 的目标是读取 MVP 需要的 EXIF 和 XMP metadata，并输出可写入 `photos` 的结构化数据。

## 依赖

本 PR 基于：

- `S1-PR1` 工程骨架
- `S1-PR2` 数据库基础
- `S1-PR3` 扫描候选和 RAW+JPEG/XMP 配对

开始前阅读：

- `Docs/glance_design_document.md` 的元数据读取、XMP sidecar、RAW 预览说明。
- `Docs/glance_architecture.md` 的 `exif` 模块职责和 schema。

## 任务

1. 实现 EXIF 读取：
   - 拍摄时间。
   - 相机品牌 / 型号。
   - 镜头。
   - 焦距。
   - 光圈。
   - 快门。
   - ISO。
   - 分辨率。
   - Orientation。
   - GPS。
   - 文件格式。

2. 实现拍摄时间回退：
   - 优先使用 EXIF DateTimeOriginal。
   - 缺失时回退到文件 `mtime`。
   - 输出 `taken_at_src = 'exif' | 'mtime'`。

3. 实现 XMP sidecar 读取：
   - MVP 必须读取 `Rating`，范围 0 到 5。
   - MVP 必须读取 `Label`。
   - 支持常见 XMP XML 命名空间写法。
   - 解析失败时不要让整次扫描崩溃，应返回可诊断错误或空 metadata。

4. 合并 RAW+JPEG+XMP metadata：
   - RAW+JPEG 组合中，优先用展示文件 JPEG 的尺寸和 Orientation 作为预览信息。
   - 如果 JPEG 缺少拍摄 metadata，可从 RAW 侧补齐。
   - XMP 的 Rating / Label 写入逻辑照片 metadata。

5. 添加测试：
   - XMP Rating / Label 解析。
   - XMP 缺字段时的行为。
   - EXIF 缺拍摄时间时回退 mtime。
   - metadata 合并规则。

## 不做

- 不解析关键词、层级标签、修图调整等复杂 XMP 字段。
- 不做完整 RAW 渲染。
- 不生成缩略图。
- 不实现筛选 UI。
- 不实现全文搜索。

## 验收标准

- `exif` 模块输出与 `photos` schema 对齐的结构化 metadata。
- XMP Rating / Label 确认进入 MVP。
- 解析失败不会中断整个扫描流程。
- 测试覆盖基础 EXIF / XMP 行为。

## 建议验证

运行：

```bash
cd src-tauri && cargo test
```
