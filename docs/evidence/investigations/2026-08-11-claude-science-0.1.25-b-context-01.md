# Claude Science 0.1.25 `B-CONTEXT-01` exact-artifact isolated-live 验收

状态：`PASS`

适用范围：`ISOLATED-LIVE(scope=isolated-request-shape)`；绑定
`next@9e08924481c8f5edb181254332d94daba0cbe4b2`、由该 exact source 构建并经 G1 receipt
绑定的 `CSSwitch Test.app`、packaged Rust Gateway、Claude Science 0.1.25，以及本轮隔离
HOME/data-dir、loopback fixture 与合成 project/session。

最后复核：2026-08-11（Asia/Taipei）

失效条件：artifact-producing source、CSSwitch bundle/Desktop/Gateway、Science package/executable、
`B-RUNTIME-01` 或 `B-CORE-01` 前置、B-CONTEXT probe card、Science 本地 project/session/memory/
Reviewer/Specialist 语义或 Gateway request envelope 任一相关事实改变。

## 结论与边界

同一个 `9e08924` exact artifact 的 11 个 B-CONTEXT 子门全部 `PASS`：plan approve/reject、
delegation、fork/restore、Memory save/search/delete、compaction、Reviewer/Specialist 本地 surface 与
two-project/two-root-session isolation。159 个脱敏请求 envelope 的四项跨域计数均为 0；产品与
fixture 的 exact owned processes 活动期 26 条 INET socket rows 全部为 loopback。每个子门都有
独立 expected/actual observation、脱敏 UI/state 指针与严格单调 event；`surface-matrix.json` 由这
11 份 sealed observation 计算，而不是无条件写入 `PASS`。

本结论只证明本地 surface、持久状态和隔离请求形态。Reviewer UI 最终仍为 `Inconclusive`；
Reviewer/Specialist 服务结果均为 `UNVERIFIED`，loopback fixture response 不计作服务成功。没有读取
账号数据库、真实用户文件、凭证、Keychain 或 SSH 状态；也不证明真实账号/provider、完整 Provider、
Skill/MCP、SSH、installed App、升级/rollback、签名、公证、DMG 或 release-ready。

## Exact identity 与前置

| Identity | Exact value | 判定 |
|---|---|---|
| artifact-producing source | `9e08924481c8f5edb181254332d94daba0cbe4b2` | `PASS` |
| source gate run | `078d462c81abcec146644c8096254069`；15/15 suites、15/15 observations | `PASS` |
| source completion seal SHA-256 | `1c071a18701ac6e6a191c6dfc3cd1513d3465bce90d3567a6460cd46f36d0fe8` | `PASS` |
| G1 binding receipt SHA-256 | `39aa3dc5866807140d42409ab8eea2f6625fa4d9f7212f06482dcc0db0c9b762` | `PASS` |
| CSSwitch bundle canonical digest | `d77cb799f2063241041cc8d17bd57a3c72f49cc7b0da6899d63a52795df1ac64` | `PASS` |
| Desktop SHA-256 | `ada760cb9dcfdd9b2151d652ff744f300a914b3bef8c07ea85ce886994458a6b` | `PASS` |
| packaged Gateway SHA-256 | `bfa05512337329f52811c2d7c08081ed2249a34c6ed98dd7fe7d83313d9b3036` | `PASS` |
| Science version / executable SHA-256 | `0.1.25` / `63b0f57aa3b9588ba9e61433d27c78df788f8fe2c1b51842db107d6697e9c03f` | `PASS` |
| `B-RUNTIME-01` 前置 | canonical run `r9e08924b`；51/51 hashes `OK` | `PASS` |
| `B-CORE-01` 前置 | canonical run `bcore-9e08924-r1`；14/14 hashes `OK` | `PASS` |

运行 artifact 直接来自保留的 G1 root `/private/tmp/g1.9e08924.91acQY/`，没有重新构建或替换
`/Applications/CSSwitch.app`。run id 为 `bcontext-9e08924-r9`；Gateway / Science / preview /
provider 端口分别为 59343 / 59344 / 59345 / 59346，禁止端口为 8765。启动从 exact
`CSSwitch Test.app` 的“一键开始”进入 registered Tauri UI/IPC production chain；operation log
固定 `sandbox_launch`、`sandbox_health ... ready` 与
`finish ... detail=ok action=started proxy_action=restarted`。

