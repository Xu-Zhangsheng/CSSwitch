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
| in-flight runtime transaction | `Config.runtime_transaction` / `RuntimeTransactionRecord` | 持久；one-click、compiled test-only profile-switch 与 interrupted-Gateway recovery writer 写 typed V2；V1 只保留兼容读取与原 wire 序列化 |
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
| command 级 mode/settings/stop/quit 串行编排 | `commands/runtime/lifecycle.rs` |
| 一键启动、history recovery 与 UI failure 投影 | `commands/runtime/one_click.rs` |
| protected projection、journal recovery、route reconcile、SSH preflight 与一键事务 | `runtime/sandbox_session/` |
| protected snapshot 合同、capture 与 restore | `runtime/sandbox_session/authority_snapshot.rs` façade及其 `authority_snapshot/` 片段 |
| one-click authority capture、verified ticket、restore 与 cleanup 接口 | `runtime/sandbox_session/authority_transaction.rs` |
| healthy daemon reopen 的独立补偿分支 | `runtime/sandbox_session/one_click/healthy_reopen.rs` |
| Gateway recovery/reuse/spawn/stop 与 typed acceptance receipt | `runtime/proxy_lifecycle.rs` façade及其 `proxy_lifecycle/controller.rs`、其他片段 |
| Science executable、runtime identity、launch/health、managed receipt 与 stop | `runtime/science.rs` façade及其 `science/host_adapter.rs`、其他 `science/` 片段 |
| 持久配置、runtime binding 与 journal schema | `desktop/src-tauri/src/config.rs` |

这些路径是当前维护映射，不改变上表的 source of truth。`runtime.rs`、
`science.rs`、`proxy_lifecycle.rs` 和部分 `sandbox_session` 根文件是保持历史
module surface 与测试 identity 的 façade；状态所有权仍由 `AppState`、
`Lifecycle`、`Config`、receipt/manifest 与 live identity 的既有组合决定。

## 锁序与并发

跨命令变更遵守固定顺序：

```text
RuntimeMutationLease(Intent | Destructive | HostBridge | Terminal)
  -> Lifecycle mutex
  -> AppState mutex
    -> config::update mutex
```

- `RuntimeMutationLease` 要求会改变 runtime context 的 production operation 先声明
  intent、destructive、host-bridge 或 terminal domain；四个 domain 复用现有
  `Lifecycle` mutex，保持 process-local 互斥与不可重入语义，而不是四把可并行锁；
- `stop_all` 先在锁内冻结 generation 与 Science runtime/confirmed-stopped/child/port/URL owner snapshot，并取得 exact stop request，随后释放 `AppState` 执行 stop script、TERM/KILL 与轮询等待，最后在锁内按 generation + 完整 owner identity CAS 发布结果；这条“锁外等待”结论只覆盖 `stop_all`。cold prior stop、history、DB recovery、compensation、mode/settings/native-exit 等 sibling stop 仍可能持 `AppState` 跨越外部等待；Gateway spawn 后的 health poll 在锁外，但 reuse health、旧进程清理与 spawn 仍在锁内；
- `Lifecycle.generation` 使锁外 probe 在 stop/clear/switch 后失效；
- `config::update` 只覆盖 load-modify-save；
- config 文件提交使用 pinned/no-follow 边界、临时文件、rename、fsync、提交前复核与回滚，但不是跨进程 advisory lock。

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
recovery。coordinator 仍拥有 operation
trace、Gateway/SSH 顺序、`CompensationOutcome` 聚合与 frontend DTO/text/recovery projection。

`GatewayController` 是 formal Gateway 的 process-local façade。它保留既有 spawn/reuse、双层
health、catalog fingerprint、generation/write-back 与 child ownership 核心，但只在全部接受
条件通过后返回非序列化 `GatewayReceipt`。receipt 同时绑定 route、`Reused/Restarted`、health
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

## 一键开始事务

冷启动或重启分支的高层顺序：

1. 读取 active profile 与 provider contract，复核端口和 Codex proof；
2. 进入 Lifecycle 串行区，先重放 one-click success finalize，再恢复 interrupted-Gateway；
   recovery 的 terminal exact record 由不可序列化、process-local affine handoff 交给同一次
   one-click，首个 checkpoint 只能用完整记录 CAS 接管；
3. 若启用 SSH，完成真实 config、alias、wrapper、sidecar/stub 预检；
4. 从 managed launch receipt 生成脱敏 durable recipe，先持久化 `PriorStopIntent`，再精确停止
   prior Science，并立即持久化 `PriorStopOutcome`（`ExactStopped|NotStopped|Unknown`）；
