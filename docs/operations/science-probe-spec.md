# Claude Science 生产链路探针合同

状态：当前

适用范围：当前 CSSwitch production source、由同一 exact source 生成的 artifact、manifest 中固定身份的 Claude Science executable/package，以及由本合同逐项授权的 isolated-live / authorized-live。

最后复核：2026-08-07

失效条件：CSSwitch production owner / registered IPC / caller、artifact composition、Claude Science identity、能力边界、证据词表、安全隔离合同或 probe 目标发生变化时，受影响 card 立即失效并复核；actual result 继续留在 dated evidence，不用旧结果重写本合同。

本文是 Claude Science production-chain probe 的唯一执行合同，只定义 fixture、授权、
gate、判定和证据输出，不记录运行结果，也不提供构建或 live 授权。统一的重构决策映射与层级定义见
[生产链路验收](real-machine-acceptance.md)；能力含义与 owner 见
[Claude Science 能力依赖](../architecture/science-capability-dependencies.md)，用户可见
状态见[产品能力地图](../features/product-science-capability-map.md)；当前 source/result 用语和更高证据层边界分别以[自动测试](testing.md)与[发布流程](release.md)为准。
本文不复制能力或证据正文。

## 1. 规格与结果分界

- probe card 中的目标层、PASS/FAIL/INCONCLUSIVE 判据和前置条件是执行合同，不是
  当前运行状态。
- actual status 只从[日期化调查索引](../evidence/investigations/README.md)及其
  evidence 进入；未建立 evidence 时只能说“没有取得结果”，不能由本规格补写
  `NOT-RUN` 或 PASS。
- 每次新运行只记录实际执行过的 probe ID 和 sub-gate；不能把相邻 probe、mock、
  旧版本、official surface 或 package string 的结果外推过来。
- 进入下一 gate 前必须实时读取依赖 probe 的 exact evidence；不能用本规格初次
  编写时的队列状态替代。

## 2. Gate 顺序

| Gate | 允许进入的队列 | 准入条件与 PASS 完成条件 | 不能外推 |
|---|---|---|---|
| G0 production source | A 中的 source card | 准入：冻结 clean exact source HEAD 并获得静态审查授权。完成：必需 A cards、fresh review 与完整 source gate 共同 PASS | source PASS 不等于可构建或运行 |
| G1 exact artifact | package/static card 与 artifact identity gate | 准入：G0 PASS 并另获构建授权。完成：CSSwitch artifact、Gateway、resource、Science executable/package identity 与同一 source manifest 精确绑定 | package/static PASS 不等于 executable 已运行 |
| G2 isolated-live | B（macOS 或专用可丢弃 Linux/WSL） | 准入：G1 PASS，并另获真实 Science 隔离测试授权，且 fixture 可完全隔离。完成：目标 B card 的 identity、normal wiring、判定与 cleanup 全部 PASS | isolated/local PASS 不等于账号、组织或真实服务 |
| G3 authorized account / org | `C-ACCOUNT-01`、`C-ORG-01` | 准入：相关 B 已闭合，专用账号/组织/设备与逐项书面授权齐备。完成：授权 capability/control 的全部 sub-gate 与 cleanup PASS | entitlement 不等于第三方 provider、其他组织或公开 release |
| G4 authorized external | 其余 C | 准入：相关 B 已闭合，destination、预算、凭证范围、数据与取消方式逐项授权。完成：该 exact subcase 的全部 sub-gate 与 cleanup PASS | 一个 service、region、server 或 provider 不代表其他对象 |

card 中的 `G0` 等前置表示已满足该 gate 的**准入**，不是要求该 card 执行前 gate 已经 PASS；
该 gate 的必需 cards 与 review/gate 结果共同建立 PASS。低 gate 的 `FAIL`、`INCONCLUSIVE`
或 `NOT-RUN` 不能靠进入更高 gate 绕过。存在依赖的 probe
必须按 card 中列出的前置顺序执行；并行只允许用于无共享 runtime、端口、账号、
fixture 或证据目录的独立 case。

## 3. 全局执行合同

### 3.1 身份与隔离

每次运行分配唯一 `<run-id>`，并在开始前冻结：

- probe ID、sub-gate、UTC 开始时间、执行者；
- 仓库 root、branch、HEAD 与 dirty/staged 摘要；
- CSSwitch artifact 的绝对路径、SHA-256、签名身份和构建版本；
- Science binary 的绝对路径、SHA-256、版本和来源；
- OS、架构、fixture 版本，以及所有目标证据层和 scope。

B 类 macOS runtime 必须使用 `/private/tmp/csswitch-science-probe-runtime/<run-id>/home/`
作为外层 HOME，并把 Science data-dir、CSSwitch state 与 mock state 都放在这个 HOME 下；日志放在同一 `<run-id>` 根，使用 OS 分配的动态
端口。不得借用真实 `HOME`、`~/.claude-science`、`~/.csswitch`、端口 `8765` 或
现有进程。Linux/WSL 使用同等语义的专用可丢弃实例和全新用户；不得挂载用户目录。

所有 fixture 使用假 token、假 security 和合成数据。测试内容禁止包含邮箱、真实
项目、真实文档、私人 URL、真实 host alias、真实 bucket 或可识别用户数据。

### 3.2 全局允许、禁止与停止

除 probe card 进一步收紧外：

- A 只允许读取目标 worktree、明确固定的 package 副本和公开记录的 metadata。
- B 只允许访问 probe runtime 根、fixture 根、loopback 和明确列出的系统只读
  executable；默认拒绝 DNS 与非 loopback egress。
- C 只允许访问授权单中列出的账号、组织、destination、region、数据和预算。
- 所有队列均禁止读取或回显 Keychain、真实 OAuth/API token、SSH key/agent、
  账号数据库、私人日志和真实用户状态；凭证只允许由用户显式注入隔离进程，
  runner 只能记录“已提供/未提供”和不可逆 fingerprint。

