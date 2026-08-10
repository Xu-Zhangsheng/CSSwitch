# 已验证状态快照

状态：当前；只汇总已绑定的 v0.8.4 分层事实

最后复核：2026-08-11（Asia/Taipei）

失效条件：release source、最终 DMG、安装 app、公开附件或维护基线任一变化时，
对应层立即失效；未受影响层仍按其 exact identity 判断。

| 层 | 当前可声明的 v0.8.4 事实 |
|---|---|
| Source / unit | exact release source 的 trusted `GATE-SOURCE` completion seal `PASS`；run `712d9f75cb2d98679dfd64aed5cb1fea` |
| Final artifact | DMG SHA-256 `23471daf…f2b2`（64 hex，与 GitHub digest 一致）；Gateway `4448c15e…57a`（64 hex）。历史 Desktop 串仅 63 hex，**已废止**，本层不再声明 Desktop 二进制 hash |
| Installed | `/Applications/CSSwitch.app` 版本 0.8.4；收尾时只检测到一个 CSSwitch app。因 Desktop hash 废止，**不再**声明“安装 hash 与最终 artifact 记录一致” |
| Signing | strict seal 校验通过；仅 ad-hoc，不是 Developer ID / notarization / Gatekeeper |
| Public | peeled tag 与 release source 一致；公开重下载 hash、镜像校验与根目录白名单通过 |
| Current remote refresh | 2026-07-30 tag/main/Release 元数据仍与上述公开 identity 一致 |

以下仍不是当前 PASS：全部真实 provider/model、真实 SSH server、官方账号 entitlement、
Science 全领域行为、Intel/Windows/WSL、Developer ID/notarization/Gatekeeper。
另有一条不覆盖公开 release/installed 事实的当前 exact-artifact 验收：
`next@9e08924481c8f5edb181254332d94daba0cbe4b2` 的 G1-bound `CSSwitch Test.app`、
packaged Gateway 与 Claude Science 0.1.25 已取得
`B-RUNTIME-01=PASS(scope=csswitch-gateway-science,loopback-provider-fixture)`；唯一 canonical
run `r9e08924b` elapsed `291.892949s`，post-cleanup 51/51 evidence hashes `OK`。同一 tuple 的
`B-CORE-01` run `bcore-9e08924-r1` 又限定闭合 synthetic project/file、permission grant/revoke/
denial、artifact lineage/diff/preview/provenance 与 annotation 下一消息传递，post-cleanup 14/14
evidence hashes `OK`。同一 tuple 的 `B-CONTEXT-01` run `bcontext-9e08924-r9` 又限定闭合
plan/delegation/fork/restore、Memory/compaction、Reviewer/Specialist local surface/request-shape 与
two-project/two-session isolation，11 份 observation/event 与 post-cleanup 89/89 evidence hashes
`OK`。r9 的 UI stop 后没有封存 stop-state snapshot，Desktop 最终由 exact PID SIGTERM 收口，故不把
正常 App exit 外推给本轮；正常 stop/exit 已由 B-RUNTIME 独立证明。同一 tuple 的
`B-PROVIDER-01` run `provider-9e08924-r1` 又闭合 11 个本地严格 Provider case：10 项由 exact App
启动 packaged Gateway，SiliconFlow 仅由同一 exact packaged Gateway 直启以保留 hostname-aware
proxy fixture，明确不写成 Desktop 级 E2E。99 个 observation/event、55 个 fixture request 与
64 个 active socket rows 全部通过；收口后 64 个 owned PID、88 个动态端口和 9 个临时路径清零，
583/583 top evidence hashes `OK`。这四项不覆盖本表的 installed/public release artifact，也不外推
真实 Provider、Skill、SSH、签名或 release-ready 状态。

完整证据与不能外推的边界见
[v0.8.4 release evidence](../../docs/evidence/releases/v0.8.4.md)；日期化调查从
[调查索引](../../docs/evidence/investigations/README.md)进入。
