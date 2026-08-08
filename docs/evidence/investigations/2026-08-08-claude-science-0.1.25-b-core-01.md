# Claude Science 0.1.25 `B-CORE-01` isolated-live 验证

状态：`INCONCLUSIVE`。第一次运行中，合成 project、project workspace 文件读写和 artifact
两版 lineage / diff / preview / execution provenance 的行为观察成功，但 pre-run manifest 与
单调 event timeline 不满足全局证据合同，不能升级为 sub-gate `PASS`；permission grant /
revoke / 越界 enforcement 与 annotation 定位 / 下一消息传递也未闭合。第二次窄重跑补齐了
此前缺失的 manifest、单调 events 和逐断言 observations，但在 outer sandbox 中不设置
acceptance opt-out 的条件下，production launch 在建立 Science PID/listener 前失败，同样不能进入
任何 B-CORE capability sub-gate。第三次运行启用了 Science 原生 filesystem sandbox，并用合规
envelope 再次完成 project、workspace 文件和两版 artifact 行为；但它使用的是同 source 新构建的
另一套 Desktop/Gateway tuple，没有对应的 exact-artifact / B-RUNTIME receipt，不能继承已验收
`CSSwitch Test.app` 的 PASS。permission fixture 路径也没有落到原生 sandbox 的 host HOME，无法
建立 grant。正式 event 的 operator observation 与后置物化的 reduced socket receipt 还记录 packaged
Gateway 已建立 non-loopback TCP/443 连接，触发 manifest 的 safety-stop，因此不能继续 revoke、
越界拒绝或 annotation，也不能把第三次运行中的成功行为观察升级为合同 `PASS`。

适用范围：production source `6e09e68654e1c43b936821c8be12330af3e19cc1`、由该 SHA 全新构建并
ad-hoc 签名的 acceptance App、`/Applications/Claude Science.app` 0.1.25、全新隔离
HOME/data-dir、固定 fake credential 和动态 loopback 端口。r1/r2 使用 outer deny-egress；r3 为了
避开 r2 未能建立且根因未闭合的 no-opt-out outer 启动条件，没有使用 outer sandbox，并设置发现
任何 non-loopback socket 立即安全停机。本文档只记录这些验证，不把文档提交改写为被构建或运行的
源码。

最后复核：2026-08-08（Asia/Taipei）

## 总判定

本轮严格按 [`B-CORE-01` 合同](../../operations/science-probe-spec.md)拆分 sub-gate：

| sub-gate | 结果 | 实际证据 |
|---|---|---|
| 证据 envelope | `INCONCLUSIVE` | pre-run manifest 缺强制身份字段；events 没有单调时间戳，动作顺序与 deadline 不可由账本审计 |
| 合成 project | `INCONCLUSIVE` | UI 新建行为成功，project id `proj_b33875fa7af5`；因 evidence envelope 缺口不升 PASS |
| project workspace write / read | `INCONCLUSIVE` | `edit_file` 写入项目相对路径并由 `read_file` 回读；因 evidence envelope 缺口不升 PASS |
| permission request / grant | `INCONCLUSIVE` | 初始 grant 列表为空；`request_host_access` 明确返回 filesystem sandbox inactive，路径已可访问且没有记录 grant |
| granted fixture read / write | `NOT-RUN` | 没有可 enforce 的 grant；没有把“已可访问”伪装成 grant 后读写 |
| revoke / revoke 后拒绝 | `NOT-RUN` | 没有 grant record 或 grant id 可撤销 |
| 越界拒绝 | `NOT-RUN` | enforcement 前提不存在；未请求 denied sibling |
| artifact lineage | `INCONCLUSIVE` | 同一 artifact id 下观察到 v1/v2、不同 version id/checksum 和版本切换；因 evidence envelope 缺口不升 PASS |
| artifact diff / preview / provenance | `INCONCLUSIVE` | v1/v2 preview、1 deletion / 2 insertions diff、两次 `edit_file` execution log 可回读；因 evidence envelope 缺口不升 PASS |
| annotation | `INCONCLUSIVE` | Markdown preview 依官方流程尝试文本选区，未出现 Annotate pill 或 pending comments chip；未发送下一条消息 |

