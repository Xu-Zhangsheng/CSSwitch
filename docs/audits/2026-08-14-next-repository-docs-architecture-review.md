# `next` 全仓文件、文档与 Science/CSSwitch 链路审计

状态：日期化审计；不替代当前路线、架构正文或实际 evidence

审计日期：2026-08-14（Asia/Taipei）

审计基线：最初 clean 的 `next@cfc4008a64d9ef41a9e4238507f85b52095f7c1f`

失效条件：HEAD、tracked 文件集合、production owner/caller、质量元数据、官方 Science surface 或
目标证据层改变时，受影响结论只保留为历史。

## 1. 结论先行

重构处于后半程，而不是已完成：核心 runtime owner、事务、恢复、Science host adapter、writer
fencing 与 durable compensation 骨架已建立；当前主要缺口已经从“大文件拆分”转为跨 owner 一致性、
非 one-click durable receipt、Science control primitive、Skill Phase 3 与证据治理。

当前 exact HEAD 不能声明 `SOURCE-GREEN`。canonical gate 两次在执行 suites 前停于 `SNAPSHOT`，
runner exit 12；低层诊断为 `SNAPSHOT_DIRTY`，但普通 Git status 是 clean。该结果是
`INCONCLUSIVE`，15 suites 为 `NOT-RUN`，不是产品代码测试 `FAIL`。

Science/CSSwitch 的主生产链已经能唯一落到 source owner；文档的主要问题不是“链路不存在”，而是
把旧 SHA/旧 run 写进稳定正文、把历史 0.1.25 artifact/live 误当成当前候选，以及漏记近期 authority
writer/replay 变化。

## 2. 范围与方法

- 机器枚举 595 个 tracked files；按目录、语言、质量目录和生产路径清单交叉核对。
- 对 167 份 tracked Markdown 执行治理分类：索引、当前 Context、稳定 architecture/features/operations、
  日期化 audit/evidence、compatibility pointer、fixture/reference。
- 以 Tauri registered surface、frontend caller、runtime owner、Gateway entry、Science host adapter、
  receipt/journal/recovery 和 provider dispatch 为轴，复核 production chain。
- 相对上一条完整 accepted source/test lineage `d2cf95e` 检查 21 commits、92 files、
  `+8710/-18724`；抽查所有权边界、dead/dormant surface、mutation inventory 和高风险大文件。
- 运行轻量 inventory / metadata / 文档检查；未构建 artifact，未启动 CSSwitch/Gateway/Science，未访问
  installed App、凭证、账号数据库、真实 provider 或用户 Science data。

“全仓排查”表示所有 tracked 文件进入机器 inventory 与治理/production-path 分类；它不声称逐字人工
审阅 595 个文件。人工深读集中在当前权威文档、最近 21 commits 的 production/quality 变更、实际
owner/caller、冲突元数据和体量热点。

## 3. 重构进度

| 状态 | 已确认内容 |
|---|---|
| 已完成 | 四个 Cargo 边界与 Gateway staging；统一 `AppState` / lifecycle serializer；typed Gateway stop；one-click transaction owner 抽取；旧 Skill Manager/profile-switch writer 删除；authority/managed-launch writer fence；durable per-target compensation replay；typed Science host adapter；Skill Phase 2 inspect-only |
| 部分完成 | `include!` 只完成物理分文件；one-click coordinator 仍跨多个 owner；Science 三套控制面 primitive 重复；provider contract 有单一 JSON 但有两套类型/validator |
| 未完成 | 当前 exact-HEAD source closure；set-mode/settings/Codex/profile durable receipt；Skill inspect→plan→confirm→apply/runtime；dormant/test-only surface 决策；quality truth reconciliation；当前 Science 0.1.27 compatibility 与所有下游证据 |

大文件本身不构成重构缺口。`commands/runtime/tests.rs`、`config.rs`、`commands/codex.rs` 与 protocol
parser 只有在能建立新的类型 owner、visibility boundary 或 failure contract 时才应继续拆分。

## 4. Science/CSSwitch 主链

