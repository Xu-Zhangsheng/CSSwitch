# 运行时状态与事务

本文回答“谁拥有运行状态、怎样持久化、锁序是什么，以及启动/切换/恢复/停止失败怎样补偿”。Science executable/data 身份见[Science runtime](science-runtime.md)；command/DTO 见[Desktop 控制面](desktop-control-plane.md)。

## 状态所有权

| 状态 | Source of truth | 持久性 |
|---|---|---|
| Gateway child、launch ID、key fingerprint、launch context | Tauri `AppState` | 进程内 |
| Science runtime identity、confirmed-stopped token、boot/history refs | Tauri `AppState` | 进程内；当前产品不保存 daemon child |
| Science version probe cache | `AppState.science_version_cache` | 进程内缓存；不等于 daemon/runtime identity 或 adoption record |
| Science executable observation / adoption | private `science-runtime-adoption/ledger.v1.json` | owner-only、bounded、no-follow、atomic/CAS；只含 allowlisted metadata 与 decision/milestone |
| pending authority cleanup retry set | `AppState.pending_authority_cleanup` | 进程内镜像；跨重启权威是 private pending-cleanup manifest |
| profile、active selection、端口、mode、SSH/Codex 设置、path secret | CSSwitch `config.json` / `Config` | 持久 |
| last healthy binding | `Config.runtime_binding` | 持久；只含公开 identity/hash 与可选的 32-hex Science adoption attempt id；旧值缺失 id 时不能授权 adoption finalize |
| in-flight runtime transaction | `Config.runtime_transaction` / `RuntimeTransactionRecord` | 持久；one-click、history recovery 与 interrupted-Gateway recovery writer 写 typed V2；历史 profile-switch V1/V2 journal 只保留兼容读取与恢复，不再有 profile-switch writer |
| in-flight one-click compensation | `Config.runtime_compensation` / path-free `RuntimeCompensationJournal` V1/V2 | 持久；V1 只兼容读取并阻断 mutation；当前 V2 只含 opaque compensation id、目标/fingerprint、受管 snapshot ticket、aggregate state、五个 typed step state 与最多两个 adoption attempt retention id；与 `runtime_transaction` 分离 |
| Science protected state rollback | private authority snapshot + manifest | 持久到 success/完整补偿/人工处置 |
| Science managed launch | stable path `science-managed-launch.v1.json` + live listener identity | schema v2 绑定 source/version/adoption attempt；schema v1 只读兼容且 provenance unknown |
| virtual login | Science credential files + CSSwitch `virtual-org.v1.json` marker | 分属 Science/CSSwitch |
| Skill ownership | Skill 内 `.import-origin` | 单包持久 |
| Skill bundle | CSSwitch bundle manifest/journal/locks | 跨操作持久 |
| SSH bridge | Science `config.toml`、CSSwitch sidecar、V2 stub | 分属 Science/CSSwitch |
| Codex auth/model generation | CSSwitch Gateway 私有文件 | 持久并带 lock/generation |

任何单一文件、端口或内存字段都不能独立证明完整运行身份。

## 当前源码 owner

| 状态或事务边界 | 当前源码 owner |
|---|---|
| `AppState` / `SharedLifecycle` 类型与进程级组合 | `desktop/src-tauri/src/lib.rs` |
| command 级 mode/settings/stop/显式 quit 串行编排 | `commands/runtime/lifecycle.rs` |
| macOS native exit 的 terminal cleanup 与进程退出投影 | `desktop/src-tauri/src/lib.rs::cleanup_for_exit_with` / `cleanup_for_exit` / `run_native_exit_event` |
| process-local Science stop 的 owner claim、锁外 execute/wait 与结果 CAS | `commands/runtime/lifecycle.rs::execute_process_local_science_stop_with` |
| 一键 IPC、锁外 auth preflight 与 UI failure 投影 | `commands/runtime/one_click.rs` |
| one-click entry/recovery policy、protected projection、healthy / cold branch dispatch、success-finalize replay/effect/read-model/failure glue | `runtime/sandbox_session/one_click.rs` |
| one-click durable-journal identity 与 transition | private `runtime/sandbox_session/one_click/transaction.rs`；只由 `one_click.rs` 的受限接口重导出给 sibling owner，wire schema 仍在 `config.rs` |
| mutating cold/recovery 的 prior stop、SSH、authority、Gateway、phase dispatch、route 与 finalize 顺序编排 | `runtime/sandbox_session/one_click/cold.rs` |
| cold path 的 managed Science launch、health、DB reverify 与 bounded restart phase | `runtime/sandbox_session/one_click/cold/science_phase.rs` |
| cold failure 的 aggregate compensation outcome 与五个 top-level effect 执行顺序 | `runtime/sandbox_session/one_click/cold/compensation.rs` |
| protected snapshot 合同、capture 与 restore | `runtime/sandbox_session/authority_snapshot.rs` façade及其 `authority_snapshot/` 片段 |
| one-click authority capture、verified ticket、restore 与 cleanup 接口 | `runtime/sandbox_session/authority_transaction.rs` |
| history exact stop、snapshot、credential publication、finalize 与 resume handoff | `runtime/sandbox_session/history_recovery.rs` |
| healthy daemon reopen 的独立补偿分支 | `runtime/sandbox_session/one_click/healthy_reopen.rs` |
| Gateway recovery/reuse/spawn/stop 与 typed acceptance receipt | `runtime/proxy_lifecycle.rs` façade及其 `proxy_lifecycle/controller.rs`、其他片段 |
| Science executable、runtime identity、launch/health、managed receipt 与 stop | `runtime/science.rs` façade及其 `science/host_adapter.rs`、其他 `science/` 片段 |
| 持久配置、runtime binding 与 journal schema | `desktop/src-tauri/src/config.rs` |

这些路径是当前维护映射，不改变上表的 source of truth。`runtime.rs`、
`science.rs`、`proxy_lifecycle.rs` 和部分 `sandbox_session` 根文件是保持历史
module surface 与测试 identity 的 façade；状态所有权仍由 `AppState`、
`Lifecycle`、`Config`、receipt/manifest 与 live identity 的既有组合决定。

显式 `quit_app` 和 macOS native exit 是两条不同的 production path。前者通过
`commands/runtime/lifecycle.rs` 复用 `stop_all` 的 process-local owner claim、锁外 wait
和结果 CAS，只有完整停止成功才调用 `app.exit(0)`。后者由 `lib.rs`
的 Tauri `RunEvent::Exit*` 处理器直接编排 terminal cleanup；它不是 frontend invoke，
但 Science stop 已复用同一个 owner-claim / wait / CAS publication helper。native exit
仍忽略 Science stop failure 并继续关闭 Gateway；任何“退出链已统一”的结论都必须
区分共享状态 owner 与不同 terminal policy。

## one-click / restore 当前边界基线

one-click durable-journal identity 与 transition 的稳定 owner 是 private
`runtime/sandbox_session/one_click/transaction.rs`；wire schema/validator 仍由 `config.rs` 拥有，
根 `one_click.rs` 只通过受限接口向 sibling coordinator、recovery 和 tests 暴露所需能力。

本节只说明当前机制和实际 production path，不保存某次 Phase、SHA、run、hash 或临时 evidence root。
exact source closure 与下游证据从[已验证状态](../../.agents/context/verified-state.md)和
[日期化审计/证据](../audits/README.md)进入；任何后继 source 都不能继承旧 seal。

