import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]


def sandbox_session_source():
    module_dir = ROOT / "desktop/src-tauri/src/runtime/sandbox_session"
    sources = sorted(module_dir.rglob("*.rs"))
    return "\n".join(path.read_text() for path in sources)


class ProfilePinContractTests(unittest.TestCase):
    def test_backend_pin_is_local_and_one_click_remains_the_apply_boundary(self):
        profiles = (ROOT / "desktop/src-tauri/src/commands/profiles.rs").read_text()
        pin = profiles.split("fn pin_active_profile_in_dir(", 1)[1].split(
            "\n#[cfg(test)]", 1
        )[0]

        self.assertIn("config::update_result", pin)
        self.assertIn("cfg.active_id = id.to_string()", pin)
        self.assertIn("config::require_no_runtime_transaction(cfg)?", pin)
        config_source = (ROOT / "desktop/src-tauri/src/config.rs").read_text()
        self.assertIn("code=runtime_transaction_in_progress", config_source)
        self.assertIn("resolve_launch_plan(profile)?", pin)
        self.assertIn('object.insert("apply_state".into()', pin)
        self.assertIn('serde_json::Value::String("pending".into())', pin)
        for forbidden in (
            "prepare_provider_auth",
            "scratch_validate_candidate",
            "set_active_profile_txn",
            "start_proxy",
            "stop_proxy",
            "probe_",
            "runtime_binding =",
            "runtime_transaction =",
            "history_recovery",
            "boot_attention",
        ):
            self.assertNotIn(forbidden, pin)

        one_click = (
            ROOT / "desktop/src-tauri/src/runtime/sandbox_session/one_click/cold.rs"
        ).read_text()
        self.assertRegex(
            one_click,
            r"begin_one_click_finalize\(\s*&dir,\s*&transaction_identity,\s*"
            r"&mut journal_progress,\s*"
            r"config::RuntimeFinalizeAction::CommitBinding\s*\{\s*"
            r"binding:\s*committed,\s*"
            r"science_adoption_attempt_id:\s*Some\(adoption_attempt_id\),\s*\},\s*\)"
            r"[\s\S]*?complete_one_click_finalize\(\s*&dir,\s*&mut journal_progress\s*\)",
        )
        self.assertRegex(
            one_click,
            r"let adoption_attempt_id\s*=\s*match\s*"
            r"committed\.science_adoption_attempt_id\.clone\(\)\s*\{\s*"
            r"Some\(attempt_id\)\s*if\s*launch_runtime\.adoption_attempt_id\(\)\s*"
            r"==\s*Some\(attempt_id\.as_str\(\)\)\s*=>\s*\{\s*attempt_id\s*\}\s*"
            r"_\s*=>\s*\{\s*return\s*Err\(rollback_context\.failure\(\s*"
            r'"Science managed receipt、runtime 与 binding adoption provenance 不一致"\s*'
            r",?\s*\)\);\s*\}\s*\};",
        )
        preset = profiles.split("fn apply_profile_preset_sync_inner_cmd", 1)[1].split(
            "// ---------- profile CRUD", 1
        )[0]
        connection = profiles.split("fn update_profile_connection_inner_cmd", 1)[1].split(
            "/// 只把 profile", 1
        )[0]
        connection_flow = profiles.split("fn update_profile_connection_with", 1)[1].split(
            "fn commit_profile_connection_in_dir", 1
        )[0]
        connection_commit = profiles.split("fn commit_profile_connection_in_dir", 1)[
            1
        ].split("/// 只把 profile", 1)[0]
        self.assertNotIn("set_active_profile_txn", preset + connection)
        self.assertNotIn("cfg.active_id == id", preset + connection)
        self.assertGreaterEqual(preset.count("load_without_runtime_transaction"), 2)
        self.assertLess(
            connection_flow.index("load_without_runtime_transaction(dir)"),
            connection_flow.index("let prepared = prepare("),
        )
        self.assertLess(
            connection_flow.index("let prepared = prepare("),
            connection_flow.index(".with_mutation("),
        )
        self.assertRegex(
            connection_flow,
            r"\.with_mutation\(\s*lifecycle::RuntimeMutationDomain::Intent\s*,",
        )
        self.assertIn("verify(&prepared, dir)?", connection_flow)
        self.assertLess(
            connection_commit.index("load_without_runtime_transaction(dir)"),
            connection_commit.index("let validated = validate(&candidate)?"),
        )
        self.assertIn("config::require_no_runtime_transaction(cfg)?", profiles)

    def test_ui_activation_has_no_skip_path_and_reports_pending_selection(self):
        main = (ROOT / "desktop/src/profile-controller.js").read_text()
        runtime = (ROOT / "desktop/src/runtime-controller.js").read_text()
        js = main + runtime
        activate = main.split("async function activate(id)", 1)[1].split(
            "\n  return {", 1
        )[0]
        boundary = runtime.split("async function checkOneClickBoundary()", 1)[1].split(
            "\nasync function runOneClick", 1
        )[0]

        self.assertIn('call("set_active_profile", { id })', activate)
        self.assertIn("当前选择", activate)
        self.assertIn("待一键开始应用", activate)
        self.assertNotIn("skipVerify", js)
        self.assertNotIn("can_skip", js)
        self.assertNotIn("pendingSkipActivateId", js)
        self.assertIn("当前选择", boundary)
        self.assertIn("当前选择 · 待一键开始应用", js)
        self.assertIn(">上次应用</span>", js)
        command = (ROOT / "desktop/src-tauri/src/commands/profiles.rs").read_text().split(
            "pub(crate) async fn set_active_profile(", 1
        )[1].split("\n}", 1)[0]
        self.assertNotIn("skip_verify", command)
        html = (ROOT / "desktop/src/index.html").read_text()
        self.assertNotIn("skipActivateBtn", html)

    def test_success_keeps_single_finally_busy_ownership_through_refresh(self):
        js = (ROOT / "desktop/src/runtime-controller.js").read_text()
        run_one_click = js.split("async function runOneClick(runtimeChoice)", 1)[1].split(
            "\nasync function importLocalSkill", 1
        )[0]
        success = run_one_click.split("// 透传后端据实回传的 msg", 1)[1].split(
            "} catch (e)", 1
        )[0]

        self.assertNotIn("setBusy(false)", success)
        finally_block = run_one_click.split("} finally {", 1)[1]
        self.assertLess(finally_block.index("setBusy(false)"), finally_block.index("refreshIfLoaded"))
        self.assertIn("await getSkillPage()?.refreshIfLoaded()", run_one_click)

    def test_committed_profile_mutations_distinguish_refresh_failure(self):
        js = (ROOT / "desktop/src/profile-controller.js").read_text()
        helper = js.split("async function loadConfigAfterCommit()", 1)[1].split(
            "// 列表优先展示", 1
        )[0]
        self.assertIn("loadConfig({ throwOnError: true })", helper)
        self.assertIn("error.configCommitted = true", helper)
        self.assertIn("已提交，但界面刷新失败", helper)
        self.assertEqual(js.count("await loadConfigAfterCommit()"), 6)
        for action in (
            "创建配置", "连接配置", "清除 key", "配置元数据", "删除配置", "当前选择",
        ):
            self.assertIn(f'committedRefreshMessage("{action}"', js)

    def test_skill_and_browser_warnings_preserve_runtime_success(self):
        session = sandbox_session_source()
        self.assertIn("configure_third_party_best_effort", session)
        self.assertIn("RegistrationStatus::Warning", session)
        self.assertIn("服务已就绪；自动打开失败。", session)
        self.assertIn('"status": "ok"', session)
        self.assertIn('"fallback_url": fallback_url', session)


if __name__ == "__main__":
    unittest.main()
