# 2026-08-04 Post-D0 只读重新基线

状态：日期化 source-only 架构与治理再基线

适用范围：本地 `next@4f03d9e8845ed42d80d1ffb9d58b5481bbe4bba4`；D0 implementation
candidate `3c2eeea32c71438b2ac24f962017597ce1ed4218`

最后复核：2026-08-04（Asia/Taipei）

失效条件：quality lineage、ChangeRecord/source-candidate/release-candidate 合同、Doctor intent、
one-click/history/config/boot read model、Science runtime adoption/provenance，或本文绑定的 source HEAD
发生实质变化时重审。

本文只记录当前源码、机器治理文件、本地 Git identity 与只读 validator 结果；它不修改产品代码，
不授权 Q0-A 实现，也不建立 artifact、installed/live、provider/Science/SSH、signing、notarization
或 public release 结论。远端 tag、GitHub release 页面与附件本轮均 `NOT-RUN`。

## 1. 结论

D0 的 source-only closure 保持成立：Doctor read-only 与显式 Skill route repair 是两个独立 typed
intent，前者不取得 mutation lease，Doctor child 使用 canonical path 与清空后的固定环境，frontend
不串联两个 intent，也不按 message 决定控制流。本轮没有发现新的 Doctor BLOCK/HIGH。

从 D0 后最新源码重新比较 Q0、O1-A、F1 与相邻候选后，本次唯一建议 NEXT 是：

**Q0-A Current release baseline and source-candidate lineage。**

理由不是继承旧排序，而是当前只读验证得到一个可执行的治理 HIGH：仓库已有本地 annotated
`v0.8.4` tag 与对应 release evidence，但机器 lineage 和 validator 仍停在 `v0.8.3 <- v0.8.2`；
`metadata` 可以 PASS，而 `impact-release` 已因从过旧 `v0.8.2` 基线重算全部历史变化而实际 FAIL。
这使当前 release-impact/promotion 路径不可用。相比之下，O1-A、F1、config 与 Science provenance
均有真实 MEDIUM，但当前 typed handoff、exact identity、mutation lease、readback/CAS 或 fail-closed
边界尚未被推翻。

本轮 findings 计数为 `BLOCK/HIGH/MEDIUM/LOW = 0/1/6/0`。Q0-A 完成后必须停下重新基线；本文
不预排随后由 F1-history、O1-A 或其他候选中的哪一项接续。

## 2. 实时现场与证据层

- branch：`next`；HEAD：`4f03d9e8845ed42d80d1ffb9d58b5481bbe4bba4`；治理编辑前 source
  worktree clean；当前仅有本文、`docs/README.md` 与 `.agents/context/known-issues.md` 三项治理候选；
- D0 implementation：`3c2eeea32c71438b2ac24f962017597ce1ed4218`；evidence-only commit：
  `4f03d9e8845ed42d80d1ffb9d58b5481bbe4bba4`；
- D0 evidence 记录的 clean-context review 为零 finding，exact implementation SHA 的 15-suite
  source gate 为 15/15 PASS；本轮没有把 evidence-only HEAD 冒充该 tested candidate；
- 当前本地 `v0.8.4` annotated tag object 为
  `9c478c092e4b6f4a0bb80ca1f215bb6e60a87014`，peeled commit 为
  `37d5cfb6600a0022d5e1bbbe9a7e181a917b12dd`；只确认本地 Git identity 与已检入 evidence，
  没有远端/public 回读；
- source inspection：完成；治理编辑前 clean worktree 上
  `validate_quality_metadata.py metadata`：PASS，`validate_quality_metadata.py impact-release` 因旧
  `v0.8.2` base 引出的历史删除、未知 policy path 与无 matching ChangeRecord 等错误 FAIL；治理候选
  存在时的 reviewer 复跑还会按预期增加 dirty-worktree failure，二者不混为同一原因；
- source completion gate：本轮 `NOT-RUN`；artifact、installed/live、真实 provider/Science/SSH、
  signing/notarization、public release：全部 `NOT-RUN`。

## 3. 当前 findings

### 3.1 HIGH｜Q0：release-impact 基线与 source promotion 谱系已实际失效

