# 运行时状态与事务

本文回答“谁拥有运行状态、怎样持久化、锁序是什么，以及启动/切换/恢复/停止失败怎样补偿”。Science executable/data 身份见[Science runtime](science-runtime.md)；command/DTO 见[Desktop 控制面](desktop-control-plane.md)。

## 状态所有权

| 状态 | Source of truth | 持久性 |
|---|---|---|
| Gateway child、launch ID、key fingerprint、launch context | Tauri `AppState` | 进程内 |
| Science runtime identity、confirmed-stopped token、boot/history refs | Tauri `AppState` | 进程内；当前产品不保存 daemon child |
| Science version observations | `AppState.science_version_cache` | 进程内缓存；不等于 daemon/runtime identity |
| pending authority cleanup retry set | `AppState.pending_authority_cleanup` | 进程内镜像；跨重启权威是 private pending-cleanup manifest |
| profile、active selection、端口、mode、SSH/Codex 设置、path secret | CSSwitch `config.json` / `Config` | 持久 |
| last healthy binding | `Config.runtime_binding` | 持久；只含公开 identity/hash |
| in-flight runtime transaction | `Config.runtime_transaction` / `RuntimeTransactionRecord` | 持久；one-click、history recovery、compiled test-only profile-switch 与 interrupted-Gateway recovery writer 写 typed V2；V1 只保留兼容读取与原 wire 序列化 |
| in-flight one-click compensation | `Config.runtime_compensation` / path-free `RuntimeCompensationJournal` V1/V2 | 持久；V1 只兼容读取并阻断 mutation；当前 V2 只含 opaque compensation id、目标/fingerprint、受管 snapshot ticket、aggregate state 与五个 typed step state；与 `runtime_transaction` 分离 |
| Science protected state rollback | private authority snapshot + manifest | 持久到 success/完整补偿/人工处置 |
| Science managed launch | `science-managed-launch.v1.json` + live listener identity | 持久 receipt 与 live 组合 |
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
| typed entry decision、protected projection、journal recovery 与 healthy / cold branch dispatch | `runtime/sandbox_session/one_click.rs` |
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
- `stop_all`、切换到 official 的 `set_mode`、需要 teardown 的 `set_settings`、native exit 与 downgrade cleanup 先在锁内冻结 generation 与 Science runtime/confirmed-stopped/child/port/URL owner snapshot，并取得 exact stop request，随后释放 `AppState` 执行 stop script、TERM/KILL 与轮询等待，最后在锁内按 generation + 完整 owner identity CAS 发布结果。陈旧 `set_mode` / `set_settings` 结果不会停止 replacement Gateway 或提交 mode/settings；`set_settings` 只在 current stop success 后按原顺序 bump generation、停 Gateway、撤销 SSH artifact 并提交设置。native exit 的陈旧结果也不得清 replacement Science，但其 best-effort policy 仍继续停 Gateway。downgrade 先 bump generation；陈旧 Science 结果保留 replacement、仍按既有 terminal stop-all policy 停 Gateway，并在 export、backup 或 v2 publication 前失败。transaction-scoped Science stop 由同一个 owner/wait/CAS executor 覆盖 cold prior stop、managed DB restart、profile-switch rollback、history recovery prior stop、live compensation cleanup 与 fresh-process compensation replay cleanup：它只在 `AppState` 锁内冻结 generation、runtime/confirmed-stopped/child/port/URL 完整 owner 和 exact stop request，锁外执行 stop script、TERM/KILL 与等待，再按 generation + 完整 owner identity CAS 发布；replacement、generation drift 或 stop failure 都 fail closed，陈旧结果绝不清除或覆盖 replacement。Gateway reuse 先在 `AppState` 下冻结 generation 与 child PID、端口、secret、provider、gateway/shim、launch id、key fingerprint 和完整 launch recipe，锁外执行 HTTP health，再按 generation + 完整 owner identity CAS 接受结果；陈旧结果 fail closed，不能清理或覆盖 replacement Gateway。Gateway spawn 同样只在 `AppState` 下冻结 generation、空 slot、secret、完整 candidate owner 与 launch recipe；candidate log、命令与环境构造、Skill bridge 配置 staging、`Command::spawn()` 和 health poll 都在锁外，再按 generation + 完整 candidate owner CAS 接受结果。generation 漂移或 replacement 已出现时停止 candidate，绝不覆盖 replacement；无法确认退出的 child owner 移交独立 registry。typed `GatewayStopOutcome::Uncertain` 必须由 destructive caller 消费，并在 config、credential 或 binding commit 前 fail closed；
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
replay owner 在 provider auth 前循环重采 config，校验 public record、private manifest、
snapshot ticket 与完整 business record，然后重放或观察一个 `pending|in_progress` step。若同进程补偿替换过
Gateway，私有 manifest 只保存其受管 health identity、path secret 与端口；fresh authority step 必须再以当前
打包 binary、uid、唯一 listener 和二次 health 复核精确停止该 candidate，不序列化或伪造 process-local
`GatewayReceipt`。authority restore 只恢复 durable filesystem/config authority，不把上个进程的 AppState/Gateway
child ownership 当作可恢复事实；完整 restored config 是“effect 已成功、outcome 未落盘”的幂等 commit marker。
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
`start_proxy` 已移除，Gateway 启动只保留在 cold/healthy/profile-switch/recovery 内部路径；S6
不改变这些 caller 的 lease、checkpoint、补偿、binding/journal commit、DTO 或可见文案。

