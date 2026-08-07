# 当前已知问题与证据缺口

状态：当前；唯一验收路线以[生产链路验收](../../docs/operations/real-machine-acceptance.md)为准

最后复核：2026-08-07（Asia/Taipei）

失效条件：production owner / caller、确定性 fixture、候选 source、artifact identity、Science / Gateway runtime、provider capability、installed/runtime、签名或公开 Release 任一相关事实改变时，受影响条目立即失效并须实时重审。

本页只登记当前仍开放的问题和各证据层缺口，不保存另一份路线或验收合同。使用前必须实时核对 branch、HEAD、worktree、目标 artifact 与 runtime；日期化 audit/evidence 只证明其绑定的 SHA、artifact、版本和环境。

## 唯一当前路线

所有旧 R3–R11、R4/R5、S7、Post-D0/Post-Q0 及其他阶段编号路线均已退役，只能从[历史审计索引](../../docs/audits/README.md)查证当时的 source closure、取舍和证据边界。旧编号、旧 sole NEXT、旧计划顺序和已完成 source gate 都不能授权或替代新的实现、artifact 或 live 阶段。

新的唯一验收顺序是：**重要重构决策 → production source → exact artifact → isolated-live → authorized live**。当前映射、每层进入条件、授权边界和故障 fixture 边界只在[生产链路验收](../../docs/operations/real-machine-acceptance.md)维护；Science 运行细则见[Science 探针合同](../../docs/operations/science-probe-spec.md)。

2026-08-07 当前 source/test candidate 为
`next@c531006595709ea1247d03932526901b056adf99`。独立 clean clone 的固定 15-suite
source gate 为 `PASS`、runner exit `0`；该提交相对现有 `e7dfde1` exact artifact
没有 Desktop production source 变化，只提交 isolated-live controller 与测试，不能据此把 artifact
identity 改写为 `c531006`。

## 当前源码问题

- **Sibling stop owner / wait 边界**：`stop_all`、切换 official 的 `set_mode` 与 teardown `set_settings` 已使用 process-local owner claim、锁外 wait 与 identity CAS；downgrade cleanup、native-exit 及其他 sibling stop caller 尚未全部收敛到同一边界。当前 owner 与缺口见[运行时状态与事务](../../docs/architecture/runtime-state-transactions.md)。
- **Gateway 锁边界**：Gateway spawn 后 health poll 已在锁外，但 reuse health、旧进程清理与 spawn 仍在 `AppState` 锁内；后续只能按当前 owner 重新定义有界任务，不能恢复旧阶段编号。
- **跨文件恢复边界**：history full-snapshot restore 已有 typed complete-record CAS、protected snapshot、唯一跨进程 effect owner 与 durable outcome；其他 sibling full-snapshot restore / multi-file crash boundary 尚未统一。
- **Science adoption ledger**：已有受校验的内容寻址 snapshot、managed identity / receipt、healthy defer 和 cross-runtime rollback guard，但没有通用 predecessor / candidate / adoption diff ledger。

## 当前证据缺口

下表只记录 canonical mapping 中仍缺的层；production owner、caller、failure boundary 和 fixture 的唯一明细在[生产链路验收的决策映射](../../docs/operations/real-machine-acceptance.md#3-重要重构决策映射)。`source anchors mapped` 不等于 source seal。

| 重要重构决策 | Production source | Exact artifact | Isolated-live | Authorized live |
|---|---|---|---|---|
| 一键入口、Gateway / Science 启动与 finalize | source anchors mapped；`c531006` exact-SHA 15-suite source gate `PASS`；该提交无 Desktop production source 变化 | current `e7dfde1` exact tuple `PASS`；`c531006` controller 已完整冻结 pre-run manifest、fixture/provider/network receipts 与树 manifest；没有 `c531006` product artifact | 最新完整尝试为 `INCONCLUSIVE(reason=science-minimal-start-not-closed-and-reopen-process-ownership-unproven)`；Desktop/Gateway start-health 有限事实成立，Science minimal start 与 reopen 未闭合，status/产品 stop/restart `NOT-RUN` | Science/provider 分项 `NOT-RUN` |
| runtime mutation 与 stop ownership | source anchors mapped；replacement/race fixture 与 sibling gap 待 source seal | current `e7dfde1` exact tuple `PASS`；后续 HEAD 未绑定 | normal stop observation `PASS`；G2 总项未 PASS；replacement/race 不由 live 外推 | normal stop `NOT-RUN` |
| authority finalize、compensation 与 replay | source/compensation/replay fixture anchors mapped；fresh source seal 待执行 | current `e7dfde1` exact tuple `PASS`；后续 HEAD 未绑定 | normal binding/finalize observation `PASS`；G2 总项未 PASS；crash/compensation/replay 不由 live 外推 | happy path `NOT-RUN`；crash window 不要求 live |
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

随后执行的完整 `B-RUNTIME-01` 尝试复用了同一 exact artifact/Science tuple 与 hardened
controller。pre-run/G1/network receipts 通过；production Desktop 与 packaged Rust Gateway
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
本段和表格都只陈述 `e7dfde1`；证据文档提交产生的后续 HEAD 没有同源 artifact，不能继承 current G1 PASS。

2026-08-06 的 A0 artifact 绑定 `9cf75d19e7853b91b2f9a7c85afbd66747cb4fa3`，早于当前生产源码变化，只能从其[日期化 artifact 审计](../../docs/audits/2026-08-06-a0-frozen-baseline-artifact.md)读取限定结果，不能外推 current source、Desktop/Science live、provider、installed、签名或 release。2026-07-30 的 Science `B-RUNTIME-01` 同样只保留为绑定当时版本与身份门禁的[日期化调查](../../docs/evidence/investigations/2026-07-30-claude-science-0.1.25-b-runtime-01.md)，不再作为当前 probe gate。

## 产品与分发边界

- 第三方模型支持不能由一次文本聊天代替。stream、tools / `tool_choice`、reasoning、structured output、vision、stop / error semantics 必须按 provider、model 与 operation 分项授权和取证；当前能力 owner 见[产品 / Claude Science 能力地图](../../docs/features/product-science-capability-map.md)。
- 外部 Skill 的 content fetched、package committed、Science discovered、Agent attached、loaded / triggered、领域执行、restart persistence、quarantine 与 detached 是不同结论。CSSwitch 只拥有窄安装 / 投影桥，不拥有 Skill runtime 或通用 MCP 管理面。
- 系统 SSH 默认关闭；parser、OpenSSH invocation、真实 server connectivity 与 scheduler 是独立层，真实 host 必须逐项授权。当前合同见[系统 SSH 配置复用](../../docs/features/system-ssh.md)。
- Web Search、hosted MCP / Connectors、Reviewer entitlement、官方 catalog / usage 由 Anthropic 账号和服务拥有；第三方 Gateway 不模拟这些 entitlement。
- source、artifact、isolated-live、authorized live、installed、signing/notarization 与 public release 相互独立。缺少的层保持 `NOT-RUN` / `INCONCLUSIVE` / 未验证，不能借邻层补绿。
