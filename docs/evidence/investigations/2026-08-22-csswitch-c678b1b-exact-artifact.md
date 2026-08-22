# 2026-08-22 CSSwitch `c678b1b` exact Acceptance artifact

状态：日期化证据；`PASS(scope=exact-artifact)`、`PASS(scope=isolated-gateway-status)`

适用范围：由 `c678b1bee2475686ebbb8b8172c24112a37c6ce9` 在干净 detached worktree 中新构建的 macOS `CSSwitch Test.app` 0.8.4。本文不覆盖 Desktop 启动、Science、Provider、SSH、installed App、签名、公证、Gatekeeper、DMG 或公开发布。

最后复核：2026-08-22（Asia/Taipei）

失效条件：artifact-producing source、Acceptance feature、Desktop/Gateway/resources、manifest/version 或本证据所述空 HOME Gateway status 任一改变时，本结论不得外推。

## 结论

`c678b1b` 的允许环境 `GATE-SOURCE` 已为 `PASS`；在用户授权的本轮中，它又生成了新的 Acceptance artifact。构建退出 `0`，包内 Desktop、Gateway、Resources/scripts、Info.plist 版本与 release staging 输出相互核对；旧 `Contents/Resources/proxy` 不存在。Gateway 在全新的空 HOME 中执行 `codex-auth status` 返回 exit `0` 和 `reason=state_missing`，前后没有生成状态文件。

本轮没有启动 `.app` / Desktop、Gateway service 或 Science；没有读取 Keychain、OAuth/token、API Key、账号数据库、SSH 私钥、真实 Science data-dir 或真实用户配置；没有接触、替换或停止 `/Applications/CSSwitch.app`。没有执行 notarization、Gatekeeper、DMG、tag、push、release 或真实服务请求。

本 evidence commit 的 pre-evidence docs-only baseline/parent 是 `434cfe30724c1088b326a2468174eedbdd00aee1`；该 evidence commit 是它的 docs-only descendant。它们校准或记录状态，但本 artifact 的 producing source 仍是 `c678b1b`，不得混同。

## Source 与构建

- artifact-producing source：`c678b1bee2475686ebbb8b8172c24112a37c6ce9`；构建前 worktree 为 clean、detached、non-shallow，且没有 ignored build residue。
- source gate：允许环境 run `fc4630f41a7930244720d4de46019f82`，aggregate `PASS`、runner exit `0`、15/15 suites；首次受托管 sandbox 限制的同 SHA run 仍仅是 `ENV-BLOCKED`。
- build command：

  ```bash
  DEV_HOME="$HOME"
  (
    cd desktop
    PATH="$DEV_HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH" \
      npm run tauri build -- --features acceptance-build --config ../test/tauri.real-machine.conf.json --bundles app
  )
  ```

- build exit：`0`；Tauri CLI `2.11.4`；Rust release completed in `1m 55s` with 14 existing dead-code warnings and no error.
- artifact path：`/private/tmp/csswitch-stage2-artifact-build-20260822/desktop/src-tauri/target/release/bundle/macos/CSSwitch Test.app`。

构建 worktree 的 lockfile 将 `@tauri-apps/cli-darwin-arm64` 固定为 `2.11.4`，但 npm 的 optional-dependency 安装在该网络环境中未落下本机 optional package。为不修改 source 或 lockfile，本轮只在该 clean worktree 的 ignored `node_modules` 中解压已完整下载并校验的同名同版本 tarball；其 SHA-256 是 `56e0849077f069779f4da7dc60674b317f6312a1825864a367e190698078b63a`，`tar -tzf` 通过，package name/version 为 `@tauri-apps/cli-darwin-arm64` / `2.11.4`，native binding 为 Mach-O arm64，随后 `tauri --version` 返回 `tauri-cli 2.11.4`。该 workaround 不改 Git tracked source、lockfile 或 artifact-producing source identity。

## Artifact identity