## Surface matrix

| 子门 | Exact observation | 结果 |
|---|---|---|
| plan approve | Project A 显示 one-step high-confidence synthetic plan；Approve 清除待审状态、保留可读 plan artifact，并完成 loopback acknowledgement | `PASS` |
| plan reject | Project B 显示同类待审 plan；Discard 后显示 `Plan discarded` 且待审 controls 消失 | `PASS` |
| delegation | checkbox 执行 checked → unchecked → checked，最终发送独立 Session A marker 并收到 fixture acknowledgement | `PASS` |
| fork | send menu 明确显示并执行 `Branch in new session`；独立 synthetic fork 与 Project A root 同时保留 | `PASS` |
| restore | fork 中编辑后为 branch 2/2；`Previous branch` 返回保留的 1/2 root branch | `PASS` |
| Memory save | Settings → Memory 从 Off / About you 0 / No notes 变为 On / About you 1，并只保存合成 note | `PASS` |
| Memory search/delete | exact marker 搜索为 1 match；删除后恢复 Off / About you 0 / No notes | `PASS` |
| compaction | summary 0 → 1；UI 显示 850K full history 与 64 current context，合成 session marker 仍可见 | `PASS` |
| Reviewer surface | Reviewer 显示 `Inconclusive`、5 checks、`claude-opus-5` 与 transcript action | `PASS(surface)` |
| Specialist surface | selector、Create new、`/Customize` request、Settings list 与 Add specialist 可见 | `PASS(surface)` |
| two-project/two-session isolation | Project B root 响应中不含 A project/session/已删除 Memory marker；四项 recorder violation 均为 0 | `PASS` |

fork 与 `/Customize` 自然生成额外 session；“two-session”只指冻结的两个 target root，不把工作流
生成的 session 错报成 fixture 漂移。Memory 是显式 workspace-global `About you` surface：合成 note
先完成 save/search，再于 Project B isolation request 前删除；本结论不把它外推为 project-scoped
Memory 的全部优先级或服务端语义。整个过程只通过产品 UI、合成状态与脱敏 request envelope 观察，
没有检查账号数据库。

## 脱敏请求形态与不串域

recorder 只保留 top-level keys、model、stream、message roles、tool names、tool choice、合成 marker
与布尔 flag；159 个 request 的 body 全部省略，159/159 只记录
`authorization_header_present=true`，未保存或回显值。

| 请求面 | 观察 |
|---|---|
| `generate_plan` | 17 个请求；model `deepseek-v4-pro`；`tool_choice=null` |
| `summarize_conversation` | 7 个请求；model `deepseek-v4-pro`；`tool_choice=null` |
| Reviewer | 9 个 `deepseek-v4-flash` 请求；forced `tool_choice={type: tool, name: verdict}` |
| Specialist create route | 16 个 `deepseek-v4-flash` 请求；forced `tool_choice={type: tool, name: create_work_item}` |

四项机器断言均为 0：同一 request 同时含 A/B project marker、同时含两个 root session marker、
Project B request 含 A project/session/已删除 Memory marker，以及 Project A request 含 B
project/session marker。最终 Project B request 只命中 Project B / Session B。

## Network、退出边界与精确清理

活动期 exact owned process 为 fixture 38195、Desktop 38230、Gateway 38243、Science 38267。
raw capture 共 26 条 socket rows：Desktop 0、fixture 1、Gateway 1、Science 24；全部为
`127.0.0.1` / `::1`，non-loopback rows 为 0，8765 rows 为 0。

本轮结束时 CSSwitch UI 的 stop 动作使 Gateway / Science 与 59343–59345 退出，但受控 UI 工具随后
失去 state-read 权限，未能封存 stop 后的 UI snapshot，也不能在不触发意外重启风险的条件下继续点
App exit。因此本记录不把“UI 显示已停止”或“App 正常 exit 0”列为 r9 证据；Desktop 38230 最终由
任务精确 PID 的 SIGTERM 收口。独立 closing inventory 证明 Desktop/Gateway/Science/fixture 四个
exact PID 与 Gateway/Science/preview/provider/8765 五个端口全部清零；合成 Memory 已删除，浏览器
target 与全部 tab 均为 0。`B-RUNTIME-01` 已另外证明正常 UI stop/exit 链，本轮不重复外推。