### 入口与五条实际路径

| 路径 | entry 与判定 | effect / transaction owner | 终点与不能外推的边界 |
|---|---|---|---|
| 手动 one-click entry | `desktop/src-tauri/src/commands/runtime/one_click.rs::one_click_login_cmd` 先把 one-click compensation 与 history recovery 重放到收敛，再捕获并复核 `OneClickEntryPreflight`；provider auth 在 `RuntimeMutationLease` 外完成，业务 entry 在 `Destructive` lease 内调用 | command 只拥有 pre-auth recovery/auth/failure DTO 边界；`runtime/sandbox_session/one_click.rs::one_click_login_entry` 是 runtime entry façade | command 不解释 journal outcome，也不启动 Gateway / Science；auth 失败前后仍会再次收敛可重放的本地 authority recovery |
| healthy reopen | `one_click_login_with_options` 通过 `capture_one_click_entry_facts` + `decide_one_click_entry` 同时证明 Science `RunningHealthy`、virtual login intact、desired binding 不要求 Science restart；分支在同一 Config writer lock 内复核 P2-A/P2-B admission 并发布 `ProfileSwitch/StartFormalGateway` durable intent，随后才允许 adoption、marker 或 Gateway effect | `one_click/healthy_reopen.rs::healthy_reopen_with_gateway_rollback` 独立拥有 Gateway ensure/catalog、binding commit、route best-effort、surface 与 config/Gateway rollback；success exact-CAS intent，rollback 先 exact-admit、保持 intent 完成 Gateway restore，最后才 exact-CAS 恢复 Config 并清 intent | 不捕获 `AuthorityTransaction`，不创建 one-click snapshot ticket 或 `RuntimeCompensationJournal`；失败只回滚该分支的 config/Gateway before-image，不能外推 cold compensation；崩溃或 rollback effect 失败保留 intent，由既有 interrupted-Gateway recovery 消费 |
| mutating cold start / restart | stopped，或 healthy 但 login / binding 不满足 reopen 时进入 `one_click/cold.rs::run_cold_one_click`；若 pending authority cleanup 实际被清除，entry 必须重采 facts 后重新判定 | cold coordinator 顺序拥有 prior Science durable stop、authority capture、SSH、Gateway/catalog、Science phase dispatch、route 与 success finalize；`cold/science_phase.rs` 拥有 managed Science launch/health/DB-restart phase；`cold/compensation.rs` 拥有五步 live compensation | 产生新的 `operation=one_click` V2 identity、verified snapshot ticket 与需要时的独立 V2 compensation；cold 并不接管 history credential commit，也不接管 interrupted-Gateway listener 的精确 stop |
| History restore-only / restore-and-resume | `history_recovery.rs::restore_history_choice_entry` 从一次性 reference 开始，以 `operation=history_recovery` V2、完整 Config authority fingerprint、typed quiescence 与 protected snapshot 完成 credential publication | restore 与 interrupted restore replay 由 `history_recovery.rs` 独立拥有；四项 history private manifest、`acquire_runtime_history_effect_lease`、complete-record CAS 和 durable restore outcome 不属于 one-click compensation | restore-only 清 journal 并保持 stopped；以后点击 one-click 是新 destructive operation。restore-and-resume 只在同一 IPC / destructive lease 内发布 `ResumeAfterHistoryRestore` terminal handoff，随后仍先清 History record，再进入现有 one-click owner |
| interrupted-Gateway recovery | `one_click_login_entry` 每轮重采 facts 后调用 `proxy_lifecycle::recover_interrupted_gateway`；只接受旧 profile-switch V1，或 `operation=profile_switch` 且处于 `StartFormalGateway|RecoverInterruptedGateway` 的 V2 | `runtime/proxy_lifecycle/recovery.rs` 拥有 listener proof、pending/outcome complete-record CAS 与 stop；runtime entry 只保留不可序列化的 affine terminal handoff | terminal record 保留到 healthy/cold 的首个接管点；ordinary one-click 不能伪造 expected record。该路径不恢复 prior Gateway，不处理 one-click/history snapshot，也不是当前 profile-switch writer |

entry recovery 的真实次序是：V2 compensation replay → interrupted history replay → exact History
resume handoff → success-finalize replay → pending-cleanup replay → interrupted-Gateway recovery →
healthy/cold route。每个有 effect 的 recovery 后都重新载入 `Config` 再决定下一步；因此任一旧
handoff、旧 facts 或旧 listener observation 都不能跨 effect 直接复用。

### `one_click.rs` 仍承担的 owner

`runtime/sandbox_session/one_click.rs` 仍是 production/test 混合的根 façade/coordinator，
不是只做 re-export 的纯 façade。当前 source 已将 durable-journal identity/transition
物理闭合到 private `one_click/transaction.rs`；根文件保留的 owner 如下：

| owner 类别 | 当前符号 / 路径 | 判定 |
|---|---|---|
| façade | `OneClickEntryPreflight::{capture,verify_unchanged}`、`one_click_login_entry`、`one_click_login_with_options`，及对 `transaction` 受限接口的 re-export | 合理保留 command → runtime 与 entry → branch 表面；durable transaction 本体已不在根文件，根文件仍有 coordinator、effect 与 recovery policy |
| coordinator | `one_click_login_entry` 的 recapture/decide/effect loop、`one_click_login_with_options` 的 healthy/cold dispatch | entry coordinator 与 branch coordinator 已逻辑分开；cold 顺序 owner 已在 `one_click/cold.rs`，根文件仍直接协调四类 recovery 与两条业务 branch |
| durable transaction | private `one_click/transaction.rs` 的 `OneClickTransactionIdentity`、`OneClickJournalProgress`、prior-stop / checkpoint / finalize transition 与 compensation step CAS helpers | owner 已物理移动；`config.rs` 仍拥有 wire schema/validator，History 仍有自己的 transaction/finalize，private replay manifest 与 live/fresh compensation effect/replay owner 均未移动 |
| effect | 根文件的 `open_science_surface`、`restart_science_identity_with_budget`、`capture_authority_after_science_quiesce` 与 DB health helpers | 与 `cold/science_phase.rs`、`ScienceHostAdapter`、`AuthorityTransaction` 的 effect owner 交叉；这些 helper 有真实 I/O、进程或 `AppState` publication，不是 façade-only glue |
| recovery | 根文件的 `replay_interrupted_one_click_finalize`、History terminal handoff consume、V1 interrupted-Science validation；同时 entry 调用 sibling compensation/history/Gateway replay | success-finalize replay/policy、顺序与 typed error conversion 仍在根文件；durable effect 本体分别仍位于 `one_click/compensation_replay.rs`、`history_recovery.rs`、`proxy_lifecycle/recovery.rs` |
| read-model | `capture_one_click_entry_facts`、`decide_one_click_entry`、`history_resume_handoff` | 这是控制流 read-model，只为 branch/recovery eligibility 服务；最终用户 publication 的只读权威回读属于 `runtime/finalize_consumer.rs::project_finalize_consumer_state`，不得与 entry facts 合并 |
| failure projection | `typed_one_click_err`、`typed_interrupted_gateway_recovery_error`、`typed_authority_cleanup_err`、`OneClickFailure`；cold compensation 生成 `ProjectedRecovery` | produce-site kind 在 runtime 内标注；最终 DTO 属于 `commands/runtime/one_click.rs::project_one_click_failure`，History resume 的失败可见性由 `history_recovery.rs::project_resume_failure` 再附加 `history_recovery.status=restored`。根文件不独占完整 projection owner |

