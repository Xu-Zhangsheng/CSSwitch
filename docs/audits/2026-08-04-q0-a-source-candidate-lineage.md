# 2026-08-04 Q0-A source-candidate lineage closure

状态：日期化 source-only 完成证据

适用范围：本地 `next`；Q0-A implementation candidate
`016b5882bf82860ca4d5cd86f1c9955083010f60`

最后复核：2026-08-04（Asia/Taipei）

失效条件：release lineage、ChangeRecord current-diff coverage、SourceCandidateRecord、source gate、
ReleaseCandidate / ReleaseEvidence promotion contract 或本文绑定的 source evidence 发生实质变化时重审。

本文所在 evidence-only commit 只封存已完成的 implementation candidate、独立审查和 exact-SHA source
gate；它不声称自身是被测试的 candidate，也不建立 artifact、installed/live、provider/Science/SSH、
signing、notarization、Gatekeeper、public release 或用户数据结论。

## 1. 结论

Q0-A 已关闭 Post-D0 基线中的 release/source lineage 治理 HIGH：

- previous release 绑定本地 annotated `v0.8.4` tag object
  `9c478c092e4b6f4a0bb80ca1f215bb6e60a87014` 及 peeled commit
  `37d5cfb6600a0022d5e1bbbe9a7e181a917b12dd`；`next` 以独立 `v0.8.4-dev`
  development source identity 表达，不冒充新 release；
- post-v0.8.4 active ChangeRecord 使用 `quality/changes/next/`，已发布 namespace 保持 byte-for-byte
  不变；production path coverage 只接受本次 base..candidate 中新增或更新的 current record，不能由
  历史 active record 兜底；
- immutable `SourceCandidateRecord` 绑定 comparison base、exact candidate、canonical change set、
  current change IDs、run manifest、completion seal、source snapshot 与 evidence manifest digest；
- promotion 顺序固定为
  `SourceCandidateRecord -> ReleaseCandidateV1 -> ReleaseEvidenceV1`。release candidate 仍须版本前进并
  绑定独立 release-profile gate；source record 不能升级成 artifact 或 public release evidence。

本窗口不进入 O1-A、F1、config、Science provenance、U1 或 release。下一动作只能是一次新的只读再基线，
由实时源码重新选择 sole NEXT。

## 2. Git lineage 与 immutable record

- main 上唯一待处理的 QR commit `010a69aa9ffc2696f927dd1fd578b31c69fb2d6a` 已通过 merge commit
  `0bf26e75988c53a59574bae0d894567ca7fb4d57` 纳入 `next`，没有静默排除；
- implementation candidate `C`：`016b5882bf82860ca4d5cd86f1c9955083010f60`；tree
  `7021316b77f1ac014a006318349304771f0c8676`；
- record：`quality/source-candidates/016b5882bf82860ca4d5cd86f1c9955083010f60.json`；
  SHA-256 `c03d8f3a138c3024c64e92ff1ee57a041787a459931bb3a9c220b49f92a1b45c`；
- record 包含 254 项 canonical `v0.8.4..C` change set 与 39 个 current change IDs。producer 使用
  exclusive no-clobber publication；partial write、replay、fd/path substitution、directory rebind、
  close/fsync 与 final-leaf replacement 均 fail closed。

record 所在 evidence-only commit 不写入自身 SHA；其治理复审也只作为外部完成 gate 报告，避免递归
evidence commit。

## 3. Focused、impact 与治理验证

exact `C` 实际通过：

- quality kernel + manifest focused：23 passed；source-gate contracts：69 passed；
- quality `metadata`、`impact-release` 与显式 `--target-ref main impact-pr`：PASS；
- `git diff --exit-code v0.8.4 -- quality/changes/v0.8.3`：PASS；
- `git diff --check`：PASS；
- producer 对真实 gate 双根布局执行无发布 `build_record()`：PASS，得到 254 项 change set 与 39 个
  change IDs。

## 4. Clean-context independent review

正式 reviewer 均使用 `fork_turns="none"`、clean context，并按 `reviewing.md` 报告四级计数。审查中
先后发现并修复：真实 evidence/state 双根布局不兼容、pathname TOCTOU、publication close/durability
不确定性、FAIL result 与 PASS seal 可矛盾组合、publication directory rebind，以及 post-fsync final
leaf replacement。

最终 exact-candidate review 绑定 `016b5882bf82860ca4d5cd86f1c9955083010f60`，结论为
`clean-context: YES`、`PASS`，`BLOCK/HIGH/MEDIUM/LOW = 0/0/0/0`。所有较早 candidate 及其 gate
在修复后失效，未用于最终 closure。

## 5. Exact-SHA source gate

正式入口：`bash test/run_all.sh --output-root /private/tmp/q0a-c3-gate.QxYSR5`

- candidate：`016b5882bf82860ca4d5cd86f1c9955083010f60`；
- run：`010ae4fdd0e24a0e26863ab57896dc20`；runner exit `0`；aggregate `PASS`；
- 15/15 TestResult 与 15/15 SourceObservation 均为严格 PASS；1384 executed、1340 passed、
  0 failed、44 approved ignored、0 skipped/todo/not-run；
- clean-commit snapshot：526 entries、12009647 bytes，`head_sha` 精确等于 candidate；
- completion seal / run manifest / evidence manifest / source snapshot SHA-256：
  `3e5b15842ccdd6ae70828257f5c37db05c047ad8cb876802f843c0a9b993d321`、
  `516ec8ae4b3378dfffa36e1f8244b64102cad429bbe10240686df4e9f19cfaee`、
  `6b403b72dabb8a13f253abf5dffaed2c79add13a1ffa1d692429a67ee97acf85`、
  `8ee8f5a6a1a28ab961664b53c320774ac3da93e448d816c5123c4e75376658a7`。

## 6. 停止与未验证边界

Q0-A 到此停止。本文和 immutable record 只支持 source/unit closure；本窗口没有构建或替换 App/DMG，
没有运行 installed runtime、真实 provider/账号/Science/SSH，没有签名、公证、Gatekeeper、push、tag、
release，也没有读取真实 credential、Keychain、SSH 私钥、账号数据库或用户 Science 数据。
