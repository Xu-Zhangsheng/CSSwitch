# 当前重构路线与证据缺口

状态：当前；唯一工程路线；验收层级和执行合同以[生产链路验收](../../docs/operations/real-machine-acceptance.md)为准

最后复核：2026-08-14（Asia/Taipei）

审计基线：最初 clean 的 `next@cfc4008a64d9ef41a9e4238507f85b52095f7c1f`

失效条件：production owner / caller、候选 source、source-gate 状态、artifact identity、Science / Gateway runtime、Provider capability、质量元数据、签名或公开 Release 任一相关事实改变时，受影响条目立即失效并须实时重审。

本页是唯一当前路线，只保存“现在到哪、还缺什么、下一步是什么”。稳定机制放在 `docs/`，
exact SHA / run / artifact / 环境的历史证据放在日期化 audit/evidence；旧阶段编号不能提供当前
NEXT、实施授权或验收结论。

## 一句话判断

重构已经进入**后半程**：核心 runtime owner、事务、恢复、Science host 边界和负向清理已经
成形；reviewer-repair candidate `50820da` 的 canonical 15-suite seal 与 fresh clean-context review 均为
`PASS`；据此生成的 metadata-promotion 候选尚未取得自己的 exact-SHA seal/review。
跨 owner mutation receipt、Science 三套控制执行面、Skill Phase 3
和所有下游动态证据仍未闭合。因此不能称“整体完成”，
也不是“刚开始”。

## 已完成的结构

- Desktop、Gateway、Codex network 与 Skill package 已形成独立 Cargo 边界；Desktop build 会精确
  staging Gateway sidecar，Gateway 的普通服务、Codex auth、Skill MCP 与 Science control 入口已分开。
- `AppState`、`Lifecycle`、`Config`、typed receipt / journal / manifest 已形成当前 runtime ownership
  骨架；Gateway uncertain-stop、四类 mutation domain、Tauri production 注册面和 exit hook 都有
  明确 owner。
- 未注册、未编译的旧 Skill Manager 和旧 profile-switch writer 已删除；one-click durable journal
  已移动到 private `one_click/transaction.rs`。
- cooperating authority writer 与 managed-launch writer 已进入共享 authority fence；durable
  compensation 已覆盖 per-target replay、authority restore、Gateway cleanup、prior Science restart
  和 snapshot cleanup。
- `ScienceHostAdapter` 已类型化 launch encoding、health/listener proof、managed receipt 与 stop surface。
- Skill 扩展面已完成 Phase 2 inspect-only、in-memory package adapter；它不等于 plan、apply、安装或
  runtime 可达。

## 部分完成，不能升级表述

- `runtime/proxy_lifecycle.rs`、`runtime/science.rs` 和 authority snapshot 仍大量使用 `include!`；
  这是物理分文件，不是完整语义模块封装。
- one-click façade仍协调 prior stop、authority、Gateway、Science phase、route 与 finalize；
  `AuthorityTransaction` 并未拥有全部 ordering / journal / compensation policy。
- Desktop bounded Science control runner、Skill package 的 `claude-science url` runner、Gateway
  Science HTTP control 是三套职责不同但 primitive 重复的控制面；Desktop route reconcile 仍有
  裸 `.output()`，缺统一 absolute deadline、process-group 与 bounded output 合同。
- Desktop 与 Gateway 共同读取同一 provider contract JSON，但仍各自定义类型和验证逻辑；共享
  digest 能防字节漂移，不能防解释逻辑漂移。
- 大文件不是单独 blocker。只有能形成新的类型 owner、visibility boundary 或 failure contract 时才拆；
  不按行数机械切分 `commands/runtime/tests.rs`、`config.rs` 或 protocol parser。

## 当前证据矩阵

| 层 | 当前状态 | 解释 |
|---|---|---|
| Current source candidate | `PENDING` | 当前 metadata-promotion 候选以实时解析的 clean `next` HEAD 为准；它是已验收 `50820da2fabed1b75f7dd7e5cf9bb910f02d0bbb` 的后继，本页不预写尚未生成的提交 identity |
| Canonical source gate | `NOT-RUN` on current metadata-promotion candidate | `50820da` 的 exact gate 在 run `cdb5aea62b19f8e8768de42058a01eee` 为 `PASS`；metadata promotion 后必须在新 SHA 重跑，旧 seal 不得继承 |
| Fresh clean-context completion review | `NOT-RUN` on current metadata-promotion candidate | `50820da` 的 clean-context review 为 `PASS` 且无 findings；metadata promotion 后必须换用新的 reviewer，旧 review 不得继承 |
| Exact artifact | `NOT-RUN` | 本轮未获构建授权 |
| Isolated / installed / authorized live | `NOT-RUN` | 本轮未获 runtime、Science、Provider、Skill、SSH 或账号授权 |
| Signing / notarization / public release | `NOT-RUN` | 不是 source refactor closure 的 blocker；只有产品/发布 ready 才进入 |

