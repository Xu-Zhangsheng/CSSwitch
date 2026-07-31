# CSSwitch 文档总入口

当前主干及 Git、Release、源码、测试、artifact 与 runtime 的分层定位见[主干基线审计](audits/2026-07-29-v084-main-baseline.md)。公开产品概览从根目录 [README 中文版](../README.md) / [English](../README.en.md) 进入；本目录按内容寿命和证据类型分类。

## 架构

- [架构索引](architecture/README.md)：按稳定问题选择当前架构正文。
- [架构总览](architecture/overview.md)：边界、所有权、数据流和失败边界。
- [Science runtime](architecture/science-runtime.md)：稳定的 binary、data-dir、身份和网络合同。
- [Desktop 控制面](architecture/desktop-control-plane.md)：回答 Tauri command/event/DTO 与 frontend caller 如何连接。
- [运行时状态与事务](architecture/runtime-state-transactions.md)：回答状态所有权、锁序、journal/snapshot/receipt 与补偿链。
- [Gateway 与 provider 路由](architecture/gateway-provider-routing.md)：回答正式/scratch Gateway、Codex、Skill stdio 与 Science control 怎样分界。
- [Claude Science 能力依赖](architecture/science-capability-dependencies.md)：解释稳定的跨 owner 依赖、第三方最小托管责任、窄 bridge、non-target 类别和故障归因。

## 运维

- [运维索引](operations/README.md)：按开发、测试、质量、验收、发布或文档治理问题选择执行合同。
- [文档治理合同](operations/document-lifecycle.md)：回答文档类型、阅读预算、临时内容生命周期和权威正文如何收敛。
- [开发](operations/development.md)
- [自动测试](operations/testing.md)
- [真机验收](operations/real-machine-acceptance.md)
- [Claude Science 0.1.25 探针规格](operations/science-probe-spec.md)：只冻结 A/B/C 探针的执行与判定合同，不记录运行结果。
- [发布](operations/release.md)
- [升级与回滚](operations/upgrade-and-rollback.md)
- [Quality kernel](operations/quality-kernel.md)：机器事实源、变化登记、source-green 与证据层边界。

## 功能合同

- [功能合同索引](features/README.md)：按用户入口或功能边界选择当前合同。
- [产品 / Claude Science 能力地图](features/product-science-capability-map.md)：维护唯一的逐能力决策表，包括 Science 官方 owner、第三方模式处理、non-target、当前证据层与 `UNKNOWN`。
- [外部 Skill bridge](features/external-skill-bridge.md)
- [系统 SSH 配置复用](features/system-ssh.md)
- [UI 信息架构](features/ui-information-architecture.md)：模型连接、Codex 设置和扩展能力的页面职责。

## 证据

- [证据索引](evidence/README.md)：按发布证据或日期化调查进入对应二级索引。
- [2026-07-31 工程重构后基线](audits/2026-07-31-v084-post-refactor-baseline.md)：绑定本地 `next` 的最终工程合并 SHA，记录职责映射、source-test 证据和未验证边界。
- [2026-07-30 文档结构与拆分审计](audits/2026-07-30-v084-document-structure.md)：记录全库 Markdown 的 keep / shrink / split / delete-pointer 结论及本期验证。
- [2026-07-30 架构与 Science 边界调研](audits/2026-07-30-v084-architecture-reconnaissance.md)：固定 exact HEAD、源码/官方资料、架构 findings、UNKNOWN 与 A/B/C 探针队列。
- [2026-07-31 `next` 运行架构再基线](audits/2026-07-31-next-runtime-architecture-rebaseline.md)：固定机械拆分完成后的 exact `next`，还原运行拓扑、状态/补偿边界、逻辑耦合、目标候选与有序迁移切片。
- [v0.8.3 测试系统审计](audits/v083-test-system-audit.md)：记录当时的 BLOCK、入口清单与证据分层，不代表当前 gate 结果。
- [v0.8.2 变更审计](audits/v082-change-audit.md)、[v0.8.1](audits/v081-change-audit.md)、[v0.8.0](audits/v080-change-audit.md)：回答各版本候选的日期化变更审查。
- [发布证据](evidence/releases/README.md)：按版本记录最终 artifact 与分发结果。
- [日期化调查](evidence/investigations/README.md)：只证明特定日期、runtime 和环境。

## 外部参考

- [外部参考索引](references/README.md)：区分当前外部参考与兼容路径。
- [外部项目参考](references/external/README.md)：记录 reviewed commit 与可借鉴边界，不作为代码来源。

## 维护约定

- 文档类型、最小元数据、临时内容删除条件与默认阅读预算以[文档治理合同](operations/document-lifecycle.md)为准。
- Agent 强制行为从 [`.agents/rules/`](../.agents/rules/) 进入；索引和兼容指针不复制正文。
