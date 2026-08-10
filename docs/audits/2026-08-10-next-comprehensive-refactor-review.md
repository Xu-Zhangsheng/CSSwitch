# `next` 全仓重构收尾审查（2026-08-10）

状态：日期化审查，非当前事实源

适用范围：`next@00a63088f93774b37d184ca11bcc2c85936e24b3`；源码、测试合同、架构文档与本地隔离 source-gate 观察

最后复核：2026-08-10

本页保存一次固定基线的全面审查结果，供后续窗口修复和重新验收使用。当前问题、唯一 NEXT 与证据晋升仍从 [known issues](../../.agents/context/known-issues.md) 和 [生产链路验收](../operations/real-machine-acceptance.md) 进入；本页不能把旧 SHA、source 测试或本地隔离观察升级为 artifact、installed、authorized-live 或 release 结论。

## 1. 结论与审查边界

- 最终 verdict：`FAIL`；`BLOCK 0`。
- 固定对象：`next@00a63088f93774b37d184ca11bcc2c85936e24b3`。
- 主工作树审查前后均 clean；本轮没有修改产品代码、Git、真实 runtime 或用户数据。
- 共执行 7 条 clean-context 独立审查线，覆盖 tracked 文件清单、Desktop/Tauri、Gateway、Science runtime、前端、Skill/SSH、测试门禁与架构文档。
- 4 个 HIGH 均由第二位独立 `gpt-5.6-sol/high` 复核，结论均为 `TRUE/HIGH`。
- 未读取真实 API Key、OAuth token、Keychain、SSH 私钥或 Science 用户数据；未运行真实 provider、真实 Science、真实 SSH、已安装 App、签名、公证或 release 验证。

`FAIL` 表示当前固定 HEAD 不能判为重构收尾完成或 `SOURCE-GREEN`，不表示重构方向失败。控制面、owner、typed receipt、durable journal 与 CAS 设计已经形成稳定骨架；剩余主要缺口在数据面资源预算、Science control runner 收敛、共享生命周期原语归属和诊断闭环。

## 2. 二审确认的 HIGH

### H1｜Gateway 连接与 CONNECT 没有资源 owner / budget

生产链：

`loopback accept -> 每连接一个线程 -> 无 deadline 的 header 读取 -> CONNECT 再创建两个 copy 线程 -> 无 idle/session/concurrency 上限`

源码锚点：

- `desktop/gateway/src/server.rs::handle_one/serve`
- `desktop/gateway/src/server/http_codec.rs::read_head`
- `desktop/gateway/src/connect.rs::handle_connect`

任意本机进程无需 path secret 即可保持慢 header；CONNECT 在 path-secret 认证前分派，成功后外层 handler 等待两个阻塞 copy 线程。单个空闲 tunnel 会长期占用约 3 个线程和多组 socket FD。64 KiB header cap 只限制大小，不限制慢发送时间；10 秒预算只覆盖 DNS 返回后的地址拨号，不覆盖 DNS、已建立会话、idle、总时长、字节或并发。

最小修复合同：accept 前全局连接额度；header/body 绝对或 idle deadline；CONNECT session/idle/byte budget、取消与确定性回收；明确 CONNECT 的认证、独立 listener 或 owner policy。

CONNECT 的通用 egress 当前已被架构文档显式记录，因此本轮不把“可连接非 denylist 目标”单独重复计算为第五个 HIGH；可证明的 HIGH 是任何本机进程可利用的无界资源面。认证、allowlist 与产品 owner 仍必须在修复时明确决定。

### H2｜非 Codex 请求与普通成功响应可无界分配内存

源码锚点：

- `desktop/gateway/src/server/http_codec.rs::content_length/read_body`
- `desktop/gateway/src/server/inference_dispatch.rs::handle_post`
- `desktop/gateway/src/messages.rs::read_body_with_deadline/get_once/post_nonstream`

所有非 Codex provider 都可把任意正 `Content-Length` 直接变成 `vec![0; len]`；只有 Codex 分支有 64 MiB 请求门。models GET 和 nonstream POST 的 2xx body 使用 `limit=None`，固定 scratch buffer 不构成累计上限，后续 JSON parse/transform/serialize 还会放大峰值内存。

