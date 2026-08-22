# A0 冻结基线 Artifact 验收

日期：2026-08-06（Asia/Taipei）

证据层：built artifact。本文只证明下述 exact source 构建出的隔离 Acceptance `.app`；不证明 installed/live、真实 provider / Science / SSH、签名、公证、Gatekeeper 或 release。

## 目标与结论

A0 冻结基线为 `9cf75d19e7853b91b2f9a7c85afbd66747cb4fa3`。构建前后 `next` 均为 clean exact `HEAD`，用户指定的输入状态为 `SOURCE-GREEN`；本轮没有重跑或扩写 source gate。

从该 checkout 冷构建的 `CSSwitch Test.app` 满足 A0 built-artifact 合同，结论为 **ARTIFACT-GREEN**：

- 构建只生成独立 Acceptance `.app`，没有 DMG、安装、tag 或 release；
- Desktop 与 Gateway 均为 arm64 Mach-O、存在且可执行；Gateway 由同一次 Desktop build 的私有 nested target 按相同 `acceptance-build` feature 构建，staged 与 packaged hash 一致；
- bundle identifier 为 `com.csswitch.test`，版本为 `0.8.4`，两项二进制均包含编译期 `.csswitch-acceptance` root；
- bundle 只含 Desktop、Gateway、icon 与 5 个 allowlist script，共 9 个普通文件；无 `Resources/proxy` 或 Python 文件；5 个 script 与 exact checkout 的 SHA-256 逐项一致；
- packaged Gateway 在全新隔离 `HOME` 上执行 `codex-auth status`，退出码为 `0`，返回 `authenticated=false`、`reason=state_missing`、`auth_generation=0`，且没有生成状态文件。

## 构建身份

构建环境为 macOS arm64、Apple SDK 26.5、Node.js 22.14.0、npm 10.9.2、cargo 1.96.1、rustc 1.96.1。使用新的 `/private/tmp` target root 执行：

```bash
cd desktop
CARGO_TARGET_DIR=<fresh-private-target> \
PATH=<stable-aarch64-toolchain>:$PATH \
  npm run tauri build -- \
    --features acceptance-build \
    --config ../test/tauri.real-machine.conf.json \
    --bundles app
```

runner exit 为 `0`；构建完成时间为 `2026-08-06T17:29:45+0800`。目标 artifact 是本轮临时路径中的 `CSSwitch Test.app`，没有复制到 `/Applications` 或其他 installed 位置。

## Artifact 身份与内容

| 项目 | 值 |
|---|---|
| exact source | `9cf75d19e7853b91b2f9a7c85afbd66747cb4fa3` |
| bundle identifier / version | `com.csswitch.test` / `0.8.4` |
| architecture | Desktop `arm64`；Gateway `arm64` |
| file count / total file bytes | 9 / 29,811,162 |
| tree manifest SHA-256 | `6c1f7110bac270aa726e9b0a2c9b02df8bc985827acd29f1d236b3c81d317786` |
| Info.plist SHA-256 | `62b8b643cf036e50d9169d3ab0b9f7d4a487a48f18dc9eeb2671c4e810e34c9a` |
| Desktop SHA-256 | `6755ad2b68a0ff076324be4ba6509fd4ae15a152e0d62bbc4d486fcb97e0e2ea` |
| Gateway SHA-256 | `a7dd8469c312c77ce1ee9c4d37aff40dc69b8c89b35b920a68bc98df98ff6a5b` |

tree manifest 按 bundle 内相对路径排序，并对每个普通文件的 SHA-256 清单再次取 SHA-256；它只绑定本轮未签名 Acceptance bundle，不是 DMG、installed app 或公开附件身份。

## 隔离状态验证

实际 Gateway CLI 的状态入口是 `codex-auth status`。本轮使用全新 `/private/tmp` 外层 `HOME` 执行：

```bash
HOME=<fresh-isolated-home> \
  <artifact>/Contents/MacOS/csswitch-gateway codex-auth status
```

脱敏结果为：

```json
{"schema_version":3,"ok":true,"command":"status","status":{"authenticated":false,"reason":"state_missing","account_hash":null,"expiry_state":"missing","expires_at":null,"auth_epoch":null,"auth_generation":0}}
```

命令退出码为 `0`；隔离 root 中只有预先创建的空 `home/`，没有 OAuth、thinking、config、Keychain 或其他状态文件。本轮没有读取或修改真实 `~/.csswitch`、`~/.csswitch-acceptance`、`~/.claude-science`、凭证或账号数据。

## 签名观察与停止点

原始 Tauri Acceptance bundle 的 Desktop 是 linker-signed ad-hoc；`codesign --verify --deep --strict` 返回 `1`，报告没有 sealed resources。A0 的 `ARTIFACT-GREEN` 只覆盖 built-artifact identity、内容与隔离 sidecar 状态，不把该观察升级成 signing PASS，也不在本阶段重签名。

A0 到此停止。Desktop UI 启动、临时安装或 `/Applications` installed runtime、真实 Science / provider / SSH、Developer ID、notarization、stapling、Gatekeeper、DMG、tag、push 与公开 release 均为 **NOT-RUN / 未建立**；不得由本文外推，也不授权继续 runtime 重构或进入发布。
