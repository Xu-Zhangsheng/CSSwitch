# 2026-08-03 Runtime 事务编排再基线

状态：日期化只读审计

适用范围：本地 `next` source line；审计起点 exact HEAD `9a2d65ff1b7996b673f6901df97ac899717b8a44`

最后复核：2026-08-03（Asia/Taipei）

失效条件：production one-click/history/profile/recovery caller、runtime journal schema、Science/Gateway receipt、AuthorityTransaction、mutation lease 或本文绑定的 exact HEAD 发生变化时重新审计。

本文记录 S6 之后一次全面、只读、code-grounded 的重新摸排。当前开放风险与唯一
建议 NEXT 以 [known issues](../../.agents/context/known-issues.md) 为当前权威；本文不授权
源码实现，也不建立 artifact、installed/runtime、live provider/Science/SSH、签名、公证或
公开 release 结论。

## 1. 结论

旧 `S7 cold/healthy/history coordinator 分片` **不能直接启动**。当前代码已经有多种
彼此独立的事务和 receipt authority；先按文件拆 coordinator 会保留甚至掩盖既有控制权
交接缺口。

本轮发现三个应先于一般职责拆分处理的 HIGH：

1. interrupted-Gateway recovery 成功后保留 terminal typed journal；同一次 command 随即
   进入普通 one-click，而普通 one-click 在没有显式 handoff 时拒绝任何 V2 journal。结果是
   “恢复成功”会确定性地变成 `manual recovery required`，不能兑现 recovery 注释所述的
   “继续启动 committed config”。
2. cold one-click 在任何 durable prior-stop intent/outcome 之前 exact-stop prior Science；
   crash 或 stop effect 已发生但 outcome publication 失败时，重启后不能区分
   `not stopped / exact stopped / unknown`，也没有 durable restart recipe。
3. 成功路径先提交 runtime binding 并清 journal，后把 authority manifest 转为
   cleanup-only/清理。两步之间 crash 会留下“runtime 已提交、无 journal、仍有
   ActiveRecovery”，fresh boot 会错误阻断。

因此唯一建议 NEXT 是下面的 `O0 Interrupted-Gateway terminal handoff`，而不是旧 S7。
它是一个窄的生产链正确性修复候选；完成后必须再次 code-grounded rebaseline，才授权下一个
阶段。其余阶段只是有限候选路线，不是批量实施许可。

## 2. 现场与证据边界

审计开始时只读确认：

- worktree：`/Users/superjj/ccproj/CSswitch`；
- branch：`next`；
- exact HEAD：`9a2d65ff1b7996b673f6901df97ac899717b8a44`；
- 起始状态：clean；
- HEAD subject：`docs(runtime): seal S6 source closure`。

本轮只检查源码、测试、机器质量记录和当前文档；没有运行产品源码测试，也没有改变源码、
配置、真实 runtime 或账号状态。三路模块摸排后由主窗口交叉核对关键生产链。子审查的
`PASS/FAIL` 只表示其只读结论，不是 source gate。

| 证据层 | 本轮结论 |
|---|---|
| source / tests inspection | 已完成；绑定上述 exact HEAD |
| 本轮文档治理测试 | 由承载本审计的工作树验证记录决定 |
| exact-HEAD 15-suite `GATE-SOURCE` | `NOT-RUN` |
| built artifact / installed runtime | `NOT-RUN` |
| live provider / 真实账号 / Science / SSH | `NOT-RUN` |
| signing / notarization / Gatekeeper / release | `NOT-RUN` |

## 3. Findings

### HIGH-1｜Gateway recovery 没有把 terminal record 交给 one-click

production command 在同一 `Destructive` mutation lease 内先调用
`recover_interrupted_gateway`，成功后立即调用 `sandbox_session::one_click_login`：

- `desktop/src-tauri/src/commands/runtime/one_click.rs:223-250`。

recovery 对 exact managed Gateway 执行 durable `pending -> stopped` CAS，terminal record
刻意保留，避免后来的 listener 被再次探测或停止：

- `desktop/src-tauri/src/runtime/proxy_lifecycle/recovery.rs:237-258,301-323,418-465`；
- `desktop/src-tauri/src/runtime/proxy_lifecycle/tests.rs:1225-1254`。

但普通 one-click 没有接收 recovery receipt/handoff；在没有 expected profile-switch handoff
时，任何 V2 journal 都被视为需要人工恢复：

- `desktop/src-tauri/src/runtime/sandbox_session/one_click.rs:507-535`。

