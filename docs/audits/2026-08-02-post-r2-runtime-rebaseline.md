# 2026-08-02 Post-R2 运行架构再基线

状态：日期化只读审计

适用范围：本地 `next` source line；review worktree exact HEAD `69a0305b5a9f1dfff8e4ee3191df15e9331d5179`

最后复核：2026-08-02（Asia/Taipei）

失效条件：`next` 的 runtime state owner、production caller、Science/Gateway stop/start 合同、mutation inventory 或 R0-R2 evidence record 发生变化时重新审计。

本文记录 R0-R2 完成后的只读审计事实与路线修正。当前阶段状态和唯一建议 NEXT 以
[known issues](../../.agents/context/known-issues.md) 为当前权威；本文不授权实现，也不建立
artifact、installed/runtime、live provider/Science/SSH、签名、公证或公开 release 结论。

## 1. 结论

- R0-R2 的 source/unit 收口继续成立，没有发现要求补产品实现或重开阶段的 BLOCK/HIGH。
- 当前 HEAD `69a0305b5a9f1dfff8e4ee3191df15e9331d5179` 是 R2-F 的 evidence-only seal；真正取得
  完整十五 suite gate 的 exact SHA 是 `be961cb27701fd2adfde699c342cdd4dcf8a3d8d`。
- 旧 R3-R11 不能原样继续。`AppState` owner 拆分与长等待移锁之前，必须先把旧 R6 的
  typed Science stop contract 前移为独立窄片。
- F5、local Skill race、history restore 与其他 sibling mutation 仍是真实风险，但它们均是
  R0 冻结并由 R1/R2 明确保留的后续边界，不是 R0-R2 漏收口。
- 唯一建议 NEXT 是 `S1 Typed Science stop contract`；本次审计未进入实现。

审查结果：`clean-context: YES`；`R0-R2 source closure: PASS`；旧 R3-R11 顺序
`REVISION REQUIRED`。

## 2. 现场基线与证据边界

现场只读检查确认：

- review worktree：`/Users/superjj/.codex/worktrees/43e6/CSswitch`；
- 状态：clean detached HEAD；
- exact HEAD：`69a0305b5a9f1dfff8e4ee3191df15e9331d5179`；
- 主工作树本地 `next`：同样指向 `69a0305b5a9f1dfff8e4ee3191df15e9331d5179`；
- `main` worktree：`010a69aa9ffc2696f927dd1fd578b31c69fb2d6a`。

`be961cb..69a0305` 只修改：

- `.agents/context/known-issues.md`；
- `quality/changes/v0.8.3/CHG-RUNTIME-TYPED-JOURNAL-R2-F.json`。

production source、tests 与 `quality/runtime-mutation-inventory.v1.json` 没有差异。因此
`be961cb` 的 source behavior 与当前 HEAD 相同，但完整 gate 的 exact-SHA 结论仍只能绑定
`be961cb`；不能把 evidence-only seal 改写为 `69a0305` 自身执行过完整 gate。

| 证据层 | 本次结论 |
|---|---|
| source/unit | R0、R1、R2 在各自记录的 exact candidate 上 PASS；R2-F 为 `be961cb` |
| current seal HEAD | production/test 与 R2-F candidate 相同；没有新的 exact-HEAD 完整 gate |
| built artifact | `NOT-RUN` |
| 临时安装、installed/runtime | `NOT-RUN` |
| live provider / 真实账号 | `NOT-RUN` |
| Claude Science / Skill domain / SSH server | `NOT-RUN` |
| signing / notarization / Gatekeeper | `NOT-RUN` |
| public release | `NOT-RUN` |

## 3. R0-R2 实际完成与 evidence lineage

### R0｜command-level characterization

R0 建立机器可读 runtime mutation inventory，并用七十四个 source-gate 发现、执行、非
ignored/skipped 的精确 characterization identity 冻结 one-click、healthy reopen、history、
Gateway recovery、mode/settings/stop/quit/native exit、profile、Codex、Skill/doctor/startup
的现有合同。完整 gate 与完成审查绑定
`b6e5d6fd246b1481287940dfd35b4622af43a0c7`。

