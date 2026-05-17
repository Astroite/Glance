# S1-PR3 Scanner, Identity, And Pairing

你是负责实现 Glance 的编码 Agent。当前 PR 的目标是实现文件发现、物理文件身份计算、媒体类型识别，以及 RAW+JPEG/XMP 的 MVP 配对规则。

## 依赖

本 PR 基于：

- `S1-PR1` 工程骨架
- `S1-PR2` 数据库基础

开始前阅读：

- `Docs/glance_design_document.md` 的文件身份策略和 RAW 预览规则。
- `Docs/glance_architecture.md` 的 `scanner`、`identity`、`photo_files`、RAW+JPEG 配对说明。

## 任务

1. 实现媒体文件发现：
   - 遍历用户选择的图库目录。
   - 识别 MVP 支持的图片和 sidecar 文件。
   - 至少覆盖：JPEG/JPG、PNG、HEIC、常见 RAW 扩展、XMP。
   - 输出结构化的扫描候选项，不直接写复杂业务逻辑。

2. 实现物理文件身份：
   - 使用 `xxh3(head 64KB + tail 64KB + file_size)` 计算 `content_hash`。
   - 同时记录 `file_size` 和 `mtime`。
   - 明确：`content_hash + file_size` 是身份，`mtime` 只用于变更检测。
   - 大文件只读取头尾片段，不做全文件 hash。

3. 实现媒体类型和角色判断：
   - JPEG / PNG / HEIC 可作为 `display` 文件。
   - RAW 文件作为 `raw`。
   - XMP 文件作为 `sidecar`。
   - 重复文件角色由后续扫描编排根据已有记录决定，本 PR 只保留类型能力。

4. 实现 RAW+JPEG/XMP 配对规则：
   - 同目录、同文件名 stem 的 RAW + JPEG 组合归为同一逻辑照片候选。
   - JPEG 优先作为展示文件。
   - 同 stem 的 XMP sidecar 关联到同一逻辑照片候选。
   - RAW-only 文件作为单独照片候选，后续可使用内嵌 JPEG 预览。

5. 添加测试：
   - 使用临时目录构造文件树。
   - 验证大小写扩展名。
   - 验证 RAW+JPEG+XMP 同 stem 配对。
   - 验证同目录不同 stem 不配对。
   - 验证不同目录同 stem 不配对。
   - 验证 `mtime` 变化不会改变身份结果，除非重新计算后 hash/size 变化。

## 不做

- 不读取 EXIF / XMP 内容。
- 不写入完整扫描结果到数据库。
- 不生成缩略图。
- 不实现 Tauri command。
- 不实现 UI。
- 不自动判断文件移动、删除、重复或离线。

## 验收标准

- `scanner` 可以输出稳定、结构化的候选文件和照片组合。
- `identity` 可以快速计算 `content_hash + file_size`。
- RAW+JPEG 组合在核心逻辑中表现为一个逻辑照片候选。
- 测试覆盖配对和身份计算边界。

## 建议验证

运行：

```bash
cd src-tauri && cargo test
```
