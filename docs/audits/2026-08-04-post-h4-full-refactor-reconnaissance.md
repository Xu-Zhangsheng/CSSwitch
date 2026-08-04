# 2026-08-04 Post-H4 全量重构摸排

状态：日期化 source-only 架构与治理再基线

适用范围：本地 `next@41fb5f2360e51bbde75dd5c5dfc43e9fdcb084e6`

最后复核：2026-08-04（Asia/Taipei）

失效条件：doctor intent、one-click/history/boot consumer、config writer、quality lineage、
ChangeRecord 或 Science runtime adoption/provenance 发生实质变化时重新审计。

本文记录 H4 后从 production frontend、Desktop IPC/runtime、相邻 Skill/Gateway owner、
quality lineage 与 Science update 链重新得到的 source 事实。它不授权实现，不建立 artifact、
installed/live、provider/Science/SSH、签名、公证或公开 release 结论。可执行顺序只在本地
`.agents/handoffs/2026-08-04-post-h4-refactor.plan.md` 维护；本文不是长期计划正文。

## 1. 结论

H4 的 typed finalize consumer 仍然正确且 fail-closed；它关闭了 degraded outcome 被 manual UI
或 auto-boot 误报为 applied/Ready 的旧 HIGH。H4 没有缩小 one-click coordinator，也没有把
frontend 从跨 IPC 编排中移出。

扩大到完整生产入口和更新记录后，旧的“无新 HIGH，直接进入 O1-A”不再成立：

1. **产品信任 HIGH**：UI 把 `运行自检` 描述成不会改变 Skill/MCP，但 backend 会取得
   `HostBridge` mutation lease，强制 reconcile、失效或重写第三方 Skill route 状态；
2. **变更治理 HIGH**：机器 release lineage 仍停在 `v0.8.3 <- v0.8.2`，ChangeRecord 也没有绑定
   exact candidate、completion seal 与 release，无法满足“CSSwitch 的更新有记录”；
3. **架构 MEDIUM**：healthy/cold/recovery decision 仍晚于 SSH/stub/pending-cleanup preparation，
   frontend history restore 仍串联第二个 destructive IPC，cold coordinator、补偿、config CAS 与
   Science update provenance 仍未解耦。

因此新的唯一建议 NEXT 是 **D0 Doctor intent split**。D0 source closure 后必须停下再基线；
后续候选顺序为 Q0 CSSwitch source/release ledger、O1-A typed entry decision，再进入 history、
cold coordinator、durable compensation、config concurrency 与 Science update ledger。后继顺序
是候选，不是自动授权。

## 2. 当前调用与所有权图

```text
WebView intent
  -> ipc-client transport
  -> Tauri command
     -> auth/preflight + mutation lease + recovery orchestration
     -> runtime one-click coordinator
        -> entry/journal classification
        -> SSH + pending cleanup
        -> Science stop + AuthorityTransaction
        -> login/history + Gateway + Science launch/DB recovery
        -> route reconcile + UI open + finalize/compensation
  -> raw outcome
  -> second finalize-consumer IPC
  -> frontend publication
```

正确边界已经存在：`ipc-client.js` 是纯 transport；H4 classifier 是窄、脱敏、只读 projection；
`GatewayReceipt`、Science launch/stop receipt 与 `AuthorityTransaction` 是三类不同 authority，不能
合成万能事务。问题在于 entry owner、跨 IPC intent 和协调器的维护责任仍未收敛。

## 3. Findings

### 3.1 HIGH：doctor 的用户合同与实际 mutation 相反

- `desktop/src/index.html:180-182` 声明诊断动作不会改变 Skill 或 MCP 配置；
- `desktop/src/runtime-controller.js:293-306` 调用 `run_doctor`；
- `desktop/src-tauri/src/commands/diagnostics.rs:20-34` 在 shell doctor 后取得
  `RuntimeMutationDomain::HostBridge` 并执行 `force_third_party_reconcile`；
- `desktop/src-tauri/src/runtime/sandbox_session/route_reconcile.rs:213-266` 会失效 route marker，
  或对运行中的 Science 执行 configure/readback 并发布新状态。

一个被理解为只读排障的按钮实际拥有修复副作用，失败时也可能留下“host 已改、marker 未提交”
的局部结果。应拆成纯 `run_doctor_read_only` 与显式 `repair_skill_route` 两个 intent；D0 完成前，
不能继续把 doctor 当成只读诊断。

### 3.2 HIGH：CSSwitch source change 与 release 没有机器可验证的 promotion 边

- `quality/release-lineage.v1.json:3-14` 仍描述 `v0.8.3`，previous release 仍是 `v0.8.2`；
- `docs/evidence/releases/v0.8.4.md` 已记录公开 `v0.8.4`；当前 `next` 又比该 tag 多出新的 source
  changes，而 package/Cargo/Tauri version 仍是 `0.8.4`；
- `quality/schema/quality-kernel.v1.schema.json` 的 ChangeRecord 没有 base/candidate SHA、run/seal
  或 release link；production-path validator 只要求 changed path 被任一 active record 覆盖；
