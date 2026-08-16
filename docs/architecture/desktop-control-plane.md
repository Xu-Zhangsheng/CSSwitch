# Desktop 控制面

本文回答“WebView、Tauri command/event、DTO 与错误怎样跨越桌面控制面”。状态与补偿由[运行时状态与事务](runtime-state-transactions.md)维护；Gateway 协议由[Gateway 与 provider 路由](gateway-provider-routing.md)维护。

## 可达层定义

| 层 | 定义 |
|---|---|
| `compiled` | 进入当前 Rust module graph 或 frontend bundle |
| `registered` | compiled 后进入 Tauri invoke handler 或 event 面 |
| `product-reachable` | 当前生产 frontend、已启用的条件 auto-boot 或受管 runtime 存在调用路径 |
| `test-only` | 只从 `cfg(test)`、ignored E2E、preview/mock 或测试 helper 到达 |
| `legacy/orphan` | 文件存在，但没有进入当前 module、invoke 或 frontend 产品图 |

`production-source` 是保守的变更影响分类，不能替代这些层。

## 边界与依赖方向

```text
WebView
  -> invoke(command, DTO)
  -> Tauri command
  -> command-specific boundary
     -> Lifecycle serializer（runtime/profile/mode/Skill route repair）
     -> picker 后短 HostBridge lease + typed host receipt + Skill package transaction（本地 Skill 安装）
  -> Config / AppState / package-private state / Gateway / Science

Tauri backend
  -> event(payload)
  -> WebView listener
```

`desktop/src-tauri/src/lib.rs::run` 是 command 注册权威入口。frontend 可达性必须检查生产 bundle 的全部调用模块：`desktop/src/main.js` 是装载、注入和主要 `call()` / listener 入口，动态导入的 `desktop/src/skill-page.js` 等模块也可以持有真实 `call` 并形成 production caller。preview 的 `mockInvoke` 只模拟 DTO，不算生产 caller。

## 当前源码 owner

控制面按维护原因落在以下 owner；根文件是组合或注册面，不重新拥有子模块实现：

| 边界 | 当前源码 owner |
|---|---|
| frontend bootstrap、共享 busy/activation/page/feedback 状态 | `desktop/src/main.js` |
| preview/mock adapter | `desktop/src/preview-adapter.js` |
| Tauri invoke/event/window transport | `desktop/src/ipc-client.js` |
| Codex OAuth、network 与 downgrade 交互 | `desktop/src/codex-controller.js` |
| runtime lifecycle、status 与一键交互 | `desktop/src/runtime-controller.js` |
| profile、catalog 与表单交互 | `desktop/src/profile-controller.js` |
| Tauri runtime command façade 与 crate-facing command surface | `desktop/src-tauri/src/commands/runtime.rs` |
| command 实现 | `commands/runtime/actions.rs`、`gateway.rs`、`lifecycle.rs`、`one_click.rs`、`status.rs` |
| command 测试身份 | `commands/runtime/tests.rs`；仍保持 `commands::runtime::tests::*` |

`commands/runtime.rs` 只保留 command attribute、签名、转发和必要 re-export；新增或
移动实现时仍须从 `lib.rs::run`、frontend caller、DTO、event 与测试 identity
整条链复核，不能只检查 façade。

## command 面

当前注册面按公共职责分组：

| 组 | 当前生产入口 |
|---|---|
| profile / settings | 配置读取、创建/编辑/删除、当前选择、模式和端口/SSH/Codex 设置 |
| runtime | 一键开始、恢复选择、停止、runtime preflight、状态、URL 和退出 |
| model / provider | 模型发现与连接验证所需的生产调用 |
| Codex | 开关、网络、auth operation、profile、logout 与 downgrade export |
| Skill | 本地包安装与已安装 Skill 列表 |
| diagnostics | 只读 doctor、显式 Skill route repair、版本、release/issue/log 入口 |

大多数 runtime/profile/mode 复合 mutation 进入 `Lifecycle`，但这不是所有 Desktop
写操作的统一锁。生产 `install_local_skill_package` 让文件 picker 保持在 lease 外；
选择完成后取得短 `HostBridge` mutation lease，重新探测 matching
`ScienceHostContext` 并构造 lease-bound `LocalSkillHostReceipt`，再用同一 receipt
完成 package commit 与 Science attach/readback。该合同闭合的是本地 Skill 的
最终 runtime-context race，不把 picker/download 放进锁，也不建立跨进程或全局 durable journal。

当前注册面没有已知的无 bundled production caller command；内部 preview/helper 不注册为产品 IPC。

S6 已移除无 bundled caller 的 `start_proxy` Tauri command；formal Gateway 只能由现有
cold、healthy reopen 或 interrupted-Gateway recovery 内部流程经 `GatewayController` 启动或复用，不再暴露
独立的 Gateway-only invoke mutation。

生产 frontend 没有发现 literal command 调用未在 Tauri 注册的反向缺口。

