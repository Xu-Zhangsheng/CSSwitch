import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
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
  "../desktop/src/runtime-controller.js?d0-doctor-intent-split"
);

function makeController(messages, busyStates = []) {
  return createRuntimeController({
    els: {},
    getConfigState: () => ({ active_id: "", profiles: [] }),
    getSkillPage: () => null,
    isBusy: () => false,
    isActivationInFlight: () => false,
    getMode: () => "proxy",
    getOfficialRuntimeState: () => "gray",
    setBusy: (on, operation) => busyStates.push([on, operation]),
    setMsg: (message, kind) => messages.push([message, kind]),
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

test("read-only Doctor intent invokes only run_doctor_read_only and renders typed status", async () => {
  const calls = [];
  const messages = [];
  const busyStates = [];
  invokeHandler = async (command, args) => {
    calls.push([command, args]);
    return {
      schema_version: 1,
      intent: "read_only_diagnostics",
      status: "passed",
      message: "typed pass without legacy report-text sentinel",
    };
  };

  await makeController(messages, busyStates).runDoctorReadOnly();

  assert.deepEqual(calls, [["run_doctor_read_only", undefined]]);
  assert.deepEqual(messages.at(-1), ["typed pass without legacy report-text sentinel", "ok"]);
  assert.deepEqual(busyStates, [
    [true, { kind: "doctorReadOnly" }],
    [false, undefined],
  ]);
});

test("explicit Skill route repair invokes only repair_skill_route and renders typed attention", async () => {
  const calls = [];
  const messages = [];
  invokeHandler = async (command, args) => {
    calls.push([command, args]);
    return {
      schema_version: 1,
      intent: "repair_skill_route",
      status: "warning",
      message: "route marker invalidated; host repair not completed",
    };
  };

  await makeController(messages).repairSkillRoute();

  assert.deepEqual(calls, [["repair_skill_route", undefined]]);
  assert.deepEqual(messages.at(-1), ["route marker invalidated; host repair not completed", "err"]);
});

test("status page names the read-only and mutating intents separately", async () => {
  const html = await readFile(new URL("../desktop/src/index.html", import.meta.url), "utf8");
  assert.match(html, /id="doctorBtn">运行只读自检</);
  assert.match(html, /id="repairSkillRouteBtn">修复 Skill 路由</);
  assert.match(html, /只读自检不会更改 Skill 或 MCP/);
  assert.match(html, /不会启动受管 Science 或正式 Gateway 服务/);
});
