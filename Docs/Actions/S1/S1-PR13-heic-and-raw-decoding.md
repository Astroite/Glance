# S1-PR13 HEIC And RAW Decoding

你是负责实现 Glance 的编码 Agent。当前 PR 的目标是补齐 MVP 中两类关键源文件支持：HEIC（通过 `libheif-rs`，feature gated）和 RAW 内嵌 JPEG 提取（通过 `rawler`）。完成后，iPhone HEIC 和 Sony ARW 等 RAW-only 照片都能进入时间线且有可用预览。

## 依赖

本 PR 基于：

- `S1-PR9` 缩略图整合 + 编码质量修复
- 评审结论 `S1-REVIEW.md` B4
- `Cargo.toml` 已引入 `rawler`、`qcms`、可选的 `libheif-rs`

开始前阅读：

- `Docs/glance_design_document.md` 5.3 缩略图、5.6 RAW 预览
- `Docs/glance_architecture.md` §9 技术选型确认
- `src-tauri/src/core/raw/mod.rs`（当前空文件）
- `src-tauri/src/core/thumbnail/mod.rs`

## 任务

1. 实现 RAW 内嵌 JPEG 提取（`core/raw`）：
   - 用 `rawler` 打开 RAW 文件，提取最大尺寸的内嵌预览 JPEG bytes。
   - 暴露 `extract_embedded_jpeg(path: &Path) -> Result<Vec<u8>, Error>`。
   - 不做 demosaic，不输出 RGB 像素，只返回 JPEG bytes。
   - 部分相机内嵌只有缩略图分辨率（如 1616×1080），有 large preview 时优先取 large。

2. RAW-only 照片的预览源策略：
   - 扫描配对中如果存在 display 文件（JPEG/HEIC），保持现行：display 文件作为缩略图来源。
   - 如果配对里**没有** display 文件，仅有 RAW：
     - 选第一个 RAW 文件作为缩略图来源。
     - 在生成缩略图时调用 `raw::extract_embedded_jpeg`，得到 JPEG bytes 后走标准 JPEG 解码 → resize → encode 链路。
     - `thumbnails.source_file_id` 指向那个 RAW 的 `photo_files.id`，`source_hash` 用 RAW 的 hash。
   - `photos.display_file_id` 在这种情况指向 RAW 文件。

3. HEIC 解码（feature gated）：
   - 仅在 `--features heic` 编译时生效。
   - 在 `core/thumbnail` 中加 `#[cfg(feature = "heic")]` 分支：检测到 `.heic` / `.heif` 扩展时调 `libheif_rs` 解码到 RGB（或 RGBA）buffer，然后接入与 JPEG 相同的 ICC → resize → encode 流程。
   - 同样把 HEIC 解码路径暴露给 EXIF 提取（`kamadak-exif` 对 HEIC 容器支持有限，可能需要 libheif 拿 EXIF metadata block 再交给 exif crate 解）。
   - 未启用 `heic` feature 时：扫描见到 `.heic` 文件应跳过缩略图生成、记录一个 `missing` 缩略图状态或简单跳过，不让扫描整体崩溃。

4. 错误处理：
   - rawler / libheif 解码失败不能 panic。
   - 失败时记录到日志、不写 `thumbnails` 行，photo 行仍然入库（用户至少看到时间线条目）。
   - 失败的 photo 在 UI 上显示占位图。

5. 文档：
   - 在 README / build instructions 加一节"启用 HEIC"，说明 Windows 上 `vcpkg install libheif` + `cargo build --features heic`。
   - 在 `Docs/glance_architecture.md` §9 把 HEIC 行更新为"libheif-rs，feature gated"。

6. 测试：
   - 单元：`raw::extract_embedded_jpeg` 用一张测试 ARW（仓库放一张小尺寸样本，或在测试中跳过 if 文件不存在 + #[ignore]）。
   - 单元：RAW-only 候选的扫描流程能写入 photo + photo_files + thumbnails。
   - HEIC 测试用 `#[cfg(feature = "heic")]` 包裹，CI 默认不跑（除非 CI 镜像装了 libheif）。

## 不做

- 不实现完整 RAW demosaic（永远不做，见设计 §6 非目标）。
- 不实现 AVIF / TIFF 高级解码。
- 不引入 GPU 解码。
- 不改前端时间线 UI（PR14）。

## 验收标准

- 仓库自带一张测试 ARW（或测试中明确路径），扫描后能看到对应 photo + 三档缩略图。
- 仓库自带一张测试 HEIC（feature 开启时），缩略图正常。
- 未启用 `heic` feature 编译仍通过。
- 启用 `heic` feature 编译需要本机 libheif，文档已写清。
- `cargo test` 默认 feature 全过。

## 建议验证

```bash
# 默认 feature
cd src-tauri && cargo test

# HEIC（需要先 vcpkg install libheif 或等价方式）
cd src-tauri && cargo test --features heic
```

人工：用一张 iPhone HEIC 和一张 Sony ARW 实际拖入图库，看缩略图是否生成正确。
