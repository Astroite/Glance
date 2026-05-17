# Actions

`Docs/Actions` 用来把设计文档拆成可执行的阶段任务。这里的文档不是产品说明，也不是长期架构设计，而是给编码 Agent 使用的执行提示词。

每个阶段使用独立目录：

```text
Docs/Actions/
├── README.md
└── S1/
    ├── S1-README.md
    ├── S1-PR1-project-scaffold.md
    ├── S1-PR2-storage-schema.md
    └── ...
```

## 编号规则

- 阶段编号使用 `S{number}`，例如 `S1`、`S2`、`S3`。
- 每个阶段目录内必须有一个 `{stage}-README.md`，说明本阶段目标、前置条件、PR 顺序和共同约束。
- 阶段内的具体任务使用 `{stage}-PR{number}-{topic}.md` 命名。
- 一个 `PR` 文件应对应一个可独立评审、可独立合并的工作单元。

## 提示词要求

每个 PR 提示词应包含：

- 当前 PR 的目标。
- 依赖哪些前置 PR 或阶段成果。
- Agent 开工前必须阅读的设计文档。
- 需要完成的具体任务。
- 明确的不做事项。
- 验收标准。
- 建议验证命令或人工验证方式。

提示词应直接面向执行者写，不要只写抽象计划。好的提示词应该能让 Agent 在不反复追问的情况下开始实现，并且不会越界到后续阶段。

## 使用方式

1. 先阅读对应阶段的 `{stage}-README.md`。
2. 按阶段 README 中列出的顺序执行 PR。
3. 执行某个 PR 前，复制或引用对应的 `{stage}-PRx-*.md` 作为 Agent 的任务提示。
4. PR 完成后，根据实际实现结果更新相关设计文档或后续 Action 文档。

## 当前阶段

当前执行阶段是：

- `S1/`：从空仓库启动 MVP 工程基础，拆分为工程骨架、SQLite schema、扫描身份、EXIF/XMP、扫描编排、缩略图管线、时间线 UI 等多个 PR。

入口文件：

- `S1/S1-README.md`

## 维护原则

- Actions 必须服从 `Docs/glance_design_document.md` 和 `Docs/glance_architecture.md`。
- 如果执行中发现设计需要调整，先更新设计文档，再更新 Actions。
- 不要在 Action 文档中引入与设计文档冲突的新决策。
- 已完成阶段可以保留作为历史记录；新阶段应新建目录，不覆盖旧阶段。
