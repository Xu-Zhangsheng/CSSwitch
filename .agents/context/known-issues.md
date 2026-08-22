# 当前重构路线与证据缺口

状态：当前；唯一工程路线；验收层级和执行合同以[生产链路验收](../../docs/operations/real-machine-acceptance.md)为准

最后复核：2026-08-22（Asia/Taipei）

当前 `main` exact source closure：`c678b1bee2475686ebbb8b8172c24112a37c6ce9`；最近 immutable accepted source candidate 仍为 `d786a2d833dfd5f95b02d15f84f122a8b9fd4225`。

失效条件：production owner / caller、candidate source、artifact identity、Science / Gateway runtime、Provider capability、质量元数据、签名或公开 Release 任一相关事实改变时，受影响条目立即失效并须实时重审。

本页是唯一当前路线，只保存“现在到哪、还缺什么、下一步是什么”。稳定机制放在 `docs/`，
exact SHA / run / artifact / 环境的历史证据放在日期化 audit/evidence；旧阶段编号不能提供当前
NEXT、实施授权或验收结论。

## 一句话判断

`main@c678b1b` 已合入 P0 至 P3 的 source 实现与仓库治理。其 exact `GATE-SOURCE` 已在
允许进程观察与 sandbox-exec 的隔离环境取得 `PASS`（runner exit `0`、15/15 suites）；托管 sandbox
的同 SHA 首次运行因禁止 `ps` / `sandbox-exec` 诊断为 `ENV-BLOCKED`，不是产品失败。P3 的窄范围
公开 GitHub 固定 commit 单 Skill 已实现 `plan → explicit apply → continue/reconcile` 与确认式卸载的
durable ledger。此结论只覆盖 source；不能升级为 artifact、runtime、Provider、Science、SSH、账号、
签名或公开 Release 完成。

`c678b1b` 还没有 `quality/source-candidates/` immutable record，因而不能称为 immutable accepted
candidate；`d786a2d…` 仍只是此前 P2 线的最近 immutable accepted candidate，不能替 `c678b1b`
继承 P3 或当前 main 的 source 结论。

## 已完成的结构

- Desktop、Gateway、Codex network 与 Skill package 已形成独立 Cargo 边界；Desktop build 会精确
  staging Gateway sidecar，Gateway 的普通服务、Codex auth、Skill MCP 与 Science control 入口已分开。
- `AppState`、`Lifecycle`、`Config`、typed receipt / journal / manifest 已形成当前 runtime ownership
  骨架；Gateway uncertain-stop、四类 mutation domain、Tauri production 注册面和 exit hook 都有
  明确 owner。
- 未注册、未编译的旧 Skill Manager 和旧 profile-switch writer 已删除；one-click durable journal
  已移动到 private `one_click/transaction.rs`。
- cooperating authority writer 与 managed-launch writer 已进入共享 authority fence；durable
  compensation 已覆盖 per-target replay、authority restore、Gateway cleanup、prior Science restart
  和 snapshot cleanup。
- `ScienceHostAdapter` 已类型化 launch encoding、health/listener proof、managed receipt 与 stop surface。
- Desktop post-start `configure-third-party` caller 已改用 same-crate bounded primitive，具备 caller 起算的
  absolute deadline、双流 output cap、private process group、异常 cleanup/reap 与 typed outcomes。
- 七类 non-one-click destructive operation 已统一进入 credential-free durable receipt/fence、per-effect
  attempt WAL、exact Config CAS/tombstone 与 boot fail-closed；one-click/P2-A/P2-B admission 和 Codex auth
  sidecar exact identity/terminal consumer 已闭合。
- Skill 扩展面已实现窄范围公开 GitHub 固定 commit 的单 Skill source 路径：Gateway coordinator
  消费明确确认，复用 exact staged archive，并以 durable ledger 投影 install、continue/reconcile 与
  confirmed removal。它不证明 artifact、安装 App、Science runtime attach、Plugin/MCP/local-package
  lifecycle 或真实服务能力。

## 部分完成，不能升级表述

