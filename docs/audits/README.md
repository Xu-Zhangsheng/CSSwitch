# 日期化审计索引

本目录保留绑定特定日期、SHA、环境或证据层的审计原文。当前问题与 sole NEXT 只从 [known issues](../../.agents/context/known-issues.md) 进入；审计不会因较新索引出现而升级为当前事实。

## 最新基线与完成证据

- [Q0-A source-candidate lineage closure（2026-08-04）](2026-08-04-q0-a-source-candidate-lineage.md)：v0.8.4 / `next` lineage、immutable source record、exact-C review 与 source gate closure。
- [Post-D0 只读重新基线（2026-08-04）](2026-08-04-post-d0-rebaseline.md)：当前最新的 source-only 架构与治理再基线。
- [D0 Doctor intent split source closure（2026-08-04）](2026-08-04-d0-doctor-intent-split.md)：D0 implementation candidate、review 与 source gate 证据。

## Runtime 重构证据链

- [Post-H4 全量重构摸排](2026-08-04-post-h4-full-refactor-reconnaissance.md)与[窄 production flow 再基线](2026-08-04-post-h4-production-flow-rebaseline.md)：D0 前的全量与窄范围输入。
- [H1–H3 后 production flow](2026-08-03-post-h1-h3-production-flow-rebaseline.md)与[Runtime 事务编排](2026-08-03-runtime-transaction-orchestration-rebaseline.md)：H1–H3、S6 后的日期化重查。
- [Post-R2 运行架构](2026-08-02-post-r2-runtime-rebaseline.md)、[`next` 运行架构](2026-07-31-next-runtime-architecture-rebaseline.md)与[工程重构后基线](2026-07-31-v084-post-refactor-baseline.md)：较早的 source-only closure 与结构基线。

## 治理与版本历史

- [v0.8.4 文档结构审计](2026-07-30-v084-document-structure.md)、[架构与 Science 边界调研](2026-07-30-v084-architecture-reconnaissance.md)与[主干基线](2026-07-29-v084-main-baseline.md)：治理起点和当时的能力边界。
- 旧版本 change / test-system audit：[v0.8.0](v080-change-audit.md)、[v0.8.1](v081-change-audit.md)、[v0.8.2](v082-change-audit.md)、[v0.8.3](v083-test-system-audit.md)。

发布 artifact 与分发结果从[发布证据索引](../evidence/releases/README.md)进入；日期化 runtime、上游和事故调查从[调查索引](../evidence/investigations/README.md)进入。
