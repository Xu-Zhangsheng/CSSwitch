import test from "node:test";
import assert from "node:assert/strict";
import {
  formatConfigMutationCommandError,
  parseConfigIntentOutcome,
  parseConfigMutationCommandError,
  parseConfigMutationOutcome,
  parseConfigMutationResponse,
} from "../desktop/src/runtime-mutation-protocol.js";

const operationId = "0123456789abcdef0123456789abcdef";

test("parses credential-free completed mutation outcomes and ignores private fields", () => {
  const outcome = parseConfigMutationOutcome({
    schema_version: 1,
    operation_id: operationId,
    operation: "set_codex_network",
    disposition: "completed",
    config_state: "after",
    runtime_state: "stopped",
    recovery_state: "not_needed",
    mode: "custom",
    proxy_url: "must-not-enter-state",
  });
  assert.deepEqual(outcome, {
    schema_version: 1,
    operation_id: operationId,
    operation: "set_codex_network",
    disposition: "completed",
    config_state: "after",
    runtime_state: "stopped",
    recovery_state: "not_needed",
  });
});

test("accepts typed intent outcomes and preserves selected/applied distinction", () => {
  const result = parseConfigIntentOutcome({
    schema_version: 1,
    operation: "set_active_profile",
    intent_id: operationId,
    disposition: "committed",
    config_state: "committed",
    selected_profile_id: "selected",
    applied_profile_id: "applied",
    validation: "accepted",
    science_running: true,
  });
  assert.equal(result.selected_profile_id, "selected");
  assert.equal(result.applied_profile_id, "applied");
  assert.equal(result.science_running, true);
});

test("attention errors are retry-disabled and can be safely formatted", () => {
  const raw = JSON.stringify({
    schema_version: 1,
    code: "config_mutation_attention",
    operation: "delete_applied_profile",
    cause: "gateway_stop_uncertain",
    phase: "effect",
    retryable: false,
    attention_required: true,
    receipt_retained: true,
    config_state: "before",
    runtime_state: "unknown",
  });
  const error = parseConfigMutationCommandError(raw);
  assert.equal(error.attention_required, true);
  assert.equal(error.receipt_retained, true);
  assert.match(formatConfigMutationCommandError(error), /不会自动重发/);
});

test("malformed or unknown envelopes fail closed without becoming retryable success", () => {
  assert.equal(parseConfigMutationResponse({ status: "ok" }), null);
  assert.equal(parseConfigMutationOutcome({
    schema_version: 1,
    operation: "set_codex_network",
    disposition: "completed",
    config_state: "after",
    runtime_state: "preserved",
    recovery_state: "not_needed",
  }), null);
  assert.throws(() => parseConfigMutationCommandError({
    schema_version: 1,
    code: "config_mutation_attention",
    operation: "set_mode_official",
    cause: "bad cause with spaces",
    phase: "effect",
    retryable: false,
    attention_required: true,
    receipt_retained: true,
    config_state: "before",
    runtime_state: "unknown",
  }));
  assert.throws(() => parseConfigMutationOutcome({
    schema_version: 1,
    operation: "set_codex_network",
    disposition: "completed",
    config_state: "after",
    runtime_state: "stopped",
    recovery_state: "not_needed",
    operation_id: "not-hex",
  }));
});
