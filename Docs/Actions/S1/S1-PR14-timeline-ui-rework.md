# S1-PR14 Timeline UI Rework

你是负责实现 Glance 的编码 Agent。当前 PR 的目标是重写时间线 UI，把虚拟滚动从 `VirtuosoGrid` 切到 `Virtuoso` 的 group API、接通 Lightbox、并消除每张缩略图独立 IPC 调用的性能问题。完成后用户可以流畅滚动一个万级图库并点开大图。

## 依赖

本 PR 基于：

- `S1-PR8` IPC 接通（含 `PhotoSummary.thumbnail_url` 字段）
- `S1-PR9` 缩略图能真实生成
- 评审结论 `S1-REVIEW.md` F1、F2、F3

开始前阅读：

- `Docs/glance_architecture.md` §2.2 前端模块、§4 Tauri Command 接口
- `Docs/glance_design_document.md` 5.1 时间线浏览
- react-virtuoso 文档的 `Virtuoso` + `groupCounts` / `groupContent` 用法
- `src/views/Timeline.tsx`、`src/views/Lightbox.tsx`、`src/components/ThumbnailGrid.tsx`

## 任务

1. 后端：`timeline_query` 直接返回缩略图 URL：
   - `PhotoSummary` 增加 `thumbnail_url: String`（PR8 已要求；本 PR 验证存在并被前端正确消费）。
   - 默认 tier=240 拼成 `asset://thumb/{photo_id}/240`。
   - 不再要求前端独立 invoke `thumbnail_url`。

2. 前端时间线重写：
   - 用 `Virtuoso`（不是 `VirtuosoGrid`）。
   - 数据预处理：把 `photos[]` 按日期分组，得到 `{ groups: string[], groupCounts: number[], flatPhotos: PhotoSummary[] }`。
     - 例：`groups = ["2024年5月3日", "2024年5月2日"]`，`groupCounts = [12, 7]`，`flatPhotos.length = 19`。
   - `<Virtuoso groupCounts={groupCounts} groupContent={index => <DateHeader>{groups[index]}</DateHeader>} itemContent={(index) => <ThumbnailItem photo={flatPhotos[index]} />} />`。
   - sticky date header 用 react-virtuoso 自带的 `groupContent` 行为。

3. 网格内排版：
   - 单张 thumbnail item 用 CSS grid / flex 实现"每行 N 列、按短边对齐"。
   - 网格模式：固定 N=8 列（或随窗口宽度响应式）。
   - 瀑布流模式：保持各张原始宽高比、按列高度分配。MVP 可以先只做"网格"，把"瀑布流"放到此 PR 内但允许只完成基础版本。
   - 不允许给每个 item 单独 `useEffect` 拉 URL；直接读 `photo.thumbnail_url`。

4. 接通 Lightbox：
   - `Timeline` 维护 `selectedPhotoId: number | null` 状态。
   - `ThumbnailItem` 点击调 `onPhotoClick(photo.id)`。
   - 选中时渲染 `<Lightbox photoId={selectedPhotoId} onClose={() => setSelectedPhotoId(null)} />`。
   - Lightbox 显示 `asset://thumb/{id}/1080`（大图档），不再尝试加载原始文件。
   - ESC 关闭、点背景关闭、键盘左右切换（左右切换在 PR 内可标 TODO，但需有占位实现）。

5. 滚动性能：
   - 视口外 item 卸载（react-virtuoso 默认行为，确认不破坏）。
   - 图片 `loading="lazy"` 保留。
   - 缩略图加载失败时不撑坏布局（用 `onError` 切换到占位元素）。

6. 视觉与可访问性（轻量）：
   - 日期 header 字号、padding 与现有 CSS 协调。
   - 缩略图聚焦态可见（键盘可达）。
   - missing 标记仍然出现在 thumbnail 角落。
   - 不做 i18n（中文 hardcode 即可）。

7. 测试：
   - 现有 TypeScript 类型检查通过。
   - 关键组件加单测可选（vitest + RTL，不强求）。
   - 人工验证：扫一个含 500+ 照片的目录，滚动到底无卡顿，sticky header 正常吸顶。

## 不做

- 不实现筛选 UI（推到 0.2）。
- 不实现地图视图。
- 不实现键盘左右快速翻图的完整逻辑（占位即可）。
- 不实现幻灯片自动播放。
- 不改后端命令签名（PR8 已定）。

## 验收标准

- 扫描一个 500+ 照片目录后：滚动流畅、日期分组 sticky、点缩略图弹出 Lightbox、ESC 关闭。
- 浏览器 devtools Network / IPC 面板：滚动过程中没有为每张缩略图额外发起 invoke。
- 缩略图 URL 来自 `photo.thumbnail_url`。
- 缺失文件缩略图位置显示占位 + missing badge。
- TypeScript build / lint 通过。

## 建议验证

```bash
pnpm lint
pnpm build
pnpm tauri dev
```

人工：滚动到底、滚回顶、点开/关闭 Lightbox、切换 grid / waterfall。
