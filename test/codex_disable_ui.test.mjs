import assert from "node:assert/strict";
import test from "node:test";

import {
  formatCodexDisableCommandError,
  parseCodexDisableCommandError,
} from "../desktop/src/codex-disable-protocol.js";

const failed = {
  code: "codex_disable_failed",
  cause: "config_commit_failed",
  phase: "restored",
  retryable: true,
  attention_required: false,
  config_state: "unchanged",
};
const attention = {
  code: "codex_disable_attention",
  cause: "restore_uncertain",
  phase: "restoring",
  retryable: false,
  attention_required: true,
  config_state: "unchanged",
};

test("accepts stable disable failure and attention projections", () => {
  assert.deepEqual(parseCodexDisableCommandError(failed), failed);
  assert.deepEqual(
    parseCodexDisableCommandError({ ...failed, cause: "science_stop_failed", phase: "stopping" }),
    { ...failed, cause: "science_stop_failed", phase: "stopping" },
  );
  assert.deepEqual(parseCodexDisableCommandError(attention), attention);
  assert.match(formatCodexDisableCommandError(failed), /精确恢复/);
  assert.match(formatCodexDisableCommandError(attention), /receipt 已保留/);
  assert.match(
    formatCodexDisableCommandError({ ...attention, config_state: "disabled" }),
    /入口已关闭/,
  );
});

test("rejects leaked, unknown, or contradictory disable errors", () => {
  const invalid = [
    { ...failed, secret: "must-not-project" },
    { ...failed, cause: "future_cause" },
    { ...failed, retryable: false },
    { ...attention, attention_required: false },
    { ...attention, code: "codex_disable_future" },
  ];
  for (const value of invalid) assert.throws(() => parseCodexDisableCommandError(value));
  assert.equal(parseCodexDisableCommandError("ordinary error"), null);
  assert.equal(parseCodexDisableCommandError({ code: "codex_auth_busy", retryable: true }), null);
});
