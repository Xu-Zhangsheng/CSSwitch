# Claude Science 0.1.25 `B-CORE-01` exact-artifact isolated-live 验收

状态：`PASS`

适用范围：`next@9e08924481c8f5edb181254332d94daba0cbe4b2`、由该 exact source 构建并经 G1 receipt 绑定的 `CSSwitch Test.app`、packaged Rust Gateway、Claude Science 0.1.25，以及本轮隔离 HOME/data-dir、loopback mock 和专用合成 project / 文件 fixture。

最后复核：2026-08-11（Asia/Taipei）

失效条件：artifact-producing source、CSSwitch bundle/Desktop/Gateway、Science package/executable、`B-RUNTIME-01` 前置、host permission 合同、artifact / annotation 结构或 `B-CORE-01` 范围内行为任一变化时，本结论不得外推。

## 结论

同一个 `9e08924` exact artifact 已完成限定的合成数据验证：project 持久回读、workspace 与显式 grant path 文件读写、permission request / grant / UI revoke、revoke 后拒绝与 sibling 越界拒绝、artifact v1 → v2 lineage / diff / preview / execution provenance，以及 annotation 真实定位和下一条消息传递均为 `PASS`。完整运行只使用专用 synthetic Git fixture 与 loopback mock；没有读取账号数据库、真实用户文件或凭证，也没有扩大到 B-CONTEXT、完整 Provider、Skill / MCP、SSH、installed、签名或发布。

## Exact identity 与前置

| Identity | Exact value | 判定 |
|---|---|---|
| artifact-producing source | `9e08924481c8f5edb181254332d94daba0cbe4b2` | `PASS` |
| source gate run | `078d462c81abcec146644c8096254069`；15/15 suites、15/15 observations | `PASS` |
| source completion seal SHA-256 | `1c071a18701ac6e6a191c6dfc3cd1513d3465bce90d3567a6460cd46f36d0fe8` | `PASS` |
| G1 binding receipt SHA-256 | `39aa3dc5866807140d42409ab8eea2f6625fa4d9f7212f06482dcc0db0c9b762` | `PASS` |
| CSSwitch bundle canonical digest | `d77cb799f2063241041cc8d17bd57a3c72f49cc7b0da6899d63a52795df1ac64` | `PASS` |
| Desktop SHA-256 | `ada760cb9dcfdd9b2151d652ff744f300a914b3bef8c07ea85ce886994458a6b` | `PASS` |
| packaged Gateway SHA-256 | `bfa05512337329f52811c2d7c08081ed2249a34c6ed98dd7fe7d83313d9b3036` | `PASS` |
| Science version / executable SHA-256 | `0.1.25` / `63b0f57aa3b9588ba9e61433d27c78df788f8fe2c1b51842db107d6697e9c03f` | `PASS` |
| `B-RUNTIME-01` 前置 | canonical run `r9e08924b`；51/51 hashes `OK` | `PASS` |

运行 artifact 直接来自保留的 G1 root `/private/tmp/g1.9e08924.91acQY/`，没有重新构建或替换 `/Applications/CSSwitch.app`。后续 evidence-only 文档提交不改写被构建或运行的 source/artifact identity。

## 范围内观察

