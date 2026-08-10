# 当前已知问题与证据缺口

状态：当前；唯一验收路线以[生产链路验收](../../docs/operations/real-machine-acceptance.md)为准

最后复核：2026-08-10（Asia/Taipei）

失效条件：production owner / caller、确定性 fixture、候选 source、artifact identity、Science / Gateway runtime、provider capability、installed/runtime、签名或公开 Release 任一相关事实改变时，受影响条目立即失效并须实时重审。

本页只登记当前仍开放的问题和各证据层缺口，不保存另一份路线或验收合同。使用前必须实时核对 branch、HEAD、worktree、目标 artifact 与 runtime；日期化 audit/evidence 只证明其绑定的 SHA、artifact、版本和环境。

## 唯一当前路线

所有旧 R3–R11、R4/R5、S7、Post-D0/Post-Q0 及其他阶段编号路线均已退役，只能从[历史审计索引](../../docs/audits/README.md)查证当时的 source closure、取舍和证据边界。旧编号、旧 sole NEXT、旧计划顺序和已完成 source gate 都不能授权或替代新的实现、artifact 或 live 阶段。

新的唯一验收顺序是：**重要重构决策 → production source → exact artifact → isolated-live → authorized live**。当前映射、每层进入条件、授权边界和故障 fixture 边界只在[生产链路验收](../../docs/operations/real-machine-acceptance.md)维护；Science 运行细则见[Science 探针合同](../../docs/operations/science-probe-spec.md)。

2026-08-10 最新已验收的 exact source/artifact binding 为
`next@06b630bb3e3fb0d63425bf48d1ffc2d3613fd992`。独立 clean、non-shallow exact clone 的固定
15-suite source gate 为 `PASS`（15/15 suites、15/15 observations、runner exit 0）；随后由该 SHA
以 `acceptance-build` 生成的 `CSSwitch Test.app`、packaged Rust Gateway 与 Claude Science 0.1.25
exact tuple 经递归 G1 validator 取得 `PASS`。同一 tuple 随后在 deny-egress 隔离环境完成
`B-RUNTIME-01=PASS`：production Desktop → packaged Gateway → Science、一键开始、5 次 loopback
provider request、单实例重开复用、产品停止/重启、再次请求、最终停止与精确清理均闭合；随后
`B-CORE-01=PASS`，限定证明合成 project / 文件读写、permission request / grant / revoke、revoke 后及
越界拒绝、artifact lineage 与 annotation 持久状态；随后同一 tuple 的
`B-CONTEXT-01=PASS(scope=isolated-request-shape)`，限定闭合 plan、delegation、fork/restore、
Memory/compaction、Reviewer/Specialist local surface 与跨 project/session 隔离。后续文档提交只记录
证据，不能改写被构建或运行的 source/artifact identity。

上一份已验收的 stop-ownership production source candidate 是
`next@65b65c13dc5db59dc3798d0dc1320e7712c726e2`。它以 owner-map baseline
`c148e428874a0ed459a25a33145e930b4e6b9b18` 为父系，完成 macOS native exit 的
process-local Science owner claim、锁外 stop/wait、generation + full identity CAS、
replacement preservation 与 Gateway best-effort policy，并在 fresh clean-context review
中取得零 finding `PASS`。首次 implementation SHA `385f0de01fc2331608ec858a18b50a9ca1025b48`
的 canonical run `6096eaeec33ea9b1be3dec9eda4e64f8` 为 sealed `FAIL`（13/15），不得当作
source PASS；修复两处测试合同后，`65b65c13` 在新的短路径、non-local detached clone
取得 canonical 15-suite `PASS`（15/15 suites、15/15 observations、runner exit 0）。最终
run ID `1ccc0111e970ef761affa6f90177d92d`，completion seal 绑定的 source snapshot
manifest SHA-256 为 `a225707be5a1c5c155f5d0b86c46c932a9fe30557df45230208fb71b0e776450`。
该 PASS 只证明 `65b65c13` production source，不建立新的 artifact 或 live 结论；后续
evidence-only 文档提交也不能改写 tested source identity。