当前机器事实并非只少一份说明文档：

- `quality/release-lineage.v1.json` 仍声明 `version=v0.8.3`，previous release 仍为 `v0.8.2`；
- `test/quality/validate_quality_metadata.py` 仍把质量记录 namespace 固定为 `v0.8.3`；D0 等
  `v0.8.4` 发布后的新 source change 也继续进入 `quality/changes/v0.8.3/`；
- `ChangeRecordV1` 表达 intent、risk、changed paths 与 test impact，但不绑定 comparison base、exact
  candidate、run/seal/snapshot 或 release edge；impact policy 可由任一历史 active
  matching record 覆盖路径，不能证明本次 change set 自己建立了新记录；
- 仓库已经有 `release-candidate.v1` 与 `validate_release_candidate`，能验证 release-profile run 的
  exact candidate、previous release 与 completion seal；因此 Q0-A 应复用该合同，而不是重造第二套
  release candidate；
- 当前 source gate 固定产生 source profile 与 exact-SHA completion seal，不是 release promotion
  run。source PASS 仍然只证明 source，不等于 artifact 或 release。

只读实跑中 `metadata` PASS，但 `impact-release` 从旧 `v0.8.2` base 看见跨两次发布和当前 `next`
的累计变化，因删除、未知 policy path 与无 matching ChangeRecord 等多项错误 FAIL。这个 FAIL 是
当前发布影响门不可用的直接证据；不能用 ordinary source green 绕过。

### 3.2 MEDIUM｜F1-history：一个用户动作仍跨两个 destructive IPC

`restore_history_choice_command` 在独立 `Destructive` lease 中 exact-stop 受管 Science，随后执行
多文件 virtual-login 更新与独立 marker 写入；backend 成功结果明确保持 stopped。frontend 收到成功
后释放 busy，再自动调用第二次 `one_click_login`。因此 stop、credential/marker publication 与第二个
operation 是不同 failure boundary；中断可留下“历史已改、Science 已停、第二次 one-click 未完成”。

exact managed receipt、typed quiescence 与 candidate/config recheck 仍会拒绝未知 runtime 或漂移，
且没有证据显示其他历史被删除，所以维持 MEDIUM。未来 F1 应优先拆成窄的 history intent boundary，
不能以机械拆 frontend controller 代替修复。

### 3.3 MEDIUM｜F1-read-model：读取、ack 与 boot publication 仍不对称

`build_get_config` 读取 `pending_notice` 后调用 `config::update` 清空它；名义 read command 因而会写
config，没有独立 typed/idempotent ack。boot failed/attention 有 event 与一次性 pull，attention pull
还会 `take()`；Ready 只写进程内 `BootState::Ready`，没有同序列的统一 snapshot/event。

H4 typed finalize classifier 仍 fail-closed，过期 history reference 也会被 backend 拒绝；当前风险是
时序相关的展示、重复/吞掉 notice 与恢复解释性，维持 MEDIUM。

### 3.4 MEDIUM｜O1-A：entry decision 仍晚于 branch-specific effect

command 在 `Destructive` lease 内先重放 interrupted finalize、执行 typed interrupted Gateway recovery，
再进入 runtime one-click；runtime 又重放 finalize，并在 healthy/cold/unknown decision 前执行 system
SSH prevalidation、stub capture 与 pending authority cleanup。真正 runtime probe、binding/login decision
随后才形成，healthy reopen 因而继承 cold/recovery preparation 的失败面。

Gateway terminal handoff、完整 journal/binding check、mutation lease 与现有 CAS 仍 fail-closed，没有发现
新的错误 success 或不受控 data mutation，因此维持 MEDIUM。O1-A 仍是后续 cold coordinator 演进的
前置，但不是 Q0-A 的前置。

### 3.5 MEDIUM｜one-click owner 与 coordinator 仍未收敛

command 与 runtime 共同拥有 entry recovery；`one_click_login_with_options` 仍同时编排 entry、SSH、
cleanup、Science stop、authority snapshot、history/login、Gateway、Science launch/DB recovery、route、
finalize 与 compensation。问题是维护原因和 failure matrix 集中，不是文件行数。H1-H4 提供的 typed
handoff、receipt、journal 与 readback 不应被合并成万能事务，也不能在 Q0-A 顺手重构。

