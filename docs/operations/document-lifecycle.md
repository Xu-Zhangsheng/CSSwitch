# 文档治理合同 v1

状态：当前

适用范围：CSSwitch 仓库内的 Agent 规则、context、维护文档、临时计划与交接。

失效条件：仓库文档入口或权威目录发生结构性变化时复核；新增文档类型不能被本合同无歧义归类时修订。

本文回答“某类内容放哪里、何时读取、何时晋升或删除”。强制的 Agent 行为只在[文档治理规则](../../.agents/rules/documentation.md)维护；本页不复制安全、Git 或测试合同。

## 1. 一个事实，一份当前正文

当前权威正文只存在于一个位置。索引只回答“去哪里回答什么问题”，不能摘要出第二份合同。日期化证据可以支持当前合同，但不能自动覆盖合同。

已接受的 Draft Spec 必须把耐久结论提炼到对应的 architecture、features、operations 或 rules；如果结论只是一次验证，则进入 evidence / audit。提炼完成的同一变更中删除 Draft Spec。Git 历史就是被删除内容的历史入口，不另建 `archive/`、`old-docs/`、`deprecated-docs/` 等目录。

## 2. 类型与权威位置

| 类型 | 回答的问题 | 权威位置与寿命 |
|---|---|---|
| Rule | Agent 无论版本都必须怎样做？ | `.agents/rules/`；短、强制、跨版本稳定 |
| Context | 当前分支、版本、已知缺口在哪里？ | `.agents/context/`；日期化、使用前实时复核 |
| Architecture | 稳定边界、所有权、状态与失败链路怎样组成？ | `docs/architecture/`；按稳定问题和维护原因拆分 |
| Feature Contract | 用户可见行为、信任边界和非目标是什么？ | `docs/features/`；实现变化时同步 |
| Operation | 人和工具怎样开发、测试、验收、发布或恢复？ | `docs/operations/`；流程与门禁变化时同步 |
| Plan | 一个有界任务准备按什么顺序执行？ | 优先使用任务系统；必须落文件时放 `.agents/handoffs/<slug>.plan.md`，默认不进入 Git |
| Draft Spec | 一个尚未接受的设计候选是什么？ | 首次确需异步评审时才创建 `docs/drafts/`、跟踪例外和索引；接受后提炼并删除 |
| Handoff | 未完成任务从哪个精确 checkpoint 继续？ | `.agents/handoffs/<slug>.md`；默认不进入 Git，也不是事实源 |
| Decision / ADR | 为什么选择一个难以从当前合同反推的耐久取舍？ | 只有真实决策需要独立保留时才创建；本期不预建 `docs/decisions/` |
| Evidence / Audit | 某日期、版本、环境或 artifact 实际证明了什么？ | `docs/evidence/` 或 `docs/audits/`；范围固定，不升级为当前真相 |
| 兼容指针 | 旧链接的当前权威入口在哪里？ | 原路径上的最短指针；到达删除条件后移除 |

Decision / ADR 只有在“保留理由”本身具有长期维护价值、且放进架构正文会干扰当前合同阅读时才独立存在。首次出现这种需要时，再创建并索引 `docs/decisions/`；不能为了目录完整性预建空结构。

## 3. 最小元数据

索引和 Rule 不要求统一头表。其他文档只在字段能防止误用时使用以下最小集合：

- `状态`：不是显然的当前合同时填写，例如 `草拟`、`待评审`、`当前`、`过期待删除`、`兼容指针`。
- `适用范围`：对象、版本、平台、环境或任务边界不能从标题判断时填写。
- `最后复核`：包含会漂移的版本、runtime、release、外部项目或当前状态时填写日期；稳定概念文档不为凑字段而填写。
- `失效条件`：Plan、Draft Spec、Handoff、Context、兼容指针必须填写；稳定正文只有明确失效触发器时填写。
- `最后更新` / `最后有效评审`：只有使用默认天数期限的 Plan、Handoff / Draft Spec 必须填写 ISO 日期；不得依赖文件 mtime。

字段写在标题后的短行中，不引入 owner、review cadence、置信度等默认维护字段。Evidence / Audit 继续记录其本身需要的日期、目标 commit / artifact、环境和证据层，不用这四个字段替代证据合同。
必填字段或必填章节不能只写 `TODO`、`TBD`、`待定` 等可见占位词；占位不算已填写。

兼容指针在 H1 后必须连续声明 `状态：兼容指针`、
`当前权威入口：[说明](仓库内相对路径)` 和 `失效条件`。自动门禁只验证该入口是
仓库内、已维护、非兼容指针的 Markdown 正文（以及可选 anchor 存在）；它不能证明
新旧正文语义等价，也不能判断该正文仍是当前事实，这两项仍由评审承担。

## 4. Plan、Draft Spec 与 Handoff 生命周期

### Plan

仅在任务包含多个依赖步骤、需要 checkpoint 或需要把执行边界交给后来者时创建。Plan 至少记录目标、范围、当前 checkpoint、`最后更新` 和失效条件。

优先使用任务系统中的临时 Plan；只有需要跨窗口保留而 Handoff 又不足以表达步骤依赖时，才写 `.agents/handoffs/<slug>.plan.md`。它仍受 handoff 目录的忽略和清理规则约束。

以下任一发生即过期：任务完成或取消；目标基线、分支或需求改变导致步骤不再可靠；另一份计划明确取代它。过期时先把已产生的耐久事实提炼到权威正文或证据，再删除 Plan。未写明期限时，最后更新 30 天后必须重新确认或删除。

只有目标、范围、依赖步骤或 checkpoint 实质变化，且作者重新核对实时状态后，才刷新 `最后更新`；排版、措辞或 touch 文件不能刷新期限。