因此，本轮只闭合 one-click durable-journal identity/transition 的物理 owner；不是删除 dead
writer，也不是合并 transaction/recovery policy。任何后续移动都必须保持 façade、业务 transition、
durable recovery effect 和 consumer read-model 四类责任可分别测试，不能用“文件过长”作为合并事务的理由。

### restore 与下一次 one-click 的一致和断裂

| 维度 | 一致性 | 明确不一致 |
|---|---|---|
| 事务身份 | 两者都使用 `config.rs::RuntimeTransactionV2`、非空 transaction id、typed operation/phase 与完整原记录 CAS | History 的 `runtime_fingerprint` 是“移除 journal 后的完整 Config authority”SHA-256；one-click 是 candidate Science environment fingerprint。restore-only 清除 History id，未来点击创建新 one-click id；显式 resume 也先消费并清除 terminal History record，再由 one-click 创建新 id |
| checkpoint / CAS | `history_recovery.rs::update_history_record` 与 `one_click.rs::write_one_click_checkpoint` 都要求磁盘完整 record 等于 expected，漂移即保留并 fail closed | History 额外要求完整 Config authority fingerprint 一直相等；one-click 以 active profile + previous binding + immutable candidate/snapshot/prior-stop identity 为 authority。两套 CAS predicate 不能互换 |
| snapshot authority | 两者都通过 `AuthorityTransaction::capture` 取得 registered `RuntimeSnapshotTicket`，success 走 cleanup-only / cleanup retry，失败保留 registered recovery | one-click rollback 使用完整 protected projection 与 `RuntimeTransactionRestoreExpectation`；History 在 credential-write crash boundary 另持久化 encryption key、OAuth tokens、active org、virtual marker 四项 private manifest，并用 history-specific pending/succeeded phases 重放 |
| compensation journal | open marker 都由 `Config::has_open_runtime_journal` 阻断普通 mutation | 只有 one-click 写独立 `RuntimeCompensationJournal` V2、固定五步与 live/fresh replay manifest；`validate_runtime_transaction_v2` 明确要求 History `compensation=not_started`。History 用自身 V2 phase + history effect lease，不借用五步 compensation |
| blocking / replay | command 在 provider auth 前收敛 replayable compensation/history，runtime entry 再复核；V1、invalid、retargeted 与 typed incomplete 均保留证据并 fail closed | one-click V2 compensation 可逐步 fresh-process replay；History 只自动处理规定的 stop/snapshot/write-pending/restore-pending/restore-succeeded phase；`HistoryCredentialPublished` 交给 finalize replay，`ResumeAfterHistoryRestore` 只允许 exact terminal handoff消费 |
| success finalize | 两者都用 `RuntimeFinalizeState::Intent`，authority cleanup 后以 exact record CAS 清 journal；中断由 `replay_interrupted_one_click_finalize` 收敛 | one-click `CommitBinding` 还要求 binding/runtime/V2 managed receipt/adoption attempt 四方一致；History 只允许 `ClearJournal|ResumeOneClick`，禁止 commit runtime binding |
| failure / History 可见性 | open journal 使 `project_one_click_failure` 与 `finalize_consumer` 保持 manual/selection pending，不从 message 猜真实状态 | History credential durable commit 后，resume one-click 失败不会回滚用户选择；`restore_history_choice_entry` / `project_resume_failure` 保留 `history_recovery.status=restored` 与轮换后的 choices。restore-only 的 consumer disposition 是 attention，不是假装 runtime ready |

这意味着“restore 后下一次 one-click 一致”只成立在共享 schema、CAS 纪律、snapshot ticket 和
fail-closed readback 层；不成立在 operation identity、fingerprint、compensation journal、effect
manifest 或成功语义层。不能把 restore-and-resume 称作一个覆盖 credential 与 runtime 的统一事务。

## 锁序与并发

跨命令变更遵守固定顺序：

```text
RuntimeMutationLease(Intent | Destructive | HostBridge | Terminal)
  -> Lifecycle mutex
  -> AppState mutex
    -> config::update mutex
      -> persistent config writer fence
```

- `RuntimeMutationLease` 要求会改变 runtime context 的 production operation 先声明
  intent、destructive、host-bridge 或 terminal domain；四个 domain 复用现有
  `Lifecycle` mutex，保持 process-local 互斥与不可重入语义，而不是四把可并行锁；
- `stop_all`、切换到 official 的 `set_mode`、需要 teardown 的 `set_settings`、native exit 与 downgrade cleanup 先在锁内冻结 generation 与 Science runtime/confirmed-stopped/child/port/URL owner snapshot，锁外取得 exact stop request；共享 transaction executor 在 probe 后、effect 前再次按 generation + 完整 owner identity 复核，随后继续锁外执行 stop script、TERM/KILL 与轮询等待，最后在锁内 CAS 发布结果。陈旧 `set_mode` / `set_settings` 结果不会停止 replacement Gateway 或提交 mode/settings；`set_settings` 只在 current stop success 后按原顺序 bump generation、停 Gateway、撤销 SSH artifact 并提交设置。native exit 的陈旧结果也不得清 replacement Science，但其 best-effort policy 仍继续停 Gateway。downgrade 先 bump generation；陈旧 Science 结果保留 replacement、仍按既有 terminal stop-all policy 停 Gateway，并在 export、backup 或 v2 publication 前失败。transaction-scoped Science stop 的中性 owner/CAS 模块覆盖 cold prior stop、managed DB restart、history recovery prior stop、live compensation cleanup 与 fresh-process compensation replay cleanup；各事务仍各自拥有 durable intent、顺序、补偿与 outcome。replacement、generation drift 或 stop failure 都 fail closed，陈旧结果绝不清除或覆盖 replacement。Gateway reuse 先在 `AppState` 下冻结 generation 与 child PID、端口、secret、provider、gateway/shim、launch id、key fingerprint 和完整 launch recipe，锁外执行 HTTP health，再按 generation + 完整 owner identity CAS 接受结果；陈旧结果 fail closed，不能清理或覆盖 replacement Gateway。Gateway spawn 同样只在 `AppState` 下冻结 generation、空 slot、secret、完整 candidate owner 与 launch recipe；candidate log、命令与环境构造、Skill bridge 配置 staging、`Command::spawn()` 和 health poll 都在锁外，再按 generation + 完整 candidate owner CAS 接受 child。generation 漂移或 replacement 已出现时停止 candidate，绝不覆盖 replacement；无法确认退出的 child owner 移交独立 registry。typed `GatewayStopOutcome::Uncertain` 必须由 destructive caller 消费，并在 config、credential 或 binding commit 前 fail closed；
- `Lifecycle.generation` 使锁外 probe 在 stop/clear/switch 后失效；
- transaction-scoped executor 只负责 process-local owner/wait/CAS，不改变各事务的 durable
  顺序：cold prior stop 保留 `PriorStopIntent` → exact stop → typed outcome；history recovery
  保留 durable intent/effect/outcome 与 effect lease；live compensation 和 fresh-process replay
  各自保留 step intent、effect、outcome、lease、crash recovery 和幂等收敛顺序。status/read model
  在锁外 stop wait 期间仍可取得 `AppState`；每个边界的 replacement、generation drift、stop
  failure fixture，以及 compensation/replay 的 crash/idempotence fixture，共同约束该合同；
