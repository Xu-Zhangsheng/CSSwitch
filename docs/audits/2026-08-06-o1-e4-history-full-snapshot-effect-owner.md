# O1-E4 history full-snapshot durable effect owner source closure

日期：2026-08-06（Asia/Taipei）

证据层：source-only。本文不证明 artifact、installed/live、真实 provider / Science / SSH、签名、公证或 release。

## 结论

O1-E4 在 `next` 的 implementation commit `efe029b4924dc312feacc00905a1ca9177d212f6` 完成，比较基线为
`7cdc355c3dc5275d948686005d3545209cc7527b`。

history credential publication 在 durable `HistoryCredentialWritePending` 后，live failure 与 fresh-process replay
进入同一个 crash-releasing cross-process exclusive effect fence。等待者必须在锁内重新读取 canonical Config，并以
完整 history transaction record 与冻结的 Config authority 重新取得所有权；漂移、Science 非 quiescent、snapshot
ticket 或四项 private manifest 身份异常都 fail closed 并保留恢复证据。

补偿 owner 先以完整记录 CAS 发布 `HistoryAuthorityRestorePending`，再幂等恢复 encryption key、OAuth tokens、
active org 与 virtual-org marker 的完整 history authority manifest，最后 CAS 发布
`HistoryAuthorityRestoreSucceeded`。恢复任一 entry 后崩溃仍保留 pending，fresh replay 会重放整个 manifest；只有
durable succeeded 才能进入 cleanup-only、清除精确 journal 并重试 snapshot cleanup。正常 credential publication、
restore-only / explicit resume DTO 与可见文本合同保持不变。

## 独立审查与 focused validation

- 首轮 fresh clean-context reviewer 发现 runtime mutation inventory 仍描述旧的 same-process / narrow replay 合同；
  inventory、schema、serialization、ordered effects、compensation 与三个 O1-E4 characterization identity 已同步；
- 第二个 fresh clean-context reviewer 完整重审 12 文件候选，最终返回 `PASS`，
  `BLOCK/HIGH/MEDIUM/LOW = 0/0/0/0`；
- O1-E4 focused tests 为 3 passed、0 failed；history crash / concurrent-config、live credential failure、V2 wire/state
  与 downgrade history-phase regression 均通过；
- exact-SHA Rust Desktop suite 为 561 executed、521 passed、0 failed、40 ignored、0 skipped / not-run；
- `cargo check --all-targets`、`cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、metadata、runtime
  mutation inventory、source-gate observation/count contract 与 `git diff --check` 均通过；
- 测试只使用临时 HOME、fake process / Science 与 loopback；未读取或消费真实凭证、provider、Science 数据或 SSH。

## exact-SHA source gate

run `e4c0d1a0f915395861d233af24e764da` 精确绑定
`efe029b4924dc312feacc00905a1ca9177d212f6`，结果为 15/15 suites、15/15 source observations、aggregate
`PASS`、runner exit `0`；source snapshot 为 553 entries、12,484,697 bytes，gate comparison base 为
`010a69aa9ffc2696f927dd1fd578b31c69fb2d6a`。

- completion seal SHA-256：`1ee7822a9f4c435c4ab893eef139ecb0bedd9c07f565fd4b38cdc1b7689a3040`
- run manifest SHA-256：`101f525d3216eb94f04fc93a5aa2faf2d958638477d0a69a9e7ffce0878f5507`
- evidence manifest SHA-256：`263c5effd5a2cf8a0ca81e22a4ad2a3804762ad853a5c5eda43bb97cdb2fe606`
- source snapshot manifest SHA-256：`ebf2dd5362c5f461ec836b33d10c893bedd23a27ff8ef041a8500a5219f31ee8`
- input digest set SHA-256：`984483663b9f61eaef38913bbef7fbdff7c6d31a2177492449fa0f0b0a2b7687`

## 停止点

O1-E4 已完成。history full-snapshot restore 不再属于未统一 sibling 边界；其他 sibling full-snapshot / multi-file
crash boundary、V1 与 typed incomplete V2 仍保持各自现有边界。此 closure 不授权其他 runtime 重构、
artifact/live/provider 验证、真实 Science / SSH、签名或 release；选择下一阶段前必须基于届时实时源码重新做只读基线。
