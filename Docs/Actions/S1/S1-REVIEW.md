# S1 Stage Review

**Review Date:** 2026-05-17
**Reviewer:** Design owner pass after S1 build agent reported "Done"
**Scope:** S1-PR1 through S1-PR7

## 1. 总体判断

S1 阶段交付了 **模块化的零件 + 完备的单元测试**，但**没有装配**。`cargo test` 53/53 通过、`pnpm build` 成功，但端到端跑不起来。

| 维度 | 状态 |
|---|---|
| 编译 / 测试 | ✅ Rust 53/53、前端构建成功 |
| 项目骨架 | ✅ 目录、Cargo / package 依赖 |
| Schema + 迁移 | ✅ 初版 migration 落库 |
| 核心模块（identity / exif / scanner / thumbnail / dao） | ⚠️ 实现 + 单测有，但与设计有偏差 |
| `core/raw`、`core/tasks` | ❌ 仅注释，未实现 |
| **Tauri 命令注册** | ❌ `generate_handler![]` 为空 |
| **AppState / DB 连接管理** | ❌ 无 |
| **asset:// 协议** | ❌ 未注册 |
| 缩略图接入扫描流程 | ❌ 未调用 |
| 前端 → 后端 IPC | ❌ 全部失败（无对应 command） |

S1 STATUS.md 已自报了 IPC / asset 协议 / commands 三项缺口，但还有大量未列出的实质 bug，详见下文。

## 2. 决策更新（本次确认）

| 决策 | 来源 | 行动 |
|---|---|---|
| `format` 字段从 `photos` 迁移到 `photo_files` | 设计文档与实现不一致；按设计走 | 后续 PR 直接改源码，不保留兼容 |
| 当前阶段允许破坏式重构 | 用户明确 | 重命名 / 改 schema / 改命令签名都不顾历史数据 |
| HEIC 支持以 `--features heic` 开关存在 | libheif-rs 需 vcpkg 系统库 | 默认 feature 不含 HEIC，CI 不强制开 |
| 引入 `rawler`、`qcms`、`libheif-rs (optional)` | 设计要求但 S1 没引入 | 已加入 `Cargo.toml` |

`Cargo.toml` 当前状态（已修改）：

```toml
rawler = "0.7.2"                                            # RAW 内嵌 JPEG
qcms = "0.3.0"                                              # 色彩管理
libheif-rs = { version = "2.7.0", optional = true }         # HEIC，需要 vcpkg

[features]
default = []
heic = ["dep:libheif-rs"]
```

## 3. 阻断性问题（P0）

无法绕过、不修复就跑不通 MVP 的问题。

### 3.1 Tauri 命令层完全是空的

- `src-tauri/src/commands/mod.rs` 只有两行注释。
- `lib.rs` 的 `tauri::generate_handler![]` 是空数组。
- 没有 `tauri::Manager::manage` 注入 DB 连接，没有任何 `tauri::State<...>`。
- 前端 `invoke('library_list')`、`invoke('timeline_query')` 等 **全部找不到 handler**。

修复需要：
1. 一个 `AppState { db: Arc<Mutex<rusqlite::Connection>> }`。
2. `Builder::setup` 里初始化 AppData 目录、打开 DB、跑 migration、把 state 装进 app。
3. 至少注册 `library_list / library_add / library_scan / library_relocate_folder / timeline_query / photo_detail / photo_relocate_file / thumbnail_url` 八个 command。
4. 一个 e2e 冒烟测试（先用 cargo test 模拟前端调用流程，或人工跑 `tauri dev`）。

### 3.2 `asset://thumb/{photoId}/{tier}` 协议没注册

- 设计明确"前端走 Tauri custom asset protocol，不走 base64、不走 HTTP server"。
- `lib.rs` 没有 `register_uri_scheme_protocol`。
- 前端 `getThumbnailUrl` 即便有返回字符串，也指向不存在的资源。

修复需要：
1. 在 `Builder` 上注册 `asset` 自定义 URI scheme。
2. Handler 解析 `asset://thumb/{photoId}/{tier}`，查 `thumbnails.source_hash`，定位 `thumbs/{tier}/{hash[0:2]}/{hash}.webp` 并返回 bytes。
3. 缓存缺失且原文件可用：排队补齐 + 返回临时占位图（或同步生成）。
4. 缓存缺失且原文件 missing：返回 placeholder。

### 3.3 缩略图根本没被生成

- `thumbnail::generate_thumbnails` 实现 + 测试都有。
- `scanner::orchestrator::run_scan` **从未调用它**。
- 扫完一个图库，`thumbnails` 表为空、`thumbs/` 目录里没文件。

修复需要：在 `create_new_photo`（以及未来 `update_existing_photo` 检测到 display 文件变化时）触发缩略图生成，并写 `thumbnails` 表。可以是同步调用（MVP）或排进 tasks 队列（更好，但 tasks 模块还没实现）。

## 4. 实质 Bug（P1，按严重度）

### B1. EXIF 数值读取大面积漏字段

`exif/mod.rs` 的辅助函数太挑类型：

