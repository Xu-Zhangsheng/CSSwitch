# Claude Science 0.1.25 `B-RUNTIME-01` exact-artifact isolated-live 验收

状态：`PASS(scope=csswitch-gateway-science,loopback-provider-fixture)`

适用范围：`next@9e08924481c8f5edb181254332d94daba0cbe4b2`、由该 exact source 全新构建并经 G1 receipt 绑定的 `CSSwitch Test.app` 0.8.4、packaged Rust Gateway、Claude Science 0.1.25 executable，以及本轮全新隔离 HOME/data-dir、固定假凭据和动态 loopback provider fixture。

最后复核：2026-08-11（Asia/Taipei）

失效条件：artifact-producing source、CSSwitch bundle/Desktop/Gateway、Science package/executable、Acceptance fixture、network policy 或 `B-RUNTIME-01` 合同任一相关身份或行为变化时，本结论不得外推。

## 结论

唯一 canonical run `r9e08924b` 从 G1 冻结的 exact `CSSwitch Test.app` production executable 启动，经 registered Tauri UI/IPC、packaged Rust Gateway 与真实 Claude Science 0.1.25，完成一键开始、loopback provider 请求、LaunchServices 单实例重开复用、产品停止、产品重启、再次请求、最终产品停止、App 退出与精确清理。55 条事件严格单调；controller overall elapsed 为 `291.892949s`，未超过冻结的 `300s` overall deadline。总判定为 `PASS`。

## Exact identity 与前置

| Identity | Exact value | 判定 |
|---|---|---|
| artifact-producing source | `9e08924481c8f5edb181254332d94daba0cbe4b2` | `PASS` |
| source gate run | `078d462c81abcec146644c8096254069`；15/15 suites、runner exit `0` | `PASS` |
| source completion seal SHA-256 | `1c071a18701ac6e6a191c6dfc3cd1513d3465bce90d3567a6460cd46f36d0fe8` | `PASS` |
| G1 binding receipt SHA-256 | `39aa3dc5866807140d42409ab8eea2f6625fa4d9f7212f06482dcc0db0c9b762` | `PASS` |
| CSSwitch bundle canonical digest | `d77cb799f2063241041cc8d17bd57a3c72f49cc7b0da6899d63a52795df1ac64` | `PASS` |
| Desktop SHA-256 | `ada760cb9dcfdd9b2151d652ff744f300a914b3bef8c07ea85ce886994458a6b` | `PASS` |
| packaged Gateway SHA-256 | `bfa05512337329f52811c2d7c08081ed2249a34c6ed98dd7fe7d83313d9b3036` | `PASS` |
| Science version / executable SHA-256 | `0.1.25` / `63b0f57aa3b9588ba9e61433d27c78df788f8fe2c1b51842db107d6697e9c03f` | `PASS` |

执行使用 clean detached `9e08924` source worktree 核验 repository identity；运行 artifact 直接来自 G1 root `/private/tmp/g1.9e08924.91acQY/`，没有重新构建或替换 `/Applications/CSSwitch.app`。后续 evidence-only 文档提交不改写被构建或运行的 source/artifact identity。

## Lifecycle 观察

| 阶段 | Exact observation | 结果 |
|---|---|---|
| production Desktop | PID `14554`，由 LaunchServices guarded launch 启动 exact Desktop | `PASS` |
| 首次 Gateway / Science | Gateway PID `17610`、`launch_id=de3841b4cebaf82c02ff0ea646a758d1`；Science PID `17634` | `PASS` |
| 一键开始与首轮 provider | Gateway `57138`、Science `57139` ready；scratch/formal 各一次 | `PASS` |
| 单实例重开 | Desktop PID 集合前后均为 `{14554}`；primary identity 保留且无第二常驻 App；Gateway PID 与 `launch_id` 不变 | `PASS` |
| 复用 provider | 两次请求命中同一 Gateway owner | `PASS` |
| 第一次产品停止 | Gateway/Science listener 均关闭，配置无漂移 | `PASS` |
| 产品重启 | Gateway PID `23964`、`launch_id=9dfcff22fc88f5b9b12b32fa7c7948ee`；Science PID `24013` | `PASS` |
| 重启后 provider | 一次请求命中新 Gateway owner | `PASS` |
| 最终停止与退出 | Gateway/Science listener 关闭，Test App 退出 | `PASS` |

