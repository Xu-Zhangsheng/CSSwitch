# CSSwitch 生产链路验收

状态：当前运维合同

适用范围：重要重构决策的 production source、exact artifact、isolated-live、authorized live 验收，以及跨版本累积的 RM 场景目录。每次执行前必须针对目标候选复核 production owner / caller、命令和 gate，并记录 exact commit / artifact、环境、授权与结果；映射或场景存在不表示任一项已经通过。发布附件的既有结果见对应 [release evidence](../evidence/releases/README.md)。

## 1. 安全护栏

- Test 编译期固定使用 `$HOME/.csswitch-acceptance`，正式构建使用 `$HOME/.csswitch`；即使都从 Finder 启动也不得互相迁移、覆盖或读取配置。自动验收仍使用每次全新的独立 `HOME`、独立 Science data-dir 和动态测试端口，形成第二层隔离。
- 准备环境时不读取、修改或删除真实 `~/.claude-science`、任何 Keychain / OAuth、SSH 私钥或真实 `~/.csswitch`。
- Codex OAuth 只写入 Acceptance data root 下的 `codex-oauth.v1.json` 与 `codex-thinking.v1.json`；guard 不创建、不选择、不解锁任何 Keychain。只有用户在 Acceptance app 中明确点击 Codex 登录 / 退出后，才允许写入或删除这些文件；不得读取、覆盖或删除正式 CSSwitch、原生 Codex 的 `~/.codex` 会话或任何 macOS Keychain 项。
- 真实 Science 的 `8765` 端口只用 `lsof` 观察基线 PID，不停止或接管。
- 已安装 CSSwitch 正在运行时，不强退用户实例；构建独立 bundle ID 的 Acceptance app。
- Gateway / Science 端口由 guard 动态分配并避开 `8765`、`1455`、`1457`；Codex 上游 OAuth callback 兼容端口仍固定尝试 `1455` / `1457`，guard 只检查至少一个空闲，不停止占位进程。
- 真实 provider、真实 Claude 登录和真实 SSH server 测试必须单独获得授权。
- 构建 artifact、运行真实 Science 的 isolated-live，以及每个真实 provider / Skill / SSH / 账号 subcase 都是相互独立的授权；source seal、文档修改或相邻 live PASS 不包含这些授权。
- 截图与日志只保留端口、PID、状态码、profile 名称和脱敏摘要，不含 key、path secret 或 nonce。

## 2. 唯一证据链

所有重要重构决策只沿以下顺序晋升，旧 R3–R11、R4/R5、S7、Post-D0/Post-Q0 或其他阶段编号不再提供 NEXT、授权或验收顺序：

| 层 | 必须回答 | 进入条件与授权 | 不能外推 |
|---|---|---|---|
| `production source` | current owner、production caller / auto-boot、failure boundary 与确定性 fixture 是否在同一 exact source 上闭合 | 实时冻结 clean exact HEAD；完成 fresh source review 与完整 source gate | 不能外推可构建、包内身份或 runtime 行为 |
| `exact artifact` | 是否由同一 exact source 生成；Desktop、Gateway、resource、manifest、版本与 hash 是否同源且可核对 | `production source=PASS`；必须另获构建授权 | 不能外推 executable 已运行、Science 可启动、provider 可用或已安装 App |
| `isolated-live` | exact artifact 的 CSSwitch production executable 是否经注册 Tauri IPC / auto-boot 走到真实 Gateway 与真实 Science production entry，并在隔离状态下完成目标 normal wiring | `exact artifact=PASS`；另获真实 Science 隔离测试授权；全新外层 HOME、其下 data-dir/state、假凭证、loopback fixture、动态端口和可归属进程 | 不能外推真实账号、provider、Skill 领域执行、SSH server、installed App、签名或 release |
| `authorized live` | 一个明确 Science/provider/model/Skill/SSH/account capability 是否在授权 scope 内取得真实结果 | 依赖的 `isolated-live=PASS`；每个对象、capability、凭证范围、预算、数据和停止方式单独授权 | 一个 happy path 不代表其他 provider/model/capability，也不证明 crash/race/replay |

每层结果只使用 `PASS`、`FAIL`、`INCONCLUSIVE` 或 `NOT-RUN`，并绑定 exact identity、scope 和证据位置。`PASS` 不能跨层继承；修复导致 identity 或 source 改变时，受影响层及其下游全部失效。installed runtime、signing/notarization 与 public release 仍是额外独立层，不能混入上述四层。

