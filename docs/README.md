# CSSwitch 文档总入口

公开产品概览从根目录 [README 中文版](../README.md) / [English](../README.en.md) 进入。本页只路由当前权威正文、当前状态和最新日期化基线，不展开历史审计结论。

## 当前权威入口

- [当前已知问题与证据缺口](../.agents/context/known-issues.md)：当前决策门、仍有效的问题和证据边界；使用前先复核实时 Git / artifact / runtime。
- [架构索引](architecture/README.md)：稳定边界、所有权、状态、数据流和失败链路。
- [功能合同索引](features/README.md)：用户可见行为、能力边界、信任边界和非目标。
- [运维索引](operations/README.md)：开发、测试、质量、真机验收、发布、升级回滚和文档治理。
- [证据索引](evidence/README.md)：按 release 或日期化调查查找明确受限的证据。
- [外部参考索引](references/README.md)：固定 reviewed commit 的外部项目参考，不作为 CSSwitch 当前事实或代码来源。

Agent 强制行为从 [AGENTS.md](../AGENTS.md) 和 [`.agents/rules/`](../.agents/rules/) 进入；索引和兼容指针不复制正文。

## 最新基线

- [2026-08-04 Post-Q0 Runtime 后续路线再基线](audits/2026-08-04-post-q0-runtime-roadmap-rebaseline.md)：当前 source-only 优先级、唯一建议实施目标与后续依赖顺序；每阶段仍需单独授权。
- [2026-08-04 Q0-A source-candidate lineage closure](audits/2026-08-04-q0-a-source-candidate-lineage.md)：最近 source-only closure；其后的实施优先级由上方 Post-Q0 再基线重新判断。
- [2026-08-04 Post-D0 只读重新基线](audits/2026-08-04-post-d0-rebaseline.md)：Q0-A 前的 source-only 架构与治理输入；后续决策以实时复核后的 [known issues](../.agents/context/known-issues.md) 为准。
- [2026-08-04 D0 Doctor intent split source closure](audits/2026-08-04-d0-doctor-intent-split.md)：最近 implementation closure 的 focused、独立审查与 exact-SHA source gate 证据。
- [v0.8.4 发布证据](evidence/releases/v0.8.4.md)：分开记录 source、artifact、installed identity、signing 与 public 层；未列层不得补写为 PASS。

这些文档不能互相升级证据层，也不替代目标 artifact、installed runtime 或公开 Release 的实时核验。

## 历史审计

- [审计索引](audits/README.md)：日期化基线、source closure、文档治理和旧版本 change audit。原文保留其绑定日期、SHA、环境和证据边界，不作为当前真相。

## 维护约定

文档类型、权威位置、临时内容删除条件和阅读预算以[文档治理合同](operations/document-lifecycle.md)为准。一个事实只维护一份当前权威正文；Git 保存被删除临时文档的历史，不新建 `archive/` 或 `old-docs/`。
