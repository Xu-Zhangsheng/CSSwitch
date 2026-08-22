# 2026-08-03 H1–H3 后 production flow 再基线

状态：日期化只读审计

适用范围：本地 `next` source line；审计起点 exact HEAD `7699e89212f2ceaf2c07e7358f6cc2f8bf3a21d2`

最后复核：2026-08-03（Asia/Taipei）

失效条件：production one-click、healthy reopen、history、profile、recovery caller，H1–H3
handoff/journal/finalize 合同，或本文绑定的 exact HEAD 发生变化时重新审计。

本文在 H1/H2/H3 source-only closure 后重新检查五类 product-reachable flow。当前开放问题和
唯一建议 NEXT 以 [known issues](../../.agents/context/known-issues.md) 为当前权威；本文不授权
源码实现，也不建立 artifact、installed/runtime、live provider/Science/SSH、签名、公证或
公开 release 结论。

## 1. 结论

H1–H3 的 backend recovery/control-transfer 合同本身仍成立，但本轮发现一个新的 `HIGH`：
H3 新增的 degraded success-finalize outcomes 没有被 manual UI 与 auto-boot 按 action/readback
消费。`degraded + manual_recovery_required` 表示 finalize 未确认提交；而
`degraded + cleanup_required` 在 normal start 可伴随已提交 binding，在 history attention 的
`ClearJournal` action 却不提交 binding。frontend 对这些状态都会落入 success/applied，auto-boot
也会进入 `BootState::Ready`。

三个旧 HIGH 的 backend 边界为：

- interrupted-Gateway terminal record 只通过同一 command 持有的 affine handoff 进入 one-click，
  并由完整 V2 record、active profile 和旧 binding 共同守卫；
- prior Science stop effect 前后已有 durable intent/outcome；不能证明 exact stop 时保留 journal
  并 fail closed；
- success finalize 已是 `intent -> authority CleanupOnly -> binding + journal clear`，两个 crash
  window 都由同一 finalize intent 重放。

因此 `O1 Operation entry 与 branch ownership` **不再是当前最高优先级**。新的有限 sole NEXT
必须先是 `H4 Finalize-degraded consumer contract`：只修 manual UI 与 auto-boot 对 H3 degraded
DTO 的分类和 read-model publication，不修改 H1–H3 backend 协议。H4 source closure 后
再次只读 rebaseline；若没有新的 HIGH，`O1-A Typed one-click entry decision` 才恢复为最高架构候选。

本轮另记录五个 `MEDIUM`：branch decision 太晚、frontend 自动串联 history restore 与 start、
history restore 缺少 durable crash/progress 合同、以及 lease/config CAS 仍主要是进程内保证。
第五项是 H3 把所有 final config writer error 统一投影为 degraded，却没有区分 config atomic
commit + rollback 双失败后的磁盘不确定态。它们没有达到 HIGH：当前 production 路径仍使用统一 mutation lease、exact managed Science
stop、完整 record/config authority recheck 和 fail-closed journal；没有发现确定性的普遍数据破坏、
未知 listener 接管或凭证边界失守。

## 2. 现场与证据边界

审计开始时只读确认：

- worktree：`/Users/superjj/ccproj/CSswitch`；
- branch：`next`；
- exact HEAD：`7699e89212f2ceaf2c07e7358f6cc2f8bf3a21d2`；
- 起始状态：clean；
- HEAD subject：`docs(quality): seal runtime recovery source evidence`。

H1–H3 tested candidate 是 `9d7133285c32e8303cc47b6ff91b25e76dccec6f`。从该 candidate 到
本审计 HEAD，`desktop/`、`test/`、`quality/runtime-mutation-inventory.v1.json` 与
`quality/test-catalog.v1.json` 无差异；后续只更新 known-issues 与 active ChangeRecord 的
evidence seal。因此本轮可复用该 candidate 的 source-test 身份，但不能声称当前 audit commit
自身运行过完整 gate。

