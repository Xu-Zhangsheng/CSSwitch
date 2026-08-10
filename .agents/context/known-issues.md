# 当前已知问题与证据缺口

状态：当前；唯一验收路线以[生产链路验收](../../docs/operations/real-machine-acceptance.md)为准

最后复核：2026-08-11（Asia/Taipei）

失效条件：production owner / caller、确定性 fixture、候选 source、artifact identity、Science / Gateway runtime、provider capability、installed/runtime、签名或公开 Release 任一相关事实改变时，受影响条目立即失效并须实时重审。

本页只登记当前仍开放的问题和各证据层缺口，不保存另一份路线或验收合同。使用前必须实时核对 branch、HEAD、worktree、目标 artifact 与 runtime；日期化 audit/evidence 只证明其绑定的 SHA、artifact、版本和环境。

## 唯一当前路线

所有旧 R3–R11、R4/R5、S7、Post-D0/Post-Q0 及其他阶段编号路线均已退役，只能从[历史审计索引](../../docs/audits/README.md)查证当时的 source closure、取舍和证据边界。旧编号、旧 sole NEXT、旧计划顺序和已完成 source gate 都不能授权或替代新的实现、artifact 或 live 阶段。

新的唯一验收顺序是：**重要重构决策 → production source → exact artifact → isolated-live → authorized live**。当前映射、每层进入条件、授权边界和故障 fixture 边界只在[生产链路验收](../../docs/operations/real-machine-acceptance.md)维护；Science 运行细则见[Science 探针合同](../../docs/operations/science-probe-spec.md)。

2026-08-10 最新已验收的 exact source/artifact binding 为
`next@9e08924481c8f5edb181254332d94daba0cbe4b2`。独立 clean exact worktree 的固定 15-suite
source gate 为 `PASS`（15/15 suites、15/15 observations、runner exit 0）；随后由该 SHA 以
`acceptance-build` 全新生成的 `CSSwitch Test.app`、packaged Rust Gateway 与 Claude Science
0.1.25 exact tuple 经递归 G1 validator 取得 `PASS`。后续 `f8ef373` 只记录 source evidence，不能
改写被构建的 source/artifact identity。该 tuple 的 `B-RUNTIME-01` 已在隔离 HOME/data-dir、真实
Science、packaged Gateway 与 loopback fake provider 下取得有边界的 `PASS`；`B-CORE-01`、
`B-CONTEXT-01`、完整 Provider、Skill、SSH 与 installed/runtime 仍为 `NOT-RUN`。旧 `06b630b`
tuple 的对应 PASS 只保留为历史日期化证据，不能继承给新 artifact。

此前 cold one-click prior Science stop 子阶段的已验收 production source candidate 是
`next@5cf3eb1670eec6dc58d5d1d873ca3834f9a76623`。cold one-click prior Science stop 保留
credential-free durable `PriorStopIntent` → exact stop → typed outcome 顺序，在 intent 后于
`AppState` 下冻结 generation、runtime、confirmed-stopped proof、child PID、port 与 URL，锁外执行
既有 stop/TERM/KILL/wait，再按 generation + full owner CAS 发布。stale publication 保留
replacement Science、持久化 `Unknown` 并在 authority snapshot 前失败。本窗口 code-grounded review
为 `PASS`（`clean-context=NO`，`BLOCK/HIGH/MEDIUM/LOW=0/0/0/0`）。`424800d` 的受限 sandbox run
`03cbbe30677f17594580c72fd369d227` 是 sealed `FAIL`（runner exit 12），多个 loopback/process fixture
同时受阻；允许本地 fixture 后的 run `59734c1198bd9b8b291bf77a5eabc8d9` 仍是 sealed `FAIL`
（14/15），暴露新增测试误用了额外 environment API。修复后的 `5cf3eb1` canonical run
`155365a14b55bcec89090a29d481bb6c` 为 15/15 suites、15/15 observations、runner exit 0；completion
seal SHA-256 为 `53051235eb8ae0720ffbe5642a6977012cb06904b5087e80e5678c3b0077da35`，source snapshot
manifest SHA-256 为 `98cbbaad757af123ad7236de216d6963cd13bf67a7fbb972b59b0028bad9d818`。该 PASS
只证明 production source，不建立新的 artifact、installed、live、签名或 release 结论。