- H4 的 exact candidate、run 与 seal hashes 只在日期化 audit 中人工关联，仓库没有不可变的
  `SourceCandidateRecord -> ReleaseCandidate -> ReleaseEvidence` 机器链。

现有 ChangeRecord 适合表达意图、风险和测试影响，但不能独自证明“这次修改是什么、在哪个 exact
SHA 验证、后来进入了哪个 release”。Q0 应先修复 lineage，再为每个新 slice 建立不可复用历史
active record 的 exact source-candidate 绑定；CHANGELOG 仍只记录真实 release。

### 3.3 MEDIUM：entry decision 晚于 branch-specific effect

`desktop/src-tauri/src/runtime/sandbox_session/one_click.rs:2330-2347` 在判断 healthy/cold 前执行
SSH prevalidation、stub capture 和 pending cleanup retry；真正的 runtime probe/binding/login decision
从 `2350-2401` 才发生。healthy reopen 因而继承 cold/recovery 的失败面。O1-A 必须先建立只读、
不可变的 `EntryFacts -> EntryDecision`，并证明 decision 前没有 SSH/stub/cleanup/OAuth/Gateway/
Science effect。

### 3.4 MEDIUM：command 与 runtime 共同拥有 entry recovery

`desktop/src-tauri/src/commands/runtime/one_click.rs:143-267` 已经执行 config/Gateway snapshot、Codex
auth、destructive lease、finalize replay 与 interrupted-Gateway recovery；runtime coordinator 在
`sandbox_session/one_click.rs:2249-2329` 又重放 finalize 并解释 handoff/journal。业务 owner 跨两层，
新增 route 必须同时修改 command、recovery、coordinator 与 source-shape tests。目标 command 只保留
IPC、允许锁外的 auth preflight 和 projection；runtime entry facade 创建并立即消费 affine handoff。

### 3.5 MEDIUM：one-click 仍是 giant coordinator

`one_click_login_with_options` 跨 `sandbox_session/one_click.rs:2249-3292`，单函数拥有 entry、SSH、
cleanup、Science stop、authority snapshot、history/login、Gateway、Science launch/DB restart、route、
binding/finalize 和全链 compensation。已抽出的 domain receipt 是正确基础；下一步应按 branch 和
typed step 收拢 owner，而不是把 receipt 合并成统一事务，也不是只做机械搬文件。

### 3.6 MEDIUM：frontend 仍拥有跨 IPC destructive workflow

- `desktop/src/runtime-controller.js:79-95` 在 `restore_history_choice` 成功后自动调用
  `runOneClick(null)`；两次操作各自取得 destructive lease；
- history backend 在 `commands/runtime/one_click.rs:269-432` exact-stop Science 并改写历史身份，
  但只用进程内 `HistoryRecoverySession`，没有 durable intent/progress；
- WebView 关闭或第二次 IPC 失败会留下“历史已恢复、Science 已停止、未重新启动”的中间态。

长期合同是一个 frontend intent 对应一个 backend operation。最小修正是 restore 成功后保持停止，
由用户显式再次启动；若产品坚持单按钮，复合事务必须由一个 backend intent 与 durable journal 拥有。

### 3.7 MEDIUM：读模型与 boot publication 仍不对称

- `get_config` 会消费 `pending_notice` 并写回 config；UI refresh 因而可能制造 config drift；
- conditional auto-boot 对 failed/attention 有 event 和补读，Ready 只更新 backend `BootState`；如果
  首次 frontend config read 早于 finalize，缺少对称的 Ready publication；
- H4 classifier 本身仍正确，这些是 consumer transport/read-model owner 的后续缺口。

后续应建立纯 `get_config_snapshot`、独立幂等 `ack_notice`，以及带 sequence 的统一 boot result
snapshot/event；不能让展示型 notice 参与 runtime candidate equality。

### 3.8 MEDIUM：durability 与 concurrency 仍有明确边界

- `RuntimeMutationLease` 和 config semantic CAS 主要是 process-local；跨进程 load/save 窗口仍可能
  lost update；
- `AtomicRollbackUncertain` 在 config writer 内部存在，但向 orchestration 被字符串化；H4 只能在
  事后 readback fail-closed，不能替代 typed commit outcome；
- cold compensation outcome 已 typed，但逐步 progress 没有持久化；多个 Science stop caller 仍持
  `AppState` 跨 shell/TERM/wait。

这些必须在 entry/branch owner 稳定后分开处理：typed writer outcome、single-writer/跨进程策略、
durable compensation replay 和锁外 stop 各自是独立 stage。

### 3.9 MEDIUM：Science snapshot identity 不是 update diff ledger

`runtime/science/executable.rs` 已安全校验固定来源并生成 SHA-256 内容寻址 snapshot；
`ScienceRuntimeIdentity` 和 managed launch receipt 只描述一个 identity，没有 predecessor、candidate
decision 或 adoption attempt。日期化 0.1.20 -> 0.1.25 调查不能替代通用机制。

