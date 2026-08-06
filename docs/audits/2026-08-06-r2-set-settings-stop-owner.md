# R2 set_settings stop owner source closure

日期：2026-08-06（Asia/Taipei）

证据层：source-only。本文不证明 artifact、installed/live、真实 provider / Science / SSH、签名、公证或 release。

## 结论

R2 在 `next` 的 final source candidate
`be47ea6026e5720ab147e1b03cfc0ffd2440eb38` 完成，比较基线为
`0e59e6f58f3817d42b9d979fd4efdd21e9a663c7`。implementation commit 为
`be47ea6026e5720ab147e1b03cfc0ffd2440eb38`。

需要 teardown 的 `set_settings` 现在与 `set_mode` / `stop_all` 复用同一 process-local Science stop
owner claim / publication 边界：在 `AppState` 锁内冻结当前 generation、runtime、confirmed-stopped、tracked
child PID、sandbox port 与 URL，并取得 exact typed stop request；锁外执行既有 stop script、TERM/KILL 与
等待；回锁后只有完整 owner CAS 仍成立才清理 tracking 并发布 outcome。

`set_settings` 保持自身既有语义：只有 current Science stop success 才 bump generation、停止 tracked
Gateway、撤销 SSH bridge/stub 并进入 settings config commit；stale、claim 或 stop failure 保留 replacement
Science、Gateway 与旧 settings，且不继续 SSH cleanup/commit。SSH cleanup 或 config commit failure 仍保持
stop-before-commit，不重启已停止 runtime。downgrade cleanup、native-exit 与其他 sibling stop owner 不在本阶段范围。

## 独立审查与 focused validation

- fresh clean-context reviewer 审查完整 implementation candidate，返回 `PASS`，
  `BLOCK/HIGH/MEDIUM/LOW = 0/0/0/0`；
- R2 regression 分别覆盖 generation-only 与 identity-only drift，证明 Science stop 等待期间
  `AppState` 可读，并在 stale publication 时保持 replacement Science、tracked Gateway、generation 与旧 settings；
- 既有 R0 `set_settings` characterizations 继续覆盖 journal preflight、stop failure、SSH bridge/stub cleanup
  failure 与 config commit failure 的 stop-before-commit 顺序；
- `set_mode` / `set_settings` / `stop_all` owner-CAS 回归 3/3、Skill runtime boundary 与 mutation inventory
  20/20、quality metadata / impact-pr、`cargo fmt`、`cargo check --all-targets` 与
  `cargo clippy --all-targets -- -D warnings` 均通过；
- 首次在受限 sandbox 运行两项 R0 loopback characterization 因动态端口 bind 被拒；同一候选在允许的隔离
  loopback 环境重跑 2/2 PASS。该环境诊断未参与最终 gate 判定；
- 所有测试使用临时目录、fake process / Science 与动态 loopback；未读取或消费真实凭证、provider、
  Science 数据或 SSH。

## exact-SHA source gate

run `6a307e628ca04b4f478b13509b75928e` 精确绑定
`be47ea6026e5720ab147e1b03cfc0ffd2440eb38`，结果为 15/15 suites、15/15 source observations、
aggregate `PASS`、runner exit `0`。Rust Desktop 为 563 executed、523 passed、0 failed、40 ignored、
0 skipped / not-run；source snapshot 为 558 entries、12,526,413 bytes，comparison base 为
`010a69aa9ffc2696f927dd1fd578b31c69fb2d6a`。

- completion seal SHA-256：`62cfdb2daba85a1fb461de736ed88a9034ba7ecd3fcac14fbfb32f6ec7e5d77c`
- run manifest SHA-256：`58a3f1b643a768bd92539dfc8191a2ad2e526cb8be348fcfd13656ab4e3aae8c`
- evidence manifest SHA-256：`546d48cbe082c5f5d894ef6c2a62ad86958890151e148f11832a0fd2d1ea16b6`
- source snapshot manifest SHA-256：`14d77641d244417e3cc9b3791bfb6d5872ad604402fc59f278fafcfab8981192`
- input digest set SHA-256：`657b602b333de65c5f5ac8eeb6016588b1f87f48911069619b68084e193da0d7`

## 停止点

R2 到此完成。本文不自动授权 downgrade cleanup、native-exit 或其他 stop owner 的迁移，也不授权
artifact/live/provider/真实 Science/SSH、签名、公证或 release。下一阶段必须基于届时实时源码重新做只读基线并只选择一个 sole NEXT。
