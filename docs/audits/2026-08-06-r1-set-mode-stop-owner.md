# R1 set_mode stop owner source closure

日期：2026-08-06（Asia/Taipei）

证据层：source-only。本文不证明 artifact、installed/live、真实 provider / Science / SSH、签名、公证或 release。

## 结论

R1 在 `next` 的 final source candidate
`8fbbf2b76eb6d3e655d7942f312628c2eb8bd6d6` 完成，比较基线为
`31eaebb6e6ce11c9df51df9cc188f0fb6ad5a4b9`。implementation commit 为
`8fc54a3aa4a396048046d81de02c0ba622a38b6a`，source-contract repair commit 为
`8fbbf2b76eb6d3e655d7942f312628c2eb8bd6d6`。

切换到 official 的 `set_mode` 现在与 `stop_all` 复用同一 process-local Science stop owner
claim / publication 边界：在 `AppState` 锁内冻结 generation、runtime、confirmed-stopped、tracked child PID、
sandbox port 与 URL，并取得 exact typed stop request；锁外执行既有 stop script、TERM/KILL 与等待；回锁后
只有完整 owner CAS 仍成立才清理 tracking 并发布 outcome。

`set_mode` 保持自身既有语义：只有 current Science stop success 才停止 tracked Gateway 并进入 mode config
commit；stale、claim 或 stop failure 保留 replacement Science、Gateway 与旧 mode。config commit failure 仍
保持 stop-before-commit，不重启已停止 runtime。`stop_all` 抽取共享 helper 后仍无条件停止 Gateway；
`set_settings`、downgrade cleanup 与其他 sibling stop owner 不在本阶段范围。

## 独立审查与 focused validation

- 首个 fresh clean-context reviewer 审查 implementation candidate，返回 `PASS`，
  `BLOCK/HIGH/MEDIUM/LOW = 0/0/0/0`；
- 首次 exact-SHA gate run `f9d6a53369c35a3e1e20fa96e7fb9aee` 为 14/15 `FAIL`：唯一失败是
  `SUITE-ORPHAN-SKILL-BOUNDARY` 的旧 source-contract 字符串锚点没有跟随共享 helper 提取；该 run
  单独保留为失败证据，不参与最终 PASS；
- repair 把静态合同同步为共享 helper 内 lock-before-claim，以及 `set_mode` / `stop_all` 各自的
  claim -> execute -> relock -> publish 顺序；目标测试 1/1、完整 Skill boundary 15/15 与 quality metadata
  均通过；
- 第二个 fresh clean-context reviewer 重审完整候选与 repair，返回 `PASS`，
  `BLOCK/HIGH/MEDIUM/LOW = 0/0/0/0`；
- R1 generation-only / identity-only 双漂移回归、既有 S2 `stop_all` owner 回归、R0 `set_mode`
  stop-before-commit characterization、`cargo fmt`、`cargo check --all-targets`、
  `cargo clippy --all-targets -- -D warnings` 与相关 quality tests 均通过；
- 所有测试使用临时目录、fake process / Science 与动态 loopback；未读取或消费真实凭证、provider、
  Science 数据或 SSH。

## exact-SHA source gate

run `8efe1c128c0bec496bc39e67404a6718` 精确绑定
`8fbbf2b76eb6d3e655d7942f312628c2eb8bd6d6`，结果为 15/15 suites、15/15 source observations、
aggregate `PASS`、runner exit `0`。Rust Desktop 为 562 executed、522 passed、0 failed、40 ignored、
0 skipped / not-run；source snapshot 为 556 entries、12,508,495 bytes，comparison base 为
`010a69aa9ffc2696f927dd1fd578b31c69fb2d6a`。

- completion seal SHA-256：`803dec98474d38c6e1a99cd57514d4bca7c68d86db3cb866eb57ffaa0ccfd353`
- run manifest SHA-256：`592cd9804be5338fe25639c34ab0cb8e334f92459186c9eb0fdc280fc42b4771`
- evidence manifest SHA-256：`e049c3f8b07131ceb24774dd6d65a23b44c4277ea793f2a921683f81c1cd31dc`
- source snapshot manifest SHA-256：`ac69bb3388568c3c59d8ea12392e2ef4bf4be27a5d4100bef12a8dcfc502eef8`
- input digest set SHA-256：`6cb47e605eee83816504d577707d1f1ad0e36ca01423dd63aeab14e59bb63b72`

## 停止点

R1 到此完成。本文不自动授权 `set_settings`、downgrade cleanup、native-exit 或其他 stop owner 的迁移，
也不授权 artifact/live/provider/真实 Science/SSH、签名、公证或 release。下一阶段必须基于届时实时源码
重新做只读基线并只选择一个 sole NEXT。
