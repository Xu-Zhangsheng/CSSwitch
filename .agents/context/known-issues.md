# 当前已知问题与证据缺口

状态：当前；唯一验收路线以[生产链路验收](../../docs/operations/real-machine-acceptance.md)为准

最后复核：2026-08-14（Asia/Taipei）

失效条件：production owner / caller、确定性 fixture、候选 source、artifact identity、Science / Gateway runtime、provider capability、installed/runtime、签名或公开 Release 任一相关事实改变时，受影响条目立即失效并须实时重审。

本页只登记当前仍开放的问题和各证据层缺口，不保存另一份路线或验收合同。使用前必须实时核对 branch、HEAD、worktree、目标 artifact 与 runtime；日期化 audit/evidence 只证明其绑定的 SHA、artifact、版本和环境。

## 唯一当前路线

所有旧 R3–R11、R4/R5、S7、Post-D0/Post-Q0 及其他阶段编号路线均已退役，只能从[历史审计索引](../../docs/audits/README.md)查证当时的 source closure、取舍和证据边界。旧编号、旧 sole NEXT、旧计划顺序和已完成 source gate 都不能授权或替代新的实现、artifact 或 live 阶段。

新的唯一验收顺序是：**重要重构决策 → production source → exact artifact → isolated-live → authorized live**。当前映射、每层进入条件、授权边界和故障 fixture 边界只在[生产链路验收](../../docs/operations/real-machine-acceptance.md)维护；Science 运行细则见[Science 探针合同](../../docs/operations/science-probe-spec.md)。

当前 Science background update managed-health / full-owner 修复的 production-code-bearing commit 是
`next@9476be5cd23ff7d78d01b59a4aa392af00bba904`；其上的本次 Context refresh 只更新当前状态，
不改变 production / test。Rust 1.96.1 clippy hygiene 修复已提交为
`next@76616ff085610dabf0112202324cc97d20fcbec6`；source/test-bearing exact candidate 为
`next@d2cf95e877aa110013a8360d6fcd72c1b38bcfb3`，包含本段的后续 Context refresh 仍只更新文档状态。
绑定 base `9c91bec` + tracked binary diff `61542943…6612` 的正式独立
candidate reviews 均为 `PASS`、四级 finding 全 0，608-test Desktop Rust suite、frontend、metadata、
inventory 与文档治理 checks 均为 `PASS`；clippy diff 的 `bash test/run-rust.sh` 与 fresh independent
code / test / documentation candidate reviews 也均为 `PASS`、四级 finding 全 0。

`next@d2cf95e877aa110013a8360d6fcd72c1b38bcfb3` 是当前 source closure：clean detached checkout
canonical GATE-SOURCE run `15dcfea64b2e3b58ded18910b676cc50` 为 15/15 suites、15/15 observations、
runner exit 0；completion seal SHA-256 为 `e1f654363c2299a636044e9d942067701c75db40ed1d75a8cad143a2479f9d05`，
source snapshot manifest SHA-256 为 `feca1e67b86d866ed5c35c75bfa80cb5e7f1cd917affabe0585467c777ded0d7`。
fresh clean-context completion review 为 `PASS`，`BLOCK/HIGH/MEDIUM/LOW=0/0/0/0`。该 tuple 只建立
`RUN-EVIDENCE-GREEN` / `SOURCE-GREEN`；artifact、installed、live、signing 与 release 不得继承。

`next@9c91bec611f98f2d87e44db9edd5b5f51210240c` 是此前一个 exact tested source：canonical
GATE-SOURCE run `85a71ae64d72aeddb935a6b17a077c95` 为 15/15 suites、15/15 observations、runner exit 0，
completion seal SHA-256 为 `98b3bc140dc00fbe5d4fad5b52d396246063ce314e5a124071939e9095295517`。
但随后正式 source review 发现 1 个 MEDIUM runtime observation 缺口，以及本文已修正的 2 个 HIGH
current-source / RM-21 evidence 漂移，因此结论为 `FAIL`；该 SHA 不是 production source closure。

当前 tuple 之前最近一条 source review 与 source gate 同时 `PASS` 的完整历史 tuple 是
`next@18a67881c7e7d760fa8deb7f53e6ba246a32d94d`。其 canonical GATE-SOURCE run
`8db59abb85f6c7d5f8d5694626ee61dc` 为 15/15 suites、15/15 observations、runner exit 0；completion
seal SHA-256 为 `7c36a8ef19d9c8000e93cd53beed565eb2c94da858d274eaf6285e9a40c9c6e0`，source snapshot
manifest SHA-256 为 `259c2b18732bc4d128484b041d6927537d6ed3dbd6957e5bfcfc76f91cf68085`。
该 SHA 内的
[ChangeRecord](../../quality/changes/next/CHG-SCIENCE-RUNTIME-ADOPTION-NEXT.json)
已纳入 healthy/deferred 不重启、binding-only retention、legacy V1 → V2 fresh replay provenance
hydration，以及 live/fresh compensation 共用 executable fingerprint 的修复与 fixture。该 `PASS`
仍只证明 source/test；但独立的 2026-08-12 artifact/live 线已从同一 exact source 新构建并绑定
Acceptance / normal 两份 artifact，完成 Science adoption scoped G2、覆盖安装、installed runtime、
真实 Provider 分项与 clean-context final review。该后续证据不回写 ChangeRecord，也不建立
Developer ID、公证、Gatekeeper、DMG 或公开 Release。完整边界见
[2026-08-12 日期化验收](../../docs/evidence/investigations/2026-08-12-csswitch-18a67881-adoption-installed-live-acceptance.md)。