外部 GitHub Skill 的源码控制面已经覆盖配置、安装与 `OPERON` attach；这只能证明
`SOURCE-CONTRACT`。Agent 在最终 artifact / installed runtime 中实际 load Skill、
调用 install/poll tool、完成卸载以及 restart 后继续可用，当前均为 `UNKNOWN`。

## 非一键 mutation 的 command/DTO 边界

会改变 runtime 或 applied profile authority 的七个非一键 destructive command 通过独立的
`config-mutation-operation.v1.json` receipt 与 `config_mutation_operation` Config fence
串行化：`set_mode_official`、`set_settings_destructive`、`codex_auth_start`、
`codex_auth_logout`、`set_codex_network`、`clear_applied_profile_key`、
`delete_applied_profile`。Rust command 只向 frontend 返回脱敏的 typed outcome 或 typed
command error；frontend 通过 `runtime-mutation-protocol.js` 严格解析 operation、status、
effect summary 与 recovery disposition，不把 receipt、fence、credential 或私有路径带过
invoke 边界。receipt 或 fence 任一存在时，普通 writer、P2-A journal 与 runtime journal 的冲突统一返回
可识别的 attention/error，不能由 UI 自动重试或把旧状态显示为已应用。

`set_active_profile`、`update_profile_connection` 与 `codex_ensure_profile` 是 intent-only
结果：它们报告 selected/updated/ensured 的 durable intent，不启动或停止 runtime，也不创建
P2-B receipt。preview adapter 只镜像这些 typed DTO 与旧字段，不扩大 production caller。
每个真实 effect 在执行前先持久化带新 `attempt_id` 的 `InProgress`，返回后再持久化 exact terminal
outcome；checkpoint 失败统一保留 durable attention。Codex auth start 的 sidecar 先等待匹配
operation id、authorization digest 的 start control；Gateway flush `start_ack` 后才授权执行，ack
之前不打开 OAuth/network flow。前端严格接受 `terminal + durable_receipt` attention，并仍通过既有
`codex-auth://operation` snapshot 观察脱敏状态。

## event 面

| Event | 发出方 | Payload | 当前边界 |
|---|---|---|---|
| `boot://publication` | auto-boot coordinator | `{sequence,state,payload}` | `sequence` 在进程内单调递增；`state=failed|attention` 时 payload 保留完整一键 DTO，其他 state 为 `idle|starting|ready` 且 payload 为空 |
| `codex-auth://operation` | Codex command | typed operation snapshot | sequence/state/error 等结构化字段保留 |

frontend 先监听 `boot://publication`，再读取非消费式 `boot_snapshot`。两条路径来自同一 `BootPublication`，使用相同 `sequence/state/payload` envelope；frontend 只接受更大的安全整数 sequence。因此 listener 建立前丢失的 event 可由 snapshot 补读，snapshot 与并发 event 的重复或倒序交付也不会重复展示。

`get_config` 只读 canonical v4 config，不迁移、不归一化写回、不创建 writer fence，也不消费 `pending_notice`。返回值同时携带 notice 展示文本与确定性内容 identity；frontend 展示后显式调用 `acknowledge_pending_notice`。ack 只在 identity 仍匹配时清除 notice；重复 ack 成功幂等，旧 identity 不会清除并发产生的新 notice。启动时的 legacy migration 仍由 setup 的独立 config load owner 完成。

## 一键 DTO 与错误投影

手动 `one_click_login` 有三种 resolved result，另有一种 invoke rejection：

1. 成功或已复用：resolved object；
2. 需要历史选择：resolved `status=attention` object；
3. 普通失败：resolved `status=error` object，包含 `stage`、`recovery_status`、`environment_status` 等；stage 来自内部 `OneClickFailureKind` 投影（`runtime/failure.rs`），**不**扫描 message 文案；
4. Codex typed auth 等 command error：invoke rejection，由 frontend `catch` 处理。

内部 operation trace、持久 journal 与 frontend stage 不是同一枚举。frontend
coarse stage 由 `OneClickFailureKind` 在产生点投影，不扫描 message 文案；当前 runtime
journal writer 使用 typed V2 operation/phase/outcome，V1 只保留 fail-closed 兼容读取与原
wire round-trip。typed journal phase 也不能与 UI coarse stage 无损互映。auto-boot 的
`boot://publication` / `boot_snapshot` 中 `state=failed` 的 payload 与手动一键共享 failed DTO shape；recovery/environment
status 从 typed failure/compensation projection 产生，message 只用于展示。

H4 在 one-click DTO 与 consumer publication 之间增加 `finalize_consumer_state`：它只读 canonical
config，按结构化 `status + recovery_status + action` 与实际 journal/binding readback 返回
`ready | attention | manual`、`open | cleared`、binding relation，以及仅在 `ready` 时可发布的
applied profile id / selection pending；`attention/manual` 固定发布 unknown。该命令不迁移配置、
不 chmod、不清 notice、不返回 journal record、path、credential 或
其他配置内容。manual UI 与 auto-boot 复用同一个 Rust classifier；journal open、回读失败或
不一致组合不得发布 applied/`BootState::Ready`，message 仍只参与展示。
auto-boot publication 的 failed/attention event 与同 sequence snapshot 补读路径也必须把 frontend applied 展示发布为
unknown；完整原 DTO 仍单独保留用于错误和 history choice 展示。

