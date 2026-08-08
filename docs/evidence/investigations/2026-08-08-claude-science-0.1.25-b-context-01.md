# Claude Science 0.1.25 `B-CONTEXT-01` isolated-live 验收

状态：`PASS`

适用范围：`ISOLATED-LIVE(scope=isolated-request-shape)`；只验证 plan、delegation、
fork/restore、memory、compaction、Reviewer/Specialist 的本地 surface、状态变化和脱敏
outbound request shape，以及两个合成 project / 两个目标 root session 不串域。

最后复核：2026-08-08（Asia/Taipei）

失效条件：目标 source/artifact/Science identity、B-CONTEXT probe card、Science 本地
project/session/memory/Reviewer/Specialist 语义或 Gateway request envelope 任一相关事实改变。

## 1. 总判定与边界

`B-CONTEXT-01=PASS`。11 个 sub-gate 均取得独立可观察的本地状态或请求形态，两个
target root 的 recorder cross-domain 计数全部为 0，活动期四个 owned process 的 INET
socket 仅见 `127.0.0.1` / `::1`。hashed closing inventory 证明 owned process、动态端口与
8765 清零；最终修复后的 hash closure 另含 post-cleanup receipt，确认 runtime 与本轮两个
临时 worktree、build target、driver 已删除。

这个 PASS 不证明 Anthropic entitlement、Reviewer 质量、Specialist 实际执行质量、真实
provider/账号、Web Search、installed App、签名、公证或 public release。Reviewer UI 在本轮
保持 `Inconclusive`；mock response 绝不计作 Reviewer 或 Specialist 服务成功。

## 2. 精确身份与前置

运行绑定的 production source/artifact tuple 沿用已通过的 `B-RUNTIME-01`、`B-CORE-01`
前置：

- exact source：`9cc0d15d457c911047585c5fb7302702e26f4e43`；
- Desktop SHA-256：
  `c02b52267272c413b666a24408076989c013a5feb176b31ca33807737618fc39`；
- packaged Rust Gateway SHA-256：
  `ccbc0839fb92ecdb70dca898edceb5041ed5eb66993dcb48c39d9ed851ea245f`；
- CSSwitch bundle canonical digest：
  `77d3096e8302e289048c3771d04b2a58517ae6730f08a55fac3aeab65da720aa`；
- Claude Science executable 0.1.25 SHA-256：
  `63b0f57aa3b9588ba9e61433d27c78df788f8fe2c1b51842db107d6697e9c03f`；
- Science package canonical digest：
  `371a10ecb2e4fc610ede1e3ab266c3dfc71d3894d9bcb9ce35515145d93c49ca`。

原 G1-bound artifact 路径已在先前清理中删除，因此本轮在同一 original manifest path 用
exact source 重建；Desktop/Gateway hash 与 canonical tuple 逐字节一致。运行前
`B-RUNTIME-01`、`B-CORE-01` 的 manifest/completion/hashes 被复制进 evidence `bindings/`。
主 worktree 当时为 `next@f0c46c19bb14562027d83912cf2e285777eac5ea`，其相对
`9cc0d15` 的后续提交只含既有证据/文档，不改写本轮 production executable identity。

## 3. 隔离 fixture 与生产入口

run id 为 `bcontext-9cc0d15-r1`。使用全新外层 HOME、其下独立 CSSwitch config 和
Science data-dir、固定合成假 key、loopback Anthropic mock、动态端口与两组标记：

- project：`BCONTEXT_PROJECT_A`、`BCONTEXT_PROJECT_B`；
- root session：`BCONTEXT_SESSION_A`、`BCONTEXT_SESSION_B`；
- Gateway / Science / preview / provider：65456 / 65457 / 65458 / 65459；
- 禁止端口：8765。

启动不是直接调用内部 helper：从 exact `CSSwitch Test.app` 的“一键开始”进入 registered
Tauri IPC / auto-boot production chain。sealed lifecycle summary 保留成功 operation 的
`sandbox_launch`、`sandbox_health` 与
`finish ... detail=ok action=started proxy_action=restarted`，并保留首次 fixture correction
attempt 的 compensated finish。

真实 HOME、真实凭证、Keychain、SSH、真实 Claude endpoint、Web Search、非 loopback
destination 与 8765 全部禁止；本轮未读取或使用。

## 4. Surface matrix