2026-08-11 Provider compatibility 线的 production code-bearing source 是
`next@e1832bd35e9384265df9911a42f841ed90c0f43c`，最终 source/test closure 是
`0ecc7e2a105b2bb270d42f95a42148a6cd23bbca`；两者之间只有 test fixture、过时 integration
断言和文档更新。production 保留 Kimi 官方 server-search 声明和
响应块，不再把 `web_search` 降成 Science OPERON 无 executor 的普通 client tool；Rust
compatibility tests 与 targeted loopback 已通过。授权 RM-47 已从隔离 Test app 贯通真实 Science：
DeepSeek、Qwen、GLM、Kimi、MiniMax、SiliconFlow、Xiaomi 的最小文本/对应子项到达真实 Provider；
OpenRouter 到达上游但因余额不足返回 402；OpenCode、Grok、Gemini 因 credential 缺失未运行。
Kimi 搜索已 PASS；PDF 尝试已到 Science 本地 Python 一次性授权，但用户随后明确把 PDF compute
移出当前 gate，固定为 `DEFERRED(non-gate)`。Kimi selector-display 仍是 `INCONCLUSIVE`，但不改变
Gateway 路由到 `kimi-k3` 和 server-search PASS。`0ecc7e2` 的 full source gate
为 15/15 `PASS`（run `903ebd2e365ac12274639fc676ca9388`）；最终 Test app Desktop / packaged
Gateway SHA-256 分别为 `98f24b69e40c2e238fbf18ae26539c44293a4f33c7d6c9b025008b840597566a` /
`56a724832dc41dafd3d8bea6d5d67446d49393260336d4cd8aaf31152fca37a0`，最终 stop/guard/tab/runtime
cleanup 也已 PASS。因此有 credential 的 RM-47 最小 Provider/Science scope 已收口；OpenRouter
足额配额、三家缺 credential 与更深的逐 Provider capability card 仍未闭合，PDF 不再属于这些阻塞项。
完整矩阵见
[RM-47 日期化证据](../../docs/evidence/investigations/2026-08-11-rm47-authorized-live-providers.md)。
该段只绑定 `0ecc7e2` 的历史 live tuple；`18a6788` installed matrix 不继承其 Kimi
server-search PASS，并以 2026-08-12 日期化证据的逐项判定为准。

RM-46 的 `d74221e2948f32cd67db0aed8920af6122d0c798` source gate 与 exact-artifact
local-mock UI PASS 继续作为独立历史层；它不包含真实 Science/provider。完整边界见
[RM-46 日期化证据](../../docs/evidence/investigations/2026-08-11-rm46-provider-configuration-ux.md)。

当前已安装 artifact 与最近一条完整 Science adoption artifact/live 历史 tuple 仍绑定
`next@18a67881c7e7d760fa8deb7f53e6ba246a32d94d`：Acceptance artifact 的 canonical / Desktop /
Gateway identity 为 `bb19a7e6…6e9109` / `56f0bde9…bec612` / `b8e96803…57819b`，递归 G1
为 `PASS`；Science 0.1.25 adoption isolated-live 完成 healthy `deferred_healthy`、cold selected、
receipt v2 / binding / finalize / reopen 一致，G2 outer seal `09f72713…80286`，scoped `PASS`。
同 source normal artifact 已 exact 安装到 `/Applications/CSSwitch.app`，canonical / Desktop /
Gateway identity 为 `24f542d1…f1c2` / `1a75a29f…fe6e` / `8a619b94…5540`，installed smoke
`PASS`。真实 Provider receipt `c30342f5…a01ea` 保留逐 operation 的 PASS / INCONCLUSIVE /
NOT-RUN；final clean-context review 为 `PASS`、四级 finding 全 0，final seal `b1c26987…080ce`。
正式 review 后四个临时 App bundle 已删除，标准安装根与本任务已知临时构建根只剩一份
`/Applications/CSSwitch.app`。完整边界见
[2026-08-12 日期化验收](../../docs/evidence/investigations/2026-08-12-csswitch-18a67881-adoption-installed-live-acceptance.md)。

`next@a60c2ee656429903f1fd8f398dc6ad8194aa9346` 继续保留为历史完整
`B-RUNTIME/B-CORE/B-CONTEXT/B-PROVIDER` tuple。其独立 clean exact worktree 的固定 15-suite
source gate 为 `PASS`（15/15 suites、15/15 observations、runner exit 0）；随后由该 SHA 以
`acceptance-build` 全新生成的 `CSSwitch Test.app`、packaged Rust Gateway 与 Claude Science
0.1.25 exact tuple 经递归 G1 validator 取得 `PASS`。当前 tuple 的 `B-RUNTIME-01` canonical run
`ra60c2eec` 又在全新隔离 HOME/data-dir、deny-egress sandbox、真实 Science 0.1.25 与 loopback fake
provider 下完成 normal one-click、reopen reuse、stop、restart、再次请求、最终 stop/exit 与精确清理，
取得 scoped `PASS`。同一 tuple 的 `B-CORE-01` canonical run `bcore-a60c2ee-r1` 又在专用合成 project / Git
fixture 下完成 permission request → exact `rw` grant → read/write → UI revoke → post-revoke denial、
sibling 越界拒绝、artifact v1/v2 lineage/diff/preview/provenance、两次 runtime restart 后回读与真实
pointer annotation 到下一消息传递，取得 scoped `PASS`。同一 tuple 的 `B-CONTEXT-01` canonical run
`bcontext-a60c2ee-r13` 又完成 11/11 子门，150 个脱敏 request envelope 的四项跨域计数为 0，
活动期 26/26 socket rows 为 loopback，UI stop/exit 与精确 cleanup 均为 `PASS`，179/179 evidence hashes
复算 `OK`。同一 tuple 的 `B-PROVIDER-01` canonical run `provider-a60c2ee-r1` 又完成 11-case
exact-artifact local-mock 矩阵：10 个 case 由 exact App 启动 packaged Gateway，SiliconFlow 由同一
exact packaged Gateway 直启；99/99 observation/event、55/55 request 与 583/583 evidence closure
均为 `PASS`。Reviewer/Specialist 服务结果仍为 `UNVERIFIED`；RM-47 真实 Provider、Skill/MCP、SSH、installed、
升级/rollback、签名与 release-ready 仍为 `NOT-RUN`。旧 `a60c2ee` 的上述 scoped PASS 与更早
`9e08924`、`06b630b` tuple 的对应 PASS
只保留为历史日期化证据，不能继承给新 artifact。

