# Quality kernel v1

`quality/` 是机器事实源，不是项目进度正文：

- schema 定义记录的可接受形状；
- requirements 与 production-path policy 定义必须覆盖什么；
- change / bug records 分别记录变更影响和已知问题，`open-not-fixed` 不能写成 fixed；
- test catalog 与 release gates 绑定固定 suite、entrypoint、identity、环境和聚合规则；
- lineage 区分 previous release、development source、source candidate、release candidate 与公开 release，不能把 source evidence 提升为 release evidence。

`quality/release-lineage.v1.json` 的 `previous_release` 是下一候选的冻结公开 comparison base；`development_source` 明确给出开发线、ChangeRecord namespace 与 record version。已公开 `quality/changes/v*` namespace 只读，发布后工作只能进入 `quality/changes/next/`。impact coverage 只接受 `base..candidate` 本次新增或更新的 matching ChangeRecord，历史 active record 不能替当前变化兜底。

production path 的 rename / copy / delete 默认继续 fail-closed。只有 `production-paths.v1.json` 中按精确文件或目录前缀登记的 `retired_path_deletions` 可以删除；每项必须绑定当前 active ChangeRecord、仍满足原 production policy 的 suite / gate 覆盖、候选中已不存在且不能与其他退役路径重叠。冻结 retirement group 只有两个：Skill Manager group 的两个 policy declaration 与其唯一 ChangeRecord introduction 的 12 个 `D` Desktop manifest；P4 dead-transaction-vocabulary group 的单一 `runtime/transaction.rs` policy declaration 与其唯一 introduction 的 `D transaction.rs`、`M runtime/mod.rs`、`M runtime/operation.rs` Desktop manifest。每组都拒绝并行 Desktop `A/M/R/C/D`、缺失路径、缺失 introduction、delete/re-add history、第三条 policy declaration 或错误 binding；这防止把 delete + add/copy 伪装成 negative refactor。该机制只表达已独立证明不在 production registration / caller graph 中的源码退役，不把目录排除出新增或修改路径的 fail-closed 检查。

clean exact candidate 的完整 `GATE-SOURCE` PASS 后，`python3 -m test.quality.source_candidate create --evidence-root <GATE_OUTPUT_ROOT> --candidate <SHA>` 从该 output root 中唯一且 state/evidence identity 一致的 sealed PASS run 读取 public manifests 与 private snapshot，并以 no-clobber 方式生成 `quality/source-candidates/<SHA>.json`。记录绑定 v0.8.4 tag identity、exact candidate、canonical Git change set、current change IDs、run manifest、completion seal、source snapshot 与 evidence manifest digest；它不保存 reviewer 结论，也不建立 release evidence。

promotion 顺序固定为 `SourceCandidateRecord -> ReleaseCandidateV1 -> ReleaseEvidenceV1`：ReleaseCandidate 必须引用同一 candidate/base 的 source record，并仍要求独立 release-profile PASS；ReleaseEvidence 再绑定同一 release candidate、artifact manifest 与 public receipt。source record 不能直接充当后二者。

当前完整 source/unit 操作入口、输出目录、安全重跑和结果词汇只在[自动测试与证据判定](testing.md)维护。`GATE-S0-LEGACY` 已 retired；只有递归验证通过的 `GATE-SOURCE` completion seal 才能建立 `RUN-EVIDENCE-GREEN` 与 `SOURCE-GREEN`。

source claim 不证明 app / DMG、installed runtime、live provider / Science、签名、公证、Gatekeeper 或公开附件。更高证据层以[发布流程](release.md)、[真机验收](real-machine-acceptance.md)和 dated evidence 为准。

生产路径变化时必须同步 machine record 和测试影响；metadata、impact 或 lineage 漂移作为当前问题进入 [known issues](../../.agents/context/known-issues.md)，不在本文复制阶段进度。历史 RUE 实施过程与旧 gate 规格从 Git 和 dated audit 追溯。
