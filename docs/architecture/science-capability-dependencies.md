# Claude Science 能力依赖

本文解释
[产品与 Claude Science 能力地图](../features/product-science-capability-map.md)
中的 ownership、第三方托管边界和故障归属。能力清单、当前证据层和 `UNKNOWN`
只在能力地图维护；本文不复制第二张支持矩阵，也不记录版本、hash、route 或一次性
观察。

## 1. 架构判断顺序

每项能力按同一顺序判断：

1. **谁拥有语义和 source of truth**：Science、CSSwitch、用户、Anthropic
   账号/组织，还是具体第三方服务；
2. **第三方模式是否缺少必要运行包络**：只有隔离、身份、Gateway、routing、
   network policy、状态一致性和诊断等必要层才由 CSSwitch 托管；
3. **CSSwitch 是否只有窄桥接**：安装/attach、SSH preflight、Codex bridge
   等入口不能升级为下游领域能力 ownership；
4. **是否明确 non-target**：不属于 CSSwitch 的管理面不能因为当前
   `UNKNOWN` 就自动变成未来实现任务；
5. **证据到哪一层**：official/source/test/package/fixture 不得外推为
   artifact、installed-live 或真实外部服务。

“Science 官方拥有”不等于“第三方模式 current live 已验证”；“CSSwitch
托管运行包络”也不等于 CSSwitch 拥有其中的 project、artifact、permission、
Skill、kernel 或 Reviewer 语义。

### 1.1 语义所有权与兼容责任分开

逐能力判断不能停在 owner。还要回答：第三方模式是否改变了该能力依赖的
runtime、model protocol、路径、环境或网络。如果改变，CSSwitch 必须保证自己
引入的兼容层，不能以“Science-owned”为由回避。

- Science-owned 的 plan、delegation、Reviewer 或 Skill trigger 如果最终依赖
  Anthropic-shaped model request，Science 继续拥有工作流语义，CSSwitch Gateway
  负责 provider 所支持范围内的 stream、tools、`tool_choice`、reasoning、
  structured output、vision、错误与停止语义；能力不足必须显式降级。
- Science-owned 的 project、artifact、permission、memory 与 environment 不经过
  CSSwitch 语义层；CSSwitch 的责任是隔离、持久化、启动、恢复和受保护投影不破坏
  它们。
- Science-owned 的 generic MCP、connector、文献、云与远端服务可以沿原生路径
  工作；CSSwitch 只在隔离环境造成明确缺口时提供窄 bridge，不能顺势创建第二套
  client、registry、credential store 或 entitlement。
- 官方 Claude 账号能力不能由虚拟登录、raw `CONNECT` 或第三方模型响应替代。

因此支持结论必须同时写明语义 owner、operation/stage 组合、CSSwitch 兼容责任、
socket transport、外部 dependency 和证据层。ownership 决定谁定义正确行为，
兼容责任决定 CSSwitch 拆分时不能丢掉什么。stage 类别不是 capability-level
单选：Reviewer 可以先走 `SCIENCE-NATIVE`，再按 operation 分支到
`MODEL-GATEWAY` 或 `OFFICIAL-ENTITLEMENT`；remote MCP 可以走
`SCIENCE-NATIVE → SCIENCE-EXTERNAL`，同时让 HTTPS transport 经过 raw
`CONNECT`。

## 2. 第三方模式必须托管的最小闭环

CSSwitch 只需要对下面四类责任形成闭环。

### 2.1 Runtime 包络

- 隔离 HOME 与持久 data-dir；
- executable/runtime identity 选择与固定；
- 受管启动、打开、复用、停止和恢复；
- 只保护 CSSwitch-owned 状态，不递归接管
  `conda/runtime/seed-assets/r-libs/sbx-bind-src` 等 Science-owned opaque
  roots。

Science 仍然拥有 project、session、artifact、environment 和本地数据库语义。
持久 data-dir 不证明 executable 版本，也不证明某项原生能力 current live。

