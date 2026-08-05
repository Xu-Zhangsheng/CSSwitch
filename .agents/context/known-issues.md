# 当前已知问题与证据缺口

状态：当前；O1-E1 source-only closure 已完成，后续阶段尚未重新基线

最后复核：2026-08-05（Asia/Taipei）

失效条件：quality lineage、Doctor / one-click / history / config / boot read model、Science runtime provenance、release source、artifact、installed/live 或公开 Release 任一相关事实改变时，受影响条目立即失效并须实时重审。

本页只保留当前仍有效的问题、决策边界和证据链接。已完成 R0–R2、S1–S6、H1–H4、D0、Q0-A、F1-0、O1-A、C1-A、F1-A、O1-B、O1-C、O1-D、F1-R 与 O1-E1 的原始结论保留在[日期化审计索引](../../docs/audits/README.md)，不在当前工作集重复。

## 当前 source candidate

最近已完成的 implementation closure 是 [O1-E1 durable compensation journal foundation](../../docs/audits/2026-08-05-o1-e1-durable-compensation-journal.md)。implementation candidate 为 `7d4ffb51359bbd6ceb8dd67a58bef4fe63c5c594`；它已通过 fresh clean-context independent review 与 exact-SHA 15-suite `GATE-SOURCE`，但本节和该审计都不能外推 artifact、installed/live 或 release 结论。

O1-E1 当前源码合同：

- 独立、path-free 的 `RuntimeCompensationJournal` V1 在首个 one-click compensation effect 前以完整 business record CAS 发布 aggregate intent；`PreJournalAbort` 与 `Journaled` 走同一 fail-closed 边界；
- authority restore 精确恢复 compensation 前的 `runtime_transaction` 并只叠加 marker；完整成功才清 marker，失败则持久化 canonical typed failed steps；
- one-click 与 normal mode/settings/profile/Codex auth/settings/downgrade mutation 在任一 journal 打开时 fail closed；只减小运行态暴露且不写 config/credential/journal 的 terminal cleanup 保持可用；
- 最终 fresh clean-context reviewer 以 `PASS`、`BLOCK/HIGH/MEDIUM = 0/0/0` 封口；run `ffa14f373a988c26687a1dff52fab7e0` 的 exact-SHA 15-suite gate 为 PASS；
- 该候选不证明 artifact、installed/live、真实 provider/Science/SSH、签名、公证或 release。

O1-E1 已完成，但没有自动继承的新 implementation sole NEXT。选择或实施后续阶段前，必须按实时源码重新比较仍开放问题、依赖与非目标。

## 仍开放的源码与架构问题

- **Runtime MEDIUM**：O1-E1 已把 path-free aggregate compensation begin/final outcome 与原 business transaction 分离并持久化，normal mutations 对开放 marker fail closed；每个 compensation effect 前后的 stepwise intent/outcome、fresh-process replay、自动收敛，以及其他 sibling full-snapshot restore / multi-file crash boundary 仍未统一。
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