| Sub-gate | 结果 | 独立观察 |
|---|---|---|
| plan approve | `PASS` | A 的 plan 从 ready/review 进入 approved/current；pending review 消失，approved artifact 可读 |
| plan reject | `PASS` | B 的 revised/replacement plan 点击 Discard 后出现 `Plan discarded`，旧 artifact 标为 non-current |
| delegation | `PASS` | A 的 `Delegation` menuitemcheckbox 先 off 再 on，最终 checked；随后发送独立 delegation marker |
| fork | `PASS` | `Branch in new session` 创建独立 `BCONTEXT_FORK_A`，源 session 保留 |
| restore | `PASS` | fork 中编辑消息后出现 branch `2/2`、`Previous branch`，源 session 未被覆盖 |
| memory save | `PASS` | Settings → Memory 从 off 变 on，合成 `About you` note 计数 0 → 1 |
| memory search | `PASS` | exact marker 搜索显示 1 match；随后删除本轮合成 note，计数回到 0 |
| compaction | `PASS` | summary 0 → 1；UI 显示 full history 850K 被压到 current working context 64 |
| Reviewer surface | `PASS(surface)` | built-in `ID: REVIEWER`、enabled/disabled switch、instructions/capabilities surface 可读；9 个 request 以 forced `verdict` tool choice 发出 |
| Specialist surface | `PASS(surface)` | session selector、Create new、`/Customize` route、Settings list 与 Add specialist control 可读；16 个 `create_work_item` forced-tool request 可区分 |
| two-project/two-session isolation | `PASS` | 两个 target root 在 UI 分离；recorder 的四项 cross-domain violation 均为 0 |

fork 和 `/Customize` 自然产生两个 workflow-generated extra session；“两个 session”指 fixture
冻结的两个 target root，不把这些被测功能创建的额外 session 错报成 fixture 漂移。

## 5. Memory 作用域

Settings → Memory 的 `About you` 是显式 workspace-global surface：合成 note 在切换到 B 后
仍可见，这是产品公开给用户的受控共享状态，不是隐式 project/session 串域。本轮用它完成
save/search 可观察状态后立即删除，再发送 project-B isolation request。因此隔离断言检查的是：

1. A/B project 与两个 root session 的历史、context 和 outbound marker 不混入另一域；
2. 已删除的 A memory marker 不进入 B isolation request；
3. 不把用户显式配置的 global memory 机制误判为 project 数据泄漏。

如果后续要验证“project-scoped memory 与 global About you 的具体优先级/删除语义”，应另立
更窄 probe；本轮不从 global surface 外推该语义。

## 6. 请求形态与不串域

recorder 只保留 top-level keys、model、stream、message roles、tool names、tool choice、合成
marker names 与布尔 flag；717 个 request 的完整 body 全部省略，凭证值也未落 evidence。

可区分的请求面包括：

- `generate_plan`：model `deepseek-v4-pro`，tool surface 存在，并观察到普通/none tool choice；
- `summarize_conversation`：7 个请求，model `deepseek-v4-pro`；
- Reviewer：9 个 `deepseek-v4-flash` 请求，`tool_choice={type: tool, name: verdict}`；
- Specialist create route：16 个 `deepseek-v4-flash` 请求，
  `tool_choice={type: tool, name: create_work_item}`。

全量机器断言：

| 断言 | 计数 |
|---|---:|
| 同一 request 同时含 project A + B marker | 0 |
| 同一 request 同时含两个 root session marker | 0 |
| project B request 含 A project/session/memory marker | 0 |
| project A request 含 B project/session marker | 0 |

Reviewer 最终 UI 为 `Inconclusive` / no structured output 或 budget exhausted；Specialist 只验证
本地 create/customize surface 和请求合同。两者服务端结果均为 `UNVERIFIED`。

## 7. Fixture 修订与噪声

pre-run manifest 保持不变，后续 fixture-only 修订记录在 `fixture-amendments.json`：

1. 首次 one-click 在 Science launch 前因合成 secret 格式不被接受而 compensation；修为
   32-hex 合成值后重新从 production UI 启动成功；
2. 首次 A request 暴露 stream framing 不匹配，mock 从 JSON 改为 Anthropic SSE；
3. approve fixture 曾在 tool result 后继续发 `generate_plan`，造成 523 个重复 tool-result
   request；修为每 turn 最多一次 tool use 后闭合。