`ra60c2eec` 的 55 条事件严格单调，5/5 provider requests consumed；LaunchServices 重开保持同一
Desktop/Gateway owner，产品重启产生新 Gateway/Science owner。controller overall elapsed 为
`298.825957s`，小于 300s hard deadline。五个 tracked PID、四个动态端口与 runtime open PID 清零，
随后 runtime、initializer 与 Python cache 临时根精确删除；post-cleanup 后 51 项 evidence hash 全部
复算 `OK`。正式 clean-context 独立审查为 `BLOCKER/HIGH/MEDIUM/LOW=0/0/0/0`、`PASS`。完整
identity、network、deadline、failed-attempt 与 cleanup 边界见
[当前 B-RUNTIME 日期化验收](../../docs/evidence/investigations/2026-08-11-claude-science-0.1.25-b-runtime-01.md)。

`bcore-a60c2ee-r1` 的 22 条事件严格单调且全为 PASS，79 条脱敏 mock hits 连续编号，11 条
annotation-transfer envelope 的结构键、选区与 comment marker 全为 true。活动期 31 个 socket rows
全部 loopback，8 个 exact PID、5 个端口与 6 个 attributable 临时路径最终清零；30 项 evidence hash
全部复算 `OK`。正式 clean-context 独立审查为 `BLOCK/HIGH/MEDIUM/LOW=0/0/0/0`、`PASS`。
本轮不读取账号数据库、真实用户文件或凭证；完整 sub-gate、identity、network 与 cleanup 边界见
[当前 B-CORE 日期化验收](../../docs/evidence/investigations/2026-08-11-claude-science-0.1.25-a60c2ee-b-core-01.md)。

`bcontext-a60c2ee-r13` 的 11 份 observation 与 11 条 event 均为 PASS，event id 1–11 严格单调；
150 个 request envelope 只保留结构、tool shape 与合成 marker，四项 project/session 跨域计数均为 0。
Reviewer root 显示 `Inconclusive` / 7 checks，compaction 显示 850K full history → 64 working context / 1
summary；活动期 26 条 socket rows 全部 loopback，8765 未使用。最终 CSSwitch Test 从产品 UI 正常
stop/exit，owned process/port、合成 Memory、浏览器 tab 与临时 runtime/driver/cache 全部清零；179 项
evidence hash 全部复算 `OK`。前序 r9–r12 的 tab binding、marker/schema 与 Reviewer capture 错误均按原
FAIL 保留，不是产品行为 FAIL。完整边界见
[当前 B-CONTEXT 日期化验收](../../docs/evidence/investigations/2026-08-11-claude-science-0.1.25-a60c2ee-b-context-01.md)。

`provider-a60c2ee-r1` 的 11/11 case decision 均为 PASS，每项 9 个 observation 与 9 个严格单调
event，共 99/99；55 个严格 fixture request 均 protocol-complete/final-ok，64 个 active socket row
全部 loopback。10 个 case 由 exact App 启动 packaged Gateway；SiliconFlow 因 production Desktop
不转发 ambient HTTP proxy，继续限定为 exact packaged Gateway direct local-mock，不外推 Desktop
E2E。完整 Provider loopback gate 从头重跑为 111/111 tests `OK`；最终 top closure 与 11 份 case
closure 又经递归复算全部 `OK`，runtime parent 与 22 个 case runtime root 均不存在。真实 provider/
model、账号、配额、计费与服务质量仍为 `NOT-RUN`；该旧矩阵本身不证明配置 UI。RM-46 配置 UX
现已由 `d74221e` 的独立 exact-artifact local-mock 验收关闭，但不能反向升级 `a60c2ee` tuple，也不能
外推 RM-47 live provider。旧矩阵的完整 identity、执行边界与 cleanup 见
[当前 B-PROVIDER 日期化验收](../../docs/evidence/investigations/2026-08-11-claude-science-0.1.25-a60c2ee-b-provider-01.md)；
RM-46 见[独立日期化证据](../../docs/evidence/investigations/2026-08-11-rm46-provider-configuration-ux.md)。

此前 cold one-click prior Science stop 子阶段的历史 production source candidate 是
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

上一份 Gateway spawn 子阶段的历史 production source closure 是
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

以下 `9e08924` exact artifact 及其 `B-RUNTIME-01`、`B-CORE-01`、`B-CONTEXT-01`、
`B-PROVIDER-01` 均为历史证据；因 current source、Desktop 与 Gateway identity 已变化，任何结果都
不得继承给当前 `a60c2ee` tuple。

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
完整 Provider、真实 provider/账号、B-CONTEXT、Skill/MCP、SSH、installed、签名和 release 不由
本项外推；该历史 tuple 的 B-CORE 由下一段独立证据固定。完整 identity、deadline、network 与 cleanup closure 见
[日期化 isolated-live 验收](../../docs/evidence/investigations/2026-08-10-claude-science-0.1.25-b-runtime-01.md)。

`9e08924` exact tuple 的 `B-CORE-01` run `bcore-9e08924-r1` 在全新隔离 HOME/data-dir、真实
Science 0.1.25、loopback mock 与两个专用 synthetic Git fixture 下完成。project / workspace 与
grant path 文件读写、request → grant → UI revoke、revoke 后拒绝、sibling 越界拒绝、artifact
v1/v2 identity / hash / diff / previous-version / execution provenance，以及 Chrome 真实选区 annotation
到下一消息传递均闭合。38 个 socket rows 全部为 loopback；四个 exact PID、五个端口、runtime、
fixture、临时 driver/cache/stage 最终清零。16 条事件严格单调且全为 PASS，14 项 evidence hash 全部
复算 `OK`，总判定为 `PASS`。本轮没有读取账号数据库、真实用户文件或凭证；B-CONTEXT、完整
Provider、Skill / MCP、SSH、installed、签名和 release 均不外推。精确 identity、sub-gate 与 cleanup
closure 见[日期化 B-CORE 验收](../../docs/evidence/investigations/2026-08-10-claude-science-0.1.25-b-core-01.md)。

