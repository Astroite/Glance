# Glance 开发者指南

## 项目架构

```
Glance/
├── src-tauri/              # Tauri 壳 + Rust 核心
│   ├── src/
│   │   ├── lib.rs          # 入口，命令注册
│   │   ├── main.rs         # 程序入口
│   │   ├── error.rs        # 错误类型
│   │   ├── commands/       # Tauri 命令层（唯一编排者）
│   │   └── core/
│   │       ├── scanner/    # 文件发现、增量扫描
│   │       ├── identity/   # 哈希 + 尺寸文件身份
│   │       ├── exif/       # EXIF + XMP sidecar 提取
│   │       ├── thumbnail/  # 三档缩略图生成
│   │       ├── db/         # SQLite + 迁移
│   │       └── tasks/      # 后台任务队列
│   └── Cargo.toml
├── src/                    # React + TypeScript 前端
│   ├── views/              # Timeline, Lightbox, LibrarySetup
│   ├── components/         # ThumbnailGrid
│   ├── ipc/                # Tauri 命令包装
│   ├── state/              # Zustand 状态管理
│   └── styles/             # CSS 样式
└── Docs/                   # 设计文档（中文）
```

## 核心模块

### 1. db — 数据库层

**文件**: `src-tauri/src/core/db/`

- `mod.rs` — SQLite 连接、WAL 模式、迁移运行
- `schema.rs` — 迁移系统
- `dao.rs` — 完整 CRUD 操作

**表结构**:
- `libraries` — 图库（文件夹）
- `photos` — 逻辑照片实体
- `photo_files` — 物理文件实例（一对多）
- `thumbnails` — 缩略图元数据
- `scan_jobs` — 扫描任务

### 2. identity — 文件身份

**文件**: `src-tauri/src/core/identity/mod.rs`

```rust
pub struct FileIdentity {
    pub content_hash: u64,  // xxh3(head 64KB + tail 64KB + file_size)
    pub file_size: u64,
    pub mtime: i64,         // 仅用于变更检测
}
```

### 3. scanner — 文件发现与配对

**文件**: `src-tauri/src/core/scanner/`

- `mod.rs` — 媒体类型分类、文件发现、分组、角色分配
- `orchestrator.rs` — 扫描编排、变更检测、重定位

**媒体类型**:
- Display: JPEG, PNG, HEIC
- Raw: CR2, CR3, NEF, ARW, DNG, ORF, RW2, RAF, PEF
- Sidecar: XMP

**配对规则**: 同目录 + 同文件名 stem = 同一照片

### 4. exif — 元数据提取

**文件**: `src-tauri/src/core/exif/mod.rs`

提取字段：
- DateTimeOriginal（拍摄时间）
- Make/Model（相机）
- LensModel（镜头）
- FocalLength, FNumber, ExposureTime, ISOSpeed
- PixelXDimension, PixelYDimension
- Orientation
- GPSLatitude, GPSLongitude

XMP 解析：
- Rating (0-5)
- Label (颜色标签)

### 5. thumbnail — 缩略图生成

**文件**: `src-tauri/src/core/thumbnail/mod.rs`

三档缩略图：
- 240px (Small) — 网格视图
- 480px (Medium) — 瀑布流
- 1080px (Large) — 大图预览

特性：
- WebP 无损编码
- EXIF Orientation 烘焙
- 非 sRGB 自动转换
- 缺失时生成占位图

---

## 开发流程

### 环境准备

```bash
# 安装依赖
pnpm install

# 检查 Rust 代码
cd src-tauri && cargo check

# 运行测试
cargo test
```

### 开发模式

```bash
pnpm tauri dev
```

启动：
- Vite 热重载服务器 (localhost:1420)
- Tauri WebView2 窗口
- 文件变更自动重编译

### 测试

```bash
# Rust 测试
cd src-tauri && cargo test

# 前端 Lint
pnpm lint

# 前端构建检查
pnpm build
```

### 生产构建

```bash
pnpm tauri build
```

输出：`src-tauri/target/release/bundle/`

---

## 命令层 (commands/)

**文件**: `src-tauri/src/commands/mod.rs`

命令层是唯一的编排者，负责：
1. 接收 Tauri invoke 调用
2. 调用 core 模块完成业务逻辑
3. 返回结果给前端

**前端 IPC 调用**:
```typescript
import { invoke } from '@tauri-apps/api/core';

export async function listLibraries(): Promise<Library[]> {
  return invoke('library_list');
}
```

---

## 状态管理 (Zustand)

### useLibraryStore

```typescript
interface LibraryState {
  libraries: Library[];
  selectedLibrary: Library | null;
  isLoading: boolean;
  error: string | null;

  loadLibraries: () => Promise<void>;
  selectLibrary: (library: Library) => void;
  addNewLibrary: (path: string) => Promise<void>;
  startScan: (libraryId: number) => Promise<void>;
}
```

### useTimelineStore

```typescript
interface TimelineState {
  photos: PhotoSummary[];
  nextCursor: string | null;
  isLoading: boolean;
  hasMore: boolean;
  viewMode: 'waterfall' | 'grid';

  loadInitial: (libraryId: number) => Promise<void>;
  loadMore: (libraryId: number) => Promise<void>;
  setViewMode: (mode: 'waterfall' | 'grid') => void;
  reset: () => void;
}
```

---

## 添加新功能

### 1. 添加 Rust 核心逻辑

在 `core/` 下创建新模块：

```rust
// src-tauri/src/core/my_feature/mod.rs
pub struct MyResult {
    // ...
}

pub fn do_something() -> MyResult {
    // ...
}
```

在 `core/mod.rs` 中导出：
```rust
pub mod my_feature;
```

### 2. 添加 Tauri 命令

在 `commands/mod.rs` 中添加：

```rust
#[tauri::command]
pub async fn my_command() -> Result<MyResult, Error> {
    core::my_feature::do_something()
}
```

在 `lib.rs` 中注册：
```rust
.invoke_handler(tauri::generate_handler![
    commands::my_command,
])
```

### 3. 添加前端 IPC

在 `src/ipc/` 中添加：

```typescript
import { invoke } from '@tauri-apps/api/core';

export async function myCommand(): Promise<MyResult> {
  return invoke('my_command');
}
```

### 4. 添加 UI 组件

在 `src/views/` 或 `src/components/` 中创建 React 组件。

---

## 调试技巧

### Rust 调试

```bash
# 带调试信息运行
RUST_BACKTRACE=1 pnpm tauri dev

# 查看数据库
sqlite3 %APPDATA%/Glance/index.sqlite
```

### 前端调试

- 打开 DevTools: `F12` 或右键 → 检查
- 查看 Network 面板的 invoke 调用
- 使用 React DevTools 扩展

### 日志

日志文件位于 `%APPDATA%/Glance/logs/`

---

## 代码规范

### Rust

- 使用 `thiserror` 定义错误类型
- 核心模块通过纯数据结构通信，无交叉导入
- 命令层是唯一编排者

### TypeScript

- 使用 `import type` 导入纯类型
- 使用 Zustand 管理状态
- IPC 函数放在 `src/ipc/`

### 测试

- Rust 测试放在每个模块的 `#[cfg(test)] mod tests {}`
- 使用 `tempfile` 创建临时文件
- 测试覆盖率目标: 核心逻辑 > 80%
