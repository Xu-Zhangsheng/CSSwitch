# 2026-08-04 Post-H4 production flow 再基线

状态：日期化 source-only 再基线

适用范围：本地 `next` source line；H4 实现 exact commit
`84c1c6696103f6d57a56e950d2dbb5ef7fbc2de1`

最后复核：2026-08-04（Asia/Taipei）

失效条件：finalize consumer projection、manual/auto-boot consumer、one-click entry ordering，或本文
绑定的 source commit 发生实质变化时重新审计。

本文只记录 source/test 事实与后续 sole NEXT。它不授权 O1-A 实现，也不建立 artifact、installed、
live provider/Science/SSH、签名、公证、公开 release 或用户数据结论。本文所在 evidence-only 文档
commit 只封存已经完成的 H4 source candidate，不声称自身就是被引用的 source seal commit。

## 1. 结论

H4 已关闭 H1–H3 后唯一的 `HIGH`。manual UI 与 auto-boot 不再把所有 resolved degraded DTO
粗暴解释为 applied/Ready；consumer 只使用 `status + recovery_status + action` 与 canonical
journal/binding readback。attention、manual 与 readback failure 对 UI 发布 unknown；normal-start
cleanup 只有 exact active binding、journal cleared 且 selection non-pending 时才可 Ready。

重新复核当前 production one-click 后没有发现新的 HIGH。最先需要收紧的仍是 branch ownership：
`one_click_login_with_options` 在决定 healthy reopen / cold restart 前执行 system-SSH prevalidation、
managed stub transaction capture 与 pending authority cleanup retry。这个次序仍在同一 mutation lease
和 fail-closed guards 内，因此维持 MEDIUM；但它会让 healthy branch 继承 cold/recovery preparation
与失败面，应该先于 cold coordinator/receipt 拆分处理。

结论是：**下一 sole NEXT 选择 `O1-A Typed one-click entry decision`；本任务不进入实现。**

## 2. H4 source closure

- candidate：`84c1c6696103f6d57a56e950d2dbb5ef7fbc2de1`，父提交
  `10429b98f4baf8a753c7f870ba0de40d68b70c7e`；
- clean-context completion review：BLOCK/HIGH/MEDIUM/LOW 均为 0；
- focused：finalize projection、permission-preserving readback、journal open/cleared、normal/history
  cleanup、existing binding、readback failure、Atomic rollback uncertainty、manual frontend、auto-boot、
  transaction contract、metadata/inventory 均 PASS；
- 15-suite `GATE-SOURCE`：run `045cb36c38bd926f563a43c2c25843bd`，15/15 suite PASS，
  1375 executed、1331 passed、0 failed、44 approved ignored、0 skipped/todo/not-run；
- completion seal / run manifest / evidence manifest / source snapshot SHA-256 分别为
  `0b6126c308b857c1048a45e6b57e8d4a7df00f0cb0e172b9236426addd42bc62`、
  `79d3c529e870714a930d7dbb04e1d194da227e7ecb810c145d6a8dab5c53cb1a`、
  `63627fa82e24114f300a02d8d3e0075eee1e7c60e1ca637f66fbabd754149f2c`、
  `f06c94da27bde6347eab315b8760ef893c95bce34843aa74bcddd29e79da4158`。

未用于 closure 的失败 run 包括：sandbox 禁止 loopback 的环境失败、与并行审查共享依赖造成的
input drift、修正 Rust test identity 总数前的 14/15，以及一次无关 Codex cancel 时序抖动。
它们均未与最终 PASS seal 混合。

## 3. H4 关闭与未关闭边界

已关闭：

- projection 读取 canonical v4，不迁移、不写 config、不清 notice、不 chmod；
- surface 只含 disposition、journal disposition、binding relation、ready-only applied id、pending
  与 cleanup warning，不返回 transaction record、path、credential 或其他 config；
- history attention 即使已有 matching binding 也不发布 applied；journal open、error、矛盾组合和
  readback failure 均保持 manual/unknown；
- auto-boot 只有 ready projection 进入 `BootState::Ready`；failed/attention event 与 one-shot 补读
  都清 stale frontend applied，同时完整原 DTO 不被包裹或改写；
- Atomic rollback 双失败的不可读实际状态 fail-closed 且不被 projection 修复；两个可读 read-model
  仅作为 classifier counterexample，不冒充实际 filesystem outcome。

未关闭：

- final config writer 仍未 typed 区分 `AtomicRollbackUncertain` 与普通 safe failure；
- healthy/cold entry decision 仍晚于部分 branch-specific preparation；
- history restore 后 frontend 仍自动启动第二个 destructive operation，且 restore 无 durable journal；
- mutation lease/config CAS 仍主要是 process-local；
- cold affine receipt chain、剩余锁外等待、durable compensation 与 update provenance 未进入本阶段。

## 4. 新 sole NEXT：O1-A Typed one-click entry decision

目标：在 branch-specific effect 前，用一个只读、不可变、typed decision snapshot 决定本次 one-click
进入 healthy reopen、cold/restart 或 recovery 路由。decision 的输入只能来自已验证 config、handoff、
managed runtime identity、binding/login read model 与显式 runtime choice；诊断或用户文案不能控制流。

允许范围：

- 抽取最小 entry decision 类型与纯 decision function；
- 将 system-SSH/stub capture、pending cleanup retry 等 effect 移到已经选定的 branch owner 内；
- 为 healthy/cold/recovery decision、identity/config drift 和 branch-specific effect absence 增加回归；
- 更新对应 ChangeRecord、inventory、catalog 与稳定架构正文。

明确排除：

- 不拆 cold coordinator 或引入新的 affine receipt chain；
- 不移除 history restore 到 one-click 的 frontend 串联，不新增 history journal；
- 不改变 H1 terminal handoff、H2 prior-stop、H3 finalize、H4 DTO/readback/publication 合同；
- 不引入跨进程 lock/CAS、durable compensation、Science update provenance 或 artifact/live/release 工作。

进入实现前仍需用户单独授权。完成条件必须包含 focused regressions、quality/document governance、
clean exact-SHA 15-suite source gate、clean-context independent review、本地授权 commit 与 attributable
clean handoff；任何 `ENV-BLOCKED` 或 `NOT-RUN` 都不能当作完成。
