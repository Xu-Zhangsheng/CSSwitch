const OPERATIONS = new Set([
  "set_mode_official",
  "set_settings_destructive",
  "codex_auth_start",
  "codex_auth_logout",
  "set_codex_network",
  "clear_applied_profile_key",
  "delete_applied_profile",
  "set_mode",
  "set_settings",
  "clear_profile_key",
  "delete_profile",
  "set_active_profile",
  "update_profile_connection",
  "codex_ensure_profile",
]);
const ERROR_CODES = new Set(["config_mutation_failed", "config_mutation_attention", "mutation_conflict"]);
const PHASES = new Set(["admission", "intent", "effect", "terminal", "unknown"]);
const CONFIG_STATES = new Set(["before", "after", "unknown", "unchanged", "committed"]);
const RUNTIME_STATES = new Set(["preserved", "stopped", "restored", "unknown"]);
const DISPOSITIONS = new Set(["completed", "no_change", "attention", "committed", "created", "existing", "rejected", "inconclusive"]);
const PROFILE_VALIDATIONS = new Set(["accepted", "rejected", "inconclusive", "not_run"]);
const HEX32 = /^[0-9a-f]{32}$/;

function jsonObject(value) {
  if (typeof value === "string") {
    try { value = JSON.parse(value); } catch { return null; }
  }
  return value && typeof value === "object" && !Array.isArray(value) ? value : null;
}

function boundedToken(value, max = 96) {
  return typeof value === "string" && value.length > 0 && value.length <= max && /^[A-Za-z0-9_.:-]+$/.test(value);
}

function hasOnlyKeys(value, allowed) {
  return Object.keys(value).every((key) => allowed.has(key));
}

export function parseConfigMutationCommandError(raw) {
  const value = jsonObject(raw);
  if (!value || !("code" in value)) return null;
  if (!ERROR_CODES.has(value.code)) {
    if (typeof value.code === "string" && (value.code.startsWith("config_mutation") || value.code === "mutation_conflict")) {
      throw new Error("CSSwitch 普通配置变更错误协议不匹配。");
    }
    return null;
  }
  const allowed = new Set([
    "schema_version", "code", "operation", "cause", "phase", "retryable",
    "attention_required", "receipt_retained", "config_state", "runtime_state",
    "message",
  ]);
  if (!hasOnlyKeys(value, allowed) || value.schema_version !== 1 || !OPERATIONS.has(value.operation) ||
      !boundedToken(value.cause) || !PHASES.has(value.phase) || typeof value.retryable !== "boolean" ||
      typeof value.attention_required !== "boolean" || typeof value.receipt_retained !== "boolean" ||
      !CONFIG_STATES.has(value.config_state) || !RUNTIME_STATES.has(value.runtime_state) ||
      (value.message != null && (typeof value.message !== "string" || value.message.length > 512 || /[\u0000\r\n]/.test(value.message)))) {
    throw new Error("CSSwitch 普通配置变更错误协议不匹配。");
  }
  if (value.code === "config_mutation_attention" && (!value.attention_required || !value.receipt_retained || value.retryable)) {
    throw new Error("CSSwitch 普通配置变更 attention 组合不匹配。");
  }
  if (value.code === "mutation_conflict" && (value.attention_required || value.receipt_retained)) {
    throw new Error("CSSwitch 普通配置变更冲突组合不匹配。");
  }
  return {
    schema_version: 1,
    code: value.code,
    operation: value.operation,
    cause: value.cause,
    phase: value.phase,
    retryable: value.retryable,
    attention_required: value.attention_required,
    receipt_retained: value.receipt_retained,
    config_state: value.config_state,
    runtime_state: value.runtime_state,
  };
}

export function formatConfigMutationCommandError(error) {
  if (error.code === "mutation_conflict") return "当前配置仍有另一项受管变更；请先完成恢复或稍后重试。";
  if (error.attention_required || error.receipt_retained) {
    return "普通配置变更需要人工处理；安全操作记录已保留，界面不会自动重发。";
  }
  return error.retryable ? "普通配置变更未提交，可以重试。" : "普通配置变更未提交，请先修复当前问题。";
}

export function parseConfigMutationOutcome(raw) {
  const value = jsonObject(raw);
  if (!value || value.schema_version !== 1 || !OPERATIONS.has(value.operation) ||
      !DISPOSITIONS.has(value.disposition) || !CONFIG_STATES.has(value.config_state) ||
      !RUNTIME_STATES.has(value.runtime_state) || !boundedToken(value.recovery_state) ||
      value.operation_id == null) return null;
  if (!HEX32.test(String(value.operation_id))) {
    throw new Error("CSSwitch 普通配置变更结果 operation_id 不匹配。");
  }
  return {
    schema_version: 1,
    operation_id: String(value.operation_id),
    operation: value.operation,
    disposition: value.disposition,
    config_state: value.config_state,
    runtime_state: value.runtime_state,
    recovery_state: value.recovery_state,
  };
}

export function parseConfigIntentOutcome(raw) {
  const value = jsonObject(raw);
  if (!value || value.schema_version !== 1 || !OPERATIONS.has(value.operation) ||
      !HEX32.test(String(value.intent_id || "")) || !DISPOSITIONS.has(value.disposition) ||
      !CONFIG_STATES.has(value.config_state)) return null;
  if (value.selected_profile_id != null && (typeof value.selected_profile_id !== "string" || value.selected_profile_id.includes("/"))) {
    throw new Error("CSSwitch 普通配置 intent profile id 不匹配。");
  }
  if (value.applied_profile_id != null && (typeof value.applied_profile_id !== "string" || value.applied_profile_id.includes("/"))) {
    throw new Error("CSSwitch 普通配置 intent applied profile id 不匹配。");
  }
  if (value.validation != null && !PROFILE_VALIDATIONS.has(value.validation)) {
    throw new Error("CSSwitch 普通配置 intent validation 不匹配。");
  }
  if (value.science_running != null && typeof value.science_running !== "boolean") {
    throw new Error("CSSwitch 普通配置 intent runtime 状态不匹配。");
  }
  return {
    schema_version: 1,
    operation: value.operation,
    intent_id: String(value.intent_id),
    disposition: value.disposition,
    config_state: value.config_state,
    selected_profile_id: value.selected_profile_id ?? null,
    applied_profile_id: value.applied_profile_id ?? null,
    validation: value.validation ?? null,
    science_running: value.science_running ?? null,
  };
}

export function parseConfigMutationResponse(raw) {
  return parseConfigMutationOutcome(raw) || parseConfigIntentOutcome(raw);
}
