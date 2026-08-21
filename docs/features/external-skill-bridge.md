# 外部 Skill 安装桥

公开 GitHub URL 路线自 v0.5.0 起提供，v0.6.0 扩展本地包、bundle 与恢复语义；当前 v0.8.4 继续使用本合同。

多平台 Plugin / Agent Skills / generic MCP 的未来兼容边界已经在
[Science Skill / MCP / Plugin 扩展控制面](../architecture/skill-mcp-plugin-control-plane.md)
冻结；除本文随后定义的 fixed-commit 单 Skill 可选确认路径外，其余目标设计尚未
实现，不改变本文的当前入口、状态码、支持范围或证据层。

CSSwitch 只提供两个窄入口，不启用 Skill Manager：

1. Science Agent 调用 `install_external_skill`，参数是公开 GitHub 仓库、Plugin/Skill 集合或准确 Skill 目录 URL。
2. CSSwitch 主面板通过系统文件选择器导入本地 `.zip` 或 `.skill`。

两条路线都由 CSSwitch 宿主执行下载或读取、包校验、原子提交和 `OPERON` 绑定。它们不调用 Anthropic Skill catalog，不读取 Science/GitHub credential，不写 SQLite、inventory、store 或 catalog。

### 可选的精确计划确认：固定 commit 的单 Skill

`install_external_skill` 在省略 `confirmation` 时保持本章既有 GitHub
安装输入、响应和副作用不变。可选的 `confirmation` 只接受下列严格形态：

```json
{"schema_version":1,"action":"plan"}
```

它只适用于已带 40 位 commit SHA 的公开 GitHub URL，并且只在完整 archive
可投影为一个无额外 payload 的 Skill 时返回 `CONFIRMATION_REQUIRED`。宿主从
固定 commit 下载、在 Desktop 继承并由 Gateway shared authority fence 持有的 host-only
mode `0700` 私有根中 staging、持久化
operation ledger，并返回精确 `source`、完整 `effects`、`operation_id`、
`plan_digest`、短期单次 `capability` 和 expiry；此步不提交 package、不修改
OPERON。capability 只出现在 mode `0600` 的最终 bridge response，不写入
status/progress、日志或 ledger。

下载 mutable remote bytes 前，host-only lifecycle 会先持久保留一个最大 archive slot；
install、removal 与 reservation 共用四项 lifecycle entry hard cap；removal 不增加 archive
bytes，但同样在任何 native effect 前被拒绝。容量满时返回 typed admission failure，不下载。
只有过期、未消费、所有 effects 仍为
`NotStarted` 且没有 intent 的 exact install plan 可经 tombstone 回收 staging 与 ledger；
consumed、in-progress、uncertain、removal 或任一 identity drift 均保留并 fail closed。
过期的 pre-receipt `Reserved` record 则只可回收其确定性 `exact-<operation_id>` child：
先以 held root/child FD 的 no-follow 打开，验证当前用户、`0700` root/child、`0600`
regular-file 白名单（`archive.zip` / receipt）与各自大小，再删除。child 缺失、mkdir 后
尚未写入或仅有已知 partial archive 都可回收；额外条目、类型/权限/owner/size 不匹配或
任一读取错误均保留整个 child 与 reservation，绝不猜测删除。
removal admission 把同一 authority-root 的真实 operation staging descriptor 传入 collector；
它绝不把 quarantine root 当作 install staging，因此由 removal 触发的 terminal install GC
仍会在删除 ledger 前回收对应 exact staging child。

用户看过并明确确认完全相同 plan 后，Agent 才能以同一 `source_url` 调用：

```json
{
  "schema_version":1,
  "action":"apply",
  "operation_id":"<plan returned verbatim>",
  "plan_digest":"<plan returned verbatim>",
  "capability":"<plan returned verbatim>"
}
```

apply 仅重开该 `operation_id` 的 durable snapshot，重新验证 capability hash、
expiry、原 source request digest、runtime/data-dir/active-org binding 和 archive
identity；它不会重新读取 mutable source URL。任一不匹配、过期、已消费或
in-progress effect 都会 fail closed 并要求新的 plan 或明确 reconcile。host
access 批准绝不等同于对 plan effects 的用户确认。

