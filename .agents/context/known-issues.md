# 当前已知问题与证据缺口

状态：当前；按 v0.8.4 release source 与 2026-07-30 文档治理基线整理

最后复核：2026-07-30（Asia/Shanghai）

失效条件：对应 change/bug record、Science 版本、release source、artifact 或 installed/live 证据改变时，受影响条目立即失效并须按当前版本重审。

已解决历史放入 CHANGELOG 或 dated evidence，不在这里重复。

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
- `sandbox_session` 第一层目录化拆分已在 `next` 完成；第二层收口评估、验证和
  Gateway `server.rs` 的后续顺序统一见
  [工程重构进度](refactor-progress.md)。任何后续拆分都不得扩大已冻结的
  Runtime/Gateway allowlist，子模块须返回 typed failure。
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