随后只删除本轮独占的 runtime root、工作树临时 driver 与临时 Python cache；保留 clean exact source
输入、G1 artifact、B-RUNTIME/B-CORE evidence 与本轮 canonical evidence。final `cleanup.json`
明确记录 runtime root、临时 driver/cache 均不存在，并保留 pre-root cleanup receipt。

## Evidence closure

证据根为
`/private/tmp/csswitch-science-probe-evidence/bcontext-9e08924-r9/B-CONTEXT-01/`。最终
`hashes.sha256` 索引 89 个文件，逐项复核为 89/89 `OK`；closing `hashes.sha256` 自身
SHA-256 为 `4bc7185055a1db26e5f4874df53e5911220193d8d7f7c4f8f7875fcf0fd2721c`。

机器可复算链包括：

- `observations.json`：11 个 expected/actual/receipt rows，`all_pass=true`；SHA-256
  `e21d8e497827a98ae3cbece1ad02b8aba6c769ac54a8dc444c73b1fac27105e4`；
- `events.ndjson`：11 个 `observation-sealed` event，event id 1–11、monotonic_ns 严格递增；
  SHA-256 `fc6a9a02f9d5ba829763f336423b005721600bd3d6962e4f43ac4717c1c94b45`；
- `surface-matrix.json`：明确声明从 sealed observations 计算且无 unconditional PASS rows；
  SHA-256 `ac9f7f030f53dfa81bbf96ebd4975ae2111b408e4642cbfe1855520b400fb8c4`；
- `request-envelopes.ndjson`：159 条 body-omitted request shape；SHA-256
  `70fb745a130ccc0a51c4409ba7444b6b4649aad1a3fab783cff1891719774f7c`；
- `cleanup.json`：runtime/temp driver/temp cache absent，exact source/G1/evidence preserved；
  SHA-256 `53725e1550d20fa02bcb96befca7db4b730348bf02a450c95d9f2763af2430d4`。

关键 digest：

- `manifest.json`：`7c90251febfdb6351f1933f796e283b37a3027c3e0707c135ce414eacfcdc164`；
- `state-transitions.json`：`2f7ec33c6c0c4c78ba25abc89e9c53895e367bd948da1ee7cd69929d6a5ec5ea`；
- `isolation-assertions.json`：`d34e716b0d1024ea6a2282305f37f0451ca4980a1dd7b7f93845eead141ca9f5`；
- `request-shape-summary.json`：`88b442a62e5fa174c43cd68fa8473e85d8ba808ab46b56ded6ba3ea5185a109a`；
- `network-active.json`：`99ead4fb8187cfaea46a8d5a47b020b7667454033299166d5b554ff85b2e72b8`；
- `inventory-after.json`：`fc39707a134771c00a35d23b2e7eae20121e00e7654737bebaca55b1cb371f87`；
- redacted `fake-open.log`：`2668797f48262480276b92e89ffaa9dd56a858fa053d09d610adde3e74e606f8`；
- `post-cleanup-absence.json`：`64170c5425afa3bc6eb029643696b48aa2938f0f1cdb2b985f5da26279654d22`。

运行时 fixture、manifest 声明与 evidence 中 preserved driver 三者 byte-identical，SHA-256 为
`d580e14f41193028eb96d4a31c141719c4d2d68935e900e16f52bda5ea3ad0e4`；manifest freeze 后没有
fixture amendments。固定 synthetic fake key 只由隔离 fixture/产品正常消费，值不进入报告或
request envelope。LaunchServices initializer 为本 run 新建并保存在同一 canonical evidence root，
没有跨 run 复用；其 initializer/source/profile 与 exact Desktop identity 均由 receipt 锁定。

## 不能外推

本记录不证明 Reviewer/Specialist 服务成功或质量，不证明 Anthropic entitlement、真实
provider/model、真实账号或凭据、完整 Provider capability matrix、Skill、MCP、SSH、installed
CSSwitch、升级/rollback、Developer ID、notarization、Gatekeeper、DMG、tag、push 或 public release；
也不把本轮 normal context path 外推为 crash、并发竞争、补偿、replay 或通用数据恢复。
