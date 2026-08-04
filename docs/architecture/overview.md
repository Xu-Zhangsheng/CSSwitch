# CSSwitch 架构总览

本文只保留跨版本的产品边界、组件依赖和高层失败边界。版本、commit、package、release 与一次验证见[日期化 audit/evidence](../audits/2026-07-30-v084-architecture-reconnaissance.md)。

## 产品边界

CSSwitch 是 Claude Science 的 provider 配置转换器、本地 inference Gateway 和隔离启动器。它负责：

- 把当前 profile 转换为 Science 使用的 Anthropic-compatible loopback endpoint；
- 管理 Rust Gateway 与隔离 Science 的生命周期、身份和失败补偿；
- 准备隔离虚拟登录并复用持久 Science state；
- 提供默认关闭的 Codex browser-only OAuth、动态模型目录与 Responses bridge；
- 提供两个窄 bridge：外部 Skill 安装/卸载，以及用户 opt-in 的系统 SSH 配置复用。

Science 仍拥有 project/session/artifact、组织、原生 Skills/connectors/Plugin 上游面、environments/kernels、Reviewer/Specialist、remote compute、updater 与 UI 语义。当前已验证的 CSSwitch 产品范围不模拟 Anthropic OAuth/catalog，不包含通用 Skill/MCP/Plugin 管理器、Science 下载器或远程访问服务。机械拆分期间不扩大该范围；后续逻辑重构可以评估受管 Skill/MCP/Plugin 扩展控制面，但最终支持类型、所有权和生命周期必须先在独立功能合同中冻结，且不能在实现、测试、artifact 或 installed runtime 证据建立前写成当前能力。

## 当前可达性

