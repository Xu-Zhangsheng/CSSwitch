# Downgrade cleanup stop owner source closure

状态：已完成；仅限 source/test

适用范围：`next@b5141a9bab393cbed5e180d3e47e1d4ae300f290` 相对 `58fc4cab46ab93069cc74f8caca864e047dc48eb` 的有限 downgrade cleanup stop-owner 变更。

失效条件：downgrade production caller、Science/Gateway stop policy、process-local owner identity、generation/CAS publication、确定性 fixture 或 canonical source gate 合同任一变化时重新审查。

## 结论

`commands/codex.rs::stop_all_before_downgrade` 已复用现有 process-local Science stop owner：generation bump 后在 `AppState` 内冻结完整 owner 与 typed stop request，锁外执行既有 stop script、TERM/KILL 与 wait，再按 generation + full identity CAS 发布。陈旧结果保留 replacement Science，仍按原 terminal stop-all policy 尝试停止 tracked Gateway，并在 export、backup 或 v2 publication 前失败。

本阶段保持既有错误组合、stop-before-effect、safe failure 不重启与 post-publication direct terminal exit 语义。one-click、history、recovery、compensation 等 transaction-scoped stop caller 未迁移；artifact、installed/runtime、真实 provider/Science/SSH、签名、公证与 release 均未运行。

## 回归与 gate

- 确定性 Rust 回归覆盖 generation-only 与 identity-only 漂移，证明 wait 期间 `AppState` 可读、stale publication 不清 replacement Science、tracked Gateway 仍按 terminal policy 停止。
- 两条既有 downgrade characterization 继续通过，固定 safe precommit failure 与 post-publication uncertainty/exit 语义。
- 首次 candidate `e419e279cb453f7554da32fe10014249bbea8633` 的 canonical run `6cac4f0f508a553ac6de5c196117ff05` 为 sealed `FAIL`（13/15）；新增 Rust identity 后两条容量合同仍断言 591、实际为 592。该 run 不得与后续 PASS 合并。
- 修复后的 exact candidate `b5141a9bab393cbed5e180d3e47e1d4ae300f290` 在新的 clean detached worktree 与空 `0700` output root 完成 canonical 15-suite `PASS`：15/15 suites、15/15 observations、runner exit 0。run ID 为 `44d725daa53005a4321d8f49f56a3868`；completion seal SHA-256 为 `f43c418b5d9a7e05739daa4381c5376135f1e07825b8c7353b2e7f281a881077`；source snapshot manifest SHA-256 为 `f66411518a46e5de9b4f7fbc7f3873bc42814aa93b3ffa270f131f82f1938c3e`。

## 独立审查

fresh clean-context reviewer 只读审查 `58fc4ca..b5141a9`，结论为 `clean-context: YES`、`PASS`，`BLOCK/HIGH/MEDIUM/LOW=0/0/0/0`。reviewer 复算 30 份 result/observation 文件，mismatch 为 0，并确认 Desktop observation 为 592 discovered、592 executed、551 passed、41 approved ignored、0 failed；新增 downgrade race test 实际执行。

## 停止边界

本阶段到此停止产品实现。`b5141a9` 是 source-only candidate，不建立新的 exact artifact、isolated-live、authorized-live、installed、签名或 release 结论。剩余 transaction-scoped stop owner/wait 是否值得继续拆分，必须从最新源码重新只读定界，不能由本阶段自动授权。
