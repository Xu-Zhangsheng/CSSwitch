# Claude Science 0.1.25 G2 隔离启动/停止链调查

状态：已执行并安全停止；G2 / 完整 `B-RUNTIME-01`
`INCONCLUSIVE(reason=safety-stop:unexpected-non-loopback-egress)`

适用范围：`next@e7dfde13636cbf3b377d01dbba3a2aee88e62822` 生成的
`CSSwitch Test.app`、固定的 Claude Science 0.1.25 executable、全新隔离 HOME、
动态 loopback Gateway / Science 端口和 deterministic loopback provider。只验证
CSSwitch production auto-boot 的 start → status → stop/cleanup；不发送 prompt。

最后复核：2026-08-07（Asia/Taipei）

## 判定

| Sub-gate | 结果 | 观察 |
|---|---|---|
| current G1 exact identity | `PASS` | run 后只读复核生成 `g1-binding-receipt.json`，唯一固定 source、artifact record、CSSwitch bundle/executables 与 Science package/executable；它是当前 G1 receipt，但不能追溯补成 G2 的 pre-run receipt |
| G2 pre-run freeze | `INCONCLUSIVE` | pre-run manifest 未完整固定 UTC/执行者、dirty/staged、fixture hash/path 和 provider launch receipt；`identity-hashes.json` 明确把 before phase 记为 `INCONCLUSIVE` |
| production start | `PASS` | `runtime-inventory.json` 证明 exact Desktop PID `79338` parent packaged Gateway PID `79350`；真实 Science PID `79375` 建立受管 listener；启动 21 秒，小于 60 秒 deadline |
| status / identity | `PASS` | `status-response.json` 为 `running=true` 且 PID/port 与 `managed-receipt.json`、listener inventory 一致；`ui-observation.json` 的 Gateway、Science、upstream 均为运行正常 |
| loopback provider wiring | `PASS(scope=observation-only)` | `fixture-config-input.json` + `runtime-binding.json` 记录 schema v3 内容与 v4 loopback binding；provider 为 loopback、0 hits 且退出，但 pre-run launch/ownership receipt 缺失，不能单独提升 G2 |
| production stop | `PASS` | `ui-observation.json` 明确“已停止代理与沙箱”；`inventory-after.json` 证明归属 PID、四个 listener 与 receipt 清零；停止 36 秒，小于 60 秒 deadline |
| cleanup | `PASS` | `cleanup.json` / `inventory-final.json` 证明 runtime root 在无 open FD 后已删除；evidence root 保留；8765 空基线保持不变 |
| network isolation | `INCONCLUSIVE(reason=safety-stop)` | 运行中观察到 Gateway 到 `198.18.0.54:443` 的非 loopback socket；按 probe 合同立即进入 safety-stop，不推断 hostname/用途，也不能合成 G2 PASS |
| open / reopen | `NOT-RUN` | 不在本次授权的启动/停止 slice 内 |
| restart | `NOT-RUN` | 不在本次授权的启动/停止 slice 内 |

本次独立取得 exact start、status、loopback provider observation、stop 与 cleanup 子项事实；
但 non-loopback egress 触发全局 safety-stop，且 pre-run manifest/fixture/provider receipt 不完整，
所以只能写
`ISOLATED-LIVE(scope=csswitch-gateway-science,start-status-stop,loopback-provider-fixture)=INCONCLUSIVE(reason=safety-stop)`。
open/reopen（不产生第二 daemon）与 restart（保持 executable / data-dir）仍为 `NOT-RUN`。

## Exact identity

- source：`next@e7dfde13636cbf3b377d01dbba3a2aee88e62822`；0 staged；仅有本任务的日期化
  evidence/context 文档编辑，没有 source、test 或 artifact 变化。
- current G1 receipt：`g1-binding-receipt.json`，文件 SHA-256
  `583465f8111b3961c04919f1e67cf2699c78b6e7b297ccacf29e7dc0db2200e9`；该 receipt 在 run 后签发，只建立当前 exact tuple，不追溯补齐 G2 准入。
- artifact record SHA-256：
  `6932897b69752dc1754b4998b37a430aefc4e38d27087f6ee4b1d7ea32931a68`。
- `CSSwitch Test.app` canonical bundle digest：
  `61fe9fda9561aa908dcd446b8b391ec5079ea4c713af0d925abbfcce036f1b8e`。