### 2.2 身份与 Gateway

- 本地虚拟登录只为隔离 Science 建立第三方模式入口；
- provider/profile/model selector、协议适配和 routing 由 CSSwitch 管理；
- 第三方 provider 拥有模型、认证、配额、计费和实际服务质量；
- 虚拟登录不产生 Anthropic OAuth、官方 catalog、usage 或 entitlement。

因此“模型请求经过 CSSwitch”不能外推为 Web Search、hosted connector、
Reviewer 或其他 Anthropic 服务已经可用。

### 2.3 Network policy

第三方模式只托管完成 model/Gateway 路径所必需的受限 network policy。Science
网络必须按 app、model、connector、sandbox、preview/voice、package、
remote compute 和 updater 分面判断。

raw TCP `CONNECT` 只证明 tunnel 可能建立，不证明 MCP client、OAuth、tool
discovery 或 tool call；对某一 Anthropic host 的拒绝也不能推广为所有 custom
connector 或所有 Science 网络都不可用。

#### 沙箱 ingress / egress 合同

这里的 sandbox 首先是身份与数据隔离，不自动等于断网、全流量代理或功能裁剪。
如果未来增加更强的 egress policy，也必须按下面的流量面分别判定，不能因为
socket 经过同一个进程就把所有 Science 出站统一归为 model Gateway 语义。

允许并由 CSSwitch 管理的 ingress：

1. 已验证来源、版本和文件身份的 Science executable；
2. 隔离 HOME、持久 data-dir、动态受管端口与 managed receipt；
3. 只在隔离 data-dir 内建立的本地虚拟登录；
4. loopback Gateway endpoint、path secret、provider/profile/model launch plan；
5. CSSwitch 明确改写的 protected projection；
6. 用户显式启用、具有独立身份和生命周期的 Skill、SSH、Codex 或其他窄 bridge。

禁止隐式 ingress：

- 真实 Claude OAuth/token、真实账号数据库或整个真实 HOME；
- 未经用户选择的 SSH、云、机构或第三方服务凭证；
- 对 Science-owned opaque roots 的递归复制、恢复、清理或权限接管；
- 没有 owner、清理规则和故障边界的环境变量、路径或进程投影。

#### Ambient environment allowlist（已闭合）

“禁止隐式 ingress”已由两层 allowlist 落实为当前源码合同：

1. Tauri → launch / stop script：`runtime/launch_env.rs` 对 child 执行
   `env_clear` 后只注入控制面 allowlist（隔离 `SANDBOX_HOME`、runtime path、
   `CSSWITCH_PROXY_URL`、显式 `CSSWITCH_HOST_HOME` 供主机侧路径护栏、SSH 开关
   与 hosts、opaque bindings、固定安全 PATH/locale/temp）；stop 脚本不得依赖
   ambient `HOME`；
2. launch script → Science：`scripts/launch-virtual-sandbox.sh` 使用
   `/usr/bin/env -i` 再次从空环境建立 allowlist（隔离 HOME、
   `ANTHROPIC_BASE_URL`、受限 proxy/`NO_PROXY`、固定 PATH、locale/temp、
   以及 SSH bridge 显式启用时的最小集合）；
3. Gateway：`configure_managed_proxy_command` 同样 `env_clear` + base
   allowlist（含绝对 host `HOME`，供 Codex `$HOME/.csswitch` 等主机态路径），再由
   formal/scratch plan 显式注入 provider secret 与合同变量；
4. provider credential 只进入 Gateway process，不进入 Science 或 launch script；
5. SSH / Skill host 等 bridge 变量仅在对应 bridge 启用路径注入。

验收：

- `runtime::launch_env` 与 `proxy_lifecycle` 的 sentinel / parent-secret 单测；
- `test/test_launch_science_env_allowlist.sh`（stub Science + 污染父环境）证明
  Science serve 子进程看不到 ambient secrets。

