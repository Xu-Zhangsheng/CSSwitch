# CSSwitch v0.8.4 工程重构后基线

审计日期：2026-07-31（Asia/Taipei）

状态：本地 `next` 工程职责拆分收口审计；只建立源码结构、source-test 与独立源码审查结论，不代表 artifact、installed/live、真实 provider、Science、SSH、签名、公证或公开发布 PASS。

适用范围：工程合并 commit `5dbee3e40bf67451b9c45c48fc7d990379308355`，其父提交为 `next@e0115cb01b4d2aec22acc9e02f7a56b3a6d983d8` 与已审查累积候选 `78be80c5ec0d78a18f2cbbf49c38f410761b44aa`。本页所在的纯文档后继不改变该工程源码树或既有 source-test 结论。

失效条件：`next` 在 `5dbee3e` 后出现相关产品源码、测试、quality 合同或状态 owner 的实质变化时，必须从新 SHA 复核受影响部分；本页继续保留为本次工程收口的日期化证据。

## 1. 结论

本轮机械职责拆分已经完成并正常合入本地 `next`：

```text
next@e0115cb
  + reviewed cumulative candidate@78be80c
  -> merge@5dbee3e
```

合并后源码树与候选源码树完全一致；没有冲突修复、行为改写或额外产品改动。
`commands/runtime.rs`、`runtime/science.rs` 与 `runtime/proxy_lifecycle.rs` 已从“已审查待合入”转为 `next` 的工程事实。此前已在 `next` 闭合的
`sandbox_session`、Gateway `server.rs` 和 frontend `main.js` 拆分也包含在同一
最终工程树中。

这些 owner 现在均有明确维护原因。没有新的独立维护原因时，不再因行数继续细分。
后续工作应先重新清点调用面、状态 owner 与测试合同，再判断是逻辑重构还是新的
结构切片。

## 2. 当前结构映射

| 稳定边界 | 重构后入口 | 独立 owner |
|---|---|---|
| Desktop frontend | `desktop/src/main.js`，589 行 bootstrap / shell | preview、IPC、Codex、runtime、profile 分别由 `preview-adapter.js`、`ipc-client.js` 与三个 controller 拥有 |
| Tauri runtime commands | `commands/runtime.rs`，177 行 command façade | actions、Gateway/model discovery、lifecycle/settings、one-click/history recovery、status/diagnostics；测试在 `commands/runtime/tests.rs` |
| Science runtime | `runtime/science.rs`，54 行同模块 façade | contracts、executable、runtime state、managed launch、lifecycle；测试在 `runtime/science/tests.rs` |
| Gateway lifecycle | `runtime/proxy_lifecycle.rs`，57 行同模块 façade | recovery、launch contract、Skill bridge、binary lookup、managed lifecycle；测试在 `runtime/proxy_lifecycle/tests.rs` |
| Gateway server | `desktop/gateway/src/server.rs`，125 行 listener / dispatch façade | HTTP codec、inference dispatch、Skill bridge host；测试在 `server/tests.rs` |
| Sandbox transaction | `runtime/sandbox_session/mod.rs`，58 行 façade | authority snapshot、catalog verify、cleanup/recovery、one-click、route reconcile、SSH preflight 与按场景拆分的 transaction tests |
| Protected snapshot | `sandbox_session/authority_snapshot.rs`，35 行同模块 façade | contracts、test seams、budget/entry identity、filesystem、capture、restore |

历史收口量只用于解释本轮维护结果：`commands/runtime.rs` 原 7,546 行、
`runtime/science.rs` 原 3,416 行、`runtime/proxy_lifecycle.rs` 原 1,589 行，
frontend `main.js` 原 2,882 行。当前行数是 `5dbee3e` 的源码事实，不是新的拆分
阈值；维护原因和状态边界优先于文件大小。

## 3. 保持的合同

各切片均以行为保持的机械搬迁冻结，最终源码继续保持：

- Tauri command 名称、签名、DTO、event 与 crate-facing surface；
- frontend production / preview 分界、command caller、cold-read event 补偿与敏感数据掩码；
- `AppState`、`Lifecycle`、`Config`、receipt/manifest 与 live identity 的既有状态 owner；
- protected projection、opaque roots、journal、补偿链与锁序；
- Gateway provider/Codex 协议、HTTP/SSE、path secret、Runtime/Gateway environment allowlist；
- Gateway recovery/reuse/spawn/stop、legacy Python listener fail-closed 清理与“无 Python fallback”边界；
- Science executable 来源、snapshot、managed receipt、listener identity、recovery/reuse/stop 与可选 Skill bridge；
- typed failure、failure 文本、权限和凭证来源；
- 历史 Rust 测试 identity 与 active Bug / ChangeRecord、suite source/evidence path、源码文本合同和 production target 的映射。