上一份已验收的 Gateway reuse-health production source candidate 是
`next@40a2b9a762fe9bfd7b8c871044a34d6fe20bd9cf`。主实现提交 `73c726a45da82d58296614b28a792e9bdbedbfdd`
在 `AppState` 下冻结 generation、tracked child PID、端口、secret、provider、gateway/shim、
launch id、key fingerprint 与完整 launch recipe，锁外执行 Gateway reuse HTTP health，再按
generation + full owner identity CAS 接受结果；stale result fail closed，不清理或覆盖 replacement，
旧进程清理与 spawn 的锁边界保持不变。主实现与随后两处 source-gate 计数修复分别由新的
fresh clean-context reviewer 取得零 finding `PASS`。首次 `73c726a` canonical run
`b6a4b64b3ac363cc1c12b80308b41e45` 为 sealed `FAIL`（13/15），原因是新增 Rust test 后两处
source-gate 容量断言仍为 573，不得当作 source PASS；修复后的 `40a2b9a` 在新的短路径 detached
worktree 取得 canonical 15-suite `PASS`（15/15 suites、15/15 observations、runner exit 0）。
最终 run ID `d43ae8a5a78eda3fe17af2316646e0f4`，completion seal 绑定的 source snapshot manifest
SHA-256 为 `23cc8300302f05d26bcfc758989ad75d961866ded00507a5307a265b49ffed8f`。该 PASS 只证明
`40a2b9a` production source，不建立新的 artifact、installed、live、签名或 release 结论；
后续 evidence-only 文档提交也不能改写 tested source identity。

当前最新已验收的 production source candidate 是
`next@06b630bb3e3fb0d63425bf48d1ffc2d3613fd992`。Gateway spawn 在 `AppState` 下只冻结
generation、空 slot、secret、完整 candidate owner 与 launch recipe；candidate log、命令与环境构造、
Skill bridge 配置 staging、`Command::spawn()` 和 health poll 均在锁外执行，再按 generation + 完整
candidate owner CAS 接受 child。generation 漂移或 replacement 已出现时不会覆盖 replacement，也不会
发布 candidate log/key；active、rejected 与 tracked-cleanup 的不确定 child owner 均由独立的 process-local
RAII registry 保留。所有 destructive caller 都必须消费 typed stop outcome，并在无法确认 child 已退出时
先于 config、credential 或 binding commit fail closed。该 SHA 的 fresh formal independent clean-context
review 为 `PASS`（`BLOCK/HIGH/MEDIUM/LOW=0/0/0/0`），并在新的 clean、non-shallow clone 取得 canonical
15-suite `PASS`（15/15 suites、15/15 observations、runner exit 0）；run ID
`cda9d6cca7deec2554afaae785611c59`，completion seal SHA-256 为
`d8ddf8a05479703a797ef0a7728eb4de3b5cf3c08d9eb53097f572892420341d`，其绑定的 source snapshot
manifest SHA-256 为 `448cb9cf24b7f64f044ff589e4f5ca6b97fb7e8710b02db39884d69a65716e33`。
首次实现候选 `4386dbf` 的 run `9c982cd561a45a766ef3d9cf1a810b48` 是 sealed `FAIL`（14/15），
只暴露并随后修复了 Skill boundary 的陈旧静态锚点，不得当作 source PASS。最终 source PASS 不建立
artifact、installed、live、真实 provider、签名或 release 结论；Skill / MCP 探针继续后置。