| 证据层 | 本轮结论 |
|---|---|
| current source / tests inspection | 已完成；绑定 `next@7699e892` |
| H1–H3 exact-candidate source seal | `9d713328` 的 15/15 `GATE-SOURCE` PASS；历史证据，不是本轮重跑 |
| 本轮文档治理 | 由承载本审计的最终验证记录决定 |
| 本轮 exact-HEAD source gate | `NOT-RUN`；本轮未修改 production/test |
| built artifact / installed runtime | `NOT-RUN` |
| live provider / 真实账号 / Science / SSH | `NOT-RUN` |
| signing / notarization / Gatekeeper / release | `NOT-RUN` |

## 3. 五类 production flow 当前边界

| flow | product-reachable entry / coordinator | persistent owner | process-local owner | H1–H3 后边界 |
|---|---|---|---|---|
| cold one-click | frontend/boot -> `commands/runtime/one_click.rs::one_click_login_cmd` -> `sandbox_session::one_click_login_with_options` | V2 journal、binding、authority manifest/snapshot、Science managed receipt | `AppState`、Gateway/Science receipt、rollback context | prior stop 已有 intent/outcome；success 已有 replayable finalize；H3 pending finalize 的 consumer projection 错误；长 coordinator 与 late branch decision 仍在 |
| healthy reopen | 与 cold 共用 command，在 runtime/login/binding 判定后进入 `one_click/healthy_reopen.rs` | binding 与可选 exact handoff journal；不建 authority snapshot | 已确认 Science identity、Gateway receipt、AppState snapshot | 不重启 Science；Gateway/catalog 后提交 binding，route 为后置 best-effort；仍先经过部分 cold/recovery preparation |
| history attention / restore | attention 在 cold coordinator；restore 是独立 `restore_history_choice_command` | 正常 attention 返回前已 finalize；H3 的已覆盖 safe failure 保留 finalize journal，atomic-uncertain 必须 readback/unknown；restore 没有独立 durable operation journal | `AppState.history_recovery`、opaque one-shot refs、typed Science quiescence | attention/restore/start 实际是三个事务；H3 degraded consumer 会伪报成功；frontend 仍在 restore 成功后自动发起 one-click |
| profile selection / apply | `set_active_profile` 只保存 selection；下一次 product one-click apply | `active_id`、后续 journal/binding | 当前 runtime identity | `set_active_profile_txn` 仍是 compiled/test-only，不得当作 product flow；H1 的 profile-switch journal 只服务 recovery handoff |
| recovery | command 先 replay success finalize，再恢复 interrupted Gateway；V1 environment、DB recovery、compensation 分别保留独立规则 | typed journal、authority manifest/snapshot、managed receipt | exact listener/child/receipt、rollback context | terminal Gateway 与 finalize 已可重放；prior-stop `NotStopped/Unknown`、中断 compensation 仍 fail closed/manual，不是通用 retry engine |

关键 source anchors：

- command preflight、lease、finalize replay、Gateway recovery 与 handoff：
  `desktop/src-tauri/src/commands/runtime/one_click.rs:143-265`；
- branch decision 前仍执行 SSH capture 与 pending cleanup retry：
  `desktop/src-tauri/src/runtime/sandbox_session/one_click.rs:2260-2418`；
- H1 exact terminal handoff / first checkpoint CAS：
  `desktop/src-tauri/src/runtime/proxy_lifecycle/recovery.rs:53-75,286-490` 与
  `desktop/src-tauri/src/runtime/sandbox_session/one_click.rs:554-600,860-949`；
- H2 durable prior stop：
  `desktop/src-tauri/src/runtime/sandbox_session/one_click.rs:717-830,2490-2608`；
- H3 finalize/replay：
  `desktop/src-tauri/src/runtime/sandbox_session/one_click.rs:1601-1769,2740-2822,3211-3270` 与
  `desktop/src-tauri/src/runtime/sandbox_session/pending_cleanup.rs:955-1028`；
- H3 degraded 的 manual UI / auto-boot consumer：
  `desktop/src/runtime-controller.js:125-153` 与 `desktop/src-tauri/src/lib.rs:392-398,484-498`；
