# Claude Science 0.1.25 `B-CONTEXT-01` `a60c2ee` exact-artifact isolated-live 验收

状态：`PASS`

适用范围：`ISOLATED-LIVE(scope=isolated-request-shape)`；绑定
`next@a60c2ee656429903f1fd8f398dc6ad8194aa9346`、由该 exact source 构建并经 G1 receipt
绑定的 `CSSwitch Test.app` 0.8.4、packaged Rust Gateway、Claude Science 0.1.25，以及本轮隔离
HOME/data-dir、loopback fixture 与合成 project/session。

最后复核：2026-08-11（Asia/Taipei）

失效条件：artifact-producing source、CSSwitch bundle/Desktop/Gateway、Science package/executable、
`B-RUNTIME-01` 或 `B-CORE-01` 前置、B-CONTEXT probe card、Science 本地 project/session/memory/
Reviewer/Specialist 语义或 Gateway request envelope 任一相关事实改变。

## 结论与边界

canonical run `bcontext-a60c2ee-r13` 的 11 个 B-CONTEXT 子门全部 `PASS`：plan approve/reject、
delegation、fork/restore、Memory save/search/delete、compaction、Reviewer/Specialist 本地 surface 与
two-project/two-root-session isolation。11 份 observation 分别封存后再计算
`surface-matrix.json`；`observations.json` 为 `all_pass=true`，不是无条件写入 PASS。

150 个脱敏 request envelope 的四项跨域计数均为 0。活动期 Desktop、fixture、Gateway 与 Science
共 26 条 owned INET socket rows，全部为 `127.0.0.1` / `::1`；non-loopback 与 8765 rows 均为 0。
CSSwitch Test 最终从产品 UI 显示“已停止代理与沙箱。”，LaunchServices invocation 正常结束为
exit 0；owned process、动态端口、合成 Memory、浏览器 tab 与本轮临时 runtime/driver/cache 均完成
精确清理。

本结论只证明本地 surface、持久状态和隔离请求形态。Reviewer UI 为 `Inconclusive`；
Reviewer/Specialist 服务结果均为 `UNVERIFIED`，loopback fixture response 不计作服务成功。没有读取
账号数据库、真实用户文件、凭证、Keychain 或 SSH 状态；也不证明真实账号/provider、完整 Provider、
Skill/MCP、SSH、installed App、升级/rollback、签名、公证、DMG 或 release-ready。

## Exact identity 与前置

| Identity | Exact value | 判定 |
|---|---|---|
| artifact-producing source | `a60c2ee656429903f1fd8f398dc6ad8194aa9346` | `PASS` |
| source gate run / completion seal | `6e5124c43c1ceadac3bd54f6408e2141` / `fe8d24939dd8b950c95984383645aa8de87dae4956c5c34c9a533b310560d86e` | `PASS` |
| G1 binding receipt SHA-256 | `b01cf7a2066576dfc5da1a59eada2faad9f586100f1b27d013cad9b4d420ddbb` | `PASS` |
| CSSwitch bundle canonical digest | `52cd48c06b2d2bffdd6d8e85d464be063e4604c71961a5cc8459879938a673c9` | `PASS` |
| Desktop SHA-256 | `77b4ebefe6e658c4f8f90e2b5177db5398be75eacf3b3f0d3364269eba5e9be8` | `PASS` |
| packaged Gateway SHA-256 | `610c0206973ec0281b6e34e171e4a6fb9899b31050642a073c4f0d1ca64887ab` | `PASS` |
| Science version / executable SHA-256 | `0.1.25` / `63b0f57aa3b9588ba9e61433d27c78df788f8fe2c1b51842db107d6697e9c03f` | `PASS` |
| `B-RUNTIME-01` 前置 | canonical run `ra60c2eec`；51/51 hashes `OK` | `PASS` |
| `B-CORE-01` 前置 | canonical run `bcore-a60c2ee-r1`；30/30 hashes `OK` | `PASS` |