`9e08924` exact tuple 的 `B-CONTEXT-01` run `bcontext-9e08924-r9` 在隔离 HOME/data-dir、真实
Science 0.1.25、packaged Gateway、loopback fixture 与合成 project/session 下完成。plan approve/reject、
delegation、fork/restore、Memory save/search/delete、compaction、Reviewer/Specialist local surface 与
two-project/two-session isolation 共 11 个子门均 `PASS`；Reviewer UI 仍为 `Inconclusive`，服务结果
保持 `UNVERIFIED`。159 个脱敏 request envelope 的四项跨域计数均为 0；exact owned process 的
26 条 socket rows 全部 loopback，四个 PID、五个端口、runtime 与临时 driver/cache 最终清零。
11 份 observation / event 与 89 项 evidence hash 全部复算 `OK`。本轮没有读取账号数据库、真实用户
文件或凭证；UI stop 后未封存 stop-state snapshot，Desktop 由 exact PID SIGTERM 收口，因此不把
正常 App exit 写作 r9 证据。完整 Provider、Skill/MCP、SSH、installed、升级/rollback、签名和
release-ready 均不外推。精确 identity、sub-gate、退出边界与 cleanup closure 见
[日期化 B-CONTEXT 验收](../../docs/evidence/investigations/2026-08-11-claude-science-0.1.25-b-context-01.md)。

`9e08924` exact tuple 的 `B-PROVIDER-01` run `provider-9e08924-r1` 在隔离 HOME/TMPDIR、
deny-egress sandbox、loopback strict fixture 与固定假凭证下完成 11-case 本地矩阵。10 个 case 由
exact App 启动 packaged Gateway；SiliconFlow 因 production Desktop 的 Gateway env allowlist 不转发
ambient HTTP proxy，改由同一 exact packaged Gateway 直启执行 hostname-preserving proxy fixture，
并明确不外推 Desktop 级 E2E。99 个 observation/event、55 个 request、64 个 active socket rows
全部通过；64 个 owned PID、88 个动态端口与 9 个临时路径最终清零，post-cleanup top closure
583/583 `OK`。真实 provider/model、账号、配额与服务质量仍为 `NOT-RUN`；精确 scope、identity、
SiliconFlow 边界与 cleanup 见
[日期化 B-PROVIDER 验收](../../docs/evidence/investigations/2026-08-11-claude-science-0.1.25-b-provider-01.md)。

此前 `06b630b` exact tuple 的完整 `B-RUNTIME-01` run `r06b630bb` 在全新隔离 HOME/data-dir、
deny-egress sandbox、真实 Science 0.1.25 与 loopback fake provider 下完成 normal production
wiring。56 条事件 0 failure，5/5 provider requests consumed；重开保持同一 Desktop/Gateway
owner，停止/重启产生新的 Gateway/Science owner，最终 8 个 exact PID、4 个动态端口、`8765`
与 runtime root 全部清零。49 项 evidence hash 全部复算 `OK`，总判定为
`PASS(scope=csswitch-gateway-science,loopback-provider-fixture)`；真实 provider/账号、Skill/MCP、
SSH、installed、签名和 release 不外推。该历史 tuple 的精确 identity 与 hash 只由本段固定，
不得链接或外推到当前 `a60c2ee` 调查。

`06b630b` exact tuple 的 `B-CORE-01` run `bcore-06b630b-r2` 只保留为历史 PASS；其 identity、DB
observation 与 23 项 evidence closure 不得继承到当前 `a60c2ee` artifact，也不再链接当前调查。

## Phase 5 one-click durable-journal owner

Phase 4 基线是 production source `139f6ee235b67284e2edc852521f24b3f501d467`。其后的
behavior-preserving Phase 5 owner 物理移动在
`8687e79bb85303c27a1f0d7137bf9dca87a482be` 完成：private
`runtime/sandbox_session/one_click/transaction.rs` 拥有 one-click durable-journal
identity/transition；root `one_click.rs` 保留 façade/coordinator、entry/recovery policy、
success-finalize replay/effect/read-model/failure glue。其 production/test-bearing final candidate
`f60a55ec1be3d101702c4b710ea8bfd3bd5be0d9` 修复 fixture、关闭该项 MEDIUM，不改变该 owner 边界；
本次 evidence-only descendant 再回填该 candidate 的 source-gate 状态。

`f60a55e` 的唯一 canonical run `0be6cd1e570a33cc5cfbdbad8a67a473` 在
`/private/tmp/csg.9RP1pi` runner exit 0：15/15 suites、15/15 observations `PASS`，
executed/passed/ignored/failed/skipped/not-run 为 `1480/1436/44/0/0/0`。completion seal SHA-256 为
`24a6d68971538e3e589d89b193fbfebfec2a80f3a3a8b67dd2cd1ae7650323b8`，evidence manifest 为
`d0119908c673ec972d745612ba07dbc67d9be0014b5184608027c870464d75e2`，run manifest 为
`14479ee89886e162c33edd08d55e00dda8ccbb8a625915754198d586c63f60b5`，source snapshot manifest 为
`0cfcd8a25c99b9d1691443c6fd6c1826f3fd087627d9c9c15b0c6f03599790eb`，input digest 为
`6e55b508df7c5c11f9409cae6d265a80db8d6a73f29c9b565aa3757044226831`。该 evidence root 保留，不得删除。

