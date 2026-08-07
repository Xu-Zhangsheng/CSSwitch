# Isolated-live pre-run receipts 与 egress guard 闭环

状态：pre-run 已执行；runtime 未执行；`INCONCLUSIVE(reason=pre-run-only)`

适用范围：`c531006595709ea1247d03932526901b056adf99` 的 isolated-live
controller/tests、`e7dfde13636cbf3b377d01dbba3a2aee88e62822` 的既有 exact
`CSSwitch Test.app`、Claude Science 0.1.25 package/executable、deterministic loopback
provider，以及尚未启动 production executable 的 pre-run evidence closure。

最后复核：2026-08-07（Asia/Taipei）

## 判定

| Sub-gate | 结果 | 证据边界 |
|---|---|---|
| controller source | `PASS` | 当前 controller SHA-256 与 `c531006` blob 均为 `7320352e3fe930e41ec416c81740a7cd388fb169ebbcbfef10eec2e0cef854ae` |
| source gate | `PASS` | 独立 clean clone、exact HEAD `c531006`、15 suites、runner exit `0` |
| G1 binding | `PASS` | pre-run manifest 递归校验 `e7dfde1` completion seal、artifact record、CSSwitch bundle/Gateway 与 Science package/executable identity |
| complete manifests / receipts | `PASS` | artifact 与 Science 全树 manifest、fixture receipt、provider launch receipt、network isolation receipt 均在 launch 前冻结 |
| network guard self-test | `PASS` | IPv4/IPv6 loopback 可用；IPv4/IPv6 TCP、UDP、DNS transport 与 mDNSResponder IPC 均以 `EPERM` 阻断；唯一系统 resolver 查询失败 |
| evidence closure | `PASS` | held directory FD 下原子写入、递归 reread/hash、public path inode binding；`hashes.sha256` 19/19 `OK` |
| production launch / runtime | `NOT-RUN` | 本轮刻意停在 pre-run，没有启动 Desktop、Gateway 或 Science |

因此本轮只关闭“完整 pre-run identity/fixture/provider/network receipts 与实际 egress
guard”缺口。总判定固定为 `INCONCLUSIVE(reason=pre-run-only)`；它不会把旧 G2 safety-stop
追溯改写成 PASS，也不证明 open、reopen、status、stop、restart 或 provider inference。

## Exact identity 与 hashes

- controller/test commit：`c531006595709ea1247d03932526901b056adf99`；提交只修改
  `test/installed_provider_matrix.py` 与 `test/test_installed_provider_matrix.py`。
- controller SHA-256：
  `7320352e3fe930e41ec416c81740a7cd388fb169ebbcbfef10eec2e0cef854ae`。
- exact-SHA source gate run：`598e5eef487b7a6595d27bc7ea2ebcef`；completion seal
  SHA-256：`ac7b2d5f1d80a05b4c1ec70eb24ace9497d46ed75a422c024178ca29782bd218`。
- G1 artifact source：`e7dfde13636cbf3b377d01dbba3a2aee88e62822`；completion seal
  SHA-256：`389442dcad1a6fdad0e6b1ba8cf9ea01da1407a5464e917d2fb98bdc1e002b60`。
- pre-run manifest SHA-256：
  `9dd77aaa73512eb9fb32542638a479cfac52d92dd877f54215575189ff449b78`。
- evidence `hashes.sha256` SHA-256：
  `9d9298e426629d7c5284a18d077bf4ec84e2cc2476631e9421772fa70cacec1f`；
  19/19 entries `OK`。

pre-run 生成时主 worktree 仍在 `25aa816`，且只含随后进入 `c531006` 的两个 controller/test
文件修改；fixture receipt 冻结的 controller hash 与 `c531006` blob、当前 tracked 文件
完全一致。随后在 exact `c531006` 独立 clean clone 上取得 source-gate seal。该双重绑定只证明
controller 与测试身份，不把 `e7dfde1` product artifact 伪写成由 `c531006` 构建。

## 不能外推

- 完整 `B-RUNTIME-01`：`NOT-RUN`；open/reopen/status/stop/restart 均未由本轮执行。
- provider prompt、stream、tools、reasoning、error semantics 与 inference hits：`NOT-RUN`。
- 真实 provider、OAuth / 账号、Skill、SSH server：`NOT-RUN`。
- installed App、签名、公证、Gatekeeper、DMG 与 public release：`NOT-RUN`。

本轮原始 evidence root 位于
`/private/tmp/csswitch-g2-fixed-receipts-20260807-v5/evidence/`。该临时路径不是仓库内耐久
artifact；本文件只保留已复核的 exact identity、hash、判定和外推边界。
