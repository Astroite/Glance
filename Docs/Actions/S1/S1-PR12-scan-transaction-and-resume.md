# S1-PR12 Scan Transaction And Resumable Scan

你是负责实现 Glance 的编码 Agent。当前 PR 的目标是让扫描具备生产级性能与可恢复能力：批量事务、流式发现文件、cursor 续扫。完成后，对一个万级图库扫描应当显著加速、并能在中途退出后从断点继续。

## 依赖

本 PR 基于：

- `S1-PR5` 扫描编排
- `S1-PR10` schema 重整
- `S1-PR11` 扫描 bug 修复
- 评审结论 `S1-REVIEW.md` B9、B10

开始前阅读：

- `Docs/glance_architecture.md` §6 后台任务模型、§8.3 增量扫描
- `src-tauri/src/core/scanner/mod.rs`
- `src-tauri/src/core/scanner/orchestrator.rs`

## 任务

1. 流式文件发现：
   - 把 `discover_media_files` 改成返回 `impl Iterator<Item = DiscoveredFile>` 或基于 channel 的 stream。
   - `group_into_candidates` 改成"按目录边界 flush"：扫到新目录时 emit 上一目录的所有 candidate。
   - 不要一次性把整个图库加载到内存。

2. 批量事务：
   - 在 orchestrator 中以 N 个 candidate（建议 N=200）为一批，每批包一次 `BEGIN; ... COMMIT;`。
   - 批与批之间允许其他读连接命中（WAL 模式下天然支持）。
   - 任一 candidate 处理失败：当前批 rollback、日志错误、整体扫描继续下一批，不让一个坏文件拖垮整个扫描。

3. cursor 续扫：
   - 每批 commit 之后调用 `dao::update_scan_job_cursor(scan_id, last_dir)`，记录已完成扫描的相对路径前缀。
   - 启动应用时检查 `scan_jobs WHERE status IN ('running', 'paused')`：
     - 若存在则提示前端 / 自动恢复（先做自动恢复 + `scan:resumed` 事件）。
   - 恢复时跳过 `path < cursor` 的文件，从 cursor 之后开始。
   - 完成时把 status 改为 `'done'` 并写 `finished_at`。

4. 取消 / 暂停：
   - 引入 `CancellationToken`（简易 `Arc<AtomicBool>` 即可）。
   - 增加 command `library_scan_pause(id)` 把 token 设为 cancel，并把 scan_job status 改 `'paused'`。
   - 增加 command `library_scan_resume(id)` 重新触发 scan，从 cursor 继续。
   - 暂停在批的边界生效，不强杀执行中的批。

5. 进度事件：
   - 每完成一批 emit `scan:progress { jobId, processed, total_estimated, current_dir }`。
   - `total_estimated` 可以是"已发现文件数"而不是"全部预扫描数"，避免预扫一遍开销。

6. 测试：
   - 大目录（脚本生成 1000+ 小文件）扫描可通过，耗时显著低于"无事务"基线（参考性能测试，不强制具体时间）。
   - 模拟"扫到一半 panic / cancel" → 重启 → 扫描从 cursor 继续，最终 photos 数量与一次性扫一致。
   - 单个坏文件（fake corrupt header）不会让 scan 整体失败。

## 不做

- 不接 IO / CPU 池（PR15）。
- 不实现前端暂停 / 恢复 UI；后端命令存在即可，UI 留给后续。
- 不改 schema（PR10）。

## 验收标准

- 1000 张小 JPEG 的合成测试目录扫描完成时间显著短于事务化前。
- 中途 pause → 再 resume 后，photos 总数与一次性扫完一致。
- 损坏文件存在时扫描结果包含 `errors[]`（在 scan_job 中或日志里），其余文件正常入库。
- `cargo test` 通过；新增至少 2 个集成测试覆盖续扫与坏文件容错。

## 建议验证

```bash
cd src-tauri && cargo test
```

人工：用一个真实大图库（如 5000+ 张）扫描，观察 CPU / 磁盘行为、进度事件、cursor 持续推进。
