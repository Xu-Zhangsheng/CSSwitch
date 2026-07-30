# 分支线角色（工程约定）

最后复核：2026-07-30（Asia/Shanghai）

失效条件：`main` / `next` / 发布 tag 的角色约定变更时立即失效。

| 线 | 角色 | 当前约定 |
|---|---|---|
| `v0.8.4` tag | 发布身份（冻结） | 不移动；对外 artifact 只认 tag |
| `main` | 干净基线 = v0.8.4 源码 + 文档治理 | 默认不收大重构；发布对齐 |
| `next` | 工程直线 | 切片 FF 合入；可逐刀 revert；**不是**当前 release 身份 |
| `codex/<slice>` | 单刀工作分支 | 从 `next` 开出，审完 FF 回 `next` |

当前 `next` 相对 `main` 的首段工程：Runtime/Gateway environment allowlist（原分支 `codex/env-allowlist-runtime-gateway`）。

历史治理/调查分支与 worktree 多数已被 `main` 吸收，默认不必继续使用；删除前须用户明确授权。