固定 GitHub plan 另外同时封存两个不可互换的内容摘要：inspection canonical
digest 用于报告/plan source，materialized package canonical digest 用于 package
commit、marker 与 fd-relative readback。两者必须各自匹配同一 retained archive；
CSSwitch 不会把不同 domain 的摘要相互比较或以其中一个替代另一个。

若进程在已持久化 effect intent 后退出，调用方只能发送
`{"schema_version":1,"action":"reconcile","operation_id":"<original>"}`。该形态
不得带 `source_url`、`plan_digest` 或 capability；Gateway 只按 ledger kind 做原生
GET / fd-relative marker-content readback，绝不重放 attach、detach 或 rename。bridge
mailbox 只保存 request/status/response；它不是 staging、ledger 或 quarantine authority。

若 durable ledger 恰好是已验证首 effect、后续 effect 尚未开始的精确前缀，调用方可使用
`{"schema_version":1,"action":"continue","operation_id":"<original>"}`。它不得带
source、digest 或 capability；host 会重新绑定 target 并读取已安装 ownership 后只执行一个
剩余 effect。过期、replacement 或任意 ledger drift 只返回
`SKILL_OPERATION_NEW_PLAN_REQUIRED`，不重放已验证 effect。

确认式**安装**当前仅覆盖一个 GitHub single Skill 的 package commit 和原生
OPERON attach/readback；不支持 local/package/bundle/Plugin/MCP 安装计划。确认式**卸载**则接纳
任一非 bundle、带精确 CSSwitch ownership marker 的已安装单 Skill（包括 `local_zip`）：plan 固定
`operon_detach` 后 `package_quarantine` 两项
effect、runtime/data-dir/org/root identity、canonical marker 和 content digest。确认 apply
在每项 effect 前写 durable intent；detach transport 不确定时只做 native GET readback，绝不
quarantine；quarantine recovery 只核验已持久化相对 destination，绝不再次 rename。bundle、
Plugin、MCP 与 local package **安装**不进入该 mode，保持原有 fail-closed 边界；
已安装且由 CSSwitch ownership marker 证明的 `local_zip` 单 Skill 则如上可进入
确认式卸载。

确认式卸载的 apply 由返回的 `operation_id`、`plan_digest` 与 capability 唯一确定；它不携带
`skill_name`、bundle id 或任何其他外层目标字段。plan 才必须携带精确已安装 Skill 名称。
卸载最终响应使用 `detach_verified`、`quarantine_commit` 与
`recovery_required`；不会借用安装的 `attach_verified` 或 `directory_commit` 语义。
若 effect 已执行而 successor ledger 未能确认持久化，Gateway 不会把结果硬编码为
`false`；`recovery_required=true`，对应 effect 字段只反映即时可信读回的
`true` / `false` / unknown 快照，仍只能以同一 `operation_id` reconcile 读取
authority-root 的 durable projection。

该情形还返回 response-only `post_effect_observation`，其中的 effect 与
`observed_true` / `observed_false` / `unknown` 来自立即可信读回；它不是 durable
receipt，也不授权重放副作用。仅后续 reconcile 成功写入 ledger 后才可报告 durable
verified 结果。

确认计划还封存当前 org Skills root 的 device/inode identity。apply、continue 与
reconcile 每次都重新 no-follow 打开该 root 并拒绝 identity drift；移除流程从 preflight
开始持有同一已验证 source leaf FD，直至 WAL intent 后的最终 no-clobber rename 前再次把
named entry、marker 和 content 与该 FD 比较。因此 bridge mailbox 中的同名 staging、ledger
或 quarantine 条目没有 operation authority。

这是 CSSwitch host capability 与合作方 shared per-Skill lock 的边界，不是对任意同 UID
进程的 source-name CAS 承诺：macOS 的 `renameatx_np(RENAME_EXCL)` 仅防止 destination
覆盖，并没有 expected source inode/generation 参数。若外部进程绕过 lock 直接改写 Science
data-dir，host 会对 revalidation 时已观察到的 drift fail closed；若竞争发生在最后一次检查后，
本协议不声称原子阻止或回滚。任何可观察到的不一致保留为 failed/uncertain recovery，绝不
以重复 rename 伪装恢复。

