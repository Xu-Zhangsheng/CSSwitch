# 2026-08-05 O1-B cold one-click coordinator source closure

状态：日期化 source-only 完成证据

适用范围：本地 `next`；O1-B implementation candidate
`c16bb91390e38a66774358f3d77390643b022d3d`

最后复核：2026-08-05（Asia/Taipei）

失效条件：production one-click entry / healthy reopen / mutating route、prior-stop、SSH、authority、
Gateway、Science、route、finalize、compensation、runtime mutation inventory、test identity 或本文绑定的
source evidence 发生实质变化时重审。

本文只封存 O1-B implementation、focused tests、独立审查与 exact-SHA source gate。它不建立
artifact、installed/live、真实 provider/Science/SSH、signing、notarization、Gatekeeper 或 public
release PASS，也不授权进入 durable stepwise compensation、read model、Science provenance 或其他后续阶段。

## 1. 范围与实现

本轮从 clean baseline `ac6c2edc359b5af34566674a17629e076477e7b1` 开始，只实施行为保持的
O1-B cold coordinator 机械切分：

- `one_click_login_with_options` 保留 config/auth/port、immutable entry facts、纯 branch decision、
  healthy reopen、pending-cleanup exact retry 与 cleanup 后 facts recapture；
- mutating cold/recovery 的 SSH prevalidation、prior Science stop/outcome、AuthorityTransaction、
  Gateway/Science、route、finalize 与现有 compensation funnel 移入
  `runtime/sandbox_session/one_click/cold.rs::run_cold_one_click`；
- 原函数从 mutating 起点开始的 832 行主体与新 cold 函数体逐字节一致；checkpoint 数量与顺序、
  complete-record CAS、锁、receipts、DTO、可见文本、history semantics 与 compensation 行为未改；
- Rust AST 与 Python source-boundary tests 改为分别验证 entry / cold 所有权、entry-before-cold ordering、
  SSH / authority / transaction 顺序和单一 commit / compensation funnel；
- architecture、runtime mutation inventory 与 ChangeRecord 同步承认：cold coordinator 仍同时编排多个
  authority，durable compensation 仍是 aggregate 而非 stepwise replayable。

## 2. Focused evidence

- `cargo fmt --check`、Desktop Rust `--no-run`：PASS；
- Rust transaction / SSH AST contracts：2/2 PASS；
- profile/runtime boundary 与 mutation inventory：24/24 PASS；
- quality kernel：16/16 PASS；document governance：4/4 PASS；quality metadata：PASS；
- 在允许隔离子进程与动态 loopback 的环境中，cold-start commit、post-receipt failure restore、
  DB restart unproven candidate block 三条定向 fake-Science 回归测试：3/3 PASS。

一次裸的并行 Desktop `cargo test --lib` 在受限/共享环境中出现 loopback `Operation not permitted`
与跨测试干扰；同一关键 selector 在允许隔离 loopback 的环境逐条通过，因此该裸运行不作为
产品失败或 source gate 结论。正式结论只取下述固定 gate。

## 3. Clean-context independent review

正式 reviewer 使用 `fork_turns="none"`，在未提交候选上实时复核 branch、HEAD、完整 diff、新文件、
文档与质量元数据，并运行只读/定向检查。结论为 `clean-context: YES`、`PASS`、
`BLOCK/HIGH/MEDIUM/LOW = 0/0/0/0`。

Reviewer 独立确认被提取的 832 行 cold 主体逐字节一致；入口只发生一次 cold dispatch；编译、
transaction / SSH contract、profile/runtime boundary、inventory、document governance 与 metadata 均通过；
没有把 durable compensation、artifact、installed/live、真实 provider/Science/SSH、signing 或 release
写成已完成。

## 4. Exact-SHA source gate

正式入口：在临时 clean detached worktree 上执行
`bash test/run_all.sh --output-root /private/tmp/csg.TwpyBh`

- candidate：`c16bb91390e38a66774358f3d77390643b022d3d`；
- comparison base：`010a69aa9ffc2696f927dd1fd578b31c69fb2d6a`；
- run：`f701772bbf07b7fdcc6c46bfcd280a20`；runner exit `0`；aggregate `PASS`；
- 15/15 test result 为 PASS，15/15 source observation 已发布；
- clean-commit source snapshot：540 entries、12237160 bytes，`head_sha` 精确等于 candidate；
- completion seal / run manifest / evidence manifest / source snapshot SHA-256：
  `94b1f06daedeb51c74d0069a0100821db937a36c94c1f3fe38e2bea5e5d588c4`、
  `42ca8bcef24b2124c20911b878e48f6319688063c44a4f9b778564906b96e869`、
  `9181d2373b44de147fe079e4e4d6edde0e79df3c4f91e762aff720e7c0387316`、
  `1161ae1d469623b40444e6e6b12f951889d17d20f68ea1238c96ff634aa67bbb`。

首次调用因输出路径过长在任何 suite 前 fail-closed；随后普通 `git status` 为 clean 的当前工作树
在严格 clean snapshot 时返回 `SNAPSHOT_DIRTY`，本轮没有把它进一步归因为某个具体 ignored 路径。
没有删除 ignored 数据，也没有把这些 preflight/snapshot 结果记为产品失败；同一 exact SHA 在新的
clean detached worktree 完成上述 PASS run。

## 5. 停止与未验证边界

O1-B 到此停止产品实现。cold coordinator 仍负责 prior-stop、authority、Gateway、Science、route 与
finalize；进一步拆分必须重新基线并单独授权。本窗口没有构建或替换 App/DMG，没有运行真实
provider/Science/SSH，没有读取真实凭证、Keychain、SSH 私钥或用户 Science 数据，也没有 push、
tag 或 release。
