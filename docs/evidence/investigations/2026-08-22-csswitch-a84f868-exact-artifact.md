# 2026-08-22 CSSwitch `a84f868` exact Acceptance artifact

状态：日期化证据；`PASS(scope=exact-artifact)`、`PASS(scope=isolated-gateway-status)`

适用范围：由 `a84f868c7f379959743c114f0b89e61703e8cff8` 在干净 detached worktree 中新构建的 macOS `CSSwitch Test.app` 0.8.4。本文不覆盖 Desktop 启动、Gateway service、Science、Provider、SSH、installed App、签名、公证、Gatekeeper、DMG 或公开发布。

最后复核：2026-08-22（Asia/Taipei）

失效条件：artifact-producing source、Acceptance feature、Desktop/Gateway/resources、manifest/version 或本文所述空 HOME Gateway status 任一改变时，本结论不得外推。

## 结论

`a84f868` 的允许环境 `GATE-SOURCE` 已为 `PASS`，并已有 immutable source-candidate record；在用户授权的本轮中，它生成了新的 Acceptance artifact。构建退出 `0`，包内 Desktop、Gateway、Resources/scripts、Info.plist 版本与 release staging 输出相互核对；旧 `Contents/Resources/proxy` 不存在。两个彼此独立的全新空 HOME 均只以 packaged Gateway 执行 `codex-auth status`，得到 exit `0`、`reason=state_missing`，且前后没有生成状态文件。

本轮没有启动 `.app` / Desktop、Gateway service 或 Science；没有读取 Keychain、OAuth/token、API Key、账号数据库、SSH 私钥、真实 Science data-dir 或真实用户配置；没有接触、替换或停止 `/Applications/CSSwitch.app`。没有执行 notarization、Gatekeeper、DMG、tag、push、release 或真实服务请求。

immutable record 的 publication 是 `f0ed7ce562c04b321f43f2f4e448705ce18857d7`。它记录并发布 source evidence，但不是 artifact-producing source；本 artifact 的唯一 producing source 仍是 `a84f868`。历史 `c678b1b` 的 Desktop-entry 或 artifact evidence 不继承给本 artifact。

## Source 与构建

- artifact-producing source：`a84f868c7f379959743c114f0b89e61703e8cff8`；构建前 worktree 为 clean、detached、non-shallow，且没有 ignored build residue。
- source gate：允许环境 root `/private/tmp/csg.4d8jOr`、run `baf12ea37b84c0ddc8d08dc4cff226ec`，aggregate `PASS`、runner exit `0`、16/16 suites。completion seal / evidence manifest / run manifest / source snapshot manifest SHA-256 分别为 `b69b0c2992a7b1963c8034d44918a01f998234f1245e233777725e274a933b20`、`c5c48ab63dd74592184381dc7fc5bd045dccd36e896c3298c7d920d9afb6324a`、`a4feae4a9c3ec6bd0dccfe006f940e667b31ee42c53b2838982d613fad53ee56`、`7c3f016bd8b24fa64b878d66bd37cf2949f12463d544436af89804f4d95393ec`。
- immutable record：`quality/source-candidates/a84f868c7f379959743c114f0b89e61703e8cff8.json`，文件 SHA-256 `655f271d0d2932f0d31e84422334027d7e8ae778917ade439da2b85472cffe2c`；publication `f0ed7ce` 只发布 record，不改变 producing source identity。
- build worktree：`/private/tmp/csswitch-p4-artifact-build-20260822`；构建前先固定 `DEV_HOME`，随后运行：

  ```bash
  (
    cd desktop
    PATH="$DEV_HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH" \
      CARGO_BUILD_JOBS=1 \
      npm run tauri build -- --features acceptance-build \
        --config ../test/tauri.real-machine.conf.json --bundles app
  )
  ```

- build exit：`0`；Tauri CLI `2.11.4`；Rust release completed in `7m 06s` with 15 existing dead-code warnings and no error。
- artifact path：`/private/tmp/csswitch-p4-artifact-build-20260822/desktop/src-tauri/target/release/bundle/macos/CSSwitch Test.app`。

lockfile 将 `@tauri-apps/cli-darwin-arm64` 固定为 `2.11.4`，但 `npm ci` 在该环境遗漏本机 optional package。为不修改 source 或 lockfile，本轮仅将完整下载、校验后的同名同版本 tarball 解压到该 clean worktree 的 ignored `node_modules`；tarball SHA-256 是 `56e0849077f069779f4da7dc60674b317f6312a1825864a367e190698078b63a`，archive 名称/version 为 `@tauri-apps/cli-darwin-arm64` / `2.11.4`，native binding 为 Mach-O arm64，随后 `tauri --version` 返回 `tauri-cli 2.11.4`。该 workaround 不改 Git tracked source、lockfile 或 artifact-producing source identity。