三类 receipt/authority 不能合并成一个“统一事务”：

| 证明 | 建立的控制权 | 当前 consumer | 明确不拥有 |
|---|---|---|---|
| `GatewayReceipt` | process-local start/reuse 的 route、accepted health/catalog 与完整 recipe | cold/healthy/profile-switch/recovery caller | crash journal、rollback、binding commit |
| Science launch/stop receipt | executable/data-dir/listener/PID/process-start/runtime SHA 与 managed record 的 exact live ownership | prior/history/DB/compensation stop 与 fresh restart | authority tree before-image 或跨进程 lease |
| `AuthorityTransaction` | protected projection capture、verified ticket、restore、cleanup/commit | one-click coordinator | prior stop、Gateway/SSH、journal、DTO 与全局编排 |

authority snapshot 会捕获 managed receipt 文件的 before-image，但这不把
`AuthorityTransaction` 变成 Science live stop authority。长期控制权交接应使用小型、affine 的
process-local handoff；durable journal 只保存 crash recovery 所需的最小 identity/outcome，不能
把 receipt 全量序列化或让诊断 DTO 参与控制流。

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
- 当前产品不执行运行中 profile switch transaction；`set_active_profile` 只提交 selection，下一次一键开始按新的 active profile 重新走完整启动与补偿链。源码中的 `set_active_profile_txn` / `PriorScienceRestored` 链是 `compiled + test-only` candidate，不属于当前 product-reachable 合同。
- 该 test-only candidate 的 profile-switch V2 只能在同一进程、同一 reconcile 调用内按原
  完整 typed record（含 transaction/target、previous binding/Gateway、operation/phase、
  exposure、compensation 与 Gateway outcome）交给首个 one-click V2
  checkpoint，或由 exact healthy-reopen CAS 提交 binding 并清除；当前 journal 消失、回退
  V1 或 retarget 时均保留当前状态并拒绝覆盖。普通 one-click 或重启不会把该记录当作可接管事务。

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
history_credential_published / resume_after_history_restore`、test-only profile-switch
`start_formal_gateway`，以及 interrupted-Gateway recovery 的
`pending|stopped|not_managed|signal_failed|exit_unconfirmed|absent_after_attempt`；其它组合
不得由 reader 推断为可恢复语义。

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

## 当前架构缺口

- cold one-click 已与 entry/healthy owner 分离，managed Science launch 与 aggregate compensation 也有
  各自 phase owner；coordinator 仍顺序拥有 prior stop、authority、Gateway、phase dispatch、route 与
  finalize。O1-E3 已让五个 top-level compensation effect 在 fresh production entry 中按 exact private
  manifest 与 registered snapshot 自动重放/收敛；V1 与 typed incomplete V2 仍明确保留为人工边界；
- canonical config writer 已有跨进程 advisory fence；history recovery 已用 typed complete-record CAS、
  protected snapshot、唯一跨进程 effect owner 与 durable restore outcome 收敛 credential publication 和 full-snapshot
  restore。其他直接 full-snapshot restore 与跨 config / sibling authority 的 multi-file crash boundary 仍未统一；
- history restore durable commit 之后的 one-click 失败不会回滚用户已选择的历史；默认 restore-only
  与以后单独点击的一键开始仍是两个 operation，只有显式 restore-and-resume 使用同一 backend handoff；
- `stop_all`、`set_mode`、teardown `set_settings`、native exit 与 downgrade cleanup 已锁外等待并使用各自的 process-local owner/CAS publication；六个 transaction-scoped Science stop 边界已统一使用共享 executor，同时保留各自 durable intent/effect/outcome、lease 与 crash recovery 顺序。该源码问题已关闭；sibling multi-file crash boundary 仍是独立缺口，不能由本项外推；
- MCP 与 SSH 的产品动态 gate 仍开放；具体当前证据缺口见 [known issues](../../.agents/context/known-issues.md)。