crash window、race、replacement、compensation、replay、CAS drift、partial write 和超时清理属于确定性故障合同。它们必须以 production entry 对应的 fixture / fault injection 证明，并在 source seal 中确认 fixture 没有绕开 owner；不要求用 live 破坏真实状态，也不能用一次 isolated/authorized happy path 冒充。

## 3. 重要重构决策映射

本节是当前唯一的 decision / production owner / caller / failure proof 映射。架构正文拥有稳定机制；本表只把它们接到统一证据链，不复制实现细节。每次 source seal 必须实时复核路径和符号，不能把本表或历史 audit 当作源码事实。

| 重要重构决策 | Production owner 与 caller | Failure boundary 与确定性证明 | Exact artifact / isolated-live / authorized-live 的最低闭合点 |
|---|---|---|---|
| 一键入口、Gateway / Science 启动与 finalize | frontend `desktop/src/runtime-controller.js` → registered `commands::runtime::one_click_login` → `commands/runtime/one_click.rs` → `runtime/sandbox_session/one_click.rs` / `one_click/cold.rs`；Gateway 由 `runtime/proxy_lifecycle.rs` façade，Science 由 `runtime/science/host_adapter.rs`，consumer readback 由 `runtime/finalize_consumer.rs` 拥有 | typed entry/phase/failure、cold/healthy/recovery 分支、启动失败与 finalize replay 使用 `commands/runtime/tests.rs`、`runtime/sandbox_session/transaction_tests/` 和对应 exact ignored fixtures | artifact 必须含同源 Desktop/Gateway/Science scripts；isolated-live 必须由 CSSwitch executable 的 production IPC/auto-boot 贯通真实 Gateway + Science，而非直接调用内部函数；真实 provider/Science capability 逐项授权 |
| runtime mutation 与 stop ownership | `lifecycle.rs::RuntimeMutationLease` 声明 mutation domain；registered `set_mode` / `set_settings` / `stop_all` / `quit_app` 由 `commands/runtime/lifecycle.rs` 编排；registered `clear_profile_key` / `delete_profile` 的 destructive mutation 由 `commands/profiles.rs` 拥有；Codex mutation / downgrade caller 在 `commands/codex.rs`，native-exit caller 在 `lib.rs`；共享 transaction stop owner 位于 `runtime/sandbox_session/transaction_science_stop.rs`，覆盖 cold prior、managed DB restart、profile rollback、history prior stop、live compensation 与 fresh replay 六类 caller | owner claim、锁外 probe/wait、probe 后与 publication 前的 generation + full identity CAS、replacement preservation 由 `commands/runtime/tests.rs` 与 `runtime/sandbox_session/transaction_tests/` 的 race/replacement fixture 证明；downgrade/native-exit 已有各自 production caller fixture，不能再写成开放 sibling gap | artifact 核对注册面；isolated-live 只跑 normal lifecycle，replacement/race 破坏分支保留 fixture；真实 normal stop 需单独 Science 授权 |
| authority finalize、compensation 与 replay | `runtime/sandbox_session/authority_transaction.rs` 拥有 protected capture/restore/cleanup façade；`transaction_science_stop.rs` 只拥有共享 process-local stop owner/CAS；`one_click/cold.rs`、`one_click/cold/compensation.rs`、`one_click/compensation_replay.rs`、`pending_cleanup.rs` 与 finalize/replay entry 各自拥有 durable intent/effect/outcome | crash-before/after intent、step outcome 丢失、CAS drift、fresh replay 与 cleanup ownership 使用 `runtime/sandbox_session/transaction_tests/` 及 fault injection；diagnostic text 不参与控制流 | artifact 要含同源 production executable/scripts；isolated-live 只证明 normal production entry；真实 happy path 可授权，crash / compensation / replay window 不进入 live |
| history full-snapshot recovery | frontend history choice → registered `restore_history_choice` → `commands/runtime/one_click.rs` → `runtime/sandbox_session/history_recovery.rs`；history prior stop 调用共享 `transaction_science_stop.rs`，resume 只消费 exact terminal handoff 后重入既有 one-click owner | sibling config writer、credential interrupt、restore interrupt、stop probe/owner drift、fresh replay 与 exact record CAS 使用 transaction tests / synthetic history fixture；真实用户 history 不是 fixture | isolated-live 使用合成 history + production IPC 完成 restore-only / restore-and-resume；默认不读取真实用户历史，真实账号/history 若确需测试必须另行授权 |
| Science fixed active / pending update adoption | registered `science_runtime_preflight` 只 bootstrap 或读取 fixed active；`lib.rs::start_science_runtime_update_scheduler` → `run_science_runtime_update_once` → `runtime/science/selection.rs` 只发现并发布 pending；registered status/action 与 `desktop/src/runtime-controller.js` 用 expected SHA 查询、keep 或 activate；`runtime/science/host_adapter.rs` 与 `adoption.rs` 拥有 managed-health proof 和 ledger observation | selection writer-lock + exact bytes/identity CAS 保证 pending 与用户动作；source discovery 留在 lifecycle 外；最终 `deferred_healthy` 必须在 lifecycle observed-context 内经过 exact receipt/status/HTTP/listener proof、generation + full-owner CAS 和 writer-lock 内 proof 重验。receipt、daemon、generation、runtime、confirmed-stopped、child PID、port 或 URL 漂移只降级 bookkeeping，保留 pending/replacement，不停止、不重启、不合成 predecessor | source 必须闭合 bootstrap/due/pending/action/stale-result/rollback、owner-race 与 proof fixture；artifact 核对 registered status/action 和事件 wiring；isolated-live 证明健康 A 运行时只出现 B pending、单次事件、无重启，accept/keep 后仍到下一次 cold start 才改变 runtime；真实 updater/Science 另行授权 |
| Science host adapter 与 Skill host bridge | `runtime/science/host_adapter.rs` 拥有 typed launch/stop host projection；本地 picker 经 registered `install_local_skill_package` → `commands/skill_install.rs`；显式 route repair 经 registered `repair_skill_route` → `commands/diagnostics.rs` → `force_third_party_reconcile`，两者都使用短 `HostBridge` lease。外部 GitHub Skill 的 production caller 是 Science Agent → managed `csswitch-skill-installer` MCP；`runtime/skill_install_bridge.rs` 注册 connector，打包 Gateway 的 `skill-install-mcp` / `install_external_skill` 实现在 `desktop/gateway/src/skill_install.rs` | host receipt/context drift、attach partial success、route-state marker persistence 与 cleanup 用假 Skill package、loopback tool/poll 和 fixture Science data-dir；source fixture 不声明 Science Agent 重启后的 Skill load persistence，不得访问真实 Skill root，也不得直接调用 installer helper 绕过 Agent/MCP route | artifact 核对 host scripts、managed route/connector 与 installer resources；isolated-live 分别从 local picker、repair IPC 和 Agent/MCP production caller 记录 install、attach、load、trigger、restart、uninstall、detach，并证明 Agent load 的重启持久性；真实 Skill / domain execution 每项授权 |
| provider protocol capabilities | profile selection/connection 由 `commands/profiles.rs`；one-click 将 effective profile 交给 `runtime/proxy_lifecycle.rs` façade；`runtime/provider.rs` 通过 `provider_contracts.rs` 生成并绑定 typed launch plan，Desktop 与打包 Gateway 共同编译、校验 `catalog/provider-contracts.v1.json` 的 exact contract id/digest；`runtime/capability_catalog.rs` 加载的 `catalog/capabilities.v1.json` 仅拥有 diagnostics/evidence rules，不是启动合同 source of truth；provider adapter 在打包 Gateway | malformed stream、tool loop、reasoning/signature、error classification、cancel/retry 等以 deterministic upstream fixture 证明，不能让 live provider 承担故障注入 | artifact 核对 Gateway identity 与 `provider-contracts.v1.json` exact digest，并把 diagnostics catalog 分开记录；isolated-live 用 loopback provider fixture 经真实 CSSwitch → Gateway → Science；authorized-live 按 provider + model + stream/tools/reasoning/error 分项，不以 text PASS 汇总 |