typed failure 投影已在 source 层建立（`runtime/failure.rs`）。后续
`sandbox_session` 与 Gateway 机械拆分不得扩大上述 allowlist；helper 移动时保持
退出条件不变，且须继续通过 typed kind 投影失败，不得恢复文案反推 stage。

egress 先按语义责任分为四类：

| 流量面 | 路径与 CSSwitch 责任 |
|---|---|
| model inference | 必须经过 loopback Gateway；协议保真、provider capability 和明确降级属于 CSSwitch |
| narrow bridge | 只进入声明的 loopback/stdio/IPC endpoint；不得共享不相关凭证或扩大目标 |
| Science-native external | connector、文献、云、generic remote MCP、remote compute 等上层语义由 Science/用户/外部服务拥有；CSSwitch 不代理其语义，但必须保留当前 transport policy 并单独诊断 |
| official entitlement | 由 Claude 账号/组织服务决定；第三方模式不注入或伪造官方身份 |

语义责任与 socket transport 是两个轴。当前启动脚本给整个 Science 进程设置
`HTTPS_PROXY` / `https_proxy` 指向 Gateway，并让 loopback 进入 `NO_PROXY`。
因此非 loopback HTTPS 即使属于 `SCIENCE-EXTERNAL`，当前也可能先以 raw
`CONNECT` 穿过 Gateway：

- Gateway 只拥有 CONNECT target parsing、Anthropic/Claude hostname denylist、
  当前无独立 deadline 的 DNS resolution、解析返回后共享剩余十秒预算的 dial、
  tunnel lifecycle 与 transport status；
- path secret、provider model routing、HTTP MCP/OAuth/tool discovery/tool call
  不属于这条 raw CONNECT 合同；
- 非 HTTPS、显式 bypass 或不遵循进程 proxy environment 的 client 可能使用不同
  transport，不能由 `HTTPS_PROXY` 推出全流量覆盖；
- 把某项外部语义移出或移入 CONNECT 都是独立行为变化，不能夹在机械模块拆分中。

diagnostics 可以报告 CSSwitch 自己观察到的 route、listener、bridge 和 transport
阶段，但不得记录 secret，也不得把“TCP 可达”写成上层产品能力成功。

### 2.4 状态一致性与诊断

CSSwitch 负责自己受管对象的状态、补偿和诊断：runtime、Gateway、route、
provider/profile 以及窄 bridge 的配置状态。诊断只能报告它能观察和拥有的层，
不能替 Science 判断 project、artifact、permission、kernel 或外部服务语义。

## 3. 原生保留而非 CSSwitch 托管

以下能力继续由 Science 管理：

- project、session、conversation、archive/import；
- plans、delegation、fork；
- files、attachments、permission UI/scope/persistence/enforcement；
- artifact、version、preview、provenance；
- annotations、memory、search、context compaction；
- Python/R/Conda、task environment、kernel 和 package lifecycle；
- Skill catalog/discovery、session load、trigger、domain execution；
- generic MCP client/loader；
- Reviewer、Specialist；
- Science updater 与最低支持版本。

CSSwitch 可以隔离、启动、保全或展示这些状态，但不得新增一套平行语义层。
例如：

- Skill discovery/attach 不等于 session load、trigger 或 domain execution；
- persistent data-dir 不等于 project/archive 行为已验证；
- package route/string 不等于 artifact、permission 或 Reviewer live；
- Python/R kernel 可用不等于 GPU 模式可用。

## 4. 窄桥接

只有同时满足以下条件，才应增加或保留 bridge：

1. 隔离 HOME/data-dir 或第三方 model path 确实切断了原本可用的入口；
2. CSSwitch 能把 bridge 限定为明确的输入、输出、身份与权限，不需要接管领域语义；
3. 用户能显式启用/关闭；默认状态和凭证来源清楚；
4. install/attach/start/restart/stop/detach/cleanup 的责任边界可分别验证；
5. bridge 失败只降级对应能力，不破坏 runtime、Gateway 或普通 Agent；
6. 证据只声明到 CSSwitch 所拥有的最后一跳，不从入口成功外推 load、tool call、
   server connectivity、费用或 entitlement。

