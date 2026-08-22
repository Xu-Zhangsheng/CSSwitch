# 代码作者规则

本规则适用于跨语言的 source 改动；语言语法、工具参数和局部风格仍服从相邻代码与其 owner。

## 实现与边界

- 使用既有 owner 与 typed outcome；配置、文件、进程、IPC 和 network 边界必须 fail closed。
- 诊断和日志不得暴露凭证、用户数据或其他敏感输入；副作用只能在成功提交点后发布。
- 把错误、timeout、cleanup 与 partial success 当作可测试的行为合同，而非仅靠日志补救。
- 注释只解释非显然的 ownership、时序、安全或失败原因，不复述控制流。
- 行为变化同步 owning test；产品行为、稳定边界或维护流程变化分别同步唯一的 Feature、Architecture 或 Operation owner。source、generated output 和 artifact 不得混作同一事实来源。

## 模块责任卡与风险

只在新增模块、跨模块边界或既有 owner 不清楚时，在对应既有的 Architecture、Feature 或 Operation owner 记录：owner、输入 / 输出、状态、副作用 / 提交点、失败归因和直接 caller。普通有界施工设计优先留在任务系统或带生命周期的 Plan；只有确需异步设计评审时才使用符合生命周期的 Draft Spec。Handoff 只链接已存在的 Plan / 权威正文与 checkpoint，不能承载第二份设计正文；接受后提炼到唯一 Architecture、Feature 或 Operation owner 并删除临时设计。普通模块内改动只引用既有边界，不重复制造文档。

变更风险取最高等级：`F3 > F2 > F1 > F0`。`F0` 是不改变公开合同、持久状态或外部副作用的局部、可直接检查改动；`F1` 是 owner 和接口不变、可逆且边界清楚的模块内改动；`F2` 涉及新模块、公开接口、跨模块数据流、owner、状态或 quality authority；`F3` 涉及持久格式 / 迁移、事务 / 恢复、安全 / 凭证、文件 / 进程 / network 副作用，或 artifact、installed、live、Provider、Science、SSH、签名、release。文件、进程或 network 副作用的目标、授权、时序、提交点、timeout、cleanup、补偿或失败报告发生变化，即使 API 和 owner 不变也属于 `F3`。

`F0` / `F1` 默认只做适用的聚焦检查；稳定的 `F2` / `F3` candidate 按 [独立审查](reviewing.md) 集中审查。完整 source closure 只在明确声明 candidate closure、quality kernel / gate authority 改变或影响无法可信界定时，按[自动测试](../../docs/operations/testing.md)执行；聚焦通过不能写成 `SOURCE-GREEN`。