| Identity | Exact value | 判定 |
|---|---|---|
| Bundle ID / short version / bundle version | `com.csswitch.test` / `0.8.4` / `0.8.4` | `PASS` |
| App size | `32976896` bytes | `PASS` |
| Desktop SHA-256 | `204d174f97f45bce9d3be33027f44b7a1c8d40218d5a85bc3035a038d006d712` | `PASS` |
| Gateway SHA-256 | `d012958d5f24a193b1ed46fa2bbcbea4cfd05f028527b6fed6c375fb0e7f32a2` | `PASS` |
| Resource-file tree SHA-256 | `f87636184bb540f15aabc7cd9ebaa28f26275764710b4fefa120a90548e0dde6` | `PASS` |
| Bundle-file tree SHA-256 | `d98bf1151ff2be9b8e6438584cf608dc9eec4620b5f5ea993bf0c97f94df89b6` | `PASS` |
| Desktop / Gateway architecture | Mach-O arm64 / Mach-O arm64 | `PASS` |
| Desktop / Gateway permissions | `-rwxr-xr-x` / `-rwxr-xr-x` | `PASS` |
| Package source / Tauri config version | `0.8.4` / `0.8.4` | `PASS` |

`desktop/src-tauri/target/release/csswitch-gateway` 与 packaged Gateway 的 SHA-256 相同，逐字节比较通过。`doctor.sh`、`launch-virtual-sandbox.sh`、`ssh-bridge/ssh`、`stop-science-sandbox.sh` 与 `verify-proxy.sh` 均与同次 release staging resources 逐字节一致；`Contents/Resources/scripts` 存在，`Contents/Resources/proxy` 不存在。Desktop 与 Gateway 均含 `.csswitch-acceptance` compiled marker，Gateway 未链接 macOS `Security.framework`。

## Empty-HOME Gateway status

只运行了 packaged Gateway 的下列只读 status，而没有启动 Desktop 或服务：

```bash
env HOME=<fresh-empty-private-tmp-home> \
  'CSSwitch Test.app/Contents/MacOS/csswitch-gateway' codex-auth status
```

结果为 exit `0`：

```json
{"schema_version":3,"ok":true,"command":"status","status":{"authenticated":false,"reason":"state_missing","account_hash":null,"expiry_state":"missing","expires_at":null,"auth_epoch":null,"auth_generation":0}}
```

运行前后 fresh HOME entries 都是 `0`，因此 `PASS(scope=isolated-gateway-status)`。本命令没有调用 Keychain；本轮也没有做系统级调用追踪，故“未观察到 Keychain 访问”不能外推为任何其他 runtime 路径的 Keychain 证明。

## 分层边界

| 层 | 结果 | 边界 |
|---|---|---|
| Production source | `PASS` | 仅为 exact `c678b1b` 的 15/15 source gate |
| Exact artifact | `PASS` | 仅为本文 hash、packaging 与 compiled Acceptance identity |
| Isolated Gateway status | `PASS` | 仅为 fresh empty HOME 的 packaged `codex-auth status` |
| Temporary / installed runtime | `NOT-RUN` | 未启动任何 App、Gateway service 或 Science；installed App 未触碰 |
| Live Provider / Science / SSH / account | `NOT-RUN` | 无真实凭据、账号或服务请求 |
| Signing / notarization / Gatekeeper | `NOT-RUN` | 未建立任何分发签名/公证/Gatekeeper PASS；独立观察仅见 linker ad-hoc、TeamIdentifier unset，且 `codesign --verify --deep --strict` 未通过，不能写成 signing PASS |
| Public release | `NOT-RUN` | 未创建 DMG、tag 或 Release |

这个 exact artifact PASS 不授权或证明 isolated-live、authorized-live、installed runtime、签名或 release。下一层若继续，只能以这个 artifact 在新的 guard-managed isolated environment 中运行 normal production entry，并另行记录结果。
