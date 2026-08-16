import {
  CODEX_AUTH_REASONS,
  formatCodexAuthCommandError,
  parseCodexAuthCommandError,
} from "./codex-auth-protocol.js";
import {
  formatCodexDisableCommandError,
  parseCodexDisableCommandError,
} from "./codex-disable-protocol.js";
import {
  formatConfigMutationCommandError,
  parseConfigIntentOutcome,
  parseConfigMutationCommandError,
  parseConfigMutationOutcome,
} from "./runtime-mutation-protocol.js";
import {
  PREVIEW,
  completeMockCodexLogin,
  getMockCodexOperation,
  setMockCodexOperation,
} from "./preview-adapter.js";
import { call, listen } from "./ipc-client.js";

const CODEX_TERMINAL_STATES = new Set(["succeeded", "failed", "cancelled"]);
const CODEX_OPERATION_STATES = new Set(["starting", "waiting", "exchanging", "committing", ...CODEX_TERMINAL_STATES]);
const CODEX_ERROR_STAGES = new Set(["proxy_config", "browser_open", "callback_wait", "token_exchange", "refresh", "revoke", "credential_commit", "profile_ensure", "cancelled", "terminal"]);
const CODEX_RESPONSE_KINDS = new Set(["json", "html", "empty", "other", "unknown"]);
const CODEX_TRANSPORT_KINDS = new Set(["timeout", "dns_connect", "proxy_connect", "tls", "http", "unknown", "durable_receipt"]);

export function parseCodexOperationSnapshot(value) {
  if (!value || value.schema_version !== 2 || !/^[0-9a-f]{32}$/.test(String(value.operation_id || "")) ||
      !Number.isSafeInteger(value.sequence) || value.sequence < 1 || value.method !== "browser" ||
      !CODEX_OPERATION_STATES.has(value.state) || !Number.isSafeInteger(value.started_at_ms) || !Number.isSafeInteger(value.updated_at_ms)) {
    throw new Error("CSSwitch Codex 登录 operation 协议不匹配。");
  }
  if (value.verification_url != null || value.user_code != null || value.expires_at_ms != null) {
    throw new Error("CSSwitch Codex 浏览器登录 operation 包含旧设备码字段。");
  }
  if (value.config_mutation_operation_id != null && !/^[0-9a-f]{32}$/.test(String(value.config_mutation_operation_id))) {
    throw new Error("CSSwitch Codex 登录 operation 缺少合法的 Config mutation operation ID。");
  }
  if (value.state !== "starting" && value.config_mutation_operation_id == null) {
    throw new Error("CSSwitch Codex 登录进行中或终态缺少 Config mutation operation ID。");
  }
  if ((["failed", "cancelled"].includes(value.state) && value.error == null) ||
      (!["failed", "cancelled"].includes(value.state) && value.error != null)) {
    throw new Error("CSSwitch Codex 登录 operation state/error 结构不匹配。");
  }
  const snap = {
    schema_version: 2,
    operation_id: String(value.operation_id),
    sequence: value.sequence,
    method: value.method,
    state: value.state,
    started_at_ms: value.started_at_ms,
    updated_at_ms: value.updated_at_ms,
    config_mutation_operation_id: value.config_mutation_operation_id ?? null,
    error: null,
  };
  if (value.error != null) {
    const error = value.error;
    if (!error || typeof error.code !== "string" || !CODEX_ERROR_STAGES.has(error.stage) || typeof error.retryable !== "boolean" ||
        (error.upstream_status != null && (!Number.isSafeInteger(error.upstream_status) || error.upstream_status < 100 || error.upstream_status > 599)) ||
        (error.response_kind != null && !CODEX_RESPONSE_KINDS.has(error.response_kind)) ||
        (error.transport_kind != null && !CODEX_TRANSPORT_KINDS.has(error.transport_kind)) ||
        (error.challenge_detected != null && typeof error.challenge_detected !== "boolean")) {
      throw new Error("CSSwitch Codex 登录错误结构不匹配。");
    }
    const durableAttention = error.code === "config_mutation_attention";
    if (durableAttention !== (error.transport_kind === "durable_receipt") ||
        (durableAttention && (error.retryable || !new Set(["terminal", "profile_ensure"]).has(error.stage))) ||
        (!durableAttention && error.stage === "terminal")) {
      throw new Error("CSSwitch Codex 登录 durable terminal attention 结构不匹配。");
    }
    snap.error = {
      code: error.code, stage: error.stage, retryable: error.retryable,
      upstream_status: error.upstream_status ?? null,
      response_kind: error.response_kind ?? null,
      challenge_detected: error.challenge_detected ?? null,
      transport_kind: error.transport_kind ?? null,
    };
  }
  return snap;
}

