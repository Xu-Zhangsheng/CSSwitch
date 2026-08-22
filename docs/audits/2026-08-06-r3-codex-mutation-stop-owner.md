# R3 Codex mutation stop owner source closure

日期：2026-08-06（Asia/Taipei）

证据层：source-only。本文不证明 artifact、installed/live、真实 provider / Science / SSH、签名、公证或 release。

## 结论

R3 在 `next` 的 final source candidate
`275a9af7a967e2f44570fd90e934c415392be371` 完成，比较基线为
`82430e86577e1955a6b9890246eddccfe30448ae`。implementation commit 为
`b547bff1c409011df85d5a96f22e199b152348ff`；测试身份合同与两处严格数量封条分别由
`2821979df5410decbb50acea9f118b63bea9c4f1`、`275a9af7a967e2f44570fd90e934c415392be371` 收口。

`prepare_codex_auth_mutation` 覆盖 logout、关闭 experimental Codex 与 `set_codex_network`。
该共享入口现在使用 process-local Science stop owner：在 probe 前冻结 generation、runtime、
confirmed-stopped、sandbox child PID、port 与 URL；回到 `AppState` 锁内按该 snapshot 做 claim-time CAS，
取得 exact typed stop request；锁外执行 stop / wait；再次回锁后按完整 owner 做 publication CAS。

probe 到 claim、claim 到 publication 两个窗口中的 replacement 都会使 mutation fail closed。claim、stop 或
stale publication 失败不会覆写 replacement、推进 Codex config mutation、bump generation 或停止 replacement
Gateway；只有 current stop success 才按既有语义 bump generation、停止 Gateway 并继续 mutation。command DTO、
既有可见成功文案不变；downgrade cleanup、native-exit 与其他 sibling owner 不在本阶段范围。

## 独立审查与 focused validation

- 首轮 fresh clean-context reviewer 发现 probe 与 claim 之间旧 detected runtime 可覆写 replacement，结论为
  `HIGH = 1`、`FAIL`；修复加入 pre-probe snapshot 与 claim-time CAS，并新增对应竞态回归；
- 修复后的 fresh reviewer 对实现返回 `PASS`、`BLOCK/HIGH/MEDIUM/LOW = 0/0/0/0`；source identity 修复后，
  两轮新的 clean-context reviewer 分别核对完整 candidate 与最后一处数量封条，均以四级 findings 全 0 封口；
- 两项 R3 回归分别覆盖 probe→claim 与 claim→publication replacement race，并证明外部 stop wait 期间
  `AppState` 可读、replacement owner / Gateway / generation 与 config bytes 不被陈旧结果推进；
- Codex mutation group 37 passed、1 approved ignored；`set_mode` / `set_settings` / Codex / `stop_all` owner-CAS
  sibling 回归 4/4；Skill runtime boundary 与 mutation inventory 20/20；
- source-gate contracts 23/23、source runtime 23/23、run-evidence contracts 177/177、quality metadata / impact-pr、
  `cargo fmt`、`cargo check --all-targets`、`cargo clippy --all-targets -- -D warnings` 与 `git diff --check` 均通过；
- 所有执行使用临时目录、fake process / Science 与隔离 loopback；未读取或消费真实凭证、provider、Science
  数据或 SSH。

## gate 修复链

首次 exact-SHA run `77d139e7bbaa43e0f27ac1d01cc12f7c` 在 implementation commit 上为 14/15：
Rust Desktop 实际 565 项全部执行成功，但固定 inventory 只有 563，唯一差异是两项新增 R3 测试。修复只加入
这两个公开 ID、把严格数量同步为 565，并把 15 个 suite 对同一 fixture 的 SHA 绑定更新为
`3a984548b04440754332c8cbeae2c000abbe45688273e94bb18e2794b897c634`；ignored / skipped 白名单未改变。

第二次 exact-SHA run `8031982ed22922a01921651863150bf2` 为 14/15：Rust Desktop 已 PASS，唯一失败是
run-evidence contract 中另一处仍为 563 的严格 count seal。该断言同步为 565 后，受影响 suite 177/177 PASS。
两次失败均保留为失败证据，不能升级为 source PASS。

主 worktree 的既有 ignored/runtime 数据使两个 preflight snapshot 诊断 fail closed；这些用户数据未被读取或
清理。最终 gate 改在一次性 clean detached exact-SHA worktree 中执行，不改变候选内容。

## exact-SHA source gate

run `099cad1183f17dc87b21009816fbaf2f` 精确绑定
`275a9af7a967e2f44570fd90e934c415392be371`，结果为 15/15 suites、15/15 source observations、
aggregate `PASS`、runner exit `0`。Rust Desktop 为 565 discovered / executed、525 passed、0 failed、
40 approved ignored、0 skipped / not-run；source snapshot 为 559 entries、12,548,374 bytes，comparison base 为
`010a69aa9ffc2696f927dd1fd578b31c69fb2d6a`。

- completion seal SHA-256：`b3d932a45cdfca80928a522f3d84560a00077f14f93f393f5310a94567f1aad4`
- run manifest SHA-256：`de0e3387d2e09934b53338c1c0f76f653277aaf02328ba8a565b1253c94d2e60`
- evidence manifest SHA-256：`b82839064a015dddc2d736a9d7058809e7a22aacb4b817c90e365d0bf96ee270`
- source snapshot manifest SHA-256：`12c3f4b9e6d06a2917b27f63616372805ad3bb7548996a2ea268f61afb68a945`
- input digest set SHA-256：`cf69caeae1f5b06da363a0d156526cc5ca291a2d296189540077c850b362834c`

## 停止点

R3 到此完成。本文不自动授权 downgrade cleanup、native-exit 或其他 stop owner 的迁移，也不授权
artifact/live/provider/真实 Science/SSH、签名、公证、push 或 release。下一阶段必须基于届时实时源码重新做
只读基线并只选择一个 sole NEXT。