本表未给任何行写入 PASS。当前 actual 缺口只在[known issues](../../.agents/context/known-issues.md)登记；执行结果进入绑定日期、SHA、artifact 和环境的 audit/evidence。

## 4. Source 自动化基线

```bash
GATE_ROOT="$(mktemp -d /private/tmp/csg.XXXXXX)"
chmod 700 "$GATE_ROOT"
bash test/run_all.sh --output-root "$GATE_ROOT"
```

先在 clean exact-HEAD source 候选上取得完整 15-suite `GATE-SOURCE` completion seal，
记录命令、退出码、HEAD、输出目录和 suite 结果；局部组件结果不能替代。该 gate 只
建立 source/unit 证据，后续 Acceptance artifact、installed/runtime 与 live 场景仍
按本矩阵分别取证。Python 仅供测试驱动与 mock 使用；产品 runtime proxy 是 Rust
sidecar。

## 5. Exact artifact 构建

以下构建步骤只有在用户对该 exact candidate 明确授权后才允许执行；Authority migration、source seal 或文档门禁都不包含构建授权。

```bash
DEV_HOME="$HOME"
(
  cd desktop
  PATH="$DEV_HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH" \
    npm run tauri build -- --features acceptance-build --config ../test/tauri.real-machine.conf.json --bundles app
)
```

