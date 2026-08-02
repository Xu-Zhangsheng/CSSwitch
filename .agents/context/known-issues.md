# 当前已知问题与证据缺口

状态：当前；按 v0.8.4 release source 与 2026-08-01 R0 分片基线整理

最后复核：2026-08-02（Asia/Taipei）

失效条件：对应 change/bug record、Science 版本、release source、artifact 或 installed/live 证据改变时，受影响条目立即失效并须按当前版本重审。

已解决历史放入 CHANGELOG 或 dated evidence，不在这里重复。

## Runtime 架构分片进度

`R0-0` 已收口：恢复仓库安全边界，并把 runtime mutation inventory
的完成条件收紧为“每个 characterization 都是 source gate 发现且实际
执行的精确身份，不得位于 ignored/skipped 集合”。该条件已在 R0-H 通过
command-level parent、source gate 精确身份与独立完成审查收口；helper 或源码
顺序检查本身仍不构成 characterization 证据。

`R0-A` 已收口（source）：五个 source gate 身份分别冻结 one-click prior-stop
错误、durable snapshot 到首个 journal 的中断窗口、snapshot capture rollback、
DB recovery restart 未证实候选，以及 late-failure prior-runtime restore。测试均在
隔离 HOME、fake Science、mock upstream 与动态 loopback 端口内执行；结论不扩展到
installed/live provider 或产品行为修复。

`R0-B` 已收口（source）：七个 source gate 父身份冻结 healthy reopen 的既有 marker
只读复用、marker bootstrap 失败、marker 写入后 Gateway/catalog rollback，以及显式
history restore 的 stop 前拒绝、exact stop 后 config/candidate/credential 失败、全量
reference 轮换和成功后仍由 frontend 另起 one-click。隔离边界为临时 HOME、fake
Science、必要时 real local Gateway、mock upstream 与动态 loopback；结论不扩展到
installed/live provider 或产品行为修复。

`R0-C` 已收口（source）：四个 source gate 身份冻结 interrupted Gateway recovery
在持久化 recovery stage 后的 stopped、identity recheck 拒绝与 stop failure 结果，
以及 start-gateway-only 启动失败时保留 runtime binding、transaction journal 与
既有 Science，同时保留已写入的 path secret / bridge key。隔离边界为临时 HOME、
fake Gateway / Science 与动态 loopback；结论不扩展到 installed/live provider 或
产品行为修复。

`R0-D` 已收口（source）：六个 source gate 身份冻结 mode 的 stop/config failure、
settings 的 stop/SSH revoke/config failure、frontend stop 的 Science-error partial
result、command quit 的 exit-on-success-only，以及 native ExitRequested/Exit 的可重复
best-effort cleanup 与 AppState Drop 的 tracked Gateway 兜底。测试均在临时 HOME、
fake Science identity、fake tracked Gateway child 与动态 loopback 端口内执行；结论
不扩展到 installed/live provider、artifact 或产品行为修复。

`R0-E` 已收口（source）：六个 source gate 身份冻结 profile select 的 changed/no-op
与拒绝路径、connection intent 的 selected/applied 与 validation tri-state、preset sync
的 stale/open transaction 和四种角色下 `selection_pending` 前后结果，以及 key clear /
profile delete 的 selected-only、applied-only、selected=applied、neither 矩阵。撤销测试
使用短寿命隔离假 Science 子进程并确认同一 PID 保持运行；结论不扩展到
installed/live provider、artifact 或产品行为修复。

`R0-F` 已收口（source）：九个 `op.codex-*` 操作的二十一个精确 source gate
身份冻结 login 的 stop-before-sidecar 与失败后不恢复、cancel 的 operation-ID 绑定、
exact NDJSON、重复请求与写失败终态化、profile ensure 的认证与幂等边界、logout 的
stop-before-revoke、refresh 的 generation guard 与成对回滚、catalog 的 scratch/formal
共享 cache epoch、cache/invalidation 顺序、models 401 guarded refresh wiring 与 scratch
child 回收、disable/network 的 stop-before-config，以及 downgrade 的
stop → export → backup → v2 publication/latch → direct exit 顺序和安全/不确定失败结果。
测试边界为临时 HOME、fake sidecar、fake managed children、私有 auth/catalog 状态与动态
loopback 端口；结论不扩展到 installed/live provider、artifact 或产品行为修复。

`R0-G` 已收口（source）：九个精确 source gate 身份冻结本地 Skill picker 期间
runtime context 变化的提交前拒绝、真实 archive 文件提交后的 attach 失败保留，Gateway
bridge startup 的 interrupted recovery、finalization recovery，以及真实 request operation
中的 install/uninstall 文件和绑定分离结果；doctor 身份执行完整 command body 与真实
reconcile wrapper，冻结 diagnostics-before-reconcile、marker invalidation/host partial
mutation；startup 身份执行 setup 使用的 config sequence，冻结 migration 的
backup-before-single-v4-commit 与 setup 忽略首次错误后 boot prepare 失败。测试只使用临时
目录/HOME、fake Science executable/context、私有 bridge mailbox、fake doctor asset 与
线程绑定 mock host mutation；结论不扩展到 installed/live provider、artifact、Science
Skill runtime 或产品行为修复。

`R0-H` 已收口（source）：整体 inventory 共绑定七十四个精确 characterization
身份，全部由 source gate 发现、实际执行，且不位于 ignored/skipped 集合；desktop
身份闭合为 522 discovered / 41 approved ignored，frontend 闭合为 39 个精确身份。
完整十五 suite `GATE-SOURCE` completion seal 与 clean-context 独立完成审查共同建立
本次 R0 source closure。结论仅冻结当前 command-level 行为，不授权 R1-R11，也不扩展
到 artifact、installed/live provider、Science、SSH、签名、公证或公开发布。

