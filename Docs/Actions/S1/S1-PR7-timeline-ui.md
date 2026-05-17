# S1-PR7 Timeline UI

你是负责实现 Glance 的编码 Agent。当前 PR 的目标是实现 MVP 首版前端体验：图库选择、扫描进度、时间线浏览、瀑布流/网格切换、年/月跳转、大图预览和缺失文件提示。

## 依赖

本 PR 基于：

- `S1-PR1` 工程骨架
- `S1-PR5` 图库扫描命令
- `S1-PR6` 缩略图 asset 协议

开始前阅读：

- `Docs/glance_design_document.md` 的时间线浏览、MVP 范围、缺失文件处理。
- `Docs/glance_architecture.md` 的 Tauri Command 接口和时间线渲染流程。

## 任务

1. 实现 IPC 封装：
   - `library.list`
   - `library.add`
   - `library.scan`
   - `timeline.query`
   - `photo.detail`
   - `thumbnail.url`
   - `photo.relocate_file`
   - `library.relocate_folder`

2. 实现 LibrarySetup：
   - 用户选择本地或 NAS 照片目录。
   - 添加图库。
   - 启动扫描。
   - 显示扫描进度和完成摘要。

3. 实现 Timeline：
   - 按年份、月份、日期组织。
   - 使用虚拟滚动，目标支持 10 万级图库。
   - 支持瀑布流视图。
   - 支持网格视图。
   - 提供视图切换控件。
   - 支持年 / 月快速跳转。
   - 缩略图通过 `asset://thumb/{photoId}/{tier}` 加载。

4. 实现 Lightbox：
   - 点击照片进入大图预览。
   - 使用 1080 档预览。
   - 展示基础 EXIF 信息和 XMP Rating / Label。
   - RAW+JPEG 组合在 UI 中表现为一张照片，可在详情中看到关联 RAW 文件。

5. 实现缺失文件体验：
   - `isMissing` 照片在时间线和 Lightbox 中明确提示"原文件缺失"。
   - 缺失时继续显示缓存预览；如果缓存也缺失，显示占位图。
   - 提供手动重定位入口。
   - 用户选择新路径后调用 `photo.relocate_file` 或 `library.relocate_folder`。
   - 不做自动移动检测。

6. 实现基础状态管理：
   - 全局轻量状态可使用 zustand 或同等简单方案。
   - 服务端数据使用 query cache 风格管理。
   - 加载、空状态、错误状态要可见。

7. 添加测试或验证：
   - IPC wrapper 单元测试或类型测试。
   - 核心组件渲染测试。
   - 至少人工验证：添加图库、扫描进度、时间线加载、视图切换、大图预览、缺失提示。

## 不做

- 不实现收藏。
- 不实现基础筛选 UI。
- 不实现地图视图。
- 不实现全文搜索。
- 不实现人脸识别或 AI 搜索。
- 不实现云同步或多用户。
- 不做营销 landing page；第一屏应是可用的图库体验。

## 验收标准

- 用户可以从空状态添加图库并看到扫描进度。
- 扫描完成后可以浏览时间线。
- 时间线支持瀑布流和网格两种视图。
- 年 / 月跳转可用。
- 大图预览可用，并展示基础 metadata。
- 缺失文件有明确提示和手动重定位入口。
- 前端不直接读取本地文件路径展示图片，只通过 asset 协议加载缩略图和预览。

## 建议验证

运行：

```bash
pnpm test
pnpm build
pnpm tauri dev
```

如果本地没有足够样例图库，需要创建一个小型测试图库验证主流程，并在 PR 说明中记录人工验证结果。
