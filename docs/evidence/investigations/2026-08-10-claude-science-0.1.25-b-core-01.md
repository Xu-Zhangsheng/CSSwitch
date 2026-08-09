# Claude Science 0.1.25 `B-CORE-01` exact-artifact isolated-live 验收

状态：`PASS`

适用范围：`next@06b630bb3e3fb0d63425bf48d1ffc2d3613fd992`、由该 exact source 构建并经 G1 receipt 绑定的 `CSSwitch Test.app`、packaged Rust Gateway、Claude Science 0.1.25，以及本轮隔离 HOME/data-dir、loopback mock 和专用合成 project / 文件 fixture。

最后复核：2026-08-10（Asia/Taipei）

失效条件：artifact-producing source、CSSwitch bundle/Desktop/Gateway、Science package/executable、`B-RUNTIME-01` 前置、host permission 合同、artifact / annotation 持久化结构或 `B-CORE-01` 范围内行为任一变化时，本结论不得外推。

## 结论

同一个 `06b630b` exact artifact 已完成本轮限定的合成数据验证：project 持久记录与文件读写、permission request / grant / revoke、revoke 后拒绝与 sibling 越界拒绝、artifact v1 → v2 lineage，以及 annotation 保存后的持久状态和下一条消息传递均为 `PASS`。完整运行只使用专用合成 fixture 与 loopback mock，没有扩大到 B-CONTEXT、真实 provider、Skill / MCP、SSH、installed、签名或发布。

## Exact identity 与前置

| Identity | Exact value | 判定 |
|---|---|---|
| artifact-producing source | `06b630bb3e3fb0d63425bf48d1ffc2d3613fd992` | `PASS` |
| source gate run | `8728828e41ee0dc8ab578856995fa530`；15/15 suites、15/15 observations | `PASS` |
| source completion seal SHA-256 | `e1ef35f06d0f88a8cbf6fa4ea8b12c129afd10b55f85f89d2074b0d23d72b455` | `PASS` |
| G1 binding receipt SHA-256 | `f6490b1765d982c4453571676cb3561f6f1c3a20a9af3850d30f8e405e795573` | `PASS` |
| CSSwitch bundle canonical digest | `634c13f2597c10cbbf75a7cac8d1af135523eccff2cb86373824695cccb32e1a` | `PASS` |
| Desktop SHA-256 | `cf0e84e6b33b761767394b6d5f3579e310bec5c07de8015cd79f2d407d9d5274` | `PASS` |
| packaged Gateway SHA-256 | `ed4dae8ec8139c4828dd0915d1594d69001e7b504c582a710e905707ef9d03d1` | `PASS` |
| Science version / executable SHA-256 | `0.1.25` / `63b0f57aa3b9588ba9e61433d27c78df788f8fe2c1b51842db107d6697e9c03f` | `PASS` |
| `B-RUNTIME-01` 前置 | run `r06b630bb`；报告起始绑定 SHA-256 `f937153fb76dc2228d63249dd04b6e6216fa1b3998f8e264e0e17c056a1969a5` | `PASS` |

运行 artifact 直接来自保留的 G1 root `/private/tmp/g1.06b630b.Yv5hDZ/`，没有重新构建或替换 `/Applications/CSSwitch.app`。后续 evidence-only 文档提交不改写被构建或运行的 source/artifact identity。

## 范围内观察