出现下列任一情况立即停止当前 case，不尝试替代账号、网络或 destination：

1. 路径解析进入真实 HOME、真实用户 data-dir 或非 fixture 文件；
2. 观察到 `8765`、无法归属的进程/端口或非预期外部 egress；
3. 程序请求 Keychain、SSH key/agent、未授权凭证、付费动作或数据上传；
4. 不能在单调 deadline 内完成身份确认、运行或精确归属清理；
5. fixture、artifact、binary、账号、组织、provider、server 或 hash 与 manifest 不符；
6. 脱敏无法保证，或继续执行会扩大权限、费用、数据或外部影响。

安全停止记为 `INCONCLUSIVE(reason=safety-stop)`，不能改写为能力 `FAIL`。已经在停止
前取得的独立 sub-gate 可以保留自己的判定，但不得合成整项 PASS。

### 3.3 判定词

| 判定 | 必须满足 |
|---|---|
| `PASS` | 身份、fixture、前置、允许访问、目标观察和清理全部满足；所有必需 sub-gate 均 PASS |
| `FAIL` | 身份和 fixture 有效且 probe 到达目标路径，但观察到与 card 明确合同相反的产品行为 |
| `INCONCLUSIVE` | 前置/fixture/entitlement 不足、环境或权限阻断、超时、安全停止、观察面不足，或无法把行为归属给目标 artifact/runtime |

“没有崩溃”、字符串存在、HTTP 200、目录可见、UI 可点击、进程启动、TCP CONNECT
或 guard 正确拦截，都不能单独构成 PASS。预期拒绝只有在 card 明确把 fail-closed
行为列为目标时才算 PASS。

### 3.4 故障分支证明边界

crash、race、replacement、compensation、replay、CAS drift、partial write 与取消/超时
必须继续使用 production entry 对应的 deterministic fixture / fault injection。source gate
必须证明 fixture 进入同一 owner、journal、receipt 或 CAS boundary，不能使用平行 mock
实现重新定义产品行为。isolated-live / authorized-live 只证明 normal production wiring 和
明确授权的 happy path；不得故意破坏真实 Science、账号、Skill、SSH 或 provider 状态来取代
故障 fixture，也不得由一次 happy path 推出上述 failure branch PASS。

### 3.5 证据与清理输出

每个 case 写入
`/private/tmp/csswitch-science-probe-evidence/<run-id>/<probe-id>/`：

- `manifest.json`：上述身份、授权引用、target layer/scope、fixture 与 deadline；
- `events.ndjson`：单调时间戳、sub-gate、动作类别和脱敏结果；不存凭证或正文；
- `observations.json`：每个断言的 expected、actual、evidence pointer 和判定；
- `inventory-before.json` / `inventory-after.json`：仅记录可归属进程、端口和路径；
- `cleanup.json`：停止、进程退出、端口释放、runtime 根处理及遗留项；
- `hashes.sha256`：证据文件 hash；原始截图/响应仅在脱敏后进入 `artifacts/`。

运行完成后新增一份日期化 investigation，文件名为
`docs/evidence/investigations/YYYY-MM-DD-claude-science-<science-version>-<probe-id>.md`，
并更新其最近一级索引。正文必须逐 sub-gate 记录 `PASS` / `FAIL` /
`INCONCLUSIVE`、target layer/scope、artifact/binary hash、授权范围、清理结果和
证据目录 hash；未运行项继续写 `NOT-RUN`。不得提交含秘密的原始日志。

清理只终止 manifest 创建且身份匹配的进程，只释放 manifest 记录的动态端口。先
收集 after inventory 与 cleanup 证据，再删除 probe runtime 根；证据根保留到
日期化 investigation 完成并核对 hash。无法精确归属的对象不删除，记录遗留并将
case 判为 `INCONCLUSIVE(reason=cleanup-ownership)`。

## 4. A｜静态 / package

本节只冻结 A 类执行与判定合同，不保存运行状态。实际结果从
[日期化调查索引](../evidence/investigations/README.md)进入；A 类不能启动
executable、打开 App、连接网络或读取用户状态。

### A-IPC-01｜production entry 可达性清单

- **目标能力**：为 current Tauri command 固定 `defined → compiled → registered →
  production caller/auto-boot → owner` 静态链；已从注册面删除的 command 必须列为
  absent，并证明当前生产链不再依赖它。
- **前置与 fixture**：G0；exact source HEAD；Rust module/invoke 注册和生产
  frontend caller 文件；生成物写入证据根。
- **允许 / 禁止访问**：允许只读目标 worktree 和本地文本分析；禁止 build、
  启动 UI/runtime、读取 preview/mock 结果后冒充生产 caller。
- **所需授权**：无需账号、网络或凭证；只需该 exact HEAD 的静态审查任务授权。
- **目标证据层**：`PRODUCTION-SOURCE`。
- **PASS**：每个 registered command 都有定义、编译入口、注册点、production caller /
  conditional auto-boot / no production caller 三态之一及路径行号；preview/mock 单列；
  `start_proxy` 等已删除注册面不再被清单当作 live command，且一键链只经 registered
  `one_click_login` / boot coordinator 进入当前 Gateway / Science owner。
- **FAIL**：清单漏项、重复归属、把 test/preview 当生产 caller，或注册集合与
  exact HEAD 不一致。
- **INCONCLUSIVE**：生成器无法解析宏/条件编译且人工复核也不能唯一归类。
- **停止与清理 / 输出**：遇到 source HEAD 漂移立即停止；不改源码，输出
  `ipc-inventory.json`、路径行号和差异摘要。

