# Claude Science 0.1.25 `B-RUNTIME-01` `a60c2ee` exact-artifact isolated-live 验收

状态：`PASS(scope=csswitch-gateway-science,loopback-provider-fixture)`

适用范围：`next@a60c2ee656429903f1fd8f398dc6ad8194aa9346`、由该 exact source 全新构建并经 G1 receipt 绑定的 `CSSwitch Test.app` 0.8.4、packaged Rust Gateway、Claude Science 0.1.25 executable，以及本轮全新隔离 HOME/data-dir、固定假凭据和动态 loopback provider fixture。

最后复核：2026-08-11（Asia/Taipei）

失效条件：artifact-producing source、CSSwitch bundle/Desktop/Gateway、Science package/executable、Acceptance fixture、network policy 或 `B-RUNTIME-01` 合同任一相关身份或行为变化时，本结论不得外推。

## 结论

本轮按 [Science 生产链路探针合同](../../operations/science-probe-spec.md)执行；exact source、artifact 与 G1 identity 的上游记录见 [`a60c2ee` exact-artifact 调查](2026-08-11-csswitch-a60c2ee-exact-artifact.md)。

唯一 canonical run `ra60c2eec` 从 G1 冻结的 exact `CSSwitch Test.app` production executable 启动，经 registered Tauri UI/IPC、packaged Rust Gateway 与真实 Claude Science 0.1.25，完成一键开始、loopback provider 请求、LaunchServices 单实例重开复用、产品停止、产品重启、再次请求、最终产品停止、App 退出与精确清理。55 条事件严格单调；controller overall elapsed 为 `298.825957s`，未超过冻结的 `300s` overall deadline。总判定为 `PASS`。

本轮 focused `test.test_installed_provider_matrix` 在独立 HOME/TMPDIR 下为 33/33 `OK`。正式 clean-context 独立审查复算 identity、hash、事件、network、lifecycle 与 cleanup 后取得 `BLOCKER/HIGH/MEDIUM/LOW=0/0/0/0`、`PASS`。

此前 `ra60c2eea` 与 `ra60c2eeb` 均因控制器 I/O 等待累计超过 300 秒而按合同安全停止，未合成 PASS，也未复用 run-id；其 PID、端口、runtime、evidence、initializer 与 cache 根已精确清理。它们不是 canonical run，也不构成产品行为 FAIL。

## Exact identity 与前置

| Identity | Exact value | 判定 |
|---|---|---|
| artifact-producing source | `a60c2ee656429903f1fd8f398dc6ad8194aa9346` | `PASS` |
| source gate run | `6e5124c43c1ceadac3bd54f6408e2141`；15/15 suites、runner exit `0` | `PASS` |
| source completion seal SHA-256 | `fe8d24939dd8b950c95984383645aa8de87dae4956c5c34c9a533b310560d86e` | `PASS` |
| G1 binding receipt SHA-256 | `b01cf7a2066576dfc5da1a59eada2faad9f586100f1b27d013cad9b4d420ddbb` | `PASS` |
| CSSwitch bundle canonical digest | `52cd48c06b2d2bffdd6d8e85d464be063e4604c71961a5cc8459879938a673c9` | `PASS` |
| Desktop SHA-256 | `77b4ebefe6e658c4f8f90e2b5177db5398be75eacf3b3f0d3364269eba5e9be8` | `PASS` |
| packaged Gateway SHA-256 | `610c0206973ec0281b6e34e171e4a6fb9899b31050642a073c4f0d1ca64887ab` | `PASS` |
| Science version / executable SHA-256 | `0.1.25` / `63b0f57aa3b9588ba9e61433d27c78df788f8fe2c1b51842db107d6697e9c03f` | `PASS` |

执行使用 clean detached `a60c2ee` source worktree 核验 repository identity；运行 artifact 直接来自 G1 root `/private/tmp/g1.a60c2ee.q4uEQl/`，没有重新构建或替换 `/Applications/CSSwitch.app`。后续 evidence-only 文档提交不改写被构建或运行的 source/artifact identity。

## Lifecycle 观察

