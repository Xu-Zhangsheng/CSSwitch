# Claude Science 0.1.25 `B-CONTEXT-01` exact-artifact isolated-live 验收

状态：`PASS`

适用范围：`ISOLATED-LIVE(scope=isolated-request-shape)`；只验证合成 project/session 下的
plan approve/reject、delegation、fork/restore、memory save/search、compaction，以及
Reviewer/Specialist 的本地 surface、持久状态和脱敏 outbound request shape。

最后复核：2026-08-10（Asia/Taipei）

失效条件：目标 source/artifact/Science identity、B-CONTEXT probe card、Science 本地
project/session/memory/Reviewer/Specialist 语义或 Gateway request envelope 任一相关事实改变。

## 1. 总判定与边界

`B-CONTEXT-01=PASS(scope=isolated-request-shape)`。11 个 sub-gate 均取得可观察状态或
请求形态；两个 target root 的四项 recorder cross-domain 计数全部为 0；活动期 owned
process 的 INET socket 仅见 `127.0.0.1` / `::1`；停止、退出、端口清零、合成 Memory
删除和精确路径清理均有封存证据。

这个 PASS 不证明 Reviewer 质量、Specialist 实际执行质量或 Anthropic entitlement，也不涉及
真实账号/provider、Skill/MCP、SSH、installed App、签名、公证或 release。Reviewer UI 保持
`Inconclusive`；mock response 不计作 Reviewer 或 Specialist 服务成功。

## 2. 精确身份与前置

本轮没有重建 artifact，直接复用并重新核对 G1 已绑定的同一个 exact artifact：

- exact source：`06b630bb3e3fb0d63425bf48d1ffc2d3613fd992`；
- Desktop SHA-256：
  `cf0e84e6b33b761767394b6d5f3579e310bec5c07de8015cd79f2d407d9d5274`；
- packaged Rust Gateway SHA-256：
  `ed4dae8ec8139c4828dd0915d1594d69001e7b504c582a710e905707ef9d03d1`；
- CSSwitch bundle canonical digest：
  `634c13f2597c10cbbf75a7cac8d1af135523eccff2cb86373824695cccb32e1a`；
- Claude Science executable 0.1.25 SHA-256：
  `63b0f57aa3b9588ba9e61433d27c78df788f8fe2c1b51842db107d6697e9c03f`；
- Science package canonical digest：
  `371a10ecb2e4fc610ede1e3ab266c3dfc71d3894d9bcb9ce35515145d93c49ca`。

运行前再次执行 G1 `hashes.sha256` 复核，5/5 为 `OK`；同一 `06b630b` tuple 的
`B-RUNTIME-01=PASS` 与 `B-CORE-01=PASS` binding 被复制进本轮 evidence
`bindings/`。detached source worktree 只用于复核 exact source，运行目标始终是保存的
G1 artifact。

## 3. 隔离 fixture 与生产入口

run id 为 `bcontext-06b630b-r1`。使用全新外层 HOME、独立 CSSwitch config/Science
data-dir、固定合成假 key、loopback mock 和两组标记：

- project：`BCONTEXT_PROJECT_A`、`BCONTEXT_PROJECT_B`；
- root session：`BCONTEXT_SESSION_A`、`BCONTEXT_SESSION_B`；
- Gateway / Science / preview / provider：51995 / 51996 / 51997 / 51998；
- 禁止端口：8765。

启动从 exact `CSSwitch Test.app` 的“一键开始”进入 registered Tauri IPC/production
chain；lifecycle 保留 `sandbox_launch`、`sandbox_health ... ready` 与
`finish ... detail=ok action=started proxy_action=restarted`。未直接调用内部启动 helper。

真实 HOME、真实凭证、Keychain、SSH、真实 Claude/provider endpoint、非 loopback destination
与 8765 均未读取或使用。

## 4. Surface matrix

