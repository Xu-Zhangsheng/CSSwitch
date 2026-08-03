import json
import os
import pathlib
import re
import stat
import subprocess
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]


def sandbox_session_source():
    module_dir = ROOT / "desktop/src-tauri/src/runtime/sandbox_session"
    sources = sorted(module_dir.rglob("*.rs"))
    return "\n".join(path.read_text() for path in sources)


def sandbox_session_one_click_source():
    module_dir = ROOT / "desktop/src-tauri/src/runtime/sandbox_session"
    sources = [module_dir / "one_click.rs"]
    sources.extend(sorted((module_dir / "one_click").rglob("*.rs")))
    return "\n".join(path.read_text() for path in sources)


def runtime_command_source():
    root = ROOT / "desktop/src-tauri/src/commands/runtime.rs"
    module_dir = ROOT / "desktop/src-tauri/src/commands/runtime"
    sources = [root]
    sources.extend(sorted(module_dir.rglob("*.rs")))
    return "\n".join(path.read_text() for path in sources)


def runtime_command_module(name):
    return (
        ROOT / "desktop/src-tauri/src/commands/runtime" / f"{name}.rs"
    ).read_text()


def science_runtime_source():
    root = ROOT / "desktop/src-tauri/src/runtime/science.rs"
    module_dir = ROOT / "desktop/src-tauri/src/runtime/science"
    sources = [root]
    sources.extend(sorted(module_dir.rglob("*.rs")))
    return "\n".join(path.read_text() for path in sources)


def proxy_lifecycle_source():
    root = ROOT / "desktop/src-tauri/src/runtime/proxy_lifecycle.rs"
    module_dir = ROOT / "desktop/src-tauri/src/runtime/proxy_lifecycle"
    sources = [root]
    sources.extend(sorted(module_dir.rglob("*.rs")))
    return "\n".join(path.read_text() for path in sources)