请求侧需要合法 path secret，但这是正常 custom/relay provider 路径，仍可使 Gateway OOM。异常或用户配置的上游也可在成功响应路径高速发送大 body。

最小修复合同：所有 provider 共用的请求上限并在分配前返回 413；body 前先判 route；models/nonstream 2xx 同时做 Content-Length 预检和逐 chunk 累计上限，超限返回稳定 502。

### H3｜Science control/probe 命令继承 ambient environment

源码锚点：

- `desktop/src-tauri/src/runtime/science/executable.rs::safe_science_version_with_timeout`
- `desktop/src-tauri/src/runtime/science/lifecycle.rs::sandbox_url/runtime_status`
- `desktop/src-tauri/src/runtime/launch_env.rs::base_process_env`

`--version`、`url`、`status` 只覆盖隔离 `HOME`，没有 `env_clear`，也没有使用项目已有的 allowlist builder。从终端或开发环境启动 Desktop 时，Science CLI 及其后代可看到 provider key、代理认证变量、`SSH_AUTH_SOCK` 和其他未知变量。这与 `launch_env` 及 Science runtime 文档声明的凭证边界不一致。

最小修复合同：建立唯一的 Science control/probe command builder，以 `env_clear + base_process_env + 隔离 HOME` 为边界；为三个真实生产 helper 增加敏感 sentinel 回归。

### H4｜Science `status/url` 没有 deadline、输出上限或后代清理

`runtime/science/lifecycle.rs::sandbox_url/runtime_status` 使用同步 `.output()`。`spawn_blocking` 只把同步工作移到 blocking pool，不提供 timeout 或取消；one-click、recovery、手动打开的部分调用还持有共享 Lifecycle lease。

CLI 自身挂起会让操作无限等待；即使直接子进程退出，只要后代持有 stdout/stderr pipe，`.output()` 仍可能等不到 EOF。项目的 version runner 与 Skill control runner 已经有私有进程组、deadline、输出上限和 kill-group，证明这不是理论威胁。

最小修复合同：抽取唯一 bounded Science control runner；具备绝对 deadline、私有进程组、有限 stdout/stderr、直接子进程和整个组的清理与 `wait`。补“直接子进程挂起”和“父进程退出但后代持 pipe”两类测试，并验证 Lifecycle lease 在有限时间释放。

## 3. MEDIUM

### M1｜共享 transaction Science stop primitive 由 `one_click.rs` 反向拥有

`runtime/sandbox_session/one_click.rs` 中的 `TransactionScienceStopBoundary`、`TransactionScienceStopOwner` 和 `execute_transaction_science_stop_with` 实际服务 cold、DB restart、profile-switch rollback、history、live compensation、fresh replay 六类边界。`history_recovery.rs` 因而反向依赖 one-click 实现层，canonical owner map 也没有完整列出该共享 owner。

应迁到中性 `transaction_science_stop` 模块；共享 owner/CAS primitive 与各事务自己的 durable intent、顺序、补偿和 outcome 分离。

### M2｜transaction stop claim 在 AppState 锁内做外部探测

`one_click.rs::execute_transaction_science_stop_with` 在持有 AppState mutex 时调用 `claim_exact_request()`；生产 closure 可进入 receipt、process、`lsof`、时间和文件系统探测，阻塞所有 AppState reader。应锁内冻结 generation/full owner，锁外 claim/probe，最后锁内 full-owner CAS 发布。

### M3｜四个无生产 caller 的 IPC 仍注册在控制面

`desktop/src-tauri/src/lib.rs` 注册：

- `list_templates`
- `validate_profile_catalog_model`
- `preview_profile_preset_sync`
- `apply_profile_preset_sync`

`apply` 会修改 catalog intent；`validate` 会在 `Lifecycle::with_observed_context` 内执行真实 scratch network/process probe。当前 bundled caller 为空，门禁允许空 caller 数组通过。没有产品 caller 前应取消注册并保留内部 helper/test；如要保留，必须补产品 owner、DTO、journey、acceptance，并把 validate 改为锁内 snapshot、锁外 probe、锁内 recheck。

### M4｜terminal Gateway cleanup 可静默无限重试