| 阶段 | 状态 | 边界 |
|---|---|---|
| `R0-A` | DONE | one-click prior stop、snapshot、DB restart 与 prior runtime restore |
| `R0-B` | DONE | healthy reopen 与 history restore |
| `R0-C` | DONE | interrupted Gateway recovery 与 start-gateway-only |
| `R0-D` | DONE | mode/settings/stop/quit/native exit |
| `R0-E` | DONE | profile select/update/sync/revoke |
| `R0-F` | DONE | Codex mutation |
| `R0-G` | DONE | Skill/bridge/doctor/startup migration |
| `R0-H` | DONE | 整体 inventory、source gate 与最终独立审查收口 |

R0 已全部收口。不得把本次 source closure 外推为产品修复或更高证据层 PASS。

## R1 typed failure/recovery envelope 进度

`R1-A` 已收口（source）：`runtime/failure.rs` 建立 crate-internal
`RuntimeError<K>` envelope，将 domain、phase、recovery disposition、environment
exposure 与 sanitized cause chain 变成 typed field；既有 `TypedOneClickFailure` 作为
one-click specialization 保留，frontend DTO keys、stage/recovery/environment 字符串与
人读 message 不变。过渡性的 `recovery_from_diagnostic_codes` 仍保留，本阶段没有提前
进入 authority cleanup、one-click compensation 或 interrupted Gateway recovery。

focused source/unit 已实际执行并 PASS：`failure::` 8 tests、
`science_operation_failures_have_stable_structured_stages` 1 test、
`auto_boot_rejects_structured_runtime_failure` 1 test，以及沙箱外完整 Rust lib
482 passed / 0 failed / 41 explicitly ignored。相同完整 lib 命令在受限沙箱内因动态
loopback / process inspection 权限产生 53 个 `Operation not permitted`，不计为产品失败。
clean-context candidate review 已 PASS，零 BLOCK/HIGH/MEDIUM/LOW，并明确不包含完整 gate。
随后 source implementation exact SHA `7e3d9db6db089fa7a143bc742bc11162aa6b42ae`
取得十五 suite `GATE-SOURCE` completion seal PASS：run id
`4b7c5c8ba0c06a55d0c3fc3d1e33a5e7`、runner exit 0，manifest 含十五个 test
result 与十五个 source observation；desktop 身份为 523 discovered / 523 executed /
41 approved ignored / 0 skipped。该 exact SHA 的 clean-context 独立完成审查亦 PASS，
零 BLOCK/HIGH/MEDIUM/LOW，并核验 DTO、typed control-flow、safe cause、范围边界、gate
identity 与全部 evidence 引用哈希。

`R1-B` 已收口（source）：`pending_cleanup.rs` 为 authority snapshot
registration、cleanup identity validation、cleanup、retry 与 manifest clear 建立 typed
phase / failure / recovery authority / success outcome；`prepare_success` 与 one-click
compensation 不再通过 `recovery_status=cleanup_required` 文本判断恢复语义。cleanup
manifest schema/CAS、marker 与目录 identity 校验、mode/owner/inode/tombstone 和 bounded
remove 边界未改变；现有 success / degraded DTO keys、值和 message 保持不变。

focused source/unit 已实际执行并 PASS：`cleanup_recovery` 4 tests 与 transaction contract
1 test；完整 Rust lib 在沙箱外为 482 passed / 0 failed / 41 explicitly ignored。相同命令
在受限沙箱内仍是既有的 429 passed / 53 `Operation not permitted` / 41 ignored，失败集中于
动态 loopback 与 process identity，不计为 R1-B 产品失败。质量 metadata 与文档治理测试
均 PASS。source implementation exact SHA `c5c6cb1a05577ac3ddfa18a633fb2277733e50d5`
取得十五 suite `GATE-SOURCE` completion seal PASS：run id
`c2b5ce9a7f5ccefe67f231a43ab3f157`、runner exit 0；exact-SHA clean-context
独立完成审查亦 PASS，零 BLOCK/HIGH/MEDIUM/LOW，并核验全部十五个 result、aggregate
decision 与引用哈希。结论不得外推到 artifact、installed、live provider、Science、SSH、
签名、公证或公开 release。

`R1-C` 已收口（source）：`one_click.rs` 用
`CompensationOutcome` 逐项表达 Science cleanup、SSH cleanup、authority restore、prior
Science restart 与 snapshot cleanup，并以 typed environment exposure 汇总 candidate /
cross-runtime 风险。one-click compensation 的 recovery/environment 只从这些 typed outcome
推导；兼容诊断 code 只由 `CompensationCause` 与 `CompensationEnvironment` 渲染，不再读取
拼接后的 message。既有 DTO keys、coarse stage、diagnostic code 与 authority snapshot / SSH /
Gateway / prior Science 操作顺序保持不变；R1-E 范围内其他过渡性 message 解析未提前清理。

focused source/unit 已实际执行并 PASS：transaction tests 沙箱外 24 passed / 0 failed /
2 explicitly ignored，三条隔离 compensation cleanup / restart / diagnostic redaction 用例均
1 passed / 0 failed；完整 Rust lib 沙箱外为 482 passed / 0 failed / 41 explicitly ignored。
受限沙箱中的 transaction tests 为 20 passed / 4 `Operation not permitted` / 2 ignored，失败
集中于动态 loopback，不计为 R1-C 产品失败。质量 metadata、impact-pr 与文档治理均 PASS；
首轮 clean-context 审查发现 prior Science disposition 联动遗漏 HIGH，修复并加入组合回归后，
由新的 clean-context reviewer 复审 PASS，零 BLOCK/HIGH/MEDIUM/LOW。source implementation
exact SHA `7d3427f34398a542a70af0c1c4a7649b7f6ed630` 取得十五 suite
`GATE-SOURCE` completion seal PASS：run id `391f1c07b49911c7f371bc78b86e7c01`、runner
exit 0；manifest 含十五个 test result 与十五个 source observation，desktop 身份为 523
discovered / 523 executed / 41 approved ignored / 0 skipped。exact-SHA clean-context 独立
完成审查亦 PASS，零 BLOCK/HIGH/MEDIUM/LOW，并复核全部三十个 result/observation 引用哈希、
seal 引用、486 个 tracked snapshot entry 与 ignored identity。结论不得外推到 artifact、
installed、live provider、Science、SSH、签名、公证或公开 release。

