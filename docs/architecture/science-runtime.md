# Science runtime 合同

本文描述跨版本的 executable 选择、HOME/data、身份和恢复合同。具体 Science 版本、hash、embedded identity、事故与 E2E 结果只进入[日期化调研](../audits/2026-07-30-v084-architecture-reconnaissance.md)或 evidence。

## 设计定位与沙箱边界

CSSwitch 是 Science 第三方模式的 **运行编排器和模型协议适配器**，不是 Science
产品本体。目标是在不读取真实 Claude 账号状态、不重建 Science 领域系统的前提
下，让第三方 provider 在自身能力范围内尽可能保留 Science 原生体验。

CSSwitch 长期拥有的稳定合同只有：

1. executable 选择、来源和 runtime identity；
2. 隔离 HOME、持久 data-dir、端口与禁止真实数据目录的布局；
3. 隔离 data-dir 内的本地虚拟登录投影；
4. loopback Gateway、path secret、provider launch plan 与 model protocol 兼容；
5. launch receipt、listener、data-dir 和进程身份对齐；
6. protected projection、pending cleanup 和精确补偿；
7. 显式、可关闭、局部失败的 Skill/SSH/Codex 等窄 bridge。

Science 继续拥有 UI、project/session、permission、artifact、memory、
environment/kernel、Agent、Skill 执行、generic MCP/Plugin 和官方服务客户端。
这并不免除 CSSwitch 的兼容责任：如果 Science 原生工作流依赖模型请求，
Gateway 必须在 provider 声明的能力内保留 stream、tools、`tool_choice`、
reasoning、structured output、vision、错误与停止语义，或给出明确、可定位的
降级。

这里的 sandbox 是身份与数据隔离，不是“全部断网”，也不能因为多种流量经过同一
Gateway 进程就把它们都解释成 model routing：

- model inference 必须进入 CSSwitch Gateway；
- 显式 bridge 只走自己的 loopback/stdio/IPC 合同；
- connector、文献、云、generic remote MCP 与 remote compute 等 Science 原生
  外部语义由 Science、用户和外部服务管理；当前非 loopback HTTPS 仍可能因进程级
  `HTTPS_PROXY` 以 raw `CONNECT` 穿过 Gateway，CSSwitch 只拥有 tunnel target
  policy、deadline 和 transport diagnostics，不拥有 MCP/connector/云等上层语义；
- 非 HTTPS、显式 proxy bypass 或不读取 proxy environment 的 client 不能由上述
  合同推出；CSSwitch 不注入其凭证、不冒充 entitlement，也不能无说明地改变现有
  transport；
- 真实 Claude OAuth/token、真实账号数据库、整个真实 HOME 和未经用户选择的外部
  凭证不得投影进第三方沙箱。

凭证边界的 process environment 合同：Tauri → launch/stop script 与
launch script → Science 均使用显式 allowlist（`runtime/launch_env.rs` +
`scripts/launch-virtual-sandbox.sh` 的 `env -i`）；stop/launch 控制面注入
`CSSWITCH_HOST_HOME`，Gateway base 注入绝对 host `HOME`（Codex 等主机态路径）；
provider secret 只进入 Gateway。sentinel 与 stub 回归见 `runtime::launch_env`
测试和 `test/test_launch_science_env_allowlist.sh`。完整所有权与 bridge 边界
见下方能力依赖正文。

完整 ownership、运行路径、bridge 准入和拆分前冻结项见
[Claude Science 能力依赖](science-capability-dependencies.md)；逐能力当前决策只在
[产品与 Claude Science 能力地图](../features/product-science-capability-map.md)
维护。

## 当前源码 owner

`desktop/src-tauri/src/runtime/science.rs` 是保持历史 module surface 的同模块
façade；生产实现按独立维护原因分布为：

