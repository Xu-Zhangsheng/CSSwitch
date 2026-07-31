import { call } from "./ipc-client.js";
import { RUNTIME_STATUS_LABELS, aggregateRuntimeStatus, normalizeRuntimeLight } from "./runtime-status-state.js";

export function createRuntimeController({
  els,
  getConfigState,
  getSkillPage,
  isBusy,
  getBusyOp,
  isActivationInFlight,
  getMode,
  getOfficialRuntimeState,
  setBusy,
  setMsg,
  setBrowserFallback,
  startOneClickFeedback,
  startDoctorFeedback,
  isCodexSource,
  renderList,
  runtimeCommandErrorText,
  syncOpenBrowserControl,
  setLight,
  setStatusText,
  setStatusRecoveryMsg,
  proxyRecoveryMessage,
}) {
  let browserOpenInFlight = false;
  let doctorInFlight = false;
  let runtimeChoiceActiveId = null;

function hideRuntimeChoice() {
  els.runtimeChoiceSec.hidden = true;
  els.runtimeChoiceText.textContent = "";
  runtimeChoiceActiveId = null;
}

function showRuntimeChoice(preflight) {
  const cachedVersion = preflight && preflight.cached_version;
  const canUseCache = preflight && preflight.status === "cached_choice_required" && !!cachedVersion;
  els.runtimeUseCacheBtn.hidden = !canUseCache;
  els.runtimeChoiceText.textContent = canUseCache
    ? "未找到通过安全预检的 Claude Science App。发现可确认版本的历史缓存：" + cachedVersion + "。你可以仅本次使用它，或前往官方页面安装 / 更新 Science。此选择不会保存。"
    : "未找到通过安全预检的 Claude Science App，历史缓存也无法确认版本。请先从官方页面安装 / 更新 Science。";
  els.runtimeChoiceSec.hidden = false;
  runtimeChoiceActiveId = getConfigState().active_id || null;
}

function hideHistoryRecovery() {
  els.historyRecoverySec.hidden = true;
  els.historyRecoveryText.textContent = "";
  els.historyRecoveryChoices.replaceChildren();
}

function showHistoryRecovery(result) {
  const choices = Array.isArray(result && result.choices) ? result.choices : [];
  els.historyRecoveryChoices.replaceChildren();
  choices.forEach((choice) => {
    if (!choice || typeof choice.reference !== "string" || !choice.reference) return;
    const button = document.createElement("button");
    button.type = "button";
    button.className = "btn primary";
    button.dataset.historyReference = choice.reference;
    button.textContent = typeof choice.label === "string" && choice.label
      ? choice.label
      : "历史记录";
    els.historyRecoveryChoices.appendChild(button);
  });
  els.historyRecoveryText.textContent =
    "检测到多份 v0.8.0 遗留历史。请选择要恢复的一份；CSSwitch 不会读取对话内容，也不会删除其他记录。若打开后发现选错，可在本次应用运行期间返回这里改选。";
  els.historyRecoverySec.hidden = false;
}

async function restoreHistoryChoice(reference) {
  if (!reference || isBusy()) return;
  setBusy(true, { kind: "historyRecovery" });
  setMsg("正在重新核验并恢复所选历史记录…");
  let restored = false;
  try {
    const result = await call("restore_history_choice", { reference });
    if (result && Array.isArray(result.choices)) showHistoryRecovery(result);
    setMsg(result && result.message || "已恢复所选历史记录。", "ok");
    restored = true;
  } catch (e) {
    setMsg("恢复历史记录失败：" + e, "err");
  } finally {
    setBusy(false);
  }
  if (restored) await runOneClick(null);
}

async function checkOneClickBoundary() {
  if (isActivationInFlight()) {
    setMsg("当前选择仍在保存。请等待完成后再一键开始。", "err");
    return false;
  }
  if (!getConfigState().active_id) {
    setMsg("还没有「当前选择」的配置。请先点「新建配置」或在列表点「设为当前」选一条，再一键开始。", "err");
    return false;
  }
  const active = (getConfigState().profiles || []).find((p) => p.id === getConfigState().active_id);
  if (isCodexSource(active)) {
    if (!getConfigState().experimental_codex_enabled) {
      setMsg("当前是 Codex 配置，但实验入口已关闭。请先在“设置 > Codex 账号与连接”重新启用。", "err");
      return false;
    }
  }
  return true;
}

async function runOneClick(runtimeChoice) {
  if (runtimeChoice) {
    if (!runtimeChoiceActiveId || runtimeChoiceActiveId !== getConfigState().active_id) {
      hideRuntimeChoice();
      setMsg("当前选择已变化，本次缓存运行选择已作废。请重新点击「一键开始」。", "err");
      return;
    }
  }
  if (!(await checkOneClickBoundary())) return;
  getSkillPage()?.invalidate();
  hideRuntimeChoice();
  setBusy(true, { kind: "oneClick" });
  setBrowserFallback("");
  startOneClickFeedback();
  try {
    const r = await call("one_click_login", { runtimeChoice: runtimeChoice || null });
    if (r && r.status === "attention" && r.action === "history_choice_required") {
      showHistoryRecovery(r);
      setMsg(r.msg || "请选择要恢复的历史记录。", "err");
      await refreshStatus();
      return;
    }
    if (r && r.status === "error") {
      const recovery = r.recovery_status === "degraded"
        ? "；刷新未完成，安全事务记录已保留，可修正问题后重试"
        : "";
      setMsg((r.message || "一键开始未完成") + recovery + "（阶段：" + (r.stage || "unknown") + "）", "err");
      setBrowserFallback(r.fallback_url);
      await refreshStatus();
      return;
    }
    // 透传后端据实回传的 msg（已重开 / 已用新配置重启 / 沿用原对话 / 已启动 / 打开失败请手动打开）。
    const active = (getConfigState().profiles || []).find((p) => p.id === getConfigState().active_id);
    const message = r.msg || "已就绪，正在打开面板…";
    if (!els.historyRecoverySec.hidden) {
      els.historyRecoveryText.textContent =
        "已打开所选历史。如果内容不对，可在本次应用运行期间选择另一份；切换前 CSSwitch 会先安全停止隔离 Science。";
    }
    setMsg(message + (isCodexSource(active)
      ? " 请在 Science 的 More models 中选择 Codex / … 后再发第一条消息；默认 Claude 壳会被明确拒绝。"
      : ""), "ok");
    setBrowserFallback(r.fallback_url);
    getConfigState().selection_pending = false;
    getConfigState().applied_profile_id = getConfigState().active_id || null;
    renderList();
    await refreshStatus();
  } catch (e) {
    setMsg("一键开始失败：" + runtimeCommandErrorText(e), "err");
  } finally {
    setBusy(false);
    await getSkillPage()?.refreshIfLoaded();
  }
}

async function importLocalSkill() {
  if (isBusy()) return;
  setBusy(true, { kind: "importSkill" });
  setMsg("正在选择并校验 Skill 包…");
  try {
    const result = await call("install_local_skill_package");
    if (result.status === "CANCELLED") {
      setMsg("已取消导入 Skill 包。");
    } else if (result.status === "INSTALLED_ATTACHED_VERIFY_REQUIRED") {
      setMsg("文件已安装并绑定 OPERON。请在 Science 中让 Agent 调用 skill(" + result.skill_name + ") 验证当前会话加载。", "ok");
    } else if (result.status === "BUNDLE_INSTALLED_ATTACHED") {
      const names = Array.isArray(result.skill_names) ? result.skill_names : [];
      const summary = names.slice(0, 4).join("、") + (names.length > 4 ? " 等" : "");
      setMsg("已安装并绑定 " + names.length + " 个 Skill" + (summary ? "：" + summary : "") + "。", "ok");
    } else {
      setMsg((result.message || "Skill 包导入未完成") + " [" + result.status + "]", "err");
    }
    if (result && result.directory_commit === true && getSkillPage()) {
      await getSkillPage().refresh();
    }
  } catch (e) {
    setMsg("导入 Skill 包失败：" + e, "err");
    await getSkillPage()?.refreshIfLoaded();
  } finally {
    setBusy(false);
  }
}

// ── 一键开始：先确认本次实际 Science runtime，再进入原启动链路。──
async function oneClick() {
  if (!(await checkOneClickBoundary())) return;
  setBusy(true, { kind: "oneClick" });
  setMsg("正在确认本次使用的 Claude Science…");
  try {
    const preflight = await call("science_runtime_preflight");
    if (preflight && preflight.status === "installed_ready") {
      setBusy(false);
      await runOneClick(null);
      return;
    }
    showRuntimeChoice(preflight || { status: "missing" });
    setMsg(preflight && preflight.status === "cached_choice_required"
      ? "Claude Science App 不可用或未通过预检。请选择是否仅本次使用已确认版本的缓存。"
      : "Claude Science App 不可用或未通过预检，且没有可安全启动的缓存版本。", "err");
  } catch (e) {
    setMsg("Science 运行环境检查失败：" + e, "err");
  } finally {
    setBusy(false);
  }
}

async function openScienceDownload() {
  try {
    await call("open_science_download_page");
    setMsg("已打开 Claude 官方下载页。安装完成后请再次点击「一键开始」。");
  } catch (e) {
    setMsg("打开 Claude 官方下载页失败：" + e, "err");
  }
}

function cancelRuntimeChoice() {
  hideRuntimeChoice();
  setMsg("已取消，本次没有启动 Claude Science。");
}

async function stopAll() {
  getSkillPage()?.invalidate();
  setBusy(true);
  setMsg("停止中…");
  try {
    await call("stop_all");
    setMsg("已停止代理与沙箱。", "ok");
    await refreshStatus();
  } catch (e) {
    setMsg("停止失败：" + e, "err");
  } finally {
    setBusy(false);
    await getSkillPage()?.refreshIfLoaded();
  }
}

async function openBrowser() {
  if (isBusy() || browserOpenInFlight) return;
  browserOpenInFlight = true;
  syncOpenBrowserControl();
  setMsg("正在获取新的 Science 地址并交给默认浏览器打开…");
  try {
    const result = await call("open_url");
    if (result && result.status === "error") {
      setBrowserFallback(result.fallback_url);
      setMsg(result.message || "打开浏览器失败；请复制 URL 手动打开。", "err");
    } else {
      setBrowserFallback("");
      setMsg((result && result.message) || "已向默认浏览器发出打开 Science 的请求；若窗口没有切到前台，请从 Dock 或其他桌面切回默认浏览器。", "ok");
    }
  } catch (e) {
    setMsg("打开浏览器失败：" + e, "err");
  } finally {
    browserOpenInFlight = false;
    syncOpenBrowserControl();
  }
}

async function runDoctor() {
  const activationBusy = isActivationInFlight() || (isBusy() && getBusyOp() && getBusyOp().kind === "activate");
  if (doctorInFlight || (isBusy() && !activationBusy)) return;
  doctorInFlight = true;
  if (els.doctorBtn) els.doctorBtn.disabled = true;
  if (activationBusy) {
    setMsg("自检中：配置后台应用仍在继续。完成后会核验 CSSwitch 管理的 Skill 路由。");
  } else {
    setBusy(true, { kind: "doctor" });
    startDoctorFeedback();
  }
  try {
    const out = await call("run_doctor");
    setMsg(out, out.includes("失败 0") ? "ok" : null);
  } catch (e) {
    setMsg("自检失败：" + e, "err");
  } finally {
    doctorInFlight = false;
    if (els.doctorBtn) els.doctorBtn.disabled = isBusy() && getBusyOp() && getBusyOp().kind !== "activate";
    if (!activationBusy) setBusy(false);
  }
}

// 简单 semver 比较：a 是否比 b 新。
function isNewer(a, b) {
  const pa = String(a).split(".").map((n) => parseInt(n, 10) || 0);
  const pb = String(b).split(".").map((n) => parseInt(n, 10) || 0);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const x = pa[i] || 0, y = pb[i] || 0;
    if (x !== y) return x > y;
  }
  return false;
}

