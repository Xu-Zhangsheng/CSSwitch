# 正式独立审查规则

- 真正的正式 reviewer 默认从 clean context 启动，不继承实现窗口对话；多 Agent 调用使用 `fork_turns="none"`。
- 本规则是正式审查 overlay，不计入零或一项领域规则预算；reviewer 最多再读一项被审领域规则，无匹配项时只使用本 overlay。它不放宽一个索引、最多两份正文或 Context 的默认首轮预算。
- reviewer prompt 必须自包含：精确 worktree / branch / HEAD、候选范围、允许与禁止项、权威入口、应执行的只读检查、严重度定义和期望输出。
- 继承上下文的审查只能提供辅助线索，不能满足最终独立审查 gate。
- 默认使用常规 reviewer；只有已经报告 `HIGH` 或 `BLOCK` 时，才把该 finding 窄化后升级为 `gpt-5.6-sol xhigh` 裁决，不用高配 reviewer 代替首轮分类。
- 同一个有界目标、slice 或候选的正式独立审查总预算最多两轮，不因更换 reviewer、模型或 prompt 重新计数。第一轮必须等实现形成稳定候选后再启动，并一次性汇总 findings；不得在每个增量修复后新增审查轮次。
- 第一轮后的目标内修复、测试和主 Agent 复核应合并完成，再启动第二轮 clean-context 最终复审。修复会使第一轮旧 PASS 失效，但只允许这一轮复审。
- 第二轮是最终审查轮。若仍为 `FAIL`，主 Agent 不得自动启动第三轮；应停止审查循环，按当前目标裁定为未通过或阻塞，向用户报告剩余问题并等待新的明确指示。用户明确开启新的任务或候选后，才重新计算审查预算。
- reviewer 发现目标外问题时，只有能直接阻断当前结果的 P0 / P1 才纳入本轮 findings；P2 及以下应记录为后续项，不得借此扩大当前 slice。
- 测试身份 hash 在 WIP 中不得随每个小改动反复旋转或核对。实现与修复合并、候选冻结后统一更新并验证一次；只有冻结后参与 hash 的文件再次变化，或 validator 给出具体 mismatch，才允许再更新一次。禁止为获得相同结论而重复运行 hash 核对。
- 正式结论必须显式报告 `clean-context: YES|NO`、findings 及最终 `PASS|FAIL`，不得用“未发现”替代 gate 结果。