- `config::update` 的进程内 mutex 只覆盖 load-modify-save；所有可能发布 canonical
  config、迁移、降级或滚动备份的公开入口还会在 pinned config 目录内取得同一
  `.config.writer.lock` advisory fence，再执行 load / modify / save。lock inode 持久保留、
  `0600`、single-link、no-follow，并在取得后复核目录项 identity；因此两个 CSSwitch
  进程不会从同一旧 snapshot 各自提交覆盖。纯 `load_current_from_read_only` 不创建或取得
  writer fence；
- config 文件提交继续使用 pinned/no-follow 边界、临时文件、rename、fsync、提交前复核与
  回滚；writer fence 只收敛 cooperating config writer，不把 sibling multi-file operation
  升级成共同事务，也不替代 expected-record CAS。

Skill bundle、Codex auth 与 SSH bridge 还各有局部锁/CAS/sidecar 事务。生产的本地
Skill 安装在文件选择前捕获 `ScienceHostContext`，picker 保持在 lease 外；选择完成后
取得短 `HostBridge` lease，在 lease 内重新探测并构造 matching typed
`LocalSkillHostReceipt`，随后用同一 receipt 完成 package commit 与 OPERON
attach/readback。attach 失败仍保留已提交文件并分别报告，不新增 durable runtime journal。

`ScienceHostAdapter` 是当前 macOS Rust + shell host 边界，不是 host-neutral extension。
typed `ScienceLaunchSpec` 只携带已选择的 runtime、端口、proxy/SSH/opaque binding 与既有
budget；adapter 内部编码 shell argv 和 allowlisted environment，并依次投影 environment
exposure、script acceptance、health、listener/runtime identity、未提交 ownership 与 durable
managed receipt。one-click/coordinator 继续拥有 authority revalidation、SSH observation、
`AppState` publication、DB reverify 与补偿顺序，但不解释 shell exit code 或自行重建 host
identity。stop 继续返回既有 `ScienceStopOutcome`，Rust proof 与 shell fail-closed 防线均保留。

`AuthorityTransaction` 是 one-click coordinator 使用的 authority façade。它把 protected
snapshot capture、已登记 `RuntimeSnapshotTicket` 复核、restore 与 typed pending-cleanup/commit
接口收拢到同一表面；底层 `OneClickAuthoritySnapshot`、`AuthorityTreeSnapshot`、manifest/CAS、
owner/mode/device/inode/tombstone 与 bounded remove 合同不变。成功路径先持久化
`RuntimeFinalizeState::Intent`，再把匹配 manifest 从 `ActiveRecovery` 转为 `CleanupOnly`，最后按
完整 V2 record、active profile 与旧 binding authority 的同一 CAS 原子提交 binding 并清 journal。
fresh process 会重放同一 finalize intent：
带 Science adoption attempt id 的 `CommitBinding` 必须先 fresh probe 当前健康 runtime，并从 exact
V2 managed receipt 回填同一 id；只有 action、binding、receipt、runtime 四方 id 相同且 receipt 在
authority cleanup 后仍为 current，才允许提交 binding / 清 journal。receipt 缺失、V1、非法、不同
attempt 或 runtime identity drift 均失败关闭并保留 journal。通过该 provenance gate 后，
若 manifest 仍为 `ActiveRecovery` 则先精确转换；若已经是 `CleanupOnly` 则直接沿 cleanup retry
合同继续；转换发布或最终 binding/journal 原子提交失败均返回 degraded 并保留 finalize journal，
不进入旧 compensation，下一次 production command 会先重放再进入 healthy reopen。空的
`CleanupOnly` manifest 是 authority 已完成的 durable evidence；manifest 完全缺失时拒绝 finalize，
避免遗忘 recovery snapshot。两种 crash window 都不会退回 destructive
recovery。coordinator 仍拥有 operation trace、Gateway/SSH 顺序与唯一 success/failure dispatch；
专属 cold compensation phase 拥有 `CompensationOutcome` 聚合与 frontend DTO/text/recovery
projection。O1-E2 在首个补偿 effect 前以 path-free one-click identity 和当前完整 business record CAS 发布独立
`runtime_compensation V2 / in_progress` 及固定五步 pending plan；Science cleanup、SSH cleanup、authority restore、
prior Science restart 与 snapshot cleanup 各自只允许以完整 compensation record CAS 从
`pending -> in_progress -> succeeded|failed|skipped(cause)`。补偿进度同时只持有一个 expected business record：
authority restore 前只能是 active record；该 restore 的 outcome 写入点按实际 config 结果单向切换到 restored
record，随后所有步骤与 aggregate completion 都只接受切换后的单值，不能在两侧任选。任一步 intent 不能发布时不执行该步及后续 effect；
任一步 outcome 不能发布时停止后续 effect 并保留恢复快照。authority restore 精确恢复补偿前
`runtime_transaction` 时保留最新 step state；全部步骤完成后才清除，失败则从 typed step outcome 推导
`incomplete + failed_steps`。因此原业务 journal 与补偿 crash marker 可以同时存在。O1-E3 在 authority
capture 完成时把固定 tree plan、before-config 与 root/backup identity 写入同一 registered snapshot 内的
0600 私有 manifest；补偿开始前再把 candidate runtime、exposure、prior Science recipe、active business
record、精确 SSH stub before/candidate transaction、prior restart launch id 与 compensation id 写入第二份私有
manifest，随后才发布公开 path-free V2 intent。compensation publication 与 provider auth 通过独立的
shared/exclusive absence fence 关闭跨进程 marker race，不长期占用普通 config writer fence。fresh production
entry 的每个单步 replay 也持有该 fence 的 crash-releasing exclusive lease，并在锁内重新读取 canonical
config、决策、执行 effect、发布 outcome；因此两个 Desktop 进程不能同时执行同一 `InProgress` effect。唯一
live compensation funnel 在公开 intent 成功后、首个 step intent/effect 前取得同一 exclusive lease 并持有到
本轮 funnel 返回，因此 live owner 与 fresh owner 也不能交错执行同一步。

同一 inode 的 authority fence 也为 cooperating CSSwitch authority writers 提供 SH：当前覆盖 OAuth
virtual-login/history credential 写入、managed SSH stub 的补偿/撤销、SSH bridge prepare/revoke、Skill route
注册/route-state 以及 Skill bridge runtime key publish；EX owner 只能通过其借用期内、不可跨线程的 scoped
bypass 调用这些 leaf，不能靠进程全局的“正在 replay”标志绕过 flock。这个协作协议不把同 UID 对 0700 config
目录的非合作 `rename`/`unlink` 宣称为 path-level CAS；获取 SH 后的 identity recheck 遇到该类 drift 必须
fail closed。managed-launch receipt 的 write/clear 与 prior-restart/lifecycle 传播已纳入同一 SH writer
协议；EX replay owner 通过 scoped bypass 进入这些 leaf，避免嵌套 SH 自锁。该保证仍只覆盖 cooperating
CSSwitch writers，不声称对同 UID noncooperative pathname tamper 的 CAS。
replay owner 在 provider auth 前循环重采 config，校验 public record、private manifest、
snapshot ticket 与完整 business record，然后重放或观察一个 `pending|in_progress` step。若同进程补偿替换过
Gateway，私有 manifest 只保存其受管 health identity、path secret 与端口；fresh authority step 必须再以当前
打包 binary、uid、唯一 listener 和二次 health 复核精确停止该 candidate，不序列化或伪造 process-local
`GatewayReceipt`。authority restore 只恢复 durable filesystem/config authority，不把上个进程的 AppState/Gateway
child ownership 当作可恢复事实；完整 restored config 是“effect 已成功、outcome 未落盘”的幂等 commit marker。

