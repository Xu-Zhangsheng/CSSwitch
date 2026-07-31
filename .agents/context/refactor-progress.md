# 当前工程重构进度

状态：当前；仅记录 `next` 工程重构线的已合入切片，不是架构、功能、发布或 live 证据正文

最后复核：2026-07-31（Asia/Taipei）

失效条件：`next` 在最后复核工程 HEAD 之后出现产品源码、测试、quality 或重构顺序的实质变化，但本页未同步复核时，受影响进度立即失效；纯文档后继不改变已记录的 source-test 结论。本轮工程重构完成、取消或被新基线取代后，先把耐久边界提炼进 `docs/architecture/`、`docs/features/` 或 `docs/operations/`，把最终 exact-SHA 结论写入 dated audit，再删除本页。

## 本页回答什么

本页只回答“哪些机械拆分已经合入 `next`、当时保持了什么边界、验证到哪一层、下一刀是什么”。细粒度改动由 Git 保存；当前架构、功能合同和执行门禁仍分别以 [`docs/architecture/`](../../docs/architecture/README.md)、[`docs/features/`](../../docs/features/README.md) 和 [`docs/operations/`](../../docs/operations/README.md) 为准。

只在一个拆分切片已经合入 `next` 后更新本页。未合入候选、临时 worktree、逐次失败日志和未冻结计划不进入完成表。

## 当前工程线

- 本地目标分支：`next`
- 最后复核工程 HEAD：`ef51c358c2c80a6015c864ebb6642fcb1ee9757b`
- 相对前一工程基线：`653ecbeb6dc8e0ea4532d3c33851ee8ae4c14557`
- 当前已完成切片：`sandbox_session` 第一层目录化拆分；`transaction_tests` 按测试场景完成第二层拆分；`mod.rs` 收口为 facade，并把一键事务与 healthy reopen 按补偿边界拆开；`authority_snapshot.rs` 按 test seams、filesystem/copy、capture 与 restore 的独立维护原因完成第二层拆分；Gateway `server.rs` 收口为 listener / method-dispatch facade，并按 HTTP codec、inference dispatch、Skill bridge host 与测试夹具拆分；前端 `main.js` 收口为 bootstrap / shell，并拆出 preview adapter、IPC client、Codex、runtime 与 profile controller
- 下一阶段：前端机械拆分已经闭合；转入逻辑重构合同评估，统一界定受管 Skill/MCP/Plugin 扩展控制面、前后端职责、故障 envelope、operation correlation、脱敏日志和只读 diagnostics，再决定后续实现切片
- 当前证据边界：source-test 与独立源码审查；没有因此新增 artifact、installed、live、真实 provider、Science、SSH、签名、公证或公开发布结论。

## 已合入切片

