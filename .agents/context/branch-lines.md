# 分支线角色（工程约定）

最后复核：2026-08-22（Asia/Taipei）

失效条件：`main` / `next` / 发布 tag 的角色约定变更时立即失效。

| 线 | 角色 | 当前约定 |
|---|---|---|
| `v0.8.4` tag | 发布身份（冻结） | 不移动；对外 artifact 只认 tag |
| `main` | 当前集成基线 | 已含 P0→P3 source 与仓库治理；不是 release、artifact 或 live 身份 |
| `next` | 既有工程线 | 不因 `main` 的集成自动获得相同 HEAD、source evidence 或 release 身份 |
| `codex/<slice>` | 单刀工作分支 | 从目标的干净基线开出；审完后按单独授权决定合入目标线 |

本文不复制易漂移的 HEAD、tested candidate 或 sole NEXT。实时 commit/worktree 必须现场复核；当前
路线、证据层与唯一建议 NEXT 从[当前重构路线](known-issues.md)进入。无论任一工程线领先多少 commit，
都不能把它称为公开 release 身份。

历史治理/调查分支与 worktree 多数已被 `main` 吸收，默认不必继续使用；删除前须用户明确授权。