未来 ledger 只允许记录脱敏 executable metadata、allowlisted CLI/route/capability digest、decision 和
launch/finalize milestone；不得读取或 diff 真实账号、组织、对话、project 或 opaque Science data。
已健康 daemon 仍不得因为发现新 candidate 被强制重启。

### 3.10 MEDIUM：Codex auth 是独立的 split operation，不应并入 O1

`desktop/src-tauri/src/commands/codex.rs` 的登录路径先在 destructive lease 中停止相关 runtime/
Gateway 并启动 auth sidecar，交互阶段在 Lifecycle lease 外继续，认证成功后再取得独立 intent lease
补建 profile。现有“授权已保存但 profile 补建失败”的 repair UI 已说明三段不是一个原子 runtime
transaction。O1-A 只应消费 opaque prepared-auth proof/snapshot；Codex sidecar、runtime stop 与 profile
repair 的 owner 收敛属于后续独立 lane。

### 3.11 LOW：文件体积不能代替 product reachability

`commands/skills.rs` 与 `skill_manager/**` 虽然体积较大，但当前未进入 Desktop module graph、Tauri
registration 或 production frontend。当前可达 Skill owner 是 mutation 侧的
`commands/skill_install.rs`、`skill-package`、Gateway bridge、`external_skill_route`，以及只读
read-model 侧的 `commands/skill_listing.rs`。这批 legacy/orphan 文件不进入本轮 runtime 重构；
未来处置前必须重新证明可达性并获得删除/迁移授权。

## 4. 目标边界

```text
Frontend
  -> one typed intent
IPC command facade
  -> decode / auth preflight / lease / result projection
Runtime entry facade
  -> pure typed decision
  -> exactly one branch coordinator
Branch coordinator
  -> narrow domain ports + distinct receipts
  -> durable checkpoints only where crash recovery needs them
Read model
  -> typed sanitized result / event / snapshot
Frontend
  -> render only; no transaction chaining or message-driven control flow
```

目标不是“一键只做一件底层 I/O”，而是一个用户 intent 只有一个 backend operation owner；内部每个
模块只有一种维护原因、输入输出 typed、receipt authority 不越界、失败能用 operation/phase 与脱敏
correlation 定位。

## 5. 权威文档影响

| 文档 | 当前判定 | 何时修改 |
|---|---|---|
| `architecture/desktop-control-plane.md` | H4 与 doctor mutation 事实准确 | D0/O1/F1 实现时更新 intent owner、command facade、boot/read model |
| `architecture/runtime-state-transactions.md` | H1-H4、receipt、process-local 边界准确 | O1/cold/config/compensation 各 stage 改变 owner 后同步 |
| `architecture/science-runtime.md` | 已明确 snapshot 不等于 ledger | Science ledger 实现后补 observation/adoption owner；不写具体版本结果 |
| `features/ui-information-architecture.md` | 本次候选已补充 doctor mutation 缺口与 H4 selected/applied/unknown 当前语义 | D0 实现后再更新为 read/repair 已分离的完成合同 |
| `operations/quality-kernel.md` | 描述现有 kernel，但 lineage/change promotion 能力不足 | Q0 schema/validator 落地时更新 |
| `operations/release.md` | source/artifact/release 分层正确 | Q0 增加 previous-release 同步、已发布版本禁止复用、promotion 校验 |
| `docs/audits/2026-08-04-post-h4-production-flow-rebaseline.md` | 对其窄 production-flow 范围仍是历史证据 | 保持原样，不增补成长期 roadmap |
| `.agents/context/known-issues.md` | 旧排序已被本次全量摸排取代 | 立即改为 D0 sole NEXT，并登记 Q0 与其余候选 |

## 6. 验证与停止边界

每个获授权 stage 都必须：

1. 从实时 worktree/HEAD 与 active records 开始；
2. 先冻结 source characterization 和允许/禁止范围；
3. focused tests、metadata/inventory/document governance 全部实际执行；
4. exact-SHA 15-suite `GATE-SOURCE` 与 clean-context independent review；
5. source、artifact、installed/live、signing、release 结论严格分层；
6. 更新对应 architecture/feature/operation 与当前 context；
7. 停下重新摸排，只选择一个新的 sole NEXT，不能自动执行本文后继候选。

本次只完成调查与治理文档，不修改产品源码，不执行 product/source completion gate，也不建立任何
新 release 结论。

## 7. 相邻模块分流

- **当前主线**：D0 doctor trust boundary、Q0 change/release ledger、随后经再基线授权的 O1；
- **后续独立 lane**：Codex auth/runtime/profile split operation；外部 Skill 的 file commit/attach
  recovery；doctor repair 的 snapshot/result correlation；
- **明确不并入 O1**：Gateway 协议转换、`skill-package` 内部机械拆分、legacy/orphan Skill Manager、
  frontend 的 GitHub release 查询；它们没有 H4 引出的共同 transaction owner。
