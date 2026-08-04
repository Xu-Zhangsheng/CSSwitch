# 2026-08-05 C1-A cross-process config writer fence source closure

状态：日期化 source-only 完成证据

适用范围：本地 `next`；C1-A final source candidate
`70940d08e266de2ad002278428d8604fddbbf37f`

最后复核：2026-08-05（Asia/Taipei）

失效条件：canonical config writer / pinned directory、migration / downgrade / backup publication、
writer lock identity、read-only projection、runtime mutation inventory、source test identity 或本文绑定的
source evidence 发生实质变化时重审。

本文只封存 post-O1-A 只读再基线、C1-A implementation、focused tests、独立审查与 exact-SHA
source gate。它不建立 artifact、installed/live、真实 provider/Science/SSH、signing、notarization、
Gatekeeper 或 public release PASS，也不授权进入完整 F1-A / F1-R、giant coordinator、read model、
Science provenance 或其他后续阶段。

## 1. Post-O1-A 再基线与 sole NEXT

只读再基线绑定 clean HEAD `d6753ba0c18aba2acbfa47106105abc6f44fcffd`。production config
mutation 当时只有 process-local `CONFIG_ACCESS`，两个 App/进程仍可分别读取同一旧 snapshot，再以各自
atomic rename 覆盖对方提交。完整 history durable transaction 需要先有可信的 canonical writer boundary；
giant coordinator、read model 与 Science provenance 则是可独立推进的其他风险面。

因此 sole NEXT 选择 **C1-A cross-process config writer fence**。范围明确排除 schema / DTO / 可见文本、
one-click / history 行为、完整多文件事务、coordinator 拆分、artifact 与 live 验证。

## 2. Implementation 与 focused evidence

- baseline：`d6753ba0c18aba2acbfa47106105abc6f44fcffd`；
- runtime implementation commit：`b55eb89e5edfb3d5920c7238574caaee08efa63f`；
- source identity governance commit / final candidate：
  `70940d08e266de2ad002278428d8604fddbbf37f`；
- pinned config directory 内的 persistent `.config.writer.lock` 以 `openat`、`O_NOFOLLOW`、`0600`、
  regular-file 与 single-link 条件打开；阻塞 acquire 后重新打开目录项并复核 dev / inode / nlink；
- process-local `CONFIG_ACCESS` 总是先于 cross-process fence；migration-capable load、full save、update、
  fallible update、downgrade、rolling backup / drop 共享该顺序；read-only current projection 不创建或获取 fence；
- 真实两个 child processes 先以 nonblocking `flock` 明确证明 B 已在 A 的 inode 上观察
  `EWOULDBLOCK/EAGAIN`，再释放 A，最终保留双方字段；另有 symlink、hardlink 与等待期间 replacement
  的 fail-closed 负向测试；
- focused config tests 69/69、runtime mutation inventory 5/5、identity / quality focused 92/92、
  quality metadata、document governance、format / diff checks 均 PASS；Rust lib candidate 全量测试为
  503 passed / 0 failed / 40 ignored，新增两条负向测试后由最终 source gate 重新覆盖。

## 3. Clean-context independent review

所有正式 reviewer 均使用 `fork_turns="none"`。首轮 implementation reviewer 绑定 fingerprint
`716fca0243f1045e5c93db4361c2e241ffb07319e143d7623b3a1021fc28576d`，结论 `FAIL`、
`BLOCK/HIGH/MEDIUM/LOW = 0/0/2/0`：双进程测试尚缺确定性 contention rendezvous，且 hardlink / acquire
后 replacement identity 分支缺负向覆盖。修复后由全新 reviewer 绑定 fingerprint
`5b833683fde319ecfc9de6662e05ba6088047942dac1bed97846dd676af31396`，确认无漂移并给出
`clean-context: YES`、`PASS`、`0/0/0/0`。

首次 exact-SHA gate 暴露 6 个新增 child-process identity 未登记；治理补丁由另一全新 reviewer 绑定
fingerprint `f7a32b0f67d06822324b1beb1dd498858caa271592fc9a72e18ddcb5da090af3`，逐项核对
545 个 observed / expected identities、15 个 catalog hash binding 与两处容量断言，结论同为
`clean-context: YES`、`PASS`、`0/0/0/0`。

## 4. Exact-SHA source gate

正式入口：`bash test/run_all.sh --output-root /private/tmp/c1b.tjJC1d/out`

- candidate：`70940d08e266de2ad002278428d8604fddbbf37f`；
- comparison base：`010a69aa9ffc2696f927dd1fd578b31c69fb2d6a`；
- run：`d9bf63dea8d3003e981b5232a4f4faea`；runner exit `0`；aggregate `PASS`；
- 15/15 test result 为 PASS，15/15 source observation 已发布；
- clean-commit source snapshot：534 entries、12092017 bytes，`head_sha` 精确等于 candidate；
- completion seal / run manifest / evidence manifest / source snapshot SHA-256：
  `db0304aa07dc35d26550e36123107cbe1adade75d94a193733df2a6385814fe4`、
  `626a753a559d2f979f908841f482fff5162b74f23358549ef41e1bd38fa44f92`、
  `ab9d3c8467e8c27ff283566fdb6104f3f9fb475b6ea0476da4d7e693980a8e46`、
  `7b2b7b9d1ec9b6a00ae7ab8adc1f6759f1701fc3a28fce06f548022440068b32`。

首次 candidate `b55eb89e...` gate run `5abe8308ebd4c68e20418daf60f61f28` 的 Rust 原始进程
为 505 passed / 0 failed / 40 ignored，但 adapter 因 6 个新增 child-process identities 未登记而以
`TEST_IDENTITY_MISMATCH` fail-closed、runner exit `12`。它不是产品测试失败，也不参与 closure；登记、
容量断言、catalog hash 与 ChangeRecord 经独立审查后，最终 candidate 完成上述 PASS run。

## 5. 停止与未验证边界

C1-A 到此停止产品实现。完成后没有自动继承的新 implementation sole NEXT；若继续，必须先按实时源码
做新的只读再基线。本窗口没有构建或替换 App/DMG，没有运行真实 provider/Science/SSH，没有读取真实
凭证、Keychain、SSH 私钥或用户 Science 数据，也没有 push、tag 或 release。
