# 2026-08-04 D0 Doctor intent split source closure

状态：日期化 source-only 完成证据

适用范围：本地 `next`；D0 implementation closure commit
`3c2eeea32c71438b2ac24f962017597ce1ed4218`

最后复核：2026-08-04（Asia/Taipei）

失效条件：Doctor command/DTO、child environment、canonical asset/Gateway resolver、Skill route
repair owner、frontend caller、runtime mutation inventory 或本文绑定的 source evidence 发生实质变化时重审。

本文所在 evidence-only commit 只封存已完成的 implementation candidate、focused checks、独立审查与
exact-SHA source gate；它不声称自身是被测试的 implementation SHA，也不建立 artifact、installed/live、
provider/Science/SSH、signing、notarization、public release 或用户数据结论。

## 1. 结论

D0 已在 source-test 层关闭 Post-H4 摸排发现的 Doctor 产品信任 HIGH：原先一个字符串 command
同时执行诊断和第三方 Skill route reconcile，现在拆为两个独立 intent。

- `run_doctor_read_only` 只读 canonical v4 config，返回 typed
  `schema_version/intent/status/message`；旧 schema 只报错，不迁移、不写、不 chmod。
- Doctor script 与 Gateway resolver 不消费父进程 `CSSWITCH_REPO` / `CSSWITCH_GATEWAY_BIN`；child
  使用 `/bin/bash`、清空继承环境，只注入固定工具路径、canonical config/Science/Gateway 路径和脱敏
  状态，并强制关闭真实 Science HOME 检查。
- `repair_skill_route` 是独立显式 command，也是 Doctor 区域唯一取得 `HostBridge` mutation lease
  的 owner；它可失效 marker 或通过一次性 control sidecar 同步第三方 Skill route，但不会为了修复
  启动受管 Science 或正式 Gateway service。
- frontend 两个按钮各自只提交一个 IPC intent，只按 typed result 渲染，不解析 message 决定控制流，
  不串联诊断与修复。

本窗口不选择新的 sole NEXT，也不执行 Q0、O1-A、F1 或其他后续阶段。下一步必须在全新窗口按最新
源码执行 Post-D0 只读重新基线。

## 2. Implementation lineage

- `68471685512dbe04552deb83c0fce43b21dda37b`：初始 intent split、typed DTO、frontend、tests、
  inventory、ChangeRecord 与稳定文档；
- `02a3da7f5ba8d292a8019acb23d9501a50d5fa50`：同步 source-evidence Rust test identity 数量；
- `dc3382d9d58e9513c4aba637b0602835509da61c`：修复首轮架构分组旧称；
- `f8a61d7673b4a7f560a0090568a513564d482215`：隔离 Doctor child environment，并补 hostile-parent
  regression 与治理谱系；
- `5364342170a1b3a360ab5447ac5c5e4f088feec9`：Doctor-specific script/Gateway canonical resolver，
  关闭 pre-resolution override；
- `3c2eeea32c71438b2ac24f962017597ce1ed4218`：收紧 mutation inventory 与 D0 implementation
  ChangeRecord exact footprint，作为最终 source candidate。

## 3. Focused 与治理验证

最终候选及其直接前序修复实际通过：

- Rust Doctor focused：4 passed、0 failed、1 approved ignored；parent test 在隔离 HOME 中执行被目录批准
  的 child identity；route reconcile focused：1 passed；
- frontend Doctor intent：3 passed；
- runtime mutation inventory + Skill boundary + document governance：24 passed；最终 inventory/boundary
  复核 20 passed；
- source observation size/count：2 passed；quality `metadata` 与 `impact-pr`：PASS；
- `test/test_ops_scripts.sh`：ALL PASS；`cargo fmt --check` 与 `git diff --check`：PASS；
- D0 implementation ChangeRecord 的 27 个 `changed_paths` 与基线
  `291bd43a92998584a88c13d5353c3cf703bee521..3c2eeea32c71438b2ac24f962017597ce1ed4218`
  exact diff 路径集合双向一致。

## 4. Clean-context independent review

所有正式 reviewer 均使用 `fork_turns="none"`、`gpt-5.6-sol high`，并按
`reviewing.md` 报告四级计数。审查过程中发现并闭合：

- UI/架构遗留 Doctor-reconcile 旧称；
- hostile parent 可开启真实 Science HOME 检查的 HIGH；
- 通用 Gateway resolver 在 child `env_clear` 前消费 override 的 MEDIUM；
- ChangeRecord path omission/overinclude 与 inventory mutation owner 等 LOW。

最终 exact-candidate review 绑定
`3c2eeea32c71438b2ac24f962017597ce1ed4218`，结论为
`clean-context: YES`、`PASS`，`BLOCK/HIGH/MEDIUM/LOW = 0/0/0/0`；并明确完整 source gate 当时仍
`NOT-RUN`，没有借用旧 SHA seal。

## 5. Exact-SHA source gate

正式入口：`bash test/run_all.sh --output-root /private/tmp/csg.FTMmuP`

- candidate：`3c2eeea32c71438b2ac24f962017597ce1ed4218`；
- run：`79a6b4ca17076464e4bbd02ebbdd42cd`；runner exit `0`；aggregate `PASS`；
- 15/15 result 与 15/15 observation 均 PASS；1380 executed、1336 passed、0 failed、
  44 approved ignored、0 skipped/todo/not-run；
- clean-commit snapshot：514 entries、12081151 bytes，`head_sha` 精确等于 candidate；
- completion seal / run manifest / evidence manifest / source snapshot SHA-256：
  `2fbd47c0a3a462963ecbf9d1f128e752eb6fbd0d2e89e7cee7baab14813cecbd`、
  `35469ee108d45c4ea249ec523144a0ad65e7d9663f939fdb5cb7faafdee0bdfc`、
  `252fedd77fbe01bf2a031721d845618c457456675a39c24ae715b0521c7d8b2b`、
  `c37da046ea3953229ad9c840eebf549e4d9e561e06b64036a764679a7b512d5c`。

主 worktree ignored data 导致的 snapshot preflight failure、旧候选 `6847168` 的 13/15 failed run，以及
随后被 reviewer 修复失效的 `02a3da7` PASS run 均未用于最终 closure，也未与本 run 混合。

## 6. 停止与未验证边界

D0 到此停止产品实现。source、artifact、installed/live、signing、release 必须严格分层：本证据只支持
source-test closure；本窗口没有构建或替换 App/DMG，没有真实 provider/Science/SSH 测试，没有签名、
公证、push、tag、release，也没有读取真实凭证、Keychain、SSH 私钥或用户 Science 数据。