| 边界 | 当前源码 owner |
|---|---|
| 基础类型、常量、receipt/runtime 合同 | `runtime/science/contracts.rs` |
| executable 选择、安全读取、snapshot 与版本识别 | `runtime/science/executable.rs` |
| live process/listener identity 与 runtime state | `runtime/science/runtime_state.rs` |
| managed launch、receipt 与启动后身份提交 | `runtime/science/managed_launch.rs` |
| preflight、probe、reuse、URL 与 stop lifecycle | `runtime/science/lifecycle.rs` |
| 历史测试身份 | `runtime/science/tests.rs`；仍保持 `runtime::science::tests::*` |

这些片段仍编译在原 `runtime::science` 模块内；拆分没有建立新的状态 owner，也没有
扩大 executable 来源、environment allowlist、凭证读取、listener 控制或 stop
权限。Science transaction 与 protected projection 继续由
`runtime/sandbox_session/` 拥有，Gateway 进程编排继续由
`runtime/proxy_lifecycle/` 拥有。

## 分离六个事实

1. **executable**：实际执行的 `claude-science` 文件；
2. **persistent data-dir**：`~/.csswitch/sandbox/home/.claude-science`；
3. **isolated HOME**：CSSwitch 第三方模式传入的 HOME；当前部署让它与 data-dir 根共址，但这不是 Science 通用 `data_dir` 语义；
4. **fixed user-level state**：Science 固定写入 `~/.claude-science` 的 config、认证与 shared package environment；在第三方模式中该 `~` 指隔离 HOME；
5. **environments / kernels / runtime resources**：starter/task environments、session kernels 与 `<data-dir>/runtime/<version>/`；
6. **live identity**：canonical executable、data-dir、监听 PID、端口和受管启动记录的组合。

官方合同中，data directory 保存 per-org conversation、artifact、delegation 和 workspace；认证 token 与 shared package environment 固定在 `~/.claude-science`，不随 `data_dir` 移动。CSSwitch 用隔离 HOME 把两类状态一起隔离，但文档和恢复逻辑仍须区分所有权。

starter Conda Python/R 环境、跨项目复用的 named task environment、session 内 Python/R kernel 与 inline package 都是不同生命周期。CSSwitch 不拥有这些环境，也不从 opaque root 推断功能可用；Node 的上游所有权/scope 仍为 `UNKNOWN`。

## 新启动选择顺序

1. 如果设置了 `SCIENCE_BIN`，它必须是绝对、非 symlink、可执行且能安全读取版本的开发 override；无效时 fail closed，不继续猜其他 binary。
2. 否则检查固定的 `~/.claude-science/bin/claude-science`。只有路径无 symlink、目录与文件由当前用户拥有且不可被 group/world 写入、文件是大小有界的 Mach-O、embedded identifier / Team ID 精确匹配当前源码枚举的 Science identity，才把同一次安全打开读取到的字节原子提交到 CSSwitch 私有、只读、SHA-256 内容寻址的 runtime snapshot；snapshot 在隔离 HOME 下版本可确认时才选择为 `official_updated`。
3. 官方 updater runtime 不可用时，使用当前安装在 `/Applications/Claude Science.app` 中的官方 executable。
4. 只有以上来源都不可用、`<CSSwitch data-dir>/bin/claude-science` 可执行且版本可确认时，preflight 才返回 `cached_choice_required`；用户可授权 `cached_once`。
5. cache 授权只在本次启动的内存中生效，不写成偏好。未知版本或缺失 cache 不可启动。

`official_updated` 只读取并快照 updater 固定路径中的单个 executable；该路径已观察到 standalone updater 与 App-seeded 两种精确 identity，当前源码只接受枚举的 exact identifier + Team ID 组合。具体字符串、hash、版本与 `source-fixed-product-pending` 结论留在日期化 audit；它们不能写成 final artifact、installed/live 或公开 release 已证明。

内容寻址 snapshot 只证明被采用 executable 的稳定字节身份，不等于一份可比较的 Science
更新 provenance ledger。当前源码没有通用的 predecessor/candidate manifest、normalized
CLI/route/capability diff 或 adoption decision record；历史版本的日期化兼容性调查不能替代该
机制。未来差异记录只能读取受校验 executable 的版本、SHA-256、embedded identity、来源与
所选 snapshot 等元数据，不能读取或 diff Science 用户账号、组织、对话、project 或其他
opaque data。

