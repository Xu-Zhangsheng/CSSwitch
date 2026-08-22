# CSSwitch 文档总入口

公开产品概览从根目录 [README 中文版](../README.md) / [English](../README.en.md) 进入。本页只路由当前权威正文、当前状态和日期化证据，不展开历史审计结论。

## 当前权威入口

- [当前重构路线与证据缺口](../.agents/context/known-issues.md)：唯一当前 NEXT、阶段完成条件、仍有效的问题和证据边界；使用前先复核实时 Git / artifact / runtime。
- [架构索引](architecture/README.md)：稳定边界、所有权、状态、数据流和失败链路。
- [功能合同索引](features/README.md)：用户可见行为、能力边界、信任边界和非目标。
- [运维索引](operations/README.md)：开发、测试、质量、生产链路验收、发布、升级回滚和文档治理。
- [证据索引](evidence/README.md)：按 release 或日期化调查查找明确受限的证据。
- [外部参考索引](references/README.md)：固定 reviewed commit 的外部项目参考，不作为 CSSwitch 当前事实或代码来源。

Agent 强制行为从 [AGENTS.md](../AGENTS.md) 和 [`.agents/rules/`](../.agents/rules/) 进入；索引和兼容指针不复制正文。

## 当前验收入口

- [生产链路验收](operations/real-machine-acceptance.md)：唯一维护“重要重构决策 → production source → exact artifact → isolated-live → authorized live”的映射、进入条件、授权和故障 fixture 边界。
- [Science 探针合同](operations/science-probe-spec.md)：在生产链路映射下维护 Science 的 source、exact artifact、isolated-live 与逐项 authorized-live probe card，不记录 actual result。
- [v0.8.4 发布证据](evidence/releases/v0.8.4.md)：只记录该 release 绑定的 source、artifact、installed identity、signing 与 public 层；未列层不得补写为 PASS。

旧 R3–R11、R4/R5、S7 与 Post-D0/Post-Q0 路线已全部退役，只能从历史审计查证当时的决定和证据。后来 audit/evidence 中的 Phase 1/2/5、O1、F1 等编号只标识其绑定 slice，不是当前 NEXT、实施授权或验收顺序。

## 历史审计

- [审计索引](audits/README.md)：日期化基线、source closure、旧阶段路线、文档治理和旧版本 change audit。原文保留其绑定日期、SHA、环境和证据边界，不作为当前真相或后续授权。

## 维护约定

文档类型、权威位置、临时内容删除条件和阅读预算以[文档治理合同](operations/document-lifecycle.md)为准。一个事实只维护一份当前权威正文；Git 保存被删除临时文档的历史，不新建 `archive/` 或 `old-docs/`。
