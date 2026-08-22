# Claude Science 0.1.25 `B-CORE-01` `a60c2ee` exact-artifact isolated-live 验收

状态：`PASS(scope=isolated-local-mock-native-science-sandbox)`

适用范围：`next@a60c2ee656429903f1fd8f398dc6ad8194aa9346`、由该 exact source 全新构建并经 G1 receipt 绑定的 `CSSwitch Test.app` 0.8.4、packaged Rust Gateway、Claude Science 0.1.25，以及本轮隔离 HOME/data-dir、固定假凭据、loopback mock 和两个专用合成 Git fixture。

最后复核：2026-08-11（Asia/Taipei）

失效条件：artifact-producing source、CSSwitch bundle/Desktop/Gateway、Science package/executable、`B-RUNTIME-01` 前置、host permission 合同、artifact / annotation 结构或 `B-CORE-01` 范围内行为任一变化时，本结论不得外推。

## 结论

同一个 `a60c2ee` exact artifact 已完成限定的合成数据验证：project 建立、重开与 runtime restart 后持久回读，workspace 与显式 grant path 文件读写，permission request / grant / UI revoke，revoke 后拒绝与 sibling 越界拒绝，artifact v1 → v2 lineage / diff / preview / execution provenance，以及 annotation 真实定位和下一条消息传递均为 `PASS`。

运行只使用专用 synthetic Git fixture、假凭据与 loopback mock；没有读取账号数据库、真实用户文件、Keychain、OAuth token 或 SSH 私钥。正式 clean-context reviewer 独立复算 identity、hash、事件、mock、network 与 cleanup 后取得 `BLOCK/HIGH/MEDIUM/LOW=0/0/0/0`、`PASS`。

## Exact identity 与前置

| Identity | Exact value | 判定 |
|---|---|---|
| artifact-producing source | `a60c2ee656429903f1fd8f398dc6ad8194aa9346` | `PASS` |
| source gate run | `6e5124c43c1ceadac3bd54f6408e2141`；15/15 suites、15/15 observations | `PASS` |
| source completion seal SHA-256 | `fe8d24939dd8b950c95984383645aa8de87dae4956c5c34c9a533b310560d86e` | `PASS` |
| G1 binding receipt SHA-256 | `b01cf7a2066576dfc5da1a59eada2faad9f586100f1b27d013cad9b4d420ddbb` | `PASS` |
| CSSwitch bundle canonical digest | `52cd48c06b2d2bffdd6d8e85d464be063e4604c71961a5cc8459879938a673c9` | `PASS` |
| Desktop SHA-256 | `77b4ebefe6e658c4f8f90e2b5177db5398be75eacf3b3f0d3364269eba5e9be8` | `PASS` |
| packaged Gateway SHA-256 | `610c0206973ec0281b6e34e171e4a6fb9899b31050642a073c4f0d1ca64887ab` | `PASS` |
| Science version / executable SHA-256 | `0.1.25` / `63b0f57aa3b9588ba9e61433d27c78df788f8fe2c1b51842db107d6697e9c03f` | `PASS` |
| `B-RUNTIME-01` 前置 | canonical run `ra60c2eec`；51/51 hashes `OK` | `PASS` |

运行 artifact 直接来自保留的 G1 root `/private/tmp/g1.a60c2ee.q4uEQl/`，没有重新构建或替换 `/Applications/CSSwitch.app`。exact source worktree 保持 detached、clean；后续 evidence-only 文档提交不改写被构建或运行的 source/artifact identity。

## 范围内观察