export function codexOperationSnapshotTransitionAccepted(current, next, allowReplacement) {
  if (current && current.operation_id === next.operation_id &&
      current.config_mutation_operation_id != null &&
      next.config_mutation_operation_id !== current.config_mutation_operation_id) {
    throw new Error("CSSwitch Codex 登录 operation 的 Config mutation identity 被替换。");
  }
  if (current && current.operation_id !== next.operation_id && !allowReplacement &&
      (!CODEX_TERMINAL_STATES.has(current.state) || next.started_at_ms < current.started_at_ms)) {
    return false;
  }
  return !(current && current.operation_id === next.operation_id && next.sequence <= current.sequence);
}

export function unwrapCodexAuthEnvelope(response, expectedCommand) {
  if (!response || response.schema_version !== 3 || response.command !== expectedCommand || typeof response.ok !== "boolean") {
    throw new Error("CSSwitch Codex 认证响应协议不匹配。");
  }
  if (!response.ok) {
    throw new Error("CSSwitch Codex 认证失败响应未通过结构化错误通道交付。");
  }
  const status = response.status;
  const validHash = status && (status.account_hash === null || typeof status.account_hash === "string");
  const validExpiry = status && (status.expires_at === null || Number.isSafeInteger(status.expires_at));
  const validEpoch = status && (status.auth_epoch === null || typeof status.auth_epoch === "string");
  const validReason = status && CODEX_AUTH_REASONS.has(status.reason);
  const validCombination = status && ((status.authenticated && status.reason === "ready") || (!status.authenticated && status.reason !== "ready"));
  if (!status || typeof status.authenticated !== "boolean" || typeof status.expiry_state !== "string" ||
      !validHash || !validExpiry || !validEpoch || !Number.isSafeInteger(status.auth_generation) || status.auth_generation < 0) {
    throw new Error("CSSwitch Codex 认证状态结构不匹配。");
  }
  if (!validReason || !validCombination) throw new Error("CSSwitch Codex 认证状态 reason 不匹配。");
  if (expectedCommand === "logout") {
    const warning = response.warning;
    const validWarning = warning == null || (
      warning && typeof warning === "object" && !Array.isArray(warning) &&
      Object.keys(warning).length === 2 &&
      warning.code === "revoke_skipped" && warning.reason === "proxy_config_invalid"
    );
    if (!/^[0-9a-f]{32}$/.test(String(response.config_mutation_operation_id || "")) ||
        status.authenticated !== false || status.reason !== "state_uncommitted" ||
        status.account_hash !== null || status.expiry_state !== "missing" ||
        status.expires_at !== null || !/^[0-9a-f]{32}$/.test(String(status.auth_epoch || "")) ||
        status.auth_generation < 1 || !validWarning) {
      throw new Error("CSSwitch Codex logout durable terminal 协议不匹配。");
    }
  }
  return {
    authenticated: status.authenticated,
    account_hash: status.account_hash,
    expiry_state: status.expiry_state,
    expires_at: status.expires_at,
    auth_epoch: status.auth_epoch,
    auth_generation: status.auth_generation,
    reason: status.reason,
  };
}