`R1-D` 已收口（source）：`recover_interrupted_gateway` 返回 typed
`InterruptedGatewayRecoveryOutcome` / `InterruptedGatewayRecoveryError`。成功路径区分
`NotNeeded` 与 `Stopped(pid)`；停止边界区分 `NotManaged` 与 `StopUnknown`，后者继续以
`SignalFailed` / `ExitUnconfirmed` 表达“信号发送失败”和“信号已发送但退出未确认”。command
层只按 error kind 投影 `AuthoritySnapshot` 或 `GatewayStart`，不再扫描中文 message。既有
frontend DTO keys/text/coarse stage、journal `recover_interrupted_gateway` stage 写入时机、最终
listener identity recheck、以及拒绝时 listener/journal 保留行为均保持不变。

focused source/unit 已实际执行并 PASS：`proxy_lifecycle` 20 passed / 0 failed / 1 explicitly
ignored，command structured-stage 1 passed / 0 failed；完整 Rust lib 沙箱外为 482 passed /
0 failed / 41 explicitly ignored。受限沙箱中恢复测试因动态 loopback 返回 `Operation not
permitted`，随后按隔离 fixture 边界在沙箱外通过，不计为产品失败。clean-context candidate
首轮审查发现三个 `NotNeeded` 早退缺少 typed 回归 LOW；已在原测试 identity 内补齐无
journal、同进程已受管、端口无 listener 及 config/journal 不改写断言，随后由新的
clean-context reviewer 复审 PASS，零 BLOCK/HIGH/MEDIUM/LOW。implementation commit
`dc8bf969f76771eb3d8c7cd9188e9d27390b2d7e` 的首次 exact-SHA gate 十四 suite PASS，唯一
`SUITE-ORPHAN-SKILL-BOUNDARY` 因仍绑定旧 `.map_err(` 源码形状而 FAIL；该 source-contract
已改为绑定 production recovery call 与 typed helper、检查 exact kind mapping 并禁止 helper
内 message `contains`，修复后 boundary module 11 passed / 0 failed，新的 clean-context
reviewer 复审 PASS，零 BLOCK/HIGH/MEDIUM/LOW。最终 source candidate exact SHA
`b93002ae95fc88a89ac5e30addd9cea693113312` 取得十五 suite `GATE-SOURCE` completion seal
PASS：run id `0ad26c3207407a43056bd981b378bb38`、runner exit 0；manifest 含十五个 test
result 与十五个 source observation，desktop 身份为 523 discovered / 523 executed / 482
passed / 0 failed / 41 approved ignored / 0 skipped / 0 not run，source snapshot 为 487 个
tracked entry。exact-SHA clean-context 独立完成审查亦 PASS，零 BLOCK/HIGH/MEDIUM/LOW，并
复核三层 manifest、全部三十个 result/observation 引用哈希、snapshot 条目与 ignored identity。
结论不得外推到 artifact、installed、live provider、Science、SSH、签名、公证或公开 release。

`R1-E` 已收口（source）：production 已删除
`recovery_from_diagnostic_codes`，`RuntimeError`、`AuthorityCleanupFailure` 与
`InterruptedGatewayRecoveryError` 不再通过 `Deref<str>` 暴露兼容性 `contains`。普通
one-click error 构造与 command DTO 投影不再读取 message；中断 Science 入口、authority
cleanup、one-click compensation 与 interrupted Gateway recovery 均从 typed outcome / recovery
disposition 显式投影。既有人读 message、frontend DTO keys、coarse stage、one-click、healthy
reopen、profile reconcile 与 auto-boot 的投影边界保持不变。

静态 source-contract 现绑定普通 one-click 构造、interrupted Science / Gateway typed recovery、
command projection、healthy reopen、profile reconcile 与 auto-boot，并禁止三个 typed error
恢复隐式字符串 `Deref`。focused failure 8 passed / 0 failed，command structured-stage、runtime
journal 与 transaction contract 各 1 passed / 0 failed，profile-switch unit 3 passed / 0 failed；
boundary module 11 passed / 0 failed，quality metadata PASS；完整 Rust lib 在允许隔离
loopback/process 的环境为 482 passed / 0 failed / 41 explicitly ignored。受限沙箱内完整 lib
因动态 loopback/process 权限出现 `Operation not permitted`，外部隔离重跑全绿，不计为产品
失败。首轮 clean-context reviewer 发现 interrupted Gateway `AuthoritySnapshot` 在删除解析器后
会从 `manual_recovery_required` 回退为 `degraded` HIGH，并指出 source-contract 未绑定该
recovery mapping MEDIUM；已为 Gateway error 增加 typed recovery disposition、补齐 DTO
recovery/environment 断言与双层 source-contract。修复后的新 clean-context candidate review
已 PASS，零 BLOCK/HIGH/MEDIUM/LOW。implementation exact SHA
`ff528eb6b62085e83b43184bfb878d8ff37e4de2` 取得十五 suite `GATE-SOURCE` completion
seal PASS：run id `0be1d0796597853ca387a00a7ab6c683`、runner exit 0；manifest 含十五个
test result 与十五个 source observation，desktop 身份为 523 discovered / 523 executed /
482 passed / 0 failed / 41 approved ignored / 0 skipped / 0 not run，source snapshot 为 488 个
tracked entry。exact-SHA clean-context 独立完成审查亦 PASS，零 BLOCK/HIGH/MEDIUM/LOW，并
复核三层 manifest、全部三十个 result/observation 引用哈希、snapshot 条目与 ignored identity。
结论不得外推到 artifact、installed、live provider、Science、SSH、签名、公证或公开 release。

