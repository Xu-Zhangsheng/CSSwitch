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
     -> Lifecycle serializer（runtime/profile/mode/doctor reconcile）
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
| diagnostics | doctor（含第三方 Skill 路由 reconcile）、版本、release/issue/log 入口 |

大多数 runtime/profile/mode 复合 mutation 进入 `Lifecycle`，但这不是所有 Desktop
写操作的统一锁。生产 `install_local_skill_package` 让文件 picker 保持在 lease 外；
选择完成后取得短 `HostBridge` mutation lease，重新探测 matching
`ScienceHostContext` 并构造 lease-bound `LocalSkillHostReceipt`，再用同一 receipt
完成 package commit 与 Science attach/readback。该合同闭合的是本地 Skill 的
最终 runtime-context race，不把 picker/download 放进锁，也不建立跨进程或全局 durable journal。

以下 command 已注册但没有当前生产 frontend caller：

- `list_templates`
- `validate_profile_catalog_model`
- `preview_profile_preset_sync`
- `apply_profile_preset_sync`

它们可能是预留面或遗留面，当前统一标为 `dormant registered / UNKNOWN`。注册本身不构成产品能力。

S6 已移除无 bundled caller 的 `start_proxy` Tauri command；formal Gateway 只能由现有
cold/healthy/profile-switch/recovery 内部流程经 `GatewayController` 启动或复用，不再暴露
独立的 Gateway-only invoke mutation。

生产 frontend 没有发现 literal command 调用未在 Tauri 注册的反向缺口。

外部 GitHub Skill 的源码控制面已经覆盖配置、安装与 `OPERON` attach；这只能证明
`SOURCE-CONTRACT`。Agent 在最终 artifact / installed runtime 中实际 load Skill、
调用 install/poll tool、完成卸载以及 restart 后继续可用，当前均为 `UNKNOWN`。

## event 面

| Event | 发出方 | Payload | 当前边界 |
|---|---|---|---|
| `boot://failed` | auto-boot coordinator | JSON value（一键 failed DTO） | 与手动一键同 keys：`action/stage/status/recovery_status/environment_status/message/fallback_url`；frontend 兼容旧 string payload |
| `boot://attention` | auto-boot coordinator | JSON value | 保留 history-choice 等 attention 对象 |
| `codex-auth://operation` | Codex command | typed operation snapshot | sequence/state/error 等结构化字段保留 |

frontend 启动时同时读取 `boot_error` / `boot_attention` command，并监听对应 event，覆盖 listener 注册前已经发生的启动结果。`boot_error` 现返回结构化 failed DTO（与 event 一致），不再只是纯 message 字符串。

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
`boot://failed` / `boot_error` 与手动一键共享 failed DTO shape；recovery/environment
status 从 typed failure/compensation projection 产生，message 只用于展示。

H4 在 one-click DTO 与 consumer publication 之间增加 `finalize_consumer_state`：它只读 canonical
config，按结构化 `status + recovery_status + action` 与实际 journal/binding readback 返回
`ready | attention | manual`、`open | cleared`、binding relation，以及仅在 `ready` 时可发布的
applied profile id / selection pending；`attention/manual` 固定发布 unknown。该命令不迁移配置、
不 chmod、不清 notice、不返回 journal record、path、credential 或
其他配置内容。manual UI 与 auto-boot 复用同一个 Rust classifier；journal open、回读失败或
不一致组合不得发布 applied/`BootState::Ready`，message 仍只参与展示。
auto-boot 的 failed/attention event 与启动后补读路径也必须把 frontend applied 展示发布为
unknown；完整原 DTO 仍单独保留用于错误和 history choice 展示。

## 选择、应用与诊断语义

- `set_active_profile` 只提交“当前选择”；运行中的 Gateway/Science 不立即切换。下一次一键开始才应用并写 runtime binding。
- history attention、`restore_history_choice` 与下一次 start 是三个独立 backend operation；当前 frontend 在 restore 成功后自动调用 one-click，因此一个用户点击会串联后两个 destructive operation。这个当前缺口不是 durable transaction 合同。
- `status` 是轻量状态投影；Science 灯的 HTTP health 不证明 listener/runtime 强身份。
- `finalize_consumer_state` 是 one-click 完成后的窄、脱敏、只读投影；它不探活、不写配置，也不
  成为新的 transaction owner。history choice 即使伴随 cleanup warning 也保持 attention；normal
  cleanup 只有 readback 确认 exact active binding 且 journal cleared 才可发布 ready；attention、
  manual 或回读失败一律清除 frontend 的 applied 展示并保持 selection pending。
- Doctor 是两个独立 intent。`run_doctor_read_only` 只读 canonical v4 config、运行脱敏
  `doctor.sh` 并投影 Codex 最近一次内存观察；旧 schema 只报错，不迁移、chmod、清 notice，
  也不取得 mutation lease、修改 route 或为诊断启动 Science/Gateway。`repair_skill_route`
  只在用户显式触发后取得 `HostBridge` mutation lease，强制 reconcile 第三方 Skill route；
  它可失效 route marker，或在健康 Science 上通过一次性 control sidecar 同步 route Skill、
  connector 与 managed prompt，但不会启动受管 Science 或正式 Gateway 服务。两个 command 各自返回 typed result；frontend
  每个动作只提交一个 intent 并渲染 `status/message`，不串联或编排事务。Doctor/repair
  结果都不等于 provider、Science、artifact、installed 或 live 验收。
- 关闭窗口只隐藏；显式退出才按受管顺序停止 Science/Gateway。

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
