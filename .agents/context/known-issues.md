# 当前已知问题与证据缺口

状态：当前；唯一验收路线以[生产链路验收](../../docs/operations/real-machine-acceptance.md)为准

最后复核：2026-08-08（Asia/Taipei）

失效条件：production owner / caller、确定性 fixture、候选 source、artifact identity、Science / Gateway runtime、provider capability、installed/runtime、签名或公开 Release 任一相关事实改变时，受影响条目立即失效并须实时重审。

本页只登记当前仍开放的问题和各证据层缺口，不保存另一份路线或验收合同。使用前必须实时核对 branch、HEAD、worktree、目标 artifact 与 runtime；日期化 audit/evidence 只证明其绑定的 SHA、artifact、版本和环境。

## 唯一当前路线

所有旧 R3–R11、R4/R5、S7、Post-D0/Post-Q0 及其他阶段编号路线均已退役，只能从[历史审计索引](../../docs/audits/README.md)查证当时的 source closure、取舍和证据边界。旧编号、旧 sole NEXT、旧计划顺序和已完成 source gate 都不能授权或替代新的实现、artifact 或 live 阶段。

新的唯一验收顺序是：**重要重构决策 → production source → exact artifact → isolated-live → authorized live**。当前映射、每层进入条件、授权边界和故障 fixture 边界只在[生产链路验收](../../docs/operations/real-machine-acceptance.md)维护；Science 运行细则见[Science 探针合同](../../docs/operations/science-probe-spec.md)。

2026-08-08 当前已验收 source/artifact binding 为
`next@9cc0d15d457c911047585c5fb7302702e26f4e43`。独立 detached exact worktree 的固定
15-suite source gate 为 `PASS`（15/15 suites、15/15 observations）；随后由该 SHA 构建并经 G1
receipt 绑定的 `CSSwitch Test.app`、packaged Rust Gateway 与 Claude Science 0.1.25 exact tuple
重新完成 `B-RUNTIME-01=PASS`，并在同一 tuple 下完成新的 `B-CORE-01=PASS`。后续文档提交只
记录证据，不能改写被构建或运行的 source/artifact identity。

## 当前源码问题

- **Sibling stop owner / wait 边界**：`stop_all`、切换 official 的 `set_mode` 与 teardown `set_settings` 已使用 process-local owner claim、锁外 wait 与 identity CAS；downgrade cleanup、native-exit 及其他 sibling stop caller 尚未全部收敛到同一边界。当前 owner 与缺口见[运行时状态与事务](../../docs/architecture/runtime-state-transactions.md)。
- **Gateway 锁边界**：Gateway spawn 后 health poll 已在锁外，但 reuse health、旧进程清理与 spawn 仍在 `AppState` 锁内；后续只能按当前 owner 重新定义有界任务，不能恢复旧阶段编号。
- **跨文件恢复边界**：history full-snapshot restore 已有 typed complete-record CAS、protected snapshot、唯一跨进程 effect owner 与 durable outcome；其他 sibling full-snapshot restore / multi-file crash boundary 尚未统一。
- **Science adoption ledger**：已有受校验的内容寻址 snapshot、managed identity / receipt、healthy defer 和 cross-runtime rollback guard，但没有通用 predecessor / candidate / adoption diff ledger。

## 当前证据缺口

下表只记录 canonical mapping 中仍缺的层；production owner、caller、failure boundary 和 fixture 的唯一明细在[生产链路验收的决策映射](../../docs/operations/real-machine-acceptance.md#3-重要重构决策映射)。`source anchors mapped` 不等于 source seal。

| 重要重构决策 | Production source | Exact artifact | Isolated-live | Authorized live |
|---|---|---|---|---|
| 一键入口、Gateway / Science 启动与 finalize | source anchors mapped；`9cc0d15` exact-SHA 15-suite source gate `PASS` | `9cc0d15` 的 `CSSwitch Test.app`、packaged Rust Gateway 与 Science 0.1.25 exact tuple 已绑定 | `B-RUNTIME-01=PASS`：一键开始、provider request、单实例重开复用、产品停止/重启、再次请求、最终停止与清理均闭合 | 真实 provider/账号分项 `NOT-RUN` |
| runtime mutation 与 stop ownership | source anchors mapped；replacement/race fixture 与 sibling gap 待 source seal | `9cc0d15` exact artifact 已由 `B-RUNTIME-01` 绑定；本行专项 artifact gate 未单独执行 | normal stop/restart observation `PASS`；replacement/race 不由 live 外推 | normal stop `NOT-RUN` |
| authority finalize、compensation 与 replay | source/compensation/replay fixture anchors mapped；fresh source seal 待执行 | `9cc0d15` exact artifact 已由 `B-RUNTIME-01` 绑定；本行专项 artifact gate 未单独执行 | normal binding/finalize observation `PASS`；crash/compensation/replay 不由 live 外推 | happy path `NOT-RUN`；crash window 不要求 live |
| history full-snapshot recovery | source anchors mapped；fresh source seal 待执行 | `NOT-RUN` | production IPC + synthetic history `NOT-RUN` | 真实用户历史不作默认 gate |
| Science host adapter 与 Skill host bridge | source anchors mapped；fresh source seal 待执行 | `NOT-RUN` | fixture install → attach → Agent load / trigger → restart persistence `NOT-RUN` | 真实 Skill / domain execution 分项 `NOT-RUN` |
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