5. 通过 `AuthorityTransaction` 固定 opaque roots、捕获 protected projection，并持久登记
   recovery disposition；
6. 从同一 candidate Science identity 计算一次 64-hex fingerprint，并从已登记 authority snapshot 取得一次经验证的 `managed_id` ticket；首个 V2 checkpoint 同时携带两者；
7. 准备 virtual login 与 SSH bridge；
8. 启动/复用 Gateway，校验 model catalog；
9. 通过 `ScienceHostAdapter` 启动 Science，按 typed exposure/health/identity phase 校验
   listener、binary、data-dir 并提交 managed receipt；
10. 复核 Science DB/catalog；
11. best-effort 配置 Skill route/connector；该步骤可能写 route marker 并调用运行中 Science control；
12. 计算 binding 并持久化 finalize intent；随后 best-effort 打开 UI 并构造成功结果，再把
    authority manifest 转为 cleanup-only/清理，最后按同一完整 V2 identity 原子提交 binding 并清 journal。

one-click 的八个 checkpoint 时机均写 V2。进程内 progress 保存上一次实际提交的完整
V2 record；后续 phase 只在磁盘记录与该完整 record 相等时推进 typed `phase` 及其对应
exposure，成功清 journal 与 binding commit 也执行相同 CAS。transaction id、candidate
fingerprint、snapshot ticket、prior binding、canonical compensation 与 Gateway outcome
必须保持不变；同 ID 的 phase、exposure 或其他字段漂移同样保留当前 journal 并
fail-closed。

统一失败补偿也受同一记录约束：`Journaled` progress 只在当前 config 仍含完整上一条
record 时允许恢复捕获前 config；成功 clear/binding commit 后 progress 进入 `Finalized`，
只在当前 journal 仍为空时允许后续补偿恢复。该 expectation 在 stop Gateway、恢复 authority
tree 或 AppState 之前先验证，并在 config commit 时再次 CAS；任一 retarget 都保留当前整份
config、authority、运行态与 recovery snapshot，将 authority restore 记为不完整；不会由补偿
覆盖刚刚拒绝的漂移记录或应用捕获态副作用。

首个 post-snapshot checkpoint 原子提交失败、且 protected mutation 尚未开始时，同一进程仍可
通过 `PreJournalAbort` 使用内存中的 registered ticket 进入既有补偿。prior stop 之前已经存在
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
当前生产可写 V2 的重启矩阵限于八个 one-click phases、test-only profile-switch
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

production command 必须保留 recovery 返回的 affine terminal handoff，并传入 one-click；
ordinary one-click API 不接受调用方伪造 expected record，缺少 handoff 时仍拒绝任意 V2。
handoff 只承载实际持久化的 terminal complete record；one-click 重新读取 config，逐字段确认
transaction、target、operation/phase、terminal outcome、binding、prior-stop/finalize 默认状态后，
才允许首个 checkpoint 用 complete-record、current active profile 与旧 binding 的联合 CAS 接管。terminal record 仍保留到接管时，因此
later-listener 不会被二次探测或停止；任一漂移都保留原 journal 并 fail-closed。

## 历史恢复

frontend 只持有一次性 opaque reference。backend 复核 active profile、port、session 后：

1. 精确停止当前受管 Science；
2. 恢复用户选择的历史组织；
3. 清理一次性 reference；
4. 返回本次 restore 的结果；下一次 start 是独立产品动作。

组织 UUID、真实路径与敏感凭证不跨 invoke 边界。

当前 frontend 在 restore 成功后仍自动调用 one-click；这是待修的控制面串联，不是长期事务
合同。目标是 frontend 一个明确 intent 对应一个 backend operation，并由用户显式发起下一次
start；不能把 history restore、cold start 和 recovery 合并成更大的万能事务。

## 停止

`stop_all`：

1. bump generation，使旧 probe/启动失效；
2. 在 `AppState` 锁内 claim exact process-local Science owner 与 stop request；
3. 释放 `AppState` 后按既有 stop script、TERM/KILL/wait 策略精确停止 Science，使 `status` 可并发复制 read model；
4. 重新取得 `AppState`，只在 lifecycle generation 与完整 owner identity 均未变化时清理 tracking 并发布 typed stop outcome；陈旧结果必须保留 replacement runtime；
5. 无论 Science 结果如何都停止 Gateway；
6. 若 Science 未验证停止，返回“Gateway 已停、Science 失败”的部分结果。