| 日期 | 切片 | 基线 → 合入结果 | 结构结果 | 行为边界与验证 |
|---|---|---|---|---|
| 2026-07-31 | `desktop/src-tauri/src/runtime/sandbox_session.rs` 第一层目录化拆分 | `b7605bb` → `6ac840b` | 运行时实现显式装载 `sandbox_session/mod.rs`；按 authority snapshot、catalog verify、pending cleanup、recovery、route reconcile、SSH preflight 与 transaction tests 拆成私有子模块；旧 `.rs` 只保留 quality impact 兼容锚点 | 目标是机械拆分，不扩大 Runtime/Gateway allowlist，不改变 crate-facing surface 或测试 ID。`impact-pr --target-ref next` PASS；完整 source gate run `4521d369104f185bd2b89e23c5633833` 为 15/15 PASS，snapshot 精确绑定 `6ac840b`；第二轮 clean-context 独立审查 PASS、无 findings。 |
| 2026-07-31 | `sandbox_session/transaction_tests.rs` 场景拆分 | `dcec25e` → `c878df0` | 父模块只保留共享环境锁、临时目录/树快照夹具与 health test；其余测试按 authority snapshot、cleanup/recovery、Gateway catalog、runtime journal、SSH behavior、SSH source contract 与 transaction source contract 拆为七个私有子模块 | 测试函数集合保持 26 个、正文无行为改写；test identity 显式迁移并同步 core contract、active Bug evidence ref、source-gate inventory 与 15 处 catalog identity hash。生产 `cargo check --lib` PASS；聚焦测试 24 PASS / 2 个既有 ignored；`impact-pr --target-ref next` PASS；完整 source gate run `f919be8a9d3685a97481ddb4086642bb` 为 15/15 PASS，snapshot 精确绑定 `c878df0`；clean-context 独立审查 PASS、无 findings。 |
| 2026-07-31 | `sandbox_session/mod.rs` 一键事务收口 | `e0cb5e8` → `64e21e0` | `mod.rs` 只保留私有子模块装载、历史 crate-facing re-export 与父级测试夹具面；30 个生产函数迁入 `one_click.rs`。仅 healthy Science 的 Gateway/config 重绑因不触碰 authority snapshot 且拥有独立补偿边界，进一步拆入 `one_click/healthy_reopen.rs`；restart、authority capture 与失败补偿继续和主事务同处，保持锁序与单一补偿漏斗 | 30 个生产函数体逐体比较零差异；未扩大 Runtime/Gateway allowlist、typed failure 或 `pub(crate)` surface，测试专用桥只到 `pub(super)`。生产 `cargo check --lib` PASS；聚焦 transaction tests 24 PASS / 2 个既有 ignored；受影响 Python 源码合同与两项 Rust AST 合同 PASS；测试身份不变；隔离 `impact-pr --target-ref next` PASS；完整 source gate run `f01db6a7c0338627efa459cb3eb540e4` 为 15/15 PASS，snapshot 精确绑定 `64e21e0`；clean-context 独立审查 PASS、无 findings。 |
| 2026-07-31 | `sandbox_session/authority_snapshot.rs` 边界拆分 | `cf7ec66` → `9b809b3` | 保留原路径作为同模块 facade；按合同与常量、共享 test seams、copy budget/entry identity、descriptor-relative filesystem primitives、capture/copy、restore 拆为六个片段。`include!` 保持历史模块与可见性；恒 false 的 path modules 只把片段纳入标准 rustfmt 发现路径 | 原函数体、声明顺序、`cfg`/`allow`、135 处 failure code、预算、protected/opaque allowlist 与 capture/restore/cleanup 安全不变量保持不变。`cargo fmt --all -- --check`、直接 rustfmt、生产 `cargo check --offline --lib`、聚焦 transaction tests 24 PASS / 2 个既有 ignored、受影响 Python 源码聚合 26 PASS / 5 个既有 skip、两项 Rust AST/flow 合同、458 个 Rust 测试身份、metadata 与 `impact-pr --target-ref next` 均 PASS；完整 source gate run `f844a3b602f4eaedfc2d9ac2cc162cc0` 为 15/15 PASS，snapshot 精确绑定 `9b809b39dc81a55f4596e9aae7ff64161d00b6e1`；最终 clean-context 独立审查 PASS、无 findings。 |
| 2026-07-31 | Gateway `server.rs` 职责拆分 | `e426bae` → `653ecbe` | `server.rs` 从协议实现主体收口为 125 行私有模块装载、listener 与 method dispatch facade；HTTP 编解码与流过滤迁入 `server/http_codec.rs`，GET/POST、provider/Codex inference 与 SSE 迁入 `server/inference_dispatch.rs`，Skill bridge host lifecycle 迁入 `server/skill_bridge_host.rs`；测试外移到 `server/tests.rs`，并以显式 path 保持 `server::tests::*` identity | 66 个顶层生产函数与 8 个方法逐体 token 比较无行为改写；唯一 crate-facing public entry 仍为 `server::serve(GatewayConfig)`，跨子模块可见性最多 `pub(super)`。CONNECT 优先分派、path secret、header/body bounds、HTTP/error/SSE、provider/Codex dispatch、Skill bridge replay/heartbeat/terminal-once/host lock、Unix/non-Unix cfg 均保持。33 个目标测试 identity 不变；`cargo fmt --check`、生产 `cargo check --lib`、`clippy --all-targets -D warnings`、Gateway 278 个 lib + 1 个 integration test、loopback 103 tests、metadata 与 `impact-pr --target-ref next` 均 PASS。完整 source gate run `fde08943ce9d77472fcf3cd5e863710d` 为 15/15 PASS，completion seal 精确绑定 `653ecbeb6dc8e0ea4532d3c33851ee8ae4c14557`；修复首次独立审查发现的 catalog ChangeRecord BLOCK 后，最终 clean-context 独立审查 PASS、无 findings。 |
| 2026-07-31 | 前端 `desktop/src/main.js` 职责拆分 | `273565e` → `ef51c35` | `main.js` 从 2882 行收口为 589 行 bootstrap / shell；浏览器预览与 mock state 迁入 `preview-adapter.js`，Tauri transport / event / window adapter 迁入 `ipc-client.js`，Codex OAuth/network/downgrade、runtime lifecycle/status 与 profile/catalog/form 分别迁入三个 controller；共享 busy / activation / page / feedback 状态仍由单一 bootstrap 组合 | Tauri command 名称与调用次数、顶层 camelCase / serde snake_case、`boot://failed`、`boot://attention`、`codex-auth://operation` listener 与 cold read、preview/production 分界、掩码与敏感数据边界保持。前端门、23 项受影响 Python 合同、模块图与浏览器 preview smoke PASS；metadata 与 `impact-pr --target-ref next` PASS；完整 source gate run `6c0cde92c610f9e053e2c2ed269ac5b0` 为 15/15 PASS，completion seal 精确绑定 `ef51c358c2c80a6015c864ebb6642fcb1ee9757b`。首次 clean-context 独立审查只报测试切片结束锚点 LOW；修复后由另一位 clean-context reviewer 复审 PASS、无 findings。 |