- frontend 现有 config read model：
  `desktop/src-tauri/src/runtime/profile.rs:98-174`；
- history restore 与 frontend chain：
  `desktop/src-tauri/src/commands/runtime/one_click.rs:269-430`、
  `desktop/src/runtime-controller.js:73-89`；
- profile selection-only product contract：
  `desktop/src-tauri/src/commands/profiles.rs:512-565`。

## 4. Findings

### HIGH-1｜H3 degraded 被 production consumer 误判为 applied / Ready

H3 在 authority conversion 或最终 config commit 失败时调用
`preserve_interrupted_success_finalize`：DTO 被改为 `status=degraded` 与
`recovery_status=manual_recovery_required`。authority prepare failure 与已覆盖的 config
pre-commit failure 会保留旧 binding 与 exact finalize journal；normal start 与 history attention
两条 production 分支都可返回这个状态。

另一个可达组合来自 `AuthorityTransaction::prepare_success`：cleanup-only 转换成功后，私有快照
删除/manifest clear 失败会返回 `status=degraded / recovery_status=cleanup_required`，随后仍执行
finalize completion。normal start 的 action 是 `CommitBinding`，因此可提交 binding；history
attention 的 action 是 `ClearJournal`，因此只清 journal而不提交 binding。相同 status/recovery
组合并不拥有单一 applied 语义。

manual UI 只特殊处理 `attention + history_choice_required` 与 `status=error`；任何其他状态都进入
success branch，并无条件把 `selection_pending=false`、`applied_profile_id=active_id`。因此 H3
的上述 safe failure 中，持久 config 仍是旧 binding/待重放 journal，页面却确定性发布“当前选择
已应用”。history attention 的 finalize failure 也不会进入 history-choice branch，而会落入
同一个伪成功分支。

auto-boot 同样只区分 `attention` 与 `error`；上述 degraded DTO 会落入 default success branch，
把 `AppState.boot` 设为 `Ready`。现有 Rust tests 验证 backend finalize 分支，却没有把这些 DTO
穿过 frontend/boot consumer 检查 action 与真实 read-model publication。
现有 frontend `get_config` 只提供 `applied_profile_id` 与 `selection_pending`，后者同时
覆盖 journal-open、无 binding 和 binding mismatch，不足以完成这个分类。

这是 product-reachable 的跨层状态合同错误，不是文案问题，严重度为 HIGH。不能把所有
`degraded` 一概当成未应用，也不能把所有 `cleanup_required` 当成 binding 已提交。consumer
必须按 `status + recovery_status + action + backend readback` 区分 normal start、history choice
与 finalize/manual recovery。

### MEDIUM-1｜healthy/cold branch ownership 仍决定得太晚

one-click 在识别 healthy/cold 之前执行 system-SSH prevalidation、可选 stub transaction capture、
pending authority cleanup retry 和 Science probe。只有在确认 running Science、login intact 与
binding match 后才进入 healthy helper。结果是 healthy reopen 仍继承部分 cold/recovery 的准备和
失败面；后续先拆 cold receipt chain 会把这条隐式依赖固化。

这不是新的 HIGH：上述动作仍位于同一 `Destructive` lease 内，branch 前没有执行 prior Science
stop 或 protected authority capture；config/handoff drift 仍 fail closed。但它是下一步最先需要
收紧的维护边界。

### MEDIUM-2｜history restore 仍由 frontend 自动串联 one-click

`restore_history_choice` 成功后，frontend 直接执行 `runOneClick(null)`。backend 的 restore 与
one-click 是两个独立注册 command，各自重新取得 mutation lease 和重新验证状态；因此没有绕过
backend guard，但一个用户 intent 暗中触发第二项 destructive operation，失败、重试和状态投影
不能独立表达。

### MEDIUM-3｜explicit history restore 没有 durable operation journal

restore 入口要求当前没有 runtime transaction，随后在 `AppState` 锁内 exact-stop 当前受管
Science，再直接恢复所选 credential identity。候选 identity、active profile、port 与 typed
quiescence 都会复核，单个文件写也使用安全原子替换；但 stop、credential/marker 写入之间的
crash window 没有 durable progress/outcome，也没有 subprocess crash characterization。