```text
Desktop WebView runtime-controller
  -> registered Tauri IPC / setup / exit hook
  -> command preflight + RuntimeMutationLease
  -> one_click entry recovery / healthy-or-cold decision
  -> Gateway lifecycle owner + provider contract/catalog
  -> ScienceHostAdapter + bundled launch/stop scripts
  -> managed receipt / listener / runtime identity
  -> Science -> loopback Gateway -> provider
  -> binding + adoption ledger + finalize/read-model
  -> compensation/history/Gateway replay on interruption
```

这条链的稳定 owner 已能在
[Desktop 控制面](../architecture/desktop-control-plane.md)、
[运行时状态与事务](../architecture/runtime-state-transactions.md)、
[Science runtime](../architecture/science-runtime.md)和
[Gateway/provider 路由](../architecture/gateway-provider-routing.md)之间唯一导航。

仍需收敛的接口是：

1. Desktop bounded Science control runner；
2. skill-package 的 `claude-science url` process runner；
3. Gateway Science HTTP control；
4. `runtime/skill_install_bridge.rs` route reconcile 当前未使用统一 bounded subprocess primitive。

它们职责不同，不应合并成一个含糊 owner；共同的 deadline、process group、bounded output 与 typed
timeout 可以成为下一条有界 source slice。

## 5. 文档事实漂移与本轮处理

### 已修正的当前权威文档

- `.agents/context/verified-state.md`：把 current candidate 从 `d2cf95e` 更新为本轮审计的
  `cfc4008a`，并把当前 source gate 严格标为 `INCONCLUSIVE / suites NOT-RUN`。
- `.agents/context/known-issues.md`：从 475 行历史 tuple 堆叠压缩为唯一当前路线；历史 identity 只链接
  dated evidence。
- `docs/architecture/runtime-state-transactions.md`：移除稳定正文中的旧 Phase 5 run/hash/tmp-root，
  修正五条实际 transaction stop caller，并保留机制而非一次结果。
- `docs/operations/real-machine-acceptance.md`：移除已删除的 profile rollback caller，补 authority writer
  fence、managed receipt writer 与 durable replay 的验收映射。
- `docs/features/product-science-capability-map.md`：新增 `AUTHORIZED-LIVE` 层，区分 `a60c2ee` /
  `d74221e` / `18a67881` 历史 tuple 与当前 `cfc4008a` 未验证状态。
- Science probe、runtime、dependency 与总览：补 adoption card、managed receipt writer fencing、当前
  DNS/dial deadline 和 Skill Phase 2 inspect-only 边界。
- README 中英文：补齐 OpenCode Go、Grok 与 Gemini 内置模板名称。

### 尚未修改、必须进入后续有界任务

- mutation inventory 仍为 `requirements-open/pending`，但
  `CHG-RUNTIME-MUTATION-R0-H` 声称 confirmed closure。
- `BUG-083-ORPHANS`、`BUG-083-RC`、`BUG-083-RETRY`、`BUG-083-RUST-COVERAGE` 仍写
  `active/open-not-fixed`，其正文却描述 trusted source gate 已修复对应合同。
- Gateway recovery、Science reattach、tool-arg bug records 仍引用已不存在的 source paths。
- production templates 与 preview 仍显示 `0.8.1 limited`；必须按真实 capability gate 改成版本中性的
  精确说明，不能机械替换版本号。

这些 JSON 和产品 copy 会影响机器 gate 或用户承诺，需要单独 source review；本轮文档治理没有替它们
猜测最终 status。

## 6. Science 官方面与证据边界