### Draft Spec

只有设计尚未接受、又确实需要跨会话或多人评审时才版本化。首次出现时同步创建 `docs/drafts/`、对应 `.gitignore` 跟踪例外和只回答问题的索引；没有具体 Draft Spec 时不创建这些结构。Draft Spec 必须标记 `草拟` 或 `待评审`，写明决策范围、未决项、接受者或接受条件、`最后有效评审`，以及失效条件。

接受不是永久保存 Draft Spec 的理由。接受后在同一治理变更中：

1. 将行为与边界提炼进 feature / architecture；
2. 将执行与门禁提炼进 operation / rule；
3. 将一次性验证结果放进 evidence / audit；
4. 更新索引；
5. 删除 Draft Spec。

被拒绝、被替代或 30 天没有有效评审进展的 Draft Spec 进入 `过期待删除`，不得继续作为实现依据；有耐久理由时先提炼为 Decision / ADR，否则直接删除。

只有具名评审者给出实质反馈、作出接受 / 拒绝决定，或作者提交针对该反馈的实质修订，才刷新 `最后有效评审`；仅改日期或整理格式不算评审进展。

### Handoff

只为尚未完成的跨窗口任务创建，内容限于精确 worktree / 分支 / checkpoint、保护边界、未完成验证、下一动作、禁止项、`最后更新` 和失效条件。它链接耐久事实，不复制规则、架构、发布历史或长测试日志。

以下最早发生者使 Handoff 失效：任务完成或取消；checkpoint 被新提交或新 handoff 替代；分支、worktree、需求或证据基线变化；最后更新 14 天后未刷新。恢复任务时先实时验证 status 与引用目标。失效后提炼新增耐久事实并删除 Handoff。

只有重新核对实时 status、引用目标和保护边界，并更新 checkpoint 或下一动作后，才刷新 `最后更新`。

## 5. 默认阅读预算

普通任务的首轮读取顺序和上限：

1. `AGENTS.md`、安全规则、Git / worktree 规则与实时 `git status`；
2. 零或一项最接近的领域 Rule；没有匹配项时不强行套用；
3. 一个最接近问题的索引；
4. 最多两份相关正文或 Context。

正式独立审查额外读取 `reviewing.md` overlay，但不增加一个索引、最多两份正文或 Context 的首轮预算。

仍无法回答时，沿正文链接一次展开一层，并说明需要补哪类证据。不得为了“先了解全部项目”预读所有 Context、Architecture、Audit 或 Evidence。

审计、事故、发布、兼容性核验可以读取对应 Evidence / Audit，但仍先限定版本、环境、功能面和证据层。探索某项架构时只读回答该稳定问题的文档；索引没有合适入口，才说明缺口，而不是先创建一批空文档。

## 6. 索引与变更闭环

- 总入口和分目录索引只写链接与“回答的问题”，不复制状态表、结论或操作步骤。
- 新建当前正文时更新最近一级索引；只有跨类别入口才更新 `docs/README.md`。
- 修改产品行为时同步 Feature Contract；改变边界、所有权或状态机时同步 Architecture；改变执行门禁时同步 Operation / Rule。
- 上游或 release 变化先落 Evidence / Audit，再判断是否改变当前合同；不得把一次观察直接写成稳定能力。
- 兼容指针必须给出当前权威入口和可判定的删除条件。到期后删除旧路径，并修复仓库内链接。
- 每次治理修改至少运行
  `python3 -m unittest test.test_document_governance -v` 与
  `git diff --check`，并确认没有越过任务允许的代码范围。该自动门禁只检查
  Git 已跟踪及未忽略候选中的 Markdown 相对链接与可渲染 anchor、Agent
  rule 与 Docs 当前正文的最近一级直接索引，以及标题后结构化的 Context /
  兼容指针 / 临时文档最小生命周期元数据。Markdown 只通过绑定仓库根目录、
  逐级拒绝符号链接的文件描述符读取；代码块、inline code 和 ignored handoff
  不进入相应 source gate 判定。通过不代表正文语义审查、产品测试或任何
  artifact / installed / live / signing / release 层通过。

拆分压力审计只使用以下四种结论：

- `keep`：内容回答同一个稳定问题，维护触发器和失效条件一致；行数大不是拆分理由。
- `shrink`：索引、兼容入口或当前正文复制了别处的权威内容；删除重复正文并改为回答问题的链接。
- `split`：文件同时维护至少两个可独立成立的稳定问题，且它们有不同维护原因或不同失效条件；新正文分别进入唯一权威位置。
- `delete-pointer`：兼容指针写明的删除条件已经满足；未满足时继续保留最短指针。

Audit / Evidence 可以因调查范围、时间线或证据链较长而较长；只要仍绑定同一日期、版本、环境或 artifact，就不因长度拆分。拆分不能制造第二套 owner、机制、support 或证据权威。

## 7. 当前骨架的轻量分类

现有 `.agents/rules/`、`.agents/context/`、`docs/architecture/`、`docs/features/`、`docs/operations/`、`docs/evidence/` 和 `docs/audits/` 已覆盖主要类型。本期只补生命周期合同，不批量迁移或按文件改写元数据。

当前骨架的收口边界：

- 架构索引已按总览、Desktop 控制面、状态事务、Gateway/provider 路由、Science runtime、Science 能力依赖与 Skill/MCP/Plugin 扩展控制面 7 个稳定问题完成路由；后续机器地图属于独立任务，不在本合同预建正文。
- 现有一个旧 Codex implementation-plan 路径只保留兼容指针，不作为 Plan 或权威正文。
- Decision / ADR、可版本化 Draft Spec 尚无真实实例，因此本期不创建目录。
