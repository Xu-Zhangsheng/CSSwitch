# 2026-08-11 RM-46 新 Provider 配置 UX 本地验收

状态：日期化证据；`PASS(scope=RM-46 exact-artifact local-mock UI)`

适用范围：`next@d74221e2948f32cd67db0aed8920af6122d0c798`、由该 exact source 新构建并只在临时目录运行的 `CSSwitch RM46 Acceptance.app` 0.8.4，以及 OpenCode Go 双 transport、Grok、Gemini 的本地 loopback mock 配置路径

最后复核：2026-08-11（Asia/Taipei）

失效条件：artifact-producing source、`acceptance-build` Provider loopback seam、Provider 模板、模型目录/协议过滤、profile 保存事务、目标 bundle 或 RM-46 合同任一变化时，本结论不得外推。

## 结论

RM-46 在 `d74221e` exact artifact 上取得限定 `PASS`：OpenCode Go OpenAI Chat、OpenCode Go Anthropic Messages、Grok 和 Gemini 均从产品“新建配置”UI 完成 scratch discovery、显式选择或手填、创建、再次编辑及保存前 scratch 校验。四次 discovery 均未写正式配置；最终正式配置只含用户明确填写的四个模型。OpenCode 已知模型按 transport 过滤，两个上游请求分别收到裸 `kimi-k3` 与 `minimax-m3`，没有 `opencode-go/` 前缀。

本轮只使用假 API key、动态 `127.0.0.1` mock、全新隔离 HOME 和唯一 Acceptance bundle ID。没有读取真实凭证、Keychain、账号数据库、SSH 私钥、真实 Science data-dir 或正式 CSSwitch 配置；没有请求真实 Provider，也没有安装 App、构建 DMG、执行 Developer ID 签名、公证、tag、push、release 或公开发布。

## Source authority

- 产品/测试实现提交：`51575dbcf94b0fa1c43f77cad1a3f7b5b092ba01`。`acceptance-build` 新增只接受显式 loopback `http(s)` URL 的 scratch Provider base override；production build 不读取该环境变量。override 只投影到 `openai-custom`、`openai-responses` 或 `relay` scratch child，并同时关闭 raw `CONNECT` 旁路。
- 容量合同同步提交：`d74221e2948f32cd67db0aed8920af6122d0c798`。它只把两处 Rust Desktop 测试清单硬编码计数从 `601` 同步为 `603`，不改变 4 MiB observation 上限或产品逻辑。
- `51575db` 的首次完整 source-gate run `42538afa80f24b4e7fbaab1476773dc7` 是 sealed `FAIL`（13/15 suites、runner exit `10`）；失败只来自上述两处陈旧 `601` 容量断言。completion seal SHA-256 为 `77be0ecf1dae5fd05172ccdef99c289a233798979929662358839ba17cc5bdd6`，不得改写为 PASS。
- authoritative source-gate run：`33e290ddc03a1a131c064cf4b5dcb7e1`，绑定 clean detached `d74221e`，15/15 suites 全部 `PASS`，runner exit `0`。
- completion seal SHA-256：`0b81438873477a24fc318017e5b8969850ffb1d65b3e842f277e3ccc802eed9b`。
- run manifest SHA-256：`0f401c8fef06aecf7c3f76eddb575ffdd5effcf5e5887bfcf667fa89e3cd50f3`。
- evidence manifest SHA-256：`7d73d8c902b77c46ee7a8fb7b84719a36cc7b5e8544a38bae86d88464b5d7a7e`。
- source snapshot manifest SHA-256：`07cfe0c07b8ebf6699044e0adf33b5c9ae67a01b89b541cf3a31f52751d4b946`。

## Build 与 artifact identity

在上述 clean detached worktree 中引用现有锁定 Node 依赖，以全新 Cargo target 执行：

```bash
CARGO_TARGET_DIR=/private/tmp/rm46-build-d74221e \
CARGO_BUILD_JOBS=1 npm run tauri build -- \
  --features acceptance-build \
  --config /private/tmp/rm46-tauri-d74221e.json \
  --bundles app
```

build exit `0`。临时配置只把产品名和 bundle ID 固定为 `CSSwitch RM46 Acceptance` / `com.csswitch.acceptance.rm46.d74221e`，窗口可见，版本仍为 `0.8.4`。产物没有复制到 `/Applications`。

| Identity | Exact value | 判定 |
|---|---|---|
| Source / arch | `d74221e2948f32cd67db0aed8920af6122d0c798` / `arm64` | `PASS` |
| Bundle ID / version | `com.csswitch.acceptance.rm46.d74221e` / `0.8.4` | `PASS` |
| 完整 ad-hoc 签名后的 canonical bundle digest | `a73d4f68720fe9635f4a58abeffb2378bd4c0bc41d0f5a809fa68b5290e63ed4` | `PASS(identity-only)` |
| 完整 ad-hoc 签名后的 Desktop SHA-256 | `877b565d5e803da47fb4e4af3f893746c45e26e3fb8dcdecfd484a6987685417` | `PASS(identity-only)` |
| 完整 ad-hoc 签名后的 packaged Gateway SHA-256 | `2877d4d9a8dcf2cd5e28b179f601d25e1a7178a730b12034edbe4674f5859ee9` | `PASS(identity-only)` |
| Info.plist SHA-256 | `44968058fa1cfcc55607822608c0d0074f27c5a9c100bb0c6bb94fe0abe18f7f` | `PASS(identity-only)` |