- `runtime/proxy_lifecycle.rs`、`runtime/science.rs` 和 authority snapshot 仍大量使用 `include!`；
  这是物理分文件，不是完整语义模块封装。
- one-click façade仍协调 prior stop、authority、Gateway、Science phase、route 与 finalize；
  `AuthorityTransaction` 并未拥有全部 ordering / journal / compensation policy。
- Desktop bounded Science control runner、Skill package 的 `claude-science url` runner、Gateway
  Science HTTP control 仍是三套职责不同的控制面；本 P1 只闭合 Desktop post-start caller，没有为
  “统一”跨 crate 重构，也没有改变 Gateway HTTP policy 或 skill-package 合同。
- Desktop 与 Gateway 共同读取同一 provider contract JSON，但仍各自定义类型和验证逻辑；共享
  digest 能防字节漂移，不能防解释逻辑漂移。
- 大文件不是单独 blocker。只有能形成新的类型 owner、visibility boundary 或 failure contract 时才拆；
  不按行数机械切分 `commands/runtime/tests.rs`、`config.rs` 或 protocol parser。

## 当前证据矩阵

| 层 | 当前状态 | 解释 |
|---|---|---|
| Current exact main source closure | `PASS` | `c678b1bee2475686ebbb8b8172c24112a37c6ce9` 的允许环境 run `fc4630f41a7930244720d4de46019f82`：aggregate `PASS`、runner exit `0`、15/15 suites；详情与首次 `ENV-BLOCKED` run 见[已验证状态](verified-state.md) |
| Last immutable accepted source candidate | `SOURCE-GREEN` | `d786a2d833dfd5f95b02d15f84f122a8b9fd4225`；其 immutable record 为 `quality/source-candidates/d786a2d833dfd5f95b02d15f84f122a8b9fd4225.json`。这不是 `c678b1b` 的 immutable candidate record |
| Exact artifact | `NOT-RUN` | 本轮未构建 artifact |
| Temporary / installed runtime | `NOT-RUN` | 本轮未启动临时或已安装 runtime，也未读取、替换或启动已安装 App |
| Live Provider / Science / SSH / account | `NOT-RUN` | 没有真实 Provider、Science、SSH 或账号请求；真实凭证与 data-dir 未读取 |
| Signing / notarization / Gatekeeper | `NOT-RUN` | source `PASS` 不推导签名、notarization 或 Gatekeeper 结论 |
| Public release | `NOT-RUN` | 未创建 tag、DMG 或 Release；不得虚构 release readiness |

证据时间线：旧 accepted candidate `d077a1c18892c8ccbfc0b70d049445a799d83fb7` 的 seal 与 review
没有继承给 P2。已验收 root 是上表明确的 `/private/tmp/csg.kTHgJq`；任何新 implementation SHA、source 修改或
运行环境变化都不能继承它的 seal。

## P0 source closure 已闭合

- `CHG-SOURCE-CANDIDATE-IMPACT-COVERAGE` 的 current ChangeRecord 覆盖已闭合 11 条既有
  post-v0.8.4 production path，并与 record 自身精确绑定；这正是本 P0 的范围，未扩展为全仓
  requirement enforcement、validator、focused test、identity fixture 或 catalog 改动。
- immutable source-candidate record 已由 fail-closed 工具从 retained g7 生成并读回验证；不得修改、
  覆写或把它当作可随 Context 一同编辑的文件。
- 旧 source-candidate 的 `PENDING` / `NOT-RUN`、旧唯一 P0 `NEXT` 和旧 SHA/root/seal 都已退役，
  不再构成当前 closure 的依据。

## P1 source slice 已闭合

- Desktop/Tauri、Gateway 与 `desktop/skill-package` 的 Science control 职责已先行冻结；production
  改动只落在 Desktop same-crate runner 与 `skill_install_bridge.rs` caller，没有跨 crate 统一。
- absolute deadline、bounded stdout/stderr、private process group、timeout/异常 descendant cleanup、
  direct-child reap/reaper ownership 与 typed failure/timeout/output-limit outcomes 均已闭合。