| Sub-gate | 结果 | 独立观察 |
|---|---|---|
| plan approve | `PASS` | A 的 plan 从 ready/review 进入 approved/current；pending review 消失，approved artifact 可读 |
| plan reject | `PASS` | B 的 ready plan 直接点击 Discard 后出现 `Plan discarded`，artifact 标为 non-current |
| delegation | `PASS` | A 的 `Delegation` 先 checked → unchecked → checked，最终开启并发送独立 marker |
| fork | `PASS` | `Branch in new session` 创建独立 `BCONTEXT_FORK_A`，源 session 保留 |
| restore | `PASS` | fork 中编辑消息后出现 branch `2/2` 与 `Previous branch`；切换后原消息仍可见 |
| memory save | `PASS` | Settings → Memory 从 off 变 on，合成 `About you` note 计数 0 → 1 |
| memory search | `PASS` | exact marker 搜索为 1 match；经用户即时确认后永久删除，计数回到 0 且显示 `No notes yet` |
| compaction | `PASS` | summary 0 → 1；UI 显示 full history 850K 被压到 current working context 64 |
| Reviewer surface | `PASS(surface)` | built-in `ID: REVIEWER`、instructions/capabilities surface 可读；7 个请求 forced `verdict` |
| Specialist surface | `PASS(surface)` | selector、Create new、`/Customize`、Settings list 与 Add specialist 可读；14 个请求 forced `create_work_item` |
| two-project/two-session isolation | `PASS` | 两个 target root 在 UI 分离；project B 最终 ACK 不含 A；四项 cross-domain violation 均为 0 |

fork 与 `/Customize` 自然生成两个额外 session；“两个 session”指 fixture 冻结的两个 target
root，不把被测工作流产生的 session 错报成 fixture 漂移。

## 5. Memory、持久状态与 compaction

Settings → Memory 的 `About you` 是显式 workspace-global surface。合成 note 完成
save/search 后先删除，再发送 project-B isolation request；因此最终断言同时证明：

1. A/B project 与两个 root session 的 outbound marker 不混入另一域；
2. 已删除的 A Memory marker 不进入 B isolation request；
3. 不把用户显式配置的 global Memory 误判为隐式 project/session 泄漏。

hashed `surface-matrix.json` 与 `state-transitions.json` 固定 Memory 0 → 1 → exact
search match 1 → cleanup 0，以及 compaction summary 0 → 1、full history 850K / current 64
的 UI 状态；`isolation-assertions.json` 固定删除发生在 B isolation request 之前。运行中
只把隔离数据库作为诊断面检查，没有复制数据库或把未封存的精确 table/row 计数写成证据。
这里验证本地 surface、可回读状态和请求形态，不外推 global/project-scoped Memory 的全部
优先级或服务端语义。

## 6. 脱敏 outbound request shape 与不串域

recorder 只保留 top-level keys、model、stream、message roles、tool names、tool choice、合成
marker names 与布尔 flag；140 个 request 的完整 body 全部省略，凭证值未落 evidence。

| 请求面 | 观察 |
|---|---|
| `generate_plan` | 18 个请求；model `deepseek-v4-pro`；普通/`none` tool choice shape |
| `summarize_conversation` | 7 个请求；model `deepseek-v4-pro` |
| Reviewer | 7 个 `deepseek-v4-flash` 请求；forced `tool_choice={type: tool, name: verdict}` |
| Specialist create route | 14 个 `deepseek-v4-flash` 请求；forced `tool_choice={type: tool, name: create_work_item}` |

机器断言全部为 0：

| 断言 | 计数 |
|---|---:|
| 同一 request 同时含 project A + B marker | 0 |
| 同一 request 同时含两个 root session marker | 0 |
| project B request 含 A project/session/deleted-Memory marker | 0 |
| project A request 含 B project/session marker | 0 |

Reviewer 最终仍为 `Inconclusive`；Specialist 只验证本地 create/customize surface 和请求合同。
两者服务端结果均为 `UNVERIFIED`。

## 7. Fixture 说明与脱敏