`R1-F` 已收口（source）：已对 R1-A–E 的 typed envelope、authority
cleanup、one-click compensation、interrupted Gateway recovery、command / auto-boot /
profile projection 与 production source-contract 做整体 inventory。聚焦矩阵实际执行并
PASS：`failure::` 8 passed、command structured-stage 1 passed、auto-boot 1 passed、
transaction 24 passed / 2 explicitly ignored、proxy lifecycle 20 passed / 1 explicitly
ignored、profile-switch 3 passed、boundary module 11 passed；完整 Rust lib 在允许隔离
loopback / process identity 的环境为 482 passed / 0 failed / 41 explicitly ignored。

敏感信息 canary 已单独执行并 PASS：compensation rollback 注入的原始 credential 未进入
诊断表面；authority snapshot path / credential canary 与 runtime journal secret-free 回归亦在
transaction matrix 中 PASS。两条 prior-Science compensation 隔离用例首次与其他测试进程
并行执行时相互干扰失败，随后按各自 isolation contract 串行重跑均 PASS；该并行方式不作为
有效证据。candidate exact SHA `0d2a4604bbcc4297555b9e45c99929b4abb7788c` 取得完整
十五 suite `GATE-SOURCE` completion seal PASS：run id
`62f2b68b35f9243756afb30c5a694fab`、runner exit 0；manifest 含十五个 test result 与
十五个 source observation，三十三个引用哈希零不匹配，source snapshot 为 489 个 tracked
entry。desktop 身份为 523 discovered / 523 executed / 482 passed / 0 failed / 41 approved
ignored / 0 skipped / 0 not run，且 discovered / ignored / skipped identities 与冻结 fixture
精确一致。exact-SHA clean-context 独立完成审查 PASS，零 BLOCK/HIGH/MEDIUM/LOW，并确认
typed recovery、DTO、journal ordering、listener retention、compensation/retry 与脱敏边界未
回归。R1 全部收口仅建立 source/unit 结论，不外推到 artifact、installed/runtime、live
provider、Science、SSH、签名、公证或公开 release。

| 阶段 | 状态 | 边界 |
|---|---|---|
| `R1-A` | DONE | typed envelope 底座、冻结 DTO 投影、exact-SHA source gate 与独立审查收口 |
| `R1-B` | DONE | authority snapshot / pending cleanup typed outcome、exact-SHA source gate 与独立审查收口 |
| `R1-C` | DONE | one-click compensation typed aggregate、exact-SHA source gate 与独立审查收口 |
| `R1-D` | DONE | interrupted Gateway recovery typed outcome、exact-SHA source gate 与独立审查收口 |
| `R1-E` | DONE | production message semantic parsing 清零、typed projection、exact-SHA source gate 与独立审查收口 |
| `R1-F` | DONE | 整体 inventory、矩阵、canary、exact-SHA source gate 与最终独立审查收口 |

R1 已全部收口。R2 已进入首个窄阶段；R3 state owner 与 R4 mutation lease 仍未触碰。

## R2 versioned typed runtime journal 进度

`R2-A` 已收口（source）：根 `Config.schema_version` 保持 `4`，
嵌套 `runtime_transaction` reader 现在区分无嵌套版本的 V1 与 `schema_version=2` 的
typed V2。V1 继续按原 wire shape 写入且不会自动升级；reader 对 future version、unknown
field、unknown V1 stage、无法证明 fingerprint 的 legacy Science environment stage，以及
非法 V2 operation / phase / exposure / Gateway outcome 组合 fail-closed。V2 schema 只保存
typed operation / phase、runtime fingerprint、environment exposure、snapshot ticket、previous
binding / Gateway public identity、compensation state 与 Gateway stop outcome；R2-A 的 V1
production writer 遇到 V2 会保留事务并拒绝改写。现有 checkpoint 时机、runtime effect、
frontend DTO、command/event 与恢复策略均未迁移。

首个 candidate exact SHA `e8ed08050f4a7d7f5a94d3c27c483defca00f699` 的十五 suite
`GATE-SOURCE` 虽取得 PASS，但随后 clean-context completion review 发现 `one_click.rs` 仍在
V1 adapter 外按 prefix 解释 legacy journal stage，评为 HIGH 并给出 FAIL；该 run 不作为
R2-A closure 证据。当前 repair 已把 prefix 识别集中到 `config.rs` 的 typed V1 environment
accessor，orchestration 只消费 typed classification；V1 wire writer 未改变。repair focused
config identities 4/4 与 one-click runtime-journal identity 1/1 PASS。首个 repaired candidate
exact SHA `7a2e87539eac2fb90758f15926bab18123e7dd68` 的完整 gate 有十四个 suite PASS，唯一
`SUITE-ORPHAN-SKILL-BOUNDARY` 因 source assertion 仍依赖已删除的 legacy `start_science`
parser 常量而 FAIL；该 run 亦不作为 closure 证据。当前 source-contract repair 已改为冻结
实际八个 V1 one-click checkpoint identity，boundary suite 11/11 PASS；inventory 5/5、quality
metadata、document governance、format 与 diff check PASS。

