# S1-PR15 Background Task Queue

你是负责实现 Glance 的编码 Agent。当前 PR 的目标是把扫描、缩略图生成、按需补齐等后台工作从"同步阻塞 command"模式迁移到统一的优先级任务队列，并按 IO / CPU 类型分别走两个 worker 池。完成后扫描和缩略图按需生成应不再卡死 IPC 主线程。

## 依赖

本 PR 基于：

- `S1-PR8` IPC + asset 协议
- `S1-PR9` 缩略图整合
- `S1-PR12` 扫描事务 / 续扫（同样在 worker 内运行）
- `Docs/glance_architecture.md` §6 后台任务模型

开始前阅读：

- `Docs/glance_architecture.md` §6 全文
- `src-tauri/src/core/tasks/mod.rs`（当前空文件）

## 任务

1. 定义任务模型：
   - `enum TaskPriority { High, Normal, Low }`。
   - `enum Task { ThumbnailOnDemand { photo_id, tier }, MetadataExtraction { photo_file_id }, ThumbnailPrefetch { photo_id }, OrphanGc, ScanLibrary { library_id } }`。
   - `enum Pool { Io, Cpu }`，每个 Task 类型固定属于哪个池。

2. 实现队列：
   - 单一全局优先级队列（高 → 中 → 低）。
   - 每个 Pool 独立一个 worker pool：
     - IO 池：2–4 个 worker（可配置，从 config.json 读，默认 3）。
     - CPU 池：`num_cpus::get()` 个 worker。
   - worker 取任务时按优先级出列。
   - 高优先级任务可以"插队"到队列前端，但不强抢正在执行的低优任务。

3. 取消 / 暂停：
   - `Arc<AtomicBool>` 形式的 cancellation token。
   - `cancel_all()` 让所有 worker 在当前任务边界停下。
   - `cancel_scan(library_id)` 仅停指定扫描。
   - 暂停粒度到批 / 单文件级别，不强杀。

4. 任务接入：
   - 扫描：`library_scan` command 不再直接跑 scan，而是 enqueue 一个 `ScanLibrary` 低 / 中优先级任务并立即返回 ScanJob。
   - 按需缩略图：asset 协议 handler 缓存未命中且原文件可用时，enqueue 高优先级 `ThumbnailOnDemand`，同步等待结果或先返回占位 + 后续刷新。
   - 三档预生成：`ScanLibrary` 完成后自动 enqueue 低优先级 `ThumbnailPrefetch` 批量任务。
   - EXIF / XMP 提取：可继续在 scan 主流程同步做（短任务），不一定必须进队列。

5. 进度事件聚合：
   - worker 完成任务后发 `tasks:progress { type, pool, queue_depth }` 事件，便于前端调试和日后展示。
   - 不在本 PR 做前端展示。

6. 持久化（仅 ScanLibrary）：
   - 应用退出 / 重启时未完成的 `ScanLibrary` 任务通过 `scan_jobs.status` 自动恢复（PR12 已实现 cursor）。
   - 其他任务（缩略图、GC）不持久化，重启时丢弃。

7. 测试：
   - 队列优先级用例：高、中、低混合提交，按预期顺序执行。
   - 取消：发起 100 个任务，调 `cancel_all` 后剩余任务被丢弃。
   - 并发：CPU 池真的并发，用一个 sleep-based 任务测量耗时。
   - 集成：模拟"扫描中前端打开 Lightbox 触发高优缩略图请求" → 缩略图请求先完成。

## 不做

- 不做分布式 / 多进程。
- 不做任务持久化（除已经通过 scan_jobs 做的扫描续扫）。
- 不做前端任务面板 UI。
- 不引入 tokio 全局 runtime（Tauri 已内置 tokio，可以复用，但不需要为此 PR 改 main runtime）。

## 验收标准

- `core/tasks/mod.rs` 不再是空文件，含完整队列 + 双 worker 池实现。
- 扫描期间前端 IPC 仍然响应（如 `timeline_query` 不被阻塞数秒）。
- Lightbox 点开未生成的大图时，能在 1 秒内看到 1080 档（小图测试场景）。
- `cargo test` 通过，新增至少 4 个测试覆盖优先级、取消、并发、集成。
- 扫描中关闭应用 → 重开 → 自动续扫。

## 建议验证

```bash
cd src-tauri && cargo test
pnpm tauri dev
```

人工：扫描进行中切换图库列表 / 打开 Lightbox 应保持响应。
