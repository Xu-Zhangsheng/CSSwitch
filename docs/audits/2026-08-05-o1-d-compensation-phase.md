# O1-D aggregate compensation phase source closure

日期：2026-08-05（Asia/Taipei）

证据层：source-only。本文不证明 artifact、installed/live、真实 provider / Science / SSH、签名、公证或 release。

## 结论

O1-D 在 `next` 的 `a20885a5b8dc2990df1316ec59ea80fe5eb578d5` 完成。它把既有
aggregate one-click compensation 从 `one_click.rs` 移到
`one_click/cold/compensation.rs`，比较基线为 `8c1f0c9471a0a9dc918baf5a7bea5b9e30a9e2cc`。

新 phase 专属拥有 typed compensation step / environment / outcome、Science cleanup、SSH cleanup、
authority restore、prior-runtime restart、snapshot cleanup、diagnostic rendering 与 recovery projection。
`run_cold_one_click` 保留唯一 `Ok` commit 与唯一 `Err` direct compensation dispatch；测试 helper 继续调用
production compensation。归一化模块可见性后，原 block 与新文件的 402 行内容及 SHA-256 相同。

本阶段没有新增 durable stepwise compensation progress / replay，没有改变 schema、checkpoint、CAS、锁、
receipt、prior-stop、authority capture、Gateway、managed Science launch、route/finalize、DTO、可见文案或
history semantics。

## 审查与 focused validation

- fresh clean-context 独立 reviewer 结论为 `PASS`，`BLOCK/HIGH/MEDIUM/LOW = 0/0/0/0`；它独立核对了
  9 个候选路径、机械等价、production 可见性、唯一 success/failure funnel、文档和 quality authority；
- Rust formatting、Desktop compile、两个 Rust contract selector、相关 Python runtime/profile/inventory、
  quality kernel、documentation governance 与 quality metadata 均通过；
- 三项隔离 fake-Science compensation 回归为 `3/3 PASS`。受限 sandbox 内的首次运行因 loopback
  `Operation not permitted` 无法执行，允许动态 loopback 后通过，不记为产品失败。

## exact-SHA source gate

受限 sandbox 内的首个完整 run `750377b6722621dffd7c14118c9e089c` 正确封口为 `FAIL`、runner exit `12`：
多个 loopback / shell suite 失败或跳过，两个 Rust adapter 为 malformed；该 run 不作为完成证据，也没有被
改写成 PASS。

随后以同一 exact SHA、同一 detached clean worktree 和全新隔离 output root 在允许所需本机测试能力的
环境重跑。run `4a7245bf3a4e22ef9912fd58ec56397f` 绑定
`a20885a5b8dc2990df1316ec59ea80fe5eb578d5`，结果为 15/15 suites PASS、15/15 source observations、
546 entries、12,260,824 bytes；gate comparison base 为
`010a69aa9ffc2696f927dd1fd578b31c69fb2d6a`。

- completion seal SHA-256：`c13f3593ee7304a7d4b001c7da25e189d2f16f6bc2599c6f4b132038af04fa9c`
- run manifest SHA-256：`b862e9e0ebd6c1d2d1d18f894e7e687e9d65b1cb403c5941d6ada3c97c9381ad`
- evidence manifest SHA-256：`44d15d4a6c971f9e09772a5a1bdbd947439c848856b8a6f8b6b57f7f159d187f`
- source snapshot manifest SHA-256：`42aa484ede29aea8e11a472ee0d39fb0bb1fe84a64d6b33e7dcbd28f4db7cd08`

## 停止点

O1-D 已完成，但不自动授权 durable stepwise compensation、read model、Science provenance 或任何其他
implementation。选择下一阶段前必须基于届时实时源码重新比较问题、依赖与非目标。
