# Claude Science 0.1.25 `B-RUNTIME-01` 完整 isolated-live 验收

状态：`PASS`；最新 exact rerun 已把 `9cc0d15` 的源码门禁、同一 artifact、真实已安装 Science、
loopback fake provider、单实例重开、停止/重启、完整网络观察与清理重新闭合。

适用范围：`next@9cc0d15d457c911047585c5fb7302702e26f4e43`、由该 SHA 构建并经 G1 receipt
绑定的 `CSSwitch Test.app`、`/Applications/Claude Science.app` 0.1.25 的真实 executable、
全新隔离 HOME/data-dir、固定 fake key、动态 loopback 端口和完整 socket observation。本文档
提交只记录证据，不把后续文档 HEAD 改写为被构建或运行的源码。下方 `6e09e68` 运行继续作为
较早 exact tuple 的独立 `PASS` 历史保留。

最后复核：2026-08-08（Asia/Taipei）

## `9cc0d15` 当前 exact rerun

当前 rerun 绑定 source-gate run `704efbab61ae0cd87fb08885ddbe6f8d`：15/15 suites、
15/15 observations，completion seal SHA-256 为
`93d10aae3cd7cd4acce734d033fcc4bde62f7d4a04707d53eb8d938aeb605cc3`。G1 binding receipt
SHA-256 为 `a6ba09cc74e2aa4e853517dbd2fa5643cb02a122b04b7b986aaef9f945646784`；canonical
bundle、Desktop、packaged Gateway SHA-256 分别为
`77d3096e8302e289048c3771d04b2a58517ae6730f08a55fac3aeab65da720aa`、
`c02b52267272c413b666a24408076989c013a5feb176b31ca33807737618fc39`、
`ccbc0839fb92ecdb70dca898edceb5041ed5eb66993dcb48c39d9ed851ea245f`。Science 0.1.25
package / executable identity 与先前运行一致。

同一 exact tuple 在
`/private/tmp/csswitch-science-probe-evidence/r9cc0d15a/B-RUNTIME-01/` 完成 production Desktop →
packaged Gateway → Science → loopback provider、一键开始、单实例重开复用、产品停止/重启、
再次请求、最终停止和外部清理，final event elapsed `225.075563s`。50 个 evidence 文件 closure
全部复算 `OK`；`hashes.sha256` 自身 SHA-256 为
`1e84ed7fb67152140f711423ec14e1e2d9fd53bfc1e91b65782064d44ca849b3`，network socket
observation SHA-256 为
`c0a9f1db8c54e3da2949720dea37eaf1efbc0bc83d4f5b24979c6aaecd6408cd`，external cleanup
SHA-256 为 `95c7662c49db51098ed4ad8deb7554e3c64535bb2771c434f483e5ef43f7fa67`。
目标 PID、动态端口和 exact runtime root 最终均清零，8765 未使用；真实 provider/账号、installed、
签名、公证、DMG、tag 与 public release 仍不由本次外推。

## `6e09e68` 较早 exact PASS（历史）

[B-RUNTIME-01 合同](../../operations/science-probe-spec.md)要求的本轮链路已经逐段实际执行：

`Desktop → packaged Rust Gateway → Claude Science → 一键开始 → provider request →`
`单实例重开复用 → 产品停止 → 产品重启 → provider request → 产品停止 → 清理`

最终结果为 `PASS`：

- exact source SHA 的 15-suite source gate 为 15/15 `PASS`，随后从该 clean worktree
  全新构建 acceptance App；
- Desktop、packaged Rust Gateway 和 Science 0.1.25 均建立 exact PID/path/hash 身份，
  全部运行在禁止非 loopback 外联的沙箱中；
- 一键开始成功，真实 Science 进入隔离 data-dir，Rust Gateway health ready；
- DeepSeek native acceptance override 只把上游导向 loopback fake provider；五次请求全部
  命中预期 path/header/body，均返回 200，0 failure；
