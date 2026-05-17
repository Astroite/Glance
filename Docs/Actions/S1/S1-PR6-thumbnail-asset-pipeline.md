# S1-PR6 Thumbnail And Asset Pipeline

你是负责实现 Glance 的编码 Agent。当前 PR 的目标是实现 240 / 480 / 1080 三档缩略图生成、缩略图元数据记录、Tauri asset 协议访问，以及缺失文件时的占位图行为。

## 依赖

本 PR 基于：

- `S1-PR1` 工程骨架
- `S1-PR2` 数据库基础
- `S1-PR5` 扫描流程和 `photos.display_file_id`

开始前阅读：

- `Docs/glance_design_document.md` 的缩略图缓存、Orientation、ICC、HEIC、RAW 预览说明。
- `Docs/glance_architecture.md` 的缩略图 schema、本地存储布局、asset 协议、任务队列。

## 任务

1. 实现缩略图路径规则：
   - AppData 下 `thumbs/{tier}/{hash[0:2]}/{hash}.webp`。
   - `hash` 使用 `thumbnails.source_hash`。
   - 不在数据库中保存绝对缩略图路径。

2. 实现三档生成：
   - 240px 短边。
   - 480px 短边。
   - 1080px 短边。
   - MVP 首次索引时为每张照片生成全部三档。

3. 实现图像处理：
   - Orientation 在生成时烘焙。
   - 非 sRGB 图像转换到 sRGB；如果依赖库暂时无法完整支持，需要用清晰 TODO 和测试边界标出。
   - 输出 WebP。
   - HEIC 不依赖 Windows 系统 HEVC 扩展。
   - RAW-only 文件优先使用内嵌 JPEG 作为预览来源，不做 demosaic。

4. 实现 asset 协议：
   - 前端访问 `asset://thumb/{photoId}/{tier}`。
   - 后端根据 `photoId` 查 `thumbnails.source_hash` 并返回对应 WebP。
   - 缩略图不存在且原文件可用：排高优先级补齐任务或同步生成一个可用结果。
   - 缩略图不存在且原文件 missing：返回占位图。
   - 不使用 base64。
   - 不启动 localhost HTTP server。

5. 实现缩略图 metadata：
   - 生成成功后写入 `thumbnails`。
   - 记录 `source_file_id`、`source_hash`、实际 width / height、`generated_at`。

6. 添加测试：
   - 缩略图路径推导。
   - 三档尺寸输出。
   - Orientation 烘焙行为可以用小样本或抽象测试覆盖。
   - missing 文件返回占位图。
   - 缩略图 metadata 写入。

## 不做

- 不实现完整 RAW 渲染。
- 不实现 AVIF。
- 不实现缩略图缓存清理 UI。
- 不实现前端时间线布局。
- 不删除孤立缓存；GC 可以留到后续 PR。

## 验收标准

- 每张已扫描照片可以生成三档 WebP 缩略图。
- `asset://thumb/{photoId}/{tier}` 可以返回图像数据。
- 缺失文件且无缓存时返回占位图。
- 不引入 HTTP server 或 base64 图片传输。
- 测试覆盖路径、尺寸、metadata 和占位图行为。

## 建议验证

运行：

```bash
cd src-tauri && cargo test
```

如有可用样例图片，也应人工检查生成缩略图方向和色彩没有明显异常。
