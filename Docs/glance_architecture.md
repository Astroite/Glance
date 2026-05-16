# Glance 架构文档

本文档描述 Glance 的技术框架。设计目标和产品定位见 `glance_design_document.md`。

---

## 1. 仓库结构

```
Glance/
├── src-tauri/                  # Tauri 壳 + Rust 核心
│   ├── src/
│   │   ├── main.rs             # 应用入口、command 注册
│   │   ├── commands/           # 前端可见的 Tauri command
│   │   ├── core/
│   │   │   ├── scanner/        # 目录遍历、增量扫描
│   │   │   ├── identity/       # hash + size 文件身份，mtime 变更检测
│   │   │   ├── exif/           # EXIF + XMP sidecar
│   │   │   ├── thumbnail/      # 三档缩略图生成
│   │   │   ├── raw/            # 内嵌 JPEG 提取
│   │   │   ├── db/             # SQLite + 迁移
│   │   │   └── tasks/          # 后台任务队列
│   │   └── error.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                        # 前端（React + TypeScript）
│   ├── views/
│   │   ├── Timeline/
│   │   ├── Lightbox/
│   │   └── LibrarySetup/
│   ├── components/
│   ├── ipc/                    # Tauri command 封装
│   ├── state/                  # 全局状态、查询缓存
│   └── styles/
├── Docs/
└── README.md
```

---

## 2. 模块划分

### 2.1 Rust 核心模块

| 模块 | 职责 | 不做 |
|---|---|---|
| `scanner` | 遍历目录、识别媒体文件、产出待处理任务 | 不读 EXIF、不生成缩略图 |
| `identity` | 计算文件身份（`hash + size`），辅助判断 `mtime` / size 变化 | 不访问数据库 |
| `exif` | 提取 EXIF + XMP sidecar，输出结构化 metadata | 不决定主显示文件 |
| `thumbnail` | 生成三档缩略图，Orientation 烘焙、sRGB 转换 | 不决定缓存策略 |
| `raw` | 从常见 RAW 格式提取内嵌 JPEG | 不做 demosaic |
| `db` | schema 迁移、CRUD、查询 | 不放业务编排 |
| `tasks` | 任务队列、并发控制、优先级、可中断 | 不直接操作文件 |
| `commands` | Tauri command 层，编排上述模块 | 不放业务逻辑（薄层） |

**模块边界原则：** 模块之间通过 plain 数据结构通信，不互相 import。`commands` 是唯一编排者，业务逻辑集中在此。

### 2.2 前端模块

- **框架：** React + TypeScript
- **构建：** Vite（Tauri 默认）
- **状态：** 全局状态用轻量方案（zustand 或类似），服务端数据用 @tanstack/query 风格的查询缓存
- **虚拟滚动：** react-virtuoso（支持变高、分组吸顶）
- **图片加载：** Tauri custom asset protocol（`asset://`），不走 base64，不走 HTTP server

---

## 3. 数据库 Schema

### 3.1 核心表

