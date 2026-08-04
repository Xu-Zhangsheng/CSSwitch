# Quality kernel v1

`quality/` 是机器事实源，不是项目进度正文：

- schema 定义记录的可接受形状；
- requirements 与 production-path policy 定义必须覆盖什么；
- change / bug records 分别记录变更影响和已知问题，`open-not-fixed` 不能写成 fixed；
- test catalog 与 release gates 绑定固定 suite、entrypoint、identity、环境和聚合规则；
- lineage 区分 previous release、development source、source candidate、release candidate 与公开 release，不能把 source evidence 提升为 release evidence。

当前完整 source/unit 操作入口、输出目录、安全重跑和结果词汇只在[自动测试与证据判定](testing.md)维护。`GATE-S0-LEGACY` 已 retired；只有递归验证通过的 `GATE-SOURCE` completion seal 才能建立 `RUN-EVIDENCE-GREEN` 与 `SOURCE-GREEN`。

source claim 不证明 app / DMG、installed runtime、live provider / Science、签名、公证、Gatekeeper 或公开附件。更高证据层以[发布流程](release.md)、[真机验收](real-machine-acceptance.md)和 dated evidence 为准。

生产路径变化时必须同步 machine record 和测试影响；metadata、impact 或 lineage 漂移作为当前问题进入 [known issues](../../.agents/context/known-issues.md)，不在本文复制阶段进度。历史 RUE 实施过程与旧 gate 规格从 Git 和 dated audit 追溯。