R0 是行为 characterization，不是产品修复。F5、stop 长持锁、Skill race、无 receipt 的
sibling mutation、`start_proxy` gap 与 terminal best-effort 语义均被保留。

### R1｜typed failure/recovery envelope

R1 建立 `RuntimeError<K>`、typed authority cleanup、`CompensationOutcome` 与 typed
interrupted-Gateway recovery，并删除 production message semantic classification；既有 frontend
DTO keys/text/coarse stage 与操作顺序保持不变。总 completion gate 绑定
`0d2a4604bbcc4297555b9e45c99929b4abb7788c`，evidence-only seal 为
`a8eda86798648e2f7227e1c7413d07062624b7a5`。

R1 子阶段 gate / seal：

| 阶段 | 被完整 gate 验证的 candidate | evidence-only seal / 备注 |
|---|---|---|
| R1-A | `7e3d9db` | `c95d6d0` |
| R1-B | `c5c6cb1` | `bcc48fa` |
| R1-C | `7d3427f` | `05e9c43` |
| R1-D | `b93002a` | `5ce328e`；`dc8bf96` 仅 14/15，不作为 closure |
| R1-E | `ff528eb` | `d25a242` |
| R1-F | `0d2a460` | `a8eda86` |

R1 没有把所有 runtime operation 统一 typed 化。Science stop、Gateway start、history、
mode/settings、Codex 与 profile revocation 仍有 string error 或无 operation receipt 的边界。

### R2｜versioned typed runtime journal

R2 建立 nested V1/V2 schema、typed phase/outcome、immutable candidate fingerprint 与 snapshot
ticket、complete-record CAS、same-process `PreJournalAbort`、interrupted-Gateway recovery outcome
和 V1 fail-closed compatibility。R2 没有移动 checkpoint 时机，没有新增 pre-stop durable
intent，也没有扩大 `runtime_transaction` 到 mode/settings/Codex/downgrade/profile revocation。

| 阶段 | 未采用的候选 | 被完整 gate 验证的 candidate | evidence-only seal |
|---|---|---|---|
| R2-A | `e8ed080` gate 后 completion review FAIL；`7a2e875` 14/15 | `deff0b7` | `0133858` |
| R2-B | `f969d56` 14/15 | `545ad28` | `7148944` |
| R2-C | `ba5e5b0` 14/15 | `14aa958` | `8f709fe` |
| R2-D | `3b434dc` gate PASS 但 review 发现跨重启 drift HIGH | `e185de9` | `4494ae9` |
| R2-E | `638fdd5` gate FAIL | `8b14715` | `1e9e3c6` |
| R2-F | — | `be961cb` | `69a0305` |

候选 gate、completion review、repair、重新 gate 的分离实际挡住了多项 source-contract 与
recovery 漏洞，因此 R1/R2 完成质量评为强；该评价仍只属于 source/unit 层。

## 4. 当前 state owner、caller、长等待与剩余风险

### 4.1 process-local owner 与长等待

[`AppState`](../../desktop/src-tauri/src/lib.rs) 同时拥有 Gateway、Science、boot、history 与
cleanup mirror。`commands/runtime/lifecycle.rs::stop_all_inner_cmd` 在持有 `AppState` mutex
时调用 `runtime/science/lifecycle.rs::stop_sandbox_with_launch_token`；后者可同步执行 stop
script、TERM/KILL 与多轮端口等待。高频 `commands/runtime/status.rs::status_inner` 也必须先
取得同一 mutex 才能复制 read model。

Science 已有 `ScienceRuntimeIdentity`、`ScienceManagedLaunchToken` 与 managed launch commit
error，但 stop façade 仍返回 `Result<(), String>`。在没有 typed request/ownership receipt/
outcome 与 stale-result CAS 前直接把等待移出锁，会产生 replacement runtime 被旧 stop 结果清除
的风险。因此旧 R3 的第一步不能只是机械字段拆分。

