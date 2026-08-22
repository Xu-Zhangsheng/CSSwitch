# Agent 稳定规则索引

所有任务先读[安全](safety.md)与 [Git / worktree](git-worktrees.md)。除正式独立审查外，再按任务选择零或一项最接近的领域规则；没有匹配项时走普通工程 / 探索 fallback，直接从[文档总入口](../../docs/README.md)选择最接近的 architecture、feature 或 operation，不强行套用专用规则。需要判定结果时再进入 quality / test / gate；Audit / evidence 只在核验版本、artifact、runtime、事故或历史结论时展开。

| 任务类型 | 专用规则 | 当前合同入口 | 质量 / 测试 / Gate | 何时展开 Audit / Evidence |
|---|---|---|---|---|
| 文档创建、修改、引用、拆分或删除 | [文档治理](documentation.md) | 对应 [architecture](../../docs/architecture/README.md)、[feature](../../docs/features/README.md) 或 [operation](../../docs/operations/README.md) | 文档链接、索引、生命周期与 changed-path 检查 | 需要判断历史迁移、版本或一次验证结论时 |
| 跨语言 source 实现、边界或风险判断 | [代码作者](code-authoring.md) | 对应 [architecture](../../docs/architecture/README.md)、[feature](../../docs/features/README.md) 或 [operation](../../docs/operations/README.md) | 按风险选择 owning check；完整 closure 只走唯一 source gate | 结论依赖 artifact、runtime 或历史证据层时 |
| 普通 feature 实现、一般 bugfix、架构探索或未列出的诊断 | 无额外领域 Rule | 选择最接近的 architecture / feature / operation | 按改动与结论进入相应 test / gate | 结论依赖日期、版本、环境或 artifact 时 |
| 自动测试与结论 | [测试与证据](testing-and-evidence.md) | 被测行为对应的 architecture / feature，执行方式对应 operation | [自动测试](../../docs/operations/testing.md)与已登记 quality / gate | 解释特定 run、artifact、installed/live 或 release 结果时 |
| 构建 / 发布 | [发布](release.md) | [发布流程](../../docs/operations/release.md)及相关 feature / architecture | source、artifact、installed/live、signing 与 public gate 分层 | 每次候选与公开发布都必须绑定对应 evidence |
| Science 启动、升级或兼容性 | [Science runtime](science-runtime.md) | [Science runtime](../../docs/architecture/science-runtime.md)与相关 feature | 自动测试或[真机验收](../../docs/operations/real-machine-acceptance.md) | 涉及上游版本、package、账号或 live 行为时 |
| 外部 Skill | [外部 Skills](external-skills.md) | [外部 Skill bridge](../../docs/features/external-skill-bridge.md) | source gate；领域执行或重启持久化另行验收 | 涉及指定 package、Science 版本或 live 能力时 |
| 系统 SSH | [系统 SSH](system-ssh.md) | [系统 SSH 功能合同](../../docs/features/system-ssh.md) | parser、OpenSSH invocation、real server 三道 gate | 涉及事故、真实 server 或版本化结果时 |
| 正式独立审查 | [独立审查](reviewing.md) overlay，及零或一项被审领域规则 | 被审候选对应的 architecture / feature / operation | 使用被审范围声明的 quality / test / gate | 只有结论依赖日期化证据时沿链接展开 |

rules 只保存跨版本稳定的 Agent 行为约束，不写 commit、worktree 数量、一次测试结果或临时下一步。