因此总判定只能是
`INCONCLUSIVE(reason=r3-exact-artifact-precondition-unproven-permission-and-annotation-not-closed-and-network-safety-stop)`。
`B-CONTEXT-01` 要求 `B-CORE-01=PASS`，其前置仍未满足，本轮没有进入 B-CONTEXT。

第二次重跑没有改变上表第一次运行的 sub-gate 判定，只补充说明为什么不能在同一安全包络中
直接重跑补绿：移除 acceptance-only `--dangerously-no-sandbox` opt-out 后，Science 没有建立
managed identity/listener，全部 capability sub-gate 都是 `NOT-RUN`；现有证据不识别 exit 70 的
唯一根因。

第三次运行也没有改变总判定。它修复了 r1 的 evidence envelope，并观察到 Science 0.1.25 原生
sandbox 在没有 outer `sandbox-exec` 的 r3 条件下启动；这不能反推 r2 的唯一失败根因，也没有建立
可行且可归因的安全嵌套条件。r3 同时缺少自身 exact artifact 对应的 G1 / B-RUNTIME receipt。manifest
把“发现任何 non-loopback socket”定义为 safety-stop；正式 event 记录的 operator observation 与
后置 reduced receipt 显示 Gateway PID `935` 有两条 `198.18.0.1 → 198.18.0.54:443` established
TCP 连接，因此立即停止全部 owned runtime，没有再启动计划中的 r4。该 reduced receipt 没有保存
完整原始 `lsof` capture、采集 UTC 或 monotonic timestamp，不能识别真实域名或请求正文；但足以
支持保守 safety-stop 与总 `INCONCLUSIVE`。

## r3｜原生 sandbox、合规 envelope 与 network safety-stop

r3 从 production source `6e09e68` 重新构建 acceptance App；Desktop SHA-256 为
`3f21079b402eb8fa7c0eb3e807e27af096b139efdeadfa3ed7b63735c762a4a4`，packaged Gateway
SHA-256 为 `a8dbade49bc2a4b31554dd7b8e56d36a8b14e454a6fb27b26cb2e7deb15ae1d1`，Science
executable identity 与 r1/r2 相同。完整 pre-run manifest SHA-256 为
`3927bf5196113aac7b231405aefa2cf1df40ff9d6f167db312512a8f60863709`；正式 event ledger 从
manifest freeze 开始，每条记录均带严格递增的 `monotonic_ns`。已通过 B-RUNTIME 的 exact tuple
是另一 `CSSwitch Test.app`，Desktop/Gateway SHA-256 分别为 `242243bd…` / `fded59d7…`；r3
没有自身的 G1 artifact tree/resource/package receipt 或 B-RUNTIME receipt。因此 manifest 中
`exact_artifact_precondition=PASS` / `b_runtime_01=PASS` 是未被证据支持的 runner 自报，复核后
明确降为 `INCONCLUSIVE(reason=same-source-different-artifact-cannot-inherit-exact-tuple-pass)`。

| r3 sub-gate | 结果 | 实际证据 |
|---|---|---|
| exact artifact / B-RUNTIME 前置 | `INCONCLUSIVE` | r3 是不同 Desktop/Gateway hash 的新 tuple；没有匹配的 G1 / B-RUNTIME receipt，manifest 自报 PASS 无效 |
| evidence envelope | `PASS` | manifest 先冻结，正式 events 严格单调；逐断言 `observations.json` 含 expected / actual / pointer / decision |
| synthetic project | `INCONCLUSIVE` | UI 创建 `proj_9c3a29c2f895` 成功；后续 safety-stop 使整轮隔离 envelope 失效 |
| project workspace write / read | `INCONCLUSIVE` | `edit_file` / `read_file` / `save_artifacts` 成功且持久文件 hash 对齐；同上不升级 PASS |
| permission request / grant | `INCONCLUSIVE` | 初始 grant 为 0；fixture 误放在 outer HOME，而工具解析 `~` 到 Science 原生 host HOME，request 返回 `granted=false` |
| granted read/write、revoke、post-revoke denial | `NOT-RUN` | 没有建立 grant；安全停机后不再重跑 |
| out-of-bounds denial | `NOT-RUN` | grant 前提未成立；安全停机后不再请求 sibling |
| artifact lineage / diff / provenance | `INCONCLUSIVE` | 同一 artifact 下 v1/v2 parent、hash、preview、diff 与 execution steps 均对齐；整轮 safety-stop 降级 |
| annotation location / next message | `NOT-RUN` | safety-stop 前没有完成 annotation 传递，停机后不继续交互 |
| network boundary | `INCONCLUSIVE` | operator observation 与后置 reduced receipt 记录两条 Gateway non-loopback established socket，触发 manifest safety-stop；完整原始 capture 未保存 |
| exact cleanup | `PASS` | owned Desktop/Gateway/Science/driver 与五个目标端口全部清零；fixture before/after hash 不变 |