运行 artifact 直接来自保留的 G1 root `/private/tmp/g1.a60c2ee.q4uEQl/`，没有重新构建或替换
`/Applications/CSSwitch.app`。exact source 输入为 clean detached
`/private/tmp/g1srca60.KmYW1V/repo`。Gateway / Science / preview / provider 端口分别为
53634 / 53635 / 53636 / 53637，禁止端口为 8765。

## 前序停滞与 canonical 边界

此前长时间停滞来自验收驱动与取证目标漂移，不是产品进程死锁。各次失败均保留原始判定，未覆盖或
晋升为 PASS：

| Run | 停止原因 | 产品失败结论 |
|---|---|---|
| `r9` | observation 写死 tab 17，而 28 份 capture receipt 实际绑定 tab 18；首个 plan UI 内容检查本身通过 | 否；sealed `FAIL(evidence_target_binding_mismatch)` |
| `r10` | preflight 发现 current marker 已为 R10，但 request-envelope tool-result flags 仍检查 R7，可能产生 false-zero 隔离计数 | 否；子门未开始 |
| `r11` | `generate_plan` fixture 仍使用旧 schema；实际 binary 要求 `task_summary`、`phases[].name`、`delegations[].steps[].title` 与 `feasibility` | 否；仅作 schema discovery |
| `r12` | Reviewer capture 在 restore 后误停在 edited fork 2/2，而 Reviewer surface 位于应取证的原分支/root；前 8 个子门已通过 | 否；sealed `FAIL(reviewer_capture_branch_navigation_error)` |

`r13` 在 UI 工作前冻结真实 in-app browser tab 5，并 preflight 全部 28 个 capture input 与 11 个
observation input；marker flags、`generate_plan` 与 `summarize_conversation` schema 均与当前 fixture
一致。Reviewer 改由 Project A root 取证，并在封存前先断言状态、checks、model、transcript action 与
Session A marker 全部存在。驱动器从 manifest freeze 到 final cleanup 保持 byte-identical，SHA-256 为
`4eecbc8234e1256473e342b9b2bf55e4023f28230758c637dae1b7f0dfc65eda`。

## Surface matrix

| 子门 | Exact observation | 结果 |
|---|---|---|
| plan approve | Project A 显示 one-step high-confidence synthetic plan；Approve 清除待审状态、保留可读 plan artifact，并完成 loopback acknowledgement | `PASS` |
| plan reject | Project B 显示同类待审 plan；Discard 后显示 `Plan discarded` 且待审 controls 消失 | `PASS` |
| delegation | checkbox 执行 checked → unchecked → checked，最终发送独立 Session A marker 并收到 fixture acknowledgement | `PASS` |
| fork | send menu 明确显示并执行 `Branch in new session`；独立 synthetic fork 与 Project A root 同时保留 | `PASS` |
| restore | fork 中编辑后为 branch 2/2；`Previous branch` 返回保留的 1/2 branch | `PASS` |
| Memory save | Settings → Memory 从 Off / About you 0 / No notes 变为 On / About you 1，并只保存合成 note | `PASS` |
| Memory search/delete | exact marker 搜索为 1 match；删除后恢复 Off / About you 0 / No notes | `PASS` |
| compaction | summary 0 → 1；UI 显示 850K full history 与 64 working context，pressure/trigger/session markers 均保留 | `PASS` |
| Reviewer surface | Project A root 显示 `Inconclusive`、7 checks、`claude-opus-5` 与 transcript action | `PASS(surface)` |
| Specialist surface | selector、Create new、`/Customize` request、Settings list、Add specialist 与 built-in Reviewer 可见 | `PASS(surface)` |
| two-project/two-session isolation | Project B root 响应不含 A project/session/已删除 Memory marker；四项 recorder violation 均为 0 | `PASS` |

fork 与 `/Customize` 自然生成额外 session；“two-session”只指冻结的两个 target root，不把工作流
生成的 session 错报成 fixture 漂移。Memory 是显式 workspace-global `About you` surface：合成 note
先完成 save/search，再于 Project B isolation request 前删除；本结论不把它外推为 project-scoped
Memory 的全部优先级或服务端语义。

## 脱敏请求形态与不串域