`8687e79` 的 `22038ed8087f38a0a0bcd20c61064ecb`（`/private/tmp/csg.VC5xLJ`）仍是 sealed
`FAIL` / RC 10 的历史 attempt：14/15 suites `PASS`、唯一
`SUITE-ORPHAN-SKILL-BOUNDARY` fixture drift 由漏列 `transaction.rs` 引起，现已由 `f60a55e` 修复，
不得混作 PASS。

`070bd0b4e223960263b91d8119a278bdc86f3678` 是回填上述证据的 evidence-only descendant；其唯一
canonical run `2247f8e529a24c9846c1040a176e634b`（`/private/tmp/csg.Y2ZbTS`）sealed `FAIL` / RC 12：
14/15 suites `PASS`，唯一 `SUITE-RUST-DESKTOP` 为 `INFRA_ERROR` / `ADAPTER_MALFORMED`，observation
reason 为 `TEST_IDENTITY_MISMATCH`。executed/passed/ignored/failed/skipped/not-run 为
`1480/1435/44/1/0/0`；唯一 failed ID 是
`desktop/src-tauri/Cargo.toml::lib::commands::runtime::tests::r0_one_click_db_restart_unproven_candidate_blocks_restore`。
completion seal 为
`756715cb68973d5d471a90b67fe9507db2f76e9adf4e30df299ab156c1b1a7ff`，evidence manifest 为
`dd14c0a4fea9e6136a809781f5b11e12cf7df2afbd7f7feaf5d1690aef9e9bc2`，run manifest 为
`330712c1be06b31509c64a515a8d1d77ca107f48cfab066a442b7a93fc53816d`，source snapshot manifest 为
`06ff544bbdab26531aa05bc84cf3903d7443979bcf958a08a794d41b8d3401e0`，input digest 为
`69f405934b2e819789090e0a4251254f3d24aa545472439e07a7274e310deafe`。执行方报告该 exact wrapper
随后的单次非 canonical focused diagnosis 为 `PASS`（1 passed、603 filtered、2.99s）；但该运行没有
canonical manifest 或 retained receipt，当前正文不能独立复核，只能作为单点未复现的诊断线索，不能把
`070bd0b` 改写为 source PASS，也不能替代 canonical seal。

`c9cf1e6a989663c8cc57ae9d837be003bed17144` 只删除失败后会受 authority rollback 影响的辅助
`serve` call-count 断言；存活 unbound PID、精确 authority mutation、blocked restore、无 listener / receipt
与 attributable cleanup 的强断言均保留，production source 未改变。该 clean exact SHA 的 canonical
run `7dee16001e4d3c7d7e5b51be212aee68`（`/private/tmp/p5g.VPWbWg`）runner exit 0、15/15 suites 与
15/15 observations 均为 `PASS`。completion seal、evidence manifest、run manifest、source snapshot
manifest 与 input digest 分别为 `0d162b0599b57d1526ded8b49e7a15d5571e6ff0cafd59844ed14d210839a6b3`、
`2cae27dd1d444cc1515d3deb493503dcf353ef8c33543aa91c786b5d87ca17a9`、
`03ca831b4f5400f458501c75e28c08676268ca61bf94a75fcc3ce3d1eeeb2cb8`、
`6daed8512c3cc417d15cc352adbe8986d7f85c7aea02bd7877abf35683a3b496` 与
`e4e087feb61aa138b4fc573819393b4df59aa253ba9b02a964e4976ced4789d6`。因此 `f60a55e` 与
`c9cf1e6` 分别只为各自 exact SHA 建立 `RUN-EVIDENCE-GREEN` / `SOURCE-GREEN`；`070bd0b` 保持 sealed
`FAIL`。本 evidence-only descendant 不继承任一历史 seal，其 exact 状态仍只由绑定该 SHA 的外部
canonical completion seal 判定。artifact、isolated-live、authorized-live、installed、Skill/MCP、SSH、
provider、signing/notarization 与 release 全部 `NOT-RUN`，且不继承。

## 当前源码问题

Provider compatibility slice 的 2026-08-11 code-bearing source 为
`next@e1832bd35e9384265df9911a42f841ed90c0f43c`；`0ecc7e2a105b2bb270d42f95a42148a6cd23bbca`
只补 test fixture 和过时 integration 断言，`2136803ced6a4c45a9ba6ce7910110fdf49ce053`
只补日期化 evidence。`0ecc7e2` 的 canonical run `903ebd2e365ac12274639fc676ca9388` 为
15/15 suites、15/15 observations、runner exit 0，completion seal SHA-256 为
`a7d31bb706ef8acb57e44abe640d3f24387cc76d7eb956fdad261c40f7d4f102`。该历史线之后先由
`18a67881` 形成完整 accepted tuple，再前进到 `9c91bec` canonical-gated source 与当前
`9476be5` production-code candidate。
旧 slice 曾重新回读
`6b1b999..e1832bd` production diff 与 live evidence，未发现漏提交的 tracked 源码；该回读不是
formal independent clean-context review。已安装 artifact / live 结论仍只绑定 `18a67881` 的独立
artifact receipt；SSH、signing 与 release 仍未建立。

