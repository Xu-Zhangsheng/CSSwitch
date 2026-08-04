# 当前已知问题与证据缺口

状态：当前；按 2026-08-04 F1-0 source-only closure 整理

最后复核：2026-08-04（Asia/Taipei）

失效条件：quality lineage、Doctor / one-click / history / config / boot read model、Science runtime provenance、release source、artifact、installed/live 或公开 Release 任一相关事实改变时，受影响条目立即失效并须实时重审。

本页只保留当前仍有效的问题、决策边界和证据链接。已完成 R0–R2、S1–S6、H1–H4、D0、Q0-A 与 F1-0 的原始结论保留在[日期化审计索引](../../docs/audits/README.md)，不在当前工作集重复。

## 当前唯一决策门

最近 implementation closure 是 [F1-0 history boundary guard](../../docs/audits/2026-08-04-f1-0-history-boundary-guard.md)：history restore 成功后 frontend 只保留 restore 结果并保持 stopped，不再自动串联第二次 `one_click_login`；用户必须再次显式点击「一键开始」。

F1-0 已在 source/unit 层完成：

- implementation candidate 是 `b731c84355f78d187831e53a5b1075deb576cc3f`；最终独立审查绑定与 commit diff 相同的 staged fingerprint，结论 `PASS`、`BLOCK/HIGH/MEDIUM/LOW = 0/0/0/0`；
- exact-SHA 15-suite `GATE-SOURCE` run `4f1ad659a1aeb767e43f7dabc5008a1b` 为 `PASS`；首次受限 sandbox run 的 loopback/子进程失败不用于 closure；
- backend history restore、失败路径与 DTO 未改；完整 F1 durable transaction、O1-A、C1、read model、artifact、installed/live、签名、公证和 public release 均未由此建立。

**当前没有获授权的 implementation sole NEXT。** 下一动作只做一次新的只读再基线，重新比较 O1-A、C1、完整 F1-A、F1-R、Science provenance 与 release qualification；不得沿用 Post-Q0 表格顺序自动进入任何阶段。

## 仍开放的源码与架构问题

- **Runtime MEDIUM**：F1 history 的 backend restore 仍跨 exact stop 与多文件 restore；用户随后可选的显式 one-click 是另一个 destructive operation，两者没有共同 durable journal。read model 仍由 `get_config` 隐式 ack notice，boot event / pull 没有统一 sequence。
- **Runtime MEDIUM**：O1-A entry decision 晚于 SSH / stub / pending cleanup；command / runtime entry owner、giant coordinator、config typed / cross-process boundary 仍未收敛。
- **Science update MEDIUM**：已有受校验的内容寻址 snapshot、managed identity / receipt、healthy defer 和 cross-runtime rollback guard，但没有通用 predecessor / candidate / adoption diff ledger。

Post-Q0 表格只保留 F1-0 前的日期化规划事实。F1-0 已改变其首阶段状态，后续范围、非目标与顺序必须重新比较；旧 R3–R11、S7、Post-D0/Post-Q0 表格或 ignored plan 都不能自动授权后续实现。

## 当前产品与 live 证据缺口

- 第三方模型支持不能由“文本聊天成功”代替。stream、tools / `tool_choice`、reasoning、structured output、vision、stop / error semantics 仍须按 provider 与 operation 分层验证；当前 v0.8.4 artifact 没有为所有 OpenCode Go、Grok、Gemini、Kimi、DeepSeek、custom relay 或 Codex 账号 / 模型建立 live PASS。当前能力边界见[产品 / Claude Science 能力地图](../../docs/features/product-science-capability-map.md)。
- Web Search、hosted MCP / Connectors、Reviewer entitlement、官方 catalog / usage 由 Anthropic 账号和服务拥有；第三方 Gateway 不模拟这些 entitlement。
- `B-RUNTIME-01` 因未取得允许的 Claude Science 0.1.25 executable identity 保持 `INCONCLUSIVE`，start / open / reopen / status / stop / restart 均为 `NOT-RUN`。这不是产品失败，也不能由静态或历史 release 证据替代；见[日期化 B-RUNTIME-01 证据](../../docs/evidence/investigations/2026-07-30-claude-science-0.1.25-b-runtime-01.md)。
- 外部 Skill install / attach、Science load / trigger、领域执行与重启持久化是不同结论。CSSwitch 只拥有窄安装 / 投影桥，不拥有 Skill runtime 或通用 MCP 管理面。
- 系统 SSH 默认关闭；opt-in 后 CSSwitch 只负责 preflight / stub / sidecar 边界。parser、OpenSSH invocation 与真实 server connectivity 必须分开；当前没有特定真实 SSH server 的 current live PASS。当前合同见[系统 SSH 配置复用](../../docs/features/system-ssh.md)。
- Codex 是默认关闭的实验窄桥。上游账号权限、动态目录与 Responses 协议会变；当前不支持设备码、多账号、代理认证、PAC、自定义 CA、系统代理自动发现或 TUN 检测。

## 分发与证据边界

- v0.8.4 公开附件是经过完整性验证的 ad-hoc seal；没有 Developer ID、notarization、stapled ticket 或 Gatekeeper acceptance。逐层事实只从 [v0.8.4 release evidence](../../docs/evidence/releases/v0.8.4.md) 进入。
- trusted `GATE-SOURCE` PASS 只证明 exact source / unit；文档治理测试也不能外推 artifact、installed/live、provider、signing 或 public release。
- 真机矩阵是应执行场景，不表示最终 DMG 已逐项执行。每次验收必须绑定 exact artifact / environment，并把 PASS、失败、阻断和未执行分开；执行合同见[真机验收](../../docs/operations/real-machine-acceptance.md)。
- Git / source、artifact、installed runtime、真实 provider、Science、SSH、签名、公证和公开 release 是相互独立的证据层；缺少的层继续记为 `NOT-RUN`、`INCONCLUSIVE` 或未验证，不以其他层补绿。