## 正式数据流

### GitHub URL

```text
用户给 Agent 准确 URL
  -> Agent 调 install_external_skill(source_url)
  -> 签名 bridge request
  -> CSSwitch Gateway 验证 Science host context
  -> 匿名解析 commit（named ref 最多一次 API）
  -> 对固定 commit 只发起一次 archive 请求，不自动重试
  -> 只接受受信 https://codeload.github.com 302
  -> archive 长连接无法安全完成时，回退到同一固定 commit 的 tree/raw 文件级下载
  -> fallback 仍属于同一 bridge request，校验 tree 完整性并报告文件数/字节进度，不生成新请求
  -> 单 Skill 旧入口 90 秒、archive bundle 30 分钟总时限，慢速分块不能无限刷新 deadline
  -> bridge 写入 <id>.status.json：source_resolution / recovery / download / validation / commit / attach
  -> status 含 2 秒心跳、elapsed_seconds、31 分钟宿主响应 deadline；Agent 只轮询同一 ID，禁止重复提交
  -> 识别单 Skill 或唯一 Nature-like Skill 集合
  -> 校验并原子提交单目录或完整集合根
  -> 获取新的 Science nonce URL
  -> nonce / cookie / CSRF / OPERON 单个或批量绑定 / GET 回读
  -> 单 Skill 由 Agent 调 skill(skill_name)；bundle 以批量回读为验收
```

Agent 不下载文件、不用 shell 或 Python 文件 API、不提供 staged package、不调用 catalog/Skill Manager，也不手工调用 `host.agents.attach_skill`。当返回 `FILES_COMMITTED_ATTACH_REQUIRED` 或 `ATTACH_STATE_UNCERTAIN` 时，Agent 使用相同 URL 重试安装工具；CSSwitch 重新验证已提交内容并重试 attach。

`HOST_ACCESS_REQUIRED` 返回 request/status/response 三个文件名和 `poll_external_skill_request`。Agent 只写一次 request，之后只用同一 request ID 调只读 poll 工具；第一次省略 `last_sequence`，后续传回上一轮 sequence。gateway 内部长轮询最多 10 秒，阶段改变或最终 response 出现即返回，Agent 不再反复读文件，也禁止用独立 `sleep` 或长时间 shell/Python 循环等待。下载核心的总 deadline 是 30 分钟，bridge 另留 60 秒给校验、提交、绑定和最终响应。超过宿主 deadline 与 5 秒 grace 仍无 response 时 poll 工具返回 `HOST_RESPONSE_TIMEOUT`，仍不得重提。宿主以不可覆盖方式原子写最终 response，随后删除 `.processing` 和 status。成功、结构化失败及 `GITHUB_TIMEOUT` 走同一收口；gateway 重启会把遗留 `.processing` 转为 `REQUEST_INTERRUPTED` 最终响应并要求通过新请求做幂等状态恢复。

### 本地包

主面板的“导入 Skill 包”调用无路径参数的 `install_local_skill_package()`。Rust 后端打开系统 picker，只显示 `.zip/.skill`；前端从不接收或提交本地路径。

选择前和选择后都要求：

- Science 是 `RunningHealthy`；
- binary canonical path、版本和文件指纹仍匹配；
- 隔离 HOME、data-dir 和 sandbox port 未变化；
- 新的一次性 nonce、认证 cookie、CSRF 和 OPERON 回读可用。

后端使用 `O_NOFOLLOW` 打开普通文件，并从同一个 file descriptor 计算 archive SHA-256 和解压。UI 自动识别单 Skill 或 bundle；bundle 成功显示安装数量和名称摘要，不要求逐个 `skill()` 验证。

## GitHub v1 范围

接受：

```text
https://github.com/<owner>/<repo>/tree/<ref>/<path...>
https://github.com/<owner>/<repo>/tree/<ref>
https://github.com/<owner>/<repo>
```