目标为 `desktop/src-tauri/target/release/bundle/macos/CSSwitch Test.app`。`acceptance-build` 是编译期 Test data-root feature：Desktop 与 Gateway 分别固定 `$HOME/.csswitch-acceptance`，build script 用同一 feature 重建并打包 Gateway sidecar。

任何构建只要存在 `CSSWITCH_SKIP_GATEWAY_STAGE` 都会直接失败；普通构建也不得复用 Acceptance 残留，Desktop 与 Gateway 必须由同一次同 feature 构建产生。artifact 验收要核对包内 Gateway 存在、可执行、与 Desktop 同次构建，并在全新隔离 `HOME` 执行包内 `csswitch-gateway codex-auth status`，确认退出码为 `0`、返回 `reason=state_missing` 且不生成状态文件，不能只证明文件存在。正常构建不启用 Acceptance feature，固定 `$HOME/.csswitch`；Acceptance 固定 `$HOME/.csswitch-acceptance`，两种构建都没有运行时改写入口。必须在导出隔离 `HOME` **之前**构建；否则 `$HOME/.rustup` 会指向空的测试 HOME。

### 5.1 历史共享根候选

2026-07-17 早期 Acceptance 候选曾错误共享正式 `$HOME/.csswitch`；该候选已经被编译期隔离根方案取代，不属于当前构建、安装或恢复步骤。历史影响与当时停线边界只在[日期化 Acceptance 证据](../evidence/investigations/2026-07-17-codex-browser-only-acceptance.md)中保留。当前流程不得寻找、复用或操作旧共享根候选，也不得据此读取或修改真实配置。

## 6. Isolated-live 准备与启动

每轮使用新的 root，避免覆写上一轮验收证据：

```bash
export CSSWITCH_REAL_TEST_ROOT="${TMPDIR:-/tmp}/csswitch-codex-acceptance-$(date +%Y%m%d-%H%M%S)"
bash test/real_machine_guard.sh preflight
```

guard 会持久化本轮随机端口；后续命令无需手填固定端口。Codex 验收在隔离 HOME 的 `.csswitch-acceptance` 中使用空的 v3 fixture：

```bash
bash test/real_machine_guard.sh prepare-codex
```

`preflight` 不执行任何 Keychain 命令。`prepare-codex` 只写入隔离 `HOME`，Codex 实验开关保持关闭，且不写 profile、token、credential ref 或 OAuth 文件。若 config 已存在会拒绝覆盖。

只有验证 RM-01 v1 -> v2 迁移时才准备 legacy fixture。该步骤要求两个非空变量；使用明确的假值，不要读取或写入真实 provider key：

```bash
DEEPSEEK_API_KEY='csswitch-migration-fixture-deepseek' \
DASHSCOPE_API_KEY='csswitch-migration-fixture-qwen' \
  bash test/real_machine_guard.sh prepare-legacy
```

随后才把当前 shell 切到 guard 生成的隔离运行环境：

```bash
eval "$(bash test/real_machine_guard.sh env)"
```

验证 SSH opt-in 时，在这个隔离 HOME 内创建空的普通 config fixture；它只用于 wrapper / fail-closed 合同，不证明真实服务器连通：

```bash
install -d -m 700 "$HOME/.ssh"
install -m 600 /dev/null "$HOME/.ssh/config"
```

启动独立 Test app：

```bash
HOME="$HOME" CSSWITCH_REPO="$CSSWITCH_REPO" \
  "$CSSWITCH_REPO/desktop/src-tauri/target/release/bundle/macos/CSSwitch Test.app/Contents/MacOS/desktop"
```

### 6.1 Codex 的停线点

首次启动后先完成 RM-42 的 bundle ID、隔离目录、端口和 `8765` 检查。打开“高级”确认 Codex 实验开关默认关闭；此时诊断必须报告 `auth=not_checked`，且不能因查看页面而读取 OAuth 文件或启动 OAuth。

**环境准备到这里停止。** 只有用户本人在场、先记录原生 Codex 登录状态并明确继续后，才打开实验开关并点击“登录 Codex”。浏览器授权、live 模型和退出 / 重登属于 RM-35～RM-40，不能由自动测试代替，也不能把“页面可见”写成 OAuth 已通过。

