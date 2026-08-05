# 2026-08-05 F1-A durable history recovery source closure

状态：日期化 source-only 完成证据

适用范围：本地 `next`；F1-A final source candidate
`c55942c23d87f17fb5add139247f97f1889a4333`

最后复核：2026-08-05（Asia/Taipei）

失效条件：history recovery schema / replay / credential before-image、Config writer authority、
Science quiescence、restore / resume DTO、runtime mutation inventory、source test identity 或本文绑定的
source evidence 发生实质变化时重审。

本文只封存 F1-A implementation、focused tests、独立审查与 exact-SHA source gate。它不建立
artifact、installed/live、真实 provider/Science/SSH、signing、notarization、Gatekeeper 或 public
release PASS，也不授权自动进入 F1-R、giant coordinator、统一 read model 或其他后续阶段。

## 1. 范围与实现

- baseline：`532af900a26d2bdb83b97713e4030e044ee15688`；
- runtime implementation commit：`e318eb29c3f95600e1c2adf118818cc42d06dcd0`；
- source identity repair / final candidate：`c55942c23d87f17fb5add139247f97f1889a4333`；
- `HistoryRecovery` 使用 typed V2 durable record 表达 stop intent/outcome、authority snapshot、credential
  write pending/published 与 terminal resume handoff；restore-only 默认保持 stopped，显式 resume 只消费
  complete-record CAS 发布的 terminal handoff；
- credential 写前保存 identity-bound 私有 before-image，crash replay 可恢复 credential / marker、转换或
  收敛 cleanup，并在完成后轮换不可持久复用的前端 reference；
- Science stop / probe 以 typed quiescence 与 exact managed runtime identity 为控制流，展示文本和
  diagnostics 不参与行为判定；
- operation fingerprint 覆盖移除 journal 后的完整 Config authority；central writer guard、checkpoint、
  finalize、replay 与 downgrade 都拒绝 sibling Config 漂移或打开的 history transaction；
- frontend 只消费私有 DTO 的 status / recovery status / action / choices，并以 readback 决定最终展示。

## 2. Focused evidence 与独立审查

implementation 候选经过多轮 clean-context review，逐次修复 config 并发覆盖、credential crash replay、
durability barrier、auth / replay 竞态、Science quiescence、cleanup / finalize DTO、私有路径投影、
inventory coverage、replay validation-to-effect window 与 downgrade bypass。最终 implementation reviewer
绑定 tracked diff fingerprint
`74912e3a45176becdcac62581159b7b18360557229076fe30f9dfc4d55832a79`，结论为
`clean-context: YES`、`PASS`、`BLOCK/HIGH/MEDIUM/LOW = 0/0/0/0`。

首次 formal gate 暴露 source-test governance 漂移后，五文件 identity repair 由全新 reviewer 绑定
fingerprint `b69bcc2d4f53e6ada62928fcf9071d952c45fe313a6d35146a0eb9f2190a3349`。该 reviewer
逐项核对 Rust desktop Cargo list 与 fixture `549/549`、15 个 catalog shared digest binding、源码边界
权威调用链，并实际得到 skill boundary `15/15`、quality focused/runtime/attempt0 `63/63`、新增
downgrade test `1/1` 与 metadata PASS；首末冻结一致，结论同为 `clean-context: YES`、`PASS`、
`0/0/0/0`。

## 3. Exact-SHA source gate

正式入口：`bash test/run_all.sh --output-root /private/tmp/g.c16F71`

- candidate：`c55942c23d87f17fb5add139247f97f1889a4333`；
- comparison base：`010a69aa9ffc2696f927dd1fd578b31c69fb2d6a`；
- run：`ec61a51f1e8906935b71713a1c2e22ba`；runner exit `0`；aggregate `PASS`；
- 15/15 test result 为 PASS，15/15 source observation 已发布；
- Rust desktop 为 509 passed / 0 failed / 40 ignored，Rust Gateway 为 285/285，frontend 为
  48/48，skill boundary 为 15/15；
- clean-commit source snapshot：537 entries、12225103 bytes，`head_sha` 精确等于 candidate；
- completion seal / run manifest / evidence manifest / source snapshot SHA-256：
  `560129de952bccf9384510a704ba93408af38c34bdfd9731d1eafaab3f22187b`、
  `780d4e6bf9fac7c9c5b19bc3bddbdc4a93a2eafc3bf63a7d78d4282d26b703bb`、
  `ff9cbb6f9c56a663d997d45684b0f98c665ba17bf3fd8697ba00d89fb5de649d`、
  `ff2c7cb07be7637be063a6edde42067f56e4d5b0a88119e45a627a8d3cbdcb6b`。

首次已提交 implementation candidate `e318eb29...` 的 formal run
`b0a0475d7d7a49bcc515a4910c70a53d` fail-closed：Rust desktop 原始进程为 509 passed / 0 failed /
40 ignored，但 fixture 漏登记新增 downgrade test；skill boundary 仍引用迁移前 command facade。
它们是 source-test contract 缺口，不是产品测试失败。修复经独立审查并提交后，最终 candidate 完成上述
单次 15-suite PASS run。主工作区因受保护的 ignored 数据无法形成 clean snapshot，正式 gate 因而在
exact-SHA 临时 detached worktree 执行；未删除或读取这些用户数据。

## 4. 停止与未验证边界

F1-A 到此停止产品实现。完成后没有自动继承的新 implementation sole NEXT；若继续，必须先按实时源码
做新的只读再基线。本窗口没有构建或替换 App/DMG，没有运行真实 provider/Science/SSH，没有读取真实
凭证、Keychain、SSH 私钥或用户 Science 数据，也没有 push、tag 或 release。
