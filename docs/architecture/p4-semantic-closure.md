# P4 语义收敛

状态：当前；已接受架构与施工序列

适用范围：从 `main@1bf0e8942f0e6b0f7a7b556823e1b38b92a71003` 出发的 Desktop/Gateway source 语义收敛；不包含 artifact、installed App、runtime、真实 Provider/Science/SSH、签名或发布。

失效条件：任一列出的 source owner/caller、provider catalog schema，或 `main` 基线在施工前发生实质变化；完成后耐久边界迁入相应 Architecture owner 并删除本文件。

本页已接受 P4 的唯一施工架构。它定义未来 source 改动的 owner、顺序与收口，不表示任何 slice 已实施，也不建立 source closure 或 artifact/runtime/live/signing/release 结论。

## 当前物理边界与决定

| 项目 | 当前物理 owner 与 caller | production reachable | 已接受决定 |
|---|---|---|---|
| 旧切换决策 | `desktop/src-tauri/src/runtime/transaction.rs`；仅 `runtime/mod.rs` 注册，内容与本文件单元测试均为 `cfg(test)` | 否；production 没有引用 | 删除文件、模块注册和其三项测试 identity；不把旧 scratch/rollback 文案重新接入当前 transaction。 |
| operation vocabulary | `desktop/src-tauri/src/runtime/operation.rs`；`OperationTrace`/timeout 常量由 profile validation、model discovery、one-click、scratch、Gateway lifecycle 与 status 消费 | 模块本体是；`ActivateProfile`、`UpdateActiveConnection`、`Commit`、`Rollback` 四个 enum value 不是 | 保留 trace、实际 stage 和 timeout owner；删除四个零 caller value 与其 `allow(dead_code)`，不新增“通用 operation coordinator”。 |
| preset-sync | `commands/profiles.rs` 的三个私有 helper 与 `runtime/profile.rs::build_preset_sync_preview`；调用仅为同文件 Rust tests，唯一前端字符串在 `desktop/src/preview-adapter.js` mock | 否；不在 `lib.rs` 的 Tauri handler，真实 frontend 无 caller | 删除 backend helper、preview builder、mock case 和相应 tests/metadata；不注册为新产品能力。 |
| provider contract | Desktop `provider_contracts.rs` 以 typed DTO 执行完整 12-id matrix；Gateway `provider_contracts.rs` 以独立 String DTO 执行 shape/selection/projection；双方各自 `include_str!`/hash 同一 JSON | 是；Desktop 生成 launch contract，Gateway 在启动时解释并执行 | 新建纯共享 crate `csswitch-provider-contracts`，作为 raw catalog 的唯一 parse、semantic validation、exact/unique selection 与 digest owner；Desktop/Gateway 只保留各自的 caller-specific projection，不再独立 validator。 |

同一字节 digest 只能证明双方读到同一 JSON，不能证明两套 validation/selection 语义相同。因此 provider 收口不得以保留两套 validator 为完成，也不得把 Gateway 的 environment/进程 policy 或 Desktop 的 profile/config policy 搬进共享 crate。

## 收敛后的 owner 图

```text
catalog/provider-contracts.v1.json
             |
             v
csswitch-provider-contracts
(parse + semantic validate + digest + selection)
       |                                             |
       v                                             v
Desktop profile/launch projection              Gateway runtime projection
profile/config policy                           environment/process policy
```

共享 crate 的输入仅是 embedded static JSON 和调用方给出的非敏感 selector；输出是已验证 catalog/contract、digest 或 typed validation error。它不得读取环境、Config、Keychain、network、文件、进程或启动 Gateway。Desktop 与 Gateway 各自继续拥有其 Duration/launch/health/protocol projection、错误投影及副作用。该 crate 自己拥有 unit tests，并以独立快速 `SUITE-RUST-PROVIDER-CONTRACTS` 纳入 source gate；这会将顶层 source suite 固定数从 15 增至 16，不让无关 crate 承担 provider schema 的 production 或 test owner。

## 施工序列

顺序是 S1 → S2 → S3 → S4。S1/S2 先删除不可达债务，彼此不依赖；S3 定义共享解释 owner 并先迁移 Desktop；S4 将 Gateway 迁入并删除两个 legacy validator。S3 到 S4 之间的双实现仅是迁移状态，不得宣称语义闭合；S4 完成前旧 Gateway parser 不可再扩展。

### S1 — 删除 test-only transaction 与未调用 operation vocabulary

风险：F1（删除没有 production caller 的 test-only branch；保留现有 trace/timeout owner）。

允许路径：

- `desktop/src-tauri/src/runtime/transaction.rs`（删除）
- `desktop/src-tauri/src/runtime/mod.rs`
- `desktop/src-tauri/src/runtime/operation.rs`
- `quality/changes/next/CHG-P4-DEAD-TRANSACTION-VOCAB.json`
- `quality/test-catalog.v1.json`
- `test/quality/fixtures/source_gate/expected_test_ids.v1.json`