当前证据不足以升级为 HIGH：未发现它会接管未知进程或无保护地覆盖不匹配候选；风险是中断后的
恢复/可解释性缺口。它应在 O1-A 后作为独立 history boundary 候选，不并入 entry slice。

### MEDIUM-4｜mutation 与 config CAS 仍主要是 process-local

四个 `RuntimeMutationDomain` 共享同一进程内 mutex；`config::update_result` 也只在进程内锁住
load-modify-save。single-instance plugin 降低正常双 UI 并发，但不是跨进程 advisory lock。
production command 当前正确持 lease，完整 V2/config authority guard 也能拒绝观察到的 drift；
因此只能声称 process-local serialization + atomic file replacement，不能声称跨进程事务。

### MEDIUM-5｜final config writer error 没有区分 rollback-uncertain

`complete_one_click_finalize` 把 `config::update_result` 的任意错误统一上投影为
`degraded/manual_recovery_required`。通常的 pre-commit failure 或 commit-sync failure + 成功
rollback 会保留旧 config；但 atomic writer 在 rename 后 commit sync 失败、随后恢复旧文件也失败时
返回 `AtomicRollbackUncertain`。此时目标可能已经是新 binding + 无 journal，也可能仍是旧状态。

现有 H3 production seam 在 config 写入前失败，只证明 safe failure 会保留 exact finalize record；
不能把这个结论外推到所有真实 filesystem error。该不确定态仍返回 conservative manual recovery，
没有证据显示它会造成未知 listener 接管，因此本轮定为 MEDIUM；H4 consumer 必须按重新读取到的
真实状态投影，读取失败时保持 unknown/manual，不能预设一定是旧 binding + open journal。

## 5. H1–H3 关闭了什么、没有关闭什么

| slice | 已关闭 | 明确未关闭 |
|---|---|---|
| H1 terminal handoff | 同一 production command 中 exact terminal record 到 one-click 的 affine 交接；later-listener 防护与完整 CAS | 一般化 recovery router、跨进程 handoff、任意 V2 自动接管 |
| H2 prior stop | stop 前 intent、stop 后 typed outcome、credential-free restart identity；不能证明时保留 journal | fresh-process 自动 restart、`NotStopped/Unknown` 自动处置、history/其他 stop 的 durable 协议 |
| H3 success finalize | 两个 success crash window 的同一 intent 重放；authority 先 CleanupOnly，再 binding/journal 原子提交 | final writer 的 `AtomicRollbackUncertain` 未单独投影；另未覆盖 compensation 逐步重放、history restore journal、跨进程 CAS、所有 stop 的 lock-free owner claim |

H1/H2 的未关闭面继续 fail closed 或保留独立 owner。H3 的 logical CAS/replay 保持 fail-closed，
但 final config writer 仍有单独的 rollback-uncertain 边界；它新增的 degraded DTO 在 production
consumer 形成了上述 HIGH。必须先闭合 consumer contract，才能进入 O1-A。

## 6. Sole NEXT：H4 Finalize-degraded consumer contract

目标：为 one-click degraded DTO 建立不依赖 message 的有限 consumer classifier，按
`status + recovery_status + action + backend readback` 区分 normal start、history choice 与
finalize/manual recovery。manual UI 与 auto-boot 不能从 active selection 或单一 degraded 字段
猜测 applied/Ready；读取失败或 atomic outcome 不可确认时保持 unknown/manual。

允许范围：

- manual `runOneClick` 在 success publication 前使用 typed status/recovery/action classifier，并从
  backend read model 刷新 `selection_pending` 与 `applied_profile_id`；读取失败时保持
  unknown/manual，不得用 active selection 猜 applied binding，也不得预设 journal 一定仍存在；
- 允许新增一个最小、脱敏、只读的 typed finalize-consumer-state projection，仅向
  manual/boot classifier 提供 exact journal disposition 与 binding relation；不暴露 transaction
  record、path、credential 或其他 config 内容，不成为新 mutation owner；