最终 candidate exact SHA `deff0b73e4c0f4876220e162ad123bece356424d` 取得十五 suite
`GATE-SOURCE` completion seal PASS：run id `d6094581cd5d3fd9972a287f5ec0a456`、runner exit 0；
manifest 含十五个 PASS test result 与十五个 PASS source observation，递归 schema/semantic/hash
回读 PASS。desktop 身份为 527 discovered / 486 passed / 0 failed / 41 approved ignored /
0 skipped / 0 todo / 0 not run，source snapshot 为 490 个 tracked entry。exact-SHA clean-context
completion review PASS，零 BLOCK/HIGH/MEDIUM/LOW。本文所在 evidence-only seal commit 只记录
上述已验证 candidate 与 run，不声称 seal commit 本身执行过完整 gate。R2-A 仅建立 source/unit
结论，不外推到 artifact、installed/runtime、live provider、Science、SSH、签名、公证或 release。
`R2-B` implementation candidate 已完成：现有八个 one-click checkpoint 保持原时机并改写
typed V2 phase；首写冻结同一个 candidate fingerprint、verified snapshot `managed_id` ticket
与 transaction id，后续 checkpoint、clear 和 binding commit 均按 exact identity fail-closed。
首写失败且 protected mutation 尚未开始时，只有同进程 `PreJournalAbort` 可持内存中的
registered ticket 进入既有补偿；重启后的 no-journal `ActiveRecovery` 仍拒绝自动恢复并要求
人工处置。destructive stop 前仍未增加 durable intent，F5 gap 明确保留。focused source/unit、
隔离 PreJournalAbort 与既有 crash/no-journal characterization 已通过，修复后 clean-context
预提交审查 PASS，零 BLOCK/HIGH/MEDIUM/LOW。首个 candidate exact SHA
`f969d56dbd293e235afc1ed97f4e4e1d63d1b074` 的完整 gate 有十四个 suite PASS；唯一
`SUITE-RUST-DESKTOP` 因新增 characterization 把 snapshot 尚未注册 ticket 的旧 capture-failure
模式也错误要求 cleanup manifest empty 而 FAIL，该 run `9ab7dd50d8794411d7c1fae43d0a1527`
不作为 closure 证据。repair 只把该断言限定到真正的 `PreJournalAbort` 模式；旧 snapshot-failure
与新 PreJournalAbort 两个隔离 identity 均 PASS，且 clean-context repair review PASS。

最终 candidate exact SHA `545ad28f0253f7ea9d2bde076ad7b136318c6b0e` 取得十五 suite
`GATE-SOURCE` completion seal PASS：run id `37f69cfd7ed2150425033872c3f7085b`、runner exit 0；
manifest 含十五个 PASS test result 与十五个 PASS source observation，递归 schema/semantic/hash
回读 PASS。desktop 身份为 528 discovered / 487 passed / 0 failed / 41 approved ignored /
0 skipped / 0 todo / 0 not run，source snapshot 为 491 个 tracked entry。exact-SHA clean-context
completion review PASS，零 BLOCK/HIGH/MEDIUM/LOW。本文所在 evidence-only seal commit 只记录
上述已验证 candidate 与 run，不声称 seal commit 本身执行过完整 gate。R2-B 仅建立 source/unit
结论，不外推 artifact、installed/runtime、live provider、Science、SSH、签名、公证或 release。
`R2-C` implementation candidate 已完成：compiled + test-only 的 profile-switch writer
保持 candidate config 与 journal 原子 publish、正式 Gateway 启动、rollback 与 finalization
时机不变，把 `start_formal_gateway` V1 string stage 改写为
`operation=profile_switch / phase=start_formal_gateway` 的 typed V2。该记录不携带 runtime
fingerprint 或 snapshot ticket，environment 保持 `not_exposed`，compensation 为
`not_started`，Gateway stop outcome 为 `not_attempted`；publish、rollback 与 clear 均按
caller 保存的完整 typed record fail-closed，包括 transaction/target、previous binding/Gateway、
operation/phase、exposure、compensation 与 Gateway outcome。当前产品可达的
`set_active_profile` 仍只是 selection intent，本阶段没有启用 test-only transaction，也没有
迁移 interrupted-Gateway recovery writer。同进程 Science reconcile 只在 caller 原始
profile-switch typed record 与当前 journal 完整相等时允许首个 one-click
V2 checkpoint 接棒或 healthy-reopen binding commit；当前 journal 消失、回退 V1 或 retarget
均拒绝覆盖，普通 one-click 与重启仍 fail-closed。focused source/unit、quality metadata、inventory、
document governance、format 与 diff check 已 PASS；隔离 profile-switch snapshot failure
rollback 在允许动态 loopback 与测试自有进程的环境 PASS。首个 candidate exact SHA
`ba5e5b02addd90807879281ce01235aed143b170` 的完整 gate 有十四个 suite PASS；唯一
`SUITE-RUST-DESKTOP` 失败来自 profile reconcile source contract 使用了实际不存在的旧
finalization 截断符，导致抽取范围泄漏到后续 secret-free 测试并误判合法的
`.contains(forbidden)`；该 run `e6cec66c85440a7f4e855e170e5bc277` 不作为 closure 证据。
repair 只把静态合同截断点绑定到真实 `update_result` finalization boundary，没有修改生产语义；
exact focused test、quality metadata 与 clean-context repair review PASS。

最终 candidate exact SHA `14aa95802127fedaded7845aa7e84e25a5aa2ed3` 取得十五 suite
`GATE-SOURCE` completion seal PASS：run id `24906181899e0c07f9bb62260fe60b63`、runner exit 0；
manifest 含十五个 PASS test result 与十五个 PASS source observation，递归 schema/semantic/hash
回读 PASS。desktop 身份为 528 discovered / 487 passed / 0 failed / 41 approved ignored /
0 skipped / 0 todo / 0 not run，source snapshot 为 492 个 tracked entry。exact-SHA clean-context
completion review PASS，零 BLOCK/HIGH/MEDIUM/LOW。本文所在 evidence-only seal commit 只记录
上述已验证 candidate 与 run，不声称 seal commit 本身执行过完整 gate。R2-C 仅建立 source/unit
结论，不外推 artifact、installed/runtime、live provider、Science、SSH、签名、公证或 release。
`R2-D` 已收口（source）：interrupted profile-switch Gateway
recovery 只接受可证明的 `start_formal_gateway|recover_interrupted_gateway` V1 兼容记录或
对应的 V2 profile-switch 记录，且 V2 compensation 必须仍为 `not_started`；V1 继续恢复时以完整原记录 CAS 原子升级为
`operation=profile_switch / phase=recover_interrupted_gateway / gateway_stop_outcome=pending`，
不再写回 string stage。既有 path-secret/health/contract/binary/uid/PID 与最终 listener
identity recheck 之后，stop 结果以第二次完整记录 CAS 持久化为
`stopped|not_managed|signal_failed|exit_unconfirmed`；前次 recovery intent 后 listener 已消失
则记为 `absent_after_attempt`；`stopped|absent_after_attempt` 为终态，不会再次探测或停止
后来出现的 listener。任一 transaction/target、previous binding/Gateway、
operation/phase、exposure、compensation 或 outcome 漂移都保留当前记录并 fail-closed。