class SkillRuntimeBoundary(unittest.TestCase):
    def test_production_startup_has_no_skill_manager_dependency(self):
        session = sandbox_session_source()
        for forbidden in (
            "skill_manager",
            "commands::skills",
            ".claude/skills",
            "scan_and_reconcile",
            "CSSWITCH_RECONCILED_DATA_DIR",
            "STORE_CONFLICT",
            "LIMIT_EXCEEDED",
        ):
            self.assertNotIn(forbidden, session)

        lib = (ROOT / "desktop/src-tauri/src/lib.rs").read_text()
        command_block = lib.split("tauri::generate_handler![", 1)[1].split("])", 1)[0]
        self.assertNotIn("commands::skills", command_block)
        self.assertNotIn("mod skill_manager;", lib)

        commands = (ROOT / "desktop/src-tauri/src/commands/mod.rs").read_text()
        self.assertNotIn("mod skills;", commands)

        catalog = json.loads((ROOT / "catalog/capabilities.v1.json").read_text())
        self.assertEqual(catalog["skills"], [])

    def test_gateway_starts_only_after_config_and_science_state_prechecks(self):
        one_click = sandbox_session_one_click_source().split(
            "fn one_click_login_with_options", 1
        )[1]
        state_check = one_click.index("let (science_state, running_runtime)")
        self.assertLess(one_click.index("config::load_from(&dir)"), state_check)
        self.assertNotIn("ensure_proxy(", one_click[:state_check])

        runtime_selection = one_click.index("let launch_runtime: ScienceRuntimeIdentity")
        self.assertGreater(runtime_selection, state_check)
        launch_check = one_click.index("if !launch.is_file()")
        normal_proxy = one_click.index("let (pport, secret, proxy_action) =", state_check)
        self.assertGreater(normal_proxy, launch_check)

    def test_launcher_never_clones_or_implicitly_selects_data_dir_runtime(self):
        launch = (ROOT / "scripts/launch-virtual-sandbox.sh").read_text()
        selection = launch.split('BIN_SOURCE="backend-selected runtime"', 1)[1].split(
            "# Use a keychain scoped", 1
        )[0]
        self.assertIn('BIN="$APP_BIN"', selection)
        self.assertNotIn('BIN="$DATA_DIR/bin/claude-science"', launch)
        self.assertNotIn("for asset in bin conda runtime seed-assets", launch)
        self.assertNotIn("cp -Rc", launch)
        self.assertIn("CSSWITCH_PROXY_URL", launch)
        self.assertIn("--proxy-url", launch)
        self.assertIn("path_contains_symlink", launch)

        stop = (ROOT / "scripts/stop-science-sandbox.sh").read_text()
        self.assertNotIn('BIN="$DATA_DIR/bin/claude-science"', stop)
        self.assertIn("path_contains_symlink", stop)

    def test_fresh_data_dir_initializes_without_reading_real_science_home(self):
        with tempfile.TemporaryDirectory(
            prefix="csswitch-runtime-init-", dir="/private/tmp"
        ) as raw_tmp:
            tmp = pathlib.Path(raw_tmp)
            outer_home = tmp / "outer-home"
            real_science = outer_home / ".claude-science"
            real_science.mkdir(parents=True)
            (real_science / "must-not-copy").write_text("private")

            sandbox_home = tmp / "sandbox-home"
            bin_dir = tmp / "bin"
            bin_dir.mkdir()
            security = bin_dir / "security"
            security.write_text("#!/bin/sh\nexit 0\n")
            security.chmod(0o700)
            marker = tmp / "science-invocation.txt"
            science = tmp / "fake-claude-science"
            science.write_text(
                "#!/bin/sh\n"
                "mkdir -p \"$HOME/.claude-science\"\n"
                f"printf 'HOME=%s\\nARGS=%s\\n' \"$HOME\" \"$*\" > \"{marker}\"\n"
                "exit 0\n"
            )
            science.chmod(0o700)
            env = os.environ.copy()
            env.update(
                {
                    "HOME": str(outer_home),
                    "SANDBOX_HOME": str(sandbox_home),
                    "SCIENCE_BIN": str(science),
                    "PATH": f"{bin_dir}:/usr/bin:/bin:/usr/sbin:/sbin",
                }
            )

            real_science.chmod(0)
            try:
                result = subprocess.run(
                    [
                        str(ROOT / "scripts/launch-virtual-sandbox.sh"),
                        "--port",
                        "19942",
                        "--proxy-url",
                        "http://127.0.0.1:19941/test-secret",
                        "--skip-oauth-forge",
                    ],
                    env=env,
                    capture_output=True,
                    text=True,
                    timeout=15,
                    check=False,
                )
            finally:
                real_science.chmod(stat.S_IRWXU)

            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn(f"HOME={sandbox_home}", marker.read_text())
            data_dir = sandbox_home / ".claude-science"
            self.assertTrue(data_dir.is_dir())
            self.assertFalse((data_dir / "must-not-copy").exists())
            self.assertFalse((data_dir / "bin").exists())

    def test_ui_cache_authorization_is_explicit_and_not_persisted(self):
        html = (ROOT / "desktop/src/index.html").read_text()
        main = (ROOT / "desktop/src/main.js").read_text()
        js = (ROOT / "desktop/src/runtime-controller.js").read_text()
        runtime = runtime_command_source()
        science = science_runtime_source()

        for element_id in (
            "runtimeChoiceSec",
            "runtimeChoiceText",
            "runtimeUseCacheBtn",
            "runtimeDownloadBtn",
            "runtimeChoiceCancelBtn",
        ):
            self.assertIn(f'id="{element_id}"', html)
        one_click = js.split("async function oneClick()", 1)[1].split(
            "async function openScienceDownload", 1
        )[0]
        self.assertLess(
            one_click.index('call("science_runtime_preflight")'),
            one_click.index("runOneClick(null)"),
        )
        self.assertIn('runOneClick("cached_once")', main)
        self.assertIn("此选择不会保存", js)
        self.assertNotIn("localStorage", one_click)
        self.assertIn('THEME_STORAGE_KEY = "csswitch-theme"', main)
        self.assertIn("runtime_choice: Option<String>", runtime)
        self.assertIn("choice == Some(CACHED_ONCE_CHOICE)", science)
        self.assertIn("fn safe_science_version(path: &Path)", science)
        self.assertIn("official_updated_science_bin()", science)
        self.assertIn("ScienceRuntimeSource::OfficialUpdated", science)
        self.assertIn("OFFICIAL_UPDATED_SCIENCE_IDENTIFIER", science)
        self.assertIn("OFFICIAL_SCIENCE_TEAM_ID", science)
        self.assertIn("sha256: [u8; 32]", science)
        self.assertIn("runtime_identity_is_current", science)
        self.assertIn('"cached_choice_required"', science)

    def test_manual_science_open_refreshes_url_and_has_visible_feedback(self):
        main = (ROOT / "desktop/src/main.js").read_text()
        js = (ROOT / "desktop/src/runtime-controller.js").read_text()
        runtime = runtime_command_source()
        actions = runtime_command_module("actions")
        runtime_tests = runtime_command_module("tests")
        system = (ROOT / "desktop/src-tauri/src/runtime/system.rs").read_text()

        handler = js.split("async function openBrowser()", 1)[1].split(
            "async function runDoctor", 1
        )[0]
        self.assertIn("if (isBusy() || browserOpenInFlight) return", handler)
        self.assertIn("browserOpenInFlight = true", handler)
        self.assertIn("browserOpenInFlight = false", handler)
        self.assertIn("syncOpenBrowserControl()", handler)
        self.assertIn("正在获取新的 Science 地址", handler)
        self.assertIn("已向默认浏览器发出打开 Science 的请求", handler)
        self.assertIn('result.status === "error"', handler)
        self.assertIn("setBrowserFallback(result.fallback_url)", handler)
        self.assertIn('setMsg("打开浏览器失败："', handler)

        control = main.split("function syncOpenBrowserControl()", 1)[1].split(
            "function syncActivationControls", 1
        )[0]
        self.assertIn("busy || runtimeController.isBrowserOpenInFlight()", control)
        self.assertIn('runtimeController.isBrowserOpenInFlight() ? "打开中…" : "浏览器打开"', control)

        command = actions.split("fn open_url_inner", 1)[1].split(
            "pub(super) async fn open_url_command", 1
        )[0]
        self.assertIn("sandbox_listener_matches_runtime", command)
        self.assertIn("sandbox_url(sandbox_port, &runtime)", command)
        self.assertNotIn("st.sandbox_url.clone()", command)
        self.assertIn("manual_open_result(url.clone(), open_in_browser(&url))", command)
        self.assertIn("CSSWITCH_FAKE_OPEN_FAIL_ONCE_FILE", runtime_tests)
        self.assertIn('failed_open["fallback_url"]', runtime_tests)
        self.assertIn('ACCEPTANCE_OPEN_BIN_ENV: &str = "CSSWITCH_ACCEPTANCE_OPEN_BIN"', system)
        self.assertIn('TEST_OPEN_BIN_ENV: &str = "CSSWITCH_TEST_OPEN_BIN"', system)
        self.assertIn('return Ok(PathBuf::from("/usr/bin/open"))', system)
        self.assertIn("if !path.is_absolute()", system)
        matrix = (ROOT / "test/installed_provider_matrix.py").read_text()
        self.assertIn('"CSSWITCH_ACCEPTANCE_OPEN_BIN": str(self.bin_dir / "open")', matrix)

    def test_simple_model_inputs_and_one_click_failures_are_visible_and_structured(self):
        js = (ROOT / "desktop/src/profile-controller.js").read_text()
        runtime_js = (ROOT / "desktop/src/runtime-controller.js").read_text()
        session = sandbox_session_source()
        runtime = runtime_command_source()
        command_one_click = runtime_command_module("one_click")
        lifecycle = proxy_lifecycle_source()
        lib = (ROOT / "desktop/src-tauri/src/lib.rs").read_text()
        one_click_source = (
            ROOT / "desktop/src-tauri/src/runtime/sandbox_session/one_click.rs"
        ).read_text()
        one_click_runtime = one_click_source.split(
            "fn one_click_login_with_options", 1
        )[1]

        submission = js.split("function catalogSubmission(kind)", 1)[1].split(
            "function catalogRolesChanged", 1
        )[0]
        for field in (
            "default_model: editor.model.value",
            "quality_model: editor.quality.value",
            "fast_model: editor.fast.value",
            "fable_model: editor.fable.value",
        ):
            self.assertIn(field, submission)
        self.assertNotIn("async function applyPresetSync", js)
        self.assertNotIn("function applyFetchResult", js)
        self.assertIn("function catalogRolesChanged(kind)", js)
        self.assertNotIn("codex.runtimeCommandErrorText", js)
        self.assertIn("codexController.runtimeCommandErrorText", js)
        bootstrap_js = (ROOT / "desktop/src/main.js").read_text()
        self.assertIn('profileController.catalogRolesChanged("wizard")', bootstrap_js)
        self.assertIn('profileController.catalogRolesChanged("connection")', bootstrap_js)

        one_click = runtime_js.split("async function runOneClick", 1)[1].split(
            "async function importLocalSkill", 1
        )[0]
        self.assertLess(
            one_click.index('r && r.status === "error"'),
            one_click.index('const message = r.msg ||'),
        )
        self.assertIn("setBusy(false)", one_click)
        self.assertIn("setBrowserFallback(r.fallback_url)", one_click)

        gateway_ready = one_click_runtime.index("verify_gateway_model_catalog_traced(")
        science_spawn = one_click_runtime.index('Command::new("zsh")', gateway_ready)
        self.assertLess(gateway_ready, science_spawn)
        self.assertEqual(one_click_runtime.count("write_one_click_checkpoint("), 8)
        checkpoint_stages = re.findall(
            r"(?s)write_one_click_checkpoint\(\s*&dir,\s*&transaction_identity,\s*"
            r"&mut journal_progress,\s*config::RuntimeTransactionPhase::(\w+),\s*\)",
            one_click_runtime,
        )
        self.assertEqual(
            checkpoint_stages,
            [
                "StopOldScience",
                "StartGateway",
                "AuthoritySnapshotActive",
                "StartScienceEnvironmentPending",
                "WaitScienceDbReverify",
                "RestartScienceAfterDbHeal",
                "VerifyScienceDbAfterRestart",
                "VerifyScienceCatalog",
            ],
        )
        self.assertRegex(
            one_click_runtime,
            r"let candidate_fingerprint = launch_runtime\.environment_transaction_id\(\);",
        )
        self.assertEqual(one_click_runtime.count("environment_transaction_id()"), 1)
        self.assertRegex(
            one_click_runtime,
            r"(?s)let snapshot_ticket = match authority_snapshot\.registered_snapshot_ticket\(\)"
            r".*?let transaction_identity = OneClickTransactionIdentity \{"
            r".*?runtime_fingerprint: candidate_fingerprint,"
            r".*?snapshot_ticket: snapshot_ticket\.clone\(\),"
            r".*?let mut journal_progress = OneClickJournalProgress::PreJournalAbort \{"
            r".*?registered_ticket: snapshot_ticket,",
        )
        self.assertNotIn("RuntimeTransactionJournal {", one_click_runtime)
        self.assertNotIn("SCIENCE_ENVIRONMENT_PENDING_STAGE_PREFIX", one_click_source)
        self.assertNotIn("AUTHORITY_SNAPSHOT_ACTIVE_STAGE_PREFIX", one_click_source)
        self.assertEqual(one_click_runtime.count("runtime_transaction = Some("), 0)

        one_click_command = command_one_click.split(
            "pub(crate) fn one_click_login_cmd", 1
        )[1].split("pub(super) async fn restore_history_choice_command", 1)[0]
        self.assertRegex(
            one_click_command,
            r"(?s)recover_interrupted_gateway\(&app, &state\)\s*"
            r"\.map_err\(typed_interrupted_gateway_recovery_error\)\?;",
        )
        recovery_projection = command_one_click.split(
            "fn typed_interrupted_gateway_recovery_error", 1
        )[1].split("impl OneClickGatewayPreflightSnapshot", 1)[0]
        self.assertIn("error.kind()", recovery_projection)
        self.assertIn("error.recovery()", recovery_projection)
        self.assertRegex(
            recovery_projection,
            r"(?s)InterruptedGatewayRecoveryErrorKind::AuthoritySnapshot\s*=>\s*\{\s*"
            r"OneClickFailureKind::AuthoritySnapshot\s*\}",
        )
        self.assertRegex(
            recovery_projection,
            r"(?s)InterruptedGatewayRecoveryErrorKind::GatewayStart\s*\|\s*"
            r"InterruptedGatewayRecoveryErrorKind::NotManaged\s*\|\s*"
            r"InterruptedGatewayRecoveryErrorKind::StopUnknown\(_\)\s*=>\s*"
            r"OneClickFailureKind::GatewayStart",
        )
        self.assertIn(
            "InterruptedGatewayRecoveryDisposition::Degraded => ProjectedRecovery::DEGRADED",
            recovery_projection,
        )
        self.assertIn(
            "InterruptedGatewayRecoveryDisposition::ManualRecoveryRequired",
            recovery_projection,
        )
        self.assertIn("ProjectedRecovery::MANUAL_RECOVERY_REQUIRED", recovery_projection)
        self.assertIn(".with_recovery(recovery)", recovery_projection)
        self.assertNotIn(".contains(", recovery_projection)
        self.assertIn(
            "recovery: InterruptedGatewayRecoveryDisposition", lifecycle
        )
        self.assertRegex(
            lifecycle,
            r"(?s)InterruptedGatewayRecoveryErrorKind::AuthoritySnapshot\s*=>\s*\{\s*"
            r"InterruptedGatewayRecoveryDisposition::ManualRecoveryRequired\s*\}",
        )
        self.assertRegex(
            lifecycle,
            r"(?s)InterruptedGatewayRecoveryErrorKind::GatewayStart\s*\|\s*"
            r"InterruptedGatewayRecoveryErrorKind::NotManaged\s*\|\s*"
            r"InterruptedGatewayRecoveryErrorKind::StopUnknown\(_\)\s*=>\s*\{\s*"
            r"InterruptedGatewayRecoveryDisposition::Degraded\s*\}",
        )
        self.assertNotIn("recovery_from_diagnostic_codes", one_click_runtime)
        self.assertIn("stop_managed_gateway_on_port", lifecycle)
        self.assertIn('health.intent == "formal"', lifecycle)
        self.assertIn("journal.previous_gateway()", lifecycle)
        self.assertIn("current == initial_for_probe", lifecycle)
        boot = lib.split("LaunchPath::BootScience", 1)[1].split("// ---------- 入口", 1)[0]
        self.assertIn("boot_result_error(&value)", boot)
        self.assertIn("boot_result_needs_attention(&value)", boot)
        self.assertIn("mark_boot_attention(&app, value)", boot)
        self.assertIn("mark_boot_failed(&app, failure)", boot)

    def test_science_runtime_identity_is_reused_for_serve_status_url_and_stop(self):
        session = sandbox_session_source()
        science = science_runtime_source()
        science_contracts = (
            ROOT / "desktop/src-tauri/src/runtime/science/contracts.rs"
        ).read_text()
        science_lifecycle = (
            ROOT / "desktop/src-tauri/src/runtime/science/lifecycle.rs"
        ).read_text()
        launch_env = (ROOT / "desktop/src-tauri/src/runtime/launch_env.rs").read_text()
        runtime = runtime_command_source()
        one_click = sandbox_session_one_click_source().split(
            "fn one_click_login_with_options", 1
        )[1]
        self.assertIn("science_bin: Path::new(&launch_runtime.path)", session)
        self.assertIn("proxy_url: &proxy_url", session)
        self.assertIn('"SCIENCE_BIN".into(), cfg.science_bin.display().to_string()', launch_env)
        self.assertIn('"CSSWITCH_PROXY_URL".into(), cfg.proxy_url.into()', launch_env)
        self.assertNotIn('.arg(&proxy_url)', session)
        self.assertIn("current.science_runtime = Some(launch_runtime.clone())", one_click)
        self.assertIn("probe_known_runtime(sport, &runtime)", session)
        self.assertIn("sandbox_listener_matches_runtime(sport, &launch_runtime)", session)
        self.assertIn("sandbox_url(sport, &launch_runtime)", session)
        self.assertIn("runtime_identity_is_current(&launch_runtime)", session)
        self.assertIn("configure_science_stop_script_command(", science)
        self.assertIn("Path::new(&runtime.path)", science)
        self.assertIn('"source": runtime.source.code()', runtime)
        for contract in (
            "ScienceStopRequest",
            "ScienceStopOwnershipReceipt",
            "ScienceStopOutcome",
            "VerifiedScienceStop",
            "ScienceStopFailureKind",
        ):
            self.assertIn(contract, science_contracts)
        for outcome in (
            "IdentityDrift",
            "SignalFailure",
            "ExitUnconfirmed",
            "ReceiptCleanupFailure",
        ):
            self.assertIn(outcome, science_contracts)
        self.assertIn("ScienceStopRequest::exact", session)
        self.assertIn("ScienceStopRequest::recover", session)
        self.assertIn("ScienceStopOwnershipReceipt::from_managed_launch", session)
        self.assertIn("require_exact_stop_of", science_contracts)
        self.assertGreaterEqual(session.count("require_exact_stop_of"), 5)
        self.assertGreaterEqual(runtime.count("require_exact_stop_of"), 1)
        self.assertIn("verified.confirmed_runtime().cloned()", session)
        force_restart = session.split(
            "pub(crate) fn force_restart_science_for_active", 1
        )[1].split("fn typed_one_click_err", 1)[0]
        history_restore = runtime_command_module("one_click").split(
            "pub(super) async fn restore_history_choice_command", 1
        )[1]
        for recovery_caller in (force_restart, history_restore):
            self.assertIn("ScienceStopRequest::exact", recovery_caller)
            self.assertIn("require_exact_stop_of", recovery_caller)
            self.assertIn("confirmed_runtime().cloned()", recovery_caller)
            self.assertNotIn("stop_sandbox_state", recovery_caller)
        self.assertIn("HistoryRecoveryScienceQuiescence", session)
        self.assertIn("HistoryRecoveryScienceQuiescence", history_restore)
        self.assertIn("ExactStopped(expected)", history_restore)
        self.assertIn("NoManagedRuntimeObserved", history_restore)
        self.assertIn("science_quiescence.clone()", history_restore)
        self.assertIn("probe_sandbox_runtime_cached", history_restore)
        self.assertIn("current_science_state != SandboxScienceState::Stopped", history_restore)
        self.assertIn(
            "session.science_quiescence =\n                        crate::HistoryRecoveryScienceQuiescence::ExactStopped(runtime)",
            history_restore,
        )
        self.assertIn("remembered_runtime_was_present", session)
        self.assertRegex(
            session,
            r"(?s)None if running_runtime_to_stop\.is_none\(\)\s*"
            r"&& !remembered_runtime_was_present\s*"
            r"&& science_state == SandboxScienceState::Stopped",
        )
        stopped_branch = force_restart.split("SandboxScienceState::Stopped =>", 1)[1].split(
            "SandboxScienceState::Unknown", 1
        )[0]
        self.assertIn("return Err", stopped_branch)
        self.assertNotIn("science_confirmed_stopped", stopped_branch)
        no_runtime_branches = force_restart.split(
            "None if confirmed_stopped.is_some()", 1
        )[1]
        self.assertIn("=> {}", no_runtime_branches)
        self.assertIn("None if proc::loopback_port_in_use", no_runtime_branches)
        no_receipt_branch = no_runtime_branches.rsplit("None =>", 1)[1]
        self.assertIn("return Err", no_receipt_branch)
        self.assertIn("没有 verified-stopped receipt", no_receipt_branch)
        self.assertIn(
            "app_state.science_confirmed_stopped = verified_stopped_runtime.clone()",
            session,
        )
        codex = (ROOT / "desktop/src-tauri/src/commands/codex.rs").read_text()
        self.assertIn("st.science_confirmed_stopped = confirmed_runtime", codex)
        for typed_publisher in (
            runtime_command_module("lifecycle"),
            runtime_command_module("one_click"),
            sandbox_session_one_click_source(),
            codex,
        ):
            self.assertNotIn("science_confirmed_stopped = Some(", typed_publisher)
        self.assertNotIn("stop_sandbox_with_launch_token", science)
        stop_contract = science_lifecycle.split(
            "pub(crate) fn stop_sandbox", 1
        )[1]
        self.assertIn(") -> ScienceStopOutcome", stop_contract)
        self.assertNotIn("Result<(), String>", stop_contract)
        lifecycle_command = runtime_command_module("lifecycle")
        stop_all = lifecycle_command.split(
            "pub(super) fn stop_all_inner_cmd", 1
        )[1].split("pub(super) async fn quit_app_command", 1)[0]
        self.assertIn("ScienceProcessLocalOwner", stop_all)
        self.assertIn("claim_science_stop_request", stop_all)
        self.assertIn("execute_science_stop", stop_all)
        self.assertIn("owner.still_owns", stop_all)
        self.assertIn("lifecycle.current_generation()", stop_all)
        self.assertNotIn("stop_sandbox_state(&app, &mut st)", stop_all)
        stop_all_flow = lifecycle_command.split(
            "pub(super) fn stop_all_inner_with", 1
        )[1].split("pub(super) async fn quit_app_command", 1)[0]
        self.assertLess(
            stop_all_flow.index("let st = lock(&state)"),
            stop_all_flow.index("execute_science(&app, request)"),
        )
        self.assertLess(
            stop_all_flow.index("execute_science(&app, request)"),
            stop_all_flow.rindex("lock(&state)"),
        )
        self.assertIn("pub(crate) fn claim_science_stop_request", science_lifecycle)
        self.assertIn("pub(crate) fn execute_science_stop", science_lifecycle)
        self.assertIn("ScienceStopRequest::exact", science_lifecycle)

    def test_system_ssh_bridge_is_opt_in_and_replaces_tunnel_entry(self):
        js = (ROOT / "desktop/src/profile-controller.js").read_text()
        html = (ROOT / "desktop/src/index.html").read_text()
        launch = (ROOT / "scripts/launch-virtual-sandbox.sh").read_text()
        wrapper = (ROOT / "scripts/ssh-bridge/ssh").read_text()
        session = sandbox_session_source()
        runtime = runtime_command_source()

        self.assertNotIn("ssh_tunnel_info", js + runtime)
        self.assertNotIn("生成 SSH 访问命令", html)
        self.assertIn("reuseSystemSsh", js + html)
        self.assertIn("reuse_system_ssh", js + runtime)
        self.assertIn('CSSWITCH_REUSE_SYSTEM_SSH', launch + session)
        self.assertIn('CSSWITCH_SYSTEM_SSH_CONFIG', launch + wrapper)
        self.assertIn('exec /usr/bin/ssh -F "$config" "$@"', wrapper)
        self.assertNotIn("ln -s", launch)
        self.assertNotIn("cp -R", launch)

    def test_explicit_exit_revokes_the_managed_science_target(self):
        lib = (ROOT / "desktop/src-tauri/src/lib.rs").read_text()
        lifecycle = runtime_command_module("lifecycle")
        js = (ROOT / "desktop/src/main.js").read_text()

        cleanup_flow = lib.split("fn cleanup_for_exit_with", 1)[1].split(
            "fn cleanup_for_exit", 1
        )[0]
        production_cleanup = lib.split("fn cleanup_for_exit<R", 1)[1].split(
            "#[derive", 1
        )[0]
        self.assertLess(
            cleanup_flow.index("stop_science("), cleanup_flow.index("stop_gateway(")
        )
        self.assertLess(
            production_cleanup.index("stop_sandbox("),
            production_cleanup.index("AppState::stop_proxy"),
        )
        quit_command = lifecycle.split("pub(super) async fn quit_app_command", 1)[1]
        self.assertLess(
            quit_command.index("stop_all_inner_cmd"), quit_command.index("exit_app.exit(0)")
        )
        quit_handler = js.split('els.quitBtn.addEventListener("click"', 1)[1].split(
            "\n  });", 1
        )[0]
        self.assertNotIn("ssh_tunnel_info", quit_handler)
        self.assertIn('setMsg("退出失败："', quit_handler)

    def test_launcher_ignores_large_external_tree_and_broken_legacy_store(self):
        with tempfile.TemporaryDirectory(
            prefix="csswitch-skill-boundary-", dir="/private/tmp"
        ) as raw_tmp:
            tmp = pathlib.Path(raw_tmp)
            outer_home = tmp / "outer-home"
            external = outer_home / ".claude" / "skills"
            legacy_store = outer_home / ".csswitch" / "skills"
            external.mkdir(parents=True)
            legacy_store.mkdir(parents=True)
            for index in range(300):
                skill = external / f"skill-{index:03d}"
                skill.mkdir()
                (skill / "SKILL.md").write_text(
                    f"---\nname: skill-{index:03d}\ndescription: boundary probe\n---\n"
                )
            (legacy_store / "inventory.v1.json").write_text("{broken")

            bin_dir = tmp / "bin"
            bin_dir.mkdir()
            security = bin_dir / "security"
            security.write_text("#!/bin/sh\nexit 0\n")
            security.chmod(0o700)

            marker = tmp / "science-invocation.txt"
            science = tmp / "fake-claude-science"
            science.write_text(
                "#!/bin/sh\n"
                f"printf 'HOME=%s\\n' \"$HOME\" > \"{marker}\"\n"
                f"printf 'ARGS=%s\\n' \"$*\" >> \"{marker}\"\n"
                "exit 0\n"
            )
            science.chmod(0o700)

            sandbox_home = tmp / "sandbox-home"
            data_dir = sandbox_home / ".claude-science"
            existing_skill = data_dir / "orgs" / "org-v043" / "skills" / "existing-skill"
            existing_skill.mkdir(parents=True)
            existing_skill_bytes = (
                b"---\nname: existing-skill\ndescription: v0.4.3 upgrade probe\n---\n"
            )
            (existing_skill / "SKILL.md").write_bytes(existing_skill_bytes)
            active_org_bytes = b'{"org_uuid":"org-v043"}\n'
            (data_dir / "active-org.json").write_bytes(active_org_bytes)
            env = os.environ.copy()
            env.update(
                {
                    "HOME": str(outer_home),
                    "SANDBOX_HOME": str(sandbox_home),
                    "SCIENCE_BIN": str(science),
                    "PATH": f"{bin_dir}:/usr/bin:/bin:/usr/sbin:/sbin",
                }
            )

            external.chmod(0)
            try:
                result = subprocess.run(
                    [
                        str(ROOT / "scripts/launch-virtual-sandbox.sh"),
                        "--port",
                        "19932",
                        "--proxy-url",
                        "http://127.0.0.1:19931/test-secret",
                        "--skip-oauth-forge",
                    ],
                    env=env,
                    capture_output=True,
                    text=True,
                    timeout=15,
                    check=False,
                )
            finally:
                external.chmod(stat.S_IRWXU)

            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            invocation = marker.read_text()
            self.assertIn(f"HOME={sandbox_home}", invocation)
            self.assertIn(
                f"--data-dir {sandbox_home / '.claude-science'}", invocation
            )
            self.assertEqual(
                (existing_skill / "SKILL.md").read_bytes(), existing_skill_bytes
            )
            self.assertEqual(
                (data_dir / "active-org.json").read_bytes(), active_org_bytes
            )
            self.assertNotIn("LIMIT_EXCEEDED", result.stdout + result.stderr)
            self.assertNotIn("STORE_CONFLICT", result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
