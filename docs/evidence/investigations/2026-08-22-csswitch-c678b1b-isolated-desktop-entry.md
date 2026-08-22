# 2026-08-22 CSSwitch `c678b1b` Acceptance Desktop 隔离生产入口

状态：日期化证据；`PASS(scope=RM-42 Desktop production entry)`、`NOT-RUN(scope=Gateway / Science normal wiring)`

适用范围：由 `c678b1bee2475686ebbb8b8172c24112a37c6ce9` 生成、并已在 exact-artifact 层通过的 macOS `CSSwitch Test.app` 0.8.4。本文只证明 packaged Desktop 的正常生产入口在一个全新 guard-managed environment 中能运行、保持隔离并安全停止；不证明 Gateway / Science 的完整链路、Provider、SSH、账号、installed App、签名、公证、Gatekeeper、DMG 或公开发布。

最后复核：2026-08-22（Asia/Taipei）

失效条件：artifact-producing source、Desktop 或 Gateway hash、Acceptance compiled data root、guard、runtime 配置、端口或本证据的运行范围任一改变时，不得继承本文结论。

## 结论

本轮以新的私有临时 root 执行 `preflight → prepare-codex → env`，并将 exact packaged
`Contents/MacOS/desktop` 作为生产入口直接启动。前两次非交互 background wrapper 的 child 在 shell
结束后消失，未产生日志；随后以前台 direct entry 运行并取得稳定的 exact Desktop PID。该进程使用
`$HOME/.csswitch-acceptance`，不会出现正式 `$HOME/.csswitch`；它将无凭据 v3 fixture 迁移为 schema 4，
但继续保持 0 profiles、空 active id、Codex 实验开关 `false` 和 guard 分配的两个动态端口。运行时和终止后
guard 都确认真实 Science 的 `8765` listener PID 不变；没有 OAuth 文件、TCP socket、Gateway 或 Science
listener。精确核验路径后向该 Desktop PID 发送 `TERM`，两秒内退出；最终 `assert-stopped` 通过，两个测试端口
均已释放。

由于 fixture 没有 profile，也没有发起 one-click、OAuth、Gateway / Science 启动或任何请求，本次 `PASS`
只覆盖 RM-42 的 Desktop production entry / isolation / stop。它不能写成完整 `isolated-live`、Gateway /
Science normal wiring、Provider、SSH、账号或 installed runtime 的通过。

## Identity 与环境

| 项目 | 实测 | 判定 |
|---|---|---|
| Artifact-producing source | `c678b1bee2475686ebbb8b8172c24112a37c6ce9` | 与 exact-artifact evidence 相同 |
| Evidence baseline | `main@54330099ec0d948b25902fd6969a0239ab9621c7` | docs-only parent；不是 producing source |
| Bundle ID / version | `com.csswitch.test` / `0.8.4` | `PASS` |
| Desktop SHA-256 | `204d174f97f45bce9d3be33027f44b7a1c8d40218d5a85bc3035a038d006d712` | 与 exact-artifact evidence 一致 |
| Gateway SHA-256 | `d012958d5f24a193b1ed46fa2bbcbea4cfd05f028527b6fed6c375fb0e7f32a2` | 与 exact-artifact evidence 一致 |
| Test root / HOME | 新建私有 `/private/tmp` root；其下 fresh `HOME` | `PASS`；未引用真实 HOME |
| Gateway / Science test ports | `53726` / `53727` | `PASS`；均不等于 `8765`、`1455`、`1457` |
| 8765 baseline | preflight、运行态 guard、final `assert-stopped` 三次一致 | `PASS` |

启动命令的环境是显式最小集合：`HOME=<fresh-home>`、`TMPDIR=/private/tmp`、受限 `PATH`、假的
`USER` / `LOGNAME`、`CSSWITCH_REPO=<c678b1b build worktree>` 和两个 guard 端口；没有继承真实凭据环境。
没有读取真实 `~/.csswitch`、`~/.claude-science`、Keychain、OAuth/token、API key、账号数据库或 SSH 私钥。

## 运行与停止证据

1. `preflight` 通过：fresh HOME、动态端口空闲、至少一个 OAuth callback 兼容端口空闲，且 8765 listener
   baseline 已记录。
2. `prepare-codex` 通过：只在 fresh HOME 写入 v3 空配置；没有 profile、token、credential ref 或 OAuth 文件，
   Codex 实验开关默认关闭。
3. 前台 direct entry 的 exact Desktop PID `30928` 在后续复核时仍为运行态，`comm` 精确等于 packaged
   `CSSwitch Test.app/Contents/MacOS/desktop`。它不是 `/Applications/CSSwitch.app`。
4. 运行态配置为 schema `4`、profiles `0`、active id 空、`experimental_codex_enabled=false`，端口仍为
   `53726` / `53727`；fresh HOME 中不存在正式 `.csswitch`，Acceptance root 下 OAuth 文件数为 `0`。
5. 该 PID 没有 TCP socket；本轮没有启动 Gateway / Science listener，也没有发出 Provider、Science、SSH、
   账号或 OAuth 请求。运行态 `guard` 通过，8765 baseline 未变。
6. 停止前再次核验 PID 的 executable identity；没有 direct child PID。对该 PID 单独发送 `TERM` 后两秒内退出。
   `assert-stopped` 通过：8765 baseline 未变，`53726` / `53727` 已释放，exact artifact process 不存在。

没有替换、启动或停止 `/Applications/CSSwitch.app`；没有进行 signing、notarization、Gatekeeper、DMG、tag、push、
release 或真实服务测试。

## 分层边界

| 层 | 结果 | 边界 |
|---|---|---|
| Production source | `PASS` | 仅 `c678b1b` 的既有 15/15 source gate |
| Exact artifact | `PASS` | 仅 [2026-08-22 exact artifact evidence](2026-08-22-csswitch-c678b1b-exact-artifact.md) 的 packaging / hash / Gateway status scope |
| Acceptance Desktop production entry / RM-42 isolation | `PASS` | 本文的 fresh HOME、dynamic-port、8765 guard、empty Codex fixture、exact PID 与安全停止 |
| Gateway / Science normal wiring | `NOT-RUN` | 本轮没有 profile 或 one-click，故没有 Gateway / Science production entry 或 `assert-running` |
| Provider / SSH / account / OAuth | `NOT-RUN` | 没有真实凭据、账号、browser 或网络请求 |
| Installed runtime | `NOT-RUN` | `/Applications/CSSwitch.app` 未触碰 |
| Signing / notarization / Gatekeeper / release | `NOT-RUN` | 没有做分发或公开层动作 |

完整 Gateway + Science normal wiring 必须在另一轮新的 guard root 中，从正常 production caller / one-click
进入，并以独立 evidence 记录；不得把本篇 Desktop entry PASS 作为替代。