Science stop 不能只信 CLI 退出码。必须结合 pre/post 唯一 listener PID、canonical executable、data-dir、launch token 与端口真实关闭；身份漂移时不发送信号。

## 诊断与失败链

- `status` 只做短超时 HTTP health 和内存 metadata 投影；
- doctor 不是强 identity 或 live provider 证明；
- route/connector 配置失败只降级外部 Skill，不阻断普通启动；
- SSH 默认关闭；启用后其 preflight 是 fail-closed；
- Codex auth/catalog 错误通常只阻断对应 Codex 操作；但 active profile 为 Codex，或 prior running Gateway 仍是 Codex 而下一次一键开始需要先取得其 proof 时，也会阻断该次启动；
- provider/Gateway、authority snapshot、runtime preflight、port identity、Science launch/health 可阻断一键开始。

## 当前架构缺口

- ~~journal/trace/frontend stage 没有统一 typed source~~ 一键/auto-boot UI stage
  已由 `OneClickFailureKind` 投影；one-click、compiled test-only profile-switch 与
  interrupted Gateway recovery writer 均写 typed V2，V1 只保留兼容读取与原 wire 序列化；
- ~~prior Science 的 verified stop 早于 durable intent~~ 已由脱敏 durable
  `PriorStopIntent/Outcome` 前移闭合；
- ~~interrupted-Gateway terminal record 无法进入同 command one-click~~ 已由 affine exact-record
  handoff 与首 checkpoint complete-record CAS 闭合；
- ~~binding/journal clear 与 authority cleanup-only 之间存在不可重放 crash window~~ 已由
  `RuntimeFinalizeState::Intent`、authority manifest 精确转换和原子 binding+journal finalize
  闭合；
- V2 compensation schema 已有状态/步骤类型，但 one-click 生产补偿没有持久化逐步进度；
- ~~`science_failure_stage()` 用字符串推断~~ 已删除生产路径；
- ~~auto-boot 丢失 `stage/recovery_status/environment_status`~~ `boot://failed` 与
  `boot_error` 现携带与手动一键同 shape 的 failed DTO；
- config 的外部并发检测不是跨进程共享锁；
- Science stop 已建立 process-local `ScienceStopRequest`、可选 exact
  `ScienceStopOwnershipReceipt` 与 `ScienceStopOutcome`；mode、settings、stop/quit、history、
  one-click compensation/DB restart、Codex downgrade 与 native exit 均从 typed outcome 判定
  verified stop 或 classified failure。端口已关闭但 data-dir / ownership receipt 不存在的
  幂等成功不会发布 `science_confirmed_stopped`，也不能满足 one-click exact cleanup 或继续
  需要停止既有 runtime 的 authority recovery。首次离线历史恢复则由进程内
  `HistoryRecoveryScienceQuiescence::NoManagedRuntimeObserved` 冻结“preflight 未发现受管
  runtime”这一不同的 typed 前置，并在 restore 前以完整 Science probe 重新校验 session、端口与
  当前 typed state；若期间出现受管 runtime，restore 必须 exact-stop 并把 session proof 推进为
  `ExactStopped` 后才能旋转引用。它不冒充 exact stop receipt。用户可见文本与
  stop/TERM/KILL/wait 顺序保持不变；
- ~~`stop_all` 持有 `AppState` 锁跨越 stop script、TERM/KILL 与轮询等待~~ 已由 S2 的
  process-local owner claim、锁外等待和 generation/identity CAS 闭合；该结论只覆盖
  `stop_all`，不自动迁移 mode/settings/native-exit 等 sibling stop caller，也不建立 S3 mutation lease；
- ~~本地 Skill 第二次 runtime-context 复核之后仍可能与 stop/switch 交错~~ S3 在
  picker 之后取得短 `HostBridge` lease，并把 matching typed host receipt 绑定到 package
  commit 与 attach/readback；picker/download 不进 lease，Gateway bridge 的独立进程事务也
  不冒充全局 durable journal；
- ~~Science launch/health/managed receipt 由 one-click coordinator 直接解释 shell process~~
  S4 以 `ScienceHostAdapter` 收拢 typed launch spec、environment exposure、health、listener
  identity、managed receipt 与 stop façade；保留现有 Rust + shell 双层 fail-closed、用户可见
  行为与 operation ordering，不实现 host-neutral extension；
- MCP 与 SSH 的产品动态 gate 仍开放。