`06b630b` exact artifact 的最终 G1 binding receipt SHA-256 为
`f6490b1765d982c4453571676cb3561f6f1c3a20a9af3850d30f8e405e795573`；CSSwitch bundle、
Desktop 与 packaged Gateway SHA-256 分别为
`634c13f2597c10cbbf75a7cac8d1af135523eccff2cb86373824695cccb32e1a`、
`cf0e84e6b33b761767394b6d5f3579e310bec5c07de8015cd79f2d407d9d5274`、
`ed4dae8ec8139c4828dd0915d1594d69001e7b504c582a710e905707ef9d03d1`。G1 authority 绑定
source-gate run `8728828e41ee0dc8ab578856995fa530` 与 completion seal SHA-256
`e1ef35f06d0f88a8cbf6fa4ea8b12c129afd10b55f85f89d2074b0d23d72b455`。packaged Gateway
在空 HOME 返回 `state_missing` 且不落文件；Science 0.1.25 package/executable identity 与已知
exact identity 一致。受限沙箱中的首次 gate run `d1e81f780fd7ef890053351267ec2282` 是 sealed
`FAIL`，不得当作 G1 authority；允许 loopback/进程 fixture 的最终原命令重跑才建立 PASS。完整
identity、失败边界与不能外推的层见[日期化 exact-artifact 调查](../../docs/evidence/investigations/2026-08-10-csswitch-06b630b-exact-artifact.md)。

`06b630b` exact tuple 的完整 `B-RUNTIME-01` run `r06b630bb` 在全新隔离 HOME/data-dir、
deny-egress sandbox、真实 Science 0.1.25 与 loopback fake provider 下完成 normal production
wiring。56 条事件 0 failure，5/5 provider requests consumed；重开保持同一 Desktop/Gateway
owner，停止/重启产生新的 Gateway/Science owner，最终 8 个 exact PID、4 个动态端口、`8765`
与 runtime root 全部清零。49 项 evidence hash 全部复算 `OK`，总判定为
`PASS(scope=csswitch-gateway-science,loopback-provider-fixture)`；真实 provider/账号、Skill/MCP、
SSH、installed、签名和 release 不外推。完整 identity、deadline、network 与 cleanup closure
见[日期化 isolated-live 验收](../../docs/evidence/investigations/2026-08-10-claude-science-0.1.25-b-runtime-01.md)。

`06b630b` exact tuple 的 `B-CORE-01` run `bcore-06b630b-r2` 在隔离 HOME/data-dir、真实
Science 0.1.25、loopback mock 与两个专用 synthetic Git fixture 下完成。project / 文件读写、
request → grant → revoke、revoke 后拒绝、sibling 越界拒绝、artifact v1/v2 parent / hash / diff /
producing frame 以及 annotation 发送前持久 DB row 与下一消息传递均闭合。活动期 39 个 socket rows
全部为 loopback；四个 exact PID、四个动态端口、`8765`、runtime、fixture 与临时 driver 最终清零。
23 项 evidence hash 全部复算 `OK`，总判定为 `PASS`。首次 r1 因 fixture root 不可申请 permission，
单独固定为 `INCONCLUSIVE`，不参与 PASS；B-CONTEXT、真实 provider、Skill / MCP、SSH、installed、
签名和 release 均不外推。精确 identity、sub-gate、持久状态与 cleanup closure 见
[日期化 B-CORE 验收](../../docs/evidence/investigations/2026-08-10-claude-science-0.1.25-b-core-01.md)。

## 当前源码问题

