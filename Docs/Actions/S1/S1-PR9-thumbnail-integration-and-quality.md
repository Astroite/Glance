# S1-PR9 Thumbnail Integration And Quality Fixes

你是负责实现 Glance 的编码 Agent。当前 PR 的目标是把缩略图生成接入扫描流程、修复编码质量与性能问题，并完成 ICC → sRGB 色彩转换。完成本 PR 后，扫描结束应能在 `thumbs/` 目录下看到真实 WebP 文件，前端时间线显示真实缩略图。

## 依赖

本 PR 基于：

- `S1-PR1`–`S1-PR7` 所有模块
- `S1-PR8` Tauri IPC + asset 协议
- 评审结论 `S1-REVIEW.md` §3.3、B2、B3、B5

开始前阅读：

- `Docs/glance_architecture.md` §3.2 缩略图路径决策、§7 本地存储布局
- `Docs/glance_design_document.md` 5.3 缩略图缓存（含 sRGB / Orientation / 多档）
- `src-tauri/src/core/thumbnail/mod.rs`
- `src-tauri/src/core/scanner/orchestrator.rs`

## 任务

1. 接入扫描流程：
   - 在 `create_new_photo` 创建完 `photo_files` 之后、`update_existing_photo` 检测到 display 文件 mtime 变化时，触发缩略图生成。
   - 仅对 `role='display'` 的物理文件生成。
   - 同步生成（MVP），任务队列化推到 PR15。
   - 生成成功后写入 `thumbnails` 表：`(photo_id, tier, source_file_id, source_hash, width, height, generated_at)`。

2. 切换 WebP 编码到 lossy：
   - 默认 quality = 85（可在常量中暴露）。
   - `image` crate 的 WebPEncoder 仅 lossless；换用 `webp` crate 或直接调 `image-webp` 的 lossy API。
   - 240 档单张缩略图目标 < 30KB，1080 档目标 < 250KB（仅作参考目标，不用做硬性测试）。

3. 切换 resize 实现到 `fast_image_resize`：
   - 不再用 `image::imageops::resize`。
   - 使用 Lanczos3 滤波器。
   - 用统一的 `SrcImage` / `DstImage` 抽象包装，便于后续接 HEIC / RAW 解码源。

4. 实现 ICC → sRGB 转换：
   - 读源 JPEG / PNG 的嵌入 ICC profile。
   - 用 `qcms` 创建 transform：源 profile → sRGB。
   - 如果源没有 ICC profile，假定 sRGB，不做转换。
   - 不在 WebP 中嵌入 profile（lossy WebP 嵌 profile 兼容性差），生产已经是 sRGB 数据。
   - 转换失败时 fallback 到原始数据 + 日志告警，不阻断扫描。

5. Orientation 烘焙保持现有逻辑，但要确认在 `fast_image_resize` 链路中执行顺序：解码 → ICC 转 sRGB → orientation 应用 → resize → encode。

6. 调整 `generate_placeholder`：
   - 输出 4:3 比例，避免与真实缩略图比例差异过大造成布局抖动。
   - 240 / 480 / 1080 各自尺寸（如 240×180）。

7. 缩略图路径与文件结构遵守现有 `thumbnail_path()` 规则不变。

8. 测试：
   - 真实小样本 JPEG（带 Adobe RGB profile，可以仓库里放一张 100×100 像素的测试图）生成缩略图后，目视验证不发灰（可以保留为人工验证，但加 sample 文件路径到测试里）。
   - 生成 240 / 480 / 1080 文件存在且大小符合预期数量级。
   - `thumbnails` 表写入正确。
   - 重复扫描不会重复生成（已存在的 thumbnail 不覆盖）。
   - Orientation = 6 的图片生成后短边维度对得上（测量 dimensions）。

## 不做

- 不实现 HEIC / RAW 来源（PR13）。
- 不实现缓存 GC（推到后续阶段）。
- 不接任务队列（PR15）。
- 不改 schema（PR10）。
- 不重写前端时间线 UI（PR14）。

## 验收标准

- 扫描一个含 JPEG 的目录后，`%APPDATA%/Glance/thumbs/{240,480,1080}/...` 下有真实 WebP 文件。
- `thumbnails` 表行数 = 已扫描 display 文件数 × 3。
- WebP 是 lossy，单张 240 文件显著小于改造前 lossless 版本。
- `pnpm tauri dev` 起来后时间线显示真实缩略图，asset 协议 hit 率高。
- 带 Adobe RGB / P3 profile 的样本图，缩略图色彩不明显偏灰。
- `cargo test` 通过。

## 建议验证

```bash
cd src-tauri && cargo test
pnpm tauri dev      # 加目录 → 扫描 → 时间线看到真实缩略图
```

人工对比 lossless / lossy 单张缩略图大小，确认 lossy 输出明显更小。