```sql
-- 图库（MVP 单图库，library_id 始终带）
CREATE TABLE libraries (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  root_path   TEXT NOT NULL,
  created_at  INTEGER NOT NULL
);

-- 照片（逻辑实体；RAW+JPEG 组合在这里表现为一张照片）
CREATE TABLE photos (
  id              INTEGER PRIMARY KEY,
  library_id      INTEGER NOT NULL REFERENCES libraries(id),
  display_file_id INTEGER,           -- 指向负责展示/缩略图的 photo_files.id，由应用层维护

  taken_at        INTEGER,           -- EXIF DateTimeOriginal，缺失回退 mtime
  taken_at_src    TEXT,              -- 'exif' | 'mtime'

  camera_make     TEXT,
  camera_model    TEXT,
  lens            TEXT,
  focal_len       REAL,
  aperture        REAL,
  shutter         REAL,              -- 秒
  iso             INTEGER,
  width           INTEGER,
  height          INTEGER,
  orientation     INTEGER,           -- 1..8 原始 EXIF 值
  gps_lat         REAL,
  gps_lon         REAL,

  rating          INTEGER,           -- 来自 XMP，0..5
  label           TEXT,              -- 来自 XMP，颜色标签

  indexed_at      INTEGER NOT NULL,
  updated_at      INTEGER NOT NULL
);

-- 文件实例（一张照片可对应多份物理文件：JPEG/RAW/sidecar/重复文件）
CREATE TABLE photo_files (
  id            INTEGER PRIMARY KEY,
  library_id    INTEGER NOT NULL REFERENCES libraries(id),
  photo_id      INTEGER NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
  path          TEXT NOT NULL,
  role          TEXT NOT NULL,       -- 'display' | 'raw' | 'sidecar' | 'duplicate'
  format        TEXT NOT NULL,       -- 'jpeg' | 'heic' | 'arw' | 'xmp' ...
  content_hash  TEXT NOT NULL,       -- xxh3(head 64KB + tail 64KB + size)
  file_size     INTEGER NOT NULL,
  mtime         INTEGER NOT NULL,
  status        TEXT NOT NULL DEFAULT 'available', -- 'available' | 'missing'
  missing_since INTEGER,
  last_seen_at  INTEGER NOT NULL,    -- 最近一次扫描看到该路径的时间
  last_scan_id  INTEGER,             -- 最近一次扫描的 job id
  UNIQUE(library_id, path)
);

-- 缩略图缓存元数据（实际路径由 source_hash 推导）
CREATE TABLE thumbnails (
  photo_id       INTEGER NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
  tier           INTEGER NOT NULL,   -- 240 | 480 | 1080
  source_file_id INTEGER NOT NULL REFERENCES photo_files(id),
  source_hash    TEXT NOT NULL,
  width          INTEGER NOT NULL,
  height         INTEGER NOT NULL,
  generated_at   INTEGER NOT NULL,
  PRIMARY KEY (photo_id, tier)
);

-- 扫描任务（支持断点续扫）
CREATE TABLE scan_jobs (
  id          INTEGER PRIMARY KEY,
  library_id  INTEGER NOT NULL REFERENCES libraries(id),
  status      TEXT NOT NULL,         -- 'running' | 'paused' | 'done' | 'failed'
  cursor      TEXT,                  -- 当前扫到的相对路径
  started_at  INTEGER NOT NULL,
  finished_at INTEGER
);

CREATE INDEX idx_photos_timeline ON photos(library_id, taken_at DESC);
CREATE INDEX idx_photo_files_photo ON photo_files(photo_id);
CREATE INDEX idx_photo_files_path_prefix ON photo_files(path);
CREATE INDEX idx_photo_files_identity ON photo_files(library_id, content_hash, file_size);
CREATE INDEX idx_photo_files_status ON photo_files(library_id, status);
```

### 3.2 设计决策

**两层结构（photos / photo_files）：**

- `photos` 是逻辑实体，时间线中的一项就是一行 `photos`。
- `photo_files` 是物理文件实例，一张照片可以关联 JPEG、RAW、XMP sidecar 或重复文件。
- 同目录、同文件名 stem 的 RAW + JPEG 在 MVP 中归为一张 `photos`；JPEG 文件的 `role='display'`，RAW 文件的 `role='raw'`。
- 若没有 JPEG，RAW-only 文件可用内嵌 JPEG 作为预览来源，但不做完整 RAW 渲染。
- `PhotoSummary.isMissing` 由关联 `photo_files.status` 推导；任一需要保留的原文件缺失时，UI 都应提示用户确认并手动重定位。

**身份哈希算法：**

- 使用 **xxh3**，非密码学用途，速度极快。
- 输入：`head 64KB + tail 64KB + file_size`，不读全文件。
- 50MB ARW 实际只读 128KB，扫描速度可控。
- `content_hash + file_size` 是物理文件身份，存放在 `photo_files` 上。
- `mtime` 不参与身份判断，仅用于变更检测：同一路径的 `mtime` 或 size 变化时重新计算 hash。
- 如果同一路径重新计算后身份变化，旧 `photo_files` 标记为 `missing`，新文件作为新的文件实例进入索引；不静默覆盖旧照片。

**缩略图路径不入库：**