这不是单纯的文案或测试缺口，而是 production control-transfer 缺口。不能通过 recovery
成功后无条件清 journal 修复，因为 terminal record 同时承担幂等性、complete-record drift
防护和“不碰 later listener”的证据。

### HIGH-2｜prior Science stop 早于 durable intent/outcome

cold coordinator 先取得 process-local prior context 和 exact ownership，随后真实停止 prior
Science；authority capture/register 与首个 `StopOldScience` V2 checkpoint 都在 stop 之后：

- `desktop/src-tauri/src/runtime/sandbox_session/one_click.rs:969-974,1980-2112`；
- `desktop/src-tauri/src/config.rs:675-688,748-767`。

现有 `ScienceStopRequest` / `ScienceStopOutcome` 是同进程 typed contract，不是 durable
prior transition。`PriorScienceDisposition` 也只是栈上状态。hard exit 或 stop side effect
已经发生但 helper 返回错误时，没有持久记录能决定是否安全 restart prior runtime。

### HIGH-3｜成功提交与 authority cleanup-only 转换顺序不原子

正常 cold 成功先由 `commit_runtime_binding` 提交 binding 并清
`runtime_transaction`，随后才执行 `AuthorityTransaction::prepare_success`：

- `desktop/src-tauri/src/runtime/sandbox_session/one_click.rs:1252-1287,2599-2620,2653-2665`；
- `desktop/src-tauri/src/runtime/sandbox_session/recovery.rs:846-918`。

history attention 分支也先 clear journal 再 prepare success：

- `desktop/src-tauri/src/runtime/sandbox_session/one_click.rs:2195-2214`。

两步间 crash 会留下旧 `ActiveRecovery` manifest。fresh boot 仍把它视为中断 authority
transaction，而不是“runtime 已提交，只剩清理”。当前没有覆盖这一精确窗口的 subprocess
crash characterization。

### MEDIUM｜compensation schema 没有成为可恢复进度

V2 schema 已有 `NotStarted / InProgress / Incomplete` 与逐步 compensation 类型，但 one-click
生产 record 一直要求 `NotStarted`。实际 compensation 执行 Science、SSH、authority、config、
AppState、Gateway、prior restart 和 snapshot cleanup 时，没有持久化逐步 intent/outcome：

- `desktop/src-tauri/src/config.rs:713-734`；
- `desktop/src-tauri/src/runtime/sandbox_session/one_click.rs:449-465,561-579,1526-1717`。

当前 complete-record guard 能 fail closed，不能在 fresh process 中安全续跑已完成的一部分补偿。

### MEDIUM｜lease 与 CAS 的承诺仍主要是进程内

四个 `RuntimeMutationDomain` 共用一把 process-local mutex。production one-click 当前正确持
`Destructive` lease，但核心 one-click/Gateway API 只靠注释要求 caller 持锁，并未把 lease
capability 放进签名。config `update_result` 也只由进程内 mutex 串行化；single-instance plugin
降低正常双 UI 概率，却不构成跨进程 file CAS/advisory lock。

因此文档只能声称：当前 production caller 在单进程内按完整 V2 record CAS；不能声称
跨进程原子事务或类型系统已禁止 future caller 绕过 lease。

## 4. 五类流程的 coordinator 与状态 owner

| 流程 | 当前 production coordinator | 持久状态 owner | process-local owner | 当前边界 |
|---|---|---|---|---|
| cold one-click | `commands/runtime/one_click.rs::one_click_login_cmd` 负责 command/preflight；`runtime/sandbox_session/one_click.rs::one_click_login_with_options` 负责大部分启动、journal、receipt 与补偿 | `Config.runtime_transaction`、binding、authority manifest/snapshot、Science managed receipt | `AppState`、operation progress、prior/rollback context | prior stop、authority、Gateway、Science、DB、route、binding 和补偿集中在一个长 coordinator |
| healthy reopen | `runtime/sandbox_session/one_click/healthy_reopen.rs` | config before-image / binding；不建立 authority snapshot | 既有 Science identity、Gateway receipt | 不重启 Science；Gateway/catalog 成功后先 binding，route 为后置 best-effort；不能套 cold 顺序 |
| history attention / restore | attention 在 cold coordinator；explicit restore 在 `commands/runtime/one_click.rs::restore_history_choice_command` | attention 返回前 one-click journal/authority 已结束；restore 本身没有 durable operation journal | `AppState.history_recovery` 与 opaque refs | attention 和 restore 是两个事务；frontend 当前成功 restore 后自动再发 one-click |
| profile selection / apply | `commands/profiles.rs::set_active_profile` 只保存 selection；下一次 one-click 才 apply | active selection、后续 one-click journal/binding | 当前 runtime identity | `runtime/profile_switch.rs::set_active_profile_txn` 是 compiled/test-only candidate，不是产品路径 |
| recovery | entry orphan-Gateway recovery、V1 compatibility/manual recovery、DB restart、full compensation 分属不同 coordinator | typed journal、authority manifest/snapshot、managed receipt | exact listener/child/receipt、rollback context | 这些 recovery 的 authority、幂等性和补偿不同，不能合成通用 retry engine |

