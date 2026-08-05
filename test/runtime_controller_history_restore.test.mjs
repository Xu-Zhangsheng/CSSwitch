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
globalThis.document = {
  createElement: () => ({ dataset: {} }),
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
      replaceChildren() {
        if (options.historyButtons) options.historyButtons.length = 0;
      },
      appendChild(button) {
        if (options.historyButtons) options.historyButtons.push(button);
      },
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

test("successful history restore stays stopped until an explicit one-click", async () => {
  const calls = [];
  const messages = [];
  const result = {
    status: "ok",
    recovery_status: "not_needed",
    action: "history_choice_restored",
    message: "已恢复所选历史记录；其他历史记录未被删除。",
    choices: [{ reference: "rotated-history-reference-a", label: "历史记录 A" }],
  };
  invokeHandler = async (command, args) => {
    calls.push([command, args]);
    if (command === "restore_history_choice") return result;
    if (command === "finalize_consumer_state") {
      assert.deepEqual(args, { outcome: result });
      return projection("attention", { applied: null, pending: true });
    }
    throw new Error(`unexpected command: ${command}`);
  };

  await makeController(calls, {
    setMsg: (text, kind) => messages.push([text, kind]),
  }).restoreHistoryChoice("history-reference-a");

  assert.deepEqual(calls, [
    ["restore_history_choice", { reference: "history-reference-a", resume: false }],
    ["finalize_consumer_state", { outcome: result }],
  ]);
  assert.deepEqual(messages.at(-1), [
    "已恢复所选历史记录；其他历史记录未被删除。 当前保持停止；请再次点击「一键开始」。",
    "ok",
  ]);
});

test("restore-only degraded finalize refreshes choices and never reports success", async () => {
  const calls = [];
  const messages = [];
  const historyButtons = [];
  const result = {
    status: "degraded",
    recovery_status: "manual_recovery_required",
    action: "history_choice_restored",
    message: "历史记录已恢复，但 durable finalize 尚未完成",
    choices: [{ reference: "rotated-top-level", label: "历史记录 A" }],
    history_recovery: {
      status: "restored",
      choices: [{ reference: "rotated-nested", label: "历史记录 A" }],
    },
  };
  invokeHandler = async (command, args) => {
    calls.push([command, args]);
    if (command === "restore_history_choice") return result;
    if (command === "finalize_consumer_state") {
      assert.deepEqual(args, { outcome: result });
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
    setMsg: (text, kind) => messages.push([text, kind]),
    historyButtons,
  }).restoreHistoryChoice("history-reference-a");

  assert.deepEqual(historyButtons.map((button) => button.dataset.historyReference), [
    "rotated-nested",
    "rotated-nested",
  ]);
  assert.equal(messages.some(([, kind]) => kind === "ok"), false);
  assert.equal(messages.at(-1)[1], "err");
  assert.equal(calls.some(([command]) => command === "one_click_login"), false);
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
    ["restore_history_choice", { reference: "history-reference-b", resume: false }],
  ]);
});

test("explicit restore-and-resume remains one backend operation", async () => {
  const calls = [];
  const messages = [];
  const historyButtons = [];
  const result = {
    status: "ok",
    action: "started",
    msg: "已恢复并启动",
    fallback_url: null,
    history_recovery: {
      status: "restored",
      choices: [{ reference: "rotated-history-reference-a", label: "历史记录 A" }],
    },
  };
  invokeHandler = async (command, args) => {
    calls.push([command, args]);
    if (command === "restore_history_choice") return result;
    if (command === "finalize_consumer_state") {
      assert.deepEqual(args, { outcome: result });
      return projection("ready");
    }
    if (command === "status") {
      return { proxy: "green", sandbox: "green", upstream: "green" };
    }
    throw new Error(`unexpected command: ${command}`);
  };

  await makeController(calls, {
    setMsg: (text, kind) => messages.push([text, kind]),
    historyButtons,
  }).restoreHistoryChoice("history-reference-a", true);

  assert.deepEqual(calls, [
    ["restore_history_choice", { reference: "history-reference-a", resume: true }],
    ["finalize_consumer_state", { outcome: result }],
    ["status", undefined],
  ]);
  assert.deepEqual(messages.at(-1), ["已恢复并启动", "ok"]);
  assert.deepEqual(
    historyButtons.map((button) => [button.dataset.historyReference, button.dataset.historyResume]),
    [
      ["rotated-history-reference-a", "false"],
      ["rotated-history-reference-a", "true"],
    ],
  );

  const failedCalls = [];
  const failedButtons = [];
  const failedResult = {
    status: "error",
    recovery_status: "manual_recovery_required",
    message: "历史已恢复，但继续启动失败",
    history_recovery: {
      status: "restored",
      choices: [{ reference: "rotated-after-failure", label: "历史记录 B" }],
    },
  };
  invokeHandler = async (command, args) => {
    failedCalls.push([command, args]);
    if (command === "restore_history_choice") return failedResult;
    if (command === "finalize_consumer_state") return projection("manual_recovery_required", {
      journal: "open",
      binding: "unknown",
      applied: null,
      pending: true,
    });
    if (command === "status") return { proxy: "gray", sandbox: "gray", upstream: "gray" };
    throw new Error(`unexpected command: ${command}`);
  };
  await makeController(failedCalls, { historyButtons: failedButtons })
    .restoreHistoryChoice("old-reference-b", true);
  assert.deepEqual(failedButtons.map((button) => button.dataset.historyReference), [
    "rotated-after-failure",
    "rotated-after-failure",
  ]);
  assert.equal(failedCalls.some(([command]) => command === "one_click_login"), false);
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
