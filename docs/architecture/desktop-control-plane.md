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
     -> runtime-context recheck + Skill package transaction（本地 Skill 安装）
  -> Config / AppState / package-private state / Gateway / Science

Tauri backend
  -> event(payload)
  -> WebView listener
```

`desktop/src-tauri/src/lib.rs::run` 是 command 注册权威入口。frontend 可达性必须检查生产 bundle 的全部调用模块：`desktop/src/main.js` 是装载、注入和主要 `call()` / listener 入口，动态导入的 `desktop/src/skill-page.js` 等模块也可以持有真实 `call` 并形成 production caller。preview 的 `mockInvoke` 只模拟 DTO，不算生产 caller。

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
写操作的统一锁。生产 `install_local_skill_package` 不接收 `SharedLifecycle`；
它在文件 picker 前后两次复核 `ScienceHostContext`，随后依赖 Skill package
commit 与 Science attach/readback 的局部边界。第二次复核不是互斥锁；其后仍可能
与 stop/switch 等 runtime mutation 交错，这是当前并发缺口，不能把“双重复核”
写成全操作串行化。

以下 command 已注册但没有当前生产 frontend caller：

- `list_templates`
- `validate_profile_catalog_model`
- `preview_profile_preset_sync`
- `apply_profile_preset_sync`
- `start_proxy`

它们可能是预留面或遗留面，当前统一标为 `dormant registered / UNKNOWN`。注册本身不构成产品能力。

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
coarse stage 由 `OneClickFailureKind` 在产生点投影，不扫描 message 文案；journal
checkpoint 仍是 recovery 用的自由字符串，不能与 UI stage 无损互映。auto-boot 的
`boot://failed` / `boot_error` 与手动一键共享 failed DTO shape。
`recovery_status` 仍可能消费 message 内的诊断码（`recovery_status=…` /
`environment_uncertain`）作为过渡；后续应在补偿点直接写入
`ProjectedRecovery`。

## 选择、应用与诊断语义

- `set_active_profile` 只提交“当前选择”；运行中的 Gateway/Science 不立即切换。下一次一键开始才应用并写 runtime binding。
- `status` 是轻量状态投影；Science 灯的 HTTP health 不证明 listener/runtime 强身份。
- `run_doctor` 先执行诊断脚本，再在 Lifecycle 边界强制 reconcile 第三方 Skill
  route；它不是纯只读诊断。Science 健康运行时，该路径可绑定 route Skill 与
  connector、清理旧 connector、解除 `customize` 并更新 managed prompt。Science
  停止、身份未知、binary/version 变化或 bridge 身份不足时，也可能使 route marker
  失效以安排下次重配；它不会仅为 doctor 启动 Science 或 Gateway。doctor 结果不等于
  provider、Science、artifact、installed 或 live 验收。
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
- 公共错误阶段应来自结构化源字段；在产品修复前，文档必须保留字符串推断缺口。
