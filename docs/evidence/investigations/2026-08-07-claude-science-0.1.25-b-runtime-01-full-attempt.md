# Claude Science 0.1.25 `B-RUNTIME-01` 完整 isolated-live 尝试

状态：已执行、已独立审查并安全清理；
`INCONCLUSIVE(reason=science-minimal-start-not-closed-and-reopen-process-ownership-unproven)`

适用范围：`next@d97be8735fb39a46ad6de2bb7babb5f263a165cd` 中由
`c531006595709ea1247d03932526901b056adf99` 提交的 isolated-live controller、
`e7dfde13636cbf3b377d01dbba3a2aee88e62822` 的 exact `CSSwitch Test.app`、
Claude Science 0.1.25 executable、全新隔离 HOME/data-dir、动态端口、
deterministic loopback provider 与 deny-egress sandbox。`d97be87` 只在既有
controller/source-gate 和 artifact tuple 之上增加文档，不把 artifact 改写为由该 HEAD 构建。

最后复核：2026-08-07（Asia/Taipei）

## 总判定

本轮不满足 [B-RUNTIME-01 合同](../../operations/science-probe-spec.md)
的 PASS，也没有足够归因写成产品 FAIL：

- 完整 pre-run identity、G1 binding、fixture/provider receipts 与 network isolation
  self-test 均通过。
- production Desktop 和 packaged Rust Gateway 建立 exact identity；Gateway health ready。
- 真实 Science 的 production launch 已执行，但没有建立目标 listener；允许保留的日志只证明
  `Science 启动命令失败` 和
  `compensation_restore_blocked_science_cleanup_unproven`，没有保留可把失败归因到产品、fixture、
  harness 或系统限制的 failure kind / exit status。
- reopen driver 产生了第二个 exact Desktop。Gateway PID 与 launch id 未变只是复用观察，不能证明
  reopen 成功，也不满足唯一进程归属。
- 初始最小 start 未闭合后停止继续升级 gate。产品 status、registered lifecycle stop 与 restart
  均为 `NOT-RUN`；后续 exact-PID 安全清理不能冒充产品 stop PASS。

因此总结果固定为
`INCONCLUSIVE(reason=science-minimal-start-not-closed-and-reopen-process-ownership-unproven)`。

## Sub-gate 结果

| Sub-gate | 结果 | 证据边界 |
|---|---|---|
| exact identity / G1 binding | `PASS` | frozen Desktop、Gateway、Science package/executable 与 artifact record 一致 |
| pre-run receipts | `PASS` | 完整 tree manifest、fixture/provider receipt、repo state 与 launch argv/env 在启动前冻结 |
| network isolation self-test | `PASS` | IPv4/IPv6 loopback 可用；非 loopback TCP/UDP、DNS transport 与 mDNSResponder IPC 均被阻断 |
| production Desktop | `PASS(scope=exact-launch-only)` | exact executable PID `32061`，controller owned process group |
| packaged Gateway | `PASS(scope=start-health-only)` | exact executable PID `32075`，`127.0.0.1:62574`，Rust/deepseek/off identity 与 health ready |
| Science minimal start | `INCONCLUSIVE` | production launch 已尝试；目标 `62575` listener 未建立，失败原因不可归属 |
| open / reopen | `INCONCLUSIVE` | 第二 Desktop PID `33670` 破坏唯一归属；Gateway PID/launch id 未变不构成 reopen PASS |
| status | `NOT-RUN` | 第二 Desktop 的 UI 状态不是首实例的可信 registered status evidence |
| product stop | `NOT-RUN` | exact-PID TERM 是安全清理，不是 `stop_all` gate |
| restart | `NOT-RUN` | 初始 start 与 reopen 未闭合后未继续 |
| provider protocol / inference | `NOT-RUN` | loopback mock 0 requests，protocol incomplete |
| cleanup | `PASS(scope=safety-cleanup)` | 已知归属 PID、四个动态端口、runtime-root open FD 与 8765 listener 均清零 |

## Exact identity 与 evidence

- controller SHA-256：
  `7320352e3fe930e41ec416c81740a7cd388fb169ebbcbfef10eec2e0cef854ae`。
- Desktop SHA-256：
  `1234a2b2df941224734517459089f913bb318bcf972eb590e69c9f6523083457`。
- packaged Gateway SHA-256：
  `edce6026b1339c11d0dd5a7240cec8284541ec955bbeb0d67bc8fad9c0693fa6`。
- Science executable SHA-256：
  `63b0f57aa3b9588ba9e61433d27c78df788f8fe2c1b51842db107d6697e9c03f`。
- pre-run manifest SHA-256：
  `80102680621446a6437dbf63f1b58553ef0d84849dfe00b469fb09644aa6cbf1`。
- network isolation receipt SHA-256：
  `19b8a6faf901b000bc0e0511cf0ae6e1d7183fc852cac252e18f750705d3acbd`。
- raw evidence root：`/private/tmp/brt/r095741-f25595/B-RUNTIME-01/`；
  `hashes.sha256` SHA-256 为
  `80e063fd0bf31536c520a36a62d794ee13eee159c995ee1eba03c05c91aca6bb`，
  24/24 entries `OK`。

独立 clean-context reviewer 对“当前证据支持上述受限 `INCONCLUSIVE` 裁定”给出 `PASS`，
`BLOCK=0`、`HIGH=0`、`MEDIUM=2`。两项 MEDIUM 正是 reopen 证据不足与 Science failure
不可归因，均已进入总判定而没有被降级或外推。

## 清理与重跑条件

两份脱敏 operation/sandbox 日志已逐字节复制进 raw evidence 并进入 hash closure。随后删除：

- exact runtime root `/private/tmp/csim.bsehqgqi`；
- 一次性 runner `/private/tmp/csswitch_b_runtime_01_runner.py`；
- 两个未启动 production App 的失败 preflight 目录；
- Test bundle 的临时 LaunchServices 注册。

删除后再次复核 raw evidence 24/24、目标 Desktop/Gateway 进程、动态端口
`62574/62575/62576/62592` 与保留端口 `8765`；均符合预期。

重跑前先修 harness/driver，而不是先假定产品缺陷：reopen 必须作用于现有 Desktop 并证明始终只有
一个 exact Desktop；harness 必须保留脱敏的 Science launch failure kind、exit/status 与最小归属。
只有修正 driver 后在有效 fixture 下仍稳定复现，才进入产品代码归因。

## 不能外推

- `B-RUNTIME-01` 不是 PASS；其他依赖它的 B probe 与全部 C probe 不得据此进入。
- 本轮不是产品 FAIL，也没有定位产品代码修复点。
- provider stream/tools/reasoning/error、真实 provider/账号、Skill、SSH、installed App、签名、
  公证、DMG 与 public release 均为 `NOT-RUN`。
