# 分支线角色（工程约定）

最后复核：2026-08-04（Asia/Taipei）

失效条件：`main` / `next` / 发布 tag 的角色约定变更时立即失效。

| 线 | 角色 | 当前约定 |
|---|---|---|
| `v0.8.4` tag | 发布身份（冻结） | 不移动；对外 artifact 只认 tag |
| `main` | 干净基线 = v0.8.4 源码 + 文档治理 | 默认不收大重构；发布对齐 |
| `next` | 工程直线 | 切片 FF 合入；可逐刀 revert；**不是**当前 release 身份 |
| `codex/<slice>` | 单刀工作分支 | 从 `next` 开出，审完 FF 回 `next` |

`next` 会随 source-only 工程切片推进，本文不再复制易漂移的 HEAD、tested candidate 或 sole NEXT。
实时 commit/worktree 必须现场复核；当前缺口、证据层与唯一建议 NEXT 从
[known issues](known-issues.md) 进入，日期化 source 结论从该页链接的最新 audit 进入。无论 `next`
领先多少 commit，都不能把它称为公开 release 身份。

历史治理/调查分支与 worktree 多数已被 `main` 吸收，默认不必继续使用；删除前须用户明确授权。