### A-PLUGIN-01｜Science Plugin 与 CSSwitch importer 矩阵

- **目标能力**：固定 Science package 的 Plugin schema/UI/import 表面，与
  CSSwitch root-Skill、Plugin candidate、hooks/MCP/agents 处理路径的组合矩阵。
- **前置与 fixture**：G1；固定目标 Science package 副本、version、build 与 SHA-256；exact source
  HEAD；只使用合成 archive 结构做静态路径枚举，不执行 archive 内容。
- **允许 / 禁止访问**：允许 package-static metadata/string/schema 和 importer
  source；禁止启动 package、导入真实 Plugin、读取用户 marketplace 或 Skill 数据。
- **所需授权**：无需账号/凭证；package 路径和 hash 必须在 manifest 中固定。
- **目标证据层**：Science 表面为 `PACKAGE-STATIC`，CSSwitch 路径为
  `PRODUCTION-SOURCE`；两层分别判定，只有 manifest 绑定后才进入 `EXACT-ARTIFACT`。
- **PASS**：矩阵覆盖 root `SKILL.md` 短路、Plugin candidate、manifest、UI、
  hooks、MCP、agents、permission、enable/disable、update；每格标出已解析、拒绝、
  未进入或未知，且不声称 Plugin runtime 支持。
- **FAIL**：package hash 不符、矩阵漏掉已知分支，或把 package surface /
  Skill 子集写成 live Plugin lifecycle。
- **INCONCLUSIVE**：package 混淆/压缩使静态入口无法可靠识别；对应格保持 unknown。
- **停止与清理 / 输出**：任何执行请求立即停止；不解包到用户目录，输出
  `plugin-surface.json`、`importer-path-matrix.json` 和双层判定。

### A-SSH-01｜SSH wrapper 期望与 validator 事实

- **目标能力**：把 wrapper exact content/hash/owner/mode/nlink 期望，与 validator
  实际检查和 source fixture 分开。
- **前置与 fixture**：G0；exact source HEAD；仓内 wrapper、installer/validator、
  settings 和测试 fixture。
- **允许 / 禁止访问**：允许读取仓内 SSH bridge 文件与 source tests；禁止读取
  `~/.ssh`、key/agent、known_hosts，禁止执行 ssh 或连接 host。
- **所需授权**：无需账号、server 或 SSH 凭证。
- **目标证据层**：`PRODUCTION-SOURCE`；已存在但未运行的测试只记
  `SOURCE-TEST(status=not-run)`。
- **PASS**：期望、安装写入、validator 检查、runtime invocation 四列逐字段对应；
  content/hash/owner/mode/nlink 的未校验项显式标出，不从测试存在推出 PASS。
- **FAIL**：validator 接受范围宽于文档安全合同却未标出，或 wrapper 内容与调用
  约定冲突。
- **INCONCLUSIVE**：平台条件编译使 exact HEAD 无法确定单一 validator 路径。
- **停止与清理 / 输出**：发现访问用户 SSH 状态的需求立即停止；输出
  `ssh-wrapper-contract.json` 和 source pointer。

### A-EVIDENCE-01｜证据层用语门禁

- **目标能力**：保证未来 probe evidence 只在实际取得的 production source、exact artifact、
  isolated-live、authorized-live 及额外 installed/signing/public-release scope 内陈述。
- **前置与 fixture**：G0；统一证据链、本文 schema，以及一组覆盖 source/package/
  installed/live/release 混层的合成正反例。
- **允许 / 禁止访问**：允许只读文档与合成 evidence；禁止读取 runtime/账号，
  禁止借历史 PASS 自动填充当前字段。
- **所需授权**：无需外部授权。
- **目标证据层**：`PRODUCTION-SOURCE`（证据文档合同本身），不产生产品能力证据。
- **PASS**：每个正例保留层、scope、artifact identity 和 `NOT-RUN`；每个越层反例
  被拒绝，`RELEASE-METADATA` 不升级为 `PUBLIC-RELEASE`。
- **FAIL**：任一 source/package/mock/installed/release 越层样例被接受，或
  `NOT-RUN` 被当成独立证据层。
- **INCONCLUSIVE**：词表或 schema 发生版本冲突，无法唯一判定。
- **停止与清理 / 输出**：发现词表已变化立即停止并先修订规格；输出
  `evidence-schema.json`、case 结果和词表版本指针。

## 5. B｜隔离 HOME + fixture/mock

本节只冻结 B 类 isolated-live 执行与判定合同，不保存运行状态；实际结果从
[日期化调查索引](../evidence/investigations/README.md)进入。`B-RUNTIME-01` 和
`B-PLATFORM-01` 只依赖各自 card 列出的 A 类前置；其余 B probe 都依赖
`A-EVIDENCE-01=PASS`、`EXACT-ARTIFACT=PASS` 和 `B-RUNTIME-01=PASS`；不得读取真实账号或发生外部
egress。

### B-RUNTIME-01｜CSSwitch → Gateway → Science production chain

- **目标能力**：从 exact CSSwitch artifact 的 production executable，经 registered
  Tauri IPC / auto-boot、真实 Gateway sidecar 与真实 Science executable 闭合 start、
  open/reopen、status、stop、restart 及 executable/data-dir identity。
- **前置与 fixture**：G2；A-IPC、A-EVIDENCE 与 EXACT-ARTIFACT PASS；固定 CSSwitch
  artifact / Gateway / Science hash；临时外层 HOME 及其下 data-dir/state；动态端口；
  loopback deterministic provider fixture。不得以直接调用内部 Rust function、test-only
  handler、mock CSSwitch executable 或 mock Science lifecycle 替代 production entry。
- **允许 / 禁止访问**：允许 probe 根、loopback、目标 bundle 内 executable；
  禁止 `/Applications` 用户实例、真实 HOME、8765、外网和现有 daemon。