- **Sibling stop owner / wait 边界**：`stop_all`、切换 official 的 `set_mode`、teardown `set_settings` 与 native exit 已使用 process-local owner claim、锁外 wait 与 identity CAS；downgrade cleanup 及其他 sibling stop caller 尚未全部收敛到同一边界。当前 owner 与缺口见[运行时状态与事务](../../docs/architecture/runtime-state-transactions.md)。
- **Gateway 锁边界**：`06b630b` 已把 candidate log、命令与环境构造、Skill bridge 配置 staging、spawn 和 health poll 全部移出 `AppState`；锁内 reservation 与 generation + full candidate-owner CAS 守护接受，replacement 不被覆盖，不确定 child 由独立 registry 持有，destructive caller 对 typed uncertain stop fail closed。该 exact SHA 的 formal independent clean-context review 为 `PASS`（`BLOCK/HIGH/MEDIUM/LOW=0/0/0/0`），canonical 15-suite 为 15/15 `PASS`。
- **跨文件恢复边界**：history full-snapshot restore 已有 typed complete-record CAS、protected snapshot、唯一跨进程 effect owner 与 durable outcome；其他 sibling full-snapshot restore / multi-file crash boundary 尚未统一。
- **Science adoption ledger**：已有受校验的内容寻址 snapshot、managed identity / receipt、healthy defer 和 cross-runtime rollback guard，但没有通用 predecessor / candidate / adoption diff ledger。

## 当前证据缺口

