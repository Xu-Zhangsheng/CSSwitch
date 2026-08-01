# 当前已知问题与证据缺口

状态：当前；按 v0.8.4 release source 与 2026-08-01 R0 分片基线整理

最后复核：2026-08-01（Asia/Taipei）

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

R0 已全部收口。后续 R1-R11 或其他重构必须另行明确目标、基线与证据边界，
不得把本次 source closure 外推为产品修复或更高证据层 PASS。

## 下一轮重构的 P0 前置

- ~~Ambient environment 泄漏~~ **已闭合（source）**：`runtime/launch_env.rs`
  对 launch/stop script 与 Gateway 执行 `env_clear` + allowlist；
  `scripts/launch-virtual-sandbox.sh` 以 `env -i` 启动 Science。sentinel 与
  stub 回归：`cargo test --lib launch_env` / `proxy_lifecycle`、
  `bash test/test_launch_science_env_allowlist.sh`。
- ~~Typed failure projection~~ **已闭合（source，stage）**：`runtime/failure.rs`
  的 `OneClickFailureKind` 在产生点标注；一键与 auto-boot 投影到冻结 coarse
  stage；生产路径不再用 `science_failure_stage` 扫文案。journal checkpoint 仍为
  string。**过渡残留**：`recovery_status` 仍可从 message 内诊断码解析
  （`recovery_from_diagnostic_codes`），应在后续补偿点完全 typed。验证：
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
