# S1-PR11 EXIF And Scanner Bug Fixes

你是负责实现 Glance 的编码 Agent。当前 PR 的目标是修复 S1 评审里列出的 EXIF 类型匹配漏字段、扫描器误归类、scan_job 状态、重扫不刷新 metadata、identity 误判等一组实质 bug。本 PR 不引入新功能，仅消除已知错误并补强相关测试。

## 依赖

本 PR 基于：

- `S1-PR4` metadata 提取
- `S1-PR5` 扫描编排
- `S1-PR10` schema 重整
- 评审结论 `S1-REVIEW.md` B1、B6、B7、B8、B11

开始前阅读：

- `Docs/Actions/S1/S1-REVIEW.md` 第四节"实质 Bug"
- `src-tauri/src/core/exif/mod.rs`
- `src-tauri/src/core/scanner/mod.rs`
- `src-tauri/src/core/scanner/orchestrator.rs`

## 任务

1. 修复 EXIF 数值读取（B1）：
   - 把 `get_exif_u32` / `get_exif_u16` 改成一组健壮的 helper，接受 `Value::Short`、`Value::Long`、`Value::SShort`、`Value::SLong`、`Value::Byte`，并按需向上转 `u32`/`u64`。
   - `get_exif_rational` 支持 `Value::Rational` 和 `Value::SRational`。
   - ISO（`PhotographicSensitivity`、`ISOSpeed`、`ISOSpeedRatings`）、宽高（`PixelXDimension` / `PixelYDimension` / `ImageWidth` / `ImageLength`）必须能正确读出来。
   - 用真实样本（仓库放 1–2 张带完整 EXIF 的小 JPEG）写集成测试，断言 ISO、aperture、focal_len、宽高、orientation 都能读到。

2. 修复扫描器 display 优选逻辑（B6）：
   - 当前实现：第二个 JPEG 出现时，第一个被推进 `raw_files`，后续按 RAW 处理。
   - 改成：用一个 `pending_displays: Vec<DiscoveredFile>` 暂存所有 display 候选，分组结束后选一个为 display（JPEG > HEIC > 其他），其余作为同 photo 的 `role='duplicate'` 文件。
   - 确保 RAW 列表里永远只含真正的 RAW 文件。

3. 修复 `update_scan_job_status`（B7）:
   - 只在 status 为 `'done'` 或 `'failed'` 时写入 `finished_at`。
   - 改成两个签名清晰的函数：`mark_scan_complete(id, added, updated, missing)`、`mark_scan_failed(id, error)`、`mark_scan_paused(id)`，或保留单一函数但内部按 status 分支。

4. 实现 metadata 刷新（B8）：
   - `update_existing_photo` 检测到 display 文件 `mtime` 或 size 变化时，重读 EXIF。
   - 检测到 sidecar 文件 mtime 变化时，重读 XMP Rating / Label。
   - 写回 `photos` 行（taken_at、camera_*、lens、focal_len、aperture、shutter、iso、width、height、orientation、gps_*、rating、label）。
   - 不重新计算 `indexed_at`（保留首次入库时间）；新增一个 `updated_at` 字段写当前时间（架构文档已包含此列）。

5. 修复 identity 误判（B11）：
   - `process_candidate` 当前会用任意一个文件命中现有 photo 决定整组归属，这导致 XMP sidecar 偶然撞 hash 时关联错误的 photo。
   - 改为：只用 `display_file` 或第一个 RAW 文件的 identity 来确定 photo 归属，sidecar 不参与。
   - 如果一个 candidate 没有 display 也没有 RAW（理论上只有孤立的 sidecar），跳过并日志告警。

6. 清理死代码与告警：
   - `ProcessOutcome::Unchanged` 当前未构造但 enum 留着。决定要么真正使用（同路径 mtime 未变时返回），要么删除。
   - 修掉 `cargo check` 产生的 warning。

7. 测试增强：
   - 真照片 EXIF 集成测试（见任务 1）。
   - 同 stem 多 JPEG 的分组用例。
   - 重扫 + sidecar mtime 变化 → rating 刷新用例。
   - 两个不同 photo 的 sidecar 内容相同（hash 相同）情况下，新 candidate 不会被关联到旧 photo。

## 不做

- 不改前端。
- 不改 schema（PR10）。
- 不修缩略图（PR9）。
- 不加 HEIC / RAW 解码（PR13）。

## 验收标准

- 真照片样本扫完后，DB 中 ISO、aperture、焦段、宽高、orientation 都非 None。
- 同目录两个 JPG（不同 stem）和一个 ARW 一个 JPG 配对场景测试通过。
- 重扫已修改 XMP 后 rating 字段被刷新。
- `cargo check` 0 warning。
- `cargo test` 通过；测试数量增加。

## 建议验证

```bash
cd src-tauri && cargo test
cd src-tauri && cargo check       # 应无 warning
```

人工验证：在 Lightbox 中看到正确的 ISO、光圈、焦段值（PR8 后已可观察）。
