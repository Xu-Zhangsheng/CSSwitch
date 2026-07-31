# 运行时状态与事务

本文回答“谁拥有运行状态、怎样持久化、锁序是什么，以及启动/切换/恢复/停止失败怎样补偿”。Science executable/data 身份见[Science runtime](science-runtime.md)；command/DTO 见[Desktop 控制面](desktop-control-plane.md)。

## 状态所有权

| 状态 | Source of truth | 持久性 |
|---|---|---|
| Gateway child、launch ID、key fingerprint、launch context | Tauri `AppState` | 进程内 |
| Science runtime identity、confirmed-stopped token、boot/history refs | Tauri `AppState` | 进程内；当前产品不保存 daemon child |
| Science version observations | `AppState.science_version_cache` | 进程内缓存；不等于 daemon/runtime identity |
| pending authority cleanup retry set | `AppState.pending_authority_cleanup` | 进程内镜像；跨重启权威是 private pending-cleanup manifest |
| profile、active selection、端口、mode、SSH/Codex 设置、path secret | CSSwitch `config.json` / `Config` | 持久 |
| last healthy binding | `Config.runtime_binding` | 持久；只含公开 identity/hash |
| in-flight runtime transaction | `Config.runtime_transaction` / `RuntimeTransactionJournal` | 持久；自由字符串 stage |
| Science protected state rollback | private authority snapshot + manifest | 持久到 success/完整补偿/人工处置 |
| Science managed launch | `science-managed-launch.v1.json` + live listener identity | 持久 receipt 与 live 组合 |
| virtual login | Science credential files + CSSwitch `virtual-org.v1.json` marker | 分属 Science/CSSwitch |
| Skill ownership | Skill 内 `.import-origin` | 单包持久 |
| Skill bundle | CSSwitch bundle manifest/journal/locks | 跨操作持久 |
| SSH bridge | Science `config.toml`、CSSwitch sidecar、V2 stub | 分属 Science/CSSwitch |
| Codex auth/model generation | CSSwitch Gateway 私有文件 | 持久并带 lock/generation |

任何单一文件、端口或内存字段都不能独立证明完整运行身份。

## 当前源码 owner

| 状态或事务边界 | 当前源码 owner |
|---|---|
| `AppState` / `SharedLifecycle` 类型与进程级组合 | `desktop/src-tauri/src/lib.rs` |
| command 级 mode/settings/stop/quit 串行编排 | `commands/runtime/lifecycle.rs` |
| 一键启动、history recovery 与 UI failure 投影 | `commands/runtime/one_click.rs` |
| protected projection、journal recovery、route reconcile、SSH preflight 与一键事务 | `runtime/sandbox_session/` |
| protected snapshot 合同、capture 与 restore | `runtime/sandbox_session/authority_snapshot.rs` façade及其 `authority_snapshot/` 片段 |
| healthy daemon reopen 的独立补偿分支 | `runtime/sandbox_session/one_click/healthy_reopen.rs` |
| Gateway recovery/reuse/spawn/stop | `runtime/proxy_lifecycle.rs` façade及其 `proxy_lifecycle/` 片段 |
| Science executable、runtime identity、managed receipt 与 stop | `runtime/science.rs` façade及其 `science/` 片段 |
| 持久配置、runtime binding 与 journal schema | `desktop/src-tauri/src/config.rs` |

这些路径是当前维护映射，不改变上表的 source of truth。`runtime.rs`、
`science.rs`、`proxy_lifecycle.rs` 和部分 `sandbox_session` 根文件是保持历史
module surface 与测试 identity 的 façade；状态所有权仍由 `AppState`、
`Lifecycle`、`Config`、receipt/manifest 与 live identity 的既有组合决定。

## 锁序与并发

跨命令变更遵守固定顺序：

```text
Lifecycle mutex
  -> AppState mutex
    -> config::update mutex
```

- `Lifecycle` 覆盖命令级复合操作，不可重入；
- `AppState` 通常只在读写进程内状态时短持有，health probe 刻意在锁外；但当前 `stop_all` 会持锁跨越 stop script、TERM/KILL 等同步等待，这是状态查询阻塞与锁争用的现有诊断点；
- `Lifecycle.generation` 使锁外 probe 在 stop/clear/switch 后失效；
- `config::update` 只覆盖 load-modify-save；
- config 文件提交使用 pinned/no-follow 边界、临时文件、rename、fsync、提交前复核与回滚，但不是跨进程 advisory lock。

Skill bundle、Codex auth 与 SSH bridge 还各有局部锁/CAS/sidecar 事务。生产的本地
Skill 安装不取得 `Lifecycle`，而是在文件选择前后复核相同
`ScienceHostContext`，再进入 package commit 与 attach/readback；因此
`Lifecycle -> AppState -> config::update` 是取得 Lifecycle 的复合运行操作锁序，
不是所有 Desktop mutation 的全局锁序。

## 三个阶段域