当前 command preflight 在 mutation lease 外完成 config/profile/Gateway/auth 读取，在进入
`Destructive` lease 后复核；这是有意缩短临界区。问题不在“前端发一个 command”，而在
backend 内部没有用清晰的 operation receipt/handoff 表达多个可失败控制权阶段。

目标边界应是：frontend 只提交一个明确的产品 intent 并渲染 typed 状态；backend command
保持薄；backend 内按 cold/healthy/history/recovery 等语义路由到有限 coordinator。一个用户
动作不能暗中串联另一个独立产品事务，例如 history restore 成功后由 frontend 自动启动。
同时也不能把所有动作合并进一条万能 durable transaction。

## 5. mutation lease、锁外等待、generation 与 CAS

| 路径 | Lifecycle lease | `AppState` 锁外等待 | stale publication guard | 结论 |
|---|---|---|---|---|
| `stop_all` | 是 | 是；claim 后释放锁执行 stop script/TERM/KILL/wait | generation + 完整 owner identity | 当前最佳参考实现；只覆盖 `stop_all` |
| Gateway start/reuse | 是 | spawn 后 health poll 在锁外；但 reuse health、旧进程清理和 spawn 仍在锁内 | generation + secret | 不能概括为“Gateway health 全部锁外” |
| cold prior stop | 是 | 否 | caller 依赖全局 lease；无独立 claim/CAS | 可阻塞 status 取得 `AppState` |
| history exact stop | 是 | 否 | session/config recheck，无 stop publication claim | 与 restore mutation 混在同一锁内路径 |
| DB recovery / compensation stop | 是 | 否 | journal guard 保护 config/authority，不等于 AppState stale-result CAS | 长等待仍在 `AppState` 锁内 |
| mode/settings/native-exit stop | 是 | 否 | typed outcome；没有统一 lock-free owner claim | 不能由 `stop_all` 的结论外推 |

`Lifecycle.generation` 是 process-local、重启后重置；它保护 AppState publish，不保护持久 config
或 journal。one-click 的 full-record comparison 能拒绝同进程字段漂移，但底层文件更新没有
跨进程 advisory lock。后续阶段必须把“process-local owner CAS”和“durable complete-record
CAS”写成两个不同合同。

## 6. 三类 receipt / authority 的控制权交接

| 证明 | 建立什么 authority | 当前交给谁 | 不能证明什么 |
|---|---|---|---|
| `GatewayReceipt` | 本进程 start/reuse 已通过 route、health、catalog 和 recipe 接受条件 | cold/healthy/profile-switch/recovery caller；当前 cold/healthy 只消费部分字段 | 不是 crash journal，不拥有 rollback，也不自动提交 binding |
| Science launch/stop receipt | executable/data-dir/listener/PID/process-start/runtime SHA/managed record 的 exact live ownership | stop request、prior stop、history、DB recovery、compensation；restart 必须产生 fresh receipt | authority snapshot 捕获 receipt 文件 before-image，不等于拥有 live stop authority |
| `AuthorityTransaction` | protected projection capture、verified snapshot ticket、restore、cleanup/commit | one-click coordinator 负责顺序、checkpoint 与补偿调用 | 不拥有 prior stop、Gateway/SSH、journal、DTO 或跨进程全局 lease |

`GatewayReceipt` 当前不 serde、不 Clone、不 Debug 是正确方向，但其完整 accepted identity/recipe
没有被类型化地消费到 final binding commit。Science launch receipt 在 cold 路径提交后又把 ownership
复制进 rollback context。`AuthorityTransaction` 则是 façade，不应升级成统一 coordinator。

推荐的长期形态是小型、affine 的 handoff：每一步只把下一步需要的 authority 转交一次；
持久 journal 记录 crash-recovery 所需的最小 identity/outcome，不能把 process-local receipt 全量
序列化，也不能让诊断 DTO 参与控制流。

## 7. journal checkpoint、补偿与 prior runtime