2026-08-14 复核的官方 [Claude Science changelog](https://claude.com/docs/claude-science/changelog.md)
已列出 0.1.27。仓库最近完整的 exact artifact/adoption/installed/provider 证据绑定 Science 0.1.25；
因此当前 0.1.27 package、adoption、installed 和 authorized-live 全部是 `NOT-RUN`。

现有 2026-07-30 official/package crosswalk 保留其访问日和 0.1.25 边界，不回写成当前版本事实。
官方 capability ownership 可以指导 capability map，不能证明 CSSwitch 当前 source、指定账号、第三方
provider、artifact 或 runtime 可用。

## 7. 文档生命周期判定

167 份 tracked Markdown 中，没有一份满足整份删除条件。日期化 audit/evidence 保留原绑定事实；
fixture Markdown 是测试输入；reference 是外部固定参考；兼容指针继续服务旧入口。

以下 11 个 tracked compatibility pointers 继续保留：

- `docs/ARCHITECTURE.md`
- `docs/DEVELOPMENT.md`
- `docs/EXTERNAL_SKILL_INSTALL.md`
- `docs/RELEASE.md`
- `docs/SCIENCE_RUNTIME.md`
- `docs/features/codex-browser-login-models-implementation-plan.md`
- `docs/operations/quality-source-gate.md`
- `docs/references/CSNATIVE.md`
- `docs/release-evidence-v0.4.2.md`
- `docs/upgrade-and-rollback.md`
- `test/REAL_MACHINE_TEST.md`

没有新的 public release 使版本指针满足删除条件；quality source-gate pointer 仍被当前 quality metadata
引用。

两份 ignored/untracked 临时 handoff 的 HEAD/checkpoint 已变化且任务已落地，满足各自失效条件：

- `.agents/handoffs/2026-08-07-refactor-real-chain-evidence.md`
- `.agents/handoffs/skill-mcp-plugin-control-plane.md`

本轮按用户的文档退役要求删除。它们未被 Git 跟踪，不能从本仓库历史恢复；耐久事实已经进入当前
Context、architecture、operations 与本审计。

## 8. Source gate 观察

在文档改动前、普通 Git status 为 clean 的 `cfc4008a` 上，两次执行：

```bash
bash test/run_all.sh --output-root <fresh-short-mode-0700-root>
```

均返回 `runner_exit=12`、`stage=SNAPSHOT`、`reason_code=SNAPSHOT_FAILED`，没有生成 run manifest 或
suite observation。第二次 canonical run id 为 `a83b524c179717e58fb66571e41fa123`；使用同一 snapshot
authority 的只读低层诊断返回 `SNAPSHOT_DIRTY`。因此：

- source gate：`INCONCLUSIVE`；
- 15 suites：`NOT-RUN`；
- 产品/测试代码：不能据此判 `FAIL`；
- 当前 source：不能据此判 `PASS`。

下一任务应先把 drift 定位到具体 Git binding、tracked identity、ignore/untracked walk 或 gate contract，
再决定是否需要 source fix 或经授权处理工作树对象。

### 同日后续诊断

后续只读诊断确认：普通 `git status` 的 clean 不覆盖 ignored 路径，而 source snapshot 会对 checkout
中的目录/文件 identity 做 no-follow 稳定性检查。复用主 checkout 当时含约 74 万个 ignored build/runtime
路径；同一 `cfc4008a` 在零 ignored 的隔离 worktree 已通过 SNAPSHOT 并实际进入全部 15 个 suites。
因此最窄修复是不改 gate、不清用户数据，改在零 ignored 的隔离 worktree 上冻结并验收最终候选。
旧两次运行仍保持 `INCONCLUSIVE`，隔离诊断运行也不冒充后续最终候选的 completion seal。

## 9. 建议顺序

1. 只诊断并闭合 `SNAPSHOT_DIRTY`；停止于可复核根因与最窄建议。
2. 单独对齐 quality machine metadata、旧 bug paths/status 与产品 compatibility copy。
3. 在 clean exact candidate 上取得 15-suite seal + fresh clean-context review。
4. 做 Science control bounded primitive slice。
5. 做非 one-click mutation receipt slice。
6. 单独规划 Skill Phase 3；不要把 inspect-only 冒充完整控制面。
7. 只有另行授权才构建 exact artifact、运行 isolated/authorized live、检查 installed/signing 或发布。

当前唯一路线及每步完成条件见
[当前重构路线与证据缺口](../../.agents/context/known-issues.md)。
