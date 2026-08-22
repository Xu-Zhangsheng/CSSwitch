# 测试与证据规则

- 证据层必须分开：source/unit、built artifact、临时安装、installed/runtime、
  live provider/账号/SSH、签名/公证/Gatekeeper、公开 release。
- 当前唯一完整 source/unit 入口是
  `bash test/run_all.sh --output-root <absolute-empty-0700-dir>`；只有递归验证的
  16-suite `GATE-SOURCE` PASS completion seal 才建立
  `RUN-EVIDENCE-GREEN` / `SOURCE-GREEN`。
- 无参数 `test/run_all.sh`、旧 `--require-release-ready`、`current-env clean` 和
  `release-ready green` 只属于历史合同，不能用于当前候选。
- 聚焦 suite 只用于诊断，不能拼成完整 source gate；stdout 摘要、局部通过或
  历史 pass 数量都不是 completion seal。
- 文件复制、Science 发现、Agent attach、Skill load/trigger、领域功能执行和重启
  持久化是不同结论；mock/loopback 不能写成 live provider。
- `PASS`、失败、`PREFLIGHT/ENV-BLOCKED`、`NEEDS-REAL-MACHINE`、未执行与需人工
  判断必须分开；后五类都不得记作通过。
- 运行真机或 installed runtime 前遵守
  [真机验收](../../docs/operations/real-machine-acceptance.md)的隔离护栏。
- 报告命令、exact commit/artifact、环境、退出码、run id / completion seal 与
  脱敏证据。执行合同和完整非声明见
  [自动测试](../../docs/operations/testing.md)。