artifact v1 `9420824a-4464-4e22-8570-651a92d6cf46` 与 v2
`bc01eba7-9e92-429b-a25a-e0bea6aa7de6` 共享 artifact id
`94fc3ce6-fb23-4dbf-ad1b-0d2a5c70b3ac`；v2 的 parent 精确指向 v1，两版持久文件 hash 均与
record 一致，UI 观察到 1 deletion / 2 insertions diff。它们是可信的行为观察，但不是可从失败
隔离 envelope 中摘出的 B-CORE PASS。

Gateway 的 source boundary 与 operator observation 相互印证：[CONNECT handler](../../../desktop/gateway/src/connect.rs)
对非 Anthropic / Claude host 会直接解析并建立 TCP，上游连接不是 default deny；r3 的两条
`198.18.0.54:443` socket 在正式 event 中记录为 packaged Gateway 所有，并由后置 reduced receipt
物化。这里不把该地址外推成特定域名或 HTTPS，也不把源码调查代替 live 网络归因。它只说明在
不改代码、不施加影响真实机其他进程的全局 firewall、且不禁用 Science 原生 filesystem sandbox
的现有边界内，没有安全依据继续 r4。

r3 raw evidence 位于
`/private/tmp/csswitch-science-probe-evidence/bcore-20260808-r3/B-CORE-01/`。14 项 closure 全部复算
`OK`，`hashes.sha256` 自身 SHA-256 为
`fa320cd974749b3ab2a15ab9eb248ef4d4a900759d8cbb134df775a1693fd1a5`。owned PID
`655/926/935/960` 和 `52853/52854/52855/52856/8765` 最终均不存在；exact runtime/build root
和一次性 driver 已删除，raw evidence 保留，fixture before/after hash 不变且没有 `output.txt`。

## r1｜Exact source、artifact 与隔离边界

- source：`6e09e68654e1c43b936821c8be12330af3e19cc1`；其 15-suite source gate 和
  `B-RUNTIME-01` 前置均为 `PASS`；
- acceptance Desktop SHA-256：
  `4cb359b10b86a88632af33901173c5036b0567fbc70dbbafb002b6c4ccc2f4e4`；
- packaged Gateway SHA-256：
  `9ddb97f31e28f9ce7117867b54b02e1e2733ac5898b56c53cdd5e7b3313d92c3`；
- Science executable SHA-256：
  `63b0f57aa3b9588ba9e61433d27c78df788f8fe2c1b51842db107d6697e9c03f`；
- 动态端口：Gateway `60231`、Science `60232`、preview `60233`、loopback provider
  `60234`；`8765` 全程没有 listener；
- network profile SHA-256：
  `1c3449c8a517034125c590d19e82890a4b893cabcd1e8e4146d4241a40a1761a`。

network self-test 证明 IPv4/IPv6 loopback 可用，非 loopback TCP/UDP、DNS transport 与
mDNSResponder IPC 均被 `EPERM` 阻断，resolver lookup 按预期失败。运行只使用合成 project、
合成 fixture 和固定 fake credential；没有读取或回显真实 API key、OAuth、Keychain、SSH 私钥、
真实账号数据库或真实用户文件。

## Project 文件与 artifact lineage