| 子门 | Exact observation | 结果 |
|---|---|---|
| project / restart persistence | project `proj_bce2bea1313d` 与 root frame `6bacc89c-9eab-4ec7-9fa1-989c71cf519b` 建立；两次由 exact Desktop UI 重启生产链后仍回到同一 project 与 artifact v2 | `PASS` |
| permission request / grant | 初始 host grant 列表为空；grant 前绝对路径读取被拒；UI 只批准 `/Users/superjj/ccproj/CSSwitch-bcore-a60c2ee-r1-allowed` 的 `rw`，host / guest path 精确匹配 | `PASS` |
| 文件读写 | grant 后 seed 回读成功；写入 `output.txt` SHA-256 为 `5bb364c68dbad599f80032aecc7e04eb86b449dc0bce7436111dcf73f1445cc1`；seed hash 保持不变 | `PASS` |
| revoke 后拒绝 | Settings → Permissions 对 exact fixture 执行 UI revoke；host grant 回读为空，同一路径读取被拒 | `PASS` |
| 越界拒绝 | sibling `deny.txt` 读取被拒，未产生新 access request，fixture HEAD、status 与 SHA-256 保持不变 | `PASS` |
| artifact lineage | artifact `60c5b1a5-ddbd-413e-90c0-43c26a37e294` 的 v2 使用 v1 version ID 作为 `version_of`；两版产品输出、文件 hash 和 previous-version UI 对齐 | `PASS` |
| artifact diff / preview / provenance | UI 明确显示 `one` deletion、`two` 与 `BETA-LINE-A60-R1` insertions，v1/v2 preview 与同一 root frame 的 edit/read/save tool steps 对齐 | `PASS` |
| annotation 定位 | 独立 clean-context Chrome witness 以真实 pointer drag 精确选择 `ALPHA-LINE-A60-R1`，保存 comment `BCORE_ANNOTATION_A60_R1`；composer pending 为 1 | `PASS` |
| annotation 传递 | 下一条消息卡片同时包含 filename、选区与 comment；11 条 loopback envelope 的结构键、选区 marker 与 comment marker 均为 true；发送后 pending 为 0 | `PASS` |

artifact v1 ID 为 `9186bbad-d4ad-4891-903e-f950c0bd971d`，SHA-256 为 `f4296b065840f913f4476a475f9fab2bdec526ffb330a16131605ea74dbd77ce`；v2 ID 为 `1505cbb5-05ed-4cc1-af34-edc26acc3260`，SHA-256 为 `2c9b531010093b3673e4f1569efb1442d9b2507e99fe1549c772b229d62881f2`。identity、lineage、annotation 与 pending 状态只来自产品正常 UI、产品 tool output、synthetic artifact 文件与脱敏 loopback envelope；一次性登录 nonce 未进入 evidence。

## 隔离、网络与清理

活动期 raw socket capture 共 31 rows，其中 11 listeners、20 established；31/31 地址均为 `127.0.0.1` / `::1`，non-loopback rows 为 0，端口 `8765` rows 为 0。driver、Desktop、三代 Gateway 与三代 Science 的 8 个 exact PID 最终全部不存在；本轮四个动态端口及 `8765` 均关闭。

runtime root、allowed / denied synthetic Git fixture、临时 driver、Python cache 与 fixture stage 共 6 个精确路径已删除。driver 源码在删除临时路径前以相同 SHA-256 `b06e7fc744ab664cf42f1777e8fa62d818b9c0460c20cca6f040382e83ce6080` 复制进 evidence。主 worktree 只保留进入本层前就存在、未读取也未改动的 `.tmp-bskill-r9-driver.py`；exact source、G1 artifact、B-RUNTIME 与本轮 canonical evidence 保留供后续层复算。

## Evidence closure

raw evidence 根为 `/private/tmp/csswitch-science-probe-evidence/bcore-a60c2ee-r1/B-CORE-01/`。最终 `hashes.sha256` 索引除自身外全部 30 个文件，`shasum -a 256 -c hashes.sha256` 为 30/30 `OK`；`hashes.sha256` 自身 SHA-256 为 `51bff6568dc44d84e41c4224f21e8497182543d5ce341ba5d11f19bcbb9ae885`。

22 条 events 的 `monotonic_ns` 严格递增且 decision 全为 `PASS`；79 条 mock hits 的 `request_number` 为 1–79，全部省略 request/response body，其中 11 条 annotation-transfer envelope 的结构键、选区 marker 与 comment marker 全为 true。copied G1/source-gate/B-RUNTIME binding 共 15 个文件与原件 byte-identical；原始 closure 分别重新复算为 G1 5/5、source gate 15/15 suites 与 15/15 observations、B-RUNTIME 51/51。CSSwitch bundle/Desktop/Gateway 与 Science package/executable identity 也再次重算一致。

## 不能外推

本记录不证明 `B-CONTEXT-01`、完整 Provider 或真实 provider / 账号、真实凭据、Skill / MCP runtime、SSH、installed CSSwitch、升级或 rollback、Developer ID 签名、公证、Gatekeeper、DMG、tag、push 或 public release，也不把本轮 normal UI / persistence path 外推为 crash、并发竞争、compensation、replay 或通用数据恢复。