正式 DMG 验收应从只读挂载的 app 复制到隔离位置，并且不设置 `CSSWITCH_REPO`；不能拿源码 build 代替最终 artifact。

`preflight` 应记录 8765 基线、创建隔离 HOME 并确认测试端口可用。每次改变运行态后执行：

```bash
bash test/real_machine_guard.sh guard
```

若 8765 PID 变化，或真实用户目录被碰触，立即停止并把该次验收记为失败 / 证据污染。

## 7. RM 场景目录（非路线）

RM-01～RM-34 保留历史编号；Codex 场景从 RM-35 继续，0.8.1 新增 provider / Codex / 会话恢复回归沿用后续编号，避免源码注释和旧证据错指。RM 编号只是跨版本场景引用，不是阶段路线、NEXT 或授权；实际执行必须先落到第 3 节的某项重要决策和证据层。矩阵存在不表示最终公开 DMG 已逐项通过。

| ID | 场景 | 操作 | 必须满足 |
|---|---|---|---|
| RM-01 | v1 -> v2 迁移 | 用假 key fixture 首次启动 | DeepSeek / Qwen profile 与 `active_id` 当前选择正确；`config.json.v1.bak` 为 `0600`；key 只显示掩码 |
| RM-02 | 新建 profile | 新建后分别取消 / 完成 | 取消不落盘；完成新增且不自动生效；同模板可多条 |
| RM-03 | 元数据编辑 | 改名和备注后重启 | 名称 / 备注持久；连接字段与 key 不变 |
| RM-04 | 非 selected / applied 连接编辑 | 对既非当前选择、也非上次应用的 profile 使用正确 key、错误 key / model、405、429 / 5xx / 断网 | 2xx 标已验证；401 / 403 与 400 / 404 / 422 明确拒绝且不落盘；405、429 / 5xx 与无响应可保存但标未验证；当前选择、binding 与运行链不变 |
| RM-05 | 当前选择切换 | DeepSeek ↔ Qwen 后观察运行链，再执行下一次一键开始 | `active_id` 只记录 selection；既有 Gateway / Science 不因选择动作切换；下一次一键开始才校验并应用候选，UI 区分 selected / applied |
| RM-06 | 候选应用失败 | 选择错误 key / model 的候选并执行下一次一键开始 | 不把候选发布为 applied / Ready；selected 与 applied 继续分开；旧 binding 或 unknown/manual 状态只按 typed outcome 与 exact readback 发布，不用文案伪造回滚成功 |
| RM-07 | selected / applied 连接编辑 | 对 selected-only、applied-only 与 selected=applied 分别保存有效、401 / 404、405 与含糊网络结果，再对当前选择执行下一次一键开始 | 编辑不热切换既有 Gateway / Science；有效值保存，401 / 403 与 400 / 404 / 422 不落盘，405、429 / 5xx 与无响应可保存但标未验证；selection 与旧 binding 在保存时不变，只有后续一键开始可应用当前选择并发布新 binding |
| RM-08 | 一键开始 | 连续点击两次 | 首次启动 Gateway + Science；再次幂等复用并 reopen；UI status 只按 health 解释 |
| RM-09 | 整链推理 | 经授权发送 minimal text 与 tool request | 实际 provider / model / tool 结果分栏；日志无 path-secret / key；8765 PID 不变 |
| RM-10 | 清 key（selected × applied） | 分别清 selected-only、applied-only、selected=applied 与 neither 的 key | 所有角色都只清目标 key且保留 `active_id` selection；applied-only 与 selected=applied 还必须清 `runtime_binding` 并停止 tracked Gateway，selected-only 与 neither 不影响当前链；backup 不可恢复旧 key |
| RM-11 | 删除 profile（selected × applied） | 分别删除 selected-only、applied-only、selected=applied 与 neither | 删除 selected 清 `active_id`；删除 applied 清 `runtime_binding` 并停止 tracked Gateway；角色分离时删除一侧保留另一侧及其相应运行状态；neither 消失且链不变；不留下悬空引用 |
| RM-12 | 端口变更 | 运行中修改 Gateway / Science port | 先停受管链再保存；旧端口释放；下次按新端口启动 |
| RM-13 | 端口冲突 | 预占候选端口 | 明确报占用；不误报 key；不杀未知占位进程 |
| RM-14 | 官方模式 | 第三方链运行时切换 | 只停测试 Gateway / Science；真实 8765 不变；切回不自启 |
| RM-15 | 全部停止 / 退出 | UI 停止后退出 | 据实报告；测试端口释放；无残留受管 desktop / gateway 子进程 |
| RM-16 | 重启恢复 | 同一隔离 HOME 重开 | profiles / 当前选择 / notes / ports 持久；不自动启动；恢复不能仅凭端口冒认 applied runtime |
| RM-17 | 包资源 | 从 `.app` 与挂载 DMG 启动 | `Contents/MacOS/{desktop,csswitch-gateway}` 与 `Contents/Resources/scripts` 齐全；无旧 `Resources/proxy`；正式包无需 `CSSWITCH_REPO` |
| RM-18 | 发布安全 | hash、codesign、spctl、stapler | 签名完整性、身份、公证、ticket、Gatekeeper 分栏；不把 ad-hoc 写成已公证 |
| RM-19 | updater runtime 优先 | 固定真实 HOME updater、App 与 stale 隔离 cache 同时存在 | 通过路径、属主、权限、Mach-O、embedded identity 校验后生成 CSSwitch 私有 SHA-256 snapshot，选择来源 `official_updated` 并复用 CSSwitch data-dir；只读取固定 executable，不读取或改写真实 HOME 的其他 Science 数据；不得把 embedded metadata 写成官方来源密码学证明 |
| RM-20 | explicit / updater / cache preflight | 合法 / 非法 `SCIENCE_BIN`，身份/权限/路径合法与非法的 updater，App 缺失与 cache 组合 | override 无效 fail closed；检测到非法 updater 时显式报错，不静默回退旧 App；cache 仅版本可读时提供 one-shot；选择不持久化 |
| RM-21 | Science active / pending 升级、冷启动采用与强身份 | bootstrap fixed active A；A 健康运行时让 24h due check 产生 pending B，并重复 scheduler/status；在独立 fixture 中分别用 stale / exact pending SHA-256 执行 `keep_active` 与 `activate_pending`，并注入发布后 snapshot 复核或目录持久化失败；另注入 receipt/daemon/generation/full-owner 漂移；接受后先 healthy reopen，再 exact stop → next cold start → reopen/recovery/stop；运行中替换 source，并模拟 stop CLI 返回 0 但 listener 未退出 | 后台检查只创建 pending，active A 与健康 daemon 不变；只有首次创建该 pending 时发送一次事件，pending 等待、status 与后续 hourly wake 不重复提醒或探查，action 已消费 pending 时旧 scheduler payload 不得重显。只有 exact managed receipt + status + HTTP health + listener + generation/full-owner CAS 可写 A→B `deferred_healthy`，证明失败保留 pending 且不得形成 A predecessor。用户动作必须匹配 exact pending SHA-256；stale hash 不修改 selection，`keep_active` 有界记录拒绝 hash，`activate_pending` 只改变下一次 cold start 的 active。写后复核或目录持久化失败按刚发布的 exact identity/bytes 做 CAS rollback，恢复 prior active+pending 并要求 UI refresh；两种动作均不热重启。下一次 cold start 才以 B 建立 selected attempt，并要求 action、V2 receipt、runtime_binding、finalize intent/runtime、reopen 使用同一 attempt id；receipt 单独存在最多恢复到 launch_committed，binding/finalize CAS 后才记 finalized。启动/reopen/recovery/stop 核对 PID、含 SHA-256 的 binary fingerprint、data-dir、port；source 替换不改变既有 snapshot；CLI 假成功时只终止前后均精确匹配的 PID并确认端口关闭；UI status 仍只代表 HTTP health |
| RM-22 | Skill Agent 控制面 | 首次配置、重复启动、注入中途失败 | 管理固定 route / connector / `customize` / prompt；成功 marker 后跳过重复；失败 warning 且如实报告可能的部分配置 |
| RM-23 | 外部 Skill 安装 | exact artifact 的 Science Agent 经 managed `csswitch-skill-installer` MCP 调用 `install_external_skill`；fixture 使用精确公开 GitHub 形态的 URL | Agent request → packaged `skill-install-mcp` → host approval → commit → native attach → `skill()` load，各阶段分开记录；不得直接调用 installer helper |
| RM-24 | Skill 重启 / 卸载 | 同 data-dir 重启，再卸载 | 重启仍 load；只 quarantine 有 marker 的导入；native detach；不走 catalog / shell |
| RM-25 | 运行中 Skill 配置漂移 | Science 运行时改变 MCP / route 预期 | 只读检查并返回 `RESTART_REQUIRED`；不并发改写；普通 Science 继续 |
| RM-26 | 系统 SSH 默认 / opt-in | 无 fixture、创建 fixture、再移除 fixture | 默认关闭不阻断；启用时 wrapper 使用 `/usr/bin/ssh -F`；启用后 config / wrapper 缺失必须 fail closed |
| RM-27 | SSH 非目标 | 检查文件与监听状态 | 不复制 `.ssh`、不启动 `sshd`、不改防火墙、不监听 `0.0.0.0`；真实 server 另行授权 |
| RM-28 | GitHub 单请求进度 | 固定 commit 的慢速 bundle 安装 | 只生成一个 request；archive / fallback 复用同一 ID；进度持续更新；最终 response 唯一；status 与 `.processing` 清理 |
| RM-29 | GitHub 重复复用 | 再安装 RM-28 的同一 URL | 返回 verified reuse；不重新下载、不重复提交、不覆盖已装内容；OPERON 绑定回读仍正确 |
| RM-30 | GitHub 失败收口 | 网络失败、无效 commit、gateway 中断恢复 | 返回结构化终态；不自动重试；不留下部分 Skill；遗留 processing 在重启后转为 interruption 响应并清理 |
| RM-31 | 本地包导入 | picker 取消；再导入单 Skill 与带 `_shared` 的 bundle ZIP / `.skill` | 取消不提交；前端不取得路径；单包 / wrapper / bundle 正确识别、校验、原子提交并绑定；同 archive 重复导入快速复用 |
| RM-32 | bundle 卸载取消 | 从任意成员发起卸载并取消 | 首次只返回 bundle 名称、完整受影响 Skill 列表和确认 ID；不 detach、不移动、不写 quarantine；取消后无第二次工具调用 |
| RM-33 | bundle 整包确认 | 重复 RM-32 并明确确认 | 精确 confirmation ID 校验；全部成员批量 detach 并整包 quarantine；不残留部分物理安装；不提供成员级删除 |
| RM-34 | v0.5.0 干净升级 | 旧 route / split connector、用户 MCP / 未知字段、已装 GitHub Skill 与新本地 ZIP 组合 | 迁移到合并 connector；用户 MCP 与未知字段保留；重启恢复、重复安装、GitHub / ZIP bundle 整包卸载均按 v0.6 合同工作 |
| RM-35 | Acceptance artifact + 用户 OAuth 后 live provider | 独立 Codex 登录 | 只由脱敏 `codex-auth status` 证明 Acceptance data root 凭据存在，不读取或输出文件内容；登录成功后 Codex profile 自动出现但当前选择不变；正式 CSSwitch 与原生 Codex 登录前后状态不变；无 token 证据泄漏 |
| RM-36 | 用户 OAuth 后 live provider | 动态多模型 | 当前账号至少返回两个可用模型；若目录返回 Sol/Terra/Luna，则 CSSwitch 与 Science 分别显示 `Codex / GPT-5.6-Sol`、`Codex / GPT-5.6-Terra`、`Codex / GPT-5.6-Luna`；请求 alias/raw id 与 Gateway 脱敏观测一致，缺失模型不伪造 |
| RM-37 | 用户 OAuth 后 live provider | 流式文本与 reasoning | 增量顺序、thinking、usage 和终态正确；CSSwitch Gateway 不持久化对话，Science 自有项目 / 对话持久化不属于失败 |
| RM-38 | 自动 mock + 用户 OAuth 后 live provider | 工具调用 | tool id / result 严格闭环；真实最小工具成功；断流 / 取消不重复执行由 mock 故障注入证明 |
| RM-39 | 自动 mock | 刷新与失效 | fake OAuth / secret store 强制 401 和 CAS；并发刷新单写者；401 只影响下一请求；不破坏真实 token |
| RM-40 | Acceptance artifact + 用户 OAuth 后 live provider | 退出与重登 | 只删除 Acceptance namespace 项；正式 CSSwitch、原生 Codex 与其他 provider 不变；只用脱敏 status 观测 |
| RM-41 | 自动 fixture + Acceptance artifact | v3 降级 | 每个 Codex profile 显式处理；API-key profiles、端口和设置完整；Codex network 字段按合同丢弃；OAuth 文件不变且不读取其内容 |
| RM-42 | Acceptance artifact | 隔离打包 | 独立 bundle ID、编译期 `$HOME/.csswitch-acceptance`、私有文件 OAuth 且不调用 Keychain；Finder 启动不读写正式 `$HOME/.csswitch`；Gateway / Science 使用动态端口，OAuth callback 仍固定 `1455` / `1457`；`8765` 与已安装 App 不变；收尾无残留进程 |
| RM-43 | Acceptance artifact | Finder 无代理环境 + 系统 TUN | Finder 启动显示 `direct`，只说明“直接 socket，可能由系统 TUN 接管”；TUN 下浏览器登录成功但不声称检测 TUN |
| RM-44 | Acceptance artifact + 本地 fixture | 显式代理 | HTTP CONNECT 与 SOCKS5h 分别完成浏览器 token exchange、模型目录与最小推理；SOCKS5h 证明域名在代理端解析；production 不注入自定义 CA |
| RM-45 | Acceptance artifact | 登录取消 | browser callback wait、慢 callback header、token exchange 取消在两秒内终态；pre-commit 取消后 generation 与 Acceptance OAuth 文件状态不变；committing 返回 `commit_in_progress` |
| RM-46 | Acceptance artifact + 本地 mock | 新 Provider 配置 | OpenCode Go 两种 transport、Grok、Gemini 分别完成 scratch discovery、显式选择/手填与保存；探测不写正式配置；OpenCode 已知模型不跨协议且上游只收到裸 ID |
| RM-47 | 用户 key 后 live provider | Provider 最小 Science E2E | 对每个已授权且 credential 非空的 profile，从 CSSwitch 一键切换贯通 packaged Gateway 与真实 Science，分别记录 selector、最小文本、必要的工具/搜索、provider 错误和停止；OpenCode Go、Grok、Gemini 仍另记 `/models`、标题、classifier 与普通两轮工具。每个 provider/model 独立分栏；缺 key、余额、Science compute 环境或 provider 瞬时错误不得借相邻 PASS 补齐，也禁止自动重试 |
| RM-48 | Acceptance artifact + 用户 OAuth 后 installed Science | Codex 标题与记忆 | 三个标题入口保存普通文本而非 JSON 编码字符串；Sonnet classifier fallback 成功后用 Science SQLite 的新增行证明记忆真实写入；classifier 不可用时仍 fail closed |
| RM-49 | Acceptance artifact + 用户 key 后 live provider | K3 多轮恢复 | 第一轮 reasoning + tool、第二轮 tool result 后继续；有效 opaque signature 可恢复，改动 reasoning/tool args/profile 后本地拒绝且不发上游 POST |
| RM-50 | Acceptance artifact + 本地 fault mock；live provider 另授权 | Kimi / DeepSeek 会话失败 | Kimi 完整 envelope 保留 signed thinking、usage、stop reason 并压紧 index；畸形/截断只发一个 terminal error 且无 `message_stop`；DeepSeek native 与 DSML detect/rewrite/off 不触发 Kimi 规则 |
| RM-51 | v0.8.0 隔离 fixture 升级到 Acceptance artifact | 历史记录恢复 | 完整登录自动补私有 marker 且不改 Science 凭证；退出后恢复同一 org；无 marker 且多 org 时只显示不透明 A/B 选择，选择前后均不删除、猜测或重写其他历史，旧 ref 重放失败 |
| RM-52 | Acceptance artifact + 当前 installed Science + 用户授权的真实 SSH config | SSH 前置校验 | 开启复用后 Science 能识别真实 config 中的 Host；关闭后只移除 CSSwitch 精确管理的 sandbox stub；不连接真实 server 也能完成前置校验，真实连通性另行授权 |

