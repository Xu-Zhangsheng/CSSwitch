# Post-Q0 Runtime 后续路线再基线

状态：当前日期化再基线

适用范围：`next@d4362632267a020de18f9b241a3a271d625b2c38`；source-only、只读规划

最后复核：2026-08-04（Asia/Taipei）

失效条件：history restore、runtime entry、config transaction、boot read model、Science adoption 或 release qualification 的相关源码、测试、权威合同发生实质变化时，受影响结论立即失效。

本文记录 Q0-A 后基于当前源码重新组织的实施顺序。它回答“下一阶段先做什么、后续依赖怎样排”，不建立 implementation、artifact、installed/live、provider、signing 或 public release PASS，也不授权自动进入任何阶段。

## 结论

当前唯一建议实施目标是 **F1-0：切断 history restore 成功后的前端自动第二次 destructive IPC**。restore 成功后明确保持 stopped，由用户再次显式启动。

F1-0 只先关闭直接产品暴露面，不把 restore 与 one-click 假装成原子事务。完整 history transaction 必须等待 runtime entry owner 与 typed config transaction 基础收敛后再做。

## 当前源码事实

- frontend 在 `restore_history_choice` 成功后自动调用 `one_click_login`；两个调用各自拥有 destructive operation 边界，当前没有共同 durable journal；
- backend history restore 已按 exact managed identity 停止当前 Science、恢复所选历史并返回 stopped 结果；
- runtime entry decision 仍晚于部分 SSH、stub 与 pending-cleanup effect，command 与 runtime 的入口所有权尚未收敛；
- `get_config` 仍会消费并清除 pending notice，boot event 与补读没有统一 sequence；
- config 更新只有进程内串行化与提交前复核，不是跨进程共享 writer fence；
- Science update 已有 identity、snapshot 与 rollback guard，但没有通用 predecessor/candidate/adoption ledger。

这些事实的稳定架构语义继续由[运行时状态与事务](../architecture/runtime-state-transactions.md)维护；本文只保存绑定当前 SHA 的优先级判断。

## 后续执行路径

| 顺序 | 阶段 | 目标 | 进入下一阶段前的停止条件 |
|---|---|---|---|
| 1 | F1-0 history boundary guard | 删除 restore 成功后的自动 `one_click_login`；UI 明确“已恢复到停止态，需要再次启动” | 一个用户动作只产生一个 restore IPC；既有 backend restore 安全不变量不变 |
| 2 | O1-A typed entry owner | 用 immutable facts、pure decision 与唯一 runtime entry façade 收拢 healthy、cold、recovery、attention/manual 入口 | decision 前无受保护 effect；每次 recovery effect 后重新采集 facts 再决策 |
| 3 | C1 typed transaction foundation | 建立 typed commit outcome、complete-record CAS、跨进程 writer fence 与跨文件 crash 边界 | drift/rollback uncertainty 不再靠文案控制流；所有相关 writer 有可检查 owner |
| 4 | F1-A durable history recovery | 由 backend 拥有 restore、credential publication、snapshot cleanup 与可选 resume handoff | 默认“仅恢复”安全结束；显式“恢复并继续”由一次 backend operation 和 durable record 驱动 |
| 5 | F1-R read model / boot | `get_config` 纯读，notice 显式幂等 ack，boot 使用带 sequence 的 snapshot + event | 读取不改变 config；丢 event 可由同 sequence snapshot 补读 |
| 6 | O1-B / O1-C runtime decomposition | 行为保持地拆 cold coordinator，再处理锁外 stop 与 durable compensation replay | owner、锁序、补偿 precondition 和 crash replay 均有 typed 测试 |
| 7 | U1 Science adoption provenance | 增加 allowlisted predecessor/candidate/adoption ledger | 不读取真实账号数据，不因新 candidate 强制打断 healthy daemon |
| 8 | Q0-B release qualification | 重新基线 pre-public qualification、release action 与 public readback evidence | 必须在具体发布候选出现后单独设计、验证和授权；当前不冻结字段级协议 |

阶段编号表达依赖顺序，不构成批量实施授权。每阶段完成后都必须停止、清洁工作区并重新基线；后续阶段可以因新证据重排或删除。

## 阶段门禁

每个 implementation 阶段独立执行：focused tests、绑定 exact SHA 的 15-suite source gate、fresh clean-context 独立审查、仅归属本阶段的文档与代码变更。修复会产生新 candidate，并重新走 gate 与审查。

commit、merge、push、tag、release、真实 provider、真实 Science、artifact、installed/live、签名与公证仍分别授权和分别记证据。source PASS 不外推任何后续层。

## 本次证据边界

- worktree：`next`，复核时 clean；
- HEAD：`d4362632267a020de18f9b241a3a271d625b2c38`；
- 执行：只读源码、测试合同、架构与治理检查，并行子 Agent 摸排与独立路线审查；
- 未执行：source gate、artifact build、installed/live、真实 provider、真实 Science、签名、公证与 public release；
- 独立审查收敛点：F1-0 可独立止损，完整 F1-A 不应早于 O1-A 与 C1；过早冻结跨文件事务和 release ledger 的字段级协议会制造未经实现验证的假闭环，因此留到对应阶段重新基线。