B1a1 将 authority replay 初始私有清单升级为 immutable v2 tree plan。production AuthorityRestore 在读取
validated compensation manifest、v2 snapshot、exact journal/ticket/managed-id/port binding 后，仍在任何
step begin 或 filesystem/config effect 前 fail closed；因此 pending 与 in-progress journal 都不推进并保留
`ActiveRecovery`。Science typed quiescence、durable progress 与 per-target effect state machine 已落地：fresh
owner 逐项复核 port、managed receipt 与 manifest identity，并将每个 tree target 的 stage、tombstone、promotion、outcome
与 cleanup 边界持久化；effect 后、outcome 前的 crash 由下一 fresh production replay 从同一 target 收敛，已终结 target
重放为 no-op。这个 registered snapshot 同时覆盖 protected Science tree、sandbox state、CSSwitch runtime 和必须 absent
的 managed receipt，所以 one-click 的 sibling multi-file crash closure 已完成。
SSH cleanup 重放原 transaction 而不是按 marker 广泛删除；prior Science 使用 durable stop recipe、精确 absent
的旧 receipt 路径、
预分配 launch id 与 fresh runtime identity 重新建立，只有 receipt 的 launch id 精确相等才允许 fresh owner 认领。

任何 retarget、unsafe
identity、V1 或 typed `incomplete` 都保留 journal/snapshot 并转人工处理。

O1-E4 把同一 effect-owner 合同扩到 history full-snapshot restore。history live failure 与 fresh replay 在
`HistoryCredentialWritePending` 后取得共享的 crash-releasing cross-process exclusive fence，并在锁内重读 canonical
Config、精确复核完整 history record 与冻结 authority。owner 先 CAS 到 `HistoryAuthorityRestorePending`，幂等恢复
encryption key、OAuth tokens、active org 与 virtual-org marker 四项 manifest，再 CAS 到
`HistoryAuthorityRestoreSucceeded`；entry 中途崩溃仍重放整个 manifest。cleanup-only、精确 journal clear 与 cleanup
retry 只接受 durable succeeded，Config / record、Science quiescence、snapshot ticket 或 manifest drift 都保留证据并
fail closed。

normal mode/settings/profile/Codex auth/settings/downgrade mutation、selection/read-model 仍把任一 journal 打开
视为 blocked/manual；显式 `stop_all` / `quit` 与 native-exit
cleanup 仍是只减小运行态暴露的 terminal cleanup，不写 config/credential，允许在 marker 打开时停机。
V1 marker 仍可严格读取并阻断，但不会被生产路径升级、推进或清除。

`GatewayController` 是 formal Gateway 的 process-local façade。它保留既有 spawn/reuse、双层
health、catalog fingerprint、generation/write-back 与 child ownership 核心；reuse health 使用
process-local owner claim、锁外 HTTP 和 generation + full-owner CAS。旧 tracked child 清理同样在
`AppState` 下冻结 generation、PID 与完整 Gateway metadata，把 affine `Child` 移交给锁外 stop/wait，
成功只按 generation + full-owner cleanup marker CAS 清空身份，失败则只在 owner 未变化时恢复 child；
replacement 永不被覆盖。旧版 Python listener 的端口探测、`lsof` / `ps`、TERM 与退出轮询也在
`AppState` 外执行，TERM 前以 UID、PID、process start、command、script 与唯一 listener 完整复核；
identity drift fail closed。spawn 的 reservation 只冻结 generation、空 slot、secret、完整 candidate
owner 与 launch recipe；candidate log、命令与环境、Skill bridge staging、`Command::spawn()` 及 health
均在 `AppState` 外，随后按 generation + 完整 candidate owner CAS 接受。stale candidate 不覆盖
replacement；退出状态不确定的 child 由独立 registry 保留。controller 只在全部接受条件通过后返回
非序列化 `GatewayReceipt`。receipt 同时绑定 route、`Reused/Restarted`、health
identity、catalog fingerprint 与完整 `GatewayLaunchRecipe`；当 host context 来自健康的
remembered Science 时，recipe 保存该 effective runtime，而不是只复制 caller 的显式参数。
`AppState.gateway_launch_context` 与 receipt 使用同一 recipe。无 bundled caller 的 registered
`start_proxy` 已移除，Gateway 启动只保留在 cold、healthy reopen 与 interrupted-Gateway recovery 内部路径；S6
不改变这些 caller 的 lease、checkpoint、补偿、binding/journal commit、DTO 或可见文案。

三类 receipt/authority 不能合并成一个“统一事务”：

| 证明 | 建立的控制权 | 当前 consumer | 明确不拥有 |
|---|---|---|---|
| `GatewayReceipt` | process-local start/reuse 的 route、accepted health/catalog 与完整 recipe | cold、healthy reopen、interrupted-Gateway recovery caller | crash journal、rollback、binding commit |
| Science launch/stop receipt | executable/data-dir/listener/PID/process-start/runtime SHA 与 managed record 的 exact live ownership | prior/history/DB/compensation stop 与 fresh restart | authority tree before-image 或跨进程 lease |
| `AuthorityTransaction` | protected projection capture、verified ticket、restore、cleanup/commit | one-click coordinator | prior stop、Gateway/SSH、journal、DTO 与全局编排 |

authority snapshot 会捕获 managed receipt 文件的 before-image，但这不把
`AuthorityTransaction` 变成 Science live stop authority。长期控制权交接应使用小型、affine 的
process-local handoff；durable journal 只保存 crash recovery 所需的最小 identity/outcome，不能
把 receipt 全量序列化或让诊断 DTO 参与控制流。

## 非一键 destructive mutation receipt

非一键、会改变 runtime 或 durable profile authority 的 destructive mutation 使用独立的
`config-mutation-operation.v1.json` receipt 与 Config 内的
`config_mutation_operation` fence；它不是 one-click/history 的 `runtime_transaction`，也不
把三类 runtime receipt 合并成一个全局事务。当前七个 operation 是
`set_mode_official`、`set_settings_destructive`、`codex_auth_start`、`codex_auth_logout`、
`set_codex_network`、`clear_applied_profile_key` 与 `delete_applied_profile`。普通 Config
writer、P2-A journal 和运行时 journal 在 receipt 或 fence 任一存在时都 fail closed；即使异常态
只剩 receipt，backend admission 仍在同一 Config writer lock 内拒绝后续 mutation，不能靠普通
command 清掉 boot attention。one-click 在 auth preflight capture、destructive lease 内的 replay/route
入口以及最终 effect route 都用同一个 secure admission reader 复核 P2-A/P2-B active receipt、clearing
receipt 与 Config fence；任一存在时，在 Gateway/Science/provider effect 前拒绝。healthy reopen 还在同一
Config writer lock 内把该最终复核线性化为 durable `StartFormalGateway` intent，消除 reader 返回到
adoption、marker 与 Gateway ensure 之间的插入窗口；P2-B begin 要么先赢并使 intent publication 零 effect
失败，要么后到并被该 runtime journal 阻断。失败 rollback 同样在 Gateway stop/restore 全程保留 intent，
只有 Gateway recovery 成功且 Config authority 仍等于 rollback admission snapshot 时才清 journal；因此
Config rollback 与 Gateway restore 之间也没有 P2-B 插入窗口。只有持有精确 fence identity 的 scoped
writer 能提交本次 mutation。

