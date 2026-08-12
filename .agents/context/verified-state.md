# 已验证状态快照

状态：当前；按 source、artifact、installed、live、signing 与 public 分层汇总

最后复核：2026-08-12（Asia/Taipei）

失效条件：受测 source、artifact identity、安装 App、Science / Gateway runtime、Provider
结果、签名或公开 Release 任一相关事实改变时，对应层立即失效；未受影响层仍按 exact identity 判断。

| 层 | 当前可声明的事实 |
|---|---|
| Current source candidate | 当前 source/test-bearing exact candidate 为 `next@d2cf95e877aa110013a8360d6fcd72c1b38bcfb3`；包含本段的后续 Context refresh 只更新文档状态。fresh clean-context completion review 与 canonical GATE-SOURCE 均为 `PASS`、四级 finding 全 0；该 tuple 建立 `SOURCE-GREEN`，但下游 artifact / installed / live 不得继承 |
| Latest canonical-gated source | `next@d2cf95e877aa110013a8360d6fcd72c1b38bcfb3` 的 canonical run `15dcfea64b2e3b58ded18910b676cc50` 为 15/15 suites、15/15 observations、exit 0；completion seal `e1f65436…9f9d05`，source snapshot manifest `feca1e67…ded0d7`；fresh completion review `PASS`，`BLOCK/HIGH/MEDIUM/LOW=0/0/0/0` |
| Last accepted source / test lineage | 当前 `next@d2cf95e877aa110013a8360d6fcd72c1b38bcfb3`；canonical gate 与 fresh completion review 同时 `PASS`，只绑定该 exact source tuple；此前历史 accepted tuple 为 `18a67881c7e7d760fa8deb7f53e6ba246a32d94d` |
| Historical Acceptance artifact / G1 | 从 `18a67881` 新构建 `com.csswitch.test` 0.8.4；canonical `bb19a7e6…6e9109`，Desktop `56f0bde9…bec612`，Gateway `b8e96803…57819b`；G1 `PASS`；临时 App 在正式 review 后已删除，receipt 保留 |
| Historical Science adoption isolated-live | `18a67881` artifact + Claude Science 0.1.25 / CLI `63b0f57a…9c03f`；healthy → `deferred_healthy` 不重启、cold candidate selected、v2 binding/finalize/reopen 一致；scoped G2 `PASS`，outer seal `09f72713…80286` |
| Current installed test artifact | 标准安装根与本任务已知临时构建根只剩 `/Applications/CSSwitch.app`；`com.csswitch.menubar` 0.8.4；canonical `24f542d1…f1c2`，tree `301508ed…5120`，Desktop `1a75a29f…fe6e`，Gateway `8a619b94…5540`；与 reviewed normal artifact exact match；installed smoke `PASS` |
| Authorized Provider live | 10 个显式请求、0 自动重试；DeepSeek / SiliconFlow 的 text + Science UI incremental + tools `PASS`；Qwen text + UI incremental `PASS`、tool `INCONCLUSIVE(400)`；Kimi text + UI incremental `PASS`，完整 tools / reasoning / native search 保持 `INCONCLUSIVE`；Xiaomi / Zhipu / MiniMax text + UI incremental `PASS`；OpenRouter `INCONCLUSIVE(quota_402)`；Codex / OpenCode 未发请求 |
| Final runtime cleanup | CSSwitch Desktop、Gateway、Science process count 均为 0；TCP 8765 listener 为 0；10 个本轮 Science tabs 已关闭；当前选择恢复 DeepSeek |
| Current installed signing | 仅 linker ad-hoc；无 Team ID / sealed resources，strict/deep verify exit 1；Developer ID、notarization、Gatekeeper 均未建立 |
| Public v0.8.4 release | 公开 DMG SHA-256 `23471daf…f2b2`、peeled tag、Release source 与旧公开 evidence 保持原结论；当前 installed test artifact 不是该公开 DMG artifact，本轮没有刷新或改变 public layer |

本轮开始时实时复核 `next` HEAD 为 `9c91bec611f98f2d87e44db9edd5b5f51210240c`。随后形成的
managed-health proof、owner race fixture、inventory 与合同修改已提交为
`9476be5cd23ff7d78d01b59a4aa392af00bba904`；其上的 Context refresh
`dfb6f59114cd54aacae91a0c94a7baaf69a71895` 只更新当时状态，不改变 production / test。Rust
1.96.1 clippy hygiene source candidate 已提交为 `76616ff085610dabf0112202324cc97d20fcbec6`；
其后的 docs-only descendant `d2cf95e877aa110013a8360d6fcd72c1b38bcfb3` 已完成 fresh
clean-context completion review 与 canonical 15-suite source gate，二者均为 `PASS`；包含本段的
后续 Context refresh 同样不改变 production / test。另有受保护的未跟踪
`.tmp-bskill-r9-driver.py`，本轮未读取、未修改。Science candidate reviews 精确绑定 base `9c91bec` 与 tracked binary diff
`61542943a9865b7ab3db61f31246f0dbc1ce4bbdb1db494a190ccfd5a37a6612`；这不允许继承
`9c91bec` 的旧 gate，也不会改写 `18a67881` 的历史证据；Science 与 clippy candidate reviews
已由 `d2cf95e` 的 exact source closure 取代。当前 HEAD 与 worktree 仍须在使用前实时复核。

Provider 结果必须按 operation 分项解释：`Science UI incremental` 只证明真实 Science 页面出现
多次增量状态，不等于 Gateway / Provider protocol-level stream + nonstream 双模式；
`credential_present` 只表示产品配置非空，不证明凭证有效、余额或 entitlement。Kimi 第二个请求
没有形成可绑定的 `server_tool_use` / `web_search_tool_result`，因此不能继承旧 RM-47 的 search
PASS。Codex 在 `catalog_verify` 500 前停下；OpenCode Science selector 没有配置要求的可用模型，
二者均未发 Provider 请求。

历史 `18a67881` tuple 的 clean-context final review 为
`PASS`、`BLOCK/HIGH/MEDIUM/LOW=0/0/0/0`。Provider receipt
SHA-256 为 `c30342f5…a01ea`，review receipt 为 `bb649e6a…f6944`，12 项 evidence manifest
为 `0302d8c3…9ac38`，final acceptance seal 为 `b1c26987…080ce`。

配置、账号数据库、Keychain、token / API Key、SSH 私钥和真实 Science data-dir 未读取或删除。
测试产生的合成 Science project 为避免破坏用户数据而保留；`last applied` 仍显示 MiniMax 的测试
历史，但没有运行中的 Gateway / Science。旧安装 backup 与四个已审临时 App bundle 已精确删除，
不可恢复。

仍不是当前 PASS：当前 source candidate 的全部下游 artifact/runtime 层；完整 protocol-level
stream/nonstream 双模式、Kimi reasoning/native search、
Qwen/Kimi 完整 tools card、OpenRouter 足额配额、Codex/OpenCode 请求、真实 SSH server、
Skill/MCP 新功能、Science 全领域行为、Intel/Windows/WSL、Developer ID/notarization/Gatekeeper、
新 DMG 或公开 Release。

完整 identity、Provider 分项、clean review、封存与不能外推的边界见
[2026-08-12 日期化验收](../../docs/evidence/investigations/2026-08-12-csswitch-18a67881-adoption-installed-live-acceptance.md)。
公开 v0.8.4 历史层见
[release evidence](../../docs/evidence/releases/v0.8.4.md)；其他历史 tuple 从
[调查索引](../../docs/evidence/investigations/README.md)进入。
