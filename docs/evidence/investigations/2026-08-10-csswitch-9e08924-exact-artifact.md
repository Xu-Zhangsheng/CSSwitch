# 2026-08-10 CSSwitch `9e08924` exact artifact 与 G1 binding

状态：日期化证据；`PASS(scope=exact-artifact)`、`PASS(scope=G1-binding)`

适用范围：`next@9e08924481c8f5edb181254332d94daba0cbe4b2`、由该 exact source 新构建的 `CSSwitch Test.app` 0.8.4、packaged Rust Gateway，以及只读固定的 `/Applications/Claude Science.app` 0.1.25 package/executable identity

最后复核：2026-08-10（Asia/Taipei）

失效条件：artifact-producing source、Desktop/Gateway/resource 内容、Acceptance feature、目标 Science package/executable identity 或 G1 validator 合同任一变化时，本结论不得外推。

## 结论

`9e08924` 是 comprehensive refactor closure 的最后一个 code-bearing production source candidate；后续 `f8ef373` 只提交 source evidence 文档，不改变被构建的源码身份。本轮在新的 clean detached worktree 由 `9e08924` 全新构建 Acceptance App，并把既有的 exact-source completion seal、CSSwitch bundle/Desktop/Gateway/resources、空 HOME Gateway status 与 Claude Science 0.1.25 package/executable identity 收敛到同一份 G1 receipt。production G1 validator 递归复核 receipt、source seal、15 个 suite、artifact record、bundle 与 Science package 后返回 `PASS`。

本轮没有启动 Desktop 或 Science，没有进入 isolated-live，也没有读取真实凭证、账号数据库、Keychain、SSH 私钥、真实 Science data-dir 或用户配置。未替换 `/Applications/CSSwitch.app`，没有构建 DMG，没有执行 Developer ID 签名、公证、Gatekeeper、tag、push、release 或公开发布。

## Source authority

- artifact-producing source：`9e08924481c8f5edb181254332d94daba0cbe4b2`；执行 worktree 为 clean detached，`git fsck --no-dangling` 通过。
- authoritative source-gate run：`078d462c81abcec146644c8096254069`。
- aggregate：15/15 suites、15/15 observations、`PASS`、runner exit `0`。
- completion seal SHA-256：`1c071a18701ac6e6a191c6dfc3cd1513d3465bce90d3567a6460cd46f36d0fe8`。
- source snapshot manifest SHA-256：`61dd5eb54b02695ff8664984c356b122c80cb3d864776992fafb88b2fc416005`。

## Build 与 artifact identity

依赖安装 `npm ci` exit `0`，随后执行：

```bash
CARGO_BUILD_JOBS=1 npm run tauri build -- \
  --features acceptance-build \
  --config ../test/tauri.real-machine.conf.json \
  --bundles app
```

single-job build exit `0`，目标为新的 detached worktree，未复用旧 Acceptance bundle。

| Identity | Exact value | 判定 |
|---|---|---|
| Bundle ID / version / arch | `com.csswitch.test` / `0.8.4` / `arm64` | `PASS` |
| Canonical bundle digest | `d77cb799f2063241041cc8d17bd57a3c72f49cc7b0da6899d63a52795df1ac64` | `PASS` |
| Desktop SHA-256 | `ada760cb9dcfdd9b2151d652ff744f300a914b3bef8c07ea85ce886994458a6b` | `PASS` |
| packaged Gateway SHA-256 | `bfa05512337329f52811c2d7c08081ed2249a34c6ed98dd7fe7d83313d9b3036` | `PASS` |
| artifact tree entries SHA-256 | `da87646db2fac2c25ba3c561c3e6a86f2a1522df4ccfb7520a244c7378b8fdbd` | `PASS` |
| Claude Science version / arch | `0.1.25` / `arm64` | `PASS(identity-only)` |
| Science package canonical digest | `371a10ecb2e4fc610ede1e3ab266c3dfc71d3894d9bcb9ce35515145d93c49ca` | `PASS(identity-only)` |
| Science executable SHA-256 | `63b0f57aa3b9588ba9e61433d27c78df788f8fe2c1b51842db107d6697e9c03f` | `PASS(identity-only)` |

复制前后的 CSSwitch bundle canonical digest 一致。包内 Desktop/Gateway 与同次 release 产物逐字节一致；`doctor.sh`、`launch-virtual-sandbox.sh`、`ssh-bridge/ssh`、`stop-science-sandbox.sh` 与 `verify-proxy.sh` 五个资源逐字节匹配 exact source，`Contents/Resources/proxy` 不存在。Desktop、Gateway 与 Science executable 均为 Mach-O arm64。

本层只观察到 linker ad-hoc signature、无 Team ID、无 sealed resources；它不构成 Developer ID、notarization、Gatekeeper 或 release-ready signing PASS。

## Gateway empty-HOME 与 G1 closure

packaged Gateway 在新的空 HOME 中执行 `codex-auth status`，exit `0`，v3 response envelope 返回 `authenticated=false`、`reason=state_missing`；运行前后 HOME 均为 0 个条目，没有生成状态文件。

- artifact observation SHA-256：`ffbd19b92cbd1a024617ae9cfed4da36cb21c71d80bb82bcb236474c98ac7227`。
- artifact tree manifest SHA-256：`883fb1cefcec3f8659f7f78ed8cab8cce5121af776bfef149aef2edb42fd914b`。
- empty-HOME observation SHA-256：`580382d97231990516b146383fca33e36bfb53b3a7229769ad756a88462d01bc`。
- identity closure SHA-256：`c965497cf526b5eb197dd742d6cc7fffda969f635f0e52f314ae1a4a5d124a72`。
- G1 binding receipt SHA-256：`39aa3dc5866807140d42409ab8eea2f6625fa4d9f7212f06482dcc0db0c9b762`。
- `hashes.sha256` SHA-256：`93a9f391b0e62c179f6a44c83f2a7a2a63bc7c11c29f970fdb6a6d359533890a`。
- 当前可复核 artifact / G1 root：`/private/tmp/g1.9e08924.91acQY/`。

`hashes.sha256` 对五份 G1 evidence 全部返回 `OK`；production G1 validator 重新计算 CSSwitch bundle、Desktop、Gateway、Science package/executable 与 source seal 后返回 `current_g1_result=PASS`、`suite_count=15`、`runner_exit=0`。

证据生成器第一次仍按旧的裸 status object 解析当前 v3 envelope，因而在写出 PASS receipt 前 fail closed；核对真实输出与空 HOME 后，只修正临时生成器的 envelope 解析，再从头生成上述 closure。该诊断不是产品失败，也没有被计入 PASS 运行。

## 不能外推

`PASS(scope=exact-artifact)` / `PASS(scope=G1-binding)` 只证明同源构建与 identity closure。以下层仍为 `NOT-RUN`：`9e08924` isolated-live、真实 provider/账号、Skill/MCP、SSH、installed App、Developer ID 签名、公证、Gatekeeper、DMG、tag、public release。旧 `06b630b` artifact 的 `B-RUNTIME-01`、`B-CORE-01` 与 `B-CONTEXT-01` 不继承给本 artifact。
