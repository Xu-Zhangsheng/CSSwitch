# 2026-08-04 F1-0 history boundary guard source closure

状态：日期化 source-only 完成证据

适用范围：本地 `next`；F1-0 implementation candidate
`b731c84355f78d187831e53a5b1075deb576cc3f`

最后复核：2026-08-04（Asia/Taipei）

失效条件：history restore frontend caller、backend restore DTO / stopped contract、runtime mutation
inventory、test identity、本文绑定的 source evidence 或 current decision gate 发生实质变化时重审。

本文只封存 F1-0 implementation、focused tests、独立审查与 exact-SHA source gate。它不建立
artifact、installed/live、真实 provider/Science/SSH、signing、notarization、Gatekeeper 或 public
release PASS，也不授权进入 O1-A、C1、完整 F1-A、F1-R 或其他后续阶段。

## 1. 结论

F1-0 已关闭 history restore 的直接 frontend 自动串联边界：

- `restoreHistoryChoice` 成功后只提交 `restore_history_choice`，不再自动调用
  `one_click_login`、`finalize_consumer_state` 或 `status`；
- frontend 保留 backend 的恢复结果，明确显示当前保持 stopped，并要求用户再次显式点击
  「一键开始」；
- backend restore 源码未改，既有 exact managed Science stop、post-stop recheck、credential / marker
  publication、reference rotation、窄 success DTO 与失败路径合同保持不变；
- restore 与用户随后可选的显式 one-click 仍是两个 destructive operation，没有共同 durable journal，
  不能解释成原子事务。

F1-0 完成后没有自动继承的新 implementation sole NEXT；必须先按实时源码做新的只读再基线。

## 2. Implementation 与 focused evidence

- baseline：`e93186ce993c8f1a574c4fe0897cedb490c4eb8b`；
- implementation candidate：`b731c84355f78d187831e53a5b1075deb576cc3f`；
- `node --test test/runtime_controller_history_restore.test.mjs`：6/6 PASS；成功 case 使用包含
  `choices` 的真实 backend DTO shape 并实际进入 `showHistoryRecovery`；
- `bash test/run-frontend.sh`：PASS；quality metadata、impact-pr、quality kernel、runtime mutation
  inventory 与 document governance 均 PASS；
- ChangeRecord `CHG-RUNTIME-HISTORY-BOUNDARY-F1-0`、test identity、catalog fixture hash、mutation
  inventory、current Context 与两份稳定架构正文已同步。

## 3. Clean-context independent review

正式 reviewer 均使用 `fork_turns="none"`。前两轮分别发现并闭合：

- success test 没有走真实 `choices -> showHistoryRecovery` 分支；
- mutation inventory 仍保留 frontend launch 旧措辞；
- current `known-issues` 仍保留 F1-0 待授权及自动第二 IPC 的旧事实。

最终 reviewer 绑定 staged diff SHA-256
`2632151ade0bc4081b541fa52d3cafbf6a3a9b5a4a867e240ad084d325ed17e9`，结论为
`clean-context: YES`、`PASS`、`BLOCK/HIGH/MEDIUM/LOW = 0/0/0/0`。该 fingerprint 与
`e93186c..b731c84` 的 committed binary diff 完全一致。

## 4. Exact-SHA source gate

正式入口：`bash test/run_all.sh --output-root /private/tmp/cg.SwTVVG`

- candidate：`b731c84355f78d187831e53a5b1075deb576cc3f`；
- run：`4f1ad659a1aeb767e43f7dabc5008a1b`；runner exit `0`；aggregate `PASS`；
- 15/15 result 与 15/15 observation 均 PASS；1384 executed、1340 passed、0 failed、
  44 approved ignored、0 skipped/todo/not-run；
- clean source snapshot：530 entries、12045589 bytes，`head_sha` 精确等于 candidate；
- completion seal / run manifest / evidence manifest / source snapshot SHA-256：
  `58b12cd39acef82b231c40f02b9eed13578af7b2e9fa42b575907c1d80b5a260`、
  `870ddab5b4d08f59a3bde881374f78299306ae7664bc94c1a7d2473e050ac079`、
  `138d70f430c1928334f18b97e23afa69713712de2bd4f7715a71c8205b60bc5f`、
  `8cb9fa2dee623a93405d6e31028dfb32086a1abed7198b8c5888862be4195e3e`。

首次在受限 sandbox 中运行的 run `e68107e6400263d4a30ad86c56224074` 因 loopback / 子进程限制
导致 Rust Gateway、desktop 与 Python loopback 大面积失败，exit `12`；它单独保留为失败事实，
不与正式非 sandbox PASS run 混合，也不用于 closure。

## 5. 停止与未验证边界

F1-0 到此停止产品实现。本窗口没有构建或替换 App/DMG，没有运行真实 provider/Science/SSH，
没有读取真实凭证、Keychain、SSH 私钥或用户 Science 数据，也没有 push、tag 或 release。