上一份已验收的 production source candidate 是
`next@b5141a9bab393cbed5e180d3e47e1d4ae300f290`。terminal downgrade cleanup 已复用
process-local Science owner claim、锁外 stop/wait 与 generation + full identity CAS；陈旧结果
保留 replacement Science，仍按原 terminal policy 停 tracked Gateway，并在 export、backup 或 v2
publication 前失败。fresh formal independent clean-context review 为 `PASS`
（`BLOCK/HIGH/MEDIUM/LOW=0/0/0/0`）。首次 candidate `e419e27` 的 run
`6cac4f0f508a553ac6de5c196117ff05` 是 sealed `FAIL`（13/15），只暴露并随后修复两处 591 → 592
source-gate 容量断言，不得当作 PASS。最终 `b5141a9` canonical run
`44d725daa53005a4321d8f49f56a3868` 为 15/15 suites、15/15 observations、runner exit 0；
completion seal SHA-256 为 `f43c418b5d9a7e05739daa4381c5376135f1e07825b8c7353b2e7f281a881077`，
source snapshot manifest SHA-256 为 `f66411518a46e5de9b4f7fbc7f3873bc42814aa93b3ffa270f131f82f1938c3e`。
该 PASS 只证明 production source，不建立新的 artifact、installed、live、签名或 release 结论；
后续 evidence-only 文档提交也不能改写 tested source identity。

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

当前最新已验收的 Gateway spawn production source closure 是
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

`9e08924` exact artifact 的最终 G1 binding receipt SHA-256 为
`39aa3dc5866807140d42409ab8eea2f6625fa4d9f7212f06482dcc0db0c9b762`；CSSwitch bundle、
Desktop 与 packaged Gateway SHA-256 分别为
`d77cb799f2063241041cc8d17bd57a3c72f49cc7b0da6899d63a52795df1ac64`、
`ada760cb9dcfdd9b2151d652ff744f300a914b3bef8c07ea85ce886994458a6b`、
`bfa05512337329f52811c2d7c08081ed2249a34c6ed98dd7fe7d83313d9b3036`。G1 authority 绑定
source-gate run `078d462c81abcec146644c8096254069` 与 completion seal SHA-256
`1c071a18701ac6e6a191c6dfc3cd1513d3465bce90d3567a6460cd46f36d0fe8`。packaged Gateway
在空 HOME 返回 `state_missing` 且不落文件；Science 0.1.25 package/executable identity 与已知
exact identity 一致。完整 identity、诊断边界与不能外推的层见
[日期化 exact-artifact 调查](../../docs/evidence/investigations/2026-08-10-csswitch-9e08924-exact-artifact.md)。

`9e08924` exact tuple 的完整 `B-RUNTIME-01` canonical run `r9e08924b` 在全新隔离 HOME/data-dir、
deny-egress sandbox、真实 Science 0.1.25 与 loopback fake provider 下完成 normal production
wiring。55 条事件严格单调，5/5 provider requests consumed；LaunchServices 重开保持同一
Desktop/Gateway owner，停止/重启产生新的 Gateway/Science owner，最终 5 个 tracked PID、4 个动态
端口与 runtime open PID 清零，随后 canonical runtime root 与 8 组旧复用 run-id 的无效 attempt
临时目录精确删除；静态 LaunchServices initializer 已复制进 evidence 并删除旧临时 root。
post-cleanup 后 51 项 evidence hash 全部复算 `OK`，elapsed `291.892949s`
小于 300s hard deadline，总判定为
`PASS(scope=csswitch-gateway-science,loopback-provider-fixture)`。
完整 Provider、真实 provider/账号、B-CORE、B-CONTEXT、Skill/MCP、SSH、installed、签名和 release
不外推。完整 identity、deadline、network 与 cleanup closure 见
[当前 isolated-live 验收](../../docs/evidence/investigations/2026-08-10-claude-science-0.1.25-b-runtime-01.md)。

此前 `06b630b` exact tuple 的完整 `B-RUNTIME-01` run `r06b630bb` 在全新隔离 HOME/data-dir、
deny-egress sandbox、真实 Science 0.1.25 与 loopback fake provider 下完成 normal production
wiring。56 条事件 0 failure，5/5 provider requests consumed；重开保持同一 Desktop/Gateway
owner，停止/重启产生新的 Gateway/Science owner，最终 8 个 exact PID、4 个动态端口、`8765`
与 runtime root 全部清零。49 项 evidence hash 全部复算 `OK`，总判定为
`PASS(scope=csswitch-gateway-science,loopback-provider-fixture)`；真实 provider/账号、Skill/MCP、
SSH、installed、签名和 release 不外推。该历史 tuple 的精确 identity 与 hash 只由本段固定，
不得链接或外推到当前 `9e08924` 调查。

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

2026-08-10 comprehensive refactor closure 的最新 code-bearing production source candidate 为
`next@9e08924481c8f5edb181254332d94daba0cbe4b2`。fresh clean-context sol high 终审为
`PASS`（`BLOCK/HIGH/MEDIUM/LOW=0/0/0/0`）；canonical run
`078d462c81abcec146644c8096254069` 为 15/15 suites、15/15 observations、runner exit 0，
completion seal SHA-256 为
`1c071a18701ac6e6a191c6dfc3cd1513d3465bce90d3567a6460cd46f36d0fe8`。首次候选
`18aabcf` 的 run `3ff7b58addee2f5c5b4b8902ad258033` 是 sealed `FAIL`（12/15），只暴露并
随后修复三处 closure regression，不得当作 PASS。该 source PASS 不建立新的 artifact、installed、
live provider、Science、SSH、signing 或 release 结论。

