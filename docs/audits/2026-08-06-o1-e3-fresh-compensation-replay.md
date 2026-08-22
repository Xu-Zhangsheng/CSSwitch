# O1-E3 fresh-process durable compensation replay owner source closure

日期：2026-08-06（Asia/Taipei）

证据层：source-only。本文不证明 artifact、installed/live、真实 provider / Science / SSH、签名、公证或 release。

## 结论

O1-E3 在 `next` 的 `8b9de3bc64aae7700707b718fec6c6874864db19` 完成，比较基线为
`06e9e84a599a62d10e561c5372925fe0cd1a79c4`。核心 implementation commit 为
`2c8f87be57dbf85934d34b98c5b4e0c3a4290b24`；其后三个 test / gate closure commit 依次绑定新增测试身份、
更新 observation 容量守卫的真实测试数，并串行化会启动 fake Codex child / sidecar 的 R0 测试。

当前 one-click compensation 在注册的私有 `0700` snapshot 内先持久化完整、`0600` 的 authority replay plan
与 compensation replay manifest，再发布 path-free V2 compensation intent。fresh process 在 provider auth 前读取并
精确绑定 compensation ID、snapshot ticket、target profile、runtime fingerprint 与 business record；V1、typed
incomplete、retarget、unsafe snapshot / manifest 或 config drift 均 fail closed 并保留人工恢复证据。

live compensation 与 fresh replay 在首个 step intent / effect 前取得同一个 crash-releasing cross-process exclusive
lease，并持有到 effect outcome CAS；provider auth 只取得 shared absence lease，publication / replay 取得 exclusive
lease。五个固定步骤按 `pending -> in_progress -> succeeded|failed|skipped(cause)` 的完整记录 CAS 顺序推进，避免两个
进程同时成为 effect owner。

fresh replay 只使用 durable exact identity：Science candidate cleanup、序列化的 SSH before/candidate transaction、
固定 authority tree/root/backup 与完整 pre-operation config、Gateway health / packaged binary / uid / listener identity、
以及 prior Science 的 exact stopped recipe。prior restart 要求旧 receipt 路径精确 absent，使用预分配 launch ID 写入并
认领 fresh receipt。authority restore 不恢复旧进程的 AppState 或 Gateway child；snapshot cleanup 只在前置步骤满足且
ticket 仍精确匹配时执行，失败保留 recovery snapshot。

## 独立审查与 focused validation

- 多轮 fresh clean-context review 依次发现并关闭 exact SSH transaction、authority effect/outcome 幂等、prior Science
  fresh receipt adoption、auth/replay race、live/fresh 双 owner、旧 receipt absence、test identity digest、observation
  count 与 Codex child-test 并发脆弱点；最终 reviewer 返回 `PASS`，BLOCK / HIGH / MEDIUM / LOW 为 `0/0/0/0`；
- 最终 exact-SHA Rust Desktop suite 为 558 executed、518 passed、0 failed、40 ignored、0 skipped / not-run；
- O1-E3 focused tests 为 7 passed、0 failed；Codex fake-process R0 group 为 8 passed、0 failed；
- `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、metadata、runtime mutation inventory、Skill
  boundary、run-evidence / source-gate contract、JSON parse 与 `git diff --check` 均通过；
- 测试只使用临时 HOME、fake process / sidecar 与 loopback，不读取或消费真实凭证、provider、Science 数据或 SSH。

## exact-SHA source gate

修复过程中保留了三个非 PASS gate run：

- `e362b8265b3447f82459d5bd1d181570` 暴露新增 test identity / inventory 未闭合及一个并发 Rust test failure；
- `fe2e639c782b826e7a4f67d0c1b97617` 暴露 Rust Desktop identity 数已从 551 增至 558，但两个容量守卫仍固定旧数；
- `0c29d98d5aab8a94c0e0e7a809ef0bea` 与随后 unchanged-SHA run
  `12d58630a843cc2f138af3a9b780c5cc` 均为 14/15 suites PASS，只在高并发 Rust Desktop 中失败同一 Codex child
  characterization；测试身份、断言和容量阈值保持不变，最终以 test-only mutex 串行化相关 fake process tests。

最终 run `ef170ed310d706039f497e1be72678da` 精确绑定
`8b9de3bc64aae7700707b718fec6c6874864db19`，结果为 15/15 suites、15/15 source observations、aggregate
`PASS`、runner exit `0`；source snapshot 为 552 entries、12,461,608 bytes，gate comparison base 为
`010a69aa9ffc2696f927dd1fd578b31c69fb2d6a`。

- completion seal SHA-256：`02cc56304be9b6af703a671cf15e46fbbe2fb737e5c6775703b92acf67b04e47`
- run manifest SHA-256：`a772223384bd7a5c976711627284cb31dc822c6ca46702e000973c8de366a7ee`
- evidence manifest SHA-256：`ac7317b37daddc789fe84f4510f99de30168a09983e0cdc9a15c7a9525aa270b`
- source snapshot manifest SHA-256：`7c011be5a1cbdab254e008504d6c6efa6a662b86711ed4ae71c4953331ae5081`
- input digest set SHA-256：`064ee609ca0a9c97af3bc1090e03eb583ee126b0002b0e9e22ab6d7d63ea6e28`

## 停止点

O1-E3 已完成。V1 与 typed incomplete V2 仍按设计要求人工恢复；其他 sibling full-snapshot / multi-file crash
boundary 不由本阶段自动统一。此 closure 不授权其他 runtime 重构、artifact/live/provider 验证、真实 Science / SSH、
签名或 release；选择下一阶段前必须基于届时实时源码重新做只读基线。