当前 focused source/unit 已 PASS：post-stage typed outcome / V1 upgrade / V2 continuation /
complete-record 双 CAS identity、真实测试进程 signal/wait/retry matrix、target/unsupported
journal/absent-listener identity 与 transaction source contract 均通过；动态 loopback 与测试
自有进程仅在隔离沙箱外执行，明确避开 8765。`cargo check --offline --lib` PASS。metadata、
inventory、document governance、quality focused 14/14、format 与 diff check 均 PASS；当前完整
Desktop Rust lib 为 528 discovered / 487 passed / 0 failed / 41 approved ignored / 0 skipped /
0 todo / 0 not run。首个 exact candidate `3b434dc85dbcd93f4c7c189455db676a20457f9e`
虽取得十五 suite gate PASS，但 completion review 发现 outcome CAS 保留的
`recover_interrupted_gateway/pending + compensation=in_progress` 漂移记录可在重启后重新进入
recovery 并驱动 listener stop；该 run `e33982446c192e09d2ae364aaed5eeb4` 不作为 closure 证据。
repair 把两个允许 V2 phase 的 eligibility 都冻结为 canonical `compensation=not_started`，并新增
save/load 重启式行为回归，证明漂移记录保留原 bytes/journal 且不探测 listener；clean-context
repair review PASS，零 BLOCK/HIGH/MEDIUM/LOW，原 HIGH CLOSED。

最终 candidate exact SHA `e185de9c22d61662acbe28af655466a27720fd36` 取得十五 suite
`GATE-SOURCE` completion seal PASS：run id `d01855a2128636df9cfec061bda56c9d`、runner exit 0；
manifest 含十五个 PASS test result 与十五个 PASS source observation，递归 semantic/hash
回读 PASS。desktop 身份为 528 discovered / 487 passed / 0 failed / 41 approved ignored /
0 skipped / 0 todo / 0 not run，source snapshot 为 493 个 tracked entry。最终 exact-SHA
clean-context completion review PASS，零 BLOCK/HIGH/MEDIUM/LOW，历史 findings CLOSED 3 / OPEN 0。
本文所在 evidence-only seal commit 只记录上述已验证 candidate 与 run，不声称 seal commit 本身
执行过完整 gate。R2-D 仅建立 source/unit 结论，不外推 artifact、installed/runtime、live
provider、Science、SSH、签名、公证或 release。唯一 `NEXT` 前移到 R2-E；当前不进入 R2-F，
更不把 2026-07-31 rebaseline 中的 R3-R11 顺序视为已自动授权的后续实施计划。

`R2-E` implementation candidate 已完成：实时 inventory 已覆盖 V1/V2 的 deserialize、
blocker、one-click、test-only profile-switch、interrupted-Gateway recovery、healthy reopen、
pending cleanup、rollback、checkpoint、binding commit 与 clear 入口。one-click 进程内
progress 现在保存上一条完整 V2 record；后续 checkpoint、成功 clear 与 binding commit
只有在磁盘 record 完整相等时才推进，transaction/candidate/ticket 相同但 prior binding、
compensation、phase/exposure 或其他字段漂移也保留当前 journal 并 fail-closed。既有测试
identity 已扩展为十种合法 V1 wire stage 的 unchanged-wire save/load，以及当前全部生产 V2
phase/outcome 的 save/load/save 重启矩阵。首轮 clean-context 审查发现 helper 拒绝漂移后，
统一补偿仍可能用捕获前 config 覆盖当前 journal HIGH；修复后 `Journaled` 补偿要求完整
record CAS，成功 clear/binding commit 后的 `Finalized` 补偿要求 journal 仍为空，retarget 时
在任何 authority/runtime restore 前立即拒绝，并保留当前 config、authority、AppState 与
recovery snapshot。新增回归先触发真实 checkpoint CAS 拒绝，再穿过生产 compensation 漏斗
并证明这些状态均保留。第二次 clean-context 审查进一步发现 guard 位于 authority/runtime
restore 之后且 inventory 漏列直接 reader/healthy-reopen rollback；当前修复已把 guard 前移到
Gateway/authority/config/AppState restore 之前，并补齐对应 inventory anchor。candidate cleanup
与 SSH cleanup 仍保持既有次序。checkpoint 时机、F5、`StopFailed` 后行为
和 journal 覆盖范围均未改变。

首次 candidate exact SHA `638fdd55987f176c89f9a12c6f39de6923b36aed` 的完整 gate
run `d15f213da1cc5c067c09c0e9d84526e3` 在 `SUITE-PY-OFFLINE` 暴露一条仍绑定旧
`&journal_progress` 调用的 profile-pin source assertion；该 run 为 FAIL，不作为 closure 证据。
修复后的测试以 whitespace-tolerant regex 绑定 `&mut journal_progress` 参数顺序，并取得独立
clean-context PASS。

最终 candidate exact SHA `8b1471550cf69178770a15f9dcc8378e7f334037` 取得十五 suite
`GATE-SOURCE` completion seal PASS：run id `d29764f44c9a39f28bf2aaf03a035177`、runner
exit 0；manifest 含十五个 PASS test result 与十五个 PASS source observation。desktop 身份为
528 discovered / 528 executed / 487 passed / 0 failed / 41 approved ignored / 0 skipped /
0 todo / 0 not run，clean source snapshot 为 494 个 tracked entry。最终 exact-SHA clean-context
completion review 递归回读 seal、manifest、results、observations 与 snapshot hash 后 PASS，零
BLOCK/HIGH/MEDIUM/LOW。本文所在 evidence-only seal commit 只记录上述已验证 candidate 与 run，
不声称 seal commit 本身执行过完整 gate。R2-E 仅建立 source/unit 结论，不外推 artifact、
installed/runtime、live provider、Science、SSH、签名、公证或 release。唯一 `NEXT` 前移到
R2-F；当前不自动进入 R2-F，更不进入 R3。