施工：删除 `runtime::transaction` 的模块注册和整个 test-only 文件；从 `OperationKind` 删除 `ActivateProfile`/`UpdateActiveConnection`，从 `OperationStage` 删除 `Commit`/`Rollback`，以及匹配 arms/`allow(dead_code)`。不得移动或重命名 production `OperationTrace`、实际 stage 或 timeout 常量。

非目标：不改 runtime journal、`OneClickFailureKind`、frontend DTO、scratch/proxy lifecycle、profile-switch 语义或 artifact/runtime 测试。

接受条件：`rg` 证明没有 `runtime::transaction`、四个已删 value 或其旧 test identity；所有仍存在的 trace caller 编译；source identity inventory 与 test catalog hash 同步更新。最小检查：Desktop library 的 `runtime::operation` 及其直接 caller tests、quality metadata/inventory 的适用检查。冻结后只做一次该候选的聚焦检查。

### S2 — 删除 dormant profile preset-sync surface

风险：F1（未注册、无 production frontend caller 的私有 preview/write path 删除）。

允许路径：

- `desktop/src-tauri/src/commands/profiles.rs`
- `desktop/src-tauri/src/runtime/profile.rs`
- `desktop/src/preview-adapter.js`
- `test/profile_preview_mock.test.mjs`
- `test/test_profile_pin_contract.py`
- `quality/core-contract-matrix.v1.json`
- `quality/changes/next/CHG-P4-PRESET-SYNC-REMOVAL.json`
- `quality/test-catalog.v1.json`
- `test/quality/fixtures/source_gate/expected_test_ids.v1.json`

施工：删除 preset preview fingerprint、apply helper、对应 Rust/MJS tests 和 mock invoke case；把 structural contract test 改为证明该 command 未注册且字符串 surface 不再存在。同步从 core-contract matrix 的 profile-selection `registered_test_ids` 删除该 mock test，保留该行中仍存在的 selection tests；不以另一项 selection test 冒充 preset-sync coverage。

非目标：不改 `create_profile`、`update_profile_connection`、`set_active_profile`、selection-pending 或下一次 one-click apply 行为；不把 mock 变成产品功能。

接受条件：Tauri handler 清单、生产 frontend 与 mock 都不包含 `preview_profile_preset_sync`/`apply_profile_preset_sync`；已有 profile selection 的 intent-only invariant 仍由其 own tests 覆盖；quality metadata/inventory 可读。最小检查：profiles Rust module tests、`node --test test/profile_preview_mock.test.mjs`、`python3 -m unittest test.test_profile_pin_contract` 和适用 quality checks。冻结后只做一次该候选的聚焦检查。

### S3 — 建立共享 provider contract 解释 owner，并迁移 Desktop

风险：F2（新跨 crate owner 与 Desktop 的 typed data flow）。

允许路径：

- `desktop/provider-contracts/Cargo.toml`
- `desktop/provider-contracts/Cargo.lock`
- `desktop/provider-contracts/src/lib.rs`
- `desktop/src-tauri/Cargo.toml`
- `desktop/src-tauri/Cargo.lock`
- `desktop/src-tauri/src/provider_contracts.rs`
- `quality/changes/next/CHG-P4-PROVIDER-CONTRACT-CORE.json`
- `quality/test-catalog.v1.json`
- `quality/release-gates.v1.json`
- `test/quality/fixtures/source_gate/expected_test_ids.v1.json`
- `test/quality/source_gate/contracts.py`
- `test/quality/source_gate/runtime.py`
- `test/quality/validate_quality_metadata.py`
- `test/quality/test_source_gate_contracts.py`
- `test/quality/test_source_gate_runtime.py`
- `test/quality/test_quality_kernel.py`
- `.agents/rules/testing-and-evidence.md`
- `docs/operations/testing.md`
- `docs/operations/development.md`
- `docs/operations/real-machine-acceptance.md`
- `docs/operations/release.md`

施工：新 crate 接管 schema v1 DTO、deny-unknown parsing、12-id matrix、digest、`template_id + api_format` exact selection 和 adapter unique-selection；其 public API 返回共享 typed contract，不能返回 raw JSON，并在 crate 内拥有 pure unit tests。注册快速、offline、repo-root 的 `SUITE-RUST-PROVIDER-CONTRACTS`：`cargo test --offline --manifest-path desktop/provider-contracts/Cargo.toml`；将它精确加入 source selection、`GATE-SOURCE.required_suite_ids`、source suite order、Cargo lock snapshot、identity fixture，以及验证这些固定顺序/计数的 named tests。同步把 source contract 文本中的“15-suite”改为“16-suite”。Desktop `provider_contracts.rs` 缩为 compatibility adapter/re-export 和 Desktop-specific projection，保留其现有 caller API，删除其中 parse/semantic validator；既有 callers 不得改动。Desktop 保持该 module 内既有 provider-contract/launch-plan behavior tests。

非目标：不改 `catalog/provider-contracts.v1.json`、provider 行为、Gateway parsing、网络、环境变量、timeout 数值、profile persistence 或 runtime wiring。Gateway 的 legacy parser 在本 slice 仅作受限迁移遗留，不能新增校验规则。

