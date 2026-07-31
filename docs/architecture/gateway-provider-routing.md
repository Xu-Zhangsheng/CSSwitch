# Gateway 与 provider 路由

本文回答“Gateway binary 有哪些运行入口、正式与 scratch 怎样分离、协议和错误边界在哪里”。桌面 IPC 见[Desktop 控制面](desktop-control-plane.md)，生命周期提交见[运行时状态与事务](runtime-state-transactions.md)。

## 构建与进程边界

Desktop build 会构建并打包同变体 Rust Gateway sidecar。当前生产运行没有 Python proxy fallback；旧 Python listener 清理只是 lifecycle 兼容路径。

同一个 `csswitch-gateway` binary 有四个入口：

| 入口 | 触发 | 职责 |
|---|---|---|
| HTTP server | 默认参数/env | 正式或 scratch provider/Codex Gateway |
| Codex auth CLI | `codex-auth` | browser-only OAuth、refresh/logout/status 等私有认证控制面 |
| Skill stdio MCP | `skill-install-mcp` | `install_external_skill` / uninstall / poll 的窄 connector |
| Science control | `science-control configure-third-party` | loopback nonce/CSRF 下配置 OPERON route Skill/connector |

这些入口共享二进制，不共享同一状态承诺。

## 当前源码 owner

| 边界 | 当前源码 owner |
|---|---|
| Gateway listener、method dispatch 与唯一 public server entry | `desktop/gateway/src/server.rs` |
| HTTP head/body codec、limits 与错误响应 | `desktop/gateway/src/server/http_codec.rs` |
| GET/POST、provider/Codex inference 与 SSE | `desktop/gateway/src/server/inference_dispatch.rs` |
| Skill install bridge host lifecycle | `desktop/gateway/src/server/skill_bridge_host.rs` |
| `server::tests::*` 测试身份 | `desktop/gateway/src/server/tests.rs` |
| Desktop 侧 launch plan、recovery、reuse/spawn/stop | `desktop/src-tauri/src/runtime/proxy_lifecycle.rs` façade |
| launch contract / allowlist、binary lookup、Science Skill bridge 与 lifecycle 编排 | `runtime/proxy_lifecycle/launch_contract.rs`、`binary.rs`、`skill_bridge.rs`、`recovery.rs`、`lifecycle.rs` |
| 旧 Python listener 与错误 sidecar 的 fail-closed 清理 | `desktop/src-tauri/src/runtime/legacy_proxy.rs`，只作为兼容清理边界 |

`server.rs` 不拥有 Desktop 进程状态；`proxy_lifecycle` 也不拥有 provider 协议实现。
前者接受冻结的 `GatewayConfig` 并服务请求，后者在 `AppState`、`Lifecycle` 与
config 合同下管理受管 sidecar。legacy proxy 路径不能作为 Python fallback 或
当前 provider implementation。

## 正式 Gateway

Tauri 从 active profile 与 provider contract 解析 launch plan；saved-model profile
另外生成 static model catalog，Codex adapter 再合并独立 network route。Skill
compatibility 使用的 capability catalog 不参与正式 Gateway launch plan。随后以
配置持久化的 loopback port、path secret 启动 Gateway；每次 spawn 都重新生成
launch ID，作为该进程的运行身份注入。普通 API-key provider 按 contract 注入
对应 credential env；Codex 不使用通用 credential env，而由 Gateway 读取
CSSwitch 私有认证状态，并接收已校验的独立 network route。

正式 server：

- 接受 Science 的 Anthropic-compatible endpoint；
- 按 provider contract 路由 Anthropic Messages、OpenAI Chat/Responses、SSE、tools/tool results；
- 提供 `/v1/models` 与严格 Science selector 映射；
- Codex profile 使用 CSSwitch 私有 auth、动态 model catalog 与 Responses transport；
- `CONNECT` 在 path-secret 认证前分派，只按 Anthropic/Claude hostname denylist
  拒绝目标；DNS resolver 本身没有 deadline，DNS 返回后的地址连接共享剩余
  10 秒预算，建立后的双向转发没有 session deadline、idle timeout、byte cap
  或并发连接/session-count 上限；accepted connection 与双向复制直接创建线程。
  listener 虽为 loopback，这仍是任何本机进程可使用的通用 TCP tunnel，不是
  受 path-secret 保护的产品 API。

