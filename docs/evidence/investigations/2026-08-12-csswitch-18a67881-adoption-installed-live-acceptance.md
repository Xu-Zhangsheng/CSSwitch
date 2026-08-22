# CSSwitch `18a67881` adoption、安装与真实 Provider 验收

日期：2026-08-12（Asia/Taipei）

## 范围与 source 身份

本轮由用户明确授权 exact artifact 构建、Claude Science adoption isolated-live、覆盖安装、
真实 Provider 调用与精确清理。受测 production source 固定为
`18a67881c7e7d760fa8deb7f53e6ba246a32d94d`；执行开始时 `next` HEAD
`4d0a87f29f145727ab8a5450968b71cef79ed00d` 只比它多
`.agents/context/known-issues.md` 的 evidence-only 更新，不是受测 production source。

- canonical source gate run：`8db59abb85f6c7d5f8d5694626ee61dc`；
- suites / observations：15/15 / 15/15 `PASS`，runner exit 0；
- completion seal SHA-256：
  `7c36a8ef19d9c8000e93cd53beed565eb2c94da858d274eaf6285e9a40c9c6e0`；
- source snapshot manifest SHA-256：
  `259c2b18732bc4d128484b041d6927537d6ed3dbd6957e5bfcfc76f91cf68085`；
- 最终 clean-context reviewer 又逐项复算 snapshot 590/590 entries 与 Git blob / mode / size / SHA，
  结果一致。

本轮没有 production 修复，因此没有生成新 source SHA，也没有重跑一个冒充新 source 的 gate。
既有 [ChangeRecord](../../../quality/changes/next/CHG-SCIENCE-RUNTIME-ADOPTION-NEXT.json)
继续只记录 `18a67881` 的 source/test closure；artifact、installed 与 authorized-live 结果只进入本页
和日期化 receipt，不回写该机器记录。

## 两份同源但独立的 artifact

| 身份 | Acceptance artifact | Normal / installed artifact |
|---|---|---|
| bundle ID / version | `com.csswitch.test` / `0.8.4` | `com.csswitch.menubar` / `0.8.4` |
| canonical bundle digest | `bb19a7e6caa6be9b0e8e3b86304d6c3e904d661f9a098c8a32a22fd54f6e9109` | `24f542d1a462b741515407ed610c1ca723458080c36941930ed4502de5baf1c2` |
| tree entries SHA-256 | `80116e28cd8347030867d5044f83f44c6f5207b5da652e54c5b1264dd46ba7e4` | `301508ed733cb796ed9ca45b39eb2836e46f8aa4b02cd15fc776ab3d76915120` |
| Desktop SHA-256 | `56f0bde9b5a90a2b8711c1fa0715fbe71afe67a55204fbe39d5b2f11dbbec612` | `1a75a29f0358eafd3b9e1de979e024e5cc2ef542bc6d2431b0ffee6b13adfe6e` |
| Gateway SHA-256 | `b8e96803925c85102b4156e74dfb6012c8fc7e59f4d3051e3410c7222257819b` | `8a619b9435dc49fe232bad1e9fc5a4139d5986500b4d56f46154e0952c715540` |

两份 App 均由 clean detached `18a67881` 全新构建；五项 packaged resources 与 exact source
逐字节一致，legacy `Resources/proxy` 不存在。Acceptance G1 receipt SHA-256 为
`c187818e8f0fec1651ef935c602a87488b4800ee1aadf0389b818e2887beb707`；normal artifact
identity receipt SHA-256 为
`5d841adf413fa833f75d9a5caee7a06cb9c07d1b548ebe8ffbd8d1d5f44a0a6d`。

二者同源但不是同一 artifact：Test build 含 acceptance-only 隔离 seam，normal build 才是本轮
覆盖安装对象。正式 review 完成后，两处 Acceptance App 与两处 normal 临时 App bundle 已精确删除；
identity、G1/G2 与 review receipt 保留。