### 3.6 MEDIUM｜config commit 与跨进程并发边界仍存在

config update 有 process-local serialization、expected-bytes pre-rename check 与 atomic replacement；
`AtomicRollbackUncertain` 也能在 writer 内部识别。但普通 orchestration 仍多把 writer error 字符串化，
且没有通用跨进程 writer policy。H4 readback 能阻止 UI 猜测 applied，不能把不确定 commit 还原成
确定结果。本缺口应在 runtime owner 稳定后独立处理。

### 3.7 MEDIUM｜Science identity/rollback 不等于 update provenance

Science updater candidate 已经过固定路径、owner/mode、Mach-O/embedded metadata 与 SHA-256 内容寻址
snapshot 校验；`ScienceRuntimeIdentity` 与 managed launch/rollback receipt 能回答“实际控制哪个
executable”，healthy daemon 不会因为发现新 candidate 被强制重启，cross-runtime environment 已暴露
时也会拒绝自动重启 predecessor。

仍缺少通用 predecessor/candidate observation、normalized allowlisted diff、adoption/defer/reject
decision 与 launch/finalize milestone ledger。因此 U1 保持 MEDIUM；它必须复用现有 identity/receipt，
且绝不能读取或 diff 真实账号、组织、对话、project、environment 或 opaque Science data。

## 4. 候选重新比较

| 候选 | 当前严重度与可达性 | 本轮判定 |
|---|---|---|
| Q0-A release baseline + source-candidate lineage | HIGH；维护者 release-impact/promotion 入口已实际 FAIL | **sole NEXT** |
| F1-history intent boundary | MEDIUM；直接产品可达，存在 stop/多文件更新/第二 IPC 中断态 | Q0-A 后重新比较，不自动接续 |
| O1-A typed entry decision | MEDIUM；产品可达，影响 branch owner 与演进失败面 | 保留候选，不进入实现 |
| F1 read-model/boot | MEDIUM；产品可达，影响 notice/boot 展示一致性 | 与 F1-history 分开评估 |
| one-click coordinator / config policy | MEDIUM；产品可达但现有 guard 仍 fail-closed | 后续独立 slice |
| U1 Science update provenance | MEDIUM；runtime adoption 可达，现有 identity/rollback 安全边界仍成立 | 独立 lane，不并入 Q0-A |

`F1` 不再视为一个不可拆的大 stage：history intent 与 read-model/boot 有不同 owner、失败边界和停止
条件。Q0-A 后的新再基线必须重新比较这些窄候选，不能沿用本表顺序。

## 5. Sole NEXT：Q0-A Current release baseline and source-candidate lineage

### 范围

1. 只修改 `quality/`、`test/quality/`、必要的 source-gate wiring 与对应 operations/context/audit；
   不修改 Desktop/Gateway/Skill/product runtime 行为；
2. 将机器 previous public release 与本地冻结 `v0.8.4` tag identity 对齐，并用显式 development
   source line 表达 `next`，不得把新 source 冒充已公开 `v0.8.4` 或预先宣称一个新 release；
3. 区分继续表达 intent/risk/test impact 的 mutable active ChangeRecord，与 evidence-only commit 中
   no-clobber 发布的 immutable `SourceCandidateRecord`。后者至少绑定 comparison base、exact candidate
   SHA、当前 change IDs/changed paths、run manifest、completion seal 与 source snapshot/evidence digest；
   implementation SHA `C` 的 reviewer verdict/count 只由日期化 audit 记录，不进入没有机器
   attestation authority 的 record；evidence SHA `E` 的治理复审是任务外部 gate，只在最终任务结果中
   报告，不写回 `E` 或本文，避免为了记录复审再产生递归 evidence commit；
4. current-diff validator 必须要求本次新增/更新的 matching record，不能由历史 active path coverage
   兜底；已公开版本 namespace 不能被不同 source 继续复用；
