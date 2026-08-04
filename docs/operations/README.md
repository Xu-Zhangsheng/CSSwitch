# 运维文档

- [文档治理合同](document-lifecycle.md)：文档类型、默认阅读预算，以及 Plan / Draft Spec / Handoff 的创建、晋升、过期和删除条件。
- [开发](development.md)：回答本地启动、组件检查、隔离 runtime 和文档维护怎样执行。
- [自动测试](testing.md)：回答测试入口、结果词汇和各证据层不能互相外推什么。
- [Quality kernel](quality-kernel.md)：回答 `quality/` 机器事实源、source claim 与变化登记怎样分界。
- [真机验收](real-machine-acceptance.md)：回答隔离护栏、准备步骤、验收矩阵和收尾怎样执行。
- [Claude Science 0.1.25 探针规格](science-probe-spec.md)：回答 A/B/C 探针的 fixture、授权、gate、判定、停止、清理和证据输出；actual status 只从日期化 evidence 进入。
- [发布](release.md)：回答 source、artifact、installed/runtime、分发与回读怎样逐层关闭。
- [升级与回滚](upgrade-and-rollback.md)：回答当前 v0.8.4 的升级、覆盖安装、v4 内回滚与旧 schema 降级步骤。

安全、Git / worktree 与证据用语属于 [Agent 规则](../../.agents/rules/)，这里描述人和工具如何执行维护流程。
