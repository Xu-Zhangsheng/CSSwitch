# 架构文档

- [架构总览](overview.md)：产品边界、所有权、数据流、网络与失败边界。
- [Science runtime](science-runtime.md)：可执行文件选择、持久 data-dir、runtime identity 和升级合同。
- [Desktop 控制面](desktop-control-plane.md)：Tauri command 注册、生产 frontend caller、event、DTO 与错误投影。
- [运行时状态与事务](runtime-state-transactions.md)：AppState/config/Lifecycle、锁序、journal、snapshot、receipt 与补偿链。
- [Gateway 与 provider 路由](gateway-provider-routing.md)：正式/scratch Gateway、Codex、Skill stdio 与 Science control 边界。
- [Claude Science 能力依赖](science-capability-dependencies.md)：解释 Science/CSSwitch/用户/外部 owner、第三方最小托管责任、窄 bridge、non-target 类别和故障归因；逐能力当前结论只在产品能力地图维护。
- [Science Skill / MCP / Plugin 扩展控制面](skill-mcp-plugin-control-plane.md)：回答多包格式的 component-wise compatibility、Agent/host/Science ownership、inspect-plan-confirm-apply、effect ledger、MCP profile 与演进/验收不变量。

这里只保留跨版本的当前合同。某次 Science 版本或事故的证据放入 [日期化调查](../evidence/investigations/README.md)。