### 4.2 F5 pre-intent gap

`runtime/sandbox_session/one_click.rs::one_click_login_with_options` 仍在 authority snapshot 与
首个 V2 checkpoint 前停止 prior Science。stop helper 返回失败时底层可能已有副作用；verified
stop 后到 snapshot/manifest 登记前崩溃，则重启后没有 durable prior-stop recovery record。

`PriorStopIntent/Outcome` 会改变 crash recovery 合同，必须与 `AuthorityTransaction` 的纯接口
等价提取分开，不得作为搬文件或 typed wrapper 顺带实现。

### 4.3 local Skill race

`commands/skill_install.rs::install_local_skill_package` 在 picker 前后两次读取并复核
`ScienceHostContext`，但第二次复核到 package commit/OPERON attach 之间不取得 `Lifecycle`
或等价 mutation lease。stop/switch 仍可在最终复核后交错。修复应使用短 host-bridge lease 与
typed host receipt，不能把整个 picker/download 放入全局锁。

### 4.4 sibling mutation 与失败链

当前 production inventory 仍包含：

- `restore_history_choice`：有 frontend caller；exact stop 后的 credential/marker mutation 无
  journal、snapshot 或 prior restart；
- mode/settings：stop-before-config/SSH commit，后续失败不恢复 prior runtime；
- Codex auth/network/disable/downgrade：有独立 supervisor lease，但无共同 runtime operation
  receipt；
- profile revoke：config-first，applied 分支随后停 Gateway并保留 Science；
- native exit：`ExitRequested` 与 `Exit` 可重复执行 best-effort cleanup，语义有意不同于 command
  quit；
- registered `start_proxy`：无 bundled caller，可更换 live Gateway、写 path secret/bridge key，
  但不提交 binding/journal，且 launch recipe 可遗漏派生的 effective Science host context。

这些 operation 不能由 one-click coordinator 代替，也不能无差别进入同一种 durable transaction。

## 5. 对旧 R3-R11 的处置

| 旧阶段 | 新处置 | 理由 |
|---|---|---|
| R3 state owners | 保留但拆分；首片合并旧 R6 的 typed Science stop contract | 单纯字段搬迁不能安全移出外部等待 |
| R4 mutation lease | 保留并拆成 intent/destructive/host-bridge/terminal domain | local Skill race 仍存在；单一粗锁会错误统一语义 |
| R5 AuthorityTransaction | 拆分 | 纯接口等价提取保留；`PriorStopIntent/Outcome` 单独授权 |
| R6 ScienceHostAdapter | stop contract 前移；launch spec/receipt 后续完成 | stop 是 R3 owner/CAS 的前置，不应等到旧 R5 后 |
| R7 GatewayController | 缩小 | interrupted recovery typing/V2 outcome 已由 R1/R2 完成；剩 start/reuse receipt、recipe 与 `start_proxy` 去留 |
| R8 RuntimeCoordinator | 保留但按 operation family 分片 | cold/healthy/history 与其他 sibling mutation 的 commit/compensation 不同 |
| R9 bridge/doctor | 拆分并延后 | Skill lease 前移；doctor inspect/reconcile 是独立产品/API 决策 |
| R10 transport policy | 移出本路线 | 属于独立安全/兼容性产品线，不能混入等价 runtime 重构 |
| R11 host-neutral | 从当前有限路线取消 | 仅保留未来方向，不构成近期实施授权 |

## 6. 新的有限路线

### S1｜Typed Science stop contract——唯一建议 NEXT

要消除的不确定性：现有 caller 如何在不解析 message 的前提下区分 verified stopped、identity
drift、signal failure、exit unconfirmed 与 receipt cleanup failure。

范围：建立 behavior-preserving 的 Science stop request、ownership receipt 与 typed outcome，
迁移 mode/settings/stop/history/one-click compensation/DB restart/native exit 等现有 caller；保持
现有锁时机、command/DTO/text、stop script、TERM/KILL/wait 顺序与 native-exit best-effort 语义。