### `sandbox_session` 拆分提交

| Commit | 作用 |
|---|---|
| `4be0359` | 首次拆出内部模块 |
| `a328be6` | 独立处理继承的 Rust 格式基线 |
| `2891472` | 修复生产 target 的 `cfg(test)` 边界、源码合同读取与测试身份/质量元数据 |
| `fb7b603` | 外移 transaction tests，并闭合 active ChangeRecord 与测试路径映射 |
| `6ac840b` | 保留旧路径兼容锚点，以显式 `#[path]` 装载新目录，消除 fail-closed rename/delete 状态 |
| `c878df0` | 按测试场景拆分 `transaction_tests`，显式迁移测试身份并闭合 active quality / evidence 映射 |
| `64e21e0` | 把 `mod.rs` 收口为 facade，将一键事务迁入 `one_click.rs`，并按独立补偿边界拆出 healthy reopen |
| `9b809b3` | 按独立维护原因拆分 authority snapshot 合同、test seams、filesystem/copy、capture 与 restore，并把 include 片段纳入标准 rustfmt 发现路径 |

### Gateway `server.rs` 拆分提交

| Commit | 作用 |
|---|---|
| `b5d64ae` | 把 Gateway listener/method dispatch、HTTP codec、inference dispatch、Skill bridge host 与测试夹具拆成独立私有模块，并同步当前 capability / active Bug 路径 |
| `653ecbe` | 把 runtime-loaded `catalog/` 纳入 production path policy，并以专用 active ChangeRecord 闭合本切片的 changed-path 与 required suite/gate 映射 |

### 前端 `main.js` 拆分提交

