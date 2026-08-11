# RM-47 真实 Provider + Science 授权验收

日期：2026-08-11（Asia/Taipei）

## 范围与身份

本轮由用户明确授权真实 Provider 请求、隔离运行、必要构建、提交与精确清理。production
code-bearing source 为 `next@e1832bd35e9384265df9911a42f841ed90c0f43c`；最终 source/test
closure 为 `0ecc7e2a105b2bb270d42f95a42148a6cd23bbca`，后续提交只补真实 server-search ID
形态的 test fixture、修正一条过时的 stream integration 断言并更新文档，不改变 production 行为。
目标为同源新构建且未安装的
`CSSwitch Test.app`、packaged Rust Gateway 和只读固定的
`/Applications/Claude Science.app` 0.1.25。

- isolated root：`/private/tmp/csswitch-live-7bfdcb6.1RqkTT`；HOME、Science data-dir、
  CSSwitch state 和合成 PDF 都在该根内；
- Gateway / Science：动态端口 `59704` / `59705`；真实 `8765` listener PID 前后保持不变；
- Desktop SHA-256：`98f24b69e40c2e238fbf18ae26539c44293a4f33c7d6c9b025008b840597566a`；
- packaged Gateway SHA-256：`56a724832dc41dafd3d8bea6d5d67446d49393260336d4cd8aaf31152fca37a0`；
- credential：只核对变量名和非空状态；8 个已授权 credential 只由隔离进程消费，未读取、
  输出或写入证据。OpenCode、Grok、Gemini 因变量缺失保持 `NOT-RUN`；
- 数据与预算：只用合成文本和单页合成 PDF；每个 provider 的主验收无自动重试。Kimi 的
  兼容诊断是人工触发的有限结构对照，每次 `max-time=90`，只输出状态、block type、字段名和
  ID 是否非空，不保存正文或搜索结果。

## CSSwitch + Science 实际链路

```text
CSSwitch profile / static model catalog
  -> 一键开始与 path-secret Gateway
  -> Science /v1/models selector
  -> agent/title/reviewer/tool-result 等独立 /v1/messages
  -> Gateway model resolver + provider adapter + SSE filter
  -> real provider
  -> Science client-tool execution / server-tool rendering / reviewer
```

因此主 agent 成功、标题/reviewer、provider 配额、Science environment/kernel 必须分别记录；
不能用其中一层替代另一层。

## Provider 结果

| Profile / model | Science selector | 真实结果 | 判定 |
|---|---|---|---|
| DeepSeek / `deepseek-v4-flash` | `DeepSeek V4 Flash` | 最小文本返回；Bash `printf` 经一次性授权执行并返回 | `PASS(text,client-tool)` |
| Qwen / `qwen-plus-latest` | `Qwen Plus` | 最小文本返回；模型产生 client tool_use，但 Science `claude-science-mcp` environment 失败、执行未完成 | `PASS(text)`；`INCONCLUSIVE(science-compute)` |
| GLM / `glm-5.2` | `glm-5.2` | 最小文本返回 | `PASS(text)` |
| Kimi / `kimi-k3` | Science 一度显示旧 selector 为 unavailable；Gateway 仍固定路由到 `kimi-k3` | 搜索块可见且不再报 OPERON tool-not-found；受控原生请求中未搜索与搜索均 200；PDF 进入本地 Python 一次性授权，最终受 Science package 环境阻断 | `PASS(text,server-search)`；`INCONCLUSIVE(pdf-compute,selector-display)` |
| MiniMax / `MiniMax-M3` | `MiniMax-M3` | 最小文本返回 | `PASS(text)` |
| OpenRouter / `anthropic/claude-sonnet-5` | `Anthropic / Claude Sonnet 5` | Science 请求到达真实上游；因账户可用额度不足以承受 Science 的 `max_tokens=128000` 返回 402，未 fallback | `INCONCLUSIVE(quota)` |
| SiliconFlow / `deepseek-ai/DeepSeek-V4-Pro` | `deepseek-ai/DeepSeek-V4-Pro` | 最小文本返回 | `PASS(text)` |
| Xiaomi / `mimo-v2.5-pro` | `mimo-v2.5-pro` | 随机验收串被模型策略拒绝；改用不含答案的自然问题后正确返回 `Paris` | `PASS(text)` |
| OpenCode Go / Grok / Gemini | — | credential 变量缺失，未发请求 | `NOT-RUN` |