- **所需授权**：仅 B 阶段隔离动态执行授权；无账号或凭证。
- **目标证据层**：`EXACT-ARTIFACT` identity +
  `ISOLATED-LIVE(scope=csswitch-gateway-science,loopback-provider-fixture)`；分别记录。
- **PASS**：每个生命周期动作返回与 inventory 一致的 identity；reopen 不产生
  第二 daemon；stop 释放归属端口；restart 保持指定 data-dir 且仍执行同一 binary。
- **FAIL**：有效 fixture 下身份漂移、复用错误进程、状态与 inventory 矛盾、停止后
  留下归属进程/端口，或 restart 改用未固定 binary。
- **INCONCLUSIVE**：artifact 不是 final、mock 不能满足最小启动、系统阻止隔离，
  或无法证明进程归属。
- **停止与清理 / 输出**：任何真实路径/8765/外连触发全局停止；输出 lifecycle
  timeline、PID/executable inventory、port ownership、status response 与 cleanup。

### B-CORE-01｜project、文件、artifact、annotation 与 permission

- **目标能力**：按 read、write、request、grant、revoke、越界拒绝、artifact
  lineage 分 gate 验证本地核心对象。
- **前置与 fixture**：G2；B-RUNTIME PASS；仅含合成文本/PDF/PNG/HTML 的 project
  fixture；fixture 内允许目录与相邻禁止目录；deterministic model mock。
- **允许 / 禁止访问**：只允许 fixture project 和显式 grant path；禁止真实文件、
  上级目录、网络、standing grant 复用到其他 run。
- **所需授权**：B 阶段授权；grant 只针对合成 fixture，由执行者在测试 UI 明示。
- **目标证据层**：`ISOLATED-LIVE(scope=isolated-local-mock)`。
- **PASS**：每个 sub-gate 单独满足：未授权访问先请求；grant 后仅目标 scope 可读写；
  revoke 后拒绝；越界始终拒绝；artifact version/diff/preview/execution provenance
  可回读；annotation 定位和下一消息传递可观察。
- **FAIL**：有效 fixture 下越界成功、revoke 后仍可访问、写错路径、lineage 指向
  错对象，或 UI/状态宣称成功但文件/record 不一致。
- **INCONCLUSIVE**：mock 无法触发所需 Agent 行为、格式不被目标 Science 支持、或只能
  观察 UI 而不能核对持久状态。
- **停止与清理 / 输出**：首次非 fixture 路径请求即停止；输出 permission events、
  fixture tree before/after、artifact/annotation assertion，不保存正文。

### B-CONTEXT-01｜plan、delegation、fork、memory 与 Reviewer surface

- **目标能力**：只验证本地 UI/surface、状态变更和 outbound request 形态，不验证
  Anthropic entitlement 或 Reviewer 质量。
- **前置与 fixture**：G2；B-RUNTIME 与 B-CORE PASS；deterministic model mock
  记录脱敏 request envelope；两 project、两个 session 的合成标记。
- **允许 / 禁止访问**：允许 fixture session/state 和 loopback recorder；禁止真实
  Claude endpoint、账号、Web Search 或把 mock response 当 Reviewer success。
- **所需授权**：B 阶段授权；无账号/凭证。
- **目标证据层**：`ISOLATED-LIVE(scope=isolated-request-shape)`。
- **PASS**：plan approve/reject、delegation、fork/restore、memory save/search/
  compaction 和 Reviewer/Specialist surface 各有独立可观察状态或 request shape；
  project/session 标记不串域；服务端结果保持未验证。
- **FAIL**：有效 local fixture 下操作落错 session/project、fork 覆盖源状态、
  memory 串域，或 request shape 与选定动作矛盾。
- **INCONCLUSIVE**：目标 Science 只在 entitlement 后暴露入口，或 recorder 不能区分
  动作；对应 sub-gate 不作推断。
- **停止与清理 / 输出**：任何真实服务 destination 立即停止；输出 surface matrix、
  state transitions 和脱敏 request schema。

### B-SKILL-01｜外部 Skill 六阶段闭环

- **目标能力**：从 exact artifact 的 Science Agent 加载受管 route/connector，调用
  `csswitch-skill-installer` MCP 的 `install_external_skill`，再按 GitHub install → attach →
  Agent load → tool/poll → uninstall → restart persistence 逐阶段验证。
- **前置与 fixture**：G2；B-RUNTIME PASS；artifact 中的 `skill-install-mcp`、受管
  route 与 connector identity 已核对；本地 loopback Git server 或精确打包的
  无网络 GitHub-response fixture；CSSwitch-owned Skill package；deterministic
  echo tool/poll。
- **允许 / 禁止访问**：只允许 fixture repository、probe Skill roots 和 loopback；
  禁止公网 GitHub、真实 `~/.csswitch/skills`、用户 Science Skill 与任意 package code
  越出 fixture；禁止直接调用 installer core/helper、直接执行 `skill-install-mcp` 或
  使用 test-only host 注入绕过 Science Agent → managed MCP production route。
- **所需授权**：B 阶段授权；无 GitHub/账号凭证。
- **目标证据层**：`ISOLATED-LIVE(scope=isolated-skill-fixture)`。
- **PASS**：Science Agent request、managed connector、packaged Gateway MCP 与
  `install_external_skill` tool identity 可串联；六阶段各自有 identity/receipt；load 发生在目标 session；tool/poll
  返回 fixture nonce；uninstall 只移除 owned package/binding；restart 后预期的
  attached/detached 与 load 状态可回读。
- **FAIL**：attach 被当 load、tool 未执行却宣称成功、poll 串 run、卸载越界，或
  restart 状态与 receipt 冲突。