- 路径直接由 `thumbnails.source_hash` 推导：`thumbs/{tier}/{hash[0:2]}/{hash}.webp`
- 每张照片在 MVP 中生成 240 / 480 / 1080 三档缩略图。
- 删除孤立缩略图靠定期 GC，扫描 `thumbs/` 与 `thumbnails.source_hash` 做差集。
- 数据库只记录缩略图来源和尺寸信息，不保存绝对缓存路径。

**`taken_at` 缺失回退 mtime：**

- 部分文件缺 EXIF DateTimeOriginal，回退到文件 mtime。
- `taken_at_src` 字段标记来源，UI 可对回退值打弱化样式。

---

## 4. Tauri Command 接口

```ts
// 图库
library.list() → Library[]
library.add(path: string) → Library
library.scan(id: number) → ScanJob
library.relocate_folder(id: number, oldPrefix: string, newPrefix: string) → number
//   ↑ 返回受影响的 photo_files 行数

// 时间线（游标分页）
timeline.query(libraryId, cursor, filters) → {
  photos: PhotoSummary[],   // id, taken_at, thumbnailUrl, orientation, w, h, isMissing
  nextCursor: string | null
}

// 单张详情
photo.detail(id) → Photo
photo.relocate_file(photoFileId: number, newPath: string) → void
//   ↑ 用户确认后手动修正单个缺失文件路径

// 缩略图（返回 asset:// URL）
thumbnail.url(photoId: number, tier: 240 | 480 | 1080) → string

// 扫描进度（事件流，非命令）
event "scan:progress" → { jobId, processed, total, current }
event "scan:done"     → { jobId, added, updated, missing }

// 配置
config.get() → Config
config.set(patch: Partial<Config>) → void
```

**约定：**

- 时间线 query 只返回摘要字段，单张详情按需取。
- 缩略图走 Tauri custom asset protocol：`asset://thumb/{photoId}/{tier}`，由后端解析到 `thumbnails.source_hash`，不传 base64。
- 缩略图缓存缺失且原文件不可用时，asset handler 返回占位图；原文件可用时可排队生成缺失缓存。
- 长任务通过事件流推进度，不靠轮询。

---

## 5. 缺失与重定位机制

Glance 不自动判断文件是移动、重复、删除还是离线。只要扫描时找不到原路径，或同一路径文件身份已经变化，就统一把旧文件实例标记为 `missing`，并在重定位前继续使用已缓存的缩略图/预览图。

支持两个层级的用户主动重定位，路径变化时不丢失索引和缩略图缓存。

### 5.1 文件夹级重定位（用户主动触发）

适用场景：盘符变化（E: → F:）、整个图库目录搬迁、NAS 挂载点改名。

**接口：** `library.relocate_folder(libraryId, oldPrefix, newPrefix)`

**逻辑：**

1. 找到所有 `photo_files.path` 以 `oldPrefix` 开头的行。
2. 按前缀替换计算新路径。
3. 对新路径做轻量扫描验证；身份匹配后批量更新 path，并将相关 `photo_files.status` 恢复为 `available`。
4. 不重建已存在缩略图。

**用户体验：** 设置页提供"图库根目录已变更"按钮，输入新路径，一键迁移。

### 5.2 文件级重定位（手动确认）

**扫描处理：**

增量扫描期间，每个文件按以下流程处理：

```
1. 先按 path 查 photo_files
   ├─ path 已存在
   │    ├─ mtime/size 未变 → 更新 last_seen_at, status='available'
   │    └─ mtime 或 size 变化 → 重新计算 hash
   │         ├─ hash+size 未变 → 更新 mtime, last_seen_at, status='available'
   │         └─ hash+size 已变 → 旧行 status='missing'，当前文件按新实例入库
   └─ path 不存在
        └─ 按 RAW+JPEG/XMP 配对规则归入已有 photo 或创建新 photo
```

扫描结束后，`last_seen_at < scan.started_at` 的行统一标记为 `missing`。Glance 不自动推断它是移动、重复、删除还是离线，也不自动把新发现的同 hash 文件改成旧文件的新位置。

**手动重定位：** `photo.relocate_file(photoFileId, newPath)`

用户选择新路径后，Glance 重新计算新路径的 `hash + size` 并与原 `photo_files` 身份比对。匹配则更新 path、mtime、last_seen_at，并把 `status` 恢复为 `available`；不匹配则提示用户确认是否作为新文件导入。