具体 owner 现在由[Desktop 控制面](../architecture/desktop-control-plane.md)、
[运行时状态与事务](../architecture/runtime-state-transactions.md)、
[Gateway 与 provider 路由](../architecture/gateway-provider-routing.md)和
[Science runtime](../architecture/science-runtime.md)原位维护。功能能力表与运维
门禁经最终源码交叉检查没有因机械拆分发生语义变化，因此不制造重复正文或虚假
功能更新。

## 4. 验证与独立审查

工程切片在各自冻结 commit 完成 production check、聚焦测试、源码合同、quality
metadata、`impact-pr` 与 clean-context 独立审查。关键 completion seal 包括：

| 切片 | 完整 source gate | 精确绑定 |
|---|---|---|
| sandbox session 第一层目录化 | `4521d369104f185bd2b89e23c5633833`，15/15 PASS | `6ac840b` |
| transaction tests 场景拆分 | `f919be8a9d3685a97481ddb4086642bb`，15/15 PASS | `c878df0` |
| sandbox one-click / healthy reopen | `f01db6a7c0338627efa459cb3eb540e4`，15/15 PASS | `64e21e0` |
| authority snapshot | `f844a3b602f4eaedfc2d9ac2cc162cc0`，15/15 PASS | `9b809b3` |
| Gateway server | `fde08943ce9d77472fcf3cd5e863710d`，15/15 PASS | `653ecbe` |
| frontend main | `6c0cde92c610f9e053e2c2ed269ac5b0`，15/15 PASS | `ef51c35` |
| Science runtime | `b801dd64667960767717cae506696fab`，15/15 PASS | `5c074ff` |
| 累积 command / Science / proxy lifecycle 候选 | `660032997c6694fba0261950ef1bb64b`，15/15 PASS | `b8c1fb173490053c2f24560c5aa812032b55441f` |

最终 proxy lifecycle reviewer 为 clean-context `PASS`、无 findings。首个独立 tester
因普通 sandbox 禁止动态 loopback 给出环境 BLOCK；未把该结果冒充产品失败或
PASS。换全新 clean-context tester 后，只对隔离的 `127.0.0.1:0` 放行，18/18
聚焦测试 `TEST PASS`、无 findings。

合并 `5dbee3e` 后还确认：两个 parent 身份精确；候选是第二 parent；merge tree 与
`78be80c` tree 相同；`git diff --check`、quality metadata 与
`impact-pr --target-ref e0115cb01b4d2aec22acc9e02f7a56b3a6d983d8` 均 PASS。

strict clippy 的四个 `runtime/failure.rs` dead-code 诊断在未触及的
`f842cbd9a612189ef1fd1143212e3555544fb020` 基线可同样复现；本轮只把屏蔽这些
已复现 baseline lint 后的 candidate-delta clippy 记为 PASS，不把完整 strict
clippy 写成 PASS。

## 5. 证据边界

以上只证明源码结构、源码合同、自动 source-test 与独立源码审查。没有运行或证明：

- final artifact、installed App 或 live runtime；
- 真实 provider、真实 Claude Science、账号或 SSH；
- 真实 Skill/MCP/Plugin 领域执行；
- 签名、公证、Gatekeeper、tag、公开 Release 或发布附件；
- 本地 `next` 已 push、合入远端主干或进入任何 release。

本次也没有读取真实 `~/.claude-science`、API key、OAuth token、Keychain、SSH
私钥或账号数据库。

## 6. 后续边界

受管 Skill/MCP/Plugin 扩展控制面、统一 failure envelope、operation correlation、
脱敏日志和只读 diagnostics 是后续逻辑重构输入，不是本轮机械拆分的遗漏，也不是
当前 capability 已支持的证明。新的结构工作只有在重新研究后确认存在独立维护原因
才应开始；不得把这些规划夹入无行为变化的机械拆分。

本页建立后，短期 `.agents/context/refactor-progress.md` 删除；Git 历史继续保存
逐切片过程，当前架构正文和本页分别承担稳定合同与日期化工程基线。