recorder 只保留 top-level keys、model、stream、message roles、tool names、tool choice、合成 marker
与布尔 flag；150 个 request 的 body 全部省略，Authorization 只记录 present 布尔值，未保存或回显值。

| 请求面 | 观察 |
|---|---|
| `generate_plan` | 18 个请求；model `deepseek-v4-pro`；`tool_choice=null` |
| `summarize_conversation` | 7 个请求；model `deepseek-v4-pro`；`tool_choice=null` |
| Reviewer | 11 个 `deepseek-v4-flash` 请求；forced `tool_choice={type: tool, name: verdict}` |
| Specialist create route | 18 个 `deepseek-v4-flash` 请求；forced `tool_choice={type: tool, name: create_work_item}` |

四项机器断言均为 0：同一 request 同时含 A/B project marker、同时含两个 root session marker、
Project B request 含 A project/session/已删除 Memory marker，以及 Project A request 含 B
project/session marker。最终 Project B request 只命中 Project B / Session B。

## Network、正常退出与精确清理

活动期 exact owned process 为 fixture 40943、Desktop 41061、Gateway 41091、Science 41116。
socket rows 为 Desktop 0、fixture 1、Gateway 1、Science 24；全部 loopback，non-loopback 与 8765 为 0。

清理先通过产品 UI 执行“全部停止”，封存“已停止代理与沙箱。”以及 Gateway / Science
“未运行 / 部分就绪”；随后执行“退出 CSSwitch”，LaunchServices invocation 为 `finished` / exit 0。
浏览器一次性 finalize 后 tab 为 0；合成 Memory 已删除且恢复 Off / About you 0 / No notes。
pre-root inventory 证明 Desktop/Gateway/Science/fixture 与动态端口/8765 全部清零。

随后只删除本轮独占 runtime root、临时 driver 与临时 Python cache；保留 clean exact source、G1
artifact、前置 evidence 与本轮 canonical evidence。`cleanup.json` 的 7 项断言全部为 true，
`production-lifecycle.json` 的 UI cleanup 与 process/port cleanup 均为 `PASS`。

## Evidence closure

证据根为
`/private/tmp/csswitch-science-probe-evidence/bcontext-a60c2ee-r13/B-CONTEXT-01/`。最终
`hashes.sha256` 索引除自身外 179 个文件，`shasum -a 256 -c hashes.sha256` 为 179/179 `OK`；
`hashes.sha256` 自身 SHA-256 为
`ba3501415d784329476c59563bb2cfb6defa92fb0e5850be27884aa01731e065`。

机器可复算链包括：

- `observations.json`：11 个 expected/actual/receipt rows，`all_pass=true`；SHA-256
  `b5b6d5f01584ec3cf17135381cc0bd0763a0112cb9aecd97d136819a7ca6177f`；
- `events.ndjson`：event id 1–11，`monotonic_ns` 严格递增；SHA-256
  `679cdafd76d1731d888e956d020061a6e804d26ed3c241057d814c71d1001090`；
- `surface-matrix.json`：11/11 rows 为 sealed observation 导出的 `PASS`；SHA-256
  `83c67067a57fb0d9e5029a002ad86ef35702daff61f52c6191e95b540d1d3b04`；
- `isolation-assertions.json`：四项 violation 均为 0；SHA-256
  `02bd14e98cf0fea412aea6ba1533be70300815be0052e71d2f03f9305e7735f7`；
- `network-active.json`：owned socket 全部 loopback、8765 未使用；SHA-256
  `37e57e5aae2e06b5086583cb285cb2087fb58415f3cbb050b62e855f39123223`；
- `cleanup.json`：正常 UI stop/exit 后的进程、端口与临时根闭合；SHA-256
  `35da1334bd90e5f7f7151e8adde67ef6a3b892e54d9a56bfc84ecc4990d99ddb`。

上游 exact identity 与前置边界分别见
[`a60c2ee` exact artifact](2026-08-11-csswitch-a60c2ee-exact-artifact.md)、
[`B-RUNTIME-01`](2026-08-11-claude-science-0.1.25-b-runtime-01.md) 与
[`B-CORE-01`](2026-08-11-claude-science-0.1.25-a60c2ee-b-core-01.md)。