| Commit | 作用 |
|---|---|
| `ef51c35` | 把浏览器预览、IPC、Codex、runtime 与 profile 职责拆出 `main.js`，同步源码合同测试，并以专用 active ChangeRecord 闭合 production/test path 与 required suite/gate 映射 |

### 本切片暴露的维护约束

- 只跑 `cargo test --lib` 不足以证明生产 target 可编译；拆分后必须覆盖非测试 target。
- source gate 的 metadata profile 不能替代 `impact-pr`；正式冻结前两者都必须通过。
- 测试若读取源码文本，模块拆分时必须改为目录聚合，并重新核对 discovered test identity。
- 使用 `include!` 保持原模块可见性时，片段不会自动进入 Cargo 的 rustfmt 模块发现；必须给标准格式门禁一个不参与编译的显式 path 索引，并以 verbose Cargo fmt 与直接 rustfmt 双重核对。
- 测试模块层级变化会改变 Rust test identity；必须同时迁移完整 discovered inventory、approved ignored 子集、core contract 与 active evidence ref，且重新绑定 catalog identity hash。
- active Bug / ChangeRecord 与 suite source path 必须随当前源码位置迁移；dated audit / evidence 中的历史路径不改写。
- Git 的 rename/delete/copy 状态受 quality policy fail-closed 约束；结构切片必须在候选冻结前检查最终 `name-status`。
- compile-time 或 runtime 装载的 `catalog/` 不是纯叙事文件；其变更必须进入 production path policy，并由真实 active ChangeRecord 覆盖。
- 拆出测试文件但需要保持 Rust test identity 时，可用显式 `#[path]` 保留原模块层级；仍须比较完整 discovered identity，不能只比较函数名。
- 主工作树中的 ignored runtime 数据可能被动态测试发现机制纳入扫描；不得为门禁删除用户数据，正式 metadata / impact / source gate 应在 clean exact-HEAD worktree 执行。
- JavaScript factory 注入的协作者名不能与函数内布尔或 DTO 局部变量同名；机械搬迁后要专门覆盖错误路径，避免正常路径通过但 catch 分支调用到被遮蔽值。
- 源码文本合同迁移到新 owner 后，切片的开始与结束锚点必须同在目标文件，并验证 `indexOf` 没有返回 `-1`；不能只改读取路径后继续用旧文件锚点。

## 下一阶段边界

`sandbox_session` 的 `mod.rs`、`transaction_tests`、`authority_snapshot` 第二层拆分、Gateway `server.rs` 职责拆分与前端 `main.js` 职责拆分已经闭合。已拆出的 runtime、Gateway 与前端 controller 默认保持稳定；没有新的独立维护原因时不继续按行数细分。下一阶段另立逻辑重构合同，评估受管 Skill/MCP/Plugin 扩展控制面，并同时冻结前后端职责、统一故障 envelope、operation correlation、脱敏日志和只读 diagnostics；这些规划不改变当前 capability map 的支持结论。后续结构切片继续遵守：

- 先冻结现有对外 surface、测试身份、Runtime/Gateway allowlist 与 typed failure 边界；
- 子模块按单一维护原因拆分，不借机改变 provider、transport 或协议语义；
- 在完整 source gate 前先验证生产 target、源码合同、quality metadata 与 `impact-pr`；
- 仍按“候选冻结 → clean-context 独立审查 → 修复后换新 reviewer”的闭环执行；
- 未经单独授权，不执行真实 provider、Science、SSH、artifact、安装、签名或发布验证。

## 最终收口

工程重构全部完成后：

1. 以最终 `next` exact SHA 重新从源码核对当前架构和功能边界；
2. 原位更新当前权威 architecture / feature / operation 正文，不创建一套平行“新版文档”；
3. 新建一份绑定最终 SHA 的 post-refactor baseline audit，记录结构映射、验证层和剩余缺口；
4. 运行文档治理检查与独立审查；
5. 删除本页，Git 历史保留过程。
