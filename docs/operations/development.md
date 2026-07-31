# 开发与维护

本文说明当前源码树的开发入口。安全、Git / worktree 和证据措辞分别以 [`.agents/rules/`](../../.agents/rules/) 为准。

## 环境

- macOS Apple Silicon（当前桌面发布目标）；
- Node.js / npm（Tauri 前端与构建）；
- Rust / Cargo（desktop backend 与 Rust gateway）；
- Python 3（测试驱动与 mock 使用，**不是** CSSwitch runtime proxy 依赖）；
- Claude Science App（只在隔离 runtime / 真机验收时需要）。

## 本地启动

```bash
cd desktop
npm install
npm run tauri dev
```

完整 source/unit gate 只在准备好 clean exact-HEAD 候选后从仓库根目录运行：

```bash
GATE_ROOT="$(mktemp -d /private/tmp/csg.XXXXXX)"
chmod 700 "$GATE_ROOT"
bash test/run_all.sh --output-root "$GATE_ROOT"
```

固定 15-suite 选择、输出目录约束和判定边界见[测试文档](testing.md)。无参数调用和
旧 `--require-release-ready` 已不再是有效入口。

## 组件级检查

```bash
(cd desktop/src-tauri && cargo fmt --check)
(cd desktop/src-tauri && cargo clippy --all-targets -- -D warnings)
(cd desktop/src-tauri && cargo test)

(cd desktop/gateway && cargo fmt --check)
(cd desktop/gateway && cargo clippy --all-targets -- -D warnings)
(cd desktop/gateway && cargo test)

python3 -m unittest discover -s test -p 'test_*.py' -v
node --check desktop/src/main.js
```

组件命令和单个 `test/run-*.sh` 只适合聚焦诊断；当前完整门禁是上述
`GATE-SOURCE`，不能由组件结果拼成 `SOURCE-GREEN`。

## Science 相邻功能工作法

1. 在隔离环境确认上游 runtime 事实；
2. 明确 source of truth 与所有权；
3. 先验证不增加存储 / 状态机的最短路径；
4. 跑一条完整 E2E，并分别记录 copy、discover、attach、load、trigger、功能执行与重启；
5. 最后再决定是否需要 UI、catalog、cache 或新存储。

Science 已拥有的能力不应在 CSSwitch 再造一套 installer、目录所有权或生命周期。

## 隔离 runtime 开发

- 使用临时外层 `HOME`、临时持久 data-dir、动态端口和假 `security`；
- 不使用真实 `~/.claude-science`、端口 `8765` 或 `/Applications/CSSwitch.app`；
- installed-App candidate、缓存 candidate、实际 PID、版本 runtime 与 data-dir 分别取证；
- live provider、真实账号或真实 SSH server 测试需要额外授权。

详细步骤见[真机验收](real-machine-acceptance.md)。

## 文档维护

文档类型、权威位置、默认阅读预算，以及临时 Plan / Draft Spec / Handoff 的晋升、过期与删除，统一见[文档治理合同](document-lifecycle.md)。

发布或重要 upstream runtime 变化后，应复核 architecture、功能限制、known issues 和 release evidence，而不是把新事实只留在聊天或 handoff。