`ref` 是单个 URL segment 的 branch/tag 或 40 位 commit SHA。40 位 SHA 不请求 commit API，仓库根使用一次 `commits/HEAD`。私有仓库、token、带 `/` 的 branch/tag、内部 symlink 物化、外部 marketplace source 和完整 Plugin runtime 不在 v1 范围。

bundle 集合根必须有直接包含 `SKILL.md` 的成员目录。集合根内所有普通文件按原相对路径映射到 Science `skills/` 根，因此 `_shared`、兄弟 Skill 和支持资源会被保留；只绑定直接成员。多个候选不猜测。`${CLAUDE_PLUGIN_ROOT}`、hooks、MCP 或 agents runtime 返回 `UNSUPPORTED_PLUGIN_RUNTIME_DEPENDENCY`。

公开仓库使用匿名请求；Gateway 不读取 `GITHUB_TOKEN`、`GH_TOKEN` 或 `.netrc`。403/429 根据 rate-limit header 区分 `GITHUB_RATE_LIMITED`，权限、404、tree 截断、网络失败和总时限超时分别返回结构化错误。

## Package 安全边界

共享 crate 是 `desktop/skill-package`（包名 `csswitch-skill-install-core`），被 Gateway 和 Tauri 通过 path dependency 复用。固定限制：

| 项目 | 上限 |
| --- | ---: |
| archive 原始大小 | 128 MiB |
| archive entries | 10,000 |
| Skill 文件数 | 512 |
| 单文件 | 4 MiB |
| Skill 总解压大小 | 32 MiB |
| bundle 文件数 | 2,000 |
| bundle 总解压大小 | 64 MiB |
| 路径 | 1,024 bytes |
| 路径深度 | 32 |

路径必须是 UTF-8 和 Unicode NFC。绝对路径、`..`、反斜杠、NUL、重复路径、父文件冲突、大小写/NFC 冲突、符号链接和特殊文件都会被拒绝。`__MACOSX/**` 与 `.DS_Store` 被忽略；来源自带 `.import-origin` 或 `.catalog_stamp` 会被拒绝。

GitHub `100755` 或 ZIP 明确 Unix executable bit 的文件落为 `0700`，其它文件为 `0600`，目录为 `0700`。不根据 shebang 推断权限。

`content_sha256` 对按路径排序后的长度前缀路径、executable bit、长度前缀内容计算；不包含 marker 和 Finder metadata。`_shared` 扫描只检查 `SKILL.md` 中的 Markdown/前置 YAML 路径以及脚本中的引号路径字面量，结果固定标记 `BEST_EFFORT`。

## Marker 与恢复

所有新安装写 Science 兼容的 version 1 `.import-origin`：

- 保留 `repo`、40 位小写 `sha`、`plugin`、`marketplace: csswitch-local-bridge`、`path`、`importedAt`、`license`；
- 扩展字段是 `csswitch_revision: 2`、`source_kind` 和完整 `content_sha256`；
- 本地包另写完整 `archive_sha256`，外层 `sha` 是其前 40 位，`repo` 是 `csswitch/local-archive`。
- bundle 每个成员另写 `bundle_id`、`bundle_name`、`bundle_content_sha256` 和 `bundle_member_path`；支持目录不伪装成 Skill。

bundle manifest 和持久 transaction journal 位于 `sandbox/skill-bundles/<org>/`。提交前锁定全部新旧顶层路径；同来源更新必须先通过完整 path hash 校验，失败会回滚或在下次同 bundle 操作恢复。

从任意 bundle 成员发起卸载时，第一次请求只返回
`BUNDLE_UNINSTALL_CONFIRMATION_REQUIRED`，其中包含 `bundle_id`、`bundle_name`、
完整 `affected_skill_names`、`confirmation_scope: whole_bundle` 以及
`partial_uninstall_supported: false`。该响应不 detach、不移动文件，也不写
quarantine。用户取消时 Agent 不再调用工具；用户明确确认后，第二次请求必须
同时提交原 `skill_name` 和响应中的精确 `confirm_bundle_id`。Gateway 会重新查找并
校验 manifest 和所有成员；ID 或归属变化时重新返回确认，不沿用旧确认。只有匹配
确认才批量 detach 并整包 quarantine，不提供成员级物理删除。