receipt 只保存脱敏的 operation id、intent/config fingerprint、runtime plan、bounded effect
checkpoint、auth sidecar identity 与 terminal digest，不保存 token、API key、OAuth 内容、私有
路径或原始 Config。写入遵循 fence-first、create-new/no-clobber、bounded 64 KiB、fsync 与
exact CAS；成功终结的顺序是 terminal receipt、terminal fence、clearing tombstone、receipt
unlink，最后清除 Config fence。中断、receipt/fence 不一致、Config drift、替换 owner 或
cleanup 失败均保留 receipt/fence 并投影 `attention`，启动恢复不会猜测或静默清除。

每个真实 effect 都先以 exact CAS 持久化 `Pending -> InProgress` 和新的 `attempt_id`，成功后才执行；
effect 返回后再 CAS 到 `Succeeded|Failed|Uncertain|Skipped`。任一 pre-effect 或 terminal checkpoint
失败都保留 typed durable attention，不能降级成普通字符串成功/失败。涉及 Codex auth start 时，
sidecar 先以 inert 状态启动；Gateway 必须先 flush 匹配 operation id 与授权 digest 的 `start_ack`，
再线性化 start authorization，ack 之前不允许 network/OAuth flow。`set_active_profile`、
`update_profile_connection` 与 `codex_ensure_profile` 是 intent-only typed outcome，不创建
P2-B receipt，也不声称 runtime 已应用。已应用 profile 的 key 清理/删除在 ConfigCommit 后还有独立
`DeleteRollingBackup` effect：只有 `config.json.bak` unlink、目录 fsync 与 exact absence 回读全部成功，
才允许 completed；任一步不确定都保留 after-image receipt/fence 与 typed attention。
`set_mode_official` 的 generation invalidation 也位于所有实际 stop effect 的 durable InProgress checkpoint
之后。`set_settings_destructive` 在 begin receipt 前记录 bridge config、ownership sidecar 与 managed stub
的 exact uid/device/inode/mode/nlink/length/digest；effect 内重新绑定 before identity，config rename 与每个
unlink 都同步相应父目录并回读 restored after-image 或 exact absence，任一 metadata/sync/readback 失败都
保持 InProgress/Uncertain receipt 与 attention，不能发布 `Succeeded(absent)` 后清 fence。

## 三个阶段域

| 阶段域 | 形态 | 用途 |
|---|---|---|
| operation trace | typed `OperationStage` | 脱敏运行日志和耗时 |
| runtime journal | versioned V1/V2；one-click 与 interrupted-Gateway recovery V2 使用 typed `phase` / outcome | crash/recovery 的持久 checkpoint |
| frontend DTO | coarse string | 用户可见失败定位 |

一键/auto-boot 失败由内部 `runtime/failure.rs` 的 `OneClickFailureKind` 在**产生点**标注，再投影到冻结的 coarse stage（`prepare|science_stop|gateway_start|catalog_verify|science_start`）与 `recovery_status` / `environment_status`。**不得**用用户文案 `contains` 反推 stage。frontend DTO、operation trace 与 runtime journal 仍是三个不同阶段域；one-click V2 的 typed phase 不改变 UI DTO。

finalize consumer 不从 degraded 文案猜测磁盘结果。只读 projection 重新读取 canonical config，
把 journal 精确投影为 `open|cleared`，把 runtime binding 投影为相对 active profile 的
`matches_active|different_active|absent`，并据此计算 consumer publication。normal start 只有 journal
cleared、binding exact match 且 selection 不 pending 时可成为 ready；history action 在 journal
cleared 时保持 attention；journal open、readback failure、unknown 或矛盾组合一律保持 manual。
只有 ready 投影可携带 applied profile id；attention、manual 与 readback failure 对 UI 发布
`applied_profile_id=null`、`selection_pending=true`，避免陈旧 binding 被呈现为本次已应用。
auto-boot 的 failed/attention publication event 与同 sequence 非消费式 snapshot 补读在渲染 DTO 前执行同一 unknown publication；
这不改变或包裹原 one-click DTO。
因此 atomic commit sync 与 rollback 双失败即使返回同一 degraded DTO，也按其后真实可读状态
分类，不预设一定保留旧 binding 或 journal。projection 不 chmod、不携带 transaction record、
snapshot ticket、cleanup path、credential 或写能力。

## 一键开始事务

冷启动或重启分支的高层顺序：

1. runtime-owned preflight 捕获 immutable config/Gateway snapshot；command 只在锁外准备 provider
   auth，进入 Lifecycle 串行区后复核 snapshot 并把控制权交给唯一 runtime entry façade；
2. façade 每轮重新读取 facts，由 pure decision 选择一次 finalize replay、interrupted-Gateway
   recovery 或业务 route；每个 recovery effect 后必须重采 facts 再决策。Gateway terminal exact
   record 只经不可序列化、process-local affine handoff 交给同一次 one-click，首个 checkpoint
   只能用完整记录 CAS 接管；
3. entry façade 在任何 branch-specific effect 前用 immutable facts 区分 healthy reopen 与 mutating
   cold/recovery；healthy 进入独立 reopen owner，不读取或消费 pending-cleanup，也不 capture SSH stub；
   mutating 分支先重试 exact pending cleanup，只有实际清理后才重新采集 facts，再把冻结的 branch
   输入交给独立 cold coordinator 完成真实 config、alias、wrapper、sidecar/stub 预检与后续事务；
4. 从 managed launch receipt 生成脱敏 durable recipe，先持久化 `PriorStopIntent`，再精确停止
   prior Science，并立即持久化 `PriorStopOutcome`（`ExactStopped|NotStopped|Unknown`）；
5. 通过 `AuthorityTransaction` 固定 opaque roots、捕获 protected projection，并持久登记
   recovery disposition；
6. 从同一 candidate Science identity 计算一次 64-hex fingerprint，并从已登记 authority snapshot 取得一次经验证的 `managed_id` ticket；首个 V2 checkpoint 同时携带两者；
7. 准备 virtual login 与 SSH bridge；
8. 启动/复用 Gateway，校验 model catalog；
9. cold coordinator 把冻结的 Gateway/SSH/authority/transaction 输入交给独立 managed Science
   launch phase owner；该 owner 通过 `ScienceHostAdapter` 启动 Science，按 typed
   exposure/health/identity phase 校验 listener、binary、data-dir 并提交 managed receipt；
10. 同一 phase owner 复核 Science DB；需要修复时 exact-stop 首次 receipt、执行一次 bounded
    restart、提交 fresh receipt 并复核 DB。返回 coordinator 后再推进 catalog checkpoint；
11. best-effort 配置 Skill route/connector；该步骤可能写 route marker 并调用运行中 Science control；
12. 计算 binding 并持久化 finalize intent；随后 best-effort 打开 UI 并构造成功结果，再把
    authority manifest 转为 cleanup-only/清理，最后按同一完整 V2 identity 原子提交 binding 并清 journal。

