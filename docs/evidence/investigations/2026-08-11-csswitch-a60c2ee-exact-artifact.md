# 2026-08-11 CSSwitch `a60c2ee` exact artifact 与 G1 binding

状态：日期化证据；`PASS(scope=exact-artifact)`、`PASS(scope=G1-binding)`

适用范围：`next@a60c2ee656429903f1fd8f398dc6ad8194aa9346`、由该 exact source 新构建的 `CSSwitch Test.app` 0.8.4、packaged Rust Gateway，以及只读固定的 `/Applications/Claude Science.app` 0.1.25 package/executable identity

最后复核：2026-08-11（Asia/Taipei）

失效条件：artifact-producing source、Desktop/Gateway/resource 内容、Acceptance feature、目标 Science package/executable identity 或 G1 validator 合同任一变化时，本结论不得外推。

## 结论

旧 `9e08924` artifact 的 `B-SKILL-01` 验收暴露：host Gateway 经 `env_clear` 后没有收到 acceptance-only GitHub loopback fixture，因而出现公共 GitHub 尝试。`48cfffd` 将经过严格 loopback 校验的 fixture 只注入 acceptance host Gateway；`a60c2ee` 又同步了新增测试的 source-gate identity inventory。两次实现/治理变更均经 fresh clean-context review 取得 `BLOCK/HIGH/MEDIUM/LOW=0/0/0/0`、`PASS`。

本轮在新的 clean detached worktree 由 `a60c2ee` 全新构建 Acceptance App，并把 exact-source completion seal、CSSwitch bundle/Desktop/Gateway/resources、空 HOME Gateway status 与 Claude Science 0.1.25 package/executable identity 收敛到同一份 G1 receipt。production G1 validator 递归复核 receipt、source seal、15 个 suite、artifact record、bundle 与 Science package 后返回 `PASS`；fresh clean-context exact-artifact 终审为 `BLOCK/HIGH/MEDIUM/LOW=0/0/0/0`、`PASS`。

本轮没有启动 Desktop 或 Science，没有进入 isolated-live，也没有读取真实凭证、账号数据库、Keychain、SSH 私钥、真实 Science data-dir 或用户配置。未替换 `/Applications/CSSwitch.app`，没有构建 DMG，没有执行 Developer ID 签名、公证、Gatekeeper、tag、push、release 或公开发布。

## Source authority

- artifact-producing source：`a60c2ee656429903f1fd8f398dc6ad8194aa9346`；执行 worktree 为 clean detached、non-shallow。
- authoritative source-gate run：`6e5124c43c1ceadac3bd54f6408e2141`。
- aggregate：15/15 suites、15/15 observations、`PASS`、runner exit `0`。
- completion seal SHA-256：`fe8d24939dd8b950c95984383645aa8de87dae4956c5c34c9a533b310560d86e`。
- source snapshot manifest SHA-256：`d339dda8632ec2a3d99c305a2b5b232bb2af45e82ec7200918343f96400175a7`。
- run manifest SHA-256：`64b3f7e1b7ee2db5e8320dc5388380cdff80a5306e2f01b9624817990855b1f2`。
- evidence manifest SHA-256：`8c3801b37b0bf73485415ef94a839097f5f431efa34964791176ed19033dc1f3`。

`48cfffd` 的第一次完整 source-gate run `b34b27f08e3992a7e445468e5bd349f4` 是 sealed `FAIL`（13/15）：Rust 实际 560 passed、0 failed、41 approved ignored，Python Skill boundary 16/16；两项失败只因新增/改名测试尚未进入 identity inventory。它不是 product-test PASS。同步 inventory 后，`a60c2ee` 的上述全新 run 才是本 artifact 的 authoritative source PASS。

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
| Canonical bundle digest | `52cd48c06b2d2bffdd6d8e85d464be063e4604c71961a5cc8459879938a673c9` | `PASS` |
| Desktop SHA-256 | `77b4ebefe6e658c4f8f90e2b5177db5398be75eacf3b3f0d3364269eba5e9be8` | `PASS` |
| packaged Gateway SHA-256 | `610c0206973ec0281b6e34e171e4a6fb9899b31050642a073c4f0d1ca64887ab` | `PASS` |
| artifact tree entries SHA-256 | `370b0355b9714b2f3ed6e665e29f5bb0bc4e9c5368ba29c7417e7418868180ca` | `PASS` |
| Claude Science version / arch | `0.1.25` / `arm64` | `PASS(identity-only)` |
| Science package canonical digest | `371a10ecb2e4fc610ede1e3ab266c3dfc71d3894d9bcb9ce35515145d93c49ca` | `PASS(identity-only)` |
| Science executable SHA-256 | `63b0f57aa3b9588ba9e61433d27c78df788f8fe2c1b51842db107d6697e9c03f` | `PASS(identity-only)` |

复制前后的 CSSwitch bundle canonical digest 一致。包内 Desktop/Gateway 与同次 release 产物逐字节一致；`doctor.sh`、`launch-virtual-sandbox.sh`、`ssh-bridge/ssh`、`stop-science-sandbox.sh` 与 `verify-proxy.sh` 五个资源逐字节匹配 exact source，`Contents/Resources/proxy` 不存在。Desktop、Gateway 与 Science executable 均为 Mach-O arm64。

本层只观察到 linker ad-hoc signature、无 Team ID、无 sealed resources；strict bundle `codesign --verify` 未通过。它不构成 Developer ID、notarization、Gatekeeper 或 release-ready signing PASS。

## Gateway empty-HOME 与 G1 closure

packaged Gateway 在新的空 HOME 中执行 `codex-auth status`，exit `0`，v3 response envelope 返回 `authenticated=false`、`reason=state_missing`；运行前后 HOME 均为空，没有生成状态文件。

- artifact observation SHA-256：`336d87c3010b6cb98bfbdc4490974a2e871567fe4ec1ab5e6cab8bf74af02fb4`。
- artifact tree manifest SHA-256：`e4c9bc0ef32befbd5a56de98911d97a827a092181b64ec1c942c775c1b801afa`。
- empty-HOME observation SHA-256：`1cfdea1b5854e02a44d10cfaeb5762505163606347936bb9a025f25db6bd4f86`。
- identity closure SHA-256：`4d71f934d0e421586e23fa2cba93f8c9c9fd3678941d6c52c22bd205914ac7e6`。
- G1 binding receipt SHA-256：`b01cf7a2066576dfc5da1a59eada2faad9f586100f1b27d013cad9b4d420ddbb`。
- `hashes.sha256` SHA-256：`e96b4962bdac16e3a654820c8cfe326f665282968addba36f44b358bf102db62`。
- 当前可复核 artifact / G1 root：`/private/tmp/g1.a60c2ee.q4uEQl/`。

`hashes.sha256` 对五份 G1 evidence 全部返回 `OK`；production G1 validator 重新计算 CSSwitch bundle、Desktop、Gateway、Science package/executable 与 source seal 后返回 `current_g1_result=PASS`、`suite_count=15`、`runner_exit=0`。

## 不能外推

`PASS(scope=exact-artifact)` / `PASS(scope=G1-binding)` 只证明同源构建与 identity closure。当前 `a60c2ee` tuple 的 `B-RUNTIME-01`、`B-CORE-01`、`B-CONTEXT-01`、Provider、Skill/MCP、SSH、installed App、升级/rollback、Developer ID 签名、公证、Gatekeeper、DMG、tag 与 public release 均为 `NOT-RUN`。旧 `9e08924` artifact 的上述 isolated-live PASS/INCONCLUSIVE 只保留为历史证据，不能继承给本 artifact。
