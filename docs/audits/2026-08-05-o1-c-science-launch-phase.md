# O1-C managed Science launch phase source closure

日期：2026-08-05（Asia/Taipei）

证据层：source-only。本文不证明 artifact、installed/live、真实 provider / Science / SSH、签名、公证或 release。

## 结论

O1-C 在 `next` 的 `7cdf95bc8ce37e4b4c32d4ae4684ac6946a60e34` 完成。它把 managed Science launch phase 从 `one_click/cold.rs` 提取到 `one_click/cold/science_phase.rs`，同时保持既有 checkpoint、CAS、锁、receipt、DTO、可见文案与 history semantics。比较基线为 `010a69aa9ffc2696f927dd1fd578b31c69fb2d6a`。

新 phase 拥有 `StartScienceEnvironmentPending`、Skill-install prelaunch registration、Science spawn / health / identity / receipt、DB reverify、exact stop 以及一次 bounded restart / fresh receipt；它在 `VerifyScienceCatalog` 前结束。cold coordinator 继续拥有 prior-stop、authority、Gateway、route、finalize 与 compensation funnel。

## 审查与 focused validation

- 首次 clean-context 独立审查为 `FAIL`，仅有一项 `MEDIUM`：quality inventory 把 prelaunch registration 与 DB 后 route reconciliation 的 effect ordering 合并。只修正文档化 ordering 后，新的 clean-context reviewer 给出 `PASS`，`BLOCK/HIGH/MEDIUM/LOW = 0/0/0/0`。
- Rust formatting、Desktop compile、两个 Rust contract test、25 项 Python runtime/profile/inventory/bridge、16 项 quality kernel、4 项 documentation governance 与 quality metadata 均通过。
- 三项隔离 fake-Science cold / rollback / DB-restart 测试均通过；受限 sandbox 内第一次因 loopback `Operation not permitted` 无法执行，允许动态 loopback 的隔离重跑为 `3/3 PASS`，不是产品失败。

## exact-SHA source gate

第一次完整 gate run `d0f9e5be5850d9e2a5890595cae9ef37` 为 `FAIL`：15 个 suite 中只有 `SUITE-RUST-DESKTOP` 的既有 Codex cancellation test 失败；同一 SHA 单测重跑为 `1/1 PASS`，完整 Desktop Rust suite 为 `509 passed, 0 failed, 40 ignored`。没有因此修改 O1-C source。

随后在第二个全新 detached worktree 上执行独立完整 gate。run `e065006a05853a7682688f4ee4aceb1b` 绑定 exact HEAD `7cdf95bc8ce37e4b4c32d4ae4684ac6946a60e34`，结果为 15/15 suites PASS、15/15 source observations、543 entries、12,251,220 bytes：

- completion seal SHA-256：`881c2140fd595532ee7bad75b29aa15f1e1e26df6efd6b10fa4356714fa2a3c6`
- run manifest SHA-256：`151ef71785ac12b0c5238200e6e8714c1e4144c3ff2562b0df69e1e477d72e54`
- evidence manifest SHA-256：`1c6124efed0beb2ba5dcdb7b176fccbf32a8ed0610296b26d523f3d43b4b2ccf`
- source snapshot manifest SHA-256：`bdd2d9ee6d90300a4eb6a4351430d0adf4b5b86f0849bf60a45210af2ba7a416`

## 停止点

O1-C 已完成，但不自动授权后续 implementation。选择下一个阶段前必须对实时源码、依赖、非目标与仍开放问题重新基线。