- **INCONCLUSIVE**：fixture transport 不能经 production MCP route 表达 installer 输入、
  Science Agent 未调用 managed tool / 未触发 load，或只能证明前若干阶段。
- **停止与清理 / 输出**：任何公网请求或真实 Skill root 访问即停止；输出六阶段
  ledger、route revision、receipt 和 owned-path cleanup。

### B-MCP-01｜local stdio echo MCP

- **目标能力**：验证 generic local stdio MCP 的注册、工具发现、permission、
  调用和 restart；不把外部 Skill 内部 MCP 自动外推为通用管理支持。
- **前置与 fixture**：G2；B-RUNTIME PASS；固定 hash 的无网络 stdio echo server，
  最小 command/args/env，合成 nonce。
- **允许 / 禁止访问**：允许启动该 fixture executable 和 pipe；禁止 shell 扩展、
  用户 env、文件/网络访问及任何其他 command。
- **所需授权**：B 阶段授权；无账号/凭证。
- **目标证据层**：`ISOLATED-LIVE(scope=isolated-local-stdio)`。
- **PASS**：注册 identity 准确；只发现预期 tool；permission scope 正确；调用返回
  nonce；restart 后按配置语义恢复；fixture 进程被精确清理。
- **FAIL**：命令/env 被篡改、额外 tool 暴露、未授权调用成功、restart 丢/串配置，
  或退出后残留 owned server。
- **INCONCLUSIVE**：入口被 entitlement 隐藏、UI 可配置但 loader 未启动，或无法
  证明调用来自目标 server。
- **停止与清理 / 输出**：server 尝试文件/网络访问立即停止；输出 registration、
  tool list、permission event、stdio transcript 的 schema/hash 和 cleanup。

### B-MCP-02｜loopback Remote MCP 与 network preference A/B

- **目标能力**：分别验证 legacy SSE 与 Streamable HTTP 的连接、tool discovery、
  permission、调用、断线恢复，以及 network preference 是否影响该流量。
- **前置与 fixture**：G2；B-RUNTIME PASS；两个仅 loopback、固定协议行为的 MCP
  server；A/B run 除 network preference 外完全相同。
- **允许 / 禁止访问**：只允许 manifest 中的 loopback URL；禁止 DNS、OAuth、
  任意 header secret、外部 CONNECT 和 hosted endpoint。
- **所需授权**：B 阶段授权；使用假 header，不用真实 OAuth。
- **目标证据层**：`ISOLATED-LIVE(scope=isolated-loopback-remote-mcp)`。
- **PASS**：两 transport 各自完成 discovery/permission/call/reconnect；A/B 的
  request path 与结果可归因于 preference，未受控差异为零。
- **FAIL**：有效 fixture 下协议路由错误、tool/auth scope 混淆、permission 绕过，
  或 preference 造成与合同相反的可复现结果。
- **INCONCLUSIVE**：仅 TCP/HTTP 连通、UI 保存但 client 未调用，或 A/B 存在未控制
  差异。
- **停止与清理 / 输出**：任何非 loopback destination 立即停止；输出 transport
  transcript schema、A/B manifest、tool assertion 与 server cleanup。

### B-ENV-01｜Python、R、shell、Node 与环境持久性

- **目标能力**：区分 starter/task env、Python/R kernel、shell、Node、persistent
  env package、inline package 和 compute monitor。
- **前置与 fixture**：G2；B-RUNTIME PASS；合成 project；离线 wheel/R package/
  npm fixture；小型 CPU/time/memory workload；每个 runtime 唯一 nonce。
- **允许 / 禁止访问**：只允许 probe env/cache 和离线 package；禁止 root、sudo、
  apt、外网 registry、GPU、用户 Conda/R/Node 环境。
- **所需授权**：B 阶段授权；无账号/凭证。
- **目标证据层**：`ISOLATED-LIVE(scope=isolated-local-environment)`。
- **PASS**：Python/R/shell/Node 各自标出 owner/scope；named env 跨 project 的实际
  行为、kernel 隔离、persistent/inline package 在 restart 前后按预期分离；
  compute monitor 对 fixture workload 可归属。
- **FAIL**：环境或 package 串到用户路径/其他 run，restart 持久性与声明相反，
  Node owner 被无证据合并，或 monitor 指错进程。
- **INCONCLUSIVE**：runtime 不存在、离线 package 不兼容、UI 无法揭示 scope，
  或只能完成语言子集。
- **停止与清理 / 输出**：出现提权/外网/package manager 非 fixture 请求即停止；
  输出 env identity、package inventory、kernel lifecycle、monitor samples 和 cleanup。

### B-PLATFORM-01｜专用 Linux / WSL whole-app

- **目标能力**：验证 whole-app 双端口、preview origin、data-dir、CLI import 和
  rollback 边界；不与本机 SSH remote compute 合并。
- **前置与 fixture**：G2；A-EVIDENCE 与 EXACT-ARTIFACT PASS；固定目标 Science Linux/WSL package；
  专用可丢弃 VM/WSL 实例、全新用户、合成 import archive、动态端口。
- **允许 / 禁止访问**：只允许实例内部 fixture 与由宿主明确映射的 loopback；
  禁止挂载用户 HOME、SSH key、真实项目、外网和共享 credential store。
- **所需授权**：B 阶段专用实例授权；无账号/凭证。
- **目标证据层**：`ISOLATED-LIVE(scope=disposable-linux-or-wsl-local-mock)`；
  Linux 与 WSL 分 case。
- **PASS**：web/preview 两端口身份不混；origin 限制符合 fixture；data-dir 重启可
  回读；import 只影响目标实例；rollback 恢复指定 snapshot 且不越界。
- **FAIL**：端口/preview 身份混淆、data-dir 落入宿主用户数据、import/rollback
  影响非目标路径，或 executable identity 漂移。
