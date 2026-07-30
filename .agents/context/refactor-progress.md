# 当前工程重构进度

状态：当前；仅记录 `next` 工程重构线的已合入切片，不是架构、功能、发布或 live 证据正文

最后复核：2026-07-31（Asia/Taipei）

失效条件：`next` 在最后复核工程 HEAD 之后出现产品源码、测试、quality 或重构顺序的实质变化，但本页未同步复核时，受影响进度立即失效；纯文档后继不改变已记录的 source-test 结论。本轮工程重构完成、取消或被新基线取代后，先把耐久边界提炼进 `docs/architecture/`、`docs/features/` 或 `docs/operations/`，把最终 exact-SHA 结论写入 dated audit，再删除本页。

## 本页回答什么

本页只回答“哪些机械拆分已经合入 `next`、当时保持了什么边界、验证到哪一层、下一刀是什么”。细粒度改动由 Git 保存；当前架构、功能合同和执行门禁仍分别以 [`docs/architecture/`](../../docs/architecture/README.md)、[`docs/features/`](../../docs/features/README.md) 和 [`docs/operations/`](../../docs/operations/README.md) 为准。

只在一个拆分切片已经合入 `next` 后更新本页。未合入候选、临时 worktree、逐次失败日志和未冻结计划不进入完成表。

## 当前工程线

- 本地目标分支：`next`
- 最后复核工程 HEAD：`c878df046b2ad97c1dcc57dda5b9e3ad8bf95f57`
- 相对前一工程基线：`6ac840b94fe0c4f958ef908abad9c70cb7ff91db`
- 当前已完成切片：`sandbox_session` 第一层目录化拆分；`transaction_tests` 按测试场景完成第二层拆分
- 下一阶段：只读评估 `sandbox_session/mod.rs` 与 `authority_snapshot.rs` 的剩余维护边界，再决定是否进入 Gateway `server.rs`
- 当前证据边界：source-test 与独立源码审查；没有因此新增 artifact、installed、live、真实 provider、Science、SSH、签名、公证或公开发布结论。

## 已合入切片

| 日期 | 切片 | 基线 → 合入结果 | 结构结果 | 行为边界与验证 |
|---|---|---|---|---|
| 2026-07-31 | `desktop/src-tauri/src/runtime/sandbox_session.rs` 第一层目录化拆分 | `b7605bb` → `6ac840b` | 运行时实现显式装载 `sandbox_session/mod.rs`；按 authority snapshot、catalog verify、pending cleanup、recovery、route reconcile、SSH preflight 与 transaction tests 拆成私有子模块；旧 `.rs` 只保留 quality impact 兼容锚点 | 目标是机械拆分，不扩大 Runtime/Gateway allowlist，不改变 crate-facing surface 或测试 ID。`impact-pr --target-ref next` PASS；完整 source gate run `4521d369104f185bd2b89e23c5633833` 为 15/15 PASS，snapshot 精确绑定 `6ac840b`；第二轮 clean-context 独立审查 PASS、无 findings。 |
| 2026-07-31 | `sandbox_session/transaction_tests.rs` 场景拆分 | `dcec25e` → `c878df0` | 父模块只保留共享环境锁、临时目录/树快照夹具与 health test；其余测试按 authority snapshot、cleanup/recovery、Gateway catalog、runtime journal、SSH behavior、SSH source contract 与 transaction source contract 拆为七个私有子模块 | 测试函数集合保持 26 个、正文无行为改写；test identity 显式迁移并同步 core contract、active Bug evidence ref、source-gate inventory 与 15 处 catalog identity hash。生产 `cargo check --lib` PASS；聚焦测试 24 PASS / 2 个既有 ignored；`impact-pr --target-ref next` PASS；完整 source gate run `f919be8a9d3685a97481ddb4086642bb` 为 15/15 PASS，snapshot 精确绑定 `c878df0`；clean-context 独立审查 PASS、无 findings。 |

### `sandbox_session` 拆分提交

| Commit | 作用 |
|---|---|
| `4be0359` | 首次拆出内部模块 |
| `a328be6` | 独立处理继承的 Rust 格式基线 |
| `2891472` | 修复生产 target 的 `cfg(test)` 边界、源码合同读取与测试身份/质量元数据 |
| `fb7b603` | 外移 transaction tests，并闭合 active ChangeRecord 与测试路径映射 |
| `6ac840b` | 保留旧路径兼容锚点，以显式 `#[path]` 装载新目录，消除 fail-closed rename/delete 状态 |
| `c878df0` | 按测试场景拆分 `transaction_tests`，显式迁移测试身份并闭合 active quality / evidence 映射 |

### 本切片暴露的维护约束

- 只跑 `cargo test --lib` 不足以证明生产 target 可编译；拆分后必须覆盖非测试 target。
- source gate 的 metadata profile 不能替代 `impact-pr`；正式冻结前两者都必须通过。
- 测试若读取源码文本，模块拆分时必须改为目录聚合，并重新核对 discovered test identity。
- 测试模块层级变化会改变 Rust test identity；必须同时迁移完整 discovered inventory、approved ignored 子集、core contract 与 active evidence ref，且重新绑定 catalog identity hash。
- active Bug / ChangeRecord 与 suite source path 必须随当前源码位置迁移；dated audit / evidence 中的历史路径不改写。
- Git 的 rename/delete/copy 状态受 quality policy fail-closed 约束；结构切片必须在候选冻结前检查最终 `name-status`。

## 下一阶段边界

`transaction_tests` 的第二层场景拆分已经闭合。继续对 `sandbox_session` 做只读的剩余收口评估：

- `mod.rs` 的一键编排、Science restart、补偿与 healthy reopen 是否已有不同维护原因；
- `authority_snapshot.rs` 的 test seams、filesystem/copy primitives 与 capture/restore 是否应分开；
- 已拆出的 transaction test 场景默认保持稳定；没有新的独立维护原因时不继续细分；
- 行数不是单独拆分理由，只有独立职责、维护触发器或失效条件成立时才继续拆。

完成上述判断后，再决定是否进入 Gateway `server.rs`。后续结构切片继续遵守：

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
