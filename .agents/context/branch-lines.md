# 分支线角色（工程约定）

最后复核：2026-08-03（Asia/Taipei）

失效条件：`main` / `next` / 发布 tag 的角色约定变更时立即失效。

| 线 | 角色 | 当前约定 |
|---|---|---|
| `v0.8.4` tag | 发布身份（冻结） | 不移动；对外 artifact 只认 tag |
| `main` | 干净基线 = v0.8.4 源码 + 文档治理 | 默认不收大重构；发布对齐 |
| `next` | 工程直线 | 切片 FF 合入；可逐刀 revert；**不是**当前 release 身份 |
| `codex/<slice>` | 单刀工作分支 | 从 `next` 开出，审完 FF 回 `next` |

当前 `next` 已推进到 `9a2d65ff1b7996b673f6901df97ac899717b8a44` 的 S6 source closure；
它包含 environment allowlist 之后的 R0-R2 与 S1-S6 工程切片，仍不是 release 身份。当前
implementation 顺序必须先参考 [2026-08-03 Runtime 事务编排再基线](../../docs/audits/2026-08-03-runtime-transaction-orchestration-rebaseline.md)，不得只按历史首段或旧 S7 路线推断。

历史治理/调查分支与 worktree 多数已被 `main` 吸收，默认不必继续使用；删除前须用户明确授权。