前置：R0 stop characterization、R1 typed envelope、现有 runtime identity 与 managed launch token。

退出条件：全部 production stop caller 不再从字符串决定控制流；现有 R0 identity 与新增 typed
projection/source contract 通过；exact candidate 取得 15-suite `GATE-SOURCE` completion seal 与
clean-context independent review。

非目标：移动锁、拆 `AppState`、新增 mutation lease、F5 durable intent、Gateway/Skill 改造、
artifact/live/release 验证。

### S2｜Science process-local owner 与锁外等待/CAS

在锁内 claim exact owner/receipt，锁外执行 stop，锁内按 generation/identity CAS 回写；首片只覆盖
`stop_all`。退出时受控 stop wait 不再阻塞 status 取得 read model，且 replacement runtime 不会被
陈旧结果清除。非目标是改变 stop policy 或推广到 Gateway。

### S3｜Typed RuntimeMutationLease 与 local Skill race closure

建立 intent、destructive、host-bridge 与 terminal domain。picker/download 保持锁外；最终 host
context recheck、package commit、attach/readback 使用正确的短 lease/receipt。非目标是把所有
operation 强塞进全局 durable journal。

### S4｜ScienceHostAdapter 等价 façade

收拢 launch spec、environment exposure、managed receipt、health 与 stop outcome；保留 Rust + shell
双层 fail-closed 和当前用户可见行为。非目标是 host-neutral extension。

### S5｜AuthorityTransaction 等价提取

复用现有 snapshot ticket、typed cleanup 与 `CompensationOutcome`，只搬 capture/restore/cleanup
接口。`PriorStopIntent/Outcome` 明确不在本阶段。

### S6｜GatewayController receipt 与 `start_proxy` 决策

建立 typed start/reuse/health/catalog/recipe receipt。先单独决定无 bundled caller 的
`start_proxy` 保留或移除；保留时记录 secret/key durable effects、effective Science host context、
完整 prior/new recipe 与 `binding_not_committed`。

### S7｜Coordinator cold/healthy/history 分片

先分别迁移 cold 与 healthy，保持各自 commit/rollback 顺序；history restore 独立建 plan，并明确
保留还是改变“stop 后失败由用户重试”合同。完成后强制再次 rebaseline，才决定
mode/settings/Codex/profile/native-exit 的后续 operation-plan 路线。

## 7. R0-R2 是否需要补充收口

默认结论：**不需要**。

理由：

1. R0、R1、R2 的阶段目标与 non-goal 清楚，均有 exact candidate、完整 source gate、聚焦矩阵与
   clean-context completion review；
2. 当前剩余 F5、锁等待、Skill race、history/sibling mutation receipt 缺口均被明确记录为后续
   风险，没有被错误宣称已经解决；
3. artifact、installed/runtime、live provider/Science/SSH、签名、公证与 release 是明确未运行的
   更高证据层，不是 source-only R0-R2 的退出条件；
4. `69a0305` 没有自己的完整 gate 是 evidence-only seal 的刻意边界。若单独希望建立
   “`next@69a0305` exact-HEAD SOURCE-GREEN”，可以再跑一次完整 gate，但这只属于可选证据增强；
   下一实施候选本来就必须重新取得 exact-SHA gate，默认没有必要重复补跑当前 seal HEAD。

只有出现以下新事实才重开 R0-R2：inventory 漏掉 production caller；已有 characterization identity
未被 gate 实际执行；typed recovery 仍由 message 控制；V1/V2 reader/CAS 可被合法输入绕过；或
对应 exact-SHA evidence/hash 无法复核。本次审计没有发现这些情况。

## 8. 本轮未执行

本轮是只读 rebaseline，没有重新运行 15-suite source gate，也没有运行 artifact、installed App、
live provider、真实账号、Claude Science、SSH、签名、公证、Gatekeeper 或公开 release 验证。
文档治理修改的验证结果应记录在承载本审计的后续提交任务中；未经独立授权，不由本审计自动
进入 S1 实现。
