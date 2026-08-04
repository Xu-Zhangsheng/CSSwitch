# 2026-08-04 O1-A typed runtime entry owner source closure

状态：日期化 source-only 完成证据

适用范围：本地 `next`；O1-A implementation candidate
`8e2c069efbf086bd44aec652a817f75f7a8e09b3`

最后复核：2026-08-04（Asia/Taipei）

失效条件：production one-click command / runtime entry caller、finalize 或 Gateway recovery、healthy
reopen / mutating route ordering、pending-cleanup / SSH preflight、runtime mutation inventory、test identity
或本文绑定的 source evidence 发生实质变化时重审。

本文只封存 post-F1-0 再基线、O1-A implementation、focused tests、独立审查与 exact-SHA
source gate。它不建立 artifact、installed/live、真实 provider/Science/SSH、signing、notarization、
Gatekeeper 或 public release PASS，也不授权进入 C1、完整 F1-A、F1-R、U1 或其他后续阶段。

## 1. Post-F1-0 再基线与 sole NEXT

只读再基线绑定 clean HEAD `da120cbb7f040f899831ec8799880c3672cfb873`。production
`one_click` 当时仍由 command 层参与 finalize / Gateway recovery，runtime coordinator 在 healthy / mutating
route 决策前执行部分 branch-specific effect。相比 C1 或完整 F1-A，O1-A 是可独立关闭的最高优先级
系统风险：先建立唯一 runtime entry owner 与 effect ordering，避免 healthy reopen 继承 cold / recovery
失败面，也为后续 durable history transaction 提供真实入口边界。

因此 sole NEXT 选择 **O1-A typed runtime entry owner**。范围明确排除 giant coordinator 全量拆分、
跨进程 writer fence、完整 history transaction、read model / boot sequence、UI 与推测性通用抽象。

## 2. Implementation 与 focused evidence

- baseline：`da120cbb7f040f899831ec8799880c3672cfb873`；
- implementation candidate：`8e2c069efbf086bd44aec652a817f75f7a8e09b3`；
- command 只保留 runtime-owned preflight、锁外 auth、destructive lease 与 typed DTO projection；
- runtime entry façade 独占 interrupted finalize / cleanup、Gateway recovery、facts recapture 与 typed route；
- coordinator 在 pending cleanup、SSH mutating preflight 与 managed-stub capture 前完成 immutable facts
  capture 和 pure healthy / mutating decision；只有 cleanup 实际 `Cleared` 才重采 facts；
- Gateway terminal handoff 保持 non-`Clone`，由 consuming `into_terminal_record(self)` 单次转移；
- Rust typed decision / transaction / SSH contracts、Python boundary 15/15、mutation inventory 5/5、
  quality kernel 16/16、quality metadata、frontend、document governance、Rust `--no-run` 与隔离
  fake-Science healthy-reopen focused test 均 PASS。

## 3. Clean-context independent review

正式 reviewer 使用 `fork_turns="none"`。首轮绑定 fingerprint
`692c161a1be34c1eb714aee619f2ad079d3aa95d3f10c1cc25afd28f6726c36f`，结论
`FAIL`、`BLOCK/HIGH/MEDIUM/LOW = 0/0/3/1`，发现 inventory test identity、Gateway caller、typed
decision / recapture coverage 与 ChangeRecord path 四处缺口；这些缺口均已修复并重跑 focused tests。

全新 reviewer 最终绑定 staged binary diff SHA-256
`9c62956b1a75b4815d41613c953845a1bfb9543d1a0bc312eec4055780655fb6` 与 baseline HEAD，确认
staged-only、无漂移，结论为 `clean-context: YES`、`PASS`、
`BLOCK/HIGH/MEDIUM/LOW = 0/0/0/0`。该 fingerprint 与
`da120cb..8e2c069` 的 committed binary diff 一致。

## 4. Exact-SHA source gate

正式入口：`bash test/run_all.sh --output-root /private/tmp/cg.lv6laa`

- candidate：`8e2c069efbf086bd44aec652a817f75f7a8e09b3`；
- comparison base：`010a69aa9ffc2696f927dd1fd578b31c69fb2d6a`；
- run：`f6382f1a67831c856998964fdf877599`；runner exit `0`；aggregate `PASS`；
- 15/15 test result 为 PASS，15/15 source observation 已发布；
- clean-commit source snapshot：532 entries、12068766 bytes，`head_sha` 精确等于 candidate；
- completion seal / run manifest / evidence manifest / source snapshot SHA-256：
  `5a30e94f62df15e459f23c4986ba26dcf0a63ac0e06b3d5e97bf2daa8cffbd6a`、
  `c1ac2232a05fa5261e0ac3115ff5eb3988327829446288aec7bde66e184f4673`、
  `aac60068a97a1870197808d1736e9437b2e6bbbf9f110d474d8aa8b8d2da333f`、
  `2bc2a5f73a5a4d28ea5ee1fa77636329040881c4bf0ffc64966c610c7e721af7`。

首次调用因输出路径过长在任何 suite 执行前 fail-closed，runner exit `12`；它不是产品失败，也不
参与 closure。随后同一 exact SHA 使用合规的短、空、0700 输出目录完成上述 PASS run。

## 5. 停止与未验证边界

O1-A 到此停止产品实现。完成后没有自动继承的新 implementation sole NEXT；若继续，必须先按
实时源码做新的只读再基线。本窗口没有构建或替换 App/DMG，没有运行真实 provider/Science/SSH，
没有读取真实凭证、Keychain、SSH 私钥或用户 Science 数据，也没有 push、tag 或 release。