bundle 完整 ad-hoc 签名前，packaged 与同次 staged Gateway SHA-256 均为 `958976166e43c0fc6a6b70a5c35fb98eb385095e87f756bb12096838a5b501fe`；随后 `codesign --force --deep --sign -` 会改写包内 Mach-O，因此表中记录签名后的最终 artifact hash。最终 `codesign --verify --deep --strict` 通过，但 Team ID 为空，只能声明 ad-hoc 完整性，不能外推 Developer ID、公证、Gatekeeper 或 release-ready。

packaged Gateway 在新的空 HOME 中执行 `codex-auth status`，exit `0`，返回 `authenticated=false`、`reason=state_missing`、`auth_generation=0`，且 HOME 中未生成状态文件。

## RM-46 UI / mock 矩阵

App 由 LaunchServices 在全新隔离 HOME 启动，`CSSWITCH_ACCEPTANCE_PROVIDER_BASE_URL` 指向动态 `127.0.0.1:50631/rm46`。交互通过 macOS Accessibility 驱动产品 UI；mock 日志只保存 method、path、鉴权头是否存在与 `model`，不保存 header 值、假 key 或完整 body。

| 模板 | Discovery UI | 明确保存的模型 | 保存前 scratch 上游 | 结果 |
|---|---|---|---|---|
| OpenCode Go — OpenAI Chat | `实时 · 2 个候选 · openai_chat · 已按官方协议表过滤 3 个` | `kimi-k3` | `POST /rm46/v1/chat/completions`，`model=kimi-k3`，无前缀 | `PASS` |
| OpenCode Go — Anthropic Messages | `实时 · 1 个候选 · anthropic · 已按官方协议表过滤 4 个` | `minimax-m3` | `POST /rm46/v1/messages`，`model=minimax-m3`，无前缀 | `PASS` |
| Grok（xAI） | `实时 · 5 个候选` | `grok-4.5` | `POST /rm46/v1/chat/completions`，`model=grok-4.5` | `PASS` |
| Gemini（OpenAI 兼容） | `实时 · 5 个候选`；随后手填非候选模型 | `gemini-rm46-manual` | `POST /rm46/chat/completions`，`model=gemini-rm46-manual` | `PASS` |

四次 discovery 共消费 4 个 `GET .../models`，四次保存前 scratch 共消费 4 个 POST；8/8 请求均带预期鉴权形态。最终 config mode 为 `0600`，profile count 为 4，`active_id` 仍为空，说明本轮只完成配置 UX，没有把任一 profile 应用为正式 runtime。最终 config SHA-256 为 `ee441c1a1a289e15bb686b4612ae9744f5f7edfa529e22cacd4e72dfa272aea5`；该隔离配置只含假 key，不进入仓库。

## Discovery 不写正式配置

- 第一条 OpenCode OpenAI discovery 前后正式 config 均不存在。
- OpenCode Anthropic discovery 前后 config SHA-256 均为 `b690dc017351fce33d850039ad8899765e08f5e3f8c751b6621e25f7a423509a`。
- Grok discovery 前后 config SHA-256 均为 `b5330c4ce477f2024c20d2805f09f12f857685f24e1782eeb5ff2be4f8624176`。
- Gemini discovery 前后 config SHA-256 均为 `1116aca568b1b23c55d270b91cd568f40f4546836714c1030bb3af3c4adadebf`。

产品 UI 在每次 discovery 后也明确显示“只有你在四个模型框中明确选择的 ID 才会保存；探测本身未修改配置”。最终 UI 同时显示四条 profile 与各自唯一模型，且最后一次状态为“已保存连接（已通过上游校验）”。最终截图 SHA-256 为 `5bf4ebf67416cffb4ecb604affcfd3290c915c386975ab898f3047c251a42568`，脱敏 mock request log SHA-256 为 `9d5dbe807089f35b12cdadafe1a1567ba100692dd12f29bcba90c8f48d0171df`；App stdout/stderr 均为 0 bytes。

## 清理与不能外推

产品“退出 CSSwitch”检测到隔离配置默认端口已有未知监听后 fail closed：它报告未发送信号且真实 `8765` 实例未受影响。本轮不把该退出路径记为 PASS；只按精确临时 bundle executable 确认并终止自己启动的 Desktop PID。之后辅助功能列表不再出现该 App，本轮 mock 已停，精确临时 App/Gateway 可执行路径无残留进程。没有向既有端口监听者或 Science 进程发送信号。

本结论只关闭 RM-46 的 exact-artifact local-mock 配置 UX。它不证明 RM-47 的真实 OpenCode Go、Grok 或 Gemini 账号/key、`/models`、标题、classifier、两轮工具、额度、计费、服务质量或错误分类；也不证明本 artifact 的 G1/Science、installed、升级/rollback、Skill/MCP、SSH、Developer ID、公证、DMG 或 release-ready。