最终对齐的 workspace 流程实际完成 write → read → save v1，随后 update → read → explicit
`version_of` save v2。两版共享 artifact id
`7b9937b1-1817-44c0-95f2-694a0e172b73` 和 root frame id
`00e15813-d7f6-48ba-8313-82dddb02e9af`：

- v1 version id `a02bbe82-5625-454a-8f5d-a86bfa9c2d72`，SHA-256
  `a73604b7c925ab7233a9d1851451d4fdfebb2c66681eafd269577b77eb1ee6d6`；
- v2 version id `11e5171a-e1e0-4232-8414-e516518a60cb`，SHA-256
  `1f9a3dd2f001d2ba4345ea68f3810154c14e022fa4ab0ccdcc08533c0421a324`。

两份持久 artifact 文件的重新计算哈希与 UI record 一致。UI 可回读 v1/v2 preview、两版 diff
和 execution log；这些是成功的行为观察，但缺少合规 evidence envelope，artifact sub-gate 仍为
`INCONCLUSIVE`。早期两次 workspace 尝试因 deterministic
mock response queue 未对齐而出现 `file not found` / 0 artifact；它们属于 harness noise，最终
对齐链路和持久文件 readback 只用于界定观察到的行为，不用于补写合同 PASS。

## Evidence envelope 缺口

运行前 manifest 已固定 probe/run id、source SHA、artifact/Science 路径与哈希、fixture 路径、
端口、scope 和 deadline，但缺少合同强制的 UTC 开始时间、执行者、repo root、branch、dirty/staged
摘要、artifact build version、Science 来源、OS/架构、fixture version 和显式 target layer。
`events.ndjson` 也没有逐条单调时间戳。运行后调查可以证明这些字段缺失，不能追溯声称它们已在
运行前冻结，也不能重建合规 timeline。因此本轮所有原本观察成功的 capability sub-gate 都保持
`INCONCLUSIVE(reason=evidence-envelope-incomplete)`；raw evidence 的
`evidence-limitations.json` 明确记录这一降级。

## r2｜no-opt-out 启动条件可行性重跑

第一次运行的 permission 结果明确显示 filesystem sandbox inactive。源码边界也明确：只有设置
`CSSWITCH_ACCEPTANCE_OUTER_SANDBOX=1` 且当前进程确实处于 deny-egress sandbox 时，
[Science launch script](../../../scripts/launch-virtual-sandbox.sh)才会注入
`--dangerously-no-sandbox`。r2 因此只测试一个最窄条件：继续使用同一 outer deny-egress
profile，但不设置该 acceptance opt-out，让 production one-click 尝试按默认参数启动 Science。
脱敏的 `launch-condition.json` 只记录 controller invocation 条件，不承担 exit 70 根因归属。

r2 在启动前冻结完整 manifest：repo root、`next@d0c12f8` 文档 HEAD、production source
`6e09e68`、clean dirty/staged 摘要、执行者、UTC、macOS 26.5.2 arm64、fixture
`bcore-fixture.v2`、`ISOLATED-LIVE(scope=isolated-local-mock)`、artifact build version、
Science 来源和 deadline 均齐备。由 `6e09e68` 全新构建并 ad-hoc 签名的 r2 Desktop SHA-256 为
`3497ce0f411ee59eb4cc13d58bea7cccf2ab18c1b55f9fbc81fd4a458dce9f4f`，packaged Gateway
SHA-256 为 `b1ea561a0063678c38a80381d717a9f298544ff0f79d6666b413831dc5520a39`；Science
executable identity 与 r1 相同。

