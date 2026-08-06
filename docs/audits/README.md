# 日期化审计索引

本目录保留绑定特定日期、SHA、环境或证据层的审计原文。当前问题与 sole NEXT 只从 [known issues](../../.agents/context/known-issues.md) 进入；审计不会因较新索引出现而升级为当前事实。

## 最新基线与完成证据

- [O1-E4 history full-snapshot durable effect owner（2026-08-06）](2026-08-06-o1-e4-history-full-snapshot-effect-owner.md)：history live/fresh 唯一跨进程 effect owner、durable full-manifest restore outcome、独立复审与 exact-SHA source gate closure。
- [O1-E3 fresh-process durable compensation replay owner（2026-08-06）](2026-08-06-o1-e3-fresh-compensation-replay.md)：唯一跨进程 replay effect owner、durable exact compensation replay、独立复审与 exact-SHA source gate closure。
- [O1-E2 durable compensation step-state foundation（2026-08-06）](2026-08-06-o1-e2-compensation-step-state.md)：五步 typed intent / outcome、完整记录 CAS、authority boundary 与 exact-SHA source gate closure。
- [O1-E1 durable compensation journal foundation（2026-08-05）](2026-08-05-o1-e1-durable-compensation-journal.md)：path-free aggregate compensation journal、normal mutation guard、独立复审与 exact-SHA source gate closure。
- [F1-R unified read model closure（2026-08-05）](2026-08-05-f1-r-read-model.md)：只读 config path、显式 notice ack、统一 sequenced boot publication、独立审查与 exact-SHA source gate closure。
- [O1-D aggregate compensation phase closure（2026-08-05）](2026-08-05-o1-d-compensation-phase.md)：aggregate compensation phase 机械切分、独立复审与 exact-SHA source gate closure。
- [O1-C managed Science launch phase closure（2026-08-05）](2026-08-05-o1-c-science-launch-phase.md)：managed Science launch phase 机械切分、独立复审与 exact-SHA source gate closure。
- [O1-B cold one-click coordinator closure（2026-08-05）](2026-08-05-o1-b-cold-coordinator.md)：entry / mutating cold coordinator 机械切分、独立审查与 exact-SHA source gate closure。
- [F1-A durable history recovery closure（2026-08-05）](2026-08-05-f1-a-history-recovery.md)：typed V2 history transaction、credential before-image / crash replay、restore/resume handoff、独立审查与 exact-SHA source gate closure。
- [C1-A cross-process config writer fence closure（2026-08-05）](2026-08-05-c1-a-config-writer-fence.md)：canonical config writer fence、真实双进程/锁 identity 负向测试、独立审查与 exact-SHA source gate closure。
- [O1-A typed runtime entry owner closure（2026-08-04）](2026-08-04-o1-a-runtime-entry-owner.md)：唯一 production runtime entry owner、typed recovery / route ordering、独立审查与 exact-SHA source gate closure。
- [F1-0 history boundary guard closure（2026-08-04）](2026-08-04-f1-0-history-boundary-guard.md)：restore 成功后保持 stopped、显式后续启动、独立审查与 exact-SHA source gate closure。
- [Post-Q0 Runtime 后续路线再基线（2026-08-04）](2026-08-04-post-q0-runtime-roadmap-rebaseline.md)：F1-0 前的 source-only 优先级与依赖输入；首阶段已完成，后续顺序不再自动有效。
- [Q0-A source-candidate lineage closure（2026-08-04）](2026-08-04-q0-a-source-candidate-lineage.md)：v0.8.4 / `next` lineage、immutable source record、exact-C review 与 source gate closure。
- [Post-D0 只读重新基线（2026-08-04）](2026-08-04-post-d0-rebaseline.md)：Q0-A 前的 source-only 架构与治理输入。
- [D0 Doctor intent split source closure（2026-08-04）](2026-08-04-d0-doctor-intent-split.md)：D0 implementation candidate、review 与 source gate 证据。

## Runtime 重构证据链

- [Post-H4 全量重构摸排](2026-08-04-post-h4-full-refactor-reconnaissance.md)与[窄 production flow 再基线](2026-08-04-post-h4-production-flow-rebaseline.md)：D0 前的全量与窄范围输入。
- [H1–H3 后 production flow](2026-08-03-post-h1-h3-production-flow-rebaseline.md)与[Runtime 事务编排](2026-08-03-runtime-transaction-orchestration-rebaseline.md)：H1–H3、S6 后的日期化重查。
- [Post-R2 运行架构](2026-08-02-post-r2-runtime-rebaseline.md)、[`next` 运行架构](2026-07-31-next-runtime-architecture-rebaseline.md)与[工程重构后基线](2026-07-31-v084-post-refactor-baseline.md)：较早的 source-only closure 与结构基线。

## 治理与版本历史

- [v0.8.4 文档结构审计](2026-07-30-v084-document-structure.md)、[架构与 Science 边界调研](2026-07-30-v084-architecture-reconnaissance.md)与[主干基线](2026-07-29-v084-main-baseline.md)：治理起点和当时的能力边界。
- 旧版本 change / test-system audit：[v0.8.0](v080-change-audit.md)、[v0.8.1](v081-change-audit.md)、[v0.8.2](v082-change-audit.md)、[v0.8.3](v083-test-system-audit.md)。

发布 artifact 与分发结果从[发布证据索引](../evidence/releases/README.md)进入；日期化 runtime、上游和事故调查从[调查索引](../evidence/investigations/README.md)进入。
