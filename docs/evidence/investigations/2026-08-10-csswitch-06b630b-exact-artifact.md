# 2026-08-10 CSSwitch `06b630b` exact artifact 与 G1 binding

状态：日期化证据；`PASS(scope=exact-artifact)`、`PASS(scope=G1-binding)`

适用范围：`next@06b630bb3e3fb0d63425bf48d1ffc2d3613fd992`、由该 exact source 新构建的 `CSSwitch Test.app` 0.8.4、packaged Rust Gateway，以及只读固定的 `/Applications/Claude Science.app` 0.1.25 package/executable identity

最后复核：2026-08-10（Asia/Taipei）

失效条件：artifact-producing source、Desktop/Gateway/resource 内容、Acceptance feature、目标 Science package/executable identity 或 G1 validator 合同任一变化时，本结论不得外推。

## 结论

`06b630b` 的 production source 已由 fresh clean-context review 与 canonical source gate 闭合；本轮从新的 clean、non-shallow detached clone 全新构建 Acceptance App，并把 source completion seal、CSSwitch bundle/Desktop/Gateway/resources、空 HOME Gateway status 与 Claude Science 0.1.25 package/executable identity 收敛到同一份 G1 receipt。production G1 validator 递归复核 receipt、source seal、15 个 suite、artifact record、bundle 与 Science package 后返回 `PASS`。

本轮没有启动 Desktop 或 Science，没有进入 isolated-live，也没有读取真实凭证、账号数据库、Keychain、SSH、真实 Science data-dir 或用户配置。未替换 `/Applications/CSSwitch.app`，没有构建 DMG，没有执行签名、公证、Gatekeeper、tag、push、release 或公开发布。

## Source authority

- artifact-producing source：`06b630bb3e3fb0d63425bf48d1ffc2d3613fd992`；两个执行 clone 均 clean、non-shallow、detached，`git fsck` 通过。
- authoritative source-gate run：`8728828e41ee0dc8ab578856995fa530`。
- aggregate：15/15 suites、15/15 observations、`PASS`、runner exit `0`。
- completion seal SHA-256：`e1ef35f06d0f88a8cbf6fa4ea8b12c129afd10b55f85f89d2074b0d23d72b455`。
- source snapshot manifest SHA-256：`c9aad6ad9a6ca3674845513df694384b79f7cb6df48c8c1b380e071a5c1e3078`。

首次在受限执行沙箱中运行同一 gate 得到 sealed `FAIL`：run
`d1e81f780fd7ef890053351267ec2282`，runner exit `12`，completion seal SHA-256
`fdcf987ce07b623d03f0274355f75c95757d4a059e960ea824b6eca34bac6efd`。该环境阻断 loopback bind、
端口 reservation 与进程生命周期 fixture，造成 5 个 suite `FAIL`、1 个 suite `BLOCKED`；它不构成
source 或 G1 PASS，也没有被覆盖。最终在允许这些 fixture 的隔离执行环境中，以未修改的 canonical
命令重新运行并取得上述 PASS seal。

## Build 与 artifact identity

最终构建命令为：

```bash
CARGO_BUILD_JOBS=1 npm run tauri build -- \
  --features acceptance-build \
  --config ../test/tauri.real-machine.conf.json \
  --bundles app
```

最终 single-job build exit `0`，目标为新的临时 clone，未复用旧 Acceptance bundle。

| Identity | Exact value | 判定 |
|---|---|---|
| Bundle ID / version / arch | `com.csswitch.test` / `0.8.4` / `arm64` | `PASS` |
| Canonical bundle digest | `634c13f2597c10cbbf75a7cac8d1af135523eccff2cb86373824695cccb32e1a` | `PASS` |
| Desktop SHA-256 | `cf0e84e6b33b761767394b6d5f3579e310bec5c07de8015cd79f2d407d9d5274` | `PASS` |
| packaged Gateway SHA-256 | `ed4dae8ec8139c4828dd0915d1594d69001e7b504c582a710e905707ef9d03d1` | `PASS` |
| artifact tree entries SHA-256 | `81afbd6cadf9e45e46a09663bc54a40a9bb71f80d5588982947c71c1e84e2aac` | `PASS` |
| Claude Science version / arch | `0.1.25` / `arm64` | `PASS(identity-only)` |
| Science package canonical digest | `371a10ecb2e4fc610ede1e3ab266c3dfc71d3894d9bcb9ce35515145d93c49ca` | `PASS(identity-only)` |
| Science executable SHA-256 | `63b0f57aa3b9588ba9e61433d27c78df788f8fe2c1b51842db107d6697e9c03f` | `PASS(identity-only)` |

复制前后的 CSSwitch bundle canonical digest 一致。包内 Desktop/Gateway 与同次 release 产物逐字节一致；
`doctor.sh`、`launch-virtual-sandbox.sh`、`ssh-bridge/ssh`、`stop-science-sandbox.sh` 与
`verify-proxy.sh` 五个资源逐字节匹配 exact source，`Contents/Resources/proxy` 不存在。Desktop、Gateway
与 Science executable 都是 Mach-O arm64。

## Gateway empty-HOME 与 G1 closure

packaged Gateway 在新的空 HOME 中执行 `codex-auth status`，exit `0`，返回
`authenticated=false`、`reason=state_missing`；运行前后 HOME 均为 0 个条目，没有生成状态文件。

- artifact observation SHA-256：`3316a67194d5c50b38a7fa6b26cb2993401a2435cff99c5879c76ed81ccd06e7`。
- artifact tree manifest SHA-256：`fb14381d961ba6ffa330f69571d5828686f715bf41afd21f78c76b7331634152`。
- empty-HOME observation SHA-256：`cbada3e58a74c8a669acd0d5b243a7d13d9cf62cda77bdef9db8f74017a31018`。
- identity closure SHA-256：`68d236897662e2f35cb7b5940c996e28dd52bf8d98f4b660a852577184745a4b`。
- G1 binding receipt SHA-256：`f6490b1765d982c4453571676cb3561f6f1c3a20a9af3850d30f8e405e795573`。
- 当前可复核 artifact / G1 root：`/private/tmp/g1.06b630b.Yv5hDZ/`。

`hashes.sha256` 对五份 G1 evidence 全部返回 `OK`；production G1 validator 重新计算 CSSwitch bundle、
Desktop、Gateway、Science package/executable 与 source seal 后返回
`current_g1_result=PASS`、`suite_count=15`、`runner_exit=0`。

## 不能外推

`PASS(scope=exact-artifact)` / `PASS(scope=G1-binding)` 只证明同源构建与 identity closure。以下层仍为
`NOT-RUN`：`06b630b` isolated-live、真实 provider/账号、Skill/MCP、SSH、installed App、签名、公证、
Gatekeeper、DMG、tag、public release。历史 `9cc0d15` artifact 的 `B-RUNTIME-01`、`B-CORE-01` 与
`B-CONTEXT-01` 不继承给本 artifact。
