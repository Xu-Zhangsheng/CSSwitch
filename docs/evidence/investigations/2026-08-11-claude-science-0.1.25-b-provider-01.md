# Claude Science 0.1.25 `B-PROVIDER-01` exact-artifact 本地 Provider 验收

日期：2026-08-11（Asia/Taipei）

状态：`PASS(scope=exact-artifact-local-mock,mixed-launch-boundary)`

失效条件：source、G1 artifact、packaged Gateway、Provider contract/catalog、Science 版本、fixture、验收驱动或 case 执行边界任一变化时，本结论失效。

## 结论

`next@9e08924481c8f5edb181254332d94daba0cbe4b2` 的 G1-bound `CSSwitch Test.app` 与 packaged Rust Gateway 已完成 `B-PROVIDER-01` 本地严格矩阵：11 个 case、99 个 observation/event、55 个 fixture request 全部 `PASS`。10 个 case 由 exact App 启动 packaged Gateway；SiliconFlow 因生产 Desktop 对 Gateway 执行环境 allowlist、不会转发 ambient HTTP proxy，只能用同一 exact packaged Gateway 直启并注入 hostname-preserving loopback proxy fixture。后者只证明 packaged Gateway 的 SiliconFlow contract、catalog、request transform、reuse/restart 与清理，不写成 Desktop 级 end-to-end PASS。

本轮没有访问真实 provider、真实账号或外网。所有 provider credential 都是固定假值，只由产品进程正常消费；request body 未进入 evidence，配置正文也未进入汇总或报告。真实 Desktop → SiliconFlow/provider API、真实模型能力、配额与服务质量继续留到最终 authorized-live 阶段。

## Exact identity 与前置绑定

- exact source：`9e08924481c8f5edb181254332d94daba0cbe4b2`；独立 source worktree 收尾仍为 clean detached HEAD。
- artifact：`/private/tmp/g1.9e08924.91acQY/CSSwitch Test.app`。
- G1 binding receipt SHA-256：`39aa3dc5866807140d42409ab8eea2f6625fa4d9f7212f06482dcc0db0c9b762`。
- Desktop SHA-256：`ada760cb9dcfdd9b2151d652ff744f300a914b3bef8c07ea85ce886994458a6b`。
- packaged Gateway SHA-256：`bfa05512337329f52811c2d7c08081ed2249a34c6ed98dd7fe7d83313d9b3036`。
- 前置 canonical closure：G1、`B-RUNTIME-01`、`B-CORE-01`、`B-CONTEXT-01` 的 hash index 均复制进本轮 `bindings/`，并与各自 authority byte-identical。
- canonical run：`provider-9e08924-r1`；evidence root：`/private/tmp/csp9e1/B-PROVIDER-01`。

每个 exact App case 重新核对完整 G1 tuple，但本轮启动的 Science 仅是隔离 fake lifecycle fixture；它不替代 G1 的真实 Science identity，也不把 fake Science 行为写成产品能力。

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

每项都完成 local Gateway `/v1/models`、scratch、formal、连续 reuse、fresh isolated HOME cold restart、active network capture、config path-only stability、log sensitive-pattern count、exact PID stop 与 runtime-root 删除。`/v1/models` 是 Gateway 静态目录观察，没有伪装成 upstream discovery。

`siliconflow` 使用 `execution_scope=exact-packaged-gateway-direct-local-mock`：

- packaged Gateway executable/hash 与 G1 一致；
- managed `anthropic-relay` contract digest、static catalog fingerprint、launch ID 与 health identity 一致；
- loopback HTTP proxy 保留 `api.siliconflow.cn` endpoint identity，使 exact-host compatibility rule 在不访问外网的条件下实际执行；
- scratch、formal、reuse 与 fresh-HOME cold restart 共消费 5 个严格 fixture request；
- production Desktop ambient proxy fixture 明确记录为 fail-closed，真实 Desktop/provider E2E 标记为 `deferred-to-final-authorized-live-stage`。

## 复算与完整门禁

- 11/11 case decision `PASS`；每项 9 个 observation、9 个严格单调 event，共 99/99。
- 每项 primary 4 个、restart 1 个严格 request，共 55；fixture 全部 `protocol_complete=true`、`final_ok=true`、failure 为空，actual request evidence 不含 `body` 字段。
- active socket rows 共 64：全部 loopback，non-loopback 为 0，端口 8765 未使用。
- 64 个 evidence-owned PID 当前均不存在；88 个记录的动态端口当前均关闭。
- 10 个 exact App case 各有 52-entry case closure；SiliconFlow 为 40-entry case closure，均逐项 `shasum -a 256 -c` 通过。
- post-run cleanup 与最小 validation receipt 纳入后，top closure 为 583/583；`hashes.sha256` 自身 SHA-256 为 `68109b64b7717461dd1ec66d2481c56684a10f72ffb8f056291b7ee8127bfebb`。
- preserved driver SHA-256 为 `a0d1bb42725ff2df3f77cffcb4284d9fa0f30c9e5ac0c21e6d744092bf45f243`；收口前与工作区副本 byte-identical。
- 本轮执行者的最小 validation receipt 记录：完整 Provider loopback gate 在隔离 HOME、HOME 外独立 TMPDIR、offline Cargo 与公共 crate cache 下重建当前 Gateway，终端观察为 `111/111` tests `OK`、`S0_LAYER loopback pass`；完整 stdout/stderr 因未预先证明 credential-free 而没有封存。首次门禁把 TMPDIR 错放在隔离 HOME 内，controller 按合同拒绝该测试 root；修正环境后从头完整重跑通过。clean-context reviewer 在 managed sandbox 独立复跑时因 loopback bind `EPERM` 得到 `ENV-BLOCKED`，因此没有独立重建 111-test PASS；两次环境结果均不计作产品失败。

## Cleanup

每个 case 的 primary/restart 两个 runtime root（共 22 个）均在各 case 收尾删除，matrix runtime parent `/private/tmp/csp9r1` 最终不存在。工作区临时 driver、controller/test/validation HOME、Python cache、隔离 Cargo home 与 test/validation TMPDIR 共 9 个精确归属路径在确认无打开文件后删除；`post-run-cleanup.json` 已纳入最终 top hash closure。exact source、G1 artifact、四层前置 canonical evidence 与本轮 canonical evidence 保留。

## 不能外推

本项不证明：

- 任何真实 Key、OAuth、Keychain、账号数据库或真实 provider/model 请求；
- SiliconFlow 的生产 Desktop 本地 mock E2E，或任何 provider 的真实服务可用性、配额、计费和输出质量；
- Science Skill/MCP、SSH server、installed App、升级、rollback；
- Developer ID 签名、公证、Gatekeeper、DMG 或 release-ready。
