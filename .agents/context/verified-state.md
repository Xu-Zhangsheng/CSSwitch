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
`next@a60c2ee656429903f1fd8f398dc6ad8194aa9346` 的 canonical 15-suite source gate、由该
exact source 新构建的 `CSSwitch Test.app`、packaged Gateway 与 Claude Science 0.1.25 identity
已由递归 G1 receipt 绑定并取得 `PASS`。bundle、Desktop、Gateway SHA-256 分别为
`52cd48c…73c9`、`77b4ebef…9be8`、`610c0206…87ab`；G1 receipt SHA-256 为
`b01cf7a2…ddbb`。当前 tuple 的 `B-RUNTIME-01` canonical run `ra60c2eec` 已在隔离 HOME/data-dir、
deny-egress sandbox、真实 Science 0.1.25 与 loopback fake provider 下取得 scoped `PASS`；55 条事件
严格单调、5/5 请求命中、controller elapsed `298.825957s < 300s`，51/51 evidence hashes 与最终
cleanup 经 clean-context 独立复算通过。其 `B-CORE-01` run `bcore-a60c2ee-r1` 又完成合成 project、
permission request/grant/revoke/denial、artifact v1→v2 lineage/diff/preview、两次 runtime restart 后回读及
真实 pointer annotation 到下一消息传递；31/31 socket rows 为 loopback，22 条事件全 PASS，30/30
evidence hashes 与精确清理经 clean-context 独立复算通过。其 `B-CONTEXT-01` canonical run
`bcontext-a60c2ee-r13` 又完成 11/11 plan/delegation/fork/restore/Memory/compaction/Reviewer/
Specialist/isolation 子门；150 个脱敏 request envelope 的四项跨域计数为 0，活动期 26/26 socket rows
为 loopback，最终 UI stop/exit、Memory/browser/process/port/runtime cleanup 全部 `PASS`，179/179
evidence hashes 复算 `OK`。其 `B-PROVIDER-01` canonical run `provider-a60c2ee-r1` 又完成 11-case
exact-artifact local-mock 矩阵：10 个 case 由 exact App 启动 packaged Gateway，SiliconFlow 由同一
exact packaged Gateway 直启；99/99 observation/event、55/55 request、64/64 loopback socket row
与 583/583 top closure 全部通过，111/111 Provider loopback tests 从头重跑 `PASS`。

当前 code-bearing RM-46 候选为 `next@d74221e2948f32cd67db0aed8920af6122d0c798`：clean detached
15-suite source gate run `33e290ddc03a1a131c064cf4b5dcb7e1` 为 15/15 `PASS`、runner exit 0；
由该 SHA 新构建、完整 ad-hoc 签名且未安装的唯一 bundle ID Acceptance App，又在隔离 HOME、假 key
与动态 loopback mock 下完成 OpenCode Go 双 transport、Grok、Gemini 的 4 次 discovery、4 次明确
选择/手填与创建、4 次保存前 scratch 校验。discovery 前后正式 config 不变；最终 4 个 profile 与
8 个 mock request 闭合，OpenCode 上游只收到裸 `kimi-k3` / `minimax-m3`。本结论只关闭 RM-46；
`a60c2ee` 的 B-RUNTIME/B-CORE/B-CONTEXT/B-PROVIDER 结果仍只绑定旧 exact tuple，不能继承给
`d74221e`。Reviewer/Specialist 服务结果、RM-47 真实 Provider、Skill/MCP、SSH、installed、
升级/rollback、Developer ID 签名、公证、DMG 与 release-ready 仍为 `NOT-RUN`。旧 `9e08924`
tuple 的 isolated-live 结论也只保留为历史日期化证据，不能继承。

完整证据与不能外推的边界见
[v0.8.4 release evidence](../../docs/evidence/releases/v0.8.4.md)；日期化调查从
[调查索引](../../docs/evidence/investigations/README.md)进入；当前 exact tuple 的上下文验收见
[`B-CONTEXT-01`](../../docs/evidence/investigations/2026-08-11-claude-science-0.1.25-a60c2ee-b-context-01.md)。
当前 exact tuple 的 Provider 本地矩阵见
[`B-PROVIDER-01`](../../docs/evidence/investigations/2026-08-11-claude-science-0.1.25-a60c2ee-b-provider-01.md)；
当前 code-bearing RM-46 配置 UX 见
[RM-46 日期化证据](../../docs/evidence/investigations/2026-08-11-rm46-provider-configuration-ux.md)。