5. 复用现有 `ReleaseCandidateV1` validation，补出并验证
   `SourceCandidateRecord -> ReleaseCandidate -> ReleaseEvidence` 的严格 promotion edge；本阶段只以
   fixtures/negative tests 证明合同，不生成真实 release evidence；
6. 将稳定语义同步到 `docs/operations/quality-kernel.md` 与 `docs/operations/release.md`，日期化结果进入
   新的 audit/evidence；CHANGELOG 仍只记录真实 release。

### 不变量

- D0 Doctor commands/DTO、child environment、frontend intents 与 source evidence 不变；
- H1 terminal handoff、H2 prior-stop、H3 finalize、H4 consumer projection，以及 O1/F1/config/Science
  runtime 行为不变；
- source completion seal 只证明 exact source candidate；release candidate 仍必须绑定 release-profile
  run，artifact/installed/live/signing/public evidence 不能由 source record 推导；
- evidence-only `SourceCandidateRecord` 所在 commit 不冒充其引用的 tested implementation candidate；
- tag、public release、CHANGELOG 和 release evidence 不因开发线 source green 自动改变；
- 不读取真实 credential、Keychain、SSH 私钥、账号数据库或用户 Science 数据。

### 排除项

- 不进入 O1-A、F1-history、F1 read-model/boot、cold coordinator、config concurrency 或 U1；
- 不修改产品版本、不构建 App/DMG、不替换 installed App、不运行真实 provider/Science/SSH；
- 不签名、公证、push、tag、创建 release、上传或回读公开附件；
- 不把旧 audit 改写成当前权威，不删除受保护 ignored handoff/user data。

### 验证门

1. focused schema/producer/validator tests 至少覆盖 stale `v0.8.2` base、冻结 `v0.8.4` identity、
   released-version source reuse、历史 active record 兜底、candidate/change-set/run/seal/snapshot/evidence
   mismatch、source-to-release 跨层误提升；所有 tamper/partial/replay 必须 fail closed；
2. `metadata`、显式 target-ref `impact-pr`、`impact-release`、inventory/catalog/schema/document governance、
   format 与 `git diff --check` 全部实际 PASS；`impact-release` 不得继续使用旧 `v0.8.2` base；
3. clean implementation candidate 上执行 exact-SHA 15-suite `GATE-SOURCE`，15/15 PASS；任何
   `ENV-BLOCKED`、`NOT-RUN`、旧 SHA seal 或 mixed run 都不算完成；
4. `fork_turns=none`、`gpt-5.6-sol high` clean-context independent review 对 exact candidate 给出
   `BLOCK/HIGH/MEDIUM/LOW = 0/0/0/0`；修复后必须换新 clean-context reviewer；
5. 明确使用两 SHA 闭环：implementation SHA `C` 完成上述 focused/impact/15-suite/review；随后
   evidence SHA `E` 只 no-clobber 发布绑定 `base..C` change set 与 `C` 的 manifest/seal/snapshot/evidence
   digest 的 record。`E` 必须实际通过 record validator、document governance、`git diff --check` 与
   新的 clean-context 治理复审；`E` 不冒充 tested candidate。该治理复审结论只作为外部完成 gate
   报告，不写回 `E`；若复审要求修复则形成新的 evidence candidate 并换新 reviewer，直至外部 gate
   PASS 后停止；
6. 获得该阶段单独授权后才可本地 commit；push、tag、release 仍分别禁止。

### 停止条件

Q0-A 的 implementation SHA `C`、source gate、独立审查与 evidence SHA `E` 的 source-candidate
record/validator/治理复审全部闭合，并完成经授权的本地治理提交、attributable cleanup 与 clean
worktree 后立即停止。随后只做一次新只读再基线并选择一个新的 sole NEXT；不得自动进入 F1、
O1-A、C1、U1 或 release 操作。

## 6. 本轮停止边界

本轮只交付重新基线与治理入口；不执行 Q0-A，不运行 source completion gate，不修改产品代码，也不
建立 artifact、installed/live、signing 或 release 结论。治理文档经 clean-context 独立审查与最小
文档门验证后可按本次授权本地提交，随后停止等待下一次实施授权。