源码存在不等于产品能力。`compiled`、`registered`、`product-reachable`、`test-only` 与 `legacy/orphan` 的唯一词表和判定边界见[Desktop 控制面](desktop-control-plane.md#可达层定义)；本总览不重复定义。精确 command 数量、orphan 清单与当前 findings 留在日期化 audit。

## 主依赖方向

```text
Desktop WebView
  -> Tauri command / event
     -> command-specific mutation boundary
        -> Lifecycle serializer（runtime/profile/mode/Skill route repair）
        -> picker 后短 HostBridge lease + typed host receipt + Skill package transaction（本地 Skill 安装）
        -> Config + AppState / package-private state
        -> Rust Gateway
           -> provider / Codex upstream
        -> Science runtime
           -> loopback Gateway
           -> Science local control plane

Science Agent
  -> CSSwitch managed local stdio connector
  -> Gateway Skill bridge worker
  -> Science native attach/detach/readback

Science remote compute
  -> optional CSSwitch SSH stub/wrapper
  -> user OpenSSH / network / server
```

## 组件边界

### Desktop / Tauri

管理配置、端口、运行模式、AppState、runtime lifecycle、UI 状态和可选 bridge 编排。command/event/DTO、frontend caller 和 auto-boot 投影以[Desktop 控制面](desktop-control-plane.md)为准。

### 状态与事务

`AppState` 拥有进程内 Gateway child、Gateway/Science identity、boot refs、Science version observation cache 和 pending authority cleanup 的重试镜像；持久 cleanup manifest 才是跨重启权威。当前产品启动脚本退出后不在 `AppState.sandbox` 保存 Science daemon child，daemon ownership 依赖 runtime identity、managed receipt 与 live listener。`Config` 持久化 profile、端口、mode、binding 与 journal。`Lifecycle` 串行化 runtime/profile/mode 等复合变更和显式 Skill route repair；只读 Doctor 不进入 mutation serializer。本地 Skill picker 保持在 lease 外，选择完成后取得短 `HostBridge` lease，在同一 lease-bound typed host receipt 下完成最终 context 复核、package commit 与 Science attach/readback。authority snapshot、managed receipt、Skill/SSH/Codex 各有局部状态。锁序、启动/切换/恢复/停止和补偿见[运行时状态与事务](runtime-state-transactions.md)。

### Rust Gateway

Gateway sidecar 处理 provider/Codex inference、模型目录、协议/SSE/tool 转换；同一 binary 还承载 scratch probe、Codex auth CLI、外部 Skill stdio MCP 和 Science control 子命令。当前生产运行没有 Python proxy fallback。详见[Gateway 与 provider 路由](gateway-provider-routing.md)。

### Science runtime

CSSwitch 在第三方模式使用隔离 HOME、持久 data-dir、受校验 executable 与 loopback ports，并以 `--no-auto-update` 启动。executable、data-dir、固定用户级状态、environment/kernel 与 live identity 是不同事实，详见[Science runtime](science-runtime.md)。

### Science 能力

project/session/artifact、Skills、MCP/connectors、Plugins、environments、Reviewer/Specialist、SSH、updater 与 network preference 的 owner、模式与证据升级条件见[Claude Science 能力依赖](science-capability-dependencies.md)。用户可见支持视角见[产品 / Science 能力地图](../features/product-science-capability-map.md)。

## 所有权

| 数据 / 能力 | Source of truth | 所有者 |
|---|---|---|
| profile、provider contract、model preset、mode、端口 | CSSwitch config/catalog | CSSwitch |
| Gateway child、route 与 private auth state | CSSwitch runtime/Gateway | CSSwitch |
| Science executable/updater candidate | 用户安装或 Science updater 写入的固定路径 | 用户 / Science |
| CSSwitch runtime snapshot/receipt/journal | CSSwitch private data root | CSSwitch |
| 隔离 Science project/session/artifact/org state | 隔离 Science HOME/data-dir | Science |
| Science environments/kernels/runtime resources | 隔离 Science HOME/data-dir | Science |
| external Skill marker/bundle journal | CSSwitch marker/private bundle root | CSSwitch |
| native Skill/binding/connector/Plugin 上游状态 | Science | Science |
| SSH config/key/agent/known_hosts/network/server | 用户 / OpenSSH / 外部系统 | 用户 / 外部 |

## 网络与安全

- Gateway 与 Science UI 只绑定 loopback；产品没有 `0.0.0.0` 开关。
- Science UI port 与 sandbox port 分开校验，`8765` 是用户真实 Science 保留端口。
- 一次性 Science URL、nonce、CSRF 和 path secret 不进入普通 status/log。
- 第三方模式不读取或复制真实 Claude 登录数据。
- Gateway raw `CONNECT` 在 path-secret 认证前分派；listener 虽只在 loopback，任何本机进程仍可使用。它只按 Anthropic/Claude hostname denylist 拒绝目标；DNS resolver 本身没有 deadline，DNS 返回后的地址连接共享剩余 10 秒预算，建立后的双向转发没有 session deadline、idle timeout、byte cap 或并发连接/session-count 上限。这条通用 TCP transport 不证明 Remote MCP。
- Science app proxy、sandbox network、package mirror、Codex route 与 provider egress 是不同网络面。
- SSH opt-in 是行为授权；不复制 `.ssh`、不启动 `sshd`、不开放监听。

## 高层失败边界

可阻断一键开始：

- profile/provider contract、Gateway spawn/health/catalog；
- authority snapshot、runtime preflight、端口与 live identity；
- Science launch/health/receipt；
- active profile 或当前仍运行的 prior Gateway 使用 Codex 时所需的 Codex auth proof；
- opt-in 后的 SSH config/stub/wrapper preflight。

只降级局部能力：

- 外部 Skill route/connector/Agent control 配置；
- 单次 Skill/MCP/SSH domain operation；
- 与当前 runtime 无关的单次 Codex auth/catalog/transport 操作。

当前缺口、数量和 product-reachable 清单只在[日期化调研 audit](../audits/2026-07-30-v084-architecture-reconnaissance.md)固定；稳定架构正文不复制版本化 findings。