```rust
fn get_exif_u32(...) -> Option<u32> {
    match &field.value {
        Value::Long(vec) => vec.first().copied(),
        _ => None,                          // ← Short / SShort / Rational 全部漏掉
    }
}
```

实际 EXIF 里 `PhotographicSensitivity (ISO)`、`ImageWidth`、`PixelXDimension`、`ImageLength` 多数相机存为 `Value::Short`。结果：**绝大多数照片读出来 ISO 和宽高都是 None**。

修复：把数值类辅助函数改成同时接受 `Short / Long / SShort / SLong`，并合理 fallback。

### B2. WebP 缩略图用了 lossless 编码

```rust
let encoder = WebPEncoder::new_lossless(&mut cursor);
```

240px 缩略图 lossless ≈ 80–150KB，lossy q=85 只要 5–15KB。10 万张照片 × 三档 ≈ **多占 20–40GB 磁盘**。

修复：改成 lossy q=85（或可配置）。注意 `image` crate 的 WebP 编码器只支持 lossless，需要换用 `webp` crate 或 `image-webp` 的 lossy 接口。

### B3. `fast_image_resize` 是死依赖

Cargo.toml 已经引了 `fast_image_resize`，但 `thumbnail/mod.rs` 用的是 `image::imageops::resize`（Lanczos3 在 6000×4000 大图上慢 5–10×）。整个 perf 卖点没兑现。

修复：把 resize 切换到 `fast_image_resize`。

### B4. HEIC 直接 panic-on-open

- `image::open` 不支持 HEIC。
- 现在 `libheif-rs` 已经作为 optional dep 引入，但代码没有 cfg 切换。
- 任何 `.heic` 文件扫到就会失败。

修复：在 thumbnail / exif 路径加 `#[cfg(feature = "heic")]` 分支，调用 libheif 解码。无 feature 时返回明确的 "不支持" 错误而不是 panic。

### B5. ICC / sRGB 转换没做

```rust
let rgb = img.to_rgb8();   // 直接丢 ICC profile
```

Adobe RGB / Display P3 的 JPEG 经过这步 → 颜色发灰（按 sRGB 解释 Adobe RGB 数据）。设计要求"统一转 sRGB 并嵌入 profile"，目前一字未做。

修复：用 `qcms` 读源图 ICC profile，转到 sRGB；在 WebP 编码前再嵌入 sRGB profile。

### B6. Scanner 把第二个 JPEG 误归为 RAW

```rust
if candidate.display_file.is_none() || (file.extension == "jpg" || ...) {
    if let Some(prev) = candidate.display_file.take() {
        candidate.raw_files.push(prev);   // ← prev 可能是 JPEG
    }
    candidate.display_file = Some(file);
}
```

`photo.jpg` 后扫到 `photo.jpeg`：旧 JPG 被推进 `raw_files`，`assign_roles` 给它打 `role=Raw`。脏数据。

修复：用一个独立的"备用 display"列表，只有 RAW 文件才进 `raw_files`。

### B7. `update_scan_job_status` 总是写 `finished_at`

```sql
UPDATE scan_jobs SET status = ?1, finished_at = ?2, ...
```

status 是 `'paused'` 或 `'running'` 时也会写完成时间。重试 / 续扫逻辑会判错。

修复：`finished_at` 只在 status 为 `'done'` 或 `'failed'` 时写入。

### B8. 增量扫描不刷新已有 photo 的 metadata

`update_existing_photo` 只更新 `photo_files`，不重读 EXIF / XMP。用户在 C1 里把 Rating 从 3 改到 5，重扫之后 DB 里还是 3。

修复：检测到 display 文件或 sidecar 的 `mtime` 变化时重读 metadata 并 update `photos` 行。

### B9. 整个扫描没有事务包裹

每次 `dao::insert_photo_file` 都是独立 commit。一次扫 5 万张 = 5 万次 fsync。**SQLite WAL 模式下也会非常慢**，且崩溃时残留半截状态。

修复：每 N 个 candidate（建议 100–500）一个事务，或者整个 scan 一个事务（小图库 OK，大图库会占内存）。

### B10. `scan_jobs.cursor` 形同虚设

- 列建了。
- DAO 写函数 `update_scan_job_cursor` 也有。
- `run_scan` 一次性把所有路径加载到内存，**没有调用过 cursor 更新**。
- 设计声称"可中断、断点续扫"，目前为零。

修复：把 `discover_media_files` 改成 streaming iterator，每处理完一组 candidate 写 cursor。

### B11. `process_candidate` 用任意一个文件命中即判定"已存在"

当前逻辑：候选组里任意一个文件 hash 命中现有 photo，整组就关联到那个 photo。如果 XMP sidecar 内容偶然撞 hash（多个空 XMP 都从同一模板复制），就会把不相关的照片合并。

修复：先按"display 或 raw 文件"的 identity 查找，不用 sidecar 的 identity 决定 photo 归属。

### B12. `format` 字段位置与设计不符

- 设计：`photo_files.format`（每个物理文件有自己的格式）。
- 实现：`photos.format`（实体级，丢信息）。
- JPEG + ARW 配对时，记 photos 上只能记一个格式。