接受条件：Desktop 不再拥有 raw catalog parser/semantic matrix；`desktop/provider-contracts/src/lib.rs` 内的 unit tests 能拒绝当前 Desktop 已拒绝的 schema/semantic mutation，并为全部 12 contracts 给出同一 digest/selection；16-suite source contract 注册准确、独立 crate test 是 offline pure test，Desktop 的 launch-plan caller 经未改变的 compatibility API 消费共享结果。最小检查：`cargo test --offline --manifest-path desktop/provider-contracts/Cargo.toml`、`desktop/src-tauri/src/provider_contracts.rs` 内的 provider-contract/launch-plan tests、列出的 quality source-contract tests。若编译器显示需要改动本 slice 允许路径之外的 caller、test 或 module，停止该 slice，先重新定界 Architecture/Context；不得扩展 allowed paths。候选冻结后先用 [change checks](../../.agents/skills/csswitch-change-checks/SKILL.md) 选择并报告范围化检查，再进行一次 [code review](../../.agents/skills/csswitch-code-review/SKILL.md) 的 F2 首轮独立审查。

### S4 — 迁移 Gateway 并删除两套 legacy validator

风险：F2（Gateway 启动时的跨 crate contract projection；不改变其 process/network policy）。

允许路径：

- `desktop/gateway/Cargo.toml`
- `desktop/gateway/Cargo.lock`
- `desktop/gateway/src/provider_contracts.rs`
- S3 的 `desktop/provider-contracts/src/lib.rs`（仅为迁移中发现且已有 Desktop/Gateway 共用的 pure API）
- `test/test_gateway_rust.py`（仅为已重建 Gateway 的 provider loopback coverage）
- `quality/changes/next/CHG-P4-GATEWAY-PROVIDER-CONTRACT-MIGRATION.json`
- `quality/test-catalog.v1.json`
- `test/quality/fixtures/source_gate/expected_test_ids.v1.json`
- `docs/architecture/gateway-provider-routing.md`（仅在迁移后提炼“共享 crate 是单一解释 owner、两个 projection 的边界”）

施工：Gateway 删除自身 raw DTO、`parse_catalog`、独立 shape validation、digest 与 selection；改为消费共享 validated contract，并仅把它投影为 Gateway `ProviderRuntimeContract`/`CodexRuntimeContract` 和现有 managed identity policy。`provider_contracts.rs` 保留其现有 caller API，因此 Gateway callers 不得改动。保留 Gateway 的 environment reads、managed-id/digest completeness、adapter mismatch、Duration conversion、Codex transport/client 与 server behavior 在 Gateway owner 内。为 exact-id managed launch、unique standalone adapter、cross-adapter rejection、Codex projection 和 Desktop/Gateway shared mutation parity 写入该 module 的 tests。

非目标：不改 Gateway wire protocol、HTTP policy、auth/OAuth、cache semantics、provider catalog 内容、Desktop profile semantics、artifact、runtime/live/installed/signing/release。

接受条件：仓库只剩 `csswitch-provider-contracts` 解释 raw catalog；Desktop/Gateway 不再各自 `include_str!`、定义 raw provider catalog DTO 或 semantic validator；两者对同一 catalog 的 digest、exact selection 和拒绝结果一致；Gateway 经未改变的 compatibility API 保持既有 managed identity fail-closed behavior。最小检查：`desktop/provider-contracts/src/lib.rs` 与 `desktop/gateway/src/provider_contracts.rs` 内的 tests，以及依赖已重建 Gateway 的 `test/test_gateway_rust.py` provider loopback test；适用 quality metadata/inventory checks。若编译器显示需要改动本 slice 允许路径之外的 caller、test 或 module，停止该 slice，先重新定界 Architecture/Context；不得扩展 allowed paths。候选冻结后先用 [change checks](../../.agents/skills/csswitch-change-checks/SKILL.md) 选择并报告范围化检查，再做 F2 首轮独立审查；修复后最多一次最终复审。

## 施工与收口节奏

每个 slice 由新的 Terra High executor 在干净、目标基线的独立 worktree 中实施；primary 只在冻结候选后按本页验收。WIP 期间不因每一个小改动旋转 SHA、运行完整 source gate 或重复启动 reviewer。每个 slice 先完成 allowed paths 内的改动、冻结候选，再一次性运行其最小 targeted checks。

S1/S2 是 F1：不启动正式独立 code review。S3/S4 是 F2：冻结后使用 [change checks](../../.agents/skills/csswitch-change-checks/SKILL.md)，再按 [code review](../../.agents/skills/csswitch-code-review/SKILL.md) 的“首轮 + 修复后的最多一次最终复审”收口。四个 source slice 与其 quality metadata 合并为明确的最终 source candidate 后，才按[自动测试](../operations/testing.md)运行一次 exact-candidate recursive 16-suite `GATE-SOURCE`；任何 focused PASS 都不得称为 `SOURCE-GREEN`。

本页本身只做文档治理检查；它不运行完整 source gate，也不产生 artifact/runtime/live/signing/release 结论。S4 接受后，应将稳定的 provider owner 边界提炼到 [Gateway 与 provider 路由](gateway-provider-routing.md)，删除本页并更新架构索引；一次性测试结果则进入日期化 evidence，而不是架构正文。