`desktop/src-tauri/src/lib.rs::RejectedGatewayRegistryState::drop`、`drain_for_terminal_with` 和 native `Exit` 路径在 `stop_child_confirmed` 持续报错时每 10ms 无限重试，丢弃错误且没有稳定诊断。Fail-closed owner 可以保留，但需要有界退避、结构化状态、外部 supervisor 或明确 fatal terminal state。

### M5｜配置已提交后，前端刷新失败仍可能显示成功

`desktop/src/profile-controller.js::loadConfig` 默认吞掉 `get_config` 错误并返回 `false`；多个 create/update/delete caller 忽略该返回值，随后用成功文案覆盖读取错误。后端已经提交但 UI 仍保留旧状态，用户可能重复操作或误判当前配置。

应统一使用 `throwOnError`，或检查布尔值并明确显示“后端已提交，UI 刷新失败”。

### M6｜system SSH wrapper 只验证形状，不验证内容身份

`runtime/sandbox_session/ssh_preflight.rs::validate_system_ssh_wrapper_path` 只检查 symlink、普通文件、大小与 executable bit，没有固定 hash、owner/nlink/mode、nofollow FD 绑定或防 TOCTOU。该功能需要用户显式开启，因此维持 MEDIUM。

### M7｜canonical 架构/验收文档与当前生产符号发生漂移

`docs/operations/real-machine-acceptance.md` 仍把部分已进入 owner/wait/CAS 的 native-exit/downgrade/transaction stop 机制写成开放 gap，也没有列出六类真实事务 caller。`.agents/context/known-issues.md` 的旧 SHA 需要限定为 adapter/bridge 局部 seal，不能被误读为当前完整 caller seam。

### M8｜source gate 私有化依赖后仍持续绑定共享 Cargo 根 `ctime`

`test/quality/source_gate/runtime.py::_directory_record/recheck` 把真实 `~/.cargo/registry` 的 `ctime_ns` 作为整场 run 的持续权威。即使已经建立内容绑定的私有依赖副本，共享根发生无内容变化的元数据更新仍会终止 canonical gate。

应在私有副本 materialize 前后闭合校验共享源；之后只复核私有 inventory/digest。共享根 provenance 应记录内容 digest，而不是持续绑定目录 `ctime`。

### M9｜source-gate failure evidence 丢失具体 drift 对象

所有 recheck 项先折叠为 bool，再统一变成 `source input drift -> INPUT_DRIFT`。failure schema 没有 suite ID、checkpoint 或非敏感 detail code，本次只能依靠未封存的文件时间元数据反推。

应使用封闭 reason 枚举，例如 `OFFLINE_ROOT_IDENTITY_CHANGED`、`CARGO_DEPENDENCY_DIGEST_CHANGED`，并在 failure evidence 中记录 `suite_id/checkpoint/detail_code`；不得记录私密绝对路径、环境原文或 credential。

## 4. LOW / 已知债务

- `test/run-loopback.sh` 对测试注入变量使用 `eval`；canonical gate 会拒绝正常路径注入，但普通 wrapper 仍可执行任意 shell。应改为固定 fixture 或 argv，不让字符串 shell 进入默认 wrapper。
- Science runtime adoption 只有内容寻址 snapshot，没有通用 predecessor/candidate manifest、normalized CLI/route/capability diff 与 adoption decision record。该缺口已由架构文档公开，不是本轮新回归。
- `one_click.rs` 与若干测试文件仍很大，但行数本身不构成职责缺陷；只有共享 owner 反向归属和生产 caller 测试不足被计入本轮 finding。

## 5. 架构裁决

### 已经成立的强项

- Desktop/Tauri control plane 与 Gateway data plane 总体分离；普通 inference 不让 Desktop 进入逐请求链路。
- process-local AppState、durable config/journal、receipt/manifest、live process identity 没有被伪装成一个万能事务。
- `GatewayReceipt`、`ScienceHostAdapter`、generation + full-owner CAS、锁外 wait、replacement preservation 是可靠的生命周期设计。
- structured stage、journal outcome、frontend DTO 分层较好；finalize readback 不从错误字符串反推状态。
- source、artifact、isolated-live、authorized-live 与 release 证据层级保持分离。

### 仍需收口的边界