当前 one-click 八个 V2 checkpoint、candidate fingerprint、snapshot ticket、previous binding /
Gateway、canonical compensation 和 Gateway outcome 已有完整记录比较。它们提供的是：

- 同一进程中 checkpoint/clear/binding commit 的 exact-record CAS；
- journal drift 时在恢复 protected authority 之前 fail closed；
- interrupted-Gateway recovery 的 durable pending/outcome 与 later-listener guard；
- snapshot 已登记但首 journal 未提交时，只能同进程用 `PreJournalAbort` 补偿，fresh boot 人工恢复。

它们没有提供：

- pre-stop durable `PriorStopIntent/Outcome`；
- prior stop unknown 的 durable restart decision/recipe；
- success binding 与 authority cleanup-only 的单一可重放 finalize 协议；
- compensation 逐步 checkpoint 与 fresh-process resume；
- 跨进程 config/journal CAS。

## 8. 可以合并与必须独立的逻辑

| 处置 | 逻辑 | 原因 |
|---|---|---|
| 可共享机制 | exact Science stop 的 claim/execute/CAS primitive | 多个 caller 需要同一 identity safety；policy/事务仍由各 coordinator 拥有 |
| 可共享机制 | full-record journal transition helper、typed finalize receipt | 可统一 CAS 纪律，不统一 operation schema 或补偿语义 |
| 可共享机制 | cold/healthy 前的 read-only decision snapshot | 应在 cold-only SSH/stub/retry side effect 前决定分支 |
| 可共享机制 | Gateway/Science accepted identity 到 binding 的 typed projection | 防止 receipt 字段被 caller 任意丢弃；不把两种 receipt 合成一种 |
| 必须独立 | cold 与 healthy reopen | Science/authority/route/binding 顺序和 rollback 不同 |
| 必须独立 | history attention、explicit restore、下一次 start | 用户选择和失败重试边界不同；禁止 frontend 自动串联独立 mutation |
| 必须独立 | profile selection 与 runtime apply | selection 是 intent，不应偷偷启动或停止 runtime |
| 必须独立 | interrupted Gateway、DB recovery、authority compensation、prior restart | 证明对象、可重放性和失败后安全动作不同 |
| 必须独立 | Science update adoption 与 CSSwitch application release | owner、artifact、版本、签名和发布证据不同 |

## 9. 有限路线与唯一 NEXT

### O0｜Interrupted-Gateway terminal handoff——唯一 NEXT

目标：成功的 recovery 或 pre-existing terminal record 能在同一 command 中把**精确 terminal
record authority** 交给 normal one-click；later listener 仍不得被再次探测或停止。

允许范围：

- recovery 返回 process-local、不可伪造的 exact handoff/receipt；
- one-click 只接受与磁盘完整 terminal V2 record 相等、active target 一致的显式 handoff；
- 在首个 one-click checkpoint 以 complete-record CAS 原子接管/替换该 record；
- drift、V1、snapshot-preserving、非 terminal 或没有 handoff 的 V2 继续 fail closed；
- command/DTO keys、可见错误文本、stop signal/wait policy 和 profile selection 语义保持不变。

禁止：recovery 成功后先无条件 clear journal；扩大到 F5/compensation schema；开始 cold/healthy
文件拆分；把 test-only profile switch 变成产品路径；改变 artifact/live/release 层。

退出条件：

1. production command chain 有 focused regression，覆盖 `Stopped` 和
   `AbsentAfterAttempt`/pre-existing terminal 继续 one-click；
2. terminal record 到首个 one-click checkpoint 使用 complete-record CAS；任一字段 drift
   保留当前 record 并阻断；
3. unrelated/snapshot V2、V1 与无显式 handoff 仍保持现有 fail-closed；
4. later listener 不被 probe/stop 的既有测试继续通过；
5. active ChangeRecord、runtime inventory/test catalog/required gates 随 production path 一起更新；
6. exact candidate 取得 focused tests、完整 15-suite `GATE-SOURCE` completion seal、文档治理
   gate 和 clean-context independent review；
7. 工作树只含 attributable changes，未获授权时不 commit/push；随后重新 rebaseline 并只授权
   一个新 NEXT。

### O1｜Operation entry 与 branch ownership

先建立只读 decision snapshot，在 cold-only SSH/stub capture、retry cleanup 等动作前决定
healthy/cold/recovery/history 路由；command 只做 IPC/preflight/projection。保持 frontend 一个
明确 intent 对应一个 backend operation，禁止新增 frontend mutation chain。

### O2｜Durable prior-runtime transition 与 finalize protocol