API key 会作为入站 Tauri command/IPC 参数进入进程，并进入正式 Gateway 的
credential env / launch context；它不进 argv 或普通日志，出站配置 DTO 只返回
掩码而不回传完整 key。

## scratch Gateway

scratch 使用同一 binary 和 provider contract，但有独立：

- 临时 loopback port；
- 临时 secret；
- `CSSWITCH_GATEWAY_INTENT`（model discovery 或 message probe）；
- child guard 与超时预算；
- 候选 credential/base/model env。

scratch 结果只验证当前候选连接/模型请求。它不提交正式 `AppState` identity、runtime binding、Science route 或产品 live 结论；mock/loopback 也不能写成真实 provider。

## Codex 路由

Codex 是默认关闭的独立产品边界：

```text
Desktop Codex command
  -> Gateway codex-auth / private files
  -> dynamic account model catalog
  -> Gateway Responses transport
  -> Codex upstream
```

auth mutation 使用私有文件锁；动态 model catalog 使用自己的进程内 mutex 与
cache file lock；inference 读取并核对 auth epoch/generation 一致的 snapshot，
但不取得 auth mutation lock，也没有独立 inference lock。三者都不读取原生
`~/.codex`。network route 是独立的 `Config.codex_network` 配置快照，在 launch
时单独解析并注入 Gateway，不写入 auth record generation/file lock；修改该配置
时仍取得 Desktop `CodexAuthSupervisor` mutation lease 以串行化控制面操作。
Codex 失败不改变其他 provider 的认证状态。

## Skill stdio MCP

`skill-install-mcp` 是 CSSwitch 外部 Skill 工作流专用 connector：

- 只接受限界 bridge dir/token/request；
- host side 完成 archive 下载、验证、提交、native attach/readback；
- install/uninstall/poll 共享 request ID 与 terminal response；
- 单 Skill attach 后仍要求会话 `skill(skill_name)` 验证 load；
- bundle attach 不等于每个成员已触发或领域执行成功。

它不是通用 MCP 管理器、Directory connector、hosted MCP 或用户自定义 MCP UI。

## Science control

`science-control configure-third-party` 只接受有 nonce 的 loopback control URL，并执行窄范围步骤：

1. attach CSSwitch route Skill；
2. attach CSSwitch internal connector；
3. 清理旧 connector；
4. detach 被禁用的官方远程管理入口；
5. 写入受管 custom prompt。

该序列不是原子事务；失败产生 warning，已完成步骤不会自动回滚，也不阻断普通
Science 启动。drift 必须分两类：local MCP/route 文件不匹配时，运行中路径只读
检查并要求 restart，`force=true` 也不会越过该结果；本地注册文件已经匹配时，
route marker 过期会让普通路径立即对运行中的 OPERON 重新执行配置，而 doctor
的 `force=true` 即使 marker 仍 current 也会强制执行同一组 attach/detach、
connector 清理与 custom-prompt 更新。

## 网络与协议边界

- Gateway 与 Science UI 只绑定 loopback；
- path secret 认证本地 inference 路径；
- raw `CONNECT` 在 HTTP path-secret 认证前分派，只有 hostname denylist；DNS
  resolver 没有 deadline，地址连接共享剩余 dial budget，已建立 session 没有
  时长/流量上限。它只是 loopback 可达的 TCP transport，不证明
  HTTP/SSE/Streamable HTTP MCP；
- Science app proxy、sandbox network、package mirror 与 Gateway provider egress 是不同网络面；
- Codex 有独立网络 route，不能代表 Science network preference；
- server/client error 按稳定 code/category 投影时才可跨层引用，provider body、HTML challenge、timeout 和 protocol mismatch 不互相混写。

## 失败与证据

| 失败面 | 影响 |
|---|---|
| formal Gateway spawn/health/catalog | 可阻断一键开始 |
| scratch candidate failure | 明确 `Auth` / `ModelError` 拒绝候选落盘；`Ambiguous` / `NoResponse` / `Unsupported` 可将连接以 `validated=false`、`committed=true` 保存，等待一键开始复验，但不提交正式 runtime 状态 |
| Codex auth/catalog/transport | 通常只阻断对应 Codex 操作；active 或 prior running Gateway 为 Codex、下一次一键开始需要 proof 时可阻断该次启动 |
| Skill stdio/Science control | 降级外部 Skill，可 warning |
| raw CONNECT target | 只说明目标 transport 失败，不证明 MCP 产品结论 |

source/unit、scratch、local mock、真实 provider、final artifact、installed/live 必须分别记录。