| 阶段域 | 形态 | 用途 |
|---|---|---|
| operation trace | typed `OperationStage` | 脱敏运行日志和耗时 |
| runtime journal | string `stage` | crash/recovery 的持久 checkpoint（**未** typed 化） |
| frontend DTO | coarse string | 用户可见失败定位 |

一键/auto-boot 失败由内部 `runtime/failure.rs` 的 `OneClickFailureKind` 在**产生点**标注，再投影到冻结的 coarse stage（`prepare|science_stop|gateway_start|catalog_verify|science_start`）与 `recovery_status` / `environment_status`。**不得**用用户文案 `contains` 反推 stage。journal checkpoint 字符串与 recovery 语义独立，本层不改。

## 一键开始事务

冷启动或重启分支的高层顺序：

1. 读取 active profile 与 provider contract，复核端口和 Codex proof；
2. 进入 Lifecycle 串行区，恢复中断 journal/cleanup；
3. 若启用 SSH，完成真实 config、alias、wrapper、sidecar/stub 预检；
4. 确认或精确停止 prior Science；
5. 固定 opaque roots，捕获 protected projection，并持久登记 recovery disposition；
6. 准备 virtual login 与 SSH bridge；
7. 启动/复用 Gateway，校验 model catalog；
8. 启动 Science，校验 health、listener、binary、data-dir 与 managed receipt；
9. 复核 Science DB/catalog；
10. best-effort 配置 Skill route/connector；该步骤可能写 route marker 并调用运行中 Science control；
11. 计算并提交 runtime binding、清除 journal，随后打开 UI。

已健康 daemon 的 reuse/reopen 分支顺序不同：它先确保 Gateway、复核 model
catalog，再提交 runtime binding 并清 journal，之后才检查或 best-effort 修复
Skill route/connector，最后生成并打开 UI URL。不能把 cold-start 的
route-before-binding 顺序外推到 reuse 分支。

OAuth、SSH、MCP 或 route 写入前必须完成 protected snapshot。`serve` 之前的失败可在身份安全时精确补偿；`serve` 之后 Science 可能迁移环境，恢复结果可降为 `environment_uncertain`。

## profile 与 mode 切换

- `set_active_profile` 只写 selection，不触碰运行态。
- 真正应用由下一次一键开始执行。
- mode 切到 official 时先 bump generation，停止受管 Science/Gateway，再持久化 mode；停机失败不提交。
- 当前产品不执行运行中 profile switch transaction；`set_active_profile` 只提交 selection，下一次一键开始按新的 active profile 重新走完整启动与补偿链。源码中的 `set_active_profile_txn` / `PriorScienceRestored` 链是 `compiled + test-only` candidate，不属于当前 product-reachable 合同。

## 历史恢复

frontend 只持有一次性 opaque reference。backend 复核 active profile、port、session 后：

1. 精确停止当前受管 Science；
2. 恢复用户选择的历史组织；
3. 清理一次性 reference；
4. 重新进入一键开始。

组织 UUID、真实路径与敏感凭证不跨 invoke 边界。

## 停止

`stop_all`：

1. bump generation，使旧 probe/启动失效；
2. 精确停止 Science；
3. 无论 Science 结果如何都停止 Gateway；
4. 若 Science 未验证停止，返回“Gateway 已停、Science 失败”的部分结果。

Science stop 不能只信 CLI 退出码。必须结合 pre/post 唯一 listener PID、canonical executable、data-dir、launch token 与端口真实关闭；身份漂移时不发送信号。

## 诊断与失败链

- `status` 只做短超时 HTTP health 和内存 metadata 投影；
- doctor 不是强 identity 或 live provider 证明；
- route/connector 配置失败只降级外部 Skill，不阻断普通启动；
- SSH 默认关闭；启用后其 preflight 是 fail-closed；
- Codex auth/catalog 错误通常只阻断对应 Codex 操作；但 active profile 为 Codex，或 prior running Gateway 仍是 Codex 而下一次一键开始需要先取得其 proof 时，也会阻断该次启动；
- provider/Gateway、authority snapshot、runtime preflight、port identity、Science launch/health 可阻断一键开始。

## 当前架构缺口

- ~~journal/trace/frontend stage 没有统一 typed source~~ 一键/auto-boot UI stage
  已由 `OneClickFailureKind` 投影；journal 仍为 recovery checkpoint 字符串；
- ~~`science_failure_stage()` 用字符串推断~~ 已删除生产路径；
- ~~auto-boot 丢失 `stage/recovery_status/environment_status`~~ `boot://failed` 与
  `boot_error` 现携带与手动一键同 shape 的 failed DTO；
- config 的外部并发检测不是跨进程共享锁；
- `stop_all` 持有 `AppState` 锁跨越外部停止与信号等待；
- 本地 Skill 安装不取得 `Lifecycle`；第二次 runtime-context 复核之后仍可能与
  stop/switch 交错；
- MCP 与 SSH 的产品动态 gate 仍开放。