async function checkUpdate() {
  setMsg("检查更新中…");
  let cur = "";
  try { cur = await call("app_version"); } catch (e) {}
  try {
    const resp = await fetch(
      "https://api.github.com/repos/SuperJJ007/CSSwitch/releases/latest",
      { headers: { Accept: "application/vnd.github+json" } }
    );
    if (!resp.ok) throw new Error("HTTP " + resp.status);
    const data = await resp.json();
    const latest = (data.tag_name || "").replace(/^v/, "");
    if (!latest) throw new Error("无版本信息");
    if (isNewer(latest, cur)) {
      setMsg("发现新版本 v" + latest + "（当前 v" + cur + "）。正在打开下载页…", "ok");
      try { await call("open_release_page"); } catch (_) {}
    } else {
      setMsg("已是最新版本（v" + cur + "）。", "ok");
    }
  } catch (e) {
    setMsg("无法自动检查更新（多为网络或代理限制）。已打开 Releases 页，请手动查看。", "err");
    try { await call("open_release_page"); } catch (_) {}
  }
}

async function refreshStatus() {
  try {
    const s = await call("status");
    setLight(els.ltProxy, s.proxy);
    setLight(els.ltSandbox, s.sandbox);
    setLight(els.ltUpstream, s.upstream);
    setStatusText("proxyStateText", s.proxy);
    setStatusText("sandboxStateText", s.sandbox);
    setStatusText("upstreamStateText", s.upstream);
    els.brandDot.className = "dot " + aggregateRuntimeStatus(s, {
      mode: getMode(),
      officialState: getOfficialRuntimeState(),
    });
    setStatusRecoveryMsg(proxyRecoveryMessage(s));
  } catch (e) {
    [els.ltProxy, els.ltSandbox, els.ltUpstream].forEach((l) => setLight(l, "unknown"));
    ["proxyStateText", "sandboxStateText", "upstreamStateText"].forEach((id) => setStatusText(id, "unknown"));
    els.brandDot.className = "dot gray";
  }
}

  return {
    isBrowserOpenInFlight: () => browserOpenInFlight,
    isDoctorInFlight: () => doctorInFlight,
    hideRuntimeChoice,
    showRuntimeChoice,
    hideHistoryRecovery,
    showHistoryRecovery,
    restoreHistoryChoice,
    runOneClick,
    importLocalSkill,
    oneClick,
    openScienceDownload,
    cancelRuntimeChoice,
    stopAll,
    openBrowser,
    runDoctor,
    checkUpdate,
    refreshStatus,
  };
}