| 阶段 | Exact observation | 结果 |
|---|---|---|
| production Desktop | PID `11095`，由 LaunchServices guarded launch 启动 exact Desktop | `PASS` |
| 首次 Gateway / Science | Gateway PID `13732`、`launch_id=5d4c273df4c913cbf2c36d25713ce62b`；Science PID `13758` | `PASS` |
| 一键开始与首轮 provider | Gateway `65174`、Science `65175` ready；scratch/formal 各一次 | `PASS` |
| 单实例重开 | Desktop PID 集合前后均为 `{11095}`；primary identity 保留且无第二常驻 App；Gateway PID 与 `launch_id` 不变 | `PASS` |
| 复用 provider | 两次请求命中同一 Gateway owner | `PASS` |
| 第一次产品停止 | Gateway/Science listener 均关闭，配置无漂移 | `PASS` |
| 产品重启 | Gateway PID `19248`、`launch_id=130761387e1decf15ca526c057d73100`；Science PID `19297` | `PASS` |
| 重启后 provider | 一次请求命中新 Gateway owner | `PASS` |
| 最终停止与退出 | Gateway/Science listener 关闭，Test App 退出 | `PASS` |

loopback mock 共收到 5 个预期请求，5/5 consumed，`protocol_complete=true`、`queue_complete=true`、`failures=[]`，owned mock process exit `0`。首次、复用与重启阶段的 Gateway executable 均为同一 packaged Rust sidecar；重开保持相同 PID/launch id，重启产生不同 PID/launch id。两代 Science 均使用同一隔离 data-dir 与冻结的 executable identity。

## 隔离、网络与清理

运行根为 `/private/tmp/csswitch-science-probe-runtime/ra60c2eec/home/`，证据根为 `/private/tmp/csswitch-science-probe-evidence/ra60c2eec/B-RUNTIME-01/`。全部 fixture 使用固定假凭据和合成请求；没有读取或回显真实 API key、OAuth、Keychain、SSH 私钥、账号数据库、真实 `~/.claude-science` 或用户配置。

network policy 为 `deny-egress-except-ip-loopback-and-owned-science-unix-control`。pre-run self-test 证明 IPv4/IPv6 loopback 可用，并以 `EPERM` 阻断 IPv4/IPv6 非 loopback TCP/UDP、DNS TCP/UDP 与 system resolver IPC；唯一系统 resolver 查询失败。动态端口为 Gateway `65174`、Science `65175`、preview `65176`、provider mock `65196`，fixture 未使用 `8765`。

runner cleanup receipt 证明 launcher、Desktop、两代 Gateway 与 provider mock 的五个 tracked PID 均已退出，四个动态端口全部关闭，runtime root 无 open PID；Science 两代 identity 与运行/停止端口观察也已闭合。随后把本轮 LaunchServices initializer source/dylib 按 hash 复制进 canonical evidence，并删除 runtime、initializer 与 Python cache 临时根。post-cleanup 再次复核当前及两个失败 run 的根均不存在、Test App 不在运行、动态端口与 `8765` 均无 listener。40 个日志/证据文件的敏感材料扫描为 0 match，Python Gateway tripwire 未触发。

最终 hash closure 包含 51 个 evidence 文件，`shasum -a 256 -c hashes.sha256` 为 51/51 `OK`；`hashes.sha256` 自身 SHA-256 为 `f94bc670e5857a227c4cb8939659d4eb05bcaaf2f82e0e80b868ec0e964306ea`。post-cleanup receipt、network isolation receipt、runner cleanup receipt SHA-256 分别为 `54318febf5a6a859517c8d86658bc0fcd2cc0c5af880b83fcc5b19e9263a1f1e`、`8f00a1eff708c3b8bfdb6310a2391a1e150d81c5bb307921ff2bddf3fb52f876`、`96ef6f992b72ae0b5f6eb6872112d42af044e50f08c268dbcfc657f208f9bc7e`。initializer source/dylib SHA-256 分别为 `c67d313fd9d8109ced200bd5cdac8b70ff9349f3078322e1011f5d0eac6cef26` / `ea67208442a1b8ced8687691d3c3d55a5f7765b4adcb9d52be5d58e1fea8dfac`。

## 不能外推

本记录只证明该 exact tuple 的 normal production wiring 与 lifecycle，以及 DeepSeek-off basic loopback request shape。它不证明完整 Provider 能力矩阵、真实 provider/账号、真实凭证、`B-CORE-01`、`B-CONTEXT-01`、Skill/MCP 领域执行、SSH server、installed CSSwitch、升级/rollback、签名、公证、Gatekeeper、DMG、tag、push 或 public release，也不把一次 happy path 外推为 crash、race、replacement、compensation、replay 或 CAS drift；这些仍由各自确定性 fixture 或单独授权的 probe 证明。
