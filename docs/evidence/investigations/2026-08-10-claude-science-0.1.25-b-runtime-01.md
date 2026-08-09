# Claude Science 0.1.25 `B-RUNTIME-01` exact-artifact isolated-live 验收

状态：`PASS(scope=csswitch-gateway-science,loopback-provider-fixture)`

适用范围：`next@06b630bb3e3fb0d63425bf48d1ffc2d3613fd992`、由该 exact source 构建并经 G1 receipt 绑定的 `CSSwitch Test.app`、packaged Rust Gateway、Claude Science 0.1.25 executable，以及本轮全新隔离 HOME/data-dir、固定假凭据和动态 loopback provider fixture。

最后复核：2026-08-10（Asia/Taipei）

失效条件：artifact-producing source、CSSwitch bundle/Desktop/Gateway、Science package/executable、Acceptance fixture、network policy 或 `B-RUNTIME-01` 合同任一相关身份或行为变化时，本结论不得外推。

## 结论

`r06b630bb` 从 exact `CSSwitch Test.app` 的 production executable 启动，经 registered Tauri UI/IPC、packaged Rust Gateway 与真实 Claude Science 0.1.25，完成一键开始、loopback provider 请求、LaunchServices 单实例重开复用、产品停止、产品重启、再次请求、最终产品停止、App 退出与精确清理。56 条事件全部成功并严格单调；从 `start_mock` 到 evidence finalize 的 elapsed 为 `248.138954s`，未超过冻结的 `300s` overall deadline。总判定为 `PASS`。

## Exact identity 与前置

| Identity | Exact value | 判定 |
|---|---|---|
| artifact-producing source | `06b630bb3e3fb0d63425bf48d1ffc2d3613fd992` | `PASS` |
| source gate run | `8728828e41ee0dc8ab578856995fa530`；15/15 suites、runner exit `0` | `PASS` |
| source completion seal SHA-256 | `e1ef35f06d0f88a8cbf6fa4ea8b12c129afd10b55f85f89d2074b0d23d72b455` | `PASS` |
| G1 binding receipt SHA-256 | `f6490b1765d982c4453571676cb3561f6f1c3a20a9af3850d30f8e405e795573` | `PASS` |
| CSSwitch bundle canonical digest | `634c13f2597c10cbbf75a7cac8d1af135523eccff2cb86373824695cccb32e1a` | `PASS` |
| Desktop SHA-256 | `cf0e84e6b33b761767394b6d5f3579e310bec5c07de8015cd79f2d407d9d5274` | `PASS` |
| packaged Gateway SHA-256 | `ed4dae8ec8139c4828dd0915d1594d69001e7b504c582a710e905707ef9d03d1` | `PASS` |
| Science version / executable SHA-256 | `0.1.25` / `63b0f57aa3b9588ba9e61433d27c78df788f8fe2c1b51842db107d6697e9c03f` | `PASS` |

执行使用 clean detached `06b630b` source worktree 核验 repository identity；运行 artifact 直接来自保留的 G1 root `/private/tmp/g1.06b630b.Yv5hDZ/`，没有重新构建或替换 `/Applications/CSSwitch.app`。后续 evidence-only 文档提交不改写被构建或运行的 source/artifact identity。

## Lifecycle 观察

| 阶段 | Exact observation | 结果 |
|---|---|---|
| production Desktop | PID `58032`，由 LaunchServices guarded launch 启动 exact Desktop | `PASS` |
| 首次 Gateway / Science | Gateway PID `61395`、`launch_id=778addb1425c8e94aea3bc569b4038b3`；Science PID `61419` | `PASS` |
| 一键开始与首轮 provider | Gateway `58908`、Science `58909` ready；scratch/formal 各一次 | `PASS` |
| 单实例重开 | Desktop PID 集合前后均为 `{58032}`；Gateway PID 与 `launch_id` 不变 | `PASS` |
| 复用 provider | 两次请求命中同一 Gateway owner | `PASS` |
| 第一次产品停止 | Gateway/Science listener 均关闭，配置无漂移 | `PASS` |
| 产品重启 | Gateway PID `69653`、`launch_id=834ddcc94456188c0a52c57bd7dc3a94`；Science PID `69702` | `PASS` |
| 重启后 provider | 一次请求命中新 Gateway owner | `PASS` |
| 最终停止与退出 | Gateway/Science listener 关闭，Test App 退出 | `PASS` |

loopback mock 共收到 5 个预期请求，5/5 consumed，`protocol_complete=true`、`queue_complete=true`、`failures=[]`，owned mock process exit `0`。首次、复用与重启阶段的 Gateway executable 均为同一 packaged Rust sidecar；重开保持相同 PID/launch id，重启产生不同 PID/launch id。两代 Science 均使用同一隔离 data-dir 与冻结的 executable identity。

## 隔离、网络与清理

运行根为 `/private/tmp/csswitch-science-probe-runtime/r06b630bb/home/`，证据根为 `/private/tmp/csswitch-science-probe-evidence/r06b630bb/B-RUNTIME-01/`。全部 fixture 使用固定假凭据和合成请求；没有读取或回显真实 API key、OAuth、Keychain、SSH 私钥、账号数据库、真实 `~/.claude-science` 或用户配置。

network policy 为 `deny-egress-except-ip-loopback-and-owned-science-unix-control`。pre-run self-test 证明 IPv4/IPv6 loopback 可用，并以 `EPERM` 阻断 IPv4/IPv6 非 loopback TCP/UDP、DNS TCP/UDP 与 system resolver IPC；唯一系统 resolver 查询失败。运行使用的动态端口为 Gateway `58908`、Science `58909`、preview `58910`、provider mock `58931`，fixture 未使用 `8765`。

最终外部清零收据绑定 controller、launcher、Desktop、mock、两代 Gateway 与两代 Science 共 8 个 exact PID：全部不存在；四个动态端口全部关闭；`8765` 前后均无 listener；runtime root 已删除。40 个日志/证据文件的敏感材料扫描为 0 match，Python Gateway tripwire 未触发。

最终 hash closure 包含 49 个 evidence 文件，`shasum -a 256 -c hashes.sha256` 为 49/49 `OK`；`hashes.sha256` 自身 SHA-256 为 `4f130e0106bafc8f4064f58bae7c51803bc53e68cee3e0371bb9a974756ba1b7`，network isolation receipt SHA-256 为 `28ff7df29c7ffa60f85d97b6fda24fe9f9f6c4a87446cc23399a41c113ecfaf3`，external cleanup receipt SHA-256 为 `0030d26b00d884f1b8a82fbf95ffbfbdbded12305dc4a848c231c2fdf6f7f7b7`。

## 不能外推

本记录只证明该 exact tuple 的 normal production wiring 与 lifecycle。它不证明真实 provider/账号、真实凭据、Skill/MCP 领域执行、SSH server、installed CSSwitch、签名、公证、Gatekeeper、DMG、tag、push、public release，也不把一次 happy path 外推为 crash、race、replacement、compensation、replay 或 CAS drift；这些仍由各自确定性 fixture 或单独授权的 probe 证明。
