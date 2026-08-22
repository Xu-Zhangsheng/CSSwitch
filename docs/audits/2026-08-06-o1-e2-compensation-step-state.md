# O1-E2 durable compensation step-state foundation source closure

日期：2026-08-06（Asia/Taipei）

证据层：source-only。本文不证明 artifact、installed/live、真实 provider / Science / SSH、签名、公证或 release。

## 结论

O1-E2 在 `next` 的 `df51ab61e5c22e4c781fe12978630b9c3161eeeb` 完成，比较基线为
`8b7026d4e45dbc1bc38b173fd340eab16b05e246`。implementation commit 为
`feat(runtime): persist compensation step state`，共修改 13 个 tracked paths。

当前 writer 只生成 path-free `RuntimeCompensationJournal` V2；V1 保持严格兼容读取并阻断 mutation，生产路径
不会升级、推进或清除 V1。V2 固定保存 Science cleanup、SSH cleanup、authority restore、prior Science
restart 与 snapshot cleanup 五个 top-level step，各自只允许
`pending -> in_progress -> succeeded|failed|skipped(cause)`。

每次 step intent、outcome 与 aggregate completion 都以完整 compensation record 和单个 exact expected
business transaction 做 CAS。authority restore 前 expected record 只能是 active transaction；专属 authority
restore outcome 写入点依据实际 config 结果单向切换到 restored transaction，之后所有步骤只能接受该 restored
record，不能在 active / restored 两侧任选。authority restore 会保留最新 step state；aggregate
`failed_steps` 只从 typed step outcome 推导。任一步 intent 或 outcome 发布失败都会停止后续 effect 并保留恢复
snapshot。

本阶段没有实现 fresh-process replay / 自动收敛，没有改变 frontend DTO / 可见文案，也没有扩大到 provider、
Science、SSH、artifact 或 release 合同。

## 独立审查与 focused validation

- 首轮 fresh clean-context reviewer 以 HIGH 指出普通 step CAS 可在 active / restored 两条 business record 间任选，
  不能证明 authority 边界前后单向；同时发现测试过早清除 transaction；
- 修复为单一 expected record、专属 authority boundary，并补充边界前后 wrong-side rejection 与 aggregate exact
  current-record 测试后，新的 fresh clean-context reviewer 返回 `findings: none`、`verdict: PASS`；
- `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings` 与完整 `cargo test --lib` 通过；完整
  Rust lib 结果为 511 passed、0 failed、40 ignored；
- compensation journal/CAS、source contract、config roundtrip/illegal-state、隔离 PreJournalAbort、runtime mutation
  inventory、Skill boundary、quality kernel、source-gate/run-evidence contract 与 documentation governance 聚焦验证通过。
  隔离测试只使用 loopback / fake process，不读取或使用真实凭证、provider 或账号。

## exact-SHA source gate

首次 CLI 预检因 output-root path 过长以 runner exit `12` 退出，没有生成 completion evidence；该结果单独记录为
环境预检失败，不作为产品失败或 PASS。随后在 implementation exact SHA 的 clean detached worktree 和新的短
`0700` output root 重跑。

run `ec8c166df2f23e4282d8d16d44a1300e` 精确绑定
`df51ab61e5c22e4c781fe12978630b9c3161eeeb`，结果为 15/15 suites、15/15 source observations、aggregate
`PASS`、runner exit `0`；source snapshot 为 550 entries、12,365,551 bytes，gate comparison base 为
`010a69aa9ffc2696f927dd1fd578b31c69fb2d6a`。

- completion seal SHA-256：`a7a0bd08fc4ac1b93032f1ee761e9cef52ccbf8da170087ed83093d825800f6c`
- run manifest SHA-256：`74f9eada427bd05bf3c034617e82645ea8f05f54da2820024c53385ae59b78cb`
- evidence manifest SHA-256：`2257eb27a66ffb818f671700a7827e2d8bd2315f871b75c7c6933212bf8837db`
- source snapshot manifest SHA-256：`17b3d68a68e72a4dd14c1d19b8c6daaca9b743b4f8ada3e01973ff0b6ad2d700`
- input digest set SHA-256：`f74bfa30b2743b1fc77619739c049ef08592ba620f2c4364ef426c29c609dd50`

## 停止点

O1-E2 已完成。此 closure 不自动授权 fresh-process compensation replay / 自动收敛、其他 runtime 重构、
artifact/live/provider 验证或 release。选择下一阶段前必须基于届时实时源码重新做只读基线。