export function createCodexController({
  els,
  getConfigState,
  isBusy,
  isActivationInFlight,
  setMsg,
  setBusy,
  loadConfig,
  renderList,
  isCodexSource,
  confirmAction,
  getStatusTimer,
  setStatusTimer,
  refreshStatus,
}) {
  // 这些状态只属于 CSSwitch Codex OAuth/network/downgrade 控制面。
  let codexAuthState = null;
  let codexAuthOperation = null;
  let codexLoginStarting = false;
  let codexProfileRepairNeeded = false;
  let codexNetworkSaving = false;
  let handledCodexTerminal = "";

function clearStaleCodexAuthState() {
  codexAuthState = null;
  codexProfileRepairNeeded = false;
  renderCodexAuthState();
}

function runtimeCommandErrorText(error) {
  let mutationError;
  try {
    mutationError = parseConfigMutationCommandError(error);
  } catch (protocolError) {
    return protocolError.message;
  }
  if (mutationError) return formatConfigMutationCommandError(mutationError);
  let disableError;
  try {
    disableError = parseCodexDisableCommandError(error);
  } catch (protocolError) {
    return protocolError.message;
  }
  if (disableError) return formatCodexDisableCommandError(disableError);
  let authError;
  try {
    authError = parseCodexAuthCommandError(error);
  } catch (protocolError) {
    clearStaleCodexAuthState();
    return protocolError.message;
  }
  if (!authError) {
    if (typeof error === "string") return error;
    if (error instanceof Error && typeof error.message === "string") return error.message;
    return "后端返回了无法识别的错误。";
  }
  clearStaleCodexAuthState();
  return formatCodexAuthCommandError(authError);
}

function renderCodexAuthState() {
  if (!els.codexAuthStatus) return;
  if (codexOperationActive()) {
    renderCodexOperation();
    return;
  }
  if (!codexAuthState) {
    els.codexAuthStatus.textContent = "尚未检查 CSSwitch Codex 登录状态。";
  } else if (!codexAuthState.authenticated) {
    els.codexAuthStatus.textContent = ["state_missing", "state_uncommitted"].includes(codexAuthState.reason)
      ? "CSSwitch Codex 尚未登录（原生 Codex 登录状态未读取）。"
      : "CSSwitch Codex 本地认证记录不完整；请检查状态，不会自动修复或重新登录。";
  } else {
    const expiryLabels = { valid: "有效", expiring: "即将到期", expired: "已到期", unknown: "有效期未知" };
    const account = codexAuthState.account_hash ? String(codexAuthState.account_hash).slice(0, 8) : "未知";
    const expiry = expiryLabels[codexAuthState.expiry_state] || "状态未知";
    els.codexAuthStatus.textContent = "已登录 · 账号标识 " + account + "… · " + expiry;
  }
  syncCodexControls();
}

function hasCodexProfile() {
  return (getConfigState().profiles || []).some((profile) => isCodexSource(profile));
}

function refreshCodexProfileRepairState() {
  codexProfileRepairNeeded = !!(
    codexAuthState && codexAuthState.authenticated &&
    getConfigState().experimental_codex_enabled && !hasCodexProfile()
  );
}

function codexOperationActive() {
  return !!(codexAuthOperation && !CODEX_TERMINAL_STATES.has(codexAuthOperation.state));
}

function codexOperationErrorText(error) {
  const labels = {
    oauth_challenge_response: "认证请求遇到上游安全挑战；请检查当前出口或代理路线。",
    oauth_unexpected_content_type: "认证端点返回了意外内容（例如 HTML），请检查代理或上游安全挑战。",
    proxy_connect_failed: "无法连接所选 Codex 代理。",
    tls_failed: "Codex 认证 TLS 连接失败。",
    oauth_network_error: "Codex 认证网络请求失败。",
    callback_timeout: "等待登录超时。",
    callback_unavailable: "本地登录回调端口 1455/1457 不可用。",
    browser_open_failed: "无法打开系统浏览器。",
    oauth_denied: "登录未获授权。",
    auth_cancelled: "登录已取消。",
    keychain_unavailable: "旧版 CSSwitch 本地认证存储不可用。",
    auth_storage_error: "CSSwitch 无法安全保存 Codex 授权。",
    identity_mismatch: "安装包内 Gateway 与 Desktop 不匹配。",
    profile_ensure_failed: "授权已保存，但 Codex 配置尚未创建。无需重新登录，可直接补建配置。",
    config_mutation_attention: "Codex 登录已停止，durable mutation 记录需要人工处理。",
  };
  let text = labels[error && error.code] || "Codex 登录未完成。";
  if (error && error.upstream_status) text += " 上游状态码 " + error.upstream_status + "。";
  return text;
}

function renderCodexOperation() {
  if (!els.codexAuthStatus || !codexAuthOperation) return;
  const op = codexAuthOperation;
  const labels = {
    starting: "正在启动 Codex 登录…",
    waiting: "正在等待浏览器授权…",
    exchanging: "授权已收到，正在交换令牌…",
    committing: "正在原子写入 CSSwitch 私有认证文件；此时取消不会中断提交。",
    succeeded: "Codex 登录成功，正在刷新状态…",
    failed: codexOperationErrorText(op.error),
    cancelled: "Codex 登录已取消；凭据与 generation 未变。",
  };
  els.codexAuthStatus.textContent = labels[op.state] || "Codex 登录状态未知。";
  syncCodexControls();
}

function acceptCodexOperationSnapshot(raw, allowReplacement) {
  const next = parseCodexOperationSnapshot(raw);
  if (!codexOperationSnapshotTransitionAccepted(codexAuthOperation, next, allowReplacement)) return;
  codexAuthOperation = next;
  codexLoginStarting = false;
  renderCodexOperation();
  if (CODEX_TERMINAL_STATES.has(next.state)) {
    const terminalKey = next.operation_id + ":" + next.sequence;
    if (handledCodexTerminal === terminalKey) return;
    handledCodexTerminal = terminalKey;
    if (next.state === "succeeded") {
      Promise.all([
        refreshCodexAuthStatus({ quiet: true }),
        loadConfig({ throwOnError: true }),
      ]).then(([authenticated]) => {
        refreshCodexProfileRepairState();
        syncCodexControls();
        if (!authenticated || !hasCodexProfile()) {
          throw new Error("登录终态与本地授权/配置状态不一致。");
        }
        const active = (getConfigState().profiles || []).find((profile) => profile.id === getConfigState().active_id);
        if (active && isCodexSource(active)) {
          setMsg("CSSwitch Codex 登录完成，Codex 配置已就绪。它仍是当前配置，但登录期间受管 Science/Gateway 已停止且未自动重启；请点击“一键开始”。原生 Codex OAuth 未被读取或改动。", "ok");
        } else {
          setMsg("CSSwitch Codex 登录完成，Codex 配置已就绪。下一步可在“模型连接 > 配置方案”中设为当前；原生 Codex OAuth 未被读取或改动。", "ok");
        }
      }).catch((error) => {
        refreshCodexProfileRepairState();
        syncCodexControls();
        setMsg("Codex 授权已完成，但刷新配置状态失败：" + error + " 请检查状态；若显示补建入口，无需重新登录。", "err");
      });
    } else if (next.state === "failed") {
      if (next.error && next.error.code === "profile_ensure_failed") {
        refreshCodexAuthStatus({ quiet: true }).then((authenticated) => {
          refreshCodexProfileRepairState();
          syncCodexControls();
          setMsg(authenticated
            ? "Codex 授权已安全保存，但配置尚未创建。请点“补建 Codex 配置”；无需重新登录。"
            : "Codex 配置创建失败，且无法确认授权状态。请先检查状态。", "err");
        });
      } else {
        codexProfileRepairNeeded = false;
        syncCodexControls();
        setMsg("CSSwitch Codex 登录失败：" + codexOperationErrorText(next.error), "err");
      }
    } else {
      codexProfileRepairNeeded = false;
      syncCodexControls();
      setMsg("CSSwitch Codex 登录已取消；未提交新的本地认证 generation。", "ok");
    }
  }
}

async function registerCodexAuthEvents() {
  await listen("codex-auth://operation", (event) => {
    try { acceptCodexOperationSnapshot(event.payload, false); }
    catch (e) { setMsg("Codex 登录事件被安全拒绝：" + e, "err"); }
  });
  try {
    const snapshot = await call("codex_auth_operation_status");
    if (snapshot) acceptCodexOperationSnapshot(snapshot, true);
  } catch (e) {
    setMsg("无法恢复 Codex 登录 operation 状态：" + e, "err");
  }
}

function syncCodexControls() {
  if (!els.codexEnabled) return;
  const enabled = !!getConfigState().experimental_codex_enabled;
  const authActive = codexOperationActive() || codexLoginStarting;
  const locked = isBusy() || isActivationInFlight() || codexNetworkSaving;
  els.codexEnabled.checked = enabled;
  els.codexEnabled.disabled = locked || authActive;
  els.codexStatusBtn.disabled = locked || authActive;
  els.codexLoginBtn.disabled = locked || authActive || !enabled;
  els.codexCancelBtn.disabled = locked || !authActive || !codexAuthOperation;
  els.codexLogoutBtn.disabled = locked || authActive || !(codexAuthState && codexAuthState.authenticated);
  if (els.codexProfileRepairBox) els.codexProfileRepairBox.hidden = !enabled || !codexProfileRepairNeeded;
  if (els.codexRepairProfileBtn) {
    els.codexRepairProfileBtn.disabled = locked || authActive || !codexProfileRepairNeeded || !(codexAuthState && codexAuthState.authenticated);
  }
  if (els.codexNetworkMode) els.codexNetworkMode.disabled = locked || authActive;
  if (els.codexProxyUrl) els.codexProxyUrl.disabled = locked || authActive || els.codexNetworkMode.value !== "custom";
  if (els.codexNetworkSaveBtn) els.codexNetworkSaveBtn.disabled = locked || authActive;
  const codexCount = (getConfigState().profiles || []).filter((profile) => isCodexSource(profile)).length;
  els.codexDowngradeBox.hidden = codexCount === 0;
  els.codexDowngradeBtn.disabled = locked || authActive || codexCount === 0;
}

function renderCodexNetwork() {
  if (!els.codexNetworkMode) return;
  const settings = getConfigState().codex_network || { mode: "auto", proxy_url: "" };
  const resolved = getConfigState().codex_network_resolved || { source: "direct", proxy_scheme: null };
  els.codexNetworkMode.value = settings.mode === "custom" ? "custom" : "auto";
  els.codexProxyUrl.value = settings.proxy_url || "";
  const sourceLabels = {
    direct: "直接 socket，可能由系统 TUN 接管",
    env_https: "来自 HTTPS_PROXY / https_proxy",
    env_all: "来自 ALL_PROXY / all_proxy",
    custom: "CSSwitch 显式代理",
    invalid: "代理配置非法",
  };
  const scheme = resolved.proxy_scheme ? " · " + resolved.proxy_scheme : "";
  els.codexNetworkResolved.textContent = "当前路线：" + (sourceLabels[resolved.source] || "未知") + scheme + "。";
  syncCodexControls();
}

function codexNetworkModeChanged() {
  if (els.codexNetworkMode.value !== "custom") els.codexProxyUrl.value = "";
  syncCodexControls();
}

async function saveCodexNetwork() {
  if (codexNetworkSaving || codexOperationActive()) return;
  const settings = {
    mode: els.codexNetworkMode.value === "custom" ? "custom" : "auto",
    proxy_url: els.codexNetworkMode.value === "custom" ? els.codexProxyUrl.value.trim() : "",
  };
  codexNetworkSaving = true;
  syncCodexControls();
  setMsg("正在校验 Codex 网络路线并停止受管 Codex 链路；保存后不会自动重启…");
  try {
    const result = await call("set_codex_network", { settings });
    const mutationOutcome = parseConfigMutationOutcome(result);
    const intentOutcome = parseConfigIntentOutcome(result);
    const destructiveCompleted = mutationOutcome &&
      mutationOutcome.operation === "set_codex_network" &&
      mutationOutcome.disposition === "completed" &&
      mutationOutcome.config_state === "after" &&
      ["stopped", "preserved"].includes(mutationOutcome.runtime_state) &&
      mutationOutcome.recovery_state === "not_needed" &&
      typeof mutationOutcome.operation_id === "string";
    const intentCommitted = intentOutcome &&
      intentOutcome.operation === "set_codex_network" &&
      ["committed", "no_change"].includes(intentOutcome.disposition) &&
      intentOutcome.config_state === "committed" &&
      intentOutcome.validation === "not_run" &&
      intentOutcome.science_running === false;
    if (!result || (!destructiveCompleted && !intentCommitted) ||
        result.mode !== settings.mode || result.restarted !== false) {
      throw new Error("Codex 网络设置响应不一致。");
    }
    getConfigState().codex_network = settings;
    getConfigState().codex_network_resolved = { source: result.source, proxy_scheme: result.proxy_scheme ?? null };
    renderCodexNetwork();
    setMsg("Codex 网络路线已保存；受管 Codex Science 与 Gateway 保持停止，其他 provider 未受影响。", "ok");
  } catch (e) {
    setMsg("Codex 网络路线未更改：" + runtimeCommandErrorText(e), "err");
  } finally {
    codexNetworkSaving = false;
    syncCodexControls();
  }
}

async function refreshCodexAuthStatus(options) {
  const opts = options || {};
  if (!opts.quiet) setMsg("正在检查 CSSwitch 自有 Codex 登录状态…");
  try {
    codexAuthState = unwrapCodexAuthEnvelope(await call("codex_auth_status"), "status");
    refreshCodexProfileRepairState();
    renderCodexAuthState();
    if (!opts.quiet) {
      const incomplete = !codexAuthState.authenticated && !["state_missing", "state_uncommitted"].includes(codexAuthState.reason);
      setMsg(codexAuthState.authenticated
        ? "CSSwitch Codex 已登录；没有读取或修改原生 Codex 登录。"
        : incomplete
        ? "CSSwitch Codex 本地认证记录不完整；不会自动修复、退出或重新登录。"
        : "CSSwitch Codex 尚未登录。请先启用实验入口，再点“登录 Codex”。",
      codexAuthState.authenticated ? "ok" : "err");
    }
    return !!codexAuthState.authenticated;
  } catch (e) {
    codexAuthState = null;
    codexProfileRepairNeeded = false;
    renderCodexAuthState();
    if (!opts.quiet) setMsg("检查 CSSwitch Codex 登录状态失败：" + runtimeCommandErrorText(e), "err");
    return false;
  }
}

async function checkCodexAuth() {
  setBusy(true, { kind: "codexAuth" });
  await refreshCodexAuthStatus();
  setBusy(false);
}

async function toggleCodexFeature() {
  const desired = !!els.codexEnabled.checked;
  const previous = !!getConfigState().experimental_codex_enabled;
  let backendChanged = false;
  setBusy(true, { kind: "codexFeature" });
  els.codexEnabled.checked = desired;
  setMsg(desired ? "正在启用 Codex 实验入口…" : "正在安全停止 Codex 链路并关闭实验入口…");
  try {
    const result = await call("set_experimental_codex_enabled", { enabled: desired });
    backendChanged = true;
    if (!result || result.experimental_codex_enabled !== desired) {
      throw new Error("后端返回的 Codex 实验开关状态不一致。");
    }
    getConfigState().experimental_codex_enabled = desired;
    if (!desired) getConfigState().templates = (getConfigState().templates || []).filter((t) => !isCodexSource(t));
    renderList();
    syncCodexControls();
    await loadConfig({ throwOnError: true });
    setMsg(desired
      ? "Codex 实验入口已启用。下一步请在“设置 > Codex 账号与连接”登录 CSSwitch Codex。"
      : "Codex 实验入口已关闭；CSSwitch 自有 OAuth 凭据仍保留，可在此处检查或退出。", "ok");
  } catch (e) {
    let disableError = null;
    try {
      disableError = parseCodexDisableCommandError(e);
    } catch (_) {
      // runtimeCommandErrorText owns the stable protocol-mismatch projection.
    }
    const committedWithAttention = !backendChanged && !desired && disableError?.config_state === "disabled";
    if (committedWithAttention) {
      backendChanged = true;
    }
    if (backendChanged) {
      getConfigState().experimental_codex_enabled = desired;
      if (!desired) getConfigState().templates = (getConfigState().templates || []).filter((t) => !isCodexSource(t));
      renderList();
      syncCodexControls();
      setMsg(committedWithAttention
        ? runtimeCommandErrorText(e)
        : "Codex 实验入口已在后端" + (desired ? "启用" : "关闭") + "，但刷新完整配置失败：" + runtimeCommandErrorText(e) + " 请重新打开 CSSwitch 确认其余界面。", "err");
    } else {
      getConfigState().experimental_codex_enabled = previous;
      els.codexEnabled.checked = previous;
      setMsg("Codex 实验入口未更改：" + runtimeCommandErrorText(e), "err");
    }
  } finally {
    setBusy(false);
  }
}

async function startCodexLogin() {
  if (!getConfigState().experimental_codex_enabled) {
    setMsg("请先启用 Codex 实验入口。", "err");
    return;
  }
  if (codexOperationActive() || codexLoginStarting) return;
  codexProfileRepairNeeded = false;
  codexLoginStarting = true;
  syncCodexControls();
  setMsg("正在打开浏览器登录；最多等待 5 分钟。请完成授权后回到这里…");
  try {
    const snapshot = await call("codex_auth_start");
    acceptCodexOperationSnapshot(snapshot, true);
    if (PREVIEW) previewCodexLogin();
  } catch (e) {
    codexLoginStarting = false;
    syncCodexControls();
    setMsg("CSSwitch Codex 登录失败：" + runtimeCommandErrorText(e), "err");
  }
}

function previewCodexLogin() {
  const base = { ...getMockCodexOperation() };
  setTimeout(() => {
    if (!getMockCodexOperation() || getMockCodexOperation().state === "cancelled") return;
    setMockCodexOperation({ ...base, sequence: 2, state: "waiting", updated_at_ms: Date.now() });
    acceptCodexOperationSnapshot(getMockCodexOperation(), false);
  }, 250);
  setTimeout(() => {
    if (!getMockCodexOperation() || getMockCodexOperation().state === "cancelled") return;
    completeMockCodexLogin();
    setMockCodexOperation({ ...base, sequence: 3, state: "succeeded", updated_at_ms: Date.now() });
    acceptCodexOperationSnapshot(getMockCodexOperation(), false);
  }, 900);
}

async function cancelCodexLogin() {
  if (!codexOperationActive() || !codexAuthOperation) return;
  els.codexCancelBtn.disabled = true;
  try {
    const result = await call("codex_auth_cancel", { operationId: codexAuthOperation.operation_id });
    if (!result || !["accepted", "commit_in_progress", "already_terminal"].includes(result.disposition)) {
      throw new Error("取消响应协议不匹配。");
    }
    setMsg(result.disposition === "commit_in_progress"
      ? "本地认证提交已经开始；将由最终成功或失败决定状态。"
      : result.disposition === "already_terminal" ? "该登录 operation 已结束。" : "取消请求已被 sidecar 接受，正在回收…");
    if (PREVIEW && getMockCodexOperation()) acceptCodexOperationSnapshot(getMockCodexOperation(), false);
  } catch (e) {
    setMsg("取消 Codex 登录失败：" + e, "err");
  } finally {
    syncCodexControls();
  }
}

async function repairCodexProfile() {
  if (isBusy() || codexOperationActive() || !codexProfileRepairNeeded) return;
  setBusy(true, { kind: "codexProfileRepair" });
  setMsg("正在用已保存的授权补建 Codex 配置；不会重新登录或切换当前 provider…");
  let disposition = null;
  try {
    const result = await call("codex_ensure_profile");
    const intent = parseConfigIntentOutcome(result);
    if (!intent || intent.operation !== "codex_ensure_profile") throw new Error("补建配置结果协议不匹配。");
    if (!result || !["created", "existing"].includes(result.disposition) ||
        typeof result.profile_id !== "string" || !result.profile_id || result.profile_id.length > 128) {
      throw new Error("补建配置响应协议不匹配。");
    }
    disposition = result.disposition;
    await loadConfig({ throwOnError: true });
    if (!hasCodexProfile()) throw new Error("补建后未能在配置列表中确认 Codex。");
    codexProfileRepairNeeded = false;
    syncCodexControls();
    setMsg(result.disposition === "created"
      ? "Codex 配置已补建。下一步可在“模型连接 > 配置方案”中设为当前。"
      : "Codex 配置已存在并确认就绪。下一步可设为当前。", "ok");
  } catch (e) {
    refreshCodexProfileRepairState();
    setMsg(disposition === "created"
      ? "Codex 配置已在后端补建，但界面刷新或回读确认失败：" + runtimeCommandErrorText(e) + " 请重新打开配置页确认，不要重复补建。"
      : "补建 Codex 配置失败：" + runtimeCommandErrorText(e) + " 已保存的授权不会因此删除，可重试。", "err");
  } finally {
    setBusy(false);
  }
}

function logoutCodex() {
  confirmAction("codex-logout", "将退出 CSSwitch 自有 Codex 登录；不会退出原生 Codex", doLogoutCodex);
}

async function doLogoutCodex() {
  setBusy(true, { kind: "codexAuth" });
  setMsg("正在安全停止 Codex 链路并退出 CSSwitch Codex…");
  try {
    const response = await call("codex_auth_logout");
    codexAuthState = unwrapCodexAuthEnvelope(response, "logout");
    codexProfileRepairNeeded = false;
    renderCodexAuthState();
    const revokeSkipped = response && response.warning && response.warning.code === "revoke_skipped" && response.warning.reason === "proxy_config_invalid";
    setMsg(revokeSkipped
      ? "已清除 CSSwitch Codex 本地凭据；因代理配置非法，远端 revoke 已安全跳过。原生 Codex 登录未受影响。"
      : "已退出 CSSwitch Codex；原生 Codex 登录未被读取或修改。", "ok");
  } catch (e) {
    setMsg("退出 CSSwitch Codex 失败：" + runtimeCommandErrorText(e), "err");
  } finally {
    setBusy(false);
  }
}

async function requestCodexDowngrade() {
  if (isBusy() || isActivationInFlight()) return;
  setBusy(true, { kind: "codexDowngradePreview" });
  setMsg("正在生成 Codex 配置降级预览；不会读取本地 OAuth 内容…");
  try {
    const preview = await call("codex_downgrade_preview");
    const profiles = preview && Array.isArray(preview.profiles) ? preview.profiles : [];
    if (preview.schema_version !== 1 || preview.action !== "export_then_remove_all" || !preview.credentials_unchanged || profiles.length < 1 || !preview.preview_fingerprint) {
      throw new Error("后端没有返回可执行的完整 Codex 降级预览。");
    }
    const ids = profiles.map((profile) => String(profile.id || ""));
    if (ids.some((id) => !id) || new Set(ids).size !== ids.length) {
      throw new Error("Codex 降级预览包含无效或重复 profile ID。");
    }
    const names = profiles.map((profile) => String(profile.name || profile.id)).join("、");
    setBusy(false);
    confirmAction(
      "codex-downgrade:" + ids.join(","),
      "将导出并移除全部 " + profiles.length + " 个 Codex 配置（" + names + "），原子降为 v2 后立即退出；CSSwitch 本地 OAuth 保留",
      () => doCodexDowngrade(ids, String(preview.preview_fingerprint))
    );
  } catch (e) {
    setBusy(false);
    setMsg("无法准备 Codex 配置降级：" + e, "err");
  }
}

async function doCodexDowngrade(expectedProfileIds, expectedPreviewFingerprint) {
  let downgradeCommitted = false;
  setBusy(true, { kind: "codexDowngrade" });
  setMsg("请选择 Codex profile 元数据导出文件。取消选择不会修改配置…");
  if (getStatusTimer()) {
    clearInterval(getStatusTimer());
    setStatusTimer(null);
  }
  try {
    const result = await call("codex_downgrade_export_all", { expectedProfileIds, expectedPreviewFingerprint });
    if (result && result.status === "CANCELLED") {
      setMsg("已取消导出与降级；配置和本地认证文件均未修改。");
      if (!PREVIEW) setStatusTimer(setInterval(refreshStatus, 2500));
      return;
    }
    if (!result || result.schema_version !== 1 || result.status !== "DOWNGRADED_EXIT_REQUIRED" || !result.exported || !result.credentials_unchanged || !result.app_exit_required) {
      throw new Error("后端降级结果协议不匹配；请勿继续操作，先退出应用并检查备份。");
    }
    downgradeCommitted = true;
    setMsg("Codex 元数据已导出、配置已降为 v2；CSSwitch 本地 OAuth 保留。后端正在终态退出，请安装旧版后再打开。", "ok");
  } catch (e) {
    const terminalFailure = String(e).includes("进程已锁存并强制退出");
    if (terminalFailure) downgradeCommitted = true;
    setMsg(downgradeCommitted
      ? (terminalFailure
          ? "v2 发布后的持久化或回滚状态不确定，后端已锁存所有配置访问并强制退出：" + e + " 本地 OAuth 未被读取或删除。"
          : "配置已安全降为 v2，但应用自动退出失败：" + e + " 请立即手动退出，不要继续操作或重新读取配置；本地 OAuth 保留。")
      : "Codex 配置降级未完成：" + e + " 如果已选择导出文件，它可能已安全落盘；当前配置仍应保持 v3。", "err");
    if (!downgradeCommitted && !PREVIEW && !getStatusTimer()) setStatusTimer(setInterval(refreshStatus, 2500));
  } finally {
    if (!downgradeCommitted) setBusy(false);
  }
}

  return {
    runtimeCommandErrorText,
    registerCodexAuthEvents,
    syncCodexControls,
    renderCodexAuthState,
    refreshCodexProfileRepairState,
    renderCodexNetwork,
    codexNetworkModeChanged,
    saveCodexNetwork,
    checkCodexAuth,
    toggleCodexFeature,
    startCodexLogin,
    cancelCodexLogin,
    repairCodexProfile,
    logoutCodex,
    requestCodexDowngrade,
  };
}
