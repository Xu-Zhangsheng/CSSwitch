import assert from "node:assert/strict";
import test from "node:test";

let invokeHandler = async () => {
  throw new Error("invoke handler not installed");
};

globalThis.window = {
  location: { search: "" },
  __TAURI__: {
    core: {
      invoke: async (command, args) => await invokeHandler(command, args),
    },
  },
};

const { createRuntimeController } = await import(
  "../desktop/src/runtime-controller.js?r0-history-restore"
);

function makeController(calls) {
  const config = {
    active_id: "profile-a",
    applied_profile_id: "profile-a",
    selection_pending: false,
    experimental_codex_enabled: false,
    profiles: [{ id: "profile-a", template_id: "deepseek" }],
  };
  const els = {
    runtimeChoiceSec: { hidden: true },
    runtimeChoiceText: { textContent: "" },
    runtimeUseCacheBtn: { hidden: true },
    historyRecoverySec: { hidden: true },
    historyRecoveryText: { textContent: "" },
    historyRecoveryChoices: {
      replaceChildren() {},
      appendChild() {},
    },
    ltProxy: {},
    ltSandbox: {},
    ltUpstream: {},
    brandDot: { className: "" },
  };
  return createRuntimeController({
    els,
    getConfigState: () => config,
    getSkillPage: () => null,
    isBusy: () => false,
    getBusyOp: () => null,
    isActivationInFlight: () => false,
    getMode: () => "proxy",
    getOfficialRuntimeState: () => "gray",
    setBusy() {},
    setMsg() {},
    setBrowserFallback() {},
    startOneClickFeedback() {},
    startDoctorFeedback() {},
    isCodexSource: () => false,
    renderList() {},
    runtimeCommandErrorText: (error) => String(error),
    syncOpenBrowserControl() {},
    setLight() {},
    setStatusText() {},
    setStatusRecoveryMsg() {},
    proxyRecoveryMessage: () => "",
  });
}

test("successful history restore invokes one_click_login exactly once after restore succeeds", async () => {
  const calls = [];
  invokeHandler = async (command, args) => {
    calls.push([command, args]);
    if (command === "restore_history_choice") {
      return { status: "ok", action: "history_choice_restored" };
    }
    if (command === "one_click_login") {
      return { status: "ok", action: "started", fallback_url: null };
    }
    if (command === "status") {
      return { proxy: "green", sandbox: "green", upstream: "green" };
    }
    throw new Error(`unexpected command: ${command}`);
  };

  await makeController(calls).restoreHistoryChoice("history-reference-a");

  assert.deepEqual(calls, [
    ["restore_history_choice", { reference: "history-reference-a" }],
    ["one_click_login", { runtimeChoice: null }],
    ["status", undefined],
  ]);
});

test("failed history restore never invokes one_click_login", async () => {
  const calls = [];
  invokeHandler = async (command, args) => {
    calls.push([command, args]);
    if (command === "restore_history_choice") {
      throw new Error("controlled restore failure");
    }
    throw new Error(`unexpected command after restore failure: ${command}`);
  };

  await makeController(calls).restoreHistoryChoice("history-reference-b");

  assert.deepEqual(calls, [
    ["restore_history_choice", { reference: "history-reference-b" }],
  ]);
});
