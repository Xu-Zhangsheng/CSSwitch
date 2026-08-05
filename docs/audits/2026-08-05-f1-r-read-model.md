# F1-R unified read model source closure

日期：2026-08-05（Asia/Taipei）

证据层：source-only。本文不证明 artifact、installed/live、真实 provider / Science / SSH、签名、公证或 release。

## 结论

F1-R 在 `next` 的 `32eb724f2615478cf99438930a9ffe5bc514154b` 完成，比较基线为
`a9b1f05c4dd2283300260b79f9766a4213b32eca`。implementation commit 为
`feat(runtime): close F1-R read model`，共修改 17 个 tracked paths。

`get_config` 现在走只读加载，不再隐式迁移、写配置、清除 pending notice 或取得 writer fence。notice 使用
确定性内容 identity，并由显式 `acknowledge_pending_notice(expectedNoticeId)` 在 canonical config transaction
中只清除匹配项；重复 ack 为幂等，过期 identity 不会清除新 notice。前端只在展示后发起 ack，ack 失败时保留
notice，允许后续再次展示。

boot 的 error / attention 分裂状态与分裂事件已收敛为单一、带单调 sequence 的 publication；前端先订阅再拉
snapshot，只接受更大的安全整数 sequence，因此重复或乱序 delivery 不会覆盖新状态。failed / attention 的
finalize unknown 与历史 UI 语义保持不变，attention 仍可重试。

本阶段没有改变 frontend DTO 可见文案、provider / Science / SSH 合同、artifact 或 release。Rust 1.96
`clippy -D warnings` 暴露的 3 处 `needless_borrow` 只做了 gate-compatible 机械清理，不改变行为。

## 独立审查与 focused validation

- fresh clean-context 独立 reviewer 结论为 `PASS`，`BLOCK/HIGH/MEDIUM/LOW = 0/0/0/0`；它核对了基线、
  17 个候选路径、只读 config path、notice identity / ack transaction、统一 boot publication、frontend
  sequence 消费、测试与文档；
- reviewer 绑定的 implementation diff fingerprint 为
  `21f9fec5f02a74b4a1c05c24d428b6cc39fca92bb769389a391870d2f64f2813`；
- Desktop `cargo check`、`cargo clippy --all-targets -- -D warnings` 与相关 Rust contract selectors 通过；
- frontend 为 `48/48 PASS`；runtime mutation inventory、Skill boundary 与 documentation governance 为
  `24/24 PASS`；quality metadata 与 `git diff --check` 通过。

## exact-SHA source gate

主 worktree 的首次 run `922b5d3130777072442e7953ae661b5d` 在 `SNAPSHOT` 阶段以
`SNAPSHOT_FAILED`、runner exit `12` 终止；它没有形成 completion evidence，不作为产品失败或 PASS。
主 worktree 中受保护的 ignored / 用户数据没有被检查、移动或删除。

随后在同一 implementation exact SHA 的 clean detached worktree 和全新隔离 output root 重跑。run
`3d6f85a8fe8670e6aad5314ef12d346d` 绑定
`32eb724f2615478cf99438930a9ffe5bc514154b`，结果为 15/15 suites evidence、15/15 source observations、
aggregate `PASS`、runner exit `0`；source snapshot 为 547 entries、12,269,482 bytes，gate comparison base 为
`010a69aa9ffc2696f927dd1fd578b31c69fb2d6a`。

- completion seal SHA-256：`f0e60af3159870d7724cd2ea7c8b5dff549edc3cfb22cfb2d7d05bff716b834e`
- run manifest SHA-256：`7a3b349738fd426b6b460e50a22e51352906f96f1e449f480874fcaa65903122`
- evidence manifest SHA-256：`9f1313d67ee1791686fe6854b9e648d8977264a7b3d97920c823c9c1dcf6f531`
- source snapshot manifest SHA-256：`cab79c177890864ebce5af94a00ee53ae90aba31562a72c8725157f30aa9a67b`

## 停止点

F1-R 已完成，但没有自动继承的新 implementation sole NEXT。选择或实施下一阶段前，必须基于届时实时源码
重新比较仍开放问题、依赖与非目标。