## Science adoption isolated-live

固定 Claude Science 为 `com.anthropic.operon` 0.1.25、arm64，CLI SHA-256
`63b0f57aa3b9588ba9e61433d27c78df788f8fe2c1b51842db107d6697e9c03f`。
G2 最终 scoped decision 为 `PASS(scope=science-adoption-isolated-live)`：

- healthy 场景得到 `deferred_healthy`，未重启既有 owner，PID / serve count 保持不变；
- cold 场景选择 official candidate，并建立新的 Gateway / Science owner；
- managed receipt v2、action、binding、finalize 与 reopen 指向同一 selected attempt；
- reopen 复用同一 exact Desktop owner，不产生第二常驻实例；
- Desktop、Gateway、Science 的 live Seatbelt 与 loopback / owned Unix 边界通过；
- 最终 PID、listener、runtime root 与 helper 均清零。

`g2/adoption-result.json` SHA-256 为
`1aefff247241a2b6cdf0a600cbc28d6cb1fa3325d403c08120cd0414d2b47a93`，52-file
`hashes.sha256` SHA-256 为
`c5c4cdd4ea69d4ce41acdb703acdea27118c0b70116445fbff9e30ae0a078dfd`，外层 seal
SHA-256 为
`09f72713787bfd8d5b1299b2382ffb5c9d5d22469dda4bee33bdd6c59cb80286`。

通用 controller manifest 的 `INCONCLUSIVE` 只是它不认识这份 adoption 专项 aggregation；不能覆盖
同目录 custom result 的 scoped `PASS`，也不能反向把 adoption 扩写成完整 `B-RUNTIME-01` 或
Science 全领域 PASS。

## 覆盖安装与 installed identity

覆盖前标准安装根只发现 `/Applications/CSSwitch.app`，bundle ID
`com.csswitch.menubar`、版本 0.8.4；旧 Desktop / Gateway SHA-256 分别为
`352763dc…` / `4448c15e…`。同卷原子 swap 后，安装目标与 normal artifact 的 `diff -qr`
为空，canonical / tree / Desktop / Gateway 均与上表 normal 列一致。

- installed isolated UI smoke：`PASS`；receipt SHA-256
  `4e06cf515f3a7087856278938144e5a97a46b846e3562f7069d07989b28e3572`；
- 旧 App 临时 backup 在安装验收后精确删除，不能恢复；
- 配置文件、账号数据库、Keychain、token / API Key 和真实 Science data-dir 均未读取、复制或删除；
- 当前选择在测试后恢复为 DeepSeek；`last applied` 保留 MiniMax 的测试历史；
- 合成 Science project 为避免破坏用户数据而保留，没有删除真实 Science data-dir；
- 标准安装根和本任务已知临时构建根最终只剩
  `/Applications/CSSwitch.app` 一份 CSSwitch App。

该唯一性结论只覆盖 `/Applications`、`/System/Applications`、`~/Applications` 顶层与本任务已知
临时构建根，不宣称扫描整块磁盘。

## Authorized live Provider 矩阵

全部调用都从 installed CSSwitch 的正常产品路径启动真实 Science；只从产品 UI 读取
`credential_present` 布尔，不读取或保存 credential 值。共发出 10 个显式请求，自动重试为 0。
正式 evidence artifacts 不保存提示词、回答正文、URL 或 credential mask。