- 生命周期事务层已有 owner/budget 思维，Gateway 数据面资源治理没有跟上。
- Science control subprocess 存在多套 runner，安全与诊断合同未收敛。
- 共享 transaction primitive 仍藏在具体 one-click 编排模块。
- registered surface、canonical owner map、真实 caller 与 acceptance 没有完全闭合。
- 六类 production caller 的部分测试依赖 `include_str`/substring/源码顺序断言；它们能防形状漂移，不能替代真实 caller 在 blocked stop、replacement 与 generation drift 下的行为测试。

## 6. Science / CSSwitch 能力与证据边界

| 能力域 | 当前源码结论 | 当前 HEAD 证据层 |
|---|---|---|
| 一键开始、恢复、停止、Gateway/Science owner | 主链完整；存在 H3/H4、M1/M2 | runtime `NOT-RUN` |
| Science executable 选择、identity、snapshot | source 机制较完整 | artifact/live `NOT-RUN` |
| Messages、Chat、Responses、SSE、tools、models | source 与 fixture 覆盖广 | 真实 provider/model `NOT-RUN` |
| Skill 安装、列举、attach/readback | source/test 有覆盖 | load、trigger、restart 当前 `NOT-RUN`；旧 B-SKILL 仍 inconclusive |
| MCP/connector | CSSwitch 只拥有部分 bridge/transport | hosted/generic MCP 不是已证明 CSSwitch 能力 |
| system SSH reuse | opt-in、总体 fail-closed；有 M6 | 真实 parser/wrapper/server `NOT-RUN` |
| Python/R/Node environment 与 kernel | Science owner；CSSwitch opaque preserve | 不从静态包面推断可用 |
| 签名、安装、发布附件 | 本轮无授权 | `NOT-RUN` |

旧 `06b630b` artifact 上的 B-RUNTIME/B-CORE/B-CONTEXT 结果不能迁移为当前 `00a63088` 的 PASS。

## 7. 本轮测试与门禁证据

### 受管沙箱 canonical gate

- run ID：`69009cfc22f04230a93d80fe0046b410`
- completion seal：存在，verdict `FAIL`
- 结果：9/15 PASS、3 FAIL、2 INFRA、1 SKIPPED
- 失败/INFRA 主要由 loopback bind 与 nested sandbox 限制触发，不能判为产品失败。

### local-only 复跑

- run ID：`7b000526cdfbec404472f738ad309693`
- 已发布 6 个 PASS，其中 `SUITE-RUST-GATEWAY` 为 287/287 PASS。
- loopback 子执行已构建 derived Gateway，但 suite result 尚未发布时发生 `INPUT_DRIFT`。
- 触发对象域：共享 `/Users/superjj/.cargo/registry` 内容 mtime 未变，但 ctime 在 suite 区间变化；私有副本与原依赖树均为 16,740 entries、197,242,612 bytes，digest 相同；clone Git clean。
- 没有 completion seal；不能把该 run 与前一 run 拼接。

因此当前固定 HEAD 只能标记：`SOURCE-GREEN: NOT ESTABLISHED`。

其他窄化观察：Gateway 重要 finding 二审运行 10 个窄 Rust 测试并通过；Science 主审运行 21 个 Python 边界测试与 1 个 SSH Rust 测试并通过；前端与 quality focused 测试通过。这些都不替代 canonical 15/15 seal。

## 8. 建议的修复与验收顺序

1. Gateway：统一 connection/header/CONNECT/request/response resource budget。
2. Science：抽出唯一 bounded、env-cleared control runner。
3. Runtime transaction：把 shared Science stop owner 移入中性模块，消除锁内外部探测。
4. Control plane：收口 dormant IPC、terminal cleanup、SSH wrapper 与配置提交后刷新语义。
5. Governance：修正 canonical owner map 和 source-gate drift diagnostics。
6. 在没有并行 Cargo 操作的独占窗口重新取得当前 HEAD 15/15 completion seal。
7. source closure 后再进入 exact artifact、installed、isolated-live、authorized-live、signing 与 release 验收；各层不得互相替代。

## 9. 下一窗口起点

恢复时先实时复核 `git status --short --branch` 和 `git rev-parse HEAD`。若仍为 clean `next@00a63088f93774b37d184ca11bcc2c85936e24b3`，本页可作为修复输入，但不是修改授权；由用户明确选择先处理 Gateway HIGH、Science HIGH 或只做文档/验收收口。基线、需求或 HEAD 变化后，先重新验证对应调用链，不机械沿用本页行号和结论。