`R2-F` 已收口（source）：已对 R2-A–E 的 nested V1/V2 schema、合法
operation / phase / exposure / compensation / Gateway outcome 组合、candidate fingerprint /
snapshot ticket / previous binding identity、完整记录 CAS、one-click 与 test-only
profile-switch writer、interrupted-Gateway recovery、V1 fail-closed compatibility 和重启读取
矩阵做整体对齐。盘点没有发现需要修改生产源码的新不一致，也没有引入新的 product 或
crash-recovery 语义。

聚焦矩阵实际执行并 PASS：config V1/V2 reader / round-trip 4 passed、one-click V2
checkpoint 与完整记录漂移回归 1 passed、transaction source contract 1 passed、test-only
profile-switch 3 passed、proxy lifecycle module 20 passed / 1 explicitly ignored，其中
interrupted Gateway recovery identities 为 4 passed / 1 explicitly ignored；
`cargo check --offline --lib` PASS。受限沙箱中的 recovery identities 有 3 项因动态 loopback /
process identity 权限失败；按隔离合同在沙箱外重跑后 recovery 为 4 passed / 0 failed /
1 ignored，完整 proxy lifecycle module 为 20 passed / 0 failed / 1 ignored。
完整 Rust lib 的首次并行执行有一条不属于 R2 的 Codex cancel 测试失败；该并行结果不作为
证据，失败 identity 单独串行重跑 PASS，随后完整 lib 单线程重跑为 487 passed / 0 failed /
41 explicitly ignored。candidate exact SHA `be961cb27701fd2adfde699c342cdd4dcf8a3d8d`
取得完整十五 suite `GATE-SOURCE` completion seal PASS：run id
`81a0f70476b075da3763390f88b4385c`、runner exit 0；manifest 含十五个 PASS test result
与十五个 PASS source observation，全部三十个引用哈希与 seal 三个顶层引用重算一致。
Desktop 身份为 528 discovered / 528 executed / 487 passed / 0 failed / 41 approved ignored /
0 skipped / 0 todo / 0 not run，clean-commit source snapshot 为 495 个 tracked entry。
exact-SHA clean-context completion review PASS，零 BLOCK/HIGH/MEDIUM/LOW，并复核全部
snapshot path / mode / size / content hash 与 evidence 脱敏边界。本文所在 evidence-only seal
commit 只记录上述已验证 candidate 与 run，不声称 seal commit 本身执行过完整 gate。
R2 全部收口仅建立 source/unit 结论，不外推 artifact、installed/runtime、live provider、
Science、SSH、签名、公证、Gatekeeper 或 release。下一步强制停止实施，只进入只读的
Post-R2 Rebaseline；R3-R11 尚未获得新的实施授权。

| 阶段 | 状态 | 边界 |
|---|---|---|
| `R2-A` | DONE | nested V1/V2 schema、V1 只读兼容、fail-closed typed accessor、exact-SHA gate 与独立审查；仍写 V1 |
| `R2-B` | DONE | 八个 one-click checkpoint V2、同一 candidate/ticket identity、PreJournalAbort、exact-SHA gate 与独立审查收口；F5 保留 |
| `R2-C` | DONE | test-only profile-switch writer typed V2、完整记录 CAS、exact-SHA gate 与独立审查收口；产品选择仍 intent-only |
| `R2-D` | DONE | interrupted-Gateway recovery typed V2 intent/outcome、canonical compensation、完整记录 CAS、跨重启 drift fail-closed、exact-SHA gate 与独立审查收口 |
| `R2-E` | DONE | 现有 typed journal 的生产 writer / reader / rollback / clear 与 V1 fail-closed compatibility 矩阵收口；完整记录 CAS、补偿前置 guard、exact-SHA gate 与独立审查完成；checkpoint/F5/StopFailed/operation scope 保持 |
| `R2-F` | DONE | R2-A-E schema、identity、CAS、recovery、compatibility 总盘点、聚焦矩阵、exact-SHA gate 与最终独立审查收口 |
| `Post-R2 Rebaseline` | NEXT-READ-ONLY | R2 完成后先停止实施，按实时源码重新规划 R3+；在新路线获明确授权前，R3-R11 均不是实施 `NEXT` |

### R2 退出与后续路线重规划门

R2-E 先实时盘点所有生产 writer、reader、upgrade 与 recovery 入口，证明合法 V2
组合、V1 fail-closed compatibility 和重启读取矩阵已经闭合；如果盘点暴露不一致，只允许
在 typed journal 现有语义内修正。它不得借机新增 pre-stop durable intent、改变
`StopFailed` 后的 stage / wait / restart 行为，也不得把 mode、settings、Codex、downgrade
或 profile revocation 纳入 `runtime_transaction`。

R2-F 是 R2 的 source 层总收口：对齐 R2-A-E 的 inventory、schema / identity / CAS
约束、focused regressions、完整 source gate 和独立审查，明确记录仍保留的 F5 与非
source 证据缺口。R2-F 不是新功能片，也不外推 artifact、installed/runtime、live、签名、
公证或 release 结论。

R2-F 完成后设置强制停止点，执行一次只读的 **Post-R2 Rebaseline**：

1. 以当时 exact HEAD 的源码、测试和当前文档为基线，重新盘点 R3-R11 所涉及的 state
   owner、caller、长等待、mutation 边界、失败链和现有 typed receipt；
2. 复核依赖图、剩余风险与可独立验收的最小切片，判断 R3 / R4 是否仍应优先，允许调整、
   合并、拆分、延后或取消旧阶段；
