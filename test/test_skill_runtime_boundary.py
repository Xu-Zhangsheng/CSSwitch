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
    def test_github_fixture_override_is_acceptance_only_and_reaches_connector_and_host(self):
        desktop_manifest = (
            ROOT / "desktop/src-tauri/Cargo.toml"
        ).read_text()
        gateway_manifest = (
            ROOT / "desktop/gateway/Cargo.toml"
        ).read_text()
        core_manifest = (
            ROOT / "desktop/skill-package/Cargo.toml"
        ).read_text()
        bridge = (
            ROOT / "desktop/src-tauri/src/runtime/skill_install_bridge.rs"
        ).read_text()
        lifecycle = (
            ROOT / "desktop/src-tauri/src/runtime/proxy_lifecycle/lifecycle.rs"
        ).read_text()
        github = (
            ROOT / "desktop/skill-package/src/github.rs"
        ).read_text()

        self.assertIn(
            "acceptance-build = []",
            desktop_manifest,
        )
        self.assertIn(
            'acceptance-build = ["csswitch-skill-install-core/acceptance-build"]',
            gateway_manifest,
        )
        self.assertIn("acceptance-build = []", core_manifest)
        self.assertIn('#[cfg(feature = "acceptance-build")]', bridge)
        self.assertIn("acceptance_github_fixture_base", bridge)
        self.assertIn("configure_acceptance_github_host_command", bridge)
        self.assertIn('"env": connector_env', bridge)
        self.assertIn(
            "configure_acceptance_github_host_command(&mut cmd)", lifecycle
        )
        self.assertIn('#[cfg(feature = "acceptance-build")]', github)
        self.assertIn("GithubEndpoints::production()?", github)
        self.assertNotIn(
            "CSSWITCH_ACCEPTANCE_GITHUB_BASE_URL",
            (ROOT / "desktop/src-tauri/src/runtime/launch_env.rs").read_text(),
        )

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
        source = sandbox_session_one_click_source()
        one_click = source.split(
            "fn one_click_login_with_options", 1
        )[1]
        state_check = one_click.index("let entry_facts = capture_one_click_entry_facts(")
        self.assertLess(one_click.index("config::load_from(&dir)"), state_check)
        self.assertNotIn("GatewayController::ensure_active(", one_click[:state_check])

        runtime_selection = one_click.index("match decide_one_click_entry(entry_facts)")
        self.assertGreater(runtime_selection, state_check)
        launch_check = one_click.index("if !launch.is_file()")
        normal_proxy = one_click.index(
            "GatewayController::ensure_active(", state_check
        )
        self.assertGreater(normal_proxy, launch_check)

        source = sandbox_session_one_click_source()
        command = runtime_command_module("one_click").split(
            "pub(crate) fn one_click_login_cmd", 1
        )[1].split("pub(super) async fn restore_history_choice_command", 1)[0]
        self.assertIn("OneClickEntryPreflight::capture", command)
        self.assertIn("one_click_login_entry(", command)
        self.assertNotIn("recover_interrupted_gateway", command)
        self.assertNotIn("replay_interrupted_one_click_finalize", command)

        facade = source.split("pub(crate) fn one_click_login_entry", 1)[1].split(
            "enum PriorScienceDisposition", 1
        )[0]
        self.assertIn("decide_one_click_entry_recovery(", facade)
        self.assertLess(
            facade.index("OneClickEntryRecoveryDecision::ReplayFinalize"),
            facade.index("replay_interrupted_one_click_finalize"),
        )
        self.assertLess(
            facade.index("OneClickEntryRecoveryDecision::ReplayFinalizeCleanup"),
            facade.index("retry_pending_authority_cleanup"),
        )
        self.assertLess(
            facade.index("OneClickEntryRecoveryDecision::RecoverGateway"),
            facade.index("recover_interrupted_gateway"),
        )
        self.assertIn("OneClickEntryRecoveryDecision::Route", facade)

        coordinator = source.split("fn one_click_login_with_options", 1)[1]
        facts = coordinator.index("capture_one_click_entry_facts(")
        decision = coordinator.index("decide_one_click_entry(entry_facts)")
        cleanup = coordinator.index("retry_pending_authority_cleanup")
        ssh_preflight = coordinator.index("prevalidate_one_click_system_ssh")
        stub_capture = coordinator.index("ManagedSshStubTransaction::capture")
        self.assertLess(facts, decision)
        self.assertLess(decision, cleanup)
        self.assertLess(cleanup, ssh_preflight)
        self.assertLess(ssh_preflight, stub_capture)
        cleanup_branch = coordinator[decision:ssh_preflight]
        self.assertIn("one_click_login_with_options(", cleanup_branch)
        self.assertIn("entry_progress.after_cleanup()", cleanup_branch)

        pure_decision = source.split("fn decide_one_click_entry(", 1)[1].split(
            "pub(crate) fn replay_interrupted_one_click_finalize", 1
        )[0]
        for protected_effect in (
            "retry_pending_authority_cleanup",
            "ManagedSshStubTransaction::capture",
            "prevalidate_one_click_system_ssh",
            "ensure_virtual_login",
            "GatewayController::ensure_active",
            "ScienceHostAdapter::stop",
        ):
            self.assertNotIn(protected_effect, pure_decision)

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
            "function renderDoctorIntentResult", 1
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
        self.assertIn("ScienceHostAdapter::listener_matches", command)
        self.assertIn("ScienceHostAdapter::url(sandbox_port, &runtime)", command)
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
        one_click_runtime = (
            ROOT / "desktop/src-tauri/src/runtime/sandbox_session/one_click/cold.rs"
        ).read_text()
        compensation_runtime = (
            ROOT
            / "desktop/src-tauri/src/runtime/sandbox_session/one_click/cold/compensation.rs"
        ).read_text()
        science_phase_runtime = (
            ROOT
            / "desktop/src-tauri/src/runtime/sandbox_session/one_click/cold/science_phase.rs"
        ).read_text()

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
            one_click.index('call("finalize_consumer_state", { outcome: r })'),
            one_click.index('const message = r.msg ||'),
        )
        self.assertIn('consumer.disposition !== "ready"', one_click)
        self.assertNotIn(
            "getConfigState().applied_profile_id = getConfigState().active_id",
            one_click,
        )
        self.assertIn("setBusy(false)", one_click)
        self.assertIn("setBrowserFallback(r.fallback_url)", one_click)

        gateway_ready = one_click_runtime.index("verify_gateway_model_catalog_traced(")
        science_dispatch = one_click_runtime.index(
            "run_managed_science_launch_phase(", gateway_ready
        )
        verify_catalog = one_click_runtime.index(
            "RuntimeTransactionPhase::VerifyScienceCatalog", science_dispatch
        )
        self.assertLess(gateway_ready, science_dispatch)
        self.assertLess(science_dispatch, verify_catalog)
        self.assertIn("ScienceHostAdapter::spawn_launch", science_phase_runtime)
        self.assertNotIn("ScienceHostAdapter::spawn_launch", one_click_runtime)
        self.assertIn("fn compensate_one_click_failure", compensation_runtime)
        self.assertNotIn("fn compensate_one_click_failure", one_click_source)
        self.assertNotIn("fn compensate_one_click_failure", one_click_runtime)
        self.assertIn(
            "pub(super) use compensation::compensate_one_click_failure",
            one_click_runtime,
        )
        self.assertEqual(
            one_click_runtime.count("write_one_click_checkpoint(")
            + science_phase_runtime.count("write_one_click_checkpoint("),
            8,
        )
        cold_checkpoint_stages = re.findall(
            r"(?s)write_one_click_checkpoint\(\s*&dir,\s*&transaction_identity,\s*"
            r"&mut journal_progress,\s*config::RuntimeTransactionPhase::(\w+),\s*\)",
            one_click_runtime,
        )
        science_checkpoint_stages = re.findall(
            r"(?s)write_one_click_checkpoint\(\s*dir,\s*transaction_identity,\s*"
            r"journal_progress,\s*config::RuntimeTransactionPhase::(\w+),\s*\)",
            science_phase_runtime,
        )
        self.assertEqual(
            cold_checkpoint_stages,
            [
                "StopOldScience",
                "StartGateway",
                "AuthoritySnapshotActive",
                "VerifyScienceCatalog",
            ],
        )
        self.assertEqual(
            science_checkpoint_stages,
            [
                "StartScienceEnvironmentPending",
                "WaitScienceDbReverify",
                "RestartScienceAfterDbHeal",
                "VerifyScienceDbAfterRestart",
            ],
        )
        self.assertRegex(
            one_click_runtime,
            r"let candidate_fingerprint = launch_runtime\.environment_transaction_id\(\);",
        )
        self.assertEqual(one_click_runtime.count("environment_transaction_id()"), 1)
        self.assertRegex(
            one_click_runtime,
            r"(?s)let snapshot_ticket = match authority_transaction\.registered_snapshot_ticket\(\)"
            r".*?let transaction_identity = OneClickTransactionIdentity \{"
            r".*?runtime_fingerprint: candidate_fingerprint,"
            r".*?snapshot_ticket: snapshot_ticket\.clone\(\),",
        )
        self.assertIn("let mut journal_progress = match prior_stop_record", one_click_runtime)
        self.assertRegex(
            one_click_runtime,
            r"(?s)Some\(record\) => OneClickJournalProgress::Journaled \{"
            r".*?record,.*?registered_ticket: snapshot_ticket,.*?"
            r"None => OneClickJournalProgress::PreJournalAbort \{"
            r".*?registered_ticket: snapshot_ticket,",
        )
        self.assertNotIn("RuntimeTransactionJournal {", one_click_runtime)
        self.assertNotIn("SCIENCE_ENVIRONMENT_PENDING_STAGE_PREFIX", one_click_source)
        self.assertNotIn("AUTHORITY_SNAPSHOT_ACTIVE_STAGE_PREFIX", one_click_source)
        self.assertEqual(one_click_runtime.count("runtime_transaction = Some("), 0)

        one_click_command = command_one_click.split(
            "pub(crate) fn one_click_login_cmd", 1
        )[1].split("pub(super) async fn restore_history_choice_command", 1)[0]
        self.assertIn("one_click_login_entry(", one_click_command)
        self.assertNotIn("recover_interrupted_gateway", one_click_command)
        self.assertNotIn("replay_interrupted_one_click_finalize", one_click_command)
        recovery_projection = one_click_source.split(
            "fn typed_interrupted_gateway_recovery_error", 1
        )[1].split("pub(super) enum TransactionScienceStopBoundary", 1)[0]
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
        self.assertIn("project_consumer_state(&value)", boot)
        self.assertIn("FinalizeConsumerDisposition::Ready", boot)
        self.assertIn("FinalizeConsumerDisposition::Attention", boot)
        self.assertIn("FinalizeConsumerDisposition::Manual", boot)
        self.assertIn("mark_boot_attention(&app, value)", boot)
        self.assertIn("mark_boot_failed(&app, value)", boot)
        main = (ROOT / "desktop/src/main.js").read_text(encoding="utf-8")
        self.assertEqual(
            main.count("runtimeController.publishFinalizeUnknown();"),
            2,
        )
        self.assertIn('listen("boot://publication"', main)
        self.assertIn('call("boot_snapshot")', main)
        self.assertIn("publication.sequence <= lastBootSequence", main)
        self.assertNotIn('listen("boot://failed"', main)
        self.assertNotIn('call("boot_attention")', main)

    def test_science_runtime_identity_is_reused_for_serve_status_url_and_stop(self):
        session = sandbox_session_source()
        science = science_runtime_source()
        science_contracts = (
            ROOT / "desktop/src-tauri/src/runtime/science/contracts.rs"
        ).read_text()
        science_lifecycle = (
            ROOT / "desktop/src-tauri/src/runtime/science/lifecycle.rs"
        ).read_text()
        science_host = (
            ROOT / "desktop/src-tauri/src/runtime/science/host_adapter.rs"
        ).read_text()
        history_recovery = (
            ROOT
            / "desktop/src-tauri/src/runtime/sandbox_session/history_recovery.rs"
        ).read_text()
        launch_env = (ROOT / "desktop/src-tauri/src/runtime/launch_env.rs").read_text()
        runtime = runtime_command_source()
        one_click = sandbox_session_one_click_source().split(
            "fn one_click_login_with_options", 1
        )[1]
        self.assertIn("science_bin: Path::new(&spec.runtime.path)", science_host)
        self.assertIn("ScienceLaunchSpec::one_click(", session)
        self.assertIn("&proxy_url,", session)
        self.assertIn('"SCIENCE_BIN".into(), cfg.science_bin.display().to_string()', launch_env)
        self.assertIn('"CSSWITCH_PROXY_URL".into(), cfg.proxy_url.into()', launch_env)
        self.assertNotIn('.arg(&proxy_url)', session)
        self.assertIn("current.science_runtime = Some(launch_runtime.clone())", one_click)
        self.assertIn(
            "ScienceHostAdapter::probe_known(cfg.sandbox_port, &runtime)", session
        )
        self.assertIn("sandbox_listener_matches_runtime(healthy.port", science_host)
        self.assertIn("ScienceHostAdapter::url(sport, &launch_runtime)", session)
        self.assertRegex(
            session,
            r"ScienceHostAdapter::validate_launch_runtime\(&?launch_runtime\)",
        )
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
        self.assertIn("fn execute_transaction_science_stop_with", session)
        self.assertIn(
            "outcome.and_then(|verified| verified.require_exact_stop_of(expected_runtime))",
            session,
        )
        self.assertIn("Ok(verified) => verified.confirmed_runtime()", session)
        self.assertIn("current.science_confirmed_stopped = Some(confirmed_runtime.clone())", session)
        force_restart = session.split(
            "pub(crate) fn force_restart_science_for_active", 1
        )[1].split("fn typed_one_click_err", 1)[0]
        history_restore = history_recovery.split(
            "pub(crate) fn restore_history_choice_entry", 1
        )[1]
        for recovery_caller in (force_restart, history_restore):
            self.assertIn("ScienceStopRequest::exact", recovery_caller)
            self.assertIn("execute_transaction_science_stop_with", recovery_caller)
        self.assertIn(
            "TransactionScienceStopBoundary::ProfileSwitchRollback", force_restart
        )
        self.assertIn(
            "TransactionScienceStopBoundary::HistoryRecoveryPriorStop", history_restore
        )
        self.assertIn("HistoryRecoveryScienceQuiescence", session)
        self.assertIn("HistoryRecoveryScienceQuiescence", history_restore)
        self.assertIn("ExactStopped(expected)", history_restore)
        self.assertIn("NoManagedRuntimeObserved", history_restore)
        self.assertIn("science_quiescence.clone()", history_restore)
        self.assertIn("ScienceHostAdapter::probe_cached", history_restore)
        self.assertIn("observed != SandboxScienceState::Stopped", history_restore)
        self.assertIn("observed_runtime.is_some()", history_restore)
        self.assertIn("session.science_quiescence =", history_restore)
        self.assertIn(
            "crate::HistoryRecoveryScienceQuiescence::ExactStopped(runtime.clone())",
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
        for owner_field in (
            "runtime: st.science_runtime.clone()",
            "confirmed_stopped: st.science_confirmed_stopped.clone()",
            "sandbox_child_pid: st.sandbox.as_ref().map(std::process::Child::id)",
            "sandbox_port: st.sandbox_port",
            "sandbox_url: st.sandbox_url.clone()",
        ):
            self.assertIn(owner_field, codex)
        self.assertRegex(
            codex,
            r"(?s)CodexScienceOwnerSnapshot::claim.*ScienceHostAdapter::probe_known",
        )
        self.assertIn("if !owner.still_owns(st, current_generation)", codex)
        self.assertIn("st.science_confirmed_stopped = owner.confirmed_stopped", codex)
        for typed_publisher in (
            runtime_command_module("lifecycle"),
            runtime_command_module("one_click"),
            codex,
        ):
            self.assertNotIn("science_confirmed_stopped = Some(", typed_publisher)
        self.assertEqual(session.count("science_confirmed_stopped = Some("), 1)
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
        self.assertIn("ScienceHostAdapter::claim_stop", stop_all)
        self.assertIn("ScienceHostAdapter::execute_stop", stop_all)
        self.assertIn("owner.still_owns", stop_all)
        self.assertIn("lifecycle.current_generation()", stop_all)
        self.assertNotIn("stop_sandbox_state(&app, &mut st)", stop_all)
        claim_flow = lifecycle_command.split(
            "fn claim_process_local_science_stop", 1
        )[1].split("fn publish_process_local_science_stop", 1)[0]
        self.assertLess(
            claim_flow.index("let st = lock(state)"),
            claim_flow.index("claim_science(owner.runtime.as_ref())"),
        )
        stop_all_flow = lifecycle_command.split(
            "pub(super) fn stop_all_inner_with", 1
        )[1].split("pub(super) async fn quit_app_command", 1)[0]
        self.assertLess(
            stop_all_flow.index("claim_process_local_science_stop"),
            stop_all_flow.index("execute_science(&app, request)"),
        )
        self.assertLess(
            stop_all_flow.index("execute_science(&app, request)"),
            stop_all_flow.rindex("lock(&state)"),
        )
        self.assertLess(
            stop_all_flow.rindex("lock(&state)"),
            stop_all_flow.index("publish_process_local_science_stop"),
        )
        set_mode_flow = lifecycle_command.split(
            "pub(super) fn set_mode_inner_with", 1
        )[1].split("pub(crate) struct UiSettings", 1)[0]
        self.assertLess(
            set_mode_flow.index("claim_process_local_science_stop"),
            set_mode_flow.index("execute_science(&app, request)"),
        )
        self.assertLess(
            set_mode_flow.index("execute_science(&app, request)"),
            set_mode_flow.index("let mut st = lock(&state)"),
        )
        self.assertLess(
            set_mode_flow.index("let mut st = lock(&state)"),
            set_mode_flow.index("publish_process_local_science_stop"),
        )
        self.assertLess(
            set_mode_flow.index("publish_process_local_science_stop"),
            set_mode_flow.index("stop_gateway(&mut st)"),
        )
        self.assertIn("require_confirmed_gateway_stop", set_mode_flow)
        set_settings_flow = lifecycle_command.split(
            "pub(super) fn set_settings_inner_with", 1
        )[1].split("pub(super) async fn stop_all_command", 1)[0]
        self.assertNotIn("stop_sandbox_state(&app, &mut st)", set_settings_flow)
        self.assertLess(
            set_settings_flow.index("claim_process_local_science_stop"),
            set_settings_flow.index("execute_science(&app, request)"),
        )
        self.assertLess(
            set_settings_flow.index("execute_science(&app, request)"),
            set_settings_flow.index("let mut st = lock(&state)"),
        )
        self.assertLess(
            set_settings_flow.index("let mut st = lock(&state)"),
            set_settings_flow.index("publish_process_local_science_stop"),
        )
        self.assertLess(
            set_settings_flow.index("publish_process_local_science_stop"),
            set_settings_flow.index("lifecycle.bump_generation()"),
        )
        self.assertLess(
            set_settings_flow.index("lifecycle.bump_generation()"),
            set_settings_flow.index("stop_gateway(&mut st)"),
        )
        self.assertLess(
            set_settings_flow.index("stop_gateway(&mut st)"),
            set_settings_flow.index("revoke_science_ssh_bridge"),
        )
        self.assertIn("require_confirmed_gateway_stop", set_settings_flow)
        self.assertLess(
            set_settings_flow.index("revoke_science_ssh_bridge"),
            set_settings_flow.index("config::update_result"),
        )
        downgrade_flow = codex.split(
            "fn stop_all_before_downgrade_with", 1
        )[1].split("fn production_home", 1)[0]
        self.assertNotIn("stop_sandbox_state(app, &mut app_state)", downgrade_flow)
        self.assertLess(
            downgrade_flow.index("lifecycle.bump_generation()"),
            downgrade_flow.index("execute_process_local_science_stop_with"),
        )
        self.assertLess(
            downgrade_flow.index("execute_process_local_science_stop_with"),
            downgrade_flow.index("stop_gateway(&mut lock(state))"),
        )
        self.assertIn("require_stopped", downgrade_flow)
        self.assertIn("pub(crate) fn claim_science_stop_request", science_lifecycle)
        self.assertIn("pub(crate) fn execute_science_stop", science_lifecycle)
        self.assertIn("ScienceStopRequest::exact", science_lifecycle)

    def test_s4_science_host_adapter_owns_launch_health_receipt_and_stop_facade(self):
        one_click = "\n".join(
            (
                (
                    ROOT
                    / "desktop/src-tauri/src/runtime/sandbox_session/one_click.rs"
                ).read_text(),
                (
                    ROOT
                    / "desktop/src-tauri/src/runtime/sandbox_session/one_click/cold.rs"
                ).read_text(),
                (
                    ROOT
                    / "desktop/src-tauri/src/runtime/sandbox_session/one_click/cold/compensation.rs"
                ).read_text(),
                (
                    ROOT
                    / "desktop/src-tauri/src/runtime/sandbox_session/one_click/cold/science_phase.rs"
                ).read_text(),
            )
        )
        runtime_lifecycle = runtime_command_module("lifecycle")
        host = (
            ROOT / "desktop/src-tauri/src/runtime/science/host_adapter.rs"
        ).read_text()

        for contract in (
            "ScienceHostAdapter",
            "ScienceLaunchSpec",
            "ScienceEnvironmentExposure",
            "ScienceLaunchAttempt",
            "ScienceHealthyLaunch",
            "ScienceVerifiedLaunch",
            "ScienceLaunchReceipt",
            "ScienceLaunchFailureKind",
        ):
            self.assertIn(contract, host)

        self.assertNotIn('Command::new("zsh")', one_click)
        self.assertNotIn("configure_science_launch_script_command", one_click)
        self.assertNotIn("SCIENCE_LAUNCH_ENVIRONMENT_EXPOSED_EXIT_CODE", one_click)
        self.assertIn(
            "launch_environment: ScienceEnvironmentExposure", one_click
        )
        self.assertNotIn("launch_attempted: bool", one_click)
        self.assertIn('Command::new("zsh")', host)
        self.assertIn("configure_science_launch_script_command", host)
        self.assertIn("SCIENCE_LAUNCH_ENVIRONMENT_EXPOSED_EXIT_CODE", host)

        initial = one_click.split("let opaque_bindings =", 1)[1].split(
            "rollback_context.set_kind(OneClickFailureKind::ScienceDbReverify)", 1
        )[0]
        ordered_initial = (
            "ScienceHostAdapter::spawn_launch",
            "transaction.observe_after_launch",
            "ScienceHostAdapter::accept_launch_script",
            "current.science_runtime = Some(launch_runtime.clone())",
            "ScienceHostAdapter::verify_health",
            'trace.stage(OperationStage::SandboxHealth, "ready")',
            "ScienceHostAdapter::verify_identity",
            "ScienceHostAdapter::commit_launch",
        )
        offsets = [initial.index(fragment) for fragment in ordered_initial]
        self.assertEqual(offsets, sorted(offsets))

        ordered_host = (
            'Command::new("zsh")',
            "configure_science_launch_script_command",
            ".spawn()",
            "proc::http_health",
            "sandbox_listener_matches_runtime",
            "uncommitted_managed_science_launch_token",
            "record_managed_science_launch",
        )
        offsets = [host.index(fragment) for fragment in ordered_host]
        self.assertEqual(offsets, sorted(offsets))
        self.assertGreater(
            host.index("SCIENCE_LAUNCH_ENVIRONMENT_EXPOSED_EXIT_CODE", offsets[2]),
            offsets[2],
        )
        self.assertIn("managed_launch_token_is_current_for_runtime", host)

        self.assertIn("ScienceHostAdapter::claim_stop", runtime_lifecycle)
        self.assertIn("ScienceHostAdapter::execute_stop", runtime_lifecycle)
        self.assertIn("ScienceHostAdapter::stop", runtime_lifecycle)
        self.assertIn("claim_science_stop_request(runtime)", host)
        self.assertIn("execute_science_stop(app, request)", host)
        self.assertIn("stop_sandbox(app, sandbox, sandbox_url, request)", host)

        rust_root = ROOT / "desktop/src-tauri/src"
        low_level_owner_paths = {
            rust_root / "runtime/science/host_adapter.rs",
            rust_root / "runtime/science/lifecycle.rs",
            rust_root / "runtime/science/managed_launch.rs",
        }
        production_paths = sorted(
            path
            for path in rust_root.rglob("*.rs")
            if path not in low_level_owner_paths
            and path.name != "tests.rs"
            and "transaction_tests" not in path.parts
        )
        production_sources = {
            path: path.read_text() for path in production_paths
        }
        production = "\n".join(production_sources.values())
        executable_path = rust_root / "runtime/science/executable.rs"
        science_phase_path = (
            rust_root / "runtime/sandbox_session/one_click/cold/science_phase.rs"
        )
        self.assertIn(
            "pub(crate) fn science_runtime_preflight",
            production_sources[executable_path],
        )
        self.assertIn(
            "ScienceHostAdapter::probe_cached",
            production_sources[executable_path],
        )
        self.assertIn(
            "ScienceHostAdapter::spawn_launch",
            production_sources[science_phase_path],
        )
        for bypass in (
            "probe_known_runtime(",
            "probe_sandbox_runtime_cached(",
            "claim_science_stop_request(",
            "execute_science_stop(",
            "stop_sandbox(",
            "record_managed_science_launch(",
            "uncommitted_managed_science_launch_token(",
            "managed_launch_token_for_runtime(",
            "managed_launch_token_is_current_for_runtime(",
            "managed_launch_token_process_is_alive(",
            "sandbox_listener_matches_runtime(",
            "sandbox_url(",
        ):
            self.assertNotIn(bypass, production)

    def test_s5_authority_transaction_owns_capture_restore_cleanup_facade(self):
        one_click = "\n".join(
            (
                (
                    ROOT
                    / "desktop/src-tauri/src/runtime/sandbox_session/one_click.rs"
                ).read_text(),
                (
                    ROOT
                    / "desktop/src-tauri/src/runtime/sandbox_session/one_click/cold.rs"
                ).read_text(),
                (
                    ROOT
                    / "desktop/src-tauri/src/runtime/sandbox_session/one_click/cold/compensation.rs"
                ).read_text(),
                (
                    ROOT
                    / "desktop/src-tauri/src/runtime/sandbox_session/one_click/cold/science_phase.rs"
                ).read_text(),
            )
        )
        facade = (
            ROOT
            / "desktop/src-tauri/src/runtime/sandbox_session/authority_transaction.rs"
        ).read_text()

        for contract in (
            "struct AuthorityTransaction",
            "fn capture(",
            "fn registered_snapshot_ticket(",
            "fn preserve_recovery(",
            "fn recovery_path(",
            "fn validate_science_restore_root(",
            "fn science_opaque_bindings_env(",
            "fn restore<R: Runtime>(",
            "fn cleanup_when_expendable(",
            "fn prepare_success(",
            "fn commit(",
        ):
            self.assertIn(contract, facade)

        for delegated_contract in (
            "OneClickAuthoritySnapshot::capture",
            "self.snapshot.registered_snapshot_ticket()",
            "self.snapshot.restore_with_gateway(",
            "self.snapshot.cleanup_when_expendable()",
            "self.snapshot.prepare_success(value)",
            "self.snapshot.commit()",
        ):
            self.assertIn(delegated_contract, facade)

        self.assertNotIn("OneClickAuthoritySnapshot", one_click)
        self.assertNotIn("restore_with_gateway(", one_click)
        self.assertIn("AuthorityTransaction::capture", one_click)
        self.assertIn("authority_transaction.restore(", one_click)
        self.assertIn("authority_transaction.cleanup_when_expendable()", one_click)
        self.assertRegex(one_click, r"authority_transaction\s*\.prepare_success")
        self.assertIn("authority_transaction.commit()", one_click)

        coordinator = one_click.split("fn one_click_login_with_options", 1)[1]
        prior_stop_start = coordinator.index(
            "if let Some(prior) = prior_science.as_ref()"
        )
        prior_stop_end = coordinator.index(
            "let prior_science_for_compensation", prior_stop_start
        )
        prior_stop = coordinator[prior_stop_start:prior_stop_end]
        self.assertEqual(prior_stop.count("stop_prior_science_with("), 1)
        self.assertEqual(prior_stop.count("ScienceHostAdapter::execute_stop("), 1)
        stop_offset = coordinator.index("stop_prior_science_with(", prior_stop_start)
        capture_offset = coordinator.index(
            "capture_authority_after_science_quiesce(", prior_stop_end
        )
        ticket_offset = coordinator.index(
            "authority_transaction.registered_snapshot_ticket()", capture_offset
        )
        checkpoint_offset = coordinator.index("write_one_click_checkpoint(", ticket_offset)
        commit_offset = coordinator.rindex("authority_transaction.commit()")
        compensation_offset = coordinator.rindex("compensate_one_click_failure(")
        self.assertLess(stop_offset, capture_offset)
        self.assertLess(capture_offset, ticket_offset)
        self.assertLess(ticket_offset, checkpoint_offset)
        self.assertLess(checkpoint_offset, commit_offset)
        self.assertLess(checkpoint_offset, compensation_offset)

        self.assertIn("struct CompensationOutcome", one_click)
        self.assertNotIn("CompensationOutcome", facade)
        self.assertIn("write_one_click_checkpoint(", one_click)
        self.assertIn("commit_runtime_binding(", one_click)
        for forbidden in (
            "PriorStopIntent",
            "PriorStopOutcome",
            "GatewayController",
            "recovery_status",
            "environment_status",
            "write_one_click_checkpoint",
            "commit_runtime_binding",
        ):
            self.assertNotIn(forbidden, facade)

    def test_s3_runtime_mutation_domains_and_local_skill_host_receipt_are_typed(self):
        lifecycle = (ROOT / "desktop/src-tauri/src/lifecycle.rs").read_text()
        for domain in ("Intent", "Destructive", "HostBridge", "Terminal"):
            self.assertIn(domain, lifecycle)
        self.assertIn("pub(crate) struct RuntimeMutationLease", lifecycle)
        self.assertIn("pub(crate) fn acquire_mutation", lifecycle)
        self.assertIn("pub(crate) fn with_mutation", lifecycle)

        production_paths = (
            "desktop/src-tauri/src/commands/codex.rs",
            "desktop/src-tauri/src/commands/diagnostics.rs",
            "desktop/src-tauri/src/commands/profiles.rs",
            "desktop/src-tauri/src/commands/runtime/lifecycle.rs",
            "desktop/src-tauri/src/commands/runtime/one_click.rs",
            "desktop/src-tauri/src/commands/runtime/gateway.rs",
            "desktop/src-tauri/src/commands/skills.rs",
            "desktop/src-tauri/src/lib.rs",
        )
        production = "\n".join(
            (ROOT / path).read_text().split("#[cfg(test)]", 1)[0]
            for path in production_paths
        )
        self.assertNotIn("with_serialized(", production)
        for domain in ("Intent", "Destructive", "HostBridge", "Terminal"):
            self.assertIn(f"RuntimeMutationDomain::{domain}", production)

        diagnostics = (
            ROOT / "desktop/src-tauri/src/commands/diagnostics.rs"
        ).read_text().split("#[cfg(test)]", 1)[0]
        self.assertIn("canonical_asset_root(app)", diagnostics)
        self.assertIn("doctor_gateway_bin_path(app)", diagnostics)
        self.assertNotIn("let root = asset_root(app)", diagnostics)
        self.assertNotIn("proxy_lifecycle::gateway_bin_path(app)", diagnostics)
        self.assertIn("cmd.env_clear()", diagnostics)

        local_skill = (
            ROOT / "desktop/src-tauri/src/commands/skill_install.rs"
        ).read_text()
        command = local_skill.split(
            "pub(crate) async fn install_local_skill_package", 1
        )[1].split("fn install_after_picker", 1)[0]
        self.assertLess(command.index("blocking_pick_file"), command.index("install_after_picker"))
        final_section = local_skill.split("fn install_after_picker", 1)[1].split(
            "fn current_science_context", 1
        )[0]
        self.assertIn("RuntimeMutationDomain::HostBridge", final_section)
        self.assertIn("LocalSkillHostReceipt", final_section)
        self.assertIn("PhantomData<&'lease RuntimeMutationLease<'guard>>", local_skill)
        receipt_decl = local_skill.split("struct LocalSkillHostReceipt", 1)[0].rsplit(
            "#[derive", 1
        )[1]
        self.assertNotIn("Clone", receipt_decl)
        install_entry = local_skill.split("fn install_selected_path", 1)[1].split("{", 1)[0]
        self.assertIn("LocalSkillHostReceipt", install_entry)
        self.assertNotIn("ScienceHostContext", install_entry)
        self.assertLess(
            final_section.index("claim_local_skill_host"),
            final_section.index("install_selected_path"),
        )
        self.assertNotIn("runtime_transaction", final_section)

    def test_s6_gateway_controller_receipt_and_removed_start_proxy_surface(self):
        lib = (ROOT / "desktop/src-tauri/src/lib.rs").read_text()
        runtime_command = (
            ROOT / "desktop/src-tauri/src/commands/runtime.rs"
        ).read_text()
        gateway_command = (
            ROOT / "desktop/src-tauri/src/commands/runtime/gateway.rs"
        ).read_text()
        controller = (
            ROOT
            / "desktop/src-tauri/src/runtime/proxy_lifecycle/controller.rs"
        ).read_text()
        lifecycle = (
            ROOT
            / "desktop/src-tauri/src/runtime/proxy_lifecycle/lifecycle.rs"
        ).read_text()

        registration = lib.split(
            ".invoke_handler(tauri::generate_handler![", 1
        )[1].split("])\n", 1)[0]
        self.assertNotIn("commands::runtime::start_proxy", registration)
        self.assertNotIn("pub(crate) async fn start_proxy", runtime_command)
        self.assertNotIn("start_proxy_command", gateway_command)
        self.assertNotIn("start_proxy_inner_cmd", gateway_command)

        for contract in (
            "pub(crate) struct GatewayController",
            "pub(crate) struct GatewayReceipt",
            "pub(crate) struct GatewayHealthReceipt",
            "pub(crate) struct GatewayCatalogReceipt",
            "pub(crate) struct GatewayLaunchRecipe",
            "pub(crate) fn ensure_active",
            "pub(crate) fn start_for",
            "route_secret: String",
            "action: ProxyAction",
            "health: GatewayHealthReceipt",
            "catalog: GatewayCatalogReceipt",
            "recipe: GatewayLaunchRecipe",
            "provider_contract_id: String",
            "provider_contract_digest: String",
            "intent: String",
            "expected_fingerprint: Option<String>",
            "accepted_fingerprint: String",
            "accepted_health: &proc::GatewayHealth",
        ):
            self.assertIn(contract, controller)

        receipt_decl = controller.split("pub(crate) struct GatewayReceipt", 1)[0].rsplit(
            "///", 1
        )[1]
        for forbidden in ("Debug", "Serialize", "Deserialize"):
            self.assertNotIn(forbidden, receipt_decl)
        self.assertIn("fn start_proxy_for_inner", lifecycle)
        self.assertNotIn("pub(crate) fn start_proxy_for_inner", lifecycle)
        self.assertIn("effective_science_runtime", lifecycle)
        self.assertIn("science_runtime: effective_science_runtime", lifecycle)
        self.assertNotIn("http_health_gateway", lifecycle)
        self.assertEqual(lifecycle.count("proc::http_gateway_health("), 2)
        self.assertEqual(lifecycle.count("accepted_gateway_health("), 2)
        self.assertEqual(
            lifecycle.count("st.gateway_launch_context = Some(recipe.clone())"), 2
        )

        caller_paths = (
            "desktop/src-tauri/src/runtime/profile_switch.rs",
            "desktop/src-tauri/src/runtime/sandbox_session/recovery.rs",
            "desktop/src-tauri/src/runtime/sandbox_session/one_click.rs",
            "desktop/src-tauri/src/runtime/sandbox_session/one_click/healthy_reopen.rs",
        )
        callers = "\n".join((ROOT / path).read_text() for path in caller_paths)
        self.assertIn("GatewayController::ensure_active", callers)
        self.assertIn("GatewayController::start_for", callers)
        self.assertNotIn("ensure_proxy(", callers)
        self.assertNotIn("start_proxy_for(", callers)

        inventory = (
            ROOT / "quality/runtime-mutation-inventory.v1.json"
        ).read_text()
        self.assertNotIn('"op.start-gateway-only"', inventory)
        self.assertNotIn('"name": "start_proxy"', inventory)

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
            cleanup_flow.index("execute_process_local_science_stop_with"),
            cleanup_flow.index("stop_gateway("),
        )
        self.assertLess(
            production_cleanup.index("ScienceHostAdapter::claim_stop"),
            production_cleanup.index("ScienceHostAdapter::execute_stop"),
        )
        self.assertLess(
            production_cleanup.index("ScienceHostAdapter::execute_stop"),
            production_cleanup.index("AppState::stop_proxy"),
        )
        quit_command = lifecycle.split("pub(super) async fn quit_app_command", 1)[1]
        self.assertLess(
            quit_command.index("RuntimeMutationDomain::Terminal"),
            quit_command.index("exit_app.exit(0)"),
        )
        self.assertIn("stop_all_inner_with", quit_command)
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
