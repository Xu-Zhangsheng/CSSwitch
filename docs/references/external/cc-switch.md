# CC Switch 架构参考

Reviewed snapshot：`farion1231/cc-switch@98ccde0050f33a1bc8b16b96287a0b6f582c5d12`。

该快照可借鉴的只有维护思路：用数据化 preset 与少数 protocol / authentication family 压缩 provider 差异；把 request / response / SSE transform 与控制面分离；按平台和 artifact 分层组织 CI。preset 数量、workflow 声明或源码支持不等于当前 release、installed artifact 或 live provider 可用。

CC Switch 面向 coding app，不能替代 CSSwitch 的 Science OAuth、sandbox、path-secret、managed child identity、provider shell 与 hosted-capability 边界。任何采用都必须由 CSSwitch 当前源码、隔离测试及 artifact/installed 证据独立建立。

本参考不复制 CC Switch 实现代码。更新时必须重新固定 reviewed commit；许可、当前 upstream 行为和发布结果未复核前，只能作为 ideas-only reference。