one-click 的八个 checkpoint 时机均写 V2。进程内 progress 保存上一次实际提交的完整
V2 record；后续 phase 只在磁盘记录与该完整 record 相等时推进 typed `phase` 及其对应
exposure，成功清 journal 与 binding commit 也执行相同 CAS。transaction id、candidate
fingerprint、snapshot ticket、prior binding、canonical compensation 与 Gateway outcome
必须保持不变；同 ID 的 phase、exposure 或其他字段漂移同样保留当前 journal 并
fail-closed。

统一失败补偿也受完整记录约束：`Journaled` progress 使用完整上一条 business record；
`PreJournalAbort` 使用 authority capture 前冻结的完整 `runtime_transaction`。两者都只在当前 config
仍与该精确值一致且没有 sibling compensation 时，原子发布独立、path-free 的
`runtime_compensation V2 / in_progress`、固定五步 pending plan 并进入 `Compensating`。aggregate durable intent
在首个补偿 effect 前完成；每个 top-level effect 又必须先把自己的 step 从 `pending` CAS 为 `in_progress`，
effect 返回后再 CAS 为 `succeeded|failed|skipped(cause)`，前后转换都绑定完整 compensation record 与当前
唯一 expected business record。authority restore 同时 CAS 当前业务 record 与最新
compensation record，再恢复捕获前 config，并只把独立 compensation record 覆盖回去；因此原
profile-switch / history / legacy journal 不会被补偿 crash marker 取代。该 marker 不复制 prior-stop recipe、
absolute runtime path、message 或 credential。终态写入 CAS 独立 record 以及允许的补偿前/恢复后 business
record：全部成功时清除，失败时从 typed step outcome 推导 canonical `failed_steps` 并写 `incomplete`。
任一 retarget 都保留当前整份 config、authority、
运行态与 recovery snapshot，不执行 blind rollback。

首个 post-snapshot checkpoint 原子提交失败、且 protected mutation 尚未开始时，同一进程仍可
通过 `PreJournalAbort` 使用内存中的 registered ticket 与冻结 business record，先发布同一 durable
compensation intent，再进入既有补偿；intent 发布失败则零补偿 effect 并保留 ActiveRecovery。prior stop 之前已经存在
不含 snapshot ticket 的 durable one-click record；stop 成功后必须先发布 exact outcome，捕获
snapshot 后再以完整记录 CAS 附加 ticket。intent/outcome 发布失败、record 漂移或 restart proof
不完整均保留 journal 并 fail-closed，不能把诊断文案当作恢复权威。

已健康 daemon 的 reuse/reopen 分支顺序不同：它先确保 Gateway、复核 model
catalog，再提交 runtime binding 并清 journal，之后才检查或 best-effort 修复
Skill route/connector，最后生成并打开 UI URL。不能把 cold-start 的
route-before-binding 顺序外推到 reuse 分支。

OAuth、SSH、MCP 或 route 写入前必须完成 protected snapshot。`serve` 之前的失败可在身份安全时精确补偿；`serve` 之后 Science 可能迁移环境，恢复结果可降为 `environment_uncertain`。

## profile 与 mode 切换

- `set_active_profile` 只写 selection，不触碰运行态。
- 真正应用由下一次一键开始执行。
- mode 切到 official 时先 bump generation，停止受管 Science/Gateway，再持久化 mode；停机失败不提交。
- 当前产品不执行运行中 profile switch transaction；`set_active_profile` 只提交 selection，下一次一键开始按新的 active profile 重新走完整启动与补偿链。已删除无生产 caller 的 `set_active_profile_txn` writer 及其 rollback/journal helpers。
- 历史遗留的 profile-switch V1/V2 journal 仍由中断 Gateway recovery 严格读取、验证并按其既有 complete-record CAS 规则处理；selection 与普通 one-click 不会重新产生或接管这类旧 writer 的 handoff。

## 中断 Gateway 恢复

应用重启后的 Gateway recovery 只接管能够证明为 profile-switch 的事务：当前 V2 必须是
`profile_switch / start_formal_gateway|recover_interrupted_gateway` 且 compensation 必须仍为
`not_started`，兼容 V1 必须是
`start_formal_gateway|recover_interrupted_gateway`；one-click snapshot、其他 V1 phase、target
漂移或同进程仍持有 Child 的路径都不会探测或停止 listener。兼容 V1 一旦需要继续恢复，
只会原子升级为 V2，不再写回 string stage。

V1 reader 只接受十种已冻结 wire stage（八种 plain stage 与两种携带合法 64-hex
fingerprint 的 environment stage），save/load 不会隐式升级或改写 wire。未知、缺失
fingerprint、future nested schema、未知字段和重复字段均在 config load 时 fail-closed。
`runtime_fingerprint` 的语义由 operation 拥有：one-click 绑定 candidate runtime，history recovery
绑定去除 journal 后的完整 Config authority；history phase 缺少该 64-hex identity 同样 fail-closed。
当前生产可写 V2 的重启矩阵包括八个 one-click phases、history recovery 的
`stop_old_science / authority_snapshot_active / history_credential_write_pending /
history_credential_published / resume_after_history_restore`，以及 interrupted-Gateway recovery 的
`pending|stopped|not_managed|signal_failed|exit_unconfirmed|absent_after_attempt`；其它组合
不得由 reader 推断为可恢复语义。历史 profile-switch journal 只作为该 recovery 的输入，不是当前可写 phase。

通过 path secret、初始公开 Gateway identity/contract 与 packaged binary 可用性检查后，recovery
先用调用方读取的**完整原记录** CAS 发布
`profile_switch / recover_interrupted_gateway / gateway_stop_outcome=pending`。既有精确 stop
随后复核 binary/uid/PID 与最终 listener identity，再尝试信号/wait；
`stopped|not_managed|signal_failed|exit_unconfirmed` 由第二次完整记录 CAS 持久化。
若前次已发布 recovery intent 而重启后 listener 已消失，则写
`absent_after_attempt`；`stopped|absent_after_attempt` 是终态，后续调用不会因端口重新出现而
再次探测或停止 listener。任一 CAS 期间的 transaction/target、previous binding/Gateway、
operation/phase、exposure、compensation 或 outcome 漂移都保留当前记录并 fail-closed；该
eligibility 在后续重启仍会拒绝 compensation 已漂移的记录，不会
回滚 stage、重启 prior Gateway 或改变既有 TERM/wait 策略。

production runtime entry façade 必须保留 recovery 返回的 affine terminal handoff，并在 post-effect
facts 重新采集后传入 one-click；command 不解释或保存 recovery outcome。ordinary one-click API
不接受调用方伪造 expected record，缺少 handoff 时仍拒绝任意 V2。
handoff 只承载实际持久化的 terminal complete record；one-click 重新读取 config，逐字段确认
transaction、target、operation/phase、terminal outcome、binding、prior-stop/finalize 默认状态后，
才允许首个 checkpoint 用 complete-record、current active profile 与旧 binding 的联合 CAS 接管。terminal record 仍保留到接管时，因此
later-listener 不会被二次探测或停止；任一漂移都保留原 journal 并 fail-closed。

## 历史恢复

frontend 只持有一次性 opaque reference，并为每份历史提供“仅恢复”和显式“恢复并启动”两个 intent。
两者都只调用一次 `restore_history_choice`；backend 复核 active profile、port、session 与可选 resume
auth preflight 后：

