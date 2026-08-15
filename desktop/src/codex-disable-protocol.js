const ERROR_CODES = new Set(["codex_disable_failed", "codex_disable_attention"]);
const ERROR_CAUSES = new Set([
  "receipt_io",
  "receipt_open",
  "receipt_invalid",
  "config_unavailable",
  "mutation_conflict",
  "identity_unproven",
  "config_drift",
  "identity_drift",
  "stop_uncertain",
  "science_stop_failed",
  "gateway_stop_failed",
  "config_commit_failed",
  "restore_failed",
  "restore_uncertain",
  "receipt_cleanup_failed",
]);
const ERROR_PHASES = new Set([
  "intent",
  "stopping",
  "effects_applied",
  "restoring",
  "restored",
  "config_committed",
  "attention",
]);
const CONFIG_STATES = new Set(["unchanged", "disabled"]);

export function parseCodexDisableCommandError(value) {
  if (!value || typeof value !== "object" || Array.isArray(value) || !("code" in value)) return null;
  if (!ERROR_CODES.has(value.code)) {
    if (typeof value.code === "string" && value.code.startsWith("codex_disable")) {
      throw new Error("CSSwitch Codex disable 错误协议不匹配。");
    }
    return null;
  }
  const keys = Object.keys(value);
  if (keys.some((key) => ![
    "code", "cause", "phase", "retryable", "attention_required", "config_state",
  ].includes(key)) || !ERROR_CAUSES.has(value.cause) || !ERROR_PHASES.has(value.phase) ||
      typeof value.retryable !== "boolean" || typeof value.attention_required !== "boolean" ||
      !CONFIG_STATES.has(value.config_state)) {
    throw new Error("CSSwitch Codex disable 错误协议不匹配。");
  }
  if (value.code === "codex_disable_failed") {
    if (!value.retryable || value.attention_required || value.config_state !== "unchanged") {
      throw new Error("CSSwitch Codex disable 可重试错误组合不匹配。");
    }
  } else if (value.retryable || !value.attention_required) {
    throw new Error("CSSwitch Codex disable attention 错误组合不匹配。");
  }
  return {
    code: value.code,
    cause: value.cause,
    phase: value.phase,
    retryable: value.retryable,
    attention_required: value.attention_required,
    config_state: value.config_state,
  };
}

export function formatCodexDisableCommandError(error) {
  if (error.code === "codex_disable_failed") {
    return "Codex 实验入口未更改；受管运行态已保持或精确恢复，可以重试。";
  }
  if (error.config_state === "disabled") {
    return "Codex 实验入口已关闭，但 durable receipt 的终结需要人工注意；安全记录已保留。请重新打开 CSSwitch 检查状态。";
  }
  return "Codex 实验入口未更改；受管运行态的恢复状态需要人工注意，durable receipt 已保留。请重新打开 CSSwitch 检查状态。";
}