同名目录无有效 marker 或来源不同返回 `SKILL_NAME_CONFLICT`。marker 匹配但内容摘要变化返回 `INSTALLED_CONTENT_CHANGED`，不覆盖。新 marker 内容一致时直接重试 attach。

旧 GitHub marker 缺少 `content_sha256` 时，只有重新下载固定 commit/path 且内容完全一致才原子升级 marker。无法下载或不一致时不自动 attach。

## Science attach control

Tauri 传给 Gateway 的 `ScienceHostContext` 包含 canonical binary path、版本、`dev/inode/size/mtime/mode` 指纹、隔离 HOME、data-dir 和 sandbox port。它参与 Gateway 复用指纹；runtime 或 context 变化会重启 Gateway。手工只启动代理且没有确认 Science 时，代理照常运行，但安装返回 `SCIENCE_NOT_READY` 且不写文件。

每次 control preflight 和 attach 都重新执行绝对路径的：

```text
claude-science url --data-dir <isolated-data-dir>
```

子进程使用 `env_clear()`，只恢复受控 `HOME`、`TMPDIR` 和 locale；provider/GitHub credential 不会继承。只接受退出码 0、64 KiB 内唯一 loopback URL、预期端口和唯一合法 nonce。上限 10 秒，超时杀整个进程组并 `wait`。

单 Skill attach 使用原 POST；bundle 使用一次 `PUT /api/agents/OPERON/skills` 提交 `attach`/`detach` 数组。两者都必须 GET 回读。请求结果不确定或 active org 在请求后变化返回 `ATTACH_STATE_UNCERTAIN`，已提交目录保留。

## 结果状态

所有结构化安装响应包含 `schema_version: 2`。主要状态：

- `INSTALLED_ATTACHED_VERIFY_REQUIRED`：文件和 OPERON 绑定已确认，仍需 Agent 调 `skill()`；
- `BUNDLE_INSTALLED_ATTACHED`：整个集合和批量 OPERON 绑定已回读确认；
- `BUNDLE_UNINSTALL_CONFIRMATION_REQUIRED`：只返回整包确认和完整受影响列表，文件与绑定尚未改动；
- `BUNDLE_UNINSTALLED_DETACHED`：从任意成员触发的整包卸载、批量 detach 和 quarantine 已确认；
- `FILES_COMMITTED_ATTACH_REQUIRED`：文件保留，attach 未完成，可用同一输入重试；
- `ATTACH_STATE_UNCERTAIN`：attach 请求结果或 org 状态无法确定，可重试；
- `SCIENCE_NOT_READY`：没有可验证 control plane，不写入新安装；
- `CANCELLED`：用户取消 picker；
- `SKILL_NAME_CONFLICT`、`INSTALLED_CONTENT_CHANGED`、`UNSUPPORTED_SHARED_DEPENDENCY`：失败且不覆盖。
- `MULTIPLE_BUNDLE_CANDIDATES`、`BUNDLE_STRUCTURE_UNSUPPORTED`、`BUNDLE_LIMIT_EXCEEDED`、`BUNDLE_PATH_CONFLICT`、`UNSUPPORTED_PLUGIN_RUNTIME_DEPENDENCY`：bundle 失败且不部分提交。

兼容字段在一个版本内保留。ZIP 返回 `source_digest_sha256`，GitHub 返回 `resolved_commit_sha`。

## 废止路线

旧的 Science-side credential/staged-fetch 方案已经废止：Science 不使用自己的 GitHub credential 下载后再向 CSSwitch 交包。原因是该路线依赖 host grant、让沙箱和凭据边界变复杂，并且会重新引入 provider/登录态差异。正式路线始终是 Agent 只发起 MCP 调用，CSSwitch 宿主下载。

## 明确不做

不做 OAuth/catalog 模拟、Science binary patch、private bearer、SQLite 写入、Skill Manager/store/inventory/deployer、私有仓库 token、完整 Claude Plugin runtime、通用 control client 或离线仅提交模式。单 Skill 卸载保持既有流程；bundle 从任意成员触发整包校验、批量 detach 和 quarantine。