历史 `18a67881` 的 artifact、Science 0.1.25 adoption、installed smoke 和 Provider 分项只保留为
历史 exact tuple。Claude Science 官方当前文档已进入 0.1.27，当前 source / artifact 对该版本的
兼容性没有验证。

## 仍开放的工程缺口

### P0｜事实与 source closure

- `SNAPSHOT_DIRTY` 已定位：普通 Git status 不显示 ignored 路径，而 source snapshot 会对 checkout
  目录身份做 no-follow 稳定性检查；复用主 checkout 中约 74 万 build/runtime ignored 路径会导致
  捕获窗口漂移。同一 SHA 在零 ignored 隔离 worktree 已通过 SNAPSHOT；不修改 gate，也不清理用户数据。
- R0 inventory 与 ChangeRecord 已依据 `50820da` 的 exact gate/review `PASS` 对齐为
  `complete/complete` 与 `confirmed`；96 个必需 characterization identity 全部 executable，和
  ignored/skipped 的交集均为空。metadata-promotion commit 仍必须取得自己的 exact gate/review。
- 四个 trusted-source-gate 旧 bug 已迁移到 `source-reproduced/source-fixed-product-pending`；Gateway
  recovery、Science reattach、tool-arg 三份记录中的已消失 source path 已替换为当前 owner。
- 内置模板与 preview 的旧 `0.8.1 limited` 已替换为版本中性、按当前 capability gate 限界的精确声明，
  并由后端 Rust 与 frontend source test 绑定一致文案。

完成条件：machine metadata、用户可见 copy 与当前 source 互不矛盾；clean exact candidate 获得固定
15-suite completion seal 和 fresh clean-context completion review。两者都 PASS 才能写
`SOURCE-GREEN`。

### P1｜Science control bounded primitive

- 先冻结 Desktop、Gateway、skill-package 三者职责，再抽取或统一 absolute deadline、bounded
  stdout/stderr、process-group cleanup 与 typed timeout。
- 最窄首刀是替换 `runtime/skill_install_bridge.rs` 中 route reconcile 的裸 `.output()`；不在同一
  slice 开始 Skill plan/apply。

完成条件：production caller 使用同一可审计 primitive 或明确不同 contract；timeout、output cap、
child cleanup 与 replacement fixture 闭合。

### P2｜非 one-click mutation receipt

- 优先处理 set-mode、set-settings、Codex/profile 操作中“先 stop、后 config commit、失败不恢复 prior
  runtime”的窗口。
- native exit 重复事件/generation、Gateway 外部 bridge lease、Skill route mutation 和其他共享事务
  缺口继续以 runtime mutation inventory 为机器权威。

完成条件：每个目标 operation 有 typed plan、durable receipt、明确 inverse / no-inverse、fresh replay
和 replacement-preserving fixture；不把全部 mutation 强行塞进 one-click journal。

### P3｜Skill Phase 3

- 单独设计并实现 inspect → plan → confirm → apply、跨进程 ledger、OPERON attach 与 final-response
  事务边界。
- 当前 Phase 2 inspect-only 不得写成产品安装、MCP lifecycle 或 Science attach 已完成。

### P4｜语义收尾

- 对 test-only `runtime/transaction.rs`、operation vocabulary 和 dormant profile preset-sync 明确选择
  “接通”或“删除”；不能无限期以 `allow(dead_code)` 保存模糊意图。
- provider contract 适合抽共享 crate 或生成 schema；先定义单一解释 owner，再迁移两套 validator。

### P5｜下游产品证据

只有 source closure 后并分别获得授权，才按 exact source → exact artifact → isolated-live →
authorized-live 推进；installed、signing/notarization 与 public release 仍是额外独立层。真实 API Key、
OAuth、Keychain、SSH key、账号数据库和真实 Science data-dir 不因本路线获得访问授权。

## 唯一立即 NEXT

下一任务只做 **P0 source closure 验收**：冻结当前 metadata promotion，在零 ignored 的隔离 checkout
对实时解析的同一 clean exact HEAD 执行 canonical 15-suite gate，再换用新的 clean-context reviewer
对该 SHA 做 completion review。只有 completion seal 与 review 都 PASS 才把该候选写成
`SOURCE-GREEN`；不得跳到
artifact/live，也不得清理复用主 checkout 的用户 ignored 数据。

## 文档退役状态

- 审计基线的 167 份 tracked Markdown 中，没有一份满足“整份删除”的 lifecycle 条件。
- 11 个 tracked 兼容指针仍有有效调用者或 release 条件，继续保留；它们不是重复权威正文。
- 两份 ignored 临时 handoff 已因 HEAD/checkpoint 变化且任务落地而满足自身失效条件，已在本轮
  退役；Git 不跟踪它们，删除后不能从本仓库历史恢复。
- 旧 R3–R11、R4/R5、S7、Post-D0/Post-Q0 只是历史 audit/evidence 标签；后来文档中的 Phase 1/2/5
  也只表示其绑定 slice，不是当前 NEXT。

完整审计方法、文件分类、Science/CSSwitch 链路和发现见
[2026-08-14 全仓审计](../../docs/audits/2026-08-14-next-repository-docs-architecture-review.md)。