| 子门 | Exact observation | 结果 |
|---|---|---|
| project | project `proj_85ee4486eb84` 与 root frame `b4d65caa-2029-4131-a40e-2ea13bce869e` 建立，并从独立 Chrome 会话回到同一 project/session | `PASS` |
| permission request / grant | 初始 host grant 列表为空；grant 前绝对路径读取被拒；UI 只批准 `/Users/superjj/ccproj/CSSwitch-bcore-9e08924-r1-allowed` 的 `rw`，host / guest path 精确匹配 | `PASS` |
| 文件读写 | grant 后 seed 回读成功；写入 `output.txt` SHA-256 为 `6b8ee854e7d543913436a71c0fe01ffcba2e499dd8f00c7341f6e67878770497`；seed SHA-256 保持 `57864358…` | `PASS` |
| revoke 后拒绝 | Settings → Permissions 对 exact fixture 执行 UI revoke；host grant 回读为空，同一路径读取被拒 | `PASS` |
| 越界拒绝 | sibling denied path `deny.txt` 读取被拒，未产生新 access request，fixture SHA-256 前后均为 `a9aeaa9d…` | `PASS` |
| artifact lineage | artifact `2b0be614-32e5-475b-9923-b8be4156a94d` 的 v2 使用 v1 version ID 作为 `version_of`；两版 product output、文件 hash 和 previous-version UI 对齐 | `PASS` |
| artifact diff / preview / provenance | UI 明确显示 one deletion、two 与 `BETA-LINE-9E-R1` insertions，v1/v2 preview 与同一 root frame 的 edit/read/save tool steps 对齐 | `PASS` |
| annotation 定位 | Chrome 真实 pointer drag 只选中 `ALPHA-LINE-9E-R1`，保存 comment `BCORE_ANNOTATION_9E_R1`；composer 显示 1 comment | `PASS` |
| annotation 传递 | 下一条消息卡片同时含 filename、选区与 comment；loopback request 三项结构/marker 均命中；发送后 pending annotation button 为 0 | `PASS` |

artifact v1 ID 为 `98633efa-94e2-4450-a3e7-2fd830fd30b8`，SHA-256 为 `c642a3e4ba53e031e794dbfbc49a9e9c1dd0fc68c74d196ccab7dac5472b93f9`；v2 ID 为 `e1ec5f94-ddef-4fb0-9b8c-d60675e7d916`，SHA-256 为 `5876694ead539810390a27f48f370b172d4cfa20c37f6453edd89516fab65899`。本轮没有读取 Science 账号数据库；identity、lineage、annotation 和 pending 状态均从产品正常 UI、产品 tool output、synthetic artifact 文件与脱敏 loopback envelope 取得。

## 隔离、网络与清理

活动期 raw socket capture 共 38 rows，其中 11 listeners、27 established；所有地址均为 `127.0.0.1` / `::1`，non-loopback rows 为 0，端口 `8765` rows 为 0。driver、Desktop、Gateway 与 Science 四个 exact PID 最终全部不存在；本轮四个动态端口及 `8765` 均关闭。

本轮 runtime root、allowed / denied synthetic Git fixture、临时 driver、Python cache 与 fixture stage 已精确删除。driver 源码在删除临时路径前以相同 SHA-256 `8cf27308…` 复制进 evidence；exact source worktree、G1 artifact 和 canonical raw evidence 保留供后续层复算。主 worktree 清理时为 clean，HEAD 为 `1dd97a6ac34264568f08a9307e5e21a887006f80`。

## Evidence closure

raw evidence 根为 `/private/tmp/csswitch-science-probe-evidence/bcore-9e08924-r1/B-CORE-01/`。最终 `hashes.sha256` 索引 14 个文件，`shasum -a 256 -c hashes.sha256` 为 14/14 `OK`；`hashes.sha256` 自身 SHA-256 为 `f7710659cdf5dc052885e7fbc1dcd25308b4985dc0bb479d6943d02d61f69607`。manifest SHA-256 为 `62e477daa32cd30a5d463883b3e6d9574f398c039b1168edc94d932166a4f7fd`；`completion-summary.json`、`cleanup-ledger.json` 与 `network-socket-observation.json` SHA-256 分别为 `32208d996585b83d19aa36390c458c029b111a7e22fb3b94e44bf2871074834a`、`0f634fbbf41af2bf17ebc195a49a87c88ba214c794549a73f5927cfcdbc387d9`、`5a0ca622ed1edf0e5fd06c3e580600296add0827ba12be84c6bd53dbc75f954d`。

16 条 events 的 `monotonic_ns` 严格递增且 decision 全为 `PASS`；79 条 mock hits 省略 request/response body，其中 11 条 annotation-transfer envelope 的结构键、选区 marker 与 comment marker 全为 true。G1 root 5/5 closure 再次复算 `OK`。

## 不能外推

本记录不证明 B-CONTEXT、完整 Provider 或真实 provider / 账号、真实凭据、Skill / MCP runtime、SSH、installed CSSwitch、签名、公证、Gatekeeper、DMG、tag、push 或 public release，也不把本轮 normal UI / persistence path 外推为 crash、并发竞争、compensation、replay 或通用数据恢复。