MiniMax 的早期 direct Gateway 探针曾遇到一次 502；随后直连与手动 Gateway 复验均 200，
Science 文本也通过。该事件只记录为上游瞬时结果，不建立自动重试策略。OpenRouter 的 402 是
配额证据，不应由 Gateway 静默压低模型、替换 provider 或伪装成 PASS。

最终 clean packaged artifact 又对本轮后半段做了无自动重试复核：MiniMax、SiliconFlow 的精确
文本标记与 Xiaomi 的自然问答均 PASS；OpenRouter 再次由 Science 到达真实上游并返回同一 402
配额错误。DeepSeek、Qwen、GLM、Kimi 使用的 artifact Desktop / Gateway hash 与该最终复核相同。

## Kimi 根因与 PDF 边界

旧实现把 Science 的 `web_search_20250305` server tool 降成普通 client `web_search`，又删除
`server_tool_use` / `web_search_tool_result`。真实 Science 随后把它交给 OPERON 本地执行，返回
`Tool 'web_search' not found on agent 'OPERON'`。`e1832bd` 改为只保留 Kimi 支持的 server
search，并在流式响应中保留相应结果块；真实 Science 已显示 search result，旧错误未再出现。

Kimi 官方 endpoint 的受控结构探针得到：

- thinking enabled、声明 server search、模型不调用时：200，只有 thinking/text；
- 强制搜索时：200，包含带非空关联 ID 的 `server_tool_use` /
  `web_search_tool_result`；
- server search 与普通 client tool 共存、thinking enabled/disabled：首轮均 200；
- 把首轮 assistant content 原样回放并提交匹配 tool result：第二轮 200。

Science PDF 尝试曾出现 Kimi 瞬时 400 `tool_call_id is not found`，相同链路后续又成功进入
本地 Python 授权；结构对照无法复现确定性 Gateway 变换错误，因此没有加入猜测性的 ID 修补或
自动重试。raw `application/pdf` document block 仍由 Gateway 本地 400 明确拒绝；text/content
document 已通过。正确边界是 Science PDF skill/OCR 先转 text/image，而不是在 Gateway 内再造一套
PDF 引擎。

本地 PDF skill 已生成并视觉/文本校验单页合成 PDF，成功请求一次性 Python 授权；执行超过 3 分钟
仍未完成。Science environment 显示 `claude-science-mcp` 失败，技术详情为 conda-forge
`repodata.json.zst` SSL unexpected EOF。对同一 URL 经当前 Gateway 做单次 CONNECT HEAD 返回
HTTP 200，所以这里只能判为 Science package/compute 环境阻断，不能写成 Kimi document 失败或
PDF E2E PASS。

Kimi server-search 行为参考官方说明：

- <https://platform.kimi.ai/docs/guide/use-web-search>
- <https://platform.kimi.ai/docs/guide/use-official-tools>
- <https://platform.kimi.ai/docs/guide/use-kimi-api-to-complete-tool-calls>

## 验证与清理

- Rust `anthropic_compat::tests`：17/17 PASS；
- Kimi loopback targeted：3/3 PASS；
- 当前工作区含受保护未跟踪文件和被忽略 runtime 内的未跟踪 `.gitignore`，可信 snapshot 会在
  suite 前 fail closed；没有为过门禁删除这些用户数据，而是在 `/private/tmp` 创建同 commit 的
  non-shallow clean clone。首次 clean-clone full gate 绑定 `12e39c2`，run
  `59a782697fa3a571be84df417db02f80` 为 14/15 后 `FAIL`：唯一失败是旧 server integration
  断言仍要求删除 `server_tool_use`。该断言在 `0ecc7e2` 对齐新合同，聚焦复验 PASS；
- full source gate：`0ecc7e2a105b2bb270d42f95a42148a6cd23bbca`，run
  `903ebd2e365ac12274639fc676ca9388`，15/15 suites、15/15 observations、runner exit 0，
  completion seal SHA-256
  `a7d31bb706ef8acb57e44abe640d3f24387cc76d7eb956fdad261c40f7d4f102`，`PASS`；
- 最终 CSSwitch UI stop、guard、浏览器 run tabs 与 runtime/credential 临时根清理：
  `PASS`；真实 8765 listener PID 保持不变，59704/59705 无残留监听，本轮 Chrome tabs 为 0，
  隔离 HOME、临时 credential/config、合成 PDF、诊断响应、source-gate clone/output 均精确删除。

本证据不建立 OpenCode/Grok/Gemini、OpenRouter 足额配额、Kimi PDF compute、installed app、
签名、公证或 release PASS。