## Artifact identity

| Identity | Exact value | 判定 |
|---|---|---|
| Bundle ID / short version / bundle version | `com.csswitch.test` / `0.8.4` / `0.8.4` | `PASS` |
| Logical file size / allocated size | `33112638` / `33132544` bytes | `PASS` |
| Desktop SHA-256 | `e8d00bccda828a48bbdcfac545e8b31ece29a7bd7ac0f04573933d1e23c1e5d6` | `PASS` |
| Gateway / same-build staged Gateway SHA-256 | `dde4e163a74cc05ddfce50962d2c9fc62d99878c4d7745ff84bc336a6dc88466` | `PASS` |
| Resource-file tree SHA-256 | `a46fc798104d2fa4e96a2974d3d96ffcc178cd7fb35c8d7fbc85a1e7dbc7cfdc` | `PASS` |
| Bundle-file tree SHA-256 | `b9038b0b85762d14e33a9738a4bea730ccea3f2538e61244d47d8d55c00488c2` | `PASS` |
| Desktop / Gateway architecture | Mach-O arm64 / Mach-O arm64 | `PASS` |
| Desktop / Gateway permissions | executable / executable | `PASS` |

packaged Gateway 与 `desktop/src-tauri/target/release/csswitch-gateway` 逐字节相同。五个 resources（`doctor.sh`、`launch-virtual-sandbox.sh`、`ssh-bridge/ssh`、`stop-science-sandbox.sh`、`verify-proxy.sh`）均与 exact source 及同次 release staging 逐字节一致；`Contents/Resources/scripts` 存在，`Contents/Resources/proxy` 不存在。Desktop 与 Gateway 均含 `.csswitch-acceptance` compiled marker，Gateway 未链接 macOS `Security.framework`。

## Empty-HOME Gateway status

两次检查都只运行 packaged Gateway 的下列只读 status，没有启动 Desktop、Gateway service 或 Science：

```bash
env HOME=<fresh-empty-private-tmp-home> \
  'CSSwitch Test.app/Contents/MacOS/csswitch-gateway' codex-auth status
```

Terra 的 fresh mode-0700 HOME（artifact observation root 下）以及主 Agent 独立的 `/private/tmp/csswitch-p4-gateway-home.xWhxFX` 都在运行前后为 0 entries。两次结果均为 exit `0`，其脱敏状态均为：

```json
{"schema_version":3,"ok":true,"command":"status","status":{"authenticated":false,"reason":"state_missing","account_hash":null,"expiry_state":"missing","expires_at":null,"auth_epoch":null,"auth_generation":0}}
```

因此 `PASS(scope=isolated-gateway-status)`。本命令没有启动服务；本轮也没有做系统级调用追踪，故不能把此处的空状态扩展为其他 runtime 路径的 Keychain 结论。

## Unsigned observation

只执行了 read-only `codesign -dvv` 和 `codesign --verify --deep --strict` 观察：前者 exit `0`，显示 linker ad-hoc 与 `TeamIdentifier=not set`；后者 exit `1`。这些观察证明本 artifact 没有建立可声明的分发签名；它们不是 signing、notarization 或 Gatekeeper `PASS`。

脱敏的 static identity 和 empty-HOME status 观察保存在 mode-0700 root `/private/tmp/csswitch-p4-artifact-observation.nXGbXi`。

## 分层边界

| 层 | 结果 | 边界 |
|---|---|---|
| Production source / immutable record | `PASS` | 仅为 exact `a84f868` 的 16-suite source gate 与 record |
| Exact artifact | `PASS` | 仅为本文 hash、packaging 与 compiled Acceptance identity |
| Isolated Gateway status | `PASS` | 仅为两个 fresh empty HOME 的 packaged `codex-auth status` |
| Desktop entry、Gateway / Science full chain、Skill runtime | `NOT-RUN` | 没有启动 Desktop、Gateway service、Science 或 Skill runtime |
| Installed runtime | `NOT-RUN` | installed App 未触碰 |
| Live Provider / Science / SSH / account | `NOT-RUN` | 无真实凭据、账号或服务请求 |
| Signing / notarization / Gatekeeper | `NOT-RUN` | 仅有 unsigned observation，未建立分发结论 |
| Public release | `NOT-RUN` | 未创建 DMG、tag 或 Release |

这个 exact artifact `PASS` 不授权或证明 isolated-live、authorized-live、installed runtime、签名或 release。下游若继续，必须为本 artifact 取得新的 guard-managed isolated environment 证据。