- Desktop SHA-256：
  `1234a2b2df941224734517459089f913bb318bcf972eb590e69c9f6523083457`。
- packaged Gateway SHA-256：
  `edce6026b1339c11d0dd5a7240cec8284541ec955bbeb0d67bc8fad9c0693fa6`。
- Science package canonical digest：
  `371a10ecb2e4fc610ede1e3ab266c3dfc71d3894d9bcb9ce35515145d93c49ca`。
- Science executable：
  `/Applications/Claude Science.app/Contents/Resources/bin/claude-science`；
  version `0.1.25`；arm64；SHA-256
  `63b0f57aa3b9588ba9e61433d27c78df788f8fe2c1b51842db107d6697e9c03f`。

上述 tuple 同时固定 artifact record、CSSwitch executable/Gateway 与 Science package/runtime。
任一字节或 source identity 变化都会使本结果失效。artifact 的当前签名事实另存于
`signing-observation.json`：ad-hoc、无 Team ID、strict verify rc `1`；这里只固定事实，
不建立 signing acceptance。公证、Gatekeeper、installed CSSwitch 与 public release 不在本轮范围。

## Runtime 与安全边界

- runtime root：
  `/private/tmp/csswitch-science-probe-runtime/g2-e7dfde1-evidence-whUvwVAJ/`；外层 HOME、
  `.csswitch-acceptance` 与 Science data-dir 全部位于该 root；证据捕获后已精确删除。
- Desktop / Gateway：PID `79338` / `79350`，exact packaged executable；Gateway listener
  `127.0.0.1:60940`。
- Science：PID `79375`，exact frozen executable，listeners
  `127.0.0.1:60941` / `[::1]:60941`，preview `127.0.0.1:60942`。
- loopback provider：PID `79288`，listener `127.0.0.1:61319`；0 hits；停止后 PID 与
  listener 均不存在。
- 8765 基线与 final snapshot 均为空，SHA-256
  `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。
- fixture 只含合成 profile、假 key 与假本地身份；命令没有读取账号、OAuth/API token、
  Keychain 内容、SSH、账号数据库、私人日志或用户文档。证据中的本机用户名、真实 HOME 前缀、
  isolated org nonce 与 launch nonce 已替换为固定占位符。
- `process-contract-observation.json` 固定实际 argv 与脱敏 env；Desktop、Gateway、Science
  与 local bridge 的 HOME/cwd 都在 runtime root，运行中 `lsof` 未观察到 `<REAL_HOME>/`
  打开文件。Science 启动 argv 保持 `--no-auto-update`。
- provider 绑定和推理入口均为 loopback，且没有 prompt / inference hit；但 Gateway 同时出现
  到 `198.18.0.54:443` 的禁止 non-loopback connection。04:56:16Z 观察后于 04:56:33Z
  触发 UI stop，04:57:18Z 完成 exact cleanup；该 safety-stop 阻止 G2 PASS。
- `deadline.json` 固定 overall 300 秒、start/stop 各 60 秒；实际 start 21 秒、stop 36 秒、
  runtime root 删除完成于 109 秒，全部在界内。

## Evidence 与清理

- evidence root：
  `/private/tmp/csswitch-science-probe-evidence/g2-e7dfde1-evidence-whUvwVAJ/B-RUNTIME-01/`
- `hashes.sha256` SHA-256：
  `f04e340a43fe1fe6ac83ed665b5c923645d734c5677d6b06ce76b97849940f84`。
- `hashes.sha256 -c`：20/20 `OK`。
- observations 的每个 assertion 都含具体 evidence pointer；运行中 inventory、managed receipt、
  脱敏 status、实际 argv/env/open-file/socket、runtime binding、UI、provider observation/hits、deadline、
  签名事实、before/after/final inventory 与 cleanup 分文件保存。
- exact probe runtime root 已删除；evidence root 保留，0 residual。

## 不能外推

- 完整 `B-RUNTIME-01` open/reopen/restart：`NOT-RUN`。
- 该 run 的 G2 / `ISOLATED-LIVE` 总判定：`INCONCLUSIVE(reason=safety-stop)`，不能写 PASS。
- prompt、stream、tools、reasoning、模型或 provider 协议能力：`NOT-RUN`。
- 真实 provider、账号、组织、Skill 领域执行、SSH server：`NOT-RUN`。
- signing、notarization、Gatekeeper、installed App、DMG、public release：`NOT-RUN`。
