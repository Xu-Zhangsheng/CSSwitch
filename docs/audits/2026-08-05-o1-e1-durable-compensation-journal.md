# O1-E1 durable compensation journal foundation source closure

日期：2026-08-05（Asia/Taipei）

证据层：source-only。本文不证明 artifact、installed/live、真实 provider / Science / SSH、签名、公证或 release。

## 结论

O1-E1 在 `next` 的 `7d4ffb51359bbd6ceb8dd67a58bef4fe63c5c594` 完成，比较基线为
`3585209629a4a80abee285db358c8fbbf8679469`。implementation commit 为
`feat(runtime): add durable compensation journal`，共修改 28 个 tracked paths。

`Config.runtime_compensation` 现在拥有独立、path-free 的 `RuntimeCompensationJournal` V1，只记录 opaque
compensation id、目标 profile、runtime fingerprint、受管 snapshot ticket 与 aggregate state。它不复制
business V2 的 prior-stop recipe、absolute runtime path、诊断 message 或 credential。

`Journaled` 与 `PreJournalAbort` 都在首个 compensation effect 前，以完整当前 business record 和 marker
absence 做 exact CAS，发布 `in_progress`；intent 不能发布时保持 snapshot 并零 compensation effect 返回
manual。authority restore 精确恢复 compensation 前的 `runtime_transaction`，只把独立 marker 叠加回捕获
config。完整 compensation 成功才清 marker；失败则以 canonical、无重复的 typed failed steps 持久化
`incomplete`。one-click 与 normal mode/settings/profile/Codex auth/settings/downgrade mutation 在任一 journal
打开时 fail closed；`stop_all`、显式 quit 与 native exit 只作为不写 config/credential/journal、只减小运行态
暴露的 terminal cleanup 保持可用。

本阶段没有实现 per-step durable intent/outcome、fresh-process compensation replay 或自动收敛，也没有改变
frontend DTO / 可见文案、provider / Science / SSH、artifact 或 release 合同。

## 独立审查与 focused validation

- 首轮 fresh clean-context reviewer 找到 `PreJournalAbort`、normal mutation bypass、prior-stop path 泄漏与
  R2-B regression；修复后复审又找到 Codex auth guard 与 terminal-cleanup 合同未闭合；
- 下一轮 fresh clean-context review 确认主实现成立，但以 MEDIUM 指出五个 profile/Codex operation 的
  inventory 漏记 marker read/guard，以及 Codex login spawn/register 顺序写反；修复权威清单后，最终
  fresh clean-context reviewer 返回 `findings: none`、`verdict: PASS`；
- Desktop Rust fmt、check、`clippy --all-targets -- -D warnings`，schema/journal CAS/source-contract/profile
  guard 与 Codex login/logout 聚焦测试通过；
- inventory、Skill boundary、source-gate runtime、run-evidence、quality kernel、documentation governance 与
  quality metadata 聚焦验证通过。隔离 loopback / fake-process 测试在允许本机测试能力后通过，不使用真实
  provider、凭证或账号。

## exact-SHA source gate

主 worktree 的首次 CLI 预检因 output-root path 过长退出；改用短路径后，严格 source snapshot 又因受保护
ignored 数据内的未跟踪 `.gitignore` 以 `SNAPSHOT_DIRTY` fail closed。两次都没有 completion evidence，
不作为产品失败或 PASS；`.sandbox` 等用户 ignored 数据没有被删除、移动或读取内容。

随后在同一 implementation exact SHA 的 clean detached worktree 和全新 0700 output root 重跑。run
`ffa14f373a988c26687a1dff52fab7e0` 绑定
`7d4ffb51359bbd6ceb8dd67a58bef4fe63c5c594`，结果为 15/15 suites PASS、15/15 source observations、
aggregate `PASS`、runner exit `0`；source snapshot 为 549 entries、12,329,949 bytes，gate comparison base 为
`010a69aa9ffc2696f927dd1fd578b31c69fb2d6a`。

- completion seal SHA-256：`488aedca768488f21db858cde381470f1696d2d80c311fcacbbf2f18cf79a5bb`
- run manifest SHA-256：`da2c55b04a47b25f26732e6b426c8100dd35b6e3ad2c3316c31762907442a07f`
- evidence manifest SHA-256：`8e61503f36df339527e71c5cd01c39b430a2dff23e70e2dc99daef0420118fcb`
- source snapshot manifest SHA-256：`dc5da5d2c010e1f7ecb5abde8c9ccde7977d334aca75d5ba0bbd73f8cdce4b00`

## 停止点

O1-E1 已完成，但不自动授权 stepwise compensation replay、其他 runtime 重构、artifact/live/provider 验证或
release。选择下一阶段前必须基于届时实时源码重新比较问题、依赖与非目标。
