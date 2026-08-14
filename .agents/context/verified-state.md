# 已验证状态快照

状态：当前；按 source、artifact、installed、live、signing 与 public 分层汇总

最后复核：2026-08-14（Asia/Taipei）

失效条件：受审 source、artifact identity、安装 App、Science / Gateway runtime、Provider
结果、签名或公开 Release 任一相关事实改变时，对应层立即失效；未受影响层仍按 exact identity 判断。

## 当前结论

| 层 | 当前可声明的事实 |
|---|---|
| Current audited source | 本轮全仓审计绑定最初 clean 的 `next@cfc4008a64d9ef41a9e4238507f85b52095f7c1f`；reconciliation lineage 已冻结到 `f7594c61f9922f3430675f36abfadea9b3a3fda0`，当前 reviewer-repair 候选以实时解析的 clean `next` HEAD 为准，本页不预写尚未生成的提交 identity |
| Current source closure | `NOT-RUN on current reviewer-repair candidate`：`f7594c61f9922f3430675f36abfadea9b3a3fda0` 的 canonical 15-suite gate 在 run `a5ab0c9f478a4fa1fad19036660be2a8` 为 `PASS`，但其 fresh review 因提前确认 machine claim 和过期 Context 为 `FAIL`。当前修复不能继承该 seal/review，必须在新 exact SHA 上重跑；因此不得声明 `SOURCE-GREEN` |
| Latest accepted full source / test lineage | `next@d2cf95e877aa110013a8360d6fcd72c1b38bcfb3` 的 canonical 15-suite gate 与 fresh completion review 均为 `PASS`。该结论只绑定该历史 exact SHA；后续 production/test 改动不得继承 |
| Historical exact artifact / Science adoption | `18a67881c7e7d760fa8deb7f53e6ba246a32d94d` 的 Acceptance artifact、Science 0.1.25 adoption、normal artifact、installed smoke 与逐 operation Provider 结果保留为日期化历史证据；它们不能证明当前 `cfc4008a`，也不能证明 Science 0.1.27 compatibility |
| Current exact artifact | `NOT-RUN`；本轮没有构建 artifact，也没有读取、替换或启动已安装 App |
| Current isolated / installed / authorized live | `NOT-RUN`；没有启动 Gateway / Science、没有读取真实 Science data-dir，也没有发出真实 Provider、Skill、SSH 或账号请求 |
| Current signing / notarization | `NOT-RUN`；历史已知安装测试 artifact 只有 ad-hoc signing 的结论不能继承给当前 source 或未来 artifact |
| Public release | 仓库当前发布证据仍以 `v0.8.4` 为最近公开层；本轮未创建 tag、DMG 或 Release，也未改变公开层 |

Claude Science 官方 changelog 在本轮复核时已列出 `0.1.27`，而仓库最近的 exact
artifact/live 兼容证据绑定 `0.1.25`。两者之间的 source、package、artifact、adoption、installed
和 authorized-live 兼容性均不得继承，当前为 `NOT-RUN`。

## 历史证据入口

- `d2cf95e` 以前的 source seal、`cfc4008a` 的全仓审计与最初 pre-suite gate 阻断见
  [2026-08-14 全仓审计](../../docs/audits/2026-08-14-next-repository-docs-architecture-review.md)。
- `18a67881` 的 artifact、Science 0.1.25 adoption、installed 与 Provider 分项见
  [2026-08-12 日期化验收](../../docs/evidence/investigations/2026-08-12-csswitch-18a67881-adoption-installed-live-acceptance.md)。
- 公开 `v0.8.4` 历史层见
  [release evidence](../../docs/evidence/releases/v0.8.4.md)。

配置、账号数据库、Keychain、token / API Key、SSH 私钥和真实 Science data-dir 本轮均未读取或
修改。使用任何状态前仍须实时复核 Git、目标 artifact 与 runtime；本页不是构建、安装、live、
签名、发布或凭证访问授权。