1. 冻结去除 journal 后的完整 Config authority fingerprint，并以
   `HistoryRecovery / StopOldScience` complete record 持久化该 identity 与 stop intent/outcome，再精确停止当前
   受管 Science；没有当前 runtime 时必须保有 typed quiescence proof；
2. 捕获 protected authority snapshot，取得 verified ticket，并以完整原记录 CAS 发布
   `AuthoritySnapshotActive`；私有恢复清单完成 fsync 后，再以完整原记录 CAS 发布
   `HistoryCredentialWritePending`，此后才允许写凭证；每个 credential / marker 文件都必须完成
   file fsync、atomic rename 与 parent-directory fsync，调用方才可继续发布 credential commit；
3. 每次 history checkpoint、credential commit、finalize 和 terminal resume handoff 消费都同时要求
   当前完整 Config authority fingerprint 未变化；mode、port、SSH setting、profile 或其它 sibling writer
   漂移时保留现状与 exact journal，绝不继续 resume。恢复用户选择的历史组织后，再以一次完整原记录 CAS 同时发布 `HistoryCredentialPublished` 与
   `ClearJournal` / `ResumeOneClick` finalize intent；同进程失败只有在**完整 Config**仍与预期相等
   时才回写 before-image，避免覆盖并发 writer；重启看到 write-pending 时按 exact ticket、清单与
   backup inode 重放 credential/marker before-image；重放任何 effect 前必须先复核冻结的完整 Config
   authority fingerprint；只要 HistoryRecovery journal 打开，central config writer fence 就拒绝所有
   journal-external sibling Config 写入，downgrade preview/commit 也必须在 export、backup 与 v2 publication
   前拒绝 open transaction，关闭校验到 effect 之间的跨进程 writer 窗口。随后必须用事务开始时
   仍被 fingerprint 证明的 port 重新探测当前 Science typed quiescence，最终清 journal 也要求同一
   fingerprint 与 exact record CAS，
   已持久化的 pending 会在 one-click provider auth preflight 前收敛；若 journal 与 lock-free auth
   preflight 竞态出现，则必须在认证失败返回前或业务 entry 前于 destructive lease 内收敛，认证不可用
   不能把 before-image 留在认证之后；
   随后再转 cleanup-only、清 journal；
4. credential publication 与 finalize intent durable commit 成功后才轮换全部一次性 reference；
   cleanup/finalize 失败时以 degraded DTO 的 `history_recovery.choices` 返回当前轮换后的 reference，
   不返回私有 snapshot 路径，并保留 exact snapshot/journal；
   cleanup 已转为 cleanup-only 后，restore-only
   清 journal 并返回 stopped，resume 则发布无 snapshot ticket 的 terminal handoff；
5. 只有 exact terminal handoff 可由同一 backend operation 消费并重新进入既有 one-click owner。
   frontend 对 restore-only 和 resume 都通过 `finalize_consumer_state` 的只读回读决定展示；restore-only
   只有 `ok + attention` 才可显示“已恢复并保持停止”，degraded/manual 或回读失败均不得误报成功。

组织 UUID、真实路径与敏感凭证不跨 invoke 边界。

默认 restore-only 仍安全结束在 stopped；用户以后单独点击一键开始仍是另一个 destructive
operation。显式 restore-and-resume 则在同一 destructive lease / backend IPC 内，以 typed terminal
handoff 衔接既有 one-click transaction。history credential 一旦完成 durable commit，后续 one-click
失败不会回滚用户已选择的历史；它沿既有 one-click 补偿与 readback 合同停在可检查状态。这不是把
history、cold start 与 recovery 合并成万能事务。

## 停止

`stop_all`：

1. bump generation，使旧 probe/启动失效；
2. 在 `AppState` 锁内 claim exact process-local Science owner 与 stop request；
3. 释放 `AppState` 后按既有 stop script、TERM/KILL/wait 策略精确停止 Science，使 `status` 可并发复制 read model；
4. 重新取得 `AppState`，只在 lifecycle generation 与完整 owner identity 均未变化时清理 tracking 并发布 typed stop outcome；陈旧结果必须保留 replacement runtime；
5. 无论 Science 结果如何都停止 Gateway；
6. 若 Science 未验证停止，返回“Gateway 已停、Science 失败”的部分结果。

`set_mode(official)` 复用同一 Science owner claim / lock-free wait / CAS 边界，但保持不同的后续语义：
只有 current Science stop 成功才停止 tracked Gateway 并进入 mode config commit；claim、stop 或 owner CAS
失败都保留 Gateway 与旧 mode。config commit 失败仍保持既有 stop-before-commit 合同，不重启已停止的 runtime。

需要 teardown 的 `set_settings` 也复用该边界，但保留 stop-before-generation 与 stop-before-commit：只有
current Science stop 成功才 bump generation、停止 tracked Gateway、撤销 SSH bridge/stub 并提交 settings；
claim、stop 或 owner CAS 失败保留 replacement Science、Gateway 与旧 settings，不进入后续 teardown。

Science stop 不能只信 CLI 退出码。必须结合 pre/post 唯一 listener PID、canonical executable、data-dir、launch token 与端口真实关闭；身份漂移时不发送信号。

## 诊断与失败链

- `status` 只做短超时 HTTP health 和内存 metadata 投影；
- doctor 不是强 identity 或 live provider 证明；
- route/connector 配置失败只降级外部 Skill，不阻断普通启动；
- SSH 默认关闭；启用后其 preflight 是 fail-closed；
- Codex auth/catalog 错误通常只阻断对应 Codex 操作；但 active profile 为 Codex，或 prior running Gateway 仍是 Codex 而下一次一键开始需要先取得其 proof 时，也会阻断该次启动；
- provider/Gateway、authority snapshot、runtime preflight、port identity、Science launch/health 可阻断一键开始。

## 稳定边界与开放范围

- cold one-click 已与 entry/healthy owner 分离，managed Science launch 与 aggregate compensation 也有
  各自 phase owner；coordinator 仍顺序拥有 prior stop、authority、Gateway、phase dispatch、route 与
  finalize。五个 top-level compensation effect 可在 fresh production entry 中按 exact private manifest
  与 registered snapshot 重放/收敛；V1 与 typed incomplete V2 保留为人工边界。
- canonical config writer 有独立跨进程 advisory fence；authority filesystem 的普通 cooperating writer
  使用共享 fence，durable replay/history restore 的 exclusive owner 只通过 scoped、不可跨线程的 bypass
  调用同一 writer leaf。两类 fence 不能互相替代。
- history recovery 以 complete-record CAS、protected snapshot、唯一跨进程 effect owner 与 durable
  restore outcome 收敛 credential publication 和 full-snapshot restore；one-click 的 registered snapshot
  不会让其他 direct restore 自动获得相同 closure。
- history durable commit 后的 one-click 失败不会回滚用户已选择的历史；restore-only 与以后单独点击
  的 one-click 是两个 operation，只有显式 restore-and-resume 使用 typed terminal handoff。
- cold prior stop、managed DB restart、history prior stop、live compensation 与 fresh replay 五条
  transaction-scoped Science stop path 共享 process-local executor，同时保留各自 durable intent、effect、
  outcome、lease 与 crash recovery 顺序。
- current evidence gaps、Skill/MCP/SSH 动态 gate 与下一步只在
  [当前重构路线](../../.agents/context/known-issues.md)维护；本架构正文不保存候选运行结果。