## 8. Skill 证据词汇

外部 Skill 至少分为：content fetched、目录 committed、Science discovered、Agent attached、`skill()` loaded / triggered、领域功能完成、重启持久化、quarantine、detached。不能用一个“安装成功”覆盖所有层。

bundled route 必须使用 `mcp-csswitch-skill-installer` 的 `install_external_skill` / `uninstall_external_skill`，不得回退到 `customize`、`host.skills.*`、shell 或手工文件删除。

## 9. Artifact 检查

对最终候选分别记录：版本、大小、SHA-256、包内 executable / resources、Gateway 可执行性、空 data root 的脱敏 status，以及是否发生 Keychain 访问。若分发者另外执行签名、公证或 Gatekeeper 验证，应作为独立分发证据记录；这些项目不是 Codex 功能验收前置。

## 10. 收尾

在 UI 停止链路并退出验收 app 后运行：

```bash
bash test/real_machine_guard.sh assert-stopped
```

确认测试端口释放、8765 PID 不变、真实用户目录未改、已安装用户 app 未被替换。若执行过 Codex 登录 / 退出，另外用原生 Codex 自己的脱敏 status 复核其会话仍在；不得用读取原生 token 文件作为证据。每项如实标为通过、失败、环境阻塞、未执行或需人工判断。