3. 产出新的有限路线：每阶段写明要消除的不确定性、范围、前置、退出条件与明确非目标；
4. 经用户明确授权新的唯一 `NEXT` 后才进入实施，不能由 R2 完成自动跳转到 R3。

2026-07-31 rebaseline 对 R3-R11 的描述在该停止点仅作为历史架构输入。无论后续路线怎样
调整，R5 中 `AuthorityTransaction` 的接口等价提取与改变 crash 行为的
`PriorStopIntent/Outcome` 仍须分开；后者继续要求证据、operation contract 和独立授权。

## 下一轮重构的 P0 前置

- ~~Ambient environment 泄漏~~ **已闭合（source）**：`runtime/launch_env.rs`
  对 launch/stop script 与 Gateway 执行 `env_clear` + allowlist；
  `scripts/launch-virtual-sandbox.sh` 以 `env -i` 启动 Science。sentinel 与
  stub 回归：`cargo test --lib launch_env` / `proxy_lifecycle`、
  `bash test/test_launch_science_env_allowlist.sh`。
- ~~Typed failure projection~~ **已闭合（source，stage）**：`runtime/failure.rs`
  的 `OneClickFailureKind` 在产生点标注；一键与 auto-boot 投影到冻结 coarse
  stage；生产路径不再用 `science_failure_stage` 扫文案。journal checkpoint 仍是与 UI
  stage 分离的持久阶段域，但 one-click、compiled test-only profile-switch 与
  interrupted-Gateway recovery writer 已写 typed V2，V1 只保留 fail-closed 兼容读取与原
  wire 序列化；domain/kind/phase/recovery/environment 已不再从 message、recovery token
  或中文错误文本反向分类，人读 message 仅作展示。验证：
  `cargo test --lib failure::`、
  `science_operation_failures_have_stable_structured_stages`、
  `auto_boot_rejects_structured_runtime_failure`。
- 本轮 runtime、Gateway 与 frontend 机械职责拆分已经在本地 `next` 收口，结构、
  验证层与后续逻辑重构边界见
  [工程重构后基线](../../docs/audits/2026-07-31-v084-post-refactor-baseline.md)。
  任何后续拆分都不得扩大已冻结的 Runtime/Gateway allowlist，子模块须返回
  typed failure。
- 已闭合的是 **process environment 边界** 与 **一键/auto-boot 故障投影**，不是
  全部真实 provider/SSH/Science 领域 live PASS，也不是 Developer ID / notarization。

## 第三方模型与 Science 原生能力

- CSSwitch 必须管理 Runtime 包络、Model Gateway、必要 network policy 和诊断恢复；
  Project/session/artifact/permission/memory/kernel/Agent/Plugin 等语义仍由 Science
  原生拥有。当前 ownership 与 stage 链见
  [能力地图](../../docs/features/product-science-capability-map.md)。
- 第三方模型支持不能由“文本聊天成功”代替。stream、tools/`tool_choice`、
  reasoning、structured output、vision、stop/error semantics 需要按 provider 与
  operation 分层验证；不支持时必须可定位降级，不能静默改写语义。
- 最终 v0.8.4 artifact 没有建立所有真实 OpenCode Go、Grok、Gemini、Kimi、
  DeepSeek、custom relay 或 Codex 账号/模型的 live PASS。
- Web Search、hosted MCP/Connectors、Reviewer entitlement、官方 catalog/usage
  依赖 Anthropic 账号与服务；第三方 Gateway 不模拟这些官方 entitlement。
- 动态 model catalog 在一次修复线观测中仍约耗时 12.756 秒。90 秒级 snapshot
  回归已修，但首次可用延迟仍是独立 UX 问题。

## Runtime、网络与窄桥

- `HTTPS_PROXY` / `NO_PROXY` 与 Gateway raw `CONNECT` 属于 socket transport。
  connector、文献、云和 updater 等能力即使借道 CONNECT，产品语义仍由
  Science/账号/外部服务拥有；不能把连接成功写成能力 PASS。
- 第三方 Science 使用 `--no-auto-update`。官方更新应先在官方 Science 路径完成，
  CSSwitch 再停止并重新启动受管链，采用通过 fixed-path/identity 检查的候选。
- 2026-07-30 的 `B-RUNTIME-01` 因没有取得允许的 Science 0.1.25 executable
  identity 而保持 `INCONCLUSIVE`；start/open/reopen/status/stop/restart 均
  `NOT-RUN`。这不是产品失败，也不能由历史 release evidence 替代。
- 外部 Skill install/attach、Science load/trigger、领域执行和重启持久化是不同
  结论。CSSwitch 只拥有窄安装/投影桥，不拥有 Skill runtime 或通用 MCP 管理面。
- 系统 SSH 默认关闭；opt-in 后 CSSwitch 只负责 preflight/stub/sidecar 边界。
  parser、OpenSSH invocation 与真实 server connectivity 必须分开；当前没有特定
  真实 SSH server 的 current live PASS。
- Codex 仍是默认关闭的实验窄桥。上游账号权限、动态目录与 Responses 协议会变；
  不支持设备码、多账号、代理认证、PAC、自定义 CA、系统代理自动发现或 TUN 检测。

## 分发与证据

- v0.8.4 公开附件为经过完整性验证的 ad-hoc seal；没有 Developer ID、
  notarization、stapled ticket 或 Gatekeeper acceptance。
- trusted `GATE-SOURCE` PASS 只证明 exact source/unit；文档治理定向测试也不能
  外推 artifact、installed/live、provider、signing 或 public release。
- 真机矩阵只是应执行场景，不表示最终 DMG 已逐项全部执行。每次验收必须绑定
  exact artifact/environment，并把 PASS、失败、阻断、未执行分开。
- v0.8.4 已建立的 source、artifact、installed identity、signing 与 public 层见
  [release evidence](../../docs/evidence/releases/v0.8.4.md)；未列层不得补写为 PASS。