loopback mock 共收到 5 个预期请求，5/5 consumed，`protocol_complete=true`、`queue_complete=true`、`failures=[]`，owned mock process exit `0`。首次、复用与重启阶段的 Gateway executable 均为同一 packaged Rust sidecar；重开保持相同 PID/launch id，重启产生不同 PID/launch id。两代 Science 均使用同一隔离 data-dir 与冻结的 executable identity。

## 隔离、网络与清理

运行根为 `/private/tmp/csswitch-science-probe-runtime/r9e08924b/home/`，证据根为 `/private/tmp/csswitch-science-probe-evidence/r9e08924b/B-RUNTIME-01/`。全部 fixture 使用固定假凭据和合成请求；没有读取或回显真实 API key、OAuth、Keychain、SSH 私钥、账号数据库、真实 `~/.claude-science` 或用户配置。

network policy 为 `deny-egress-except-ip-loopback-and-owned-science-unix-control`。pre-run self-test 证明 IPv4/IPv6 loopback 可用，并以 `EPERM` 阻断 IPv4/IPv6 非 loopback TCP/UDP、DNS TCP/UDP 与 system resolver IPC；唯一系统 resolver 查询失败。运行使用的动态端口为 Gateway `57138`、Science `57139`、preview `57140`、provider mock `57159`，fixture 未使用 `8765`。

runner cleanup receipt 证明 launcher、Desktop、两代 Gateway 与 provider mock 共 5 个 tracked PID 均已退出；四个动态端口全部关闭，runtime root 无 open PID。随后 post-cleanup 删除 canonical runtime root，并删除此前复用 `r9e08924a` 的 8 组无效 attempt runtime/evidence 临时目录；LaunchServices sandbox initializer 的 source/dylib 按 hash 复制进 canonical evidence 后，其旧临时 root 也已删除。receipt 复核 Test App 不在应用列表、动态端口和 8765 均无 listener，同时只保留 G1 root、exact source worktree 与 canonical evidence root。40 个日志/证据文件的敏感材料扫描为 0 match，Python Gateway tripwire 未触发。

最终 hash closure 包含 51 个 evidence 文件，`shasum -a 256 -c hashes.sha256` 为 51/51 `OK`；`hashes.sha256` 自身 SHA-256 为 `db03502a1de87e465b9e64af89c7981a6a3b0e4b2f7ec4bb527ac4380d2c393f`，post-cleanup receipt SHA-256 为 `e288fb5e6f33b0a6cc146a86f47e51fd1e2294a819a8f868d7f094161bc2625a`，initializer source/dylib SHA-256 分别为 `c67d313fd9d8109ced200bd5cdac8b70ff9349f3078322e1011f5d0eac6cef26` / `9bf1cea699d1a696ba2a4aa37754bd32cc9b89b004e6fefc7643242d1592bb3f`，network isolation receipt SHA-256 为 `123d48097cebac04cf3c4dccf346e7ef53e797a4b57cb5b1ec76a5fdf8eb6ada`，runner cleanup receipt SHA-256 为 `e61f2bca8ce1543be747cb8f7e335a7e4aac59d1e54c5e511a93f5ad388a50ed`。同一 exact source 的聚焦 `test.test_installed_provider_matrix` 在允许 loopback/Unix socket 的本机环境中为 33/33 `OK`；受限 sandbox 的先行尝试只有 9 个可运行用例通过，其余 24 个统一因 socket `EPERM` 阻断，不计为产品失败。

## 不能外推

本记录只证明该 exact tuple 的 normal production wiring 与 lifecycle，以及 DeepSeek-off basic loopback request shape。它不证明完整 Provider 能力矩阵、真实 provider/账号、真实凭据、`B-CORE-01`、`B-CONTEXT-01`、Skill/MCP 领域执行、SSH server、installed CSSwitch、签名、公证、Gatekeeper、DMG、tag、push 或 public release，也不把一次 happy path 外推为 crash、race、replacement、compensation、replay 或 CAS drift；这些仍由各自确定性 fixture 或单独授权的 probe 证明。