network self-test 仍为 `PASS`。八条事件全部带 `CLOCK_MONOTONIC_RAW` 纳秒值并严格递增；
`observations.json` 对 manifest、timeline、network、launch condition、Science start、provider、
capability sub-gate、fixture 与两层 cleanup 逐项记录 expected、actual、evidence pointer 和判定。
production one-click 建立 Desktop `95000` 与 Gateway `95009`，随后 Science start script 以
`script_failed / exit_code=70` 失败；Science PID/listener 没有建立，CSSwitch 因 managed launch
身份无法确认而拒绝调用 stop 或发送信号。loopback mock PID `94723` 收到 0 个 provider request。
这不是 permission 能力 `FAIL`：运行没有到达 project、file、permission、artifact 或 annotation
路径。exit 70 是 launch script 对 Science `serve` 非零退出的通用映射，原始 stdout/stderr 没有进入
raw evidence，不能唯一归因为 filesystem sandbox 或 outer deny-egress。总判定只能是
`INCONCLUSIVE(reason=science-start-script-exit-70-before-identity-under-no-opt-out-outer-sandbox)`。

r2 raw evidence 位于
`/private/tmp/csswitch-science-probe-evidence/bcore-20260808-r2/B-CORE-01/`。12 项 closure
全部复算 `OK`，`hashes.sha256` 自身 SHA-256 为
`0b584bb7c99ddff2b2ba605d9f82d56acf72d4140d63db25bc570d6e7e9ac238`。owned PID
`94723/95000/95009` 和 `49950/49951/49952/49953/8765` 最终均不存在，runtime/build root
已删除且 fixture before/after 哈希不变。owned process/port 在 60 秒内清零；完整 runtime/build
root 删除在 failure 后 103.365 秒完成，超过 manifest 的 60 秒 cleanup deadline，因此只有最终
清零状态可确认，完整 cleanup timeline 保持 `INCONCLUSIVE(reason=cleanup-deadline-exceeded)`。

## Permission 与 annotation 的停止边界

permission 工具返回的 `granted=true` 不能按字面升级为 grant PASS；同一结果同时明确写明
filesystem sandbox 未激活、该路径本来就可访问、不会记录 grant，随后 grant 列表仍为空。
在没有 enforcement 与 grant id 的条件下继续读写、revoke 或越界请求，无法证明合同要求的
scope 与拒绝语义，所以本轮在首次明确缺少前提处停止，没有触碰 host fixture。

[Claude Science Annotations 官方说明](https://claude.com/docs/claude-science/annotations)要求在
Markdown 产物中选中文本并点击 Annotate，保存后的 pending annotation 随下一条消息传递。本轮在
v2 preview 与全屏 preview 中进行了四次 user-like 文本选区尝试，但 UI 没有出现 Annotate pill、
highlight badge 或 composer comments chip。为避免伪造 annotation，没有用脚本修改页面状态，也没有
发送不含 annotation 的下一条消息；该 sub-gate 保持 `INCONCLUSIVE`。

## 证据闭合与清理

raw evidence 位于
`/private/tmp/csswitch-science-probe-evidence/bcore-20260808-r1/B-CORE-01/`，包含 manifest、
events、permission events、fixture tree before/after、artifact / annotation assertions、
inventory before/after、evidence limitations、network receipt、cleanup 和 redacted mock request
形态；不保存 prompt、fixture 或 artifact 正文。16 项 `hashes.sha256` 全部复算 `OK`，closure
自身 SHA-256 为
`48f68a1558d5d664eb985883950ba616cbcb5ec2556675fba12d9858bce558f0`。

本轮 exact Desktop / Gateway / Science / mock PID `88515/88526/88550/89259` 均已退出；
`60231/60232/60233/60234/8765` 均关闭。精确 runtime root
`/private/tmp/csswitch-science-probe-runtime/bcore-20260808-r1` 和一次性 build root
`/private/tmp/csswitch-bcore-build.tdizFC` 已删除；sibling raw evidence 保留。没有停止或删除其他
Science、Python 或用户进程/路径。这些是最终清零状态观察；缺少单调事件时间戳意味着 cleanup
timeline 与 deadline 仍为 `INCONCLUSIVE`，不能写成合同级 cleanup PASS。

## 不能外推

本记录不证明 permission enforcement、annotation propagation、`B-CONTEXT-01`、真实 provider / 账号、
真实凭证、Skill、SSH、installed CSSwitch、正式签名、公证、DMG、tag 或 public release；这些保持
`NOT-RUN` / `INCONCLUSIVE`。本轮没有改产品代码，没有 push、tag、release 或替换已安装 App。