修复：写第二个 migration，把 `format` 列从 `photos` 移到 `photo_files`，并清除老列。同时更新 DAO 和扫描流程。当前阶段不保留兼容数据。

### B13. `photo_files` schema 与设计不齐

| 项 | 设计 | 实现 |
|---|---|---|
| `library_id` 列 | 有 | 无 |
| `UNIQUE` 约束 | `(library_id, path)` | `(path)` |
| `missing_since` 列 | 有 | 无 |
| identity 索引 | `(library_id, content_hash, file_size)` | `(content_hash, file_size)` |
| status 索引 | `(library_id, status)` | `(status)` |

多图库放在 0.2，但 schema 应该一开始就预留。修复见上述 migration。

## 5. 前端实质问题（P1）

### F1. Lightbox 从来没被打开

`Lightbox.tsx` 写好了，但 `Timeline.tsx` 的 `ThumbnailGrid` 没传 `onPhotoClick` 处理函数。**点缩略图无反应**。

修复：在 `Timeline` 维护选中的 photoId state，传给 `ThumbnailGrid → ThumbnailItem` 的 `onClick`，渲染 `<Lightbox photoId={...} onClose={...} />`。

### F2. 时间线虚拟滚动用法不对

```tsx
<VirtuosoGrid
  totalCount={groupedPhotos.length}        // ← 日期组数当 cell 数
  itemContent={(index) => {
    const group = groupedPhotos[index];
    return <div>...日期标题 + ThumbnailGrid...</div>
  }}
/>
```

`VirtuosoGrid` 是按"等宽等高 cell"设计的。这里把整个日期组（含 header + 多张缩略图）塞进单个 cell：列数错乱、高度估计失效、sticky header 丢失。

修复：改用 `<Virtuoso>` + `groupCounts` / `groupContent` API。`groupCounts={[3, 5, 12, ...]}` 描述每个日期组有多少张照片，`groupContent` 渲染日期分隔，`itemContent` 渲染单张缩略图，sticky header 自动支持。

### F3. 每张缩略图独立 invoke 一次拿 URL

```tsx
useEffect(() => {
  const url = await getThumbnailUrl(photo.id, tier);
  setThumbnailUrl(url);
}, [photo.id, viewMode]);
```

200 张可见缩略图 = 200 次 IPC 往返。Tauri command 序列化 + 跨 webview 边界成本可观。

修复：`timeline.query` 直接在 `PhotoSummary` 里返回 `asset://thumb/{photoId}/{tier}` 字符串，前端不再 invoke。

## 6. 偏离 / 待补功能

- `core/raw/mod.rs` 是空文件 — 设计要求用 rawler 提内嵌 JPEG，目前没实现。RAW-only 照片无法生成预览。
- `core/tasks/mod.rs` 是空文件 — 设计要求 IO 池 + CPU 池 + 优先级队列，目前所有扫描和缩略图都是单线程同步。
- 设计要求"图库不可达时进入离线状态"，但 scanner 在根目录不存在时直接 crash，没有 graceful 处理。

## 7. 建议的后续 PR 顺序

按依赖和 ROI 排：

| PR | 主题 | 含 |
|---|---|---|
| **S1-PR8** | Tauri IPC 接通 | 3.1 + 3.2 + F3 |
| **S1-PR9** | 缩略图修复 + 接入扫描流程 | 3.3 + B2 + B3 + B5 |
| **S1-PR10** | Schema 重整 | B12 + B13（一个 migration） |
| **S1-PR11** | EXIF / Scanner bug 修复 | B1 + B6 + B7 + B8 + B11 |
| **S1-PR12** | 扫描事务 + 断点续扫 | B9 + B10 |
| **S1-PR13** | HEIC + RAW 解码（启用 `heic` feature） | B4 + raw/ 模块 |
| **S1-PR14** | 前端时间线重写 | F1 + F2 |
| **S1-PR15** | 任务队列（IO / CPU 池） | tasks/ 模块 |

PR8–PR9 是发布前必做；PR10–PR12 是质量底线；PR13 决定 HEIC/RAW 进 MVP 还是 0.2；PR14 决定时间线体验；PR15 可以推到 S2。

## 8. 当前命令验证

```bash
# 后端
cd src-tauri && cargo test         # 53 个测试通过（不能验证 IPC，因为没注册）
cd src-tauri && cargo check        # 默认 feature 通过
cd src-tauri && cargo check --features heic   # 需要本机已 `vcpkg install libheif`

# 前端
pnpm build                          # 通过
pnpm tauri dev                      # 启动后所有功能按钮无响应
```

## 9. 给下一个 Agent 的提醒

- 不要再把"测试通过"等同于"功能可用"。先验证一条最小端到端路径：选目录 → 扫描 → 看到一张缩略图 → 点开 Lightbox。
- 改 schema 不需要兼容旧数据，直接重写 migration 文件即可。
- 改任何 command 签名时，要同步改前端 `src/ipc/*.ts`。
- 先开 PR8，否则后面所有 PR 都没法人工验证。
