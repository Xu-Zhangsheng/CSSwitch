# Claude Science 0.1.25 `a60c2ee` `B-PROVIDER-01` exact-artifact 本地 Provider 验收

日期：2026-08-11（Asia/Taipei）

状态：`PASS(scope=exact-artifact-local-mock,mixed-launch-boundary)`

失效条件：source、G1 artifact、packaged Gateway、Provider contract/catalog、Science 版本、fixture、
验收驱动或 case 执行边界任一变化时，本结论失效。

## 结论

`next@a60c2ee656429903f1fd8f398dc6ad8194aa9346` 的 G1-bound `CSSwitch Test.app` 与
packaged Rust Gateway 已完成 `B-PROVIDER-01` 本地严格矩阵：11 个 case、99 个
observation/event、55 个 fixture request 全部 `PASS`。10 个 case 由 exact App 启动 packaged
Gateway；SiliconFlow 因 production Desktop 的 Gateway env allowlist 不转发 ambient HTTP proxy，
继续使用同一 exact packaged Gateway 直启并注入 hostname-preserving loopback proxy fixture。
后者只证明 packaged Gateway 的 SiliconFlow contract、catalog、request transform、reuse/restart
与清理，不写成 Desktop 级 end-to-end PASS。

本轮没有访问真实 provider、真实账号或外网。所有 provider credential 都是固定假值，只由产品
进程正常消费；request body 未进入 evidence，配置正文也未进入汇总或报告。真实 Desktop →
SiliconFlow/provider API、真实模型能力、配额、计费与服务质量继续留到逐项授权的 authorized-live
阶段。

本矩阵也不替代 [RM-46](../../operations/real-machine-acceptance.md) 的新 Provider 配置 UX：
OpenCode Go 双协议、Grok 与 Gemini 的 scratch discovery、显式选择/手填和 production save
仍须作为独立 artifact 子门验证，不能由 Gateway 协议矩阵外推。

## Exact identity 与前置绑定

- exact source：`a60c2ee656429903f1fd8f398dc6ad8194aa9346`；复核时独立 source worktree 仍为 clean detached HEAD。
- artifact：`/private/tmp/g1.a60c2ee.q4uEQl/CSSwitch Test.app`。
- G1 binding receipt SHA-256：`b01cf7a2066576dfc5da1a59eada2faad9f586100f1b27d013cad9b4d420ddbb`。
- Desktop SHA-256：`77b4ebefe6e658c4f8f90e2b5177db5398be75eacf3b3f0d3364269eba5e9be8`。
- packaged Gateway SHA-256：`610c0206973ec0281b6e34e171e4a6fb9899b31050642a073c4f0d1ca64887ab`。
- Claude Science：0.1.25；package/executable identity 由同一 G1 receipt 固定。
- 前置 canonical closure：G1、`B-RUNTIME-01`、`B-CORE-01`、`B-CONTEXT-01` 的 hash index
  均复制进本轮 `bindings/`。
- canonical run：`provider-a60c2ee-r1`；evidence root：
  `/private/tmp/cspa60e1/B-PROVIDER-01`。
- preserved driver SHA-256：
  `7032233c3e51d5f36ee3ec4a96d245f4ea093d1d4ecb4fdac93c6ec1e1b406f2`。

每个 exact App case 都重新核对完整 G1 tuple。本轮启动的 Science 只属于隔离 fake lifecycle
fixture；它不替代 G1 的真实 Science identity，也不把 fake Science 行为写成产品能力。

## Case 与执行边界

exact App + packaged Gateway 的 10 个 case：

- `custom-chat`
- `deepseek-detect`
- `deepseek-off`
- `deepseek-rewrite`
- `kimi`
- `qwen-chat`
- `qwen-stream`
- `qwen-tools`
- `relay-force`
- `responses`

每项都完成 local Gateway `/v1/models`、scratch、formal、连续 reuse、fresh isolated HOME cold
restart、active network capture、config path-only stability、log sensitive-pattern count、exact PID stop
与 runtime-root 删除。这里的 `/v1/models` 是 Gateway 静态目录观察，没有伪装成 upstream
discovery。

`siliconflow` 使用 `execution_scope=exact-packaged-gateway-direct-local-mock`：

- packaged Gateway executable/hash 与 G1 一致；
- managed `anthropic-relay` contract digest、static catalog fingerprint、launch ID 与 health identity 一致；
- loopback HTTP proxy 保留 `api.siliconflow.cn` endpoint identity，使 exact-host compatibility rule
  在不访问外网的条件下实际执行；
- scratch、formal、reuse 与 fresh-HOME cold restart 共消费 5 个严格 fixture request；
- production Desktop ambient proxy fixture 继续 fail closed，真实 Desktop/provider E2E 为
  `deferred-to-final-authorized-live-stage`。

## 复算与完整门禁

- 11/11 case decision `PASS`；每项 9 个 observation、9 个严格单调 event，共 99/99。
- 每项 primary 4 个、restart 1 个严格 request，共 55；fixture 全部
  `protocol_complete=true`、`final_ok=true`、failure 为空，actual request evidence 不含 `body` 字段。
- active socket rows 共 64：全部 loopback，non-loopback 为 0，端口 8765 未使用。
- validation receipt 记录 84 个 owned PID observation 均已退出、64 个记录 listener port 均关闭、
  22 个 runtime root 均不存在。
- 10 个 exact App case 各有 52-entry case closure；SiliconFlow 为 40-entry case closure，
  均逐项 `shasum -a 256 -c` 通过。
- post-run cleanup 与 validation receipt 纳入后，top closure 为 583/583；`hashes.sha256` 自身
  SHA-256 为 `427960aef54798a3e95ffa435537ea414d464036161e83a3a12ea005b6a71f28`。
- 完整 Provider loopback gate 在隔离 HOME、HOME 外独立 TMPDIR、offline Cargo 与公共 crate
  cache 下从头重跑，结果为 `111/111` tests `OK`、`S0_LAYER loopback pass`。首次 fresh Cargo
  home 漏配公共 rsproxy sparse-index mapping，执行前判为 `ENV-SETUP-FAILED`；修正后完整重跑
  通过，不计作产品失败。
- 当前状态更新前又递归复算 top closure 与 11 份 case closure，全部返回 `OK`；exact source
  worktree 仍 clean，G1 artifact 与 receipt hash 未漂移。

## Cleanup

每个 case 的 primary/restart runtime root（共 22 个）均在 case 收尾删除，matrix runtime parent
`/private/tmp/cspa60r1` 最终不存在。验证过程可精确归属的临时 driver、HOME、TMPDIR 与 Cargo
root 已删除；post-run cleanup receipt 已纳入最终 top hash closure。exact source、G1 artifact、
四层前置 canonical evidence 与本轮 canonical evidence 保留。

## 不能外推

本项不证明：

- 任何真实 Key、OAuth、Keychain、账号数据库或真实 provider/model 请求；
- SiliconFlow 的 production Desktop 本地 mock E2E；
- OpenCode Go、Grok、Gemini 的 RM-46 scratch-discovery / selection / save 配置 UX；
- 任何 provider 的真实服务可用性、配额、计费、输出质量和实际 stream/tools/reasoning/error；
- Science Skill/MCP、SSH server、installed App、升级、rollback；
- Developer ID 签名、公证、Gatekeeper、DMG 或 release-ready。