下表只记录 canonical mapping 中仍缺的层；production owner、caller、failure boundary 和 fixture 的唯一明细在[生产链路验收的决策映射](../../docs/operations/real-machine-acceptance.md#3-重要重构决策映射)。`source anchors mapped` 不等于 source seal。

| 重要重构决策 | Production source | Exact artifact | Isolated-live | Authorized live |
|---|---|---|---|---|
| 一键入口、Gateway / Science 启动与 finalize | `06b630b` exact-SHA review + canonical 15-suite `PASS`；Gateway reservation / 锁外 spawn / full-owner CAS、rejected/uncertain child owner 与 destructive caller fail-closed 已闭合；formal independent clean-context review `PASS`（`0/0/0/0`） | `06b630b` 的 `CSSwitch Test.app`、packaged Rust Gateway 与 Science 0.1.25 exact tuple 已由递归 G1 receipt 绑定并 `PASS`；installed/signing/release 不外推 | 同一 `06b630b` tuple 的 `B-RUNTIME-01=PASS`；`B-CORE-01=PASS` 限定闭合合成 project / 文件、permission、artifact lineage 与 annotation 持久状态；`B-CONTEXT-01=PASS(scope=isolated-request-shape)` 限定闭合上下文 local surface/request shape 与 project/session 隔离 | 真实 provider/账号分项 `NOT-RUN` |
| runtime mutation 与 stop ownership | `65b65c13` exact-SHA review + canonical 15-suite `PASS`；native-exit replacement/race 与 best-effort Gateway policy 已闭合，downgrade 等 sibling gap 仍开放 | `9cc0d15` exact artifact 已由 `B-RUNTIME-01` 绑定；本行专项 artifact gate 未单独执行 | normal stop/restart observation `PASS`；replacement/race 不由 live 外推 | normal stop `NOT-RUN` |
| authority finalize、compensation 与 replay | source/compensation/replay fixture anchors mapped；fresh source seal 待执行 | `9cc0d15` exact artifact 已由 `B-RUNTIME-01` 绑定；本行专项 artifact gate 未单独执行 | normal binding/finalize observation `PASS`；crash/compensation/replay 不由 live 外推 | happy path `NOT-RUN`；crash window 不要求 live |
| history full-snapshot recovery | source anchors mapped；fresh source seal 待执行 | `NOT-RUN` | production IPC + synthetic history `NOT-RUN` | 真实用户历史不作默认 gate |
| Science host adapter 与 Skill host bridge | current owner/seam anchors mapped；最新 current source `555d4e8` canonical 15-suite `PASS`。本轮 formal review 只覆盖 Gateway 旧清理，Science host adapter / Skill host bridge 专项独立审查仍为 `NOT-RUN`。日期化调查记录的 synthetic `c4a1159` 只有 13/13，且当前 repo 无法解析该 object，不另行构成 current production source PASS | `c4a1159` exact artifact 只作为当次调查的日期化 identity；`555d4e8` current exact artifact `NOT-RUN` | 日期化 `B-SKILL-01=INCONCLUSIVE(reason=safety-stop)`：Science 在对话前尝试非预期外部 destination；六阶段均 `NOT-RUN` | 真实 Skill / domain execution 分项 `NOT-RUN` |
| provider protocol capabilities | source/test/fixture anchors mapped；fresh source seal 待执行 | `NOT-RUN` | current artifact + Gateway + real Science + loopback provider fixture `NOT-RUN` | stream/tools/reasoning/error 按 provider/model `NOT-RUN` |

2026-08-07 的 `c531006` controller 闭环已固定完整 artifact / Science tree manifest、fixture
receipt、provider launch receipt 与 network isolation receipt。pre-run manifest SHA-256 为
`9dd77aaa73512eb9fb32542638a479cfac52d92dd877f54215575189ff449b78`；closing
`hashes.sha256` SHA-256 为
`9d9298e426629d7c5284a18d077bf4ec84e2cc2476631e9421772fa70cacec1f`，19/19 条目复核
`OK`。网络 self-test 实际允许 IPv4/IPv6 loopback，并以 `EPERM` 阻断 IPv4/IPv6 TCP、UDP、
DNS transport 与 mDNSResponder IPC；唯一系统 resolver 查询失败。该轮没有启动 Desktop / Gateway /
Science，固定判定为 `INCONCLUSIVE(reason=pre-run-only)`，不能追溯升级旧 G2，也不能代替完整
`B-RUNTIME-01`。exact identity 与不能外推的范围见
[日期化 pre-run 调查](../../docs/evidence/investigations/2026-08-07-isolated-live-pre-run-receipts-egress-guard.md)。

2026-08-08 的完整 `B-RUNTIME-01` 使用 `6e09e68` clean source gate 与同 SHA 全新构建的
exact artifact，在 deny-egress 隔离环境中完成 Desktop → packaged Rust Gateway → Science、
loopback fake provider、一键开始、单实例重开复用、产品停止/重启、再次请求、最终停止和
精确清理，总判定为 `PASS`。独立 clean-context 代码审查为 `PASS`，
`BLOCK/HIGH/MEDIUM/LOW=0/0/0/0`。真实 provider/账号、Skill、SSH、installed、签名、公证与
public release 仍为 `NOT-RUN`；精确身份、断言与 cleanup closure 见
[日期化完整验收](../../docs/evidence/investigations/2026-08-08-claude-science-0.1.25-b-runtime-01.md)。

2026-08-08 的新 `B-CORE-01` r12 使用 `9cc0d15` 的完整 source-gate seal、同一 exact artifact 的
G1 receipt 与重新完成的 `B-RUNTIME-01=PASS`。在全新隔离 HOME/data-dir、真实 Science 0.1.25、
loopback mock 和两个专用 synthetic Git fixture 下，request → grant → read/write → UI revoke →
revoke 后拒绝 → sibling 越界拒绝全部闭合；artifact v1/v2 parent、hash、diff、previous-version 和
producing cell 对齐；Markdown 真实选区 annotation 随下一消息传递，mock 的结构键、选区和 note
标志均为 true。完整 raw socket capture 含 39 rows，全部为 `127.0.0.1` / `::1`，non-loopback=0，
8765=0。四个目标 PID、五个端口、fixture、runtime 和临时 driver 最终全部清零，primary worktree
回到 clean exact HEAD。因此当前总判定为 `B-CORE-01=PASS`；`B-CONTEXT-01` 的 B-CORE 前置已满足，
但不表示 B-CONTEXT 本身已运行。精确 sub-gate、身份、签名非目标和 cleanup 见
[日期化验证](../../docs/evidence/investigations/2026-08-08-claude-science-0.1.25-b-core-01.md)。

2026-08-08 的 `B-CONTEXT-01` 继续绑定 `9cc0d15` exact artifact / Science 0.1.25 tuple，
只验证 local surface、状态与 isolated request shape。plan approve/reject、delegation、fork/restore、
memory save/search、compaction、Reviewer/Specialist surface 全部取得独立观察；两个合成 project / 两个
target root session 的 717 个脱敏 request envelope 中，四项 cross-domain violation 均为 0。Reviewer
保持 `Inconclusive`，Reviewer/Specialist 服务端结果仍为 `UNVERIFIED`。活动期 non-loopback socket=0、
8765=0，hashed closing 进程/端口清零；最终修复后的 28-entry closure 已脱敏一次性 nonce，
并含 post-cleanup receipt 确认 runtime 与本轮两个临时 worktree/build/driver 已删除。因此当前
`B-CONTEXT-01=PASS(scope=isolated-request-shape)`。global `About you` memory 的显式共享 surface
不外推为 project-scoped memory 的全部语义；精确状态、fixture loop 噪声、网络与 cleanup 见
[日期化验收](../../docs/evidence/investigations/2026-08-08-claude-science-0.1.25-b-context-01.md)。

2026-08-10 的新 `B-CONTEXT-01` 绑定同一个 `06b630b` G1 exact artifact / Science 0.1.25
tuple，没有重建 artifact。plan approve/reject、delegation、fork/restore、Memory save/search、
compaction、Reviewer/Specialist local surface 与两 project/两 target root session 隔离均取得
独立观察；140 个脱敏 request envelope 的四项 cross-domain violation 均为 0。Reviewer 保持
`Inconclusive`，Reviewer/Specialist 服务端结果为 `UNVERIFIED`。活动期 non-loopback=0、
8765=0；停止后 owned process/端口清零，合成 Memory 删除，runtime/source worktree/driver/pycache
精确清理。23-entry hash closure 已脱敏一次性 nonce 并纳入 post-cleanup receipt。因此当前
`B-CONTEXT-01=PASS(scope=isolated-request-shape)`；真实账号/provider、Skill/MCP、SSH、
installed、签名与 release 均不在本轮范围。精确状态、身份、网络与 cleanup 见
[日期化验收](../../docs/evidence/investigations/2026-08-10-claude-science-0.1.25-b-context-01.md)。

2026-08-09 的 `B-SKILL-01` 绑定 `c4a1159` clean source gate 与同 SHA 新构建的 exact
`CSSwitch Test.app` / packaged Gateway / Science 0.1.25 tuple。Science 在用户对话前的 bundled
warmup 已尝试非预期外部 destination，立即触发该 probe 的严格停止条件；停止前只完成
identity、隔离根、fixture 与 deny-egress 护栏核对，六阶段均为 `NOT-RUN`。停止后继续的
host-access / managed tool 诊断属于程序偏差，已排除在正式判定之外。因此当前只能固定为
`INCONCLUSIVE(reason=safety-stop)`，不能由 source/artifact 邻层或停止后观察补绿。
四个专用端口和全部 attributable process 已清零；精确 identity、ledger、hash 与禁止绕过边界见
[日期化调查](../../docs/evidence/investigations/2026-08-09-claude-science-0.1.25-b-skill-01.md)。

先前 r1 的 evidence envelope 缺口、r2 的 no-opt-out outer 启动失败，以及 r3 的 exact-artifact
不匹配、permission fixture 误布置和 non-loopback safety-stop 均继续保留为日期化历史证据；它们
不能反推各自运行已 PASS，也不再覆盖 r12 对新 exact source/artifact 的当前判定。

2026-08-07 的历史 `B-RUNTIME-01` 完整尝试复用了当日冻结的 exact artifact/Science tuple
与 hardened controller。pre-run/G1/network receipts 通过；production Desktop 与 packaged Rust Gateway
建立 exact identity 且 Gateway health ready，但真实 Science launch 没有建立目标 listener，允许
保留的日志不足以在产品、fixture、harness 或系统限制间归因。reopen driver 又产生第二个 exact
Desktop，因此 Gateway PID/launch id 未变不能升级为 reopen PASS。产品 status、stop、restart
保持 `NOT-RUN`；exact-PID 清理只建立 safety cleanup。独立 clean-context reviewer 接受总判定
`INCONCLUSIVE(reason=science-minimal-start-not-closed-and-reopen-process-ownership-unproven)`；
runtime root 与一次性 runner 已删除，raw evidence 24/24 hash closure 保留。精确边界见
[日期化完整尝试](../../docs/evidence/investigations/2026-08-07-claude-science-0.1.25-b-runtime-01-full-attempt.md)。

2026-08-07 的 run 后只读 G1 receipt 已把 `next@e7dfde13636cbf3b377d01dbba3a2aee88e62822`、
`CSSwitch Test.app`、packaged Gateway/resources 与 Claude Science 0.1.25 package/executable
固定为当前 exact tuple；该 receipt 不能追溯替代 G2 的 pre-run manifest。授权的 G2 run 中，
production auto-boot 建立 exact Gateway/Science listener 与 managed receipt，UI stop-all 后
端口、PID、receipt 和 open FD 清零，随后删除 exact runtime root，8765 基线不变。完整 `B-RUNTIME-01` 的
open/reopen/restart 仍为 `NOT-RUN`；provider 协议、真实 provider/账号、Skill/SSH、installed、
签名、公证和 public release 也未运行。loopback provider 与 0 inference hits 只作为 observation 保留；
运行中观察到 Gateway 到 `198.18.0.54:443` 的禁止 non-loopback socket 后触发 safety-stop，加上
pre-run manifest/provider receipt 不完整，G2 总判定为 `INCONCLUSIVE(reason=safety-stop)`，不能写 PASS。
exact identity、授权与 cleanup 见
[日期化调查](../../docs/evidence/investigations/2026-08-07-claude-science-0.1.25-g2-start-stop.md)。
本段只陈述 `e7dfde1` 的历史 G1/G2 记录；当时的后续证据文档 HEAD 没有同源 artifact，
不能继承该轮 G1 PASS，也不覆盖上方 `6e09e68` 的当前映射。

2026-08-06 的 A0 artifact 绑定 `9cf75d19e7853b91b2f9a7c85afbd66747cb4fa3`，早于当前生产源码变化，只能从其[日期化 artifact 审计](../../docs/audits/2026-08-06-a0-frozen-baseline-artifact.md)读取限定结果，不能外推 current source、Desktop/Science live、provider、installed、签名或 release。2026-07-30 的 Science `B-RUNTIME-01` 同样只保留为绑定当时版本与身份门禁的[日期化调查](../../docs/evidence/investigations/2026-07-30-claude-science-0.1.25-b-runtime-01.md)，不再作为当前 probe gate。

## 产品与分发边界

- 第三方模型支持不能由一次文本聊天代替。stream、tools / `tool_choice`、reasoning、structured output、vision、stop / error semantics 必须按 provider、model 与 operation 分项授权和取证；当前能力 owner 见[产品 / Claude Science 能力地图](../../docs/features/product-science-capability-map.md)。
- 外部 Skill 的 content fetched、package committed、Science discovered、Agent attached、loaded / triggered、领域执行、restart persistence、quarantine 与 detached 是不同结论。CSSwitch 只拥有窄安装 / 投影桥，不拥有 Skill runtime 或通用 MCP 管理面。
- 系统 SSH 默认关闭；parser、OpenSSH invocation、真实 server connectivity 与 scheduler 是独立层，真实 host 必须逐项授权。当前合同见[系统 SSH 配置复用](../../docs/features/system-ssh.md)。
- Web Search、hosted MCP / Connectors、Reviewer entitlement、官方 catalog / usage 由 Anthropic 账号和服务拥有；第三方 Gateway 不模拟这些 entitlement。
- source、artifact、isolated-live、authorized live、installed、signing/notarization 与 public release 相互独立。缺少的层保持 `NOT-RUN` / `INCONCLUSIVE` / 未验证，不能借邻层补绿。
