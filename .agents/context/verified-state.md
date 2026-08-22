# 已验证状态快照

状态：当前；按 source、artifact、installed、live、signing 与 public 分层汇总

最后复核：2026-08-22（Asia/Taipei）

失效条件：受审 source、artifact identity、安装 App、Science / Gateway runtime、Provider
结果、签名或公开 Release 任一相关事实改变时，对应层立即失效；未受影响层仍按 exact identity 判断。

## 当前结论

| 层 | 当前可声明的事实 |
|---|---|
| Current exact main source closure | `c678b1bee2475686ebbb8b8172c24112a37c6ce9`：`GATE-SOURCE` 允许环境 run `fc4630f41a7930244720d4de46019f82` 在 `/private/tmp/csg.Ki8DX4` 取得 aggregate `PASS`、runner exit `0`、15/15 suites。托管 sandbox 的同 SHA run `eb30f96cd5fbc2b93d4be2c2e18be06b` 因禁止 `ps` / `sandbox-exec` 为 `ENV-BLOCKED`；该诊断不能写成产品失败 |
| Immutable accepted source candidate | 最近有 immutable record 的 candidate 仍是 `d786a2d833dfd5f95b02d15f84f122a8b9fd4225`：其 metadata ChangeRecord coverage 与 impact-release 为 `PASS`，记录为 `quality/source-candidates/d786a2d833dfd5f95b02d15f84f122a8b9fd4225.json`（SHA-256 `5aa204b54c594e2410a7eeee98daf4c278907a83a77e484886dafa603c0b9b95`）。`c678b1b` 当前没有对应 immutable record，不能继承此身份 |
| P0 impact-release repair | `CLOSED`：真实 carry-forward ChangeRecord 精确覆盖 11 个既有 post-v0.8.4 production paths 加 record 自身；没有借此扩展 validator、focused test、identity fixture 或 catalog 改动 |
| P1 Science control bounded primitive | `CLOSED`：Desktop post-start `configure-third-party` caller 使用 same-crate bounded primitive；absolute deadline、双流 output cap、private process group、异常 cleanup/reap 与 typed outcomes 均有 replacement-preserving source tests；Gateway 与 skill-package 职责未合并 |
| P2 non-one-click mutation receipts | `CLOSED`：set-mode、set-settings、applied-profile revoke、Codex auth/network 等七类 destructive operation 已由 credential-free durable receipt/fence、per-effect attempt WAL、exact CAS/tombstone、boot fail-closed 与 replacement-preserving recovery 覆盖；one-click/P2-A/P2-B admission、auth sidecar identity/terminal 和 frontend exact mutation identity 已闭合 |
| P3 single-Skill operation control | `CLOSED`（source）：公开 GitHub 固定 commit 的完整单 Skill 通过 Gateway coordinator 进入 immutable plan、显式确认后的 apply、operation-id-only continue/reconcile；install 与 confirmed removal 分别以 durable effect ledger 投影。Plugin、MCP、local-package/bundle、artifact、runtime attach/进程与真实服务不在此结论内 |
| Current exact artifact | `NOT-RUN`；本轮没有构建 artifact，也没有读取、替换或启动已安装 App |
| Temporary / installed runtime | `NOT-RUN`；没有启动 Gateway / Science 或 temporary/installed runtime |
| Live Provider / Science / SSH / account | `NOT-RUN`；没有发出真实 Provider、Science、SSH 或账号请求，也没有读取真实 Science data-dir、账号数据库、Keychain、token / API Key 或 SSH 私钥 |
| Current signing / notarization / Gatekeeper | `NOT-RUN`；source 结论不能继承为任何 artifact 或安装包的签名、notarization 或 Gatekeeper 结论 |
| Public release | `NOT-RUN`；仓库历史最近公开层仍为 `v0.8.4`，本轮未创建 tag、DMG 或 Release，亦未改变公开层 |

`d786a2d…` 的 immutable identity和其历史 gate 不会继承给 `c678b1b`。相反，`c678b1b` 的当前
source closure只来自上列同 SHA 的 15-suite run；它不等于 immutable candidate、artifact 或任何后续层。

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