- **INCONCLUSIVE**：平台 package/virtualization 不可用、无法隔离网络/挂载，
  或只完成 Linux/WSL 之一。
- **停止与清理 / 输出**：发现宿主用户目录或外网访问立即停止；输出实例 image
  identity、port/origin map、data/import hashes、rollback diff 和实例销毁记录。

### B-DATA-01｜fake S3-compatible 与 Featured registration

- **目标能力**：验证 loopback fake S3-compatible 的 registration/import 请求，
  以及无凭证 Featured connector 的本地 registration surface；不声称真实服务调用。
- **前置与 fixture**：G2；B-RUNTIME PASS；loopback fake object store、合成 bucket/
  object、假 credentials；无网络 Featured registration fixture。
- **允许 / 禁止访问**：只允许 fixture endpoint/bucket；禁止真实 cloud host、
  metadata service、Featured destination、用户凭证和非预期上传。
- **所需授权**：B 阶段授权；只用假凭证。
- **目标证据层**：`ISOLATED-LIVE(scope=isolated-registration-and-fake-data)`。
- **PASS**：S3-compatible registration、list/read/import 的 endpoint、bucket、
  auth shape 和对象 hash 匹配；Featured 只判 registration surface；两者分开。
- **FAIL**：有效 fixture 下发往错误 endpoint、上传未授权对象、凭证出现在日志，
  或把 Featured registration 写成真实 connector success。
- **INCONCLUSIVE**：client 强制 TLS/public DNS/entitlement，或只能保存配置不能
  观察数据请求。
- **停止与清理 / 输出**：任何 cloud metadata/public destination 请求立即停止；
  输出 fake-store transcript schema、object hashes、registration record 和 cleanup。

### B-SSH-01｜Science parser acceptance

- **目标能力**：只验证 Science 对合成 SSH config/Host alias 的 parser acceptance。
- **前置与 fixture**：G2；A-SSH 与 B-RUNTIME PASS；probe HOME 下合成 `.ssh/config`
  和不存在的 recorder destination；覆盖 Include、Host、ProxyJump 与拒绝样例。
- **允许 / 禁止访问**：只允许读取 fixture config；禁止执行 ssh、读取用户
  `~/.ssh`、key/agent/known_hosts 或建立 socket。
- **所需授权**：B 阶段授权；无 server/SSH 凭证。
- **目标证据层**：`ISOLATED-LIVE(scope=isolated-parser-only)`。
- **PASS**：接受/拒绝矩阵与 fixture 逐例一致；只证明 parser，不记录 connectivity。
- **FAIL**：有效 fixture 下接受危险/越界 path、拒绝合同内安全 alias，或读取用户
  config。
- **INCONCLUSIVE**：parser 入口只有在后续 connect 才执行，无法在禁止 socket
  条件下观察。
- **停止与清理 / 输出**：出现 ssh 执行或 socket 即停止；输出 parser matrix、
  config hashes、访问 inventory 和 cleanup。

### B-SSH-02｜wrapper 与 OpenSSH invocation recorder

- **目标能力**：无网络证明 wrapper identity、`/usr/bin/ssh -F`、Include 与 env
  传递；不连 server。
- **前置与 fixture**：G2；A-SSH、B-RUNTIME、B-SSH-01 PASS；固定 wrapper；
  只记录 argv/env/path 的无网络 recorder 与合成 config。
- **允许 / 禁止访问**：允许执行 fixture wrapper/recorder 和只读验证
  `/usr/bin/ssh` identity；禁止真实 connect、用户 config/key/agent、DNS 和 socket。
- **所需授权**：B 阶段授权；无 server/SSH 凭证。
- **目标证据层**：`ISOLATED-LIVE(scope=isolated-invocation-only)`。
- **PASS**：wrapper hash/owner/mode/nlink 符合 A 期望；argv 精确包含预期
  `/usr/bin/ssh -F <fixture>` 语义；Include/env 只来自 fixture；recorder 证明没有
  network syscall。
- **FAIL**：调用绕过 wrapper/`-F`、继承用户 SSH env、参数注入、wrapper identity
  漂移或产生网络。
- **INCONCLUSIVE**：平台无法在不连接的情况下替换/观察目标 invocation。
- **停止与清理 / 输出**：首次 socket/DNS/用户 SSH path 访问即停止；输出
  wrapper identity、argv/env allowlist、syscall/network assertion 和 cleanup。

### B-PLUGIN-01｜无害 Science 原生 Plugin fixture

- **目标能力**：验证 Science 原生 Plugin 最小 lifecycle surface；不测试 CSSwitch
  通用 Plugin 管理（该管理面是非目标）。
- **前置与 fixture**：G2；A-PLUGIN PASS 且找到明确可执行的 Science 原生入口；
  B-RUNTIME PASS；无 hooks/MCP/agents/网络/文件权限的最小合成 Plugin。
- **允许 / 禁止访问**：只允许 fixture Plugin 与 probe state；禁止 hooks、MCP、
  agents、用户 marketplace、真实 Plugin 和外部下载。
- **所需授权**：B 阶段授权；无账号/凭证。
- **目标证据层**：`ISOLATED-LIVE(scope=isolated-native-plugin-fixture)`。
- **PASS**：A 已确认的最小 install/enable/disable/remove/restart sub-gate 按实际
  入口闭合，identity 和 state 可回读；未在 A 确认的生命周期仍保持 NOT-RUN。
- **FAIL**：有效入口/fixture 下状态与 UI/record 冲突、remove 越界、restart 恢复
  错 identity，或执行了 fixture 禁止组件。
- **INCONCLUSIVE**：A 未找到明确原生入口、入口要求账号/catalog、或只能观察静态
  surface；此时不得构造隐藏 route 继续。