- **Gateway 锁边界**：`06b630b` 已把 candidate log、命令与环境构造、Skill bridge 配置 staging、spawn 和 health poll 全部移出 `AppState`；锁内 reservation 与 generation + full candidate-owner CAS 守护接受，replacement 不被覆盖，不确定 child 由独立 registry 持有，destructive caller 对 typed uncertain stop fail closed。该 exact SHA 的 formal independent clean-context review 为 `PASS`（`BLOCK/HIGH/MEDIUM/LOW=0/0/0/0`），canonical 15-suite 为 15/15 `PASS`。
- **跨文件恢复边界**：history full-snapshot restore 已有 typed complete-record CAS、protected snapshot、唯一跨进程 effect owner 与 durable outcome；其他 sibling full-snapshot restore / multi-file crash boundary 尚未统一。
- **Science adoption record**：`18a6788` 已建立只含 allowlisted executable metadata 的 predecessor / candidate / normalized diff ledger，区分 `deferred_healthy`、`rejected` 与 `selected`，并把 managed receipt schema v2 绑定到 launch/finalize milestone；schema v1 只读兼容且 provenance unknown。对应 ChangeRecord、正式 source clean-context review 和 canonical 15-suite 均已 `PASS`；同 SHA 的独立 Acceptance G1 与 adoption G2 又建立 exact-artifact isolated-live scoped `PASS`，normal artifact 已 exact 安装并完成 installed/runtime 与真实 Provider 分项。Signing、公证与 release 仍未建立。
- **Science update observation**：`9c91bec` review 证明 background selection 曾直接消费 source probe 前克隆的 `science_runtime`，无法证明 daemon、receipt、listener 或 full owner 仍健康。`9476be5` 已把 pending publication 与 observation 解耦，并要求 lifecycle observed-context、managed-health proof、generation + full-owner CAS 以及 ledger writer-lock 内最终重验。完整 608-test Desktop Rust suite 已 `PASS`（567 passed / 41 approved ignored），frontend、metadata、inventory 与文档治理 checks 已 `PASS`；该修复已纳入 `d2cf95e` 的 fresh clean-context completion review 与 canonical 15-suite `PASS` source closure。
- **Rust source-gate hygiene candidate**：`76616ff` 已在不改变 product behavior / test identity 的边界内修复 Rust 1.96.1 warnings；`bash test/run-rust.sh` 为 `PASS`，Desktop 567 passed / 41 approved ignored、Gateway 290 library + 1 CLI integration passed，两套 fmt 与 `clippy --all-targets -- -D warnings` 全绿。绑定 `dfb6f59` + exact diff 的 fresh independent code / test / documentation candidate reviews 均为 `PASS`、四级 finding 全 0；包含其 docs-only Context descendant 的 exact `d2cf95e` completion review 与 canonical 15-suite GATE-SOURCE 也均为 `PASS`。
- **旧 Skill Manager negative refactor candidate**：当前 Phase 1 候选从 `da55374` 基线删除未注册、未编译的 `desktop/src-tauri/src/commands/skills.rs`（2,052 行）与 `desktop/src-tauri/src/skill_manager/`（13,485 行），并把 `test_skill_runtime_boundary.py` 的假 production path 替换为真实注册的 `commands/skill_install.rs`；current runtime inventory、S3 ChangeRecord 与仅绑定这两个精确 orphan 路径的 fail-closed deletion mapping 同步对齐。Skill boundary 16/16、quality kernel 16/16、runtime mutation inventory 5/5、quality metadata / Phase 0 target impact、Desktop clippy `-D warnings` 与双 workspace fmt 均为 `PASS`；PY offline 为 30 passed / 2 个 loopback `ENV-BLOCKED` skipped。完整 Desktop 首轮 608 项只因 canonical 前置 Gateway binary 尚未构建而得到 566 passed / 1 precondition failure / 41 approved ignored；按 canonical 顺序构建 Gateway 后，该唯一失败测试原样重跑 1/1 `PASS`。首轮 formal independent clean-context review 因 deletion + add/copy 组合仍可绕过 policy 而以 1 个 HIGH `FAIL`；第二轮发现累计 release diff 检查会误伤 Phase 1 后的合法 Desktop 变更；第三轮发现取最近新增 commit 可被 ChangeRecord delete/re-add 绕过，三轮均以 1 个 HIGH `FAIL`。候选现要求该 ChangeRecord 历史中恰好一次新增，并只对未提交 worktree 或唯一 introduction commit 冻结完整 Desktop manifest；伪装、缺失、累计 diff 与 delete/re-add 均有回归。修复后的 fresh review 与 clean exact-SHA canonical 15-suite 当前仍为 `NOT-RUN`，因此不建立 source、artifact、Skill runtime 或 MCP 能力结论。
- **Skill / MCP / Plugin 目标设计**：多包格式的 component-wise compatibility、Agent / deterministic host / Science ownership、inspect-plan-confirm-apply、effect ledger、MCP revision/profile/auth 与演进/验收不变量已在[扩展控制面架构合同](../../docs/architecture/skill-mcp-plugin-control-plane.md)冻结。当前 v1 bridge 行为不变；新 parser、operation ledger、Science/MCP adapter、exact artifact、`B-SKILL-01`、分 profile `B-MCP` 与真实服务均为 `NOT-RUN`。旧 Skill Manager 删除是独立 negative refactor，不得把死源码接入新控制面。
## 当前证据缺口