snapshot 位于 `<CSSwitch data root>/runtime-snapshots/science/`，不在 Science data-dir 内。CSSwitch 不扫描、复制或读取真实 Science 账号、组织、配置、`conda`、`runtime` 或 `seed-assets`，不下载 Science、不调用 updater，也不覆盖 Science cache。检测到候选但本地校验失败时会显式报错，不静默回退旧 App。

embedded identifier / Team ID 只作为格式与误选防护，不声称密码学证明文件来自 Anthropic。该路径沿用 CSSwitch 已有的“信任当前用户安装的本地 Science”边界；复制前后复核同一 source inode/metadata，snapshot 以完整 SHA-256 命名并进入 host-context fingerprint。启动、恢复和停止等强控制路径会重新验证 snapshot，内容变化时 fail closed；高频 UI `status` 是下文明确的轻量例外，只投影 HTTP health 与已有 metadata。`sandbox_url()` 也不是独立的强身份边界：runtime 不再 current 或 CLI URL 获取失败时，它会回退到裸 `http://127.0.0.1:<port>`；手动打开入口会先验证 listener/runtime，但冷启动与 reuse 路径依赖调用它之前已经完成的身份检查。updater 随后替换 source 不会改变已运行 daemon 的 executable 身份。为支持 CSSwitch 自身重启后的接管，恢复探测会在私有 snapshot 目录中重新验证已有的内容寻址 executable；历史 snapshot 只参与现有 daemon 的身份恢复，不改变 stopped-to-started 的新启动选择顺序。

## 启动与网络参数

新进程使用预检后的 binary 和固定 data-dir，并显式传入：

- `--host 127.0.0.1`；
- CSSwitch 选择的 UI port；
- 单独校验的 `--sandbox-port`；
- `--no-auto-update`。

Gateway 同样只监听 loopback。端口健康不等于身份健康；公共网络暴露不属于当前合同。

## 一键启动的事务快照边界

CSSwitch 是启动与路由插件，不拥有 Science 的语言环境。冷启动事务只快照可能被 CSSwitch 启动准备流程改写、且失败时必须精确补偿的受保护状态：

- `encryption.key`、`.oauth-tokens/`、`active-org.json`、`.key-backups/`、`auth-owner.lock`；
- `config.toml`、`csswitch-ssh-bridge.v1.json`、`mcp/`、`.csswitch-route-state.json`；
- `orgs/`（包含组织数据库、历史和组织内 Skills，不能按缓存处理）。

`conda/`、`runtime/`、`seed-assets/`、`r-libs/`、`sbx-bind-src/` 是 Science-owned opaque roots。CSSwitch 只对这五个固定顶层入口做 no-follow 的目录、owner、权限和 device/inode 绑定校验，并在任何受保护写入前及 `serve` spawn 紧前重验；不得递归遍历、读取、复制、fsync、删除或回滚其内容。未知的其他顶层入口同样按外部状态原地保留，除非它与受保护入口冲突或根目录身份不安全。这个边界不依赖 APFS clone，也不把环境误称为可重建缓存。

这是当前 **protected projection** 合同。旧候选曾对整棵 authority 做 full-tree snapshot，因 Conda 大文件、runtime symlink、entry/逻辑容量而多次失效；该历史只证明当时的故障与修复过程，不表示当前仍递归 clone 全树。后继关系见[0.1.25 compatibility evidence](../evidence/investigations/2026-07-28-claude-science-0.1.25-compatibility.md)末尾指针。

私有快照根一旦建立，就在复制任何受保护状态之前，以 marker、精确 path/device/inode 和原子 manifest 的 `active_recovery` disposition 持久登记；完整快照仍必须在任何 OAuth、SSH bridge、MCP 或路由写入之前完成。只有成功、完整补偿或无需继续恢复的选择分支，才能通过 compare-and-swap 把同一票据转成 `cleanup_only`，随后才允许删除；因此 capture 中途崩溃不会留下未登记的敏感 orphan，config journal 即使被补偿回滚，也不能把仍需恢复的快照误删。进程若在活动事务中崩溃，下一次启动不得自动删除这份快照，也不得把可能的部分写入态当作新基线；当前策略是保留快照并要求人工恢复。旧版 `start_science` 日志没有 runtime 指纹，升级后按 `environment_uncertain`、`newer_runtime_required` 失败关闭，不得跨 runtime 自动启动。