| Profile / Science model | Text final | Science UI incremental | Tools | Reasoning | Native search | Stop / cleanup |
|---|---|---|---|---|---|---|
| DeepSeek / `DeepSeek V4 Flash` | `PASS` | `PASS` | `PASS` | `NOT-RUN` | `NOT-RUN` | `PASS` |
| Qwen / `Qwen Plus` | `PASS` | `PASS` | `INCONCLUSIVE(http_400_before_tool)` | `NOT-RUN` | `NOT-RUN` | `PASS` |
| SiliconFlow / `DeepSeek V4 Pro` | `PASS` | `PASS` | `PASS` | `NOT-RUN` | `NOT-RUN` | `PASS` |
| OpenRouter / `Sonnet 5` | `INCONCLUSIVE(quota_402)` | `INCONCLUSIVE(quota_402)` | `INCONCLUSIVE(quota_402)` | `NOT-RUN` | `NOT-RUN` | `PASS` |
| Kimi / `Kimi K3` | `PASS(request_2)` | `PASS` | `INCONCLUSIVE(http_400_after_tool_result_before_final)` | `INCONCLUSIVE(no_surface)` | `INCONCLUSIVE(no_server_tool_binding)` | `PASS` |
| Codex（实验） | `NOT-RUN(catalog_probe_500)` | `NOT-RUN` | `NOT-RUN` | `NOT-RUN` | `NOT-RUN` | `PASS` |
| OpenCode Go / intended `kimi-k3` | `NOT-RUN(no_available_profile_model)` | `NOT-RUN` | `NOT-RUN` | `NOT-RUN` | `NOT-RUN` | `PASS` |
| Xiaomi / `MiMo V2.5 Pro` | `PASS` | `PASS` | `NOT-RUN` | `NOT-RUN` | `NOT-RUN` | `PASS` |
| Zhipu / `GLM 5.2` | `PASS` | `PASS` | `NOT-RUN` | `NOT-RUN` | `NOT-RUN` | `PASS` |
| MiniMax / `MiniMax M3` | `PASS` | `PASS` | `NOT-RUN` | `NOT-RUN` | `NOT-RUN` | `PASS` |

Kimi 的第一个请求已观察到一次批准和独立 tool-result 节点，但 tool result 回送上游后遇到
HTTP 400 / compute 类错误，最终回答未闭合，所以完整 tools card 仍为 `INCONCLUSIVE`。
第二个请求有最终文本和增量变化，但 Science 没有显示可绑定的
`server_tool_use` / `web_search_tool_result`、search card 或来源节点，且回答明确表示未执行
provider-native search，因此不能继承旧 RM-47 的 Kimi search PASS。

`Science UI incremental` 只证明真实 Science 页面在一次请求中出现多个增量状态；Science 没有独立
nonstream 控件，本轮不把它外推为 Gateway / Provider 协议级 stream + nonstream 双模式 PASS。
`credential_present=true` 也只证明产品配置有 credential，不证明 credential 有效、足额或具备账号
entitlement。

Provider matrix receipt SHA-256：
`c30342f5e9793eaf34aa258e18360515e6716524061519a63339f109df9a01ea`。

## 最终 review、封存与边界

正式 reviewer 从 clean context 启动，独立复核 source、两份 artifact、G1/G2、Science、installed、
Provider 分项与最终进程清理：

- `clean-context: YES`；
- `BLOCK/HIGH/MEDIUM/LOW=0/0/0/0`；
- 最终 gate：`PASS`；
- review receipt SHA-256：
  `bb649e6a146581b161da602f48a1dd743fb65a04716d27dbbdd9bdd3109f6944`。

12 项核心证据的 final evidence manifest SHA-256 为
`0302d8c3377d8a95ecc77b8402b9614b7ac8c58417c3779f7a830c3ecc39ac38`，逐项
`shasum -a 256 -c` 全部 `OK`。最终 acceptance seal SHA-256 为
`b1c26987bab716951b5c745dfb43c42a67e467520e171932afea007c6fd080ce`。

最终 Desktop、Gateway、Science process count 均为 0，TCP 8765 listener 为 0，本轮 10 个
Science tabs 已精确关闭。签名仍只是 linker ad-hoc；bundle strict/deep verify exit 1，Developer ID、
notarization、stapling、Gatekeeper、DMG、push、tag、Release 均未建立。本轮也没有实施或验证新的
SSH、Skill 或 MCP 功能。