fixture 从首次运行即使用被接受的 32-hex 合成 secret、Anthropic SSE framing，并限制每 turn
最多一个 `tool_use`。运行后只把 plan reject 描述收紧为本轮直接观察到的
`ready → discarded/non-current`；该修订不改变 fixture 或产品状态。

重新连接本地 UI 时使用的一次性 loopback nonce 在封存前全部替换为 `[REDACTED]`。最终
sensitive-pattern scan 未发现 raw nonce、private key、真实 Bearer/token 或 key pattern；
evidence 内 URL 仅为 `localhost` / `127.0.0.1`。

## 8. Network、停止与精确清理

活动期 exact owned process 为 Desktop 85478、Gateway 85485、Science 85509、fixture 85433。
`network-active.json` 证明：

- Desktop：无 INET row；
- Gateway：仅 `127.0.0.1:51995` listener；
- Science：仅 `127.0.0.1` / `::1` listeners 与本地 established rows；
- fixture：仅 `127.0.0.1:51998` listener；
- non-loopback：0；8765：0。

从 CSSwitch UI 执行“全部停止”，产品显示“已停止代理与沙箱”；随后执行“退出 CSSwitch”，
App exit 0，最后停止 fixture。hashed closing inventory 中 Desktop/Gateway/Science/fixture
无残留，51995–51998 与 8765 无 listener，合成 Memory 已删除，浏览器 tab 已 finalize。

随后只删除本轮独占对象：

- runtime root：`/private/tmp/csswitch-science-probe-runtime/bcontext-06b630b-r1`；
- detached source worktree：`/private/tmp/csswitch-bcontext-06b630b-source`；
- temporary driver：`/Users/superjj/ccproj/CSswitch/.tmp-bcontext-driver.py`；
- temporary bytecode cache：`/private/tmp/csswitch-bcontext-06b630b-pycache`。

`post-cleanup-absence.json` 固定四个路径、worktree registration 与五个端口的 absence；
其他 worktree 未触碰。G1 artifact 是本轮只读输入、并非 B-CONTEXT-owned cleanup 对象，继续
保留在原路径。

## 9. Evidence closure

证据根：
`/private/tmp/csswitch-science-probe-evidence/bcontext-06b630b-r1/B-CONTEXT-01`

关键 digest：

- `manifest.json`：
  `2952dae86001b39679553f8066e8929479bec8a2e179a5c842560ba4f11c717d`；
- `surface-matrix.json`：
  `d60929a5e09d98d13a926ed04350941470c3d2824ebe16804f802259e0dfcad0`；
- `isolation-assertions.json`：
  `6f609efb78f0e857e9d1a0be8c7728ea242b76f20aa05ad12394ea6112de4a7e`；
- `request-shape-summary.json`：
  `28317e19aa74f40094665241032fe6a94fb7d17c0803ff3be2c830e18be78946`；
- `network-active.json`：
  `cc0f4feea321f95c40a23c93e8ac64e2cb0ef6319cbb6e39e48752dee822318a`；
- `inventory-after.json`：
  `c0bfcfb8d9a95e74331f6303e44e5b8592a491f9104e4e2bdbe5b5daa617c2b8`；
- redacted `fake-open.log`：
  `28dacfa931d546615cc54b1729c1140a27f03d88bc0a1a12e992f66f997253f3`；
- `post-cleanup-absence.json`：
  `f4342ff1a3a1b3014d8e2b8f09c8efc1bf04d399c5e9b48ebbb0e19e1c5c692e`；
- closing `hashes.sha256`：
  `2d09c125cbddbe95a00d34e6b2c1d44cb02b8ff46b6a3115d82ad2bb90f9ecec`。

`hashes.sha256` 固定 23 个 evidence 文件；逐项复核为 23/23 `OK`。

## 10. 不外推

本记录不证明 Reviewer/Specialist 服务成功或质量，不证明 Anthropic entitlement、真实
provider/model、真实账号、Skill、MCP、SSH、installed App、签名、公证或 release；也不把
global `About you` Memory 外推为 project-scoped Memory 的全部语义。