## 10. 2026-08-10 收口执行结果

本节取代第 8–9 节的后续执行建议，但不改写本页原始审查基线。实时基线为
`next@a325802404a5346cd2cb82a95be9d58dfac44277` 加未提交工作树 overlay；因为用户未授权
commit，overlay 不是 exact HEAD，也没有 production source seal。

### 10.1 finding disposition

- 旧 H1–H4：当前 production owner、budget、env-cleared bounded runner 与确定性测试锚点均在，
  未发现重开证据。
- M1–M2：共享 transaction Science stop owner 已移入中性模块；probe、exact effect 与 wait
  在 `AppState` 锁外执行，effect 前及 publication 时均执行 generation + full-owner CAS。
- M3：四个无 production frontend caller 的 dormant Tauri IPC 已取消注册；仅保留的历史行为
  characterization 被限定为 test-only。
- M4–M6：terminal cleanup 现有封闭诊断与封顶退避；前端明确区分 backend 已提交与刷新失败；
  SSH wrapper 以 nofollow FD 校验 exact bytes/owner/mode/nlink，并在私有目录生成只读 snapshot，
  Science 的长期 `PATH` 不再指向可替换的原 wrapper。
- M7–M9：canonical owner/caller、Gateway 资源总览与验收映射已同步；私有 Cargo view 建立后不再
  持续绑定共享 registry `ctime`；input drift evidence 使用封闭 `suite_id/checkpoint/detail_code`
  组合，JSON Schema 与 semantic validator 一致。
- LOW：loopback wrapper 的字符串 `eval` 已由固定 fixture 枚举替代。Science adoption ledger
  继续作为已公开的 `DESIGN-CHOICE / OPEN DEBT`，大文件行数继续排除；两者都不是本轮
  actionable finding。

### 10.2 验证与独立审查

- Desktop Rust：599 tests `PASS`；Gateway Rust：290 library tests + 1 CLI integration `PASS`；
  Science runner 定向 6 tests `PASS`。
- source-gate contracts/runtime focused：71/71 `PASS`；profile/Codex/inventory：18/18 `PASS`；
  frontend 固定入口 `ALL PASS`；loopback retry fixture 与 shell syntax `PASS`；
  quality metadata `PASS`；`git diff --check` `PASS`。
- `test/test_scripts.sh` 在允许本地 fixture/process 的环境以原命令 `ALL PASS`。受管沙箱内
  `test/run-offline.sh` 为 31 `PASS`、2 个 loopback/process fixture `SKIPPED`，不外推为完整 gate。
- clean-context 审查依次暴露并修复了反向模块依赖、SSH pathname TOCTOU、drift schema/semantic
  分裂、canonical 文档漂移和 test identity digest/排序。针对最新 overlay 的最终
  clean-context sol high 复核为 `PASS`，`BLOCK/HIGH/MEDIUM/LOW = 0/0/0/0`。

### 10.3 canonical source-gate 边界

在没有其他 Cargo 测试进程的窗口执行唯一入口：

```bash
bash test/run_all.sh --output-root /private/tmp/csg.Xvtbnw
```

公共 CLI 以 `runner_exit=12`、`reason=internal-failure` fail closed；输出根保持为空，未创建
run layout、run ID、suite observation、aggregate 或 completion seal。随后只运行同一 production
preflight seam 诊断，精确异常为 `worktree is not clean`。因此本轮 15 个 suite 均为
`NOT-RUN(preflight)`，不是 suite `FAIL`；`SOURCE-GREEN: NOT ESTABLISHED`。当前 exact HEAD
`a325802…` 不包含本节所述 overlay；在禁止自行 commit 的边界内，不可能为该 overlay 取得
“current exact HEAD 15/15” seal，也不得用临时 stash、伪 commit 或旧 SHA 的 PASS 替代。

### 10.4 证据层边界

本轮没有构建或验证 exact artifact，没有替换 installed App，没有启动真实 provider、真实
Science 或真实 SSH，没有读取 credential，也没有执行 signing、notarization、tag、push、release。
这些层全部保持 `NOT-RUN`；旧 artifact/live 证据只绑定各自原 SHA，不能升级本工作树候选。
