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

function makeController(calls, options = {}) {
  const config = options.config || {
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
    setMsg: options.setMsg || (() => {}),
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

function projection(disposition, {
  journal = "cleared",
  binding = "matches_active",
  applied = "profile-a",
  pending = false,
  cleanup = false,
} = {}) {
  return {
    disposition,
    journal_disposition: journal,
    binding_relation: binding,
    applied_profile_id: applied,
    selection_pending: pending,
    cleanup_required: cleanup,
  };
}

test("successful history restore invokes one_click_login exactly once after restore succeeds", async () => {
  const calls = [];
  invokeHandler = async (command, args) => {
    calls.push([command, args]);
    if (command === "restore_history_choice") {
      return { status: "ok", action: "history_choice_restored" };
    }
    if (command === "one_click_login") {
      return { status: "ok", action: "started", recovery_status: "not_needed", fallback_url: null };
    }
    if (command === "finalize_consumer_state") {
      return projection("ready");
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
    ["finalize_consumer_state", { outcome: { status: "ok", action: "started", recovery_status: "not_needed", fallback_url: null } }],
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

test("manual one-click publishes applied only from a confirmed normal cleanup readback", async () => {
  const calls = [];
  const messages = [];
  const config = {
    active_id: "profile-a",
    applied_profile_id: "old-profile",
    selection_pending: true,
    experimental_codex_enabled: false,
    profiles: [{ id: "profile-a", template_id: "deepseek" }],
  };
  const dto = {
    status: "degraded",
    action: "started",
    recovery_status: "cleanup_required",
    msg: "runtime started",
    fallback_url: null,
  };
  invokeHandler = async (command, args) => {
    calls.push([command, args]);
    if (command === "one_click_login") return dto;
    if (command === "finalize_consumer_state") {
      assert.deepEqual(args, { outcome: dto });
      return projection("ready", { cleanup: true });
    }
    if (command === "status") return { proxy: "green", sandbox: "green", upstream: "green" };
    throw new Error(`unexpected command: ${command}`);
  };

  await makeController(calls, {
    config,
    setMsg: (text, kind) => messages.push([text, kind]),
  }).runOneClick(null);

  assert.equal(config.applied_profile_id, "profile-a");
  assert.equal(config.selection_pending, false);
  assert.match(messages.at(-1)[0], /待安全清理/);
});

test("history cleanup-required remains attention and never publishes the active profile", async () => {
  const calls = [];
  const config = {
    active_id: "profile-a",
    applied_profile_id: "old-profile",
    selection_pending: true,
    experimental_codex_enabled: false,
    profiles: [{ id: "profile-a", template_id: "deepseek" }],
  };
  const dto = {
    status: "degraded",
    action: "history_choice_required",
    recovery_status: "cleanup_required",
    choices: [],
    msg: "choose history",
  };
  invokeHandler = async (command) => {
    calls.push(command);
    if (command === "one_click_login") return dto;
    if (command === "finalize_consumer_state") {
      return projection("attention", {
        binding: "matches_active",
        applied: null,
        pending: true,
        cleanup: true,
      });
    }
    if (command === "status") return { proxy: "gray", sandbox: "gray", upstream: "gray" };
    throw new Error(`unexpected command: ${command}`);
  };

  const controller = makeController([], { config });
  await controller.runOneClick(null);

  assert.equal(config.applied_profile_id, null);
  assert.equal(config.selection_pending, true);
});

test("open finalize journal and readback failure both publish unknown manual state", async () => {
  for (const readbackFails of [false, true]) {
    const calls = [];
    const messages = [];
    const config = {
      active_id: "profile-a",
      applied_profile_id: "old-profile",
      selection_pending: true,
      experimental_codex_enabled: false,
      profiles: [{ id: "profile-a", template_id: "deepseek" }],
    };
    const dto = {
      status: "degraded",
      action: "started",
      recovery_status: "manual_recovery_required",
      msg: "finalize pending",
    };
    invokeHandler = async (command) => {
      calls.push(command);
      if (command === "one_click_login") return dto;
      if (command === "finalize_consumer_state") {
        if (readbackFails) throw new Error("controlled readback failure");
        return projection("manual", {
          journal: "open",
          binding: "matches_active",
          applied: null,
          pending: true,
        });
      }
      if (command === "status") return { proxy: "gray", sandbox: "gray", upstream: "gray" };
      throw new Error(`unexpected command: ${command}`);
    };

    await makeController(calls, {
      config,
      setMsg: (text, kind) => messages.push([text, kind]),
    }).runOneClick(null);

    assert.equal(config.applied_profile_id, null);
    assert.equal(config.selection_pending, true);
    assert.equal(messages.at(-1)[1], "err");
    assert.match(messages.at(-1)[0], readbackFails ? /不会把当前选择误报为已应用/ : /待下次显式一键操作重放/);
  }
});

test("auto-boot non-ready publication clears stale applied presentation", () => {
  const config = {
    active_id: "profile-a",
    applied_profile_id: "profile-a",
    selection_pending: false,
    experimental_codex_enabled: false,
    profiles: [{ id: "profile-a", template_id: "deepseek" }],
  };
  const controller = makeController([], { config });

  controller.publishFinalizeUnknown();

  assert.equal(config.applied_profile_id, null);
  assert.equal(config.selection_pending, true);
});