- `manual_recovery_required` 先回读 binding 与 finalize journal：journal 仍开放时不发布
  applied/choice-ready，下一次显式 one-click 才可先 replay；journal 已清时必须按
  exact binding 与 history action 分别投影，不得仍假定可 replay；回读失败或组合不一致时
  保持 unknown/manual；
- `action=history_choice_required + cleanup_required` 仍按 choice flow 处理，可显示 cleanup warning，
  但不得发布 applied/Ready；normal-start `cleanup_required` 只有在 readback 确认 binding 后才可
  发布 applied，并继续显示 cleanup pending；
- auto-boot 使用同一 classifier：manual/unknown 进入可见 non-Ready recovery，history choice 进入
  attention，只有 readback 支持的 normal-start outcome 才能进入 Ready；完整 DTO 必须保留；
- 增加 frontend controller 与 Rust boot production-consumer regressions，并更新 active
  ChangeRecord、test catalog、runtime inventory / required gate identity（仅在实际受影响时）。

明确禁止：

- 不改变 H1 terminal handoff、H2 prior-stop schema/outcome、H3 finalize/manifest/replay 顺序；
- 不新增或重命名 H1–H3 one-click outcome DTO keys/schema；除上述最小只读
  projection 外不扩张 backend surface，不从 message/cleanup text 反推控制流；
- 不顺带移除 history restore -> one-click 自动串联，不新增 history durable journal；
- 不进入 O1 branch ownership、cold coordinator/receipt 拆分、compensation checkpoint、跨进程
  lock/CAS、剩余锁外等待或 Science update provenance；
- 不改变 profile selection-only、正常 success 与 cleanup-only degraded 的 backend 产生语义，
  也不改变 artifact/live/release 语义。

退出条件：

1. manual normal-start H3 degraded 不从 active selection 猜 applied/pending；刷新成功时与实际
   binding/journal 一致，刷新失败或 outcome uncertain 时保持 unknown/manual，并显示 recovery；
2. history `manual_recovery_required` 在 journal 仍开放时不发布 choice-ready，下一次显式
   one-click replay 后才按新结果显示 choices；journal 已清时按 history action 投影
   choice/attention，不得显示可 replay；history `cleanup_required` 保持 choice/attention +
   cleanup warning，且不发布 applied；
3. auto-boot 的 manual/unknown degraded 不进入 `BootState::Ready`；history cleanup-required 进入
   attention 而不是 Ready；完整 DTO 可由可见 recovery surface 消费；
4. `status=ok`、`status=attention`、`status=error`、normal-start `cleanup_required`、history
   `cleanup_required`，以及 `manual_recovery_required` 的 journal-open / journal-cleared / readback-failed
   组合均有 counterexample regression，证明没有按单一 status/recovery 粗暴分类；
5. 新增的 read projection 只返回分类所需 typed disposition/relation，不携带完整
   journal、path、credential 或可写能力，并有 source-contract / serialization 回归；
6. H1 later-listener、H2 prior-stop intent/outcome、H3 两个 finalize replay window 的既有身份继续 PASS；
7. focused tests、quality/document governance、clean exact-candidate 15-suite `GATE-SOURCE` 与
   clean-context independent review 全部闭合；
8. attributable changes 与证据边界清楚；未获独立授权时不 commit/push；完成后重新 rebaseline，
   只选择一个新的 sole NEXT，届时重新判断 O1-A。

H4 是有限修复候选，不是本次审计自动授予的实现许可。

## 7. 审查结论

两路辅助 clean-context source review 对新 HIGH 发生分歧：一路只报告四个 MEDIUM，另一路发现
H3 degraded consumer 缺口。主窗口沿 production DTO 到 manual UI/auto-boot 逐行复核后确认该
HIGH 成立，并据此否决原 O1-A sole NEXT。最终文档候选的 clean-context gate 由本次任务交付
记录；任何后续实质修订都会使该 gate 失效。review PASS 不得外推为
source/artifact/live/release PASS。