- 重开没有产生第二个 Desktop，Gateway PID 与 `launch_id` 保持不变；
- 产品 UI 的两次停止都直接成功并关闭 Gateway/Science listener；重启产生新的 Gateway PID
  和新的 `launch_id`，随后请求再次成功；
- 最终四个动态端口关闭，七个精确归属 PID 均不存在，临时控制器退出并回收其 helper。

2026-08-07 的[完整尝试](2026-08-07-claude-science-0.1.25-b-runtime-01-full-attempt.md)
仍是其当时 artifact 的历史 `INCONCLUSIVE` 记录；本次使用新的 exact SHA、构建和证据闭合，
不回写旧结论。

## Source gate 与 artifact binding

- source：`6e09e68654e1c43b936821c8be12330af3e19cc1`，clean detached worktree；
- source gate run id：`3819eadf41db2db38ee23e7cbf8992c5`；
- completion seal：15 个 suite 均为 `PASS`、`runner_exit=0`、
  `aggregate_decision=PASS`；seal SHA-256
  `1a0c5096de7fcdd1812d9df3b76563388c67dc423dae9d3f9cdd4b26e5f44923`；
- private source snapshot 位于该 run 的 `state/runs/<run-id>/snapshot/`，不在 public
  evidence 目录；其 manifest SHA-256 为
  `6c5afce41c0c5c19569cfc47a2d709ad8069a19a63ec477501997c044b88fefa`；
- build：`npm ci` 后执行 acceptance-build 的 Tauri App bundle 构建，成功；没有替换正式 App；
- CSSwitch Test canonical bundle digest：
  `3474ef2e817240914164d41110157acdb125682a0c191a1112761b79125b0409`；
- Desktop SHA-256：
  `242243bdcb7dfee4464b398aa2c10956b0a579270410620c81ce6c7a66dbe883`；
- packaged Gateway SHA-256：
  `fded59d736c9023d22af732fe50f7311e0ea274b4962bc9bfbb8020dce4f094c`；
- Science 0.1.25 package canonical digest：
  `371a10ecb2e4fc610ede1e3ab266c3dfc71d3894d9bcb9ce35515145d93c49ca`；
- Science executable SHA-256：
  `63b0f57aa3b9588ba9e61433d27c78df788f8fe2c1b51842db107d6697e9c03f`；
- G1 binding receipt SHA-256：
  `3a272f8675549ac6ab21a5d426f0be56c72da34cdde2e4b8613f13b3a3353761`。

## Runtime 观察

| 阶段 | exact observation | 结果 |
|---|---|---|
| Desktop | PID `64975`，exact freshly-built executable | `PASS` |
| 首次 Gateway | PID `67670`，`launch_id=3ea08bd8da4304b0750242905ece7e65`，`gateway=rust` | `PASS` |
| 一键开始 | Gateway `67670`、Science `67694` ready，UI 显示代理/沙箱/上游运行 | `PASS` |
| provider 首轮 | scratch + formal 各一次，HTTP 200 | `PASS` |
| 重开 | exact Desktop PID 集合前后均为 `{64975}`；Gateway PID/launch id 不变 | `PASS` |
| provider 复用 | 两次请求，HTTP 200 | `PASS` |
| 产品停止 | UI `已停止代理与沙箱。`；`59123/59124` 关闭 | `PASS` |
| 产品重启 | Gateway PID `74690`，`launch_id=829f153bdbda693f3213b0cd5ce0ec56` | `PASS` |
| provider 重启后 | 一次请求，HTTP 200 | `PASS` |
| 最终产品停止 | UI 直接成功；Gateway/Science listener 关闭 | `PASS` |

provider mock 共收到 5 个请求，5/5 consumed，`protocol_complete=true`、
`queue_complete=true`、`failures=[]`、owned process exit 0。首次、重开与重启阶段均使用同一
预选 DeepSeek/off fixture。首次 start 应用配置后建立 checkpoint；此后重开、停止、重启与
最终停止阶段的配置内容和哈希保持不变，changed/unexpected path 均为空。

