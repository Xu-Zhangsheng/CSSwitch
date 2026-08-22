# CSSwitch Agent 规则入口

本文件只负责索引，不保存版本快照、测试结果或架构正文。

## 阅读顺序

1. 所有任务先读[安全规则](.agents/rules/safety.md)和[Git / worktree 规则](.agents/rules/git-worktrees.md)。
2. 再按任务读取 [`.agents/rules/`](.agents/rules/) 下零或一项最接近的领域规则；没有匹配项时不强行套用。涉及文档创建、修改、引用或删除时读[文档治理规则](.agents/rules/documentation.md)。
3. 涉及版本、分支、worktree、验证状态或已知问题时，先实时复核，再参考 [`.agents/context/`](.agents/context/) 的日期化快照。
4. 架构、运维、功能合同、历史证据和外部参考从[文档总入口](docs/README.md)进入。

默认首轮只加载解决任务所需的一个索引和最多两份相关正文或 Context；需要更多证据时沿链接逐级展开，不预读全部 context、audit 或 evidence。正式独立审查额外读取 [`reviewing.md`](.agents/rules/reviewing.md) overlay，但不增加领域规则、索引、正文或 Context 的预算。

## 信息权威顺序

目标 artifact / runtime > tag 与公开 release > 当前源码和测试 > `.agents/context/` > `docs/` > memory / handoff。

`.agents/rules/` 是 Agent 的稳定行为规则；`.agents/context/` 只是可更新的当前状态索引；临时交接不得替代两者。

## 工作边界

- 未提交内容按用户数据保护；只读任务只允许检查和报告。
- commit、push、tag、release、替换已安装 App或运行真实 provider 测试均需明确授权。
- 真实 API Key、OAuth token、Keychain、SSH 私钥和账号数据库始终不得读取或回显；真实 provider 测试只能消费用户显式提供给隔离进程的输入，授权测试不等于授权检查凭证内容。
- 临时 handoff 放入 `.agents/handoffs/`，长期事实应进入 rules、context 或 docs。

## 收口纪律

- 同一个有界目标或候选的正式独立审查最多两轮：首轮集中发现问题，修复合并完成后只做一轮最终复审；不得因逐项修复反复启动新的 reviewer。详细规则见 [`reviewing.md`](.agents/rules/reviewing.md)。
- WIP 阶段不因每次小改动反复旋转或核对测试身份 hash；先完成目标内改动并冻结候选，再统一更新一次、验证一次。只有冻结后相关源文件再次变化或 validator 报告具体不一致时才重做。
- 始终围绕用户当前目标和预先声明的收口清单工作。目标外发现只有直接阻断当前结果的 P0 / P1 才可纳入本轮；其余记录并延后，不得演变成无上限加固或审查循环。