如果只是为了让 Science 原生 client 在隔离环境中获得用户明确选择的 config、
environment 或 network，优先做可审计的最小投影；不要新增通用管理 UI、registry
或第二套 credential store。

### 4.1 外部 Skill

CSSwitch 只拥有安全来源、安装、投影和 CSSwitch-owned package 的
attach/detach 前置链路。Science Agent/session 拥有 load、trigger，Skill
代码和其工具环境拥有 domain execution。

故障必须按阶段切开：

1. discovery；
2. install；
3. attach/detach；
4. session load；
5. trigger/tool execution；
6. restart persistence。

前三层可能属于 CSSwitch bridge；后三层不能仅凭前置成功归因给 CSSwitch。

### 4.2 SSH / HPC / SLURM

SSH 链路有五个 owner：

1. CSSwitch opt-in、preflight、sidecar/stub transaction；
2. Science Host alias parser 与 host record；
3. wrapper 到系统 OpenSSH 的 invocation；
4. 用户 config、key、agent、known_hosts；
5. DNS、network、server、scheduler 与远端命令。

parser acceptance、OpenSSH invocation、real server connectivity 必须分别判断。
CSSwitch 不读取 key，不拥有 server/scheduler，也不能把系统 OpenSSH fixture
写成真实 Science SSH live。

whole-app remote Linux/WSL 是把 App/CLI、web UI、preview、data-dir 与端口部署
到另一平台，不是本机 Science 的 SSH remote compute，两者不能合并。

### 4.3 Codex

Codex bridge 只拥有 browser-only OAuth、模型目录和 Responses 适配的明确入口，
默认关闭且不读取原生 `~/.codex`。Codex 账号、provider 服务和 Science 行为仍由
各自 owner 决定。

## 5. 明确 non-target

CSSwitch 不托管：

- 真实 Claude OAuth、官方模型 catalog、subscription、usage、entitlement；
- 官方 Skill catalog/marketplace/private sources parity；
- 通用 local/remote MCP 管理面；
- Plugin registry/runtime、hooks、权限、启停与更新；
- Featured/Directory/Anthropic-hosted connectors；
- Web Search、机构/付费文献凭证和 paywall；
- S3/GCS/Azure 凭证、bucket policy 和云数据；
- GPU 主机/driver、Modal 账号/预算、BioNeMo 或其他 endpoint 计费；
- Reviewer/Specialist 的替代实现；
- analytics/Admin API、offboarding、compliance 和组织策略；
- 用户 SSH key、远端 server、scheduler 或主机安全状态。

这些项目的 `UNKNOWN` 用来限制结论，不构成 CSSwitch 的默认 backlog。

## 6. 故障归属

| 症状 | 先定位到 | 不应直接归因 |
|---|---|---|
| Science 起不来、复用/停止/恢复异常 | CSSwitch runtime 包络、identity、隔离 HOME/data-dir | project 或模型能力 |
| 第三方模型请求失败 | profile/selector、Gateway、协议、route、provider | Science 所有网络面 |
| project/session/artifact 状态异常 | Science 本地状态；再检查 CSSwitch 是否破坏隔离或恢复边界 | provider routing |
| 权限卡、grant/revoke 或越界访问异常 | Science permission engine、用户授权与资源权限 | CSSwitch 路径投影 |
| Skill 看不见或无法 attach | CSSwitch discovery/install/attach 前置链路 | session load/domain execution |
| Skill 已 attach 但不 load/trigger | Science Agent/session、Skill runtime 和工具环境 | 安装成功本身 |
| MCP 无法调用 | 先区分 Skill-internal stdio、generic stdio、custom remote、hosted | raw CONNECT 或“有 MCP”这个总标签 |
| SSH 失败 | parser、OpenSSH invocation、用户配置、network/server/scheduler 逐层定位 | CSSwitch 单一 owner |
| Web Search/hosted connector/官方 Reviewer entitlement 不可用 | Anthropic service、账号/org entitlement、Science client 与 network | 第三方模型 Gateway |
| Reviewer 第三方模型分支失败 | Science workflow → Gateway protocol/route → provider 按实际 operation 逐层定位 | 官方 entitlement 或 Science workflow 单一 owner |
| Python/R/Conda/GPU 异常 | Science environment/kernel；GPU 另查主机与安全模式 | CSSwitch 将 opaque roots 当缓存管理 |
| 更新后行为变化 | 先确定实际 runtime identity，再对照能力 owner 和依赖面 | seed App 版本或静态字符串单独定性 |