| 子门 | Exact observation | 结果 |
|---|---|---|
| project | 持久 project `proj_95425ad9b5f6` 与 root frame `4781de64-5916-468a-a690-612ecc1bdaa2` 建立 | `PASS` |
| permission request / grant | 初始 grant 列表为空；grant 前绝对路径读取被拒；UI 明确批准专用 allowed root 的 `rw`，host / guest path 均精确匹配 | `PASS` |
| 文件读写 | grant 后读取成功；写入文件 SHA-256 为 `aebebbcf34a97e90c632d45222dd5ab3563fd6cc20a562fe0c0792998418cba5`；原 seed SHA-256 保持不变 | `PASS` |
| revoke 后拒绝 | UI revoke 后 grant 列表为空，同一路径读取被拒 | `PASS` |
| 越界拒绝 | sibling denied root 读取被拒，未产生新的 access request，fixture SHA-256 保持不变 | `PASS` |
| artifact lineage | artifact `5890d428-5ffe-4bbc-b28e-06708be87826` 的 v2 parent 精确指向 v1；两版 hash、producing frame、previous-version UI 与 diff UI 对齐 | `PASS` |
| annotation 持久状态 | 真实选区 `ALPHA-LINE-06B-R2` 与 comment `BCORE_ANNOTATION_06B_R2` 在发送前存在持久 DB row，target checksum 匹配 v2 | `PASS` |
| annotation 传递 | 下一条消息卡片含 filename、选区与 comment；loopback request 三项结构/marker 均命中；发送后 pending count 为 0 | `PASS` |

artifact v1 ID 为 `2c3eb7ef-670a-4e11-a6f7-85706a49d67f`，SHA-256 为 `c727e792db15fd793e49d39fa828246b5233ae39aad8fff4bc27472b815cd0f4`；v2 ID 为 `b51c2f5f-4129-4650-84ba-00077930fead`，SHA-256 为 `6fd5a338dca0e8e31628c09bda5f954898b18cbeeb905f71308a69ec8057bfed`。annotation row ID 为 `9f1fc0e8-f5c8-485a-8644-4e56a9d30c81`，target 为 `av:b51c2f5f-4129-4650-84ba-00077930fead`。

## 隔离、网络、失败尝试与清理

活动期 raw socket capture 共 39 rows，其中 11 listeners、28 established；所有地址均为 `127.0.0.1` / `::1`，non-loopback rows 为 0，端口 `8765` rows 为 0。Desktop、Gateway、Science 和 driver 的 4 个 exact PID 最终全部不存在；本轮 4 个动态端口以及 `8765` 均关闭。

本轮 runtime root、allowed / denied fixture 和外部 driver 已精确删除；失败尝试 r1 的 driver 及 r2 fixture stage 也已删除。候选 r2 evidence、失败 r1 evidence 和 exact G1 artifact 按证据保留。主 worktree 在清理核对时为 clean，HEAD 为 `d1ba10ec26f989142b032b6d453d6c708299a5a2`。

r1 因 fixture 位于 Science 无法申请 host permission 的 `/private/tmp`，固定为 `INCONCLUSIVE(reason=fixture-root-unavailable)`；grant 后各子门未运行，且 r1 不参与本次 PASS。r2 改用 HOME 内专用 sibling synthetic Git fixture 后完成全部范围内子门，这只修复 fixture 可达性，不改变产品实现或验收范围。

## Evidence closure

raw evidence 根为 `/private/tmp/csswitch-science-probe-evidence/bcore-06b630b-r2/B-CORE-01/`。最终 `hashes.sha256` 索引 23 个文件，`shasum -a 256 -c hashes.sha256` 为 23/23 `OK`；`hashes.sha256` 自身 SHA-256 为 `e0973c24672efa7b67551d3224886dd98ba34924eff28c43ce104a4653b2c379`。`completion-summary.json` SHA-256 为 `0622ab2f53191183df366f65a4b730c92aaa17bb11aeaeb0e9b29b6f73046ccd`，`cleanup-ledger.json` SHA-256 为 `68410cb1280f4ba4b9d1994c432839af020ac9fabaf35000346f2078c94b211c`。

证据包内 G1 closure 的 5 项引用复算 5/5 `OK`；source completion seal 所引用的 evidence manifest、run manifest 与 `snapshot/source-snapshot-manifest.json` 均按原相对路径保留并复算一致。

## 不能外推

本记录不证明 B-CONTEXT、真实 provider / 账号、真实凭据、Skill / MCP runtime、SSH、installed CSSwitch、签名、公证、Gatekeeper、DMG、tag、push 或 public release，也不把本轮 normal UI / persistence path 外推为 crash、并发竞争、compensation、replay 或通用数据恢复。