- **停止与清理 / 输出**：出现下载、hook/MCP/agent 执行或用户 marketplace 访问
  立即停止；输出 lifecycle matrix、state diff、identity 和 cleanup。

## 6. C｜Authorized live：逐项明确授权

每个 C case 必须有独立授权记录，至少写明 probe ID/subcase、账号/组织/
destination、允许的数据、凭证注入方式、请求/费用上限、取消方式、时间窗和证据
脱敏规则。一个 C 授权不得复用于其他 subcase；是否已经执行只查日期化 evidence。

### C-PUBLIC-01｜开放文献或公开 Featured connector

- **目标能力**：对一个明确开放、无需账号的文献 source 或 Featured connector
  做最小只读网络请求。
- **前置与 fixture**：G4；相关 B-DATA PASS；固定公开 URL、许可/robots/请求方法、
  响应大小和一次请求上限；合成查询词。
- **允许 / 禁止访问**：只允许授权 URL 和必要重定向 allowlist；禁止登录、paywall
  绕过、cookie、上传、本地文件及第二 destination。
- **所需授权**：逐 URL 的公开网络访问授权；任何凭证/付费提示使授权失效。
- **目标证据层**：`AUTHORIZED-LIVE(scope=authorized-public-source)`。
- **PASS**：目标 runtime 发出不超过上限的只读请求，返回公开内容/metadata，
  destination、license boundary 和无上传可核对。
- **FAIL**：有效公开 fixture 下请求路径错误、发生非预期上传/额外 destination，
  或产品宣称成功但没有目标响应。
- **INCONCLUSIVE**：限速、地区、许可变化、challenge、服务故障或请求凭证/付费。
- **停止与清理 / 输出**：首次凭证/付费/上传/非 allowlist redirect 即停止；输出
  URL origin、method/status/byte count、response hash 和脱敏摘要。

### C-ACCOUNT-01｜专用测试账号 entitlement

- **目标能力**：逐项验证 Web Search、catalog、Directory、hosted MCP、Plugin 和
  Reviewer entitlement；不得聚合成一个账号 PASS。
- **前置与 fixture**：G3；相关 B-CONTEXT/B-MCP/B-PLUGIN 已有结果；专用测试账号、
  空白 profile/project、明确订阅与 region；每项单独 case。
- **允许 / 禁止访问**：只允许测试账号和该项官方 endpoint；禁止个人账号、真实
  历史、联系人、私人 connector、跨项浏览或修改订阅。
- **所需授权**：逐 capability 的测试账号授权；token 只注入隔离进程且不记录。
- **目标证据层**：`AUTHORIZED-LIVE(scope=test-account,<capability>,<plan>,<region>)`。
- **PASS**：目标入口、entitlement 决定、最小调用和结果 identity 对该 capability
  一致；各项单独判定，Reviewer 只证明实际输入/输出边界，不声称方法质量。
- **FAIL**：账号 entitlement 明确有效且 fixture 合法，但目标调用可复现地违背
  官方/产品合同，或跨 capability/账号泄漏状态。
- **INCONCLUSIVE**：plan/region/rollout 不明、登录挑战、配额、服务故障或授权项
  未开放。
- **停止与清理 / 输出**：请求个人数据、额外 scope、购买/升级或未知 endpoint
  即停止；输出账号不可逆 fingerprint、plan/region、逐项 request/result 和 logout。

### C-ORG-01｜测试组织、telemetry、Admin 与 offboarding

- **目标能力**：在测试组织和专用设备上分别验证 analytics/Admin API、telemetry、
  organization policy、offboarding 与设备本地残留。
- **前置与 fixture**：G3；`B-RUNTIME-01`、`B-CORE-01` 和 `B-CONTEXT-01` PASS；
  专用组织、管理员/成员测试身份、可擦除设备或 VM、合成 project；server-side
  与 device-local inventory 基线。
- **允许 / 禁止访问**：只允许测试组织/设备和合成数据；禁止生产组织、个人设备、
  真实成员、真实项目和跨 tenant 查询。
- **所需授权**：组织 owner 对角色、endpoint、telemetry capture、offboarding 和
  设备检查逐项授权。
- **目标证据层**：`AUTHORIZED-LIVE(scope=test-org,test-device,<control>)`。
- **PASS**：每个 control 单独闭合；offboarding 同时记录 server access 变化与本地
  artifact/state 残留，二者不互相外推。
- **FAIL**：有效测试组织下策略未生效、API 越权、telemetry 超出声明，或把服务端
  删除写成本地已清除。
- **INCONCLUSIVE**：组织功能未 entitlement、审计窗口未到、设备 inventory 不完整
  或 role 无法唯一归属。
- **停止与清理 / 输出**：发现生产 tenant/真实成员/额外 telemetry 字段即停止；
  收集 after inventory 后恢复测试组织的角色和策略、撤销测试身份的 session/token、
  删除合成组织数据，并擦除或销毁专用设备/VM；任一清理回读失败则
  `INCONCLUSIVE(reason=cleanup)`。输出角色/策略 fingerprint、API/telemetry
  schema、offboarding 前后双 inventory，以及组织恢复、身份撤销、数据删除和设备
  擦除/销毁 receipt。

### C-EXTERNAL-01｜真实付费或外部计算/数据服务

- **目标能力**：Modal、BioNeMo/inference endpoint、真实 cloud、付费文献和 GPU
  五类各自独立验证；本 probe ID 不是共享授权。
- **前置与 fixture**：G4；对应 B-ENV 或 B-DATA PASS；每类固定 service/region、
  最小合成输入、请求/费用/运行时上限、取消命令和资源标签。
- **允许 / 禁止访问**：只允许该 subcase 的 service/resource；禁止跨 service、
  生产 bucket、真实科研数据、无限重试、后台常驻和未授权模型/region。