- success、spawn failure、nonzero、timeout、双流 oversized output、descendant/no residue、cleanup
  handoff、invalid JSON 与 incomplete contract 均有 replacement-preserving tests；固定 Gateway argv、
  environment-only control URL、loopback/JSON/connector/用户错误合同保持不变。
- 本 slice 没有进入 P3 Skill control、artifact、temporary/installed runtime、live Provider、Science、SSH、
  account、signing 或 release。

## P2 source slice 已闭合

- set-mode、set-settings、clear/delete applied profile、Codex login/logout 与 Codex network 七类
  destructive operation 具有 typed plan、credential-free durable receipt/fence、明确 inverse/no-inverse、
  per-effect Pending→InProgress attempt WAL、exact terminal/clearing tombstone 与 fresh boot replay。
- ordinary Config writer、P2-A、one-click 和 P2-B 之间的 admission 在真实 effect 前 fail-closed；
  receipt-only、fence-only、clearing resurrection 与 capture→entry race 均有 zero-effect regression。
- SSH bridge/stub revoke、applied-profile backup scrub、Gateway rollback、Codex auth sidecar start/cancel/exit
  均绑定 durable after-image 或 exact process identity；typed terminal consumer 要求 exact mutation ID。
- 本 slice 没有进入 P3 Skill apply、artifact、temporary/installed runtime、live Provider、Science、SSH、
  account、signing 或 release。

## 仍开放的工程缺口

### P3｜窄范围单 Skill source 合同已闭合，证据与扩展面仍开放

- 公开 GitHub 固定 commit、完整单 Skill 的 source 路径已具备 immutable plan、显式 apply capability、
  durable operation ledger、continue/reconcile 和 confirmed removal；其实现边界以
  [Skill / MCP / Plugin 扩展控制面](../../docs/architecture/skill-mcp-plugin-control-plane.md)为准。
- Plugin、MCP、local-package、bundle、runtime attach/进程与真实服务不在该 source 合同内；不得把
  单 Skill 的 source 通过写成产品安装、MCP lifecycle 或 Science attach 已完成。

### P4｜语义收尾

- 对 test-only `runtime/transaction.rs`、operation vocabulary 和 dormant profile preset-sync 明确选择
  “接通”或“删除”；不能无限期以 `allow(dead_code)` 保存模糊意图。
- provider contract 适合抽共享 crate 或生成 schema；先定义单一解释 owner，再迁移两套 validator。

### 下游产品证据

只有 source closure 后并分别获得授权，才按 exact source → exact artifact → isolated-live →
authorized-live 推进；installed、signing/notarization 与 public release 仍是额外独立层。真实 API Key、
OAuth、Keychain、SSH key、账号数据库和真实 Science data-dir 不因本路线获得访问授权。

## 唯一立即 NEXT

P3 source 已闭合；唯一下一证据动作是从 `main@c678b1b` 构建并核验 exact artifact。完成前不得把
source gate 外推为 temporary/installed runtime、live Provider、Science、SSH、账号、signing、
notarization、Gatekeeper 或 public release；这些层当前全部仍为 `NOT-RUN`。artifact 通过后才按
production source → exact artifact → isolated-live → authorized-live 的顺序另行决定后续范围。

## 文档退役状态

- 审计基线的 167 份 tracked Markdown 中，没有一份满足“整份删除”的 lifecycle 条件。
- 11 个 tracked 兼容指针仍有有效调用者或 release 条件，继续保留；它们不是重复权威正文。
- 两份 ignored 临时 handoff 已因 HEAD/checkpoint 变化且任务落地而满足自身失效条件，已在本轮
  退役；Git 不跟踪它们，删除后不能从本仓库历史恢复。
- 旧 R3–R11、R4/R5、S7、Post-D0/Post-Q0 只是历史 audit/evidence 标签；后来文档中的 Phase 1/2/5
  也只表示其绑定 slice，不是当前 NEXT。

完整审计方法、文件分类、Science/CSSwitch 链路和发现见
[2026-08-14 全仓审计](../../docs/audits/2026-08-14-next-repository-docs-architecture-review.md)。