这 523 次是 fixture loop noise，只解释异常大的 request/history 数，不作为任何产品 PASS
计数。plan approve 的最终 UI 状态、plan reject 的明确 discarded 状态、compaction 的 0 → 1
summary 状态和 cross-domain machine assertions 各自独立成立。

immutable manifest 中 driver SHA/PID 是冻结时的旧值；最终 driver SHA/PID 只从 amendment
读取。所有修订都发生在合成 fixture，未引入真实 destination、credential 或正文持久化。

首次 seal 后独立审查发现 `fake-open.log` 含一次性 launch nonce。虽然该值已过期且只指向
loopback，仍按安全规则把原值机械替换为 `[REDACTED]`，确认 evidence root 中 raw nonce=0，
并使旧 closure 失效后重新生成最终 hash closure。

## 8. Network、停止与清理

活动期 exact owned process 为 Desktop 9054、Gateway 9195、Science 9220、fixture 10184。
完整 INET socket capture 中：

- Desktop：0 行；
- Gateway：`127.0.0.1:65456` listener；
- Science：仅 `127.0.0.1` / `::1` listeners 与本地 established rows；
- fixture：`127.0.0.1:65459` listener；
- non-loopback：0；8765：0。

从 CSSwitch UI 执行“全部停止”后，Gateway/Science 从正常变为未运行，产品显示“已停止代理
与沙箱”；确认 65456/65457/65458 及 exact Gateway/Science PID 清零后，再执行“退出
CSSwitch”，App 进程 exit 0，最后停止 fixture。hashed closing inventory 中
Desktop/Gateway/Science/fixture 均无残留，65456–65459 与 8765 均无 listener，合成 memory
note 已删除，browser tab 已 finalize；该 inventory 明示 runtime cleanup 当时尚待执行。

随后按归属删除 runtime root、两个本轮 detached worktree、临时 build target 与临时 driver，
并用实时 `test` / `git worktree list` / exact-path `lsof` 只读复核：

- first rebuild worktree：`/private/tmp/csswitch-bcontext-9cc0d15`；
- first rebuild target：`/private/tmp/csswitch-bcontext-9cc0d15-target`；
- exact artifact rebuild worktree：`/private/tmp/bcore-gate-parent.98x4SO/repo`；
- runtime root：`/private/tmp/csswitch-science-probe-runtime/bcontext-9cc0d15-r1`；
- driver：`/Users/superjj/ccproj/CSswitch/.tmp-bcontext-driver.py`。

最终 `post-cleanup-absence.json` 固定这些 exact path、worktree-list 与 port absence 断言并纳入
修复后的 hash closure。其他 worktree 未触碰。

## 9. Evidence closure

保留证据根：
`/private/tmp/csswitch-science-probe-evidence/bcontext-9cc0d15-r1/B-CONTEXT-01`

关键 digest：

- `manifest.json`：
  `cc58f7b36f9d95cd76c244d147bc4bf5ea4d06ec63c3e7aa03b25e738bb6af50`；
- `surface-matrix.json`：
  `d60929a5e09d98d13a926ed04350941470c3d2824ebe16804f802259e0dfcad0`；
- `isolation-assertions.json`：
  `6f609efb78f0e857e9d1a0be8c7728ea242b76f20aa05ad12394ea6112de4a7e`；
- `network-active.json`：
  `bc14fe4e2d8e2c7f135bcfe1892f078408a10ed42c8b97167601cb4386235624`；
- `inventory-after.json`：
  `3a262659dcebd82c873282d614204ebb85b306d2e327c2e73e55894ca44f86b7`；
- redacted `fake-open.log`：
  `9028ab6c92dfe4c81666214ca88b58e8dc3182f1993535626fed0366a8eac1e2`；
- `post-cleanup-absence.json`：
  `f02f0c3fa59718ab9c46f29fb3e365f1e56571cd9fb9d94987fb314eae15c6c9`；
- closing `hashes.sha256`：
  `57442eda72f1bf4b2dc40ddf44a913e2d077c6668aa6ad54df2a929cd6803394`。

`hashes.sha256` 固定 28 个 evidence 文件；逐项复核为 28/28 `OK`，raw nonce scan 为 0。

## 10. 不外推

本记录不证明 Reviewer/Specialist 服务成功或质量，不证明真实 Anthropic entitlement、真实
provider/model、账号、Web Search、Skill、MCP、SSH、installed App、签名、公证、release，
也不把 global `About you` memory 外推为 project-scoped memory 的全部语义。