- **所需授权**：每类单独授权最小权限凭证、最高费用、最大时长、数据与删除/
  取消方式；GPU 只用专用无秘密主机。
- **目标证据层**：`AUTHORIZED-LIVE(scope=<service>,<region>,<resource>,<budget>)`。
- **PASS**：该 subcase 的 submit/read-or-result/cancel-or-complete/cleanup 全闭合，
  费用和资源不超过上限；cloud read/write/delete、付费文献许可、GPU sandbox 风险
  分别记录。
- **FAIL**：有效授权/服务下错误资源或 region、超预算、取消无效、数据越界、
  资源泄漏，或结果 identity 不匹配。
- **INCONCLUSIVE**：配额/许可/库存/服务故障、凭证 scope 不足、安全策略阻断，
  或无法证明 cleanup/费用。
- **停止与清理 / 输出**：接近预算/时限、请求新 scope、出现真实数据或取消失败
  立即停止并执行预授权取消；输出 service receipt、resource tags、usage/cost、
  result hash 和 deletion/cancellation receipt。

### C-SSH-01｜真实 SSH server

- **目标能力**：在一个专用 server 上分别验证 parser、host key、auth、network 和
  最小远端命令；不把任一阶段合并。
- **前置与 fixture**：G4；A-SSH、B-SSH-01/02 PASS；专用测试 host/user、固定 host
  key fingerprint、最小目录和无秘密命令；连接/命令次数上限。
- **允许 / 禁止访问**：只允许授权 host/port/user 和测试目录；禁止用户真实
  key/agent、ProxyJump 到其他 host、sudo、scheduler/生产目录和任意转发。
- **所需授权**：逐 host 的 server owner 授权；专用短期 credential 由用户注入
  隔离进程，不读取内容。
- **目标证据层**：`AUTHORIZED-LIVE(scope=authorized-test-ssh-server)`。
- **PASS**：parser、host-key verification、auth、network 和 allowlisted remote
  command 各自 PASS；命令只创建/读取/删除 nonce 文件，server 无残留。
- **FAIL**：有效 server/credential 下 host key 未校验、连接错误 host/user、
  auth/env 泄漏、命令越界或清理失败。
- **INCONCLUSIVE**：DNS/network/server downtime、credential scope、host-key
  rotation 或 scheduler 环境阻断。
- **停止与清理 / 输出**：host key 不符、跳转/提权/额外 host/目录请求立即停止；
  输出各 gate、server fingerprint、command allowlist、nonce hash 和远端清理回读。

### C-PROVIDER-01｜真实第三方 provider/model

- **目标能力**：验证一个明确 provider/model/region 的认证、模型身份、最小非流式/
  流式请求、配额错误和停止后的路由恢复。
- **前置与 fixture**：G4；B-RUNTIME PASS；专用 provider project/key、固定 model
  ID、合成 prompt、请求/token/费用上限和无自动重试。
- **允许 / 禁止访问**：只允许授权 provider origin/model；禁止模型自动替换、
  其他 provider、用户真实 prompt/file、训练/存储 opt-in 和未授权遥测。
- **所需授权**：逐 provider/model 的最小权限 credential、预算、region 和数据
  retention 授权；credential 仅注入隔离进程。
- **目标证据层**：`AUTHORIZED-LIVE(scope=<provider>,<model>,<region>,<quota>)`。
- **PASS**：upstream identity、selector、实际响应 model、stream/non-stream 和
  usage receipt 一致；故意超出一个受控软限额时错误分类正确且不 fallback；stop 后
  不遗留该 route。
- **FAIL**：有效授权下错 model/provider、静默 fallback、凭证泄漏、usage 超限、
  错误分类导致重试风暴或 stop 后仍路由。
- **INCONCLUSIVE**：provider outage、quota/region 不确定、model rollout 变化、
  policy refusal 或无法取得可归属 usage。
- **停止与清理 / 输出**：任何新 origin/model、预算逼近、真实数据请求或日志泄密
  立即停止；输出 provider/model fingerprint、request/response schema、usage/
  error receipt、route before/after 和 credential-free cleanup。

## 7. 执行批次与关闭条件

新执行任务按未闭合项及其依赖选择下列批次，不能因为较早批次已有部分 evidence
就整批重跑，也不能跳过未满足前置：

1. A source 批次：只运行 current owner / caller / fixture 发生变化或尚未闭合的项目；
2. exact artifact 批次：只有另获构建授权后，才从同一 exact source 生成并核对 artifact / Science identity；
3. B isolated-live 基础批次：`B-RUNTIME-01` 未取得有效 PASS 时按其 evidence 限制处理；
4. B isolated-live 能力批次：按已满足依赖选择独立 probe，一次只共享一个受管 runtime；
5. C authorized-live 账号/组织批次：每个 capability/control 单独授权；
6. C authorized-live 外部批次：每个 destination/service/server/provider 单独授权。

每批完成后使用不继承上下文的 reviewer，只读取本规格、该批 evidence、必要索引与
最多两份目标正文。reviewer 必须按 BLOCK/HIGH/MEDIUM/LOW 报告证据越层、安全边界、
fixture 有效性、判定和清理。BLOCK/HIGH 修复后必须换新 reviewer；未解决的
MEDIUM/LOW 进入 evidence 限制，不得静默升级。

只有 actual evidence 写入并通过相应审查后，才能更新日期化 audit/evidence。产品
能力或稳定 owner/边界真的变化时，才同步 feature/architecture；规格 PASS 本身
不自动改变产品状态。未取得的项按目标层保留
`NOT-RUN(target=EXACT-ARTIFACT|ISOLATED-LIVE|AUTHORIZED-LIVE)`。