在任何 prior stop effect 前发布最小 `PriorStopIntent`，stop 后发布 typed outcome；冻结 exact
ownership 摘要与可安全 restart 的 recipe。另把 runtime binding、terminal journal 和
ActiveRecovery→CleanupOnly 收敛为 fresh boot 可判定、可重放的 finalize 协议。两个协议可在同一
阶段设计，但必须分别测试 crash window，不能变成一条无限扩张的 journal。

### O3｜Cold receipt chain 与 bounded coordinator

在 O1/O2 合同稳定后，把 giant cold coordinator 拆为有限步骤，让 Gateway、Science、Authority
receipt 以 affine handoff 进入 commit bundle；模块拥有自己的 effect/compensation primitive，
coordinator 只拥有顺序和聚合 outcome。保持 healthy 独立。

### O4｜History boundary 与 frontend/backend separation

attention、restore、start 保持三个明确动作；移除 frontend `restore -> one_click` 自动串联。
restore 返回 typed result/新 status，失败后刷新 read model；用户显式发起下一次 start。exact
stop 可复用 shared claim/execute/CAS primitive，但 history session proof 和 retry policy 独立。

### O5｜剩余等待、compensation 与 update provenance

逐个迁移 mode/settings/history/DB recovery/compensation 的锁外等待，不做全局一次性换锁；每片
要求 deterministic owner/generation/identity drift 测试。再决定 durable compensation checkpoint
和跨进程 config lock/CAS。Science candidate adoption 增加不读取用户数据的 predecessor/candidate
差异记录；CSSwitch 每个 production slice 使用 ChangeRecord，只有真实 release 才写 CHANGELOG。

## 10. 测试矩阵与每阶段退出纪律

| 维度 | 必须覆盖 |
|---|---|
| product reachability | `lib.rs` registration、frontend caller、boot/manual command 链；test-only/compiled 明确排除 |
| operation semantics | cold、healthy、history attention、restore、selection/apply、每类 recovery 分别冻结 |
| receipt transfer | exact match、missing、duplicate/second consume、field drift、wrong target、wrong phase/outcome |
| concurrency | barrier/channel seam；lease contention、锁外 status、generation-only drift、identity-only drift、replacement preservation；不依赖 sleep 猜时序 |
| crash windows | prior intent 前后、stop outcome 前后、snapshot register/首 journal、每 checkpoint effect 两侧、binding/finalize、每 compensation step |
| journal compatibility | V1 round-trip、future/malformed V2、complete-record CAS、terminal later-listener guard、cross-process writer experiment |
| frontend | 一个产品 intent 一个 command；无 history restore 自动 one-click；typed status/error projection，不解析 message 控制流 |
| secrecy | journal/DTO/log/update-diff 不含 path secret、key、token、真实路径、组织/对话内容 |
| update record | Science current/candidate version、SHA-256、embedded identity、source、chosen snapshot 和 effect summary；CSSwitch active ChangeRecord/exact-SHA evidence |
| gates | focused suites、quality validators、doc governance、`git diff --check`、clean exact-candidate 15-suite source gate、clean-context review |

每阶段只有满足自己的自动化矩阵、exact-SHA source seal、独立审查和 attributable clean handoff
后才可标记完成。`ENV-BLOCKED`、`NOT-RUN`、artifact/live/release 未授权均不能改写成 PASS。

## 11. 权威文档修订结论

本轮确认以下当前正文需要同步修正：

- `runtime-state-transactions.md`：不能把所有 AppState 持锁都写成短持有/health 锁外；补充
  receipt transfer、成功 finalize 和 terminal Gateway handoff 缺口；history 不再写成 frontend
  自动重新进入 one-click 的稳定目标合同。
- `desktop-control-plane.md` 与 `overview.md`：local Skill 已使用短 `HostBridge` lease/typed
  receipt；journal 已是 typed V2，frontend coarse failure 不再解析 message 控制流。
- `science-runtime.md` / product capability map：content-addressed snapshots 不是可比较的更新
  provenance ledger；当前缺少通用 predecessor/candidate/adoption diff record。
- `quality-kernel.md`：`GATE-S0-LEGACY` 已 retired，当前 source policy 由 active
  `GATE-SOURCE` 承担；不得继续写成尚未切换。
- `known-issues.md` / `branch-lines.md`：旧 S7 路线改为历史背景，当前唯一 NEXT 改为 O0，
  并绑定 `next@9a2d65f`。

这些是文档事实修订，不表示 O0 或后续阶段已获得实现、commit、push 或 release 授权。