---

## 6. 后台任务模型

### 6.1 任务队列

单一全局优先级队列，三档优先级：

| 优先级 | 任务类型 | 触发 |
|---|---|---|
| 高 | 可见缩略图按需补齐 | 前端滚动到视口，缓存未命中且原文件可用 |
| 中 | EXIF / XMP 读取 | 扫描发现新文件 |
| 低 | 三档缩略图预生成、孤立 GC | 扫描完成后 |

### 6.2 Worker 池

| 池 | 容量 | 任务类型 |
|---|---|---|
| IO 池 | 2–4 | 文件读取（限制并发以保护 NAS / SMB） |
| CPU 池 | `num_cpus` | 解码、缩放、哈希计算 |

### 6.3 可中断 / 断点续扫

- 每个扫描任务在 `scan_jobs.cursor` 中保存当前位置。
- 应用退出或用户暂停时，下次启动可从 cursor 继续。
- 任务取消通过 cooperative cancellation token，不强杀线程。

---

## 7. 本地存储布局

```
%APPDATA%/Glance/
├── index.sqlite              # 主库（WAL 模式）
├── index.sqlite-wal
├── index.sqlite-shm
├── thumbs/
│   ├── 240/
│   │   └── ab/
│   │       └── ab12cd...webp # 前两位 hash 分桶，避免单目录百万文件
│   ├── 480/
│   └── 1080/
├── config.json               # 用户设置
└── logs/
    └── glance.log
```

**说明：**

- SQLite 启用 WAL 模式，读写并发友好。
- 缩略图按 `hash[0:2]` 分桶，避免单目录文件过多导致文件系统性能退化。
- 缩略图格式默认 WebP（高压缩比、支持透明），后续可选 AVIF。

---

## 8. 关键数据流

### 8.1 首次扫描

```
用户选目录
  → library.add(path) 建图库
  → 创建 scan_job, status='running'
  → scanner 遍历目录
       → 按目录 + 文件名 stem 识别 RAW+JPEG+XMP 组合
       → 每个媒体文件入 IO 池
            → identity 算 hash
            → exif 读 metadata
            → db 写 photos + photo_files
            → 为 display 文件生成 240 / 480 / 1080 三档缩略图
  → scan_job.status='done', 触发事件 scan:done
```

### 8.2 时间线渲染

```
前端 timeline.query(libraryId, cursor)
  → SQL: SELECT id, taken_at, display_file_id, orientation, w, h
         FROM photos WHERE library_id=? AND taken_at < ?
         ORDER BY taken_at DESC LIMIT 200
  → 返回摘要列表
  → 前端虚拟滚动使用 asset://thumb/{photoId}/240 加载
  → 若缩略图不存在且原文件可用，asset handler 触发高优先级补齐任务
  → 若缩略图不存在且原文件缺失，asset handler 返回占位图
```

### 8.3 增量扫描

```
扫描启动时记 scan.started_at
  → 遍历目录，逻辑同 5.2
  → 扫描结束后：
       photo_files WHERE last_seen_at < scan.started_at → status='missing'
       UI 展示"原文件缺失"，继续使用缓存预览并提供手动重定位入口
```

---

## 9. 技术选型确认

| 项 | 选择 | 备注 |
|---|---|---|
| 桌面壳 | Tauri | WebView2，安装包 ~10MB |
| 核心语言 | Rust | |
| 前端框架 | React + TypeScript | |
| 构建 | Vite | Tauri 默认 |
| 虚拟滚动 | react-virtuoso | 变高 + 分组吸顶 |
| 数据库 | SQLite（WAL） | |
| 搜索 | SQLite FTS5 | 中文用 jieba-rs 自带 tokenizer |
| 身份哈希 | xxh3（head+tail+size） | |
| EXIF | kamadak-exif | |
| 缩略图编解码 | image + fast_image_resize | |
| HEIC | libheif | 不依赖系统 codec |
| RAW 提取 | rawler | 纯 Rust，提内嵌 JPEG |
| 视频预览 | FFmpeg，可选 | 默认不内置，按需下载 |
| 图片传输 | Tauri custom asset protocol | 不走 base64 / HTTP |