真实 Science 在首次运行和重启后分别绑定 PID `67694`、`74739`；两次都由 PID 独占
`127.0.0.1:59124`，executable hash 都与冻结的 Science 0.1.25 identity 相同，并使用同一隔离
data-dir。四个 lifecycle port observation 分别证明 initial/restarted 为 open、first/final stop
为 closed。

## 隔离、安全与清理

network policy 为
`deny-egress-except-ip-loopback-and-owned-science-unix-control`。self-test 证明 IPv4/IPv6
loopback 可用，而非 loopback TCP/UDP、DNS transport 与 system resolver IPC 被阻断；
端口 `8765` 未使用。运行只使用固定 fake credential；没有读取或回显真实 API key、OAuth、
Keychain、SSH 私钥、账号数据库或真实 `~/.claude-science`。

40 个运行日志/证据文件的敏感材料扫描为 0 match，Python gateway tripwire 未触发。
最终 mock 正常退出；`59123/59124/59125/59141` 均无 listener；Desktop、launcher、mock、
两代 Gateway 和两代 Science 的七个精确归属 PID 均不存在。driver cleanup receipt 为
`ok=true`、runtime-root open PID 为空；补充的 exact PID/port 外部清理 receipt SHA-256 为
`37a15a4263e372ae2614e3efad60f5f15edf9dedebd3f0d955f863c54870bef4`。该收据还证明
controller PID `63612` 已以 exit 0 退出；after inventory 与 driver cleanup 落位后，精确的
probe runtime 根已删除，sibling evidence 保留供复核。

pre-run manifest 冻结 `observe_start=60s`、`stop=60s`、`overall=300s`。结构化事件从
`start_mock` 到 `evidence-finalized` 的 monotonic elapsed 为 209.591220 秒，严格落在 overall
deadline 内；56 条事件都带同一 `time.monotonic` 时钟字段并严格递增，顶层失败为 0。未用
放宽 deadline 的替代 harness。运行使用合同规定的
`/private/tmp/csswitch-science-probe-runtime/r6e09e68a/home/`，case evidence 位于合同规定的
`/private/tmp/csswitch-science-probe-evidence/r6e09e68a/B-RUNTIME-01/`。

最终原始包包含 `events.ndjson`、`observations.json`、`inventory-before/after.json`、
`cleanup.json`、两份 Science live identity、四份 lifecycle port observation、五份 UI
accessibility observation、逐断言 observations 和 provider result。49 项 hash closure 全部复算 `OK`；manifest
`hashes.sha256` 的 SHA-256 为
`7a7305beec8678eaeb493614828701f7da442c6a4cce565cf2abb21eb8043e8e`；该最终
closure 在 controller exit 0、外部 PID/port 复核和 runtime-root 删除后生成并复算 49/49
`OK`；最终 `inventory-after.json`、`cleanup.json`、`observations.json` 与
`external-cleanup.json` 都处于同一 closure 内。

## 修复与独立审查边界

本轮实际修复分为两条窄边界：

1. acceptance-build 下，仅 DeepSeek/Qwen 的 native upstream 可由显式 loopback override
   注入；production build 和非 loopback 值不接受该覆盖。
2. Science stop command 返回 non-zero 时，只有当前完整 managed launch token（PID、start time、
   executable、listener、receipt）仍一致才允许 TERM/KILL exact process；任一 identity drift
   都拒绝发信号，stop command unavailable 仍保持失败而不伪装成功。

相关聚焦测试、完整 source gate 与 clean-context 独立代码审查均通过。最终代码审查结论：
`PASS`，`BLOCK/HIGH/MEDIUM/LOW=0/0/0/0`。

## 不能外推

本记录不证明真实 provider/账号、真实凭证、Skill、SSH、正式安装的 CSSwitch App、签名、公证、
DMG、tag、public release 或其他 B/C probe；这些均为 `NOT-RUN`。本轮也没有 push、tag、release
或替换 `/Applications/CSSwitch.app`。