下表只记录 canonical mapping 中仍缺的层；production owner、caller、failure boundary 和 fixture 的唯一明细在[生产链路验收的决策映射](../../docs/operations/real-machine-acceptance.md#3-重要重构决策映射)。`source anchors mapped` 不等于 source seal。

| 重要重构决策 | Production source | Exact artifact | Isolated-live | Authorized live |
|---|---|---|---|---|
| Phase 5 one-click durable-journal owner | `8687e79` 已将 identity/transition owner 物理移至 private `one_click/transaction.rs`；`f60a55e` 修复 fixture，canonical `0be6cd1e…` 为 15/15 `PASS`。`8687e79` 的 `22038ed…` sealed `FAIL` 是已修复 fixture drift 的历史 attempt；`070bd0b` 的 `2247f8e…` 亦保持 sealed `FAIL` / RC 12。`c9cf1e6` 删除不稳定的辅助 serve-count 断言，production source 未变；其 clean exact-SHA canonical run `7dee1600…` 为 15/15 `PASS`，completion/evidence/run/source manifest 与 input digest 分别为 `0d162b05…` / `2cae27dd…` / `03ca831b…` / `6daed851…` / `e4e087fe…`，绑定保留的 `/private/tmp/p5g.VPWbWg`。本 evidence-only descendant 的 exact 状态不继承，仍只由后续绑定该 SHA 的 canonical seal 判定 | `NOT-RUN` | `NOT-RUN` | `NOT-RUN`；provider、installed App/runtime、Skill/MCP、SSH、signing、notarization 与 release 不继承 |
| 一键入口、Gateway / Science 启动与 finalize | `d2cf95e` fresh completion review + canonical 15-suite `PASS`；Gateway reservation / 锁外 spawn / full-owner CAS、rejected/uncertain child owner 与 destructive caller fail-closed 继续闭合 | 历史 `18a6788` Acceptance G1 与独立 normal artifact identity 均 `PASS`；当前 source candidate artifact `NOT-RUN` | 历史 `18a6788` adoption G2 `PASS(scope=science-adoption-isolated-live)`；当前 source candidate `NOT-RUN` | installed normal 历史 product path 已运行；Provider 分项见本表最后一行，非闭合项保持 INCONCLUSIVE / NOT-RUN |
| runtime mutation 与 stop ownership | stop_all、set_mode、set_settings、native exit 与 downgrade cleanup 保持既有 owner / 锁外 wait / CAS；cold prior、managed DB restart、history prior stop、live compensation 与 fresh-process replay cleanup 已统一为 transaction-scoped 完整 owner + exact request / 锁外 wait / generation + full-owner CAS；历史 profile-switch rollback owner 已随 dead writer 删除；`d2cf95e` review/gate `PASS` | 历史 `18a6788` Test G1 / normal identity `PASS`；当前 source candidate `NOT-RUN` | 历史 healthy deferred/cold selected/reopen/cleanup `PASS`；replacement/race/crash 由 source fixture 证明，当前 source candidate live `NOT-RUN` | installed normal 历史 Provider Stop/cleanup `PASS` |
| Science fixed active / pending update adoption | `9476be5` 已实现 managed-health proof + generation/full-owner CAS；该修复已纳入 `d2cf95e` 的 608-test Desktop Rust suite、frontend、metadata、inventory、fresh completion review 与 canonical 15-suite `PASS` source closure | `NOT-RUN`；不得继承 `18a6788` artifact | `NOT-RUN`；RM-21 必须按 active/pending 与 next-cold-start 合同重跑 | `NOT-RUN`；真实 updater/Science 需另行授权 |
| authority finalize、compensation 与 replay | durable step intent/effect/outcome、lease、crash/idempotence fixture 与共享 transaction stop executor 已映射；`d2cf95e` fresh completion review / canonical gate `PASS`；历史 `18a6788` 另闭合 receipt/action/binding/runtime provenance 与 live/fresh compensation artifact 语义 | 历史 `18a6788` G1 exact artifact `PASS`；当前 source candidate `NOT-RUN` | 历史 receipt v2 / action / binding / finalize / reopen exact association `PASS`；当前 source candidate `NOT-RUN` | 历史 installed happy path `PASS`；crash window仍不由 live happy path外推 |
| history full-snapshot recovery | history durable intent/effect/outcome、effect lease 与共享 transaction prior-stop executor 已映射；`a60c2ee` canonical 15-suite `PASS` | 同一 `a60c2ee` G1 exact artifact `PASS` | production IPC + synthetic history `NOT-RUN` | 真实用户历史不作默认 gate |
| Science host adapter 与 Skill host bridge | `a60c2ee` 闭合 acceptance host Gateway fixture 注入；comprehensive source review 未替代 Skill 专项能力审查 | 同一 `a60c2ee` G1 exact artifact `PASS`；只证明 bundle identity，不证明 Skill runtime | current tuple `B-SKILL-01=NOT-RUN`；旧 artifact 的安全停止/公共 GitHub 尝试只保留为历史问题证据 | 真实 Skill / domain execution分项 `NOT-RUN` |
| provider protocol capabilities | `e1832bd` Kimi server-search compatibility + targeted tests；`a60c2ee` protocol matrix 与 `d74221e` RM-46 保留各自历史 gate；当前 `d2cf95e` canonical 15-suite source closure `PASS` | installed Gateway identity `8a619b94…5540` 已固定；当前 source candidate artifact `NOT-RUN` | `a60c2ee` `B-PROVIDER-01` 与 `d74221e` RM-46 只保留历史 local-mock gate | installed live 历史结果：DeepSeek / SiliconFlow text + UI incremental + tools `PASS`；Qwen text + UI incremental `PASS`、tool `INCONCLUSIVE(400)`；Kimi text + UI incremental `PASS`，完整 tools / reasoning / native search `INCONCLUSIVE`；Xiaomi / Zhipu / MiniMax text + UI incremental `PASS`；OpenRouter `INCONCLUSIVE(quota_402)`；Codex / OpenCode 未发请求；stop/tab/runtime cleanup `PASS` |

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
回到 clean exact HEAD。因此该历史 tuple 的总判定为 `B-CORE-01=PASS`；当时
`B-CONTEXT-01` 的 B-CORE 前置已满足，
但不表示 B-CONTEXT 本身已运行。精确 sub-gate、身份、签名非目标和 cleanup 见
[日期化验证](../../docs/evidence/investigations/2026-08-08-claude-science-0.1.25-b-core-01.md)。

2026-08-08 的 `B-CONTEXT-01` 继续绑定 `9cc0d15` exact artifact / Science 0.1.25 tuple，
只验证 local surface、状态与 isolated request shape。plan approve/reject、delegation、fork/restore、
memory save/search、compaction、Reviewer/Specialist surface 全部取得独立观察；两个合成 project / 两个
target root session 的 717 个脱敏 request envelope 中，四项 cross-domain violation 均为 0。Reviewer
保持 `Inconclusive`，Reviewer/Specialist 服务端结果仍为 `UNVERIFIED`。活动期 non-loopback socket=0、
8765=0，hashed closing 进程/端口清零；最终修复后的 28-entry closure 已脱敏一次性 nonce，
并含 post-cleanup receipt 确认 runtime 与本轮两个临时 worktree/build/driver 已删除。因此该历史 tuple
`B-CONTEXT-01=PASS(scope=isolated-request-shape)`。global `About you` memory 的显式共享 surface
不外推为 project-scoped memory 的全部语义；精确状态、fixture loop 噪声、网络与 cleanup 见
[日期化验收](../../docs/evidence/investigations/2026-08-08-claude-science-0.1.25-b-context-01.md)。

2026-08-10 的新 `B-CONTEXT-01` 绑定同一个 `06b630b` G1 exact artifact / Science 0.1.25
tuple，没有重建 artifact。plan approve/reject、delegation、fork/restore、Memory save/search、
compaction、Reviewer/Specialist local surface 与两 project/两 target root session 隔离均取得
独立观察；140 个脱敏 request envelope 的四项 cross-domain violation 均为 0。Reviewer 保持
`Inconclusive`，Reviewer/Specialist 服务端结果为 `UNVERIFIED`。活动期 non-loopback=0、
8765=0；停止后 owned process/端口清零，合成 Memory 删除，runtime/source worktree/driver/pycache
精确清理。23-entry hash closure 已脱敏一次性 nonce 并纳入 post-cleanup receipt。因此该历史 tuple
`B-CONTEXT-01=PASS(scope=isolated-request-shape)`；真实账号/provider、Skill/MCP、SSH、
installed、签名与 release 均不在本轮范围。精确状态、身份、网络与 cleanup 见
[日期化验收](../../docs/evidence/investigations/2026-08-10-claude-science-0.1.25-b-context-01.md)。

2026-08-09 的 `B-SKILL-01` 绑定 `c4a1159` clean source gate 与同 SHA 新构建的 exact
`CSSwitch Test.app` / packaged Gateway / Science 0.1.25 tuple。Science 在用户对话前的 bundled
warmup 已尝试非预期外部 destination，立即触发该 probe 的严格停止条件；停止前只完成
identity、隔离根、fixture 与 deny-egress 护栏核对，六阶段均为 `NOT-RUN`。停止后继续的
host-access / managed tool 诊断属于程序偏差，已排除在正式判定之外。因此该历史 tuple 只能固定为
`INCONCLUSIVE(reason=safety-stop)`，不能由 source/artifact 邻层或停止后观察补绿。
四个专用端口和全部 attributable process 已清零；精确 identity、ledger、hash 与禁止绕过边界见
[日期化调查](../../docs/evidence/investigations/2026-08-09-claude-science-0.1.25-b-skill-01.md)。

先前 r1 的 evidence envelope 缺口、r2 的 no-opt-out outer 启动失败，以及 r3 的 exact-artifact
不匹配、permission fixture 误布置和 non-loopback safety-stop 均继续保留为日期化历史证据；它们
不能反推各自运行已 PASS，也不再覆盖 r12 对当时新 exact source/artifact 的判定。

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
固定为该轮 exact tuple；该 receipt 不能追溯替代 G2 的 pre-run manifest。授权的 G2 run 中，
production auto-boot 建立 exact Gateway/Science listener 与 managed receipt，UI stop-all 后
端口、PID、receipt 和 open FD 清零，随后删除 exact runtime root，8765 基线不变。完整 `B-RUNTIME-01` 的
open/reopen/restart 仍为 `NOT-RUN`；provider 协议、真实 provider/账号、Skill/SSH、installed、
签名、公证和 public release 也未运行。loopback provider 与 0 inference hits 只作为 observation 保留；
运行中观察到 Gateway 到 `198.18.0.54:443` 的禁止 non-loopback socket 后触发 safety-stop，加上
pre-run manifest/provider receipt 不完整，G2 总判定为 `INCONCLUSIVE(reason=safety-stop)`，不能写 PASS。
exact identity、授权与 cleanup 见
[日期化调查](../../docs/evidence/investigations/2026-08-07-claude-science-0.1.25-g2-start-stop.md)。
本段只陈述 `e7dfde1` 的历史 G1/G2 记录；当时的后续证据文档 HEAD 没有同源 artifact，
不能继承该轮 G1 PASS，也不覆盖上方 `6e09e68` 的当时映射。

2026-08-06 的 A0 artifact 绑定 `9cf75d19e7853b91b2f9a7c85afbd66747cb4fa3`，早于当前生产源码变化，只能从其[日期化 artifact 审计](../../docs/audits/2026-08-06-a0-frozen-baseline-artifact.md)读取限定结果，不能外推 current source、Desktop/Science live、provider、installed、签名或 release。2026-07-30 的 Science `B-RUNTIME-01` 同样只保留为绑定当时版本与身份门禁的[日期化调查](../../docs/evidence/investigations/2026-07-30-claude-science-0.1.25-b-runtime-01.md)，不再作为当前 probe gate。

## 产品与分发边界

- 第三方模型支持不能由一次文本聊天代替。stream、tools / `tool_choice`、reasoning、structured output、vision、stop / error semantics 必须按 provider、model 与 operation 分项授权和取证；当前能力 owner 见[产品 / Claude Science 能力地图](../../docs/features/product-science-capability-map.md)。
- 外部 Skill 的 content fetched、package committed、Science discovered、Agent attached、loaded / triggered、领域执行、restart persistence、quarantine 与 detached 是不同结论。CSSwitch 只拥有窄安装 / 投影桥，不拥有 Skill runtime 或通用 MCP 管理面。
- 系统 SSH 默认关闭；parser、OpenSSH invocation、真实 server connectivity 与 scheduler 是独立层，真实 host 必须逐项授权。当前合同见[系统 SSH 配置复用](../../docs/features/system-ssh.md)。
- Web Search、hosted MCP / Connectors、Reviewer entitlement、官方 catalog / usage 由 Anthropic 账号和服务拥有；第三方 Gateway 不模拟这些 entitlement。
- source、artifact、isolated-live、authorized live、installed、signing/notarization 与 public release 相互独立。缺少的层保持 `NOT-RUN` / `INCONCLUSIVE` / 未验证，不能借邻层补绿。
