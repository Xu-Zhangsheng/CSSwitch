import assert from "node:assert/strict";
import test from "node:test";

import { formatCodexAuthCommandError, parseCodexAuthCommandError } from "../desktop/src/codex-auth-protocol.js";

test("accepts every structured authentication error combination", () => {
  for (const reason of ["state_missing", "state_uncommitted", "oauth_missing", "thinking_missing", "record_mismatch"]) {
    assert.equal(parseCodexAuthCommandError({ code: "codex_login_required", reason, retryable: false }).reason, reason);
  }
  const causes = {
    keychain_unavailable: true,
    interaction_timeout: true,
    sidecar_spawn_failed: false,
    sidecar_protocol_error: false,
    identity_mismatch: false,
    auth_state_invalid: false,
    storage_unavailable: true,
    unsupported_platform: false,
    auth_state_changed: true,
  };
  for (const [cause, retryable] of Object.entries(causes)) {
    assert.equal(parseCodexAuthCommandError({ code: "codex_auth_unavailable", cause, retryable }).cause, cause);
  }
  assert.equal(parseCodexAuthCommandError({ code: "codex_auth_busy", retryable: true }).code, "codex_auth_busy");
});

test("keeps normal login, damaged records, and unavailable causes visibly distinct", () => {
  const normal = parseCodexAuthCommandError({ code: "codex_login_required", reason: "state_missing", retryable: false });
  const damaged = parseCodexAuthCommandError({ code: "codex_login_required", reason: "thinking_missing", retryable: false });
  const unavailable = parseCodexAuthCommandError({ code: "codex_auth_unavailable", cause: "keychain_unavailable", retryable: true });
  assert.match(formatCodexAuthCommandError(normal), /尚未登录/);
  assert.match(formatCodexAuthCommandError(damaged), /记录不完整/);
  assert.match(formatCodexAuthCommandError(unavailable), /状态不可用/);
});

test("rejects unknown fields and illegal reason cause retryability combinations", async () => {
  const invalid = [
    { code: "codex_login_required", reason: "ready", retryable: false },
    { code: "codex_login_required", reason: "state_missing", cause: "keychain_unavailable", retryable: false },
    { code: "codex_auth_unavailable", cause: "keychain_unavailable", retryable: false },
    { code: "codex_auth_unavailable", reason: "state_missing", cause: "storage_unavailable", retryable: true },
    { code: "codex_auth_busy", retryable: false },
    { code: "codex_auth_busy", retryable: true, message: "do not parse" },
    { code: "future_code", retryable: false },
  ];
  for (const value of invalid) assert.throws(() => parseCodexAuthCommandError(value));
  assert.equal(parseCodexAuthCommandError("ordinary error"), null);

  globalThis.window = {
    __TAURI__: { core: { invoke: async () => null } },
    location: { search: "" },
  };
  const {
    codexOperationSnapshotTransitionAccepted,
    parseCodexOperationSnapshot,
  } = await import("../desktop/src/codex-controller.js");
  const durableTerminal = {
    schema_version: 2,
    operation_id: "11".repeat(16),
    sequence: 8,
    method: "browser",
    state: "failed",
    started_at_ms: 100,
    updated_at_ms: 200,
    config_mutation_operation_id: "22".repeat(16),
    error: {
      code: "config_mutation_attention",
      stage: "terminal",
      retryable: false,
      transport_kind: "durable_receipt",
    },
  };
  const accepted = parseCodexOperationSnapshot(durableTerminal);
  assert.equal(accepted.error.code, "config_mutation_attention");
  assert.equal(codexOperationSnapshotTransitionAccepted(null, accepted, false), true);
  assert.equal(codexOperationSnapshotTransitionAccepted(accepted, accepted, false), false);
  assert.throws(() => parseCodexOperationSnapshot({
    ...durableTerminal,
    error: { ...durableTerminal.error, transport_kind: "unknown" },
  }));
  assert.throws(() => parseCodexOperationSnapshot({ ...durableTerminal, error: null }));
  assert.throws(() => parseCodexOperationSnapshot({
    ...durableTerminal,
    state: "succeeded",
  }));
  assert.throws(() => codexOperationSnapshotTransitionAccepted(accepted, {
    ...accepted,
    sequence: 9,
    config_mutation_operation_id: "33".repeat(16),
  }, false));
});