## 选择、应用与诊断语义

- `set_active_profile` 只提交“当前选择”；运行中的 Gateway/Science 不立即切换。下一次一键开始才应用并写 runtime binding。
- history attention 后，每份选择都有“仅恢复”和显式“恢复并启动”。两者都只提交一次
  `restore_history_choice` destructive IPC：restore-only 经 typed history journal / protected snapshot /
  cleanup finalize 后保持 stopped；restore-and-resume 由 backend 发布并消费 exact terminal handoff，
  再进入既有 one-click owner。用户以后单独点击「一键开始」仍是另一个 operation；frontend 不自动
  串联第二个 IPC，也不把 history DTO 当 applied/ready 证明。若重启遗留
  `HistoryCredentialWritePending`，one-click command 会先重验当前 Science quiescence 并收敛
  credential before-image；若该 journal 与 lock-free provider auth 竞态出现，则在 auth failure 返回前
  或 business entry 前收敛。失败保留 exact journal 并投影 manual recovery。
  history journal 还冻结去除 journal 后的完整 Config authority fingerprint；任一 sibling mode、port、
  SSH setting 或 profile 漂移都会阻止 credential commit/finalize/resume。credential commit 后的降级结果
  返回 nested rotated choices，但不返回私有 snapshot 路径；restore-only 也必须经 consumer readback，
  只有 `ok + attention` 才显示恢复成功。
- `status` 是轻量状态投影；Science 灯的 HTTP health 不证明 listener/runtime 强身份。
- `finalize_consumer_state` 是 one-click 完成后的窄、脱敏、只读投影；它不探活、不写配置，也不
  成为新的 transaction owner。history choice 即使伴随 cleanup warning 也保持 attention；normal
  cleanup 只有 readback 确认 exact active binding 且 journal cleared 才可发布 ready；attention、
  manual 或回读失败一律清除 frontend 的 applied 展示并保持 selection pending。
- Doctor 是两个独立 intent。`run_doctor_read_only` 只读 canonical v4 config、在清空继承环境后
  仅注入固定 `PATH`、canonical config/Science/Gateway 路径与脱敏状态；script 与 Gateway 的开发回退
  也只沿当前 executable ancestry 查找，不消费父进程 `CSSWITCH_REPO` / `CSSWITCH_GATEWAY_BIN`，再运行 `doctor.sh` 并投影
  Codex 最近一次内存观察；生产入口强制关闭真实 Science HOME 检查，旧 schema 只报错，不迁移、chmod、清 notice，
  也不取得 mutation lease、修改 route 或为诊断启动 Science/Gateway。`repair_skill_route`
  只在用户显式触发后取得 `HostBridge` mutation lease，强制 reconcile 第三方 Skill route；
  它可失效 route marker，或在健康 Science 上通过一次性 control sidecar 同步 route Skill、
  connector 与 managed prompt，但不会启动受管 Science 或正式 Gateway 服务。两个 command 各自返回 typed result；frontend
  每个动作只提交一个 intent 并渲染 `status/message`，不串联或编排事务。Doctor/repair
  结果都不等于 provider、Science、artifact、installed 或 live 验收。
- 关闭窗口只隐藏；frontend 显式 `quit_app` 由
  `commands/runtime/lifecycle.rs` 复用完整 `stop_all` 边界，只有停止成功才退出。
  macOS/Tauri `RunEvent::Exit*` 则是 `desktop/src-tauri/src/lib.rs` 拥有的独立 native-exit
  cleanup path，不是 frontend command caller；它保留 best-effort policy，Science stop
  复用 `commands/runtime/lifecycle.rs::execute_process_local_science_stop_with` 的 owner
  claim、锁外 wait 与 CAS，但忽略 stop failure 并继续关闭 Gateway。不得把这种退出策略
  与显式 quit 的“停止失败则不退出”合并。

## 条件入口

- auto-boot 只有进程环境 `CSSWITCH_AUTO_BOOT_ON_LAUNCH=1` 时才进入；否则启动路径
  显示主面板。
- 内嵌 Science WebView 只有 `CSSWITCH_SCIENCE_WEBVIEW_SPIKE=1` 时才尝试；未设置、
  非 `1` 或构建失败时继续使用系统浏览器。
- 仓内未发现生产 UI 或 packaging 为这两个变量赋值的 producer。因此以上是
  `SOURCE-CONTRACT` 的条件分支，不足以证明普通安装默认启用；普通 installed 行为
  保持 `UNKNOWN`。

## 维护规则

- 新增/删除 command 时，同时检查 `lib.rs::run` 注册、`main.js` 及其生产动态导入模块中的 caller、preview mock 和 DTO。
- 新增 event 时，明确 payload schema、冷启动丢事件的补读策略和敏感字段。
- “有 command”“有 mock”“有测试”不得写成“UI 可达”。
- 公共错误阶段与 recovery/environment 状态必须来自结构化源字段；message 只用于展示，不能参与控制流。