## 7. 维护不变量与验证

任何实现变更都必须保持以下稳定边界：

1. 每个状态和语义的唯一 owner；
2. `CSSWITCH-RUNTIME`、`MODEL-GATEWAY`、`SCIENCE-NATIVE`、
   `NARROW-BRIDGE`、`SCIENCE-EXTERNAL`、`OFFICIAL-ENTITLEMENT`
   六类可组合 stage，以及语义 stage 与 socket transport 的分离；
3. sandbox ingress/egress 与 protected/opaque 边界；
4. bridge 准入、关闭、重启、补偿与局部降级规则；
5. 现有 Tauri command/event/DTO、Gateway wire behavior、锁序、journal、
   receipt 和 recovery 的行为特征测试；
6. Desktop、transaction、Gateway/provider、runtime adapter 的一键 typed failure
   domain 已由 `runtime/failure.rs` 建立；bridge / Science-native / external
   service 的局部 typed error 保持各自所有权，不并入全局 mega-enum；
7. ambient environment 两层 allowlist 与 sentinel-secret regressions 已闭合
   （见上文与 `runtime/launch_env`）；后续变更不得扩大该 allowlist。

维护这些边界不要求先证明每项 Science 能力 current live，也不要求解决所有
`UNKNOWN`。owner、路径和不变量必须无歧义；具体版本/provider 的兼容结果可以继续是
`UNKNOWN`。实现变化后再绑定 exact Science artifact、CSSwitch artifact、provider/model 和环境，
验证 stream、tools、`tool_choice`、reasoning、structured output、vision、错误
映射、停止/流终止语义、Science-native 状态保全、bridge restart 与独立外部
流量；每项按 provider capability 记录支持或可定位降级。动态结果用于修复
adapter、更新 `UNKNOWN` 和兼容声明；除非上游合同真实改变，不反向扩大
CSSwitch ownership。

## 8. Science 更新影响的最小判断

Science 更新后只回答下面几个问题，不新增一套 gate 或执行流水线：

1. runtime identity、CLI 参数、data-dir 或启动方式是否改变；
2. CSSwitch 必须托管的身份/Gateway/network/state 边界是否改变；
3. 原生能力的 owner 是否改变，还是只增加了 UI/package-static 表面；
4. Skill、MCP、environment、SSH 等窄桥接的接口边界是否改变；
5. 变化当前只在 official/source/package 哪一层，哪些 live 结果仍是
   `UNKNOWN`。

日期化变化留在 audit/evidence；只有稳定 ownership 或托管决策改变时，才更新
能力地图和本文。这样可以快速判断影响，又不会把一次版本调查变成长生命周期
基础设施。

## 9. 文档边界

- 完整能力清单、逐能力 ownership/托管/non-target 结论、当前证据层与
  `UNKNOWN`：只在能力地图的单一决策表维护；
- 本文只维护这些逐项结论的稳定解释规则：跨 owner 依赖、最小托管责任、窄桥接
  与 non-target 类别边界、故障归因；不复制第二套逐能力状态表；
- exact version/hash/package/route/一次观察：留在日期化 audit/evidence；
- 不在这里新增 handoff、probe 执行、GATE-SOURCE、schema、lint 或 lifecycle
  合同；
- source/test/package/fixture 不能写成 artifact、installed-live 或真实服务。