事务在调用 `claude-science serve` 前失败时，可以精确补偿受保护状态。调用后，Science 可能已经迁移或修改自身环境，即使受保护状态已恢复，结果也必须标为 `environment_uncertain`；跨 runtime 失败不得用旧 runtime 自动重启这个已暴露环境。无法证明候选 Science 已停止时，不得在其下方恢复凭据或组织数据库，必须保留受限恢复快照并返回 `cleanup_required`。

## 运行中身份与恢复

CSSwitch 在内存中记录实际 launch binary path、来源（`explicit`、`official_updated`、`installed_app` 或 `cached_once`）、版本和选择时文件指纹。启动、复用、恢复与停止操作使用这份 runtime metadata，并在需要控制 daemon 的边界做强身份检查。URL helper 也接收该 metadata，但自身允许上述 localhost fallback，不能单独作为强身份证明。

停止不能只信任 Science CLI 的退出码：部分版本会返回成功并删除 lockfile，但 daemon 仍在监听。CSSwitch 在调用 CLI 前后都要求 sandbox port 的唯一监听 PID 与已记录 executable 精确匹配；CLI 后端口仍存活时，只向这一个前后均匹配的 PID 发送 TERM，超时后再次复核同一身份才 KILL，并以端口实际关闭作为成功条件。监听身份变化时不发送信号并返回错误。

高频 UI `status` 是例外：它只对 sandbox port 做短超时 HTTP health，并把内存中的 source / version metadata 投影到诊断结果；launch binary path 仍只保留在 `AppState`，不跨 status DTO 暴露。该路径不反复调用 `claude-science status`，不重新核对监听 PID，也不能证明当前监听者就是已记录 runtime。

CSSwitch 自身重启后，只能在以下条件同时满足时接管已有 daemon：

- 监听 PID 的 canonical executable 与候选 binary 匹配；
- 候选 CLI 确认的是同一 data-dir daemon；
- 端口与受管状态一致。

单独的端口占用或 `status` 成功不足以证明身份。已健康 daemon 应复用，而不是只因 App 版本或可选 bridge 状态变化被强制重启。

## 升级合同

官方模式 updater 写入新 runtime 后，下一次 stopped-to-started 启动生成并选择对应内容 snapshot；如果没有 updater runtime 而用户更新了 Claude Science App，则下一次启动重新选择 App 内的 executable。两条路径都继续复用原 CSSwitch data-dir。使用 updater snapshot 的已健康 daemon 保持其不可变 executable，不因 source 出现新版本而强制重启；正常停止后下一次启动才切换。CSSwitch 不迁移或覆盖组织、项目和 Skill 数据。

每次上游 App 更新后，分别验证：

1. 实际 selected binary 与 `--version`；
2. data-dir 复用且没有读取真实 HOME runtime 资产；
3. live PID、executable、runtime directory、data-dir 与端口属于同一运行；
4. start / reopen / recovery / url / stop 的强身份一致，并单独确认 UI status 只表示 HTTP health；
5. 外部 Skill route、install / attach / load / restart / uninstall / detach；
6. bridge 不兼容仍只产生 warning，普通 Agent 可工作。

一次上游版本测试不能推出通用最低版本。观察接口变化时，应只关闭受影响 bridge 并如实报告，而不是替换或降级用户 App。

## 非目标

- 不把 `@` artifact / output 当成持久 Skill 注册；
- 不把 `<data-dir>/runtime/<version>/skills` 当外部 Skill 安装目标；
- 不通过 OAuth、私有 bearer、数据库写入或 binary patch 管理 Science；
- 不为 SSH、Skill 或 provider 失败扩大 runtime 权限。
