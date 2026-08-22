# 2026-08-09 Claude Science 0.1.25 `B-SKILL-01` isolated-live 调查

状态：日期化证据；`INCONCLUSIVE(reason=safety-stop)`

适用范围：`B-SKILL-01`、artifact-producing source `c4a1159487a7b7fa83cae7d18d64510aa5faec71`、`CSSwitch Test.app` 0.8.4、packaged Rust Gateway 与 `/Applications/Claude Science.app` 0.1.25 的隔离 tuple

最后复核：2026-08-09（Asia/Taipei）

失效条件：CSSwitch 外部 Skill bridge、acceptance Git fixture seam、Science host-access / writable-root 行为、目标 artifact 或 Science 版本任一改变时，本调查不得外推。

## 结论

Science 0.1.25 在用户对话前尝试访问非预期外部 destination，命中
[Science 探针合同](../../operations/science-probe-spec.md#b-skill-01外部-skill-六阶段闭环)的
立即停止条件。本轮因此固定为 `INCONCLUSIVE(reason=safety-stop)`，不能写成 `PASS`。

停止前只完成 source / artifact identity、隔离根、loopback fixture 与 deny-egress 护栏核对；
六阶段尚未开始，均为 `NOT-RUN`。停止后曾在同一 deny-egress 边界内继续做 host-access
定位，这是程序偏差，所得 Agent / managed connector / packaged Gateway MCP、
`HOST_ACCESS_REQUIRED`、rw grant 与 `edit_file` 矛盾观察全部排除在正式判定之外，不能作为
任何 B-SKILL sub-gate，也不改变主理由 `safety-stop`。

## Source 与 artifact 绑定

- primary 基线：`next@d13e319905fa9ef4b7664fbb8a54371b98c9998c`；artifact-producing
  synthetic commit：`c4a1159487a7b7fa83cae7d18d64510aa5faec71`，clean。
- exact-SHA source gate：13/13 suites `PASS`；run ID
  `1464f35dbb43ec3060a3f7539b2a20ab`；completion seal SHA-256
  `90eeb882077d3ed6b2cb3402f3ee8f561f7542b0ded8c26f10663d584645f1a3`。
- `CSSwitch Test.app` canonical digest：
  `242fdaf9b6cec7ddc20057095ccbf4115de44e9534b4e3d84b03e8a0f255b077`；Desktop
  executable SHA-256：
  `79a759c3b76b178652b4fe77d6f6e44365c94ce21fb038006df9018faa85442d`；packaged
  Gateway SHA-256：
  `80f446ba36c22899af856492e6602c535529e13a2c5edfca9734d17cf6f241c2`。
- Claude Science 0.1.25 executable SHA-256：
  `63b0f57a71ab47490a14ad02c054d23ffdb717198f39df802bf19829c67ba64d`；package
  canonical digest：
  `371a10ec31e9dd5c84a375669f595f453b8feb69b42fd002950b10f67beb40b7`。
- acceptance-only source seam 只在 `acceptance-build` 中把 GitHub API/raw endpoint
  替换为无认证、根路径、显式非保留测试端口的 loopback HTTP URL；production build 不读取该 env。Desktop
  只把该值投影进 managed connector，未把它扩散到普通 Science launch env。

## 六阶段 ledger

| 阶段 | 观察 | 判定 |
|---|---|---|
| GitHub install | 对话前已 safety-stop；正式阶段未开始 | `NOT-RUN` |
| attach | 对话前已 safety-stop | `NOT-RUN` |
| Agent load | 对话前已 safety-stop | `NOT-RUN` |
| tool / poll | 对话前已 safety-stop | `NOT-RUN` |
| uninstall | 对话前已 safety-stop | `NOT-RUN` |
| restart persistence | 对话前已 safety-stop | `NOT-RUN` |

停止后的诊断夹具曾记录 `connector_advertised=true`、`git_requests=[]` 与
`install:edit-file-writable-root-mismatch`，但这些字段均不进入 ledger。原始字段
`public_destination_seen=false` 只表示 Git fixture 没有观察到 public GitHub destination，
不否定 Science warmup 已尝试非预期外部 destination。

## 网络与停止边界

首次启动中，Science 0.1.25 在用户对话前会主动 warm up bundled MCP / Conda 资源，日志出现
外部 destination 尝试。外层 seatbelt 以 `EPERM` 阻断 non-loopback socket，network receipt
也确认外部 IPv4 与 DNS transport 被拒绝、8765 未占用。该外部 destination 尝试触发
`INCONCLUSIVE(reason=safety-stop)`；停止后继续的有界诊断属于程序偏差，已整体排除，不能
保留为正式 sub-gate 或替代重跑。

没有读取真实 API key、OAuth token、Keychain、SSH、真实 Science Skill root 或真实
`~/.csswitch/skills`，没有替换已安装 App，也没有直接执行 `skill-install-mcp`、installer
core/helper 或写 Science 数据库。准确 source URL 保持 GitHub 形态，但 acceptance build 的
API/archive transport 只指向 `127.0.0.1`；因为 request 未提交，fixture Git server 未收到下载。

## Evidence envelope 与清理

本轮 raw evidence 保存在一次性隔离目录并以 hash 固定；以下 hash 仅用于审计 safety-stop
与程序偏差，不为停止后的诊断建立 B-SKILL 证据资格：

- pre-run manifest：`601a69edd999b70947a0a87ffa86e90c06668ac03e5463fe3277a63b2356ba95`
- network isolation receipt：`386b1f61fe85a0c1d154b6f4cfe3f8039a7adcd16245db48affc2e3f1a4cc65e`
- final fixture observation：`30051dbcac0b6dc133016a46a70c2847e1ee84dbfb8a77e5767c0d5ecdc610e2`
- fixture transcript：`ff62c630b9cbd4acbe0c9b63cd3900751b399b62be91d50c44d65bc4a20f5c17`
- managed local MCP config：`0307cf95488750cb1745858a74f3a26c79ea425ce7554f1ce266ec08d6fc70dd`
- route state：`cc4af449ea82eddb8fd01dca7fd43340aa48f9334ed315cbac32030d53eaa656`

最终通过 CSSwitch 产品“全部停止”收口，再退出 `CSSwitch Test.app`、Playwright 与 loopback
fixture。Gateway 63695、Science 63696、sandbox 63697 和 fixture 63722 四个专用端口均无
listener；按运行标记检索没有残留 attributable process。隔离 runtime、临时构建 clone、source
gate 目录和 fixture driver 只在审查与提交完成后删除；该删除不改变本调查的
`INCONCLUSIVE` 判定。

要重新进入 `B-SKILL-01`，必须先使用不会在对话前发出非预期外部请求的 Science runtime
或经评审的隔离启动方式，从全新 tuple 重跑。若新运行随后复现 host-access 矛盾，再按该
独立停止点记录新的 `INCONCLUSIVE`；不得沿用本轮停止后观察，也不得用 DB 注入、外部代写
request、直接执行 MCP 或 Python/shell 文件 API 绕过边界。