- **Gateway 锁边界**：`06b630b` 已把 candidate log、命令与环境构造、Skill bridge 配置 staging、spawn 和 health poll 全部移出 `AppState`；锁内 reservation 与 generation + full candidate-owner CAS 守护接受，replacement 不被覆盖，不确定 child 由独立 registry 持有，destructive caller 对 typed uncertain stop fail closed。该 exact SHA 的 formal independent clean-context review 为 `PASS`（`BLOCK/HIGH/MEDIUM/LOW=0/0/0/0`），canonical 15-suite 为 15/15 `PASS`。
- **跨文件恢复边界**：history full-snapshot restore 已有 typed complete-record CAS、protected snapshot、唯一跨进程 effect owner 与 durable outcome；其他 sibling full-snapshot restore / multi-file crash boundary 尚未统一。
- **Science adoption ledger**：已有受校验的内容寻址 snapshot、managed identity / receipt、healthy defer 和 cross-runtime rollback guard，但没有通用 predecessor / candidate / adoption diff ledger。

## 当前证据缺口

下表只记录 canonical mapping 中仍缺的层；production owner、caller、failure boundary 和 fixture 的唯一明细在[生产链路验收的决策映射](../../docs/operations/real-machine-acceptance.md#3-重要重构决策映射)。`source anchors mapped` 不等于 source seal。

| 重要重构决策 | Production source | Exact artifact | Isolated-live | Authorized live |
|---|---|---|---|---|
| 一键入口、Gateway / Science 启动与 finalize | `9e08924` comprehensive exact-source review + canonical 15-suite `PASS`；Gateway reservation / 锁外 spawn / full-owner CAS、rejected/uncertain child owner 与 destructive caller fail-closed 继续闭合 | `9e08924` 的 `CSSwitch Test.app`、packaged Rust Gateway 与 Science 0.1.25 exact tuple 已由递归 G1 receipt 绑定并 `PASS`；installed/signing/release 不外推 | current tuple `B-RUNTIME-01=PASS`：真实 Science + packaged Gateway + loopback fixture 的 normal production wiring/lifecycle；B-CORE/B-CONTEXT 不外推 | 真实 provider/账号分项 `NOT-RUN` |
| runtime mutation 与 stop ownership | stop_all、set_mode、set_settings、native exit 与 downgrade cleanup 保持既有 owner / 锁外 wait / CAS；cold prior、managed DB restart、profile-switch rollback、history prior stop、live compensation 与 fresh-process replay cleanup 已统一为 transaction-scoped 完整 owner + exact request / 锁外 wait / generation + full-owner CAS；`9e08924` canonical 15-suite `PASS` | 同一 `9e08924` G1 exact artifact `PASS` | normal one-click / stop / restart lifecycle `PASS`；replacement/race/crash 仍由 source fixture 证明 | installed normal stop `NOT-RUN` |
| authority finalize、compensation 与 replay | durable step intent/effect/outcome、lease、crash/idempotence fixture 与共享 transaction stop executor 已映射；`9e08924` canonical 15-suite `PASS` | 同一 `9e08924` G1 exact artifact `PASS` | normal one-click transaction UI outcome `PASS`；durable compensation/replay 不由 happy path 外推 | installed happy path `NOT-RUN`；crash window 不要求 live |
| history full-snapshot recovery | history durable intent/effect/outcome、effect lease 与共享 transaction prior-stop executor 已映射；`9e08924` canonical 15-suite `PASS` | 同一 `9e08924` G1 exact artifact `PASS` | production IPC + synthetic history `NOT-RUN` | 真实用户历史不作默认 gate |
| Science host adapter 与 Skill host bridge | current owner/seam anchors mapped；`555d4e8` 仅是 host adapter / bridge 局部历史 source seal，早于当前 transaction caller seam 与后续 HEAD，不能表述为当前完整 caller seal。comprehensive source review 未替代 Skill 专项能力审查；日期化 synthetic `c4a1159` 只有 13/13，且当前 repo 无法解析该 object | 同一 `9e08924` G1 exact artifact `PASS`；只证明 bundle identity，不证明 Skill runtime | 日期化 `B-SKILL-01=INCONCLUSIVE(reason=safety-stop)` 只绑定旧 artifact；`9e08924` 六阶段均 `NOT-RUN` | 真实 Skill / domain execution分项 `NOT-RUN` |
| provider protocol capabilities | source/test/fixture anchors mapped；`9e08924` canonical 15-suite `PASS` | 同一 `9e08924` G1 exact artifact `PASS` | DeepSeek-off basic loopback request shape 5/5 `PASS`；完整 Provider capability matrix `NOT-RUN` | stream/tools/reasoning/error 按 provider/model `NOT-RUN` |

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
