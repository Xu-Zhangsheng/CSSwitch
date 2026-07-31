import copy
import json
import re
import unittest
from pathlib import Path

from test.quality.validate_quality_metadata import Validator


ROOT = Path(__file__).resolve().parents[1]
INVENTORY_PATH = ROOT / "quality/runtime-mutation-inventory.v1.json"
SCHEMA_NAME = "runtime-mutation-inventory.v1.schema.json"
MUTATION_CLASSES = {
    "config-mutation",
    "runtime-mutation",
    "intent-mutation",
    "terminal-mutation",
    "host-bridge-mutation",
}
NATIVE_MUTATION_CLASSES = {
    "runtime-mutation",
    "terminal-mutation",
    "startup-mutation",
}
EXPECTED_NATIVE_HOOKS = {
    "setup_config_load",
    "setup_boot",
    "single_instance_boot",
    "exit_requested_cleanup",
    "exit_cleanup",
    "app_state_drop",
    "window_close_requested",
    "reopen",
}
EXPECTED_STATE_OWNERS = {
    "app.boot",
    "app.cleanup",
    "app.gateway",
    "app.history",
    "app.science",
    "authority.private",
    "codex.auth",
    "codex.supervisor",
    "config.access-gate",
    "config.binding",
    "config.desired",
    "config.transaction",
    "gateway.bridge-key",
    "lifecycle.global",
    "process.gateway",
    "process.science",
    "science.route",
    "skill.host",
}
EXPECTED_DURABLE_RECORDS = {
    "record.authority-snapshot-v1",
    "record.codex-auth-state-v1",
    "record.codex-oauth-v1",
    "record.codex-thinking-v1",
    "record.config-v4",
    "record.gateway-bridge-key-v1",
    "record.history-marker-v1",
    "record.pending-cleanup-v1",
    "record.route-configuration-v1",
    "record.runtime-binding-v1",
    "record.runtime-transaction-v1",
    "record.sandbox-ssh-stub-v2",
    "record.science-credential-v1",
    "record.science-receipt-v1",
    "record.science-ssh-bridge-v1",
    "record.skill-package-v1",
}
EXPECTED_OPERATIONS = {
    "op.codex-auth-cancel",
    "op.codex-auth-refresh",
    "op.codex-auth-start",
    "op.codex-downgrade",
    "op.codex-enable",
    "op.codex-ensure-profile",
    "op.codex-logout",
    "op.codex-network",
    "op.doctor-reconcile",
    "op.healthy-reopen",
    "op.history-restore",
    "op.install-local-skill",
    "op.interrupted-gateway-recovery",
    "op.native-exit",
    "op.one-click",
    "op.quit-command",
    "op.revoke-profile",
    "op.select-profile",
    "op.set-mode",
    "op.set-settings",
    "op.start-gateway-only",
    "op.startup-config-migration",
    "op.stop-all",
    "op.sync-preset",
    "op.update-connection",
}
EXPECTED_SURFACE_CONTRACT = {
    "app_version": ("read-only", "none"),
    "apply_profile_preset_sync": ("intent-mutation", "op.sync-preset"),
    "boot_attention": ("ui-one-shot", "none"),
    "boot_error": ("read-only", "none"),
    "clear_profile_key": ("runtime-mutation", "op.revoke-profile"),
    "codex_auth_cancel": ("runtime-mutation", "op.codex-auth-cancel"),
    "codex_auth_logout": ("runtime-mutation", "op.codex-logout"),
    "codex_auth_operation_status": ("read-only", "none"),
    "codex_auth_start": ("runtime-mutation", "op.codex-auth-start"),
    "codex_auth_status": ("transient-probe", "none"),
    "codex_downgrade_export_all": ("terminal-mutation", "op.codex-downgrade"),
    "codex_downgrade_preview": ("read-only", "none"),
    "codex_ensure_profile": ("intent-mutation", "op.codex-ensure-profile"),
    "create_profile": ("config-nonruntime", "none"),
    "delete_profile": ("runtime-mutation", "op.revoke-profile"),
    "fetch_models": ("transient-probe", "none"),
    "get_config": ("config-nonruntime", "none"),
    "install_local_skill_package": ("host-bridge-mutation", "op.install-local-skill"),
    "list_installed_skills": ("read-only", "none"),
    "list_templates": ("read-only", "none"),
    "one_click_login": ("runtime-mutation", "op.one-click"),
    "open_logs": ("external-side-effect", "none"),
    "open_official": ("external-side-effect", "none"),
    "open_release_page": ("external-side-effect", "none"),
    "open_science_download_page": ("external-side-effect", "none"),
    "open_url": ("external-side-effect", "none"),
    "preview_profile_preset_sync": ("read-only", "none"),
    "quit_app": ("terminal-mutation", "op.quit-command"),
    "report_bug": ("external-side-effect", "none"),
    "restore_history_choice": ("runtime-mutation", "op.history-restore"),
    "run_doctor": ("host-bridge-mutation", "op.doctor-reconcile"),
    "science_runtime_preflight": ("transient-probe", "none"),
    "set_active_profile": ("intent-mutation", "op.select-profile"),
    "set_codex_network": ("runtime-mutation", "op.codex-network"),
    "set_experimental_codex_enabled": ("runtime-mutation", "op.codex-enable"),
    "set_mode": ("runtime-mutation", "op.set-mode"),
    "set_settings": ("runtime-mutation", "op.set-settings"),
    "start_proxy": ("runtime-mutation", "op.start-gateway-only"),
    "status": ("read-only", "none"),
    "stop_all": ("runtime-mutation", "op.stop-all"),
    "update_profile_connection": ("intent-mutation", "op.update-connection"),
    "update_profile_metadata": ("config-nonruntime", "none"),
    "validate_profile_catalog_model": ("transient-probe", "none"),
}
EXPECTED_NATIVE_CONTRACT = {
    "app_state_drop": ("terminal-mutation", "op.native-exit"),
    "exit_cleanup": ("terminal-mutation", "op.native-exit"),
    "exit_requested_cleanup": ("terminal-mutation", "op.native-exit"),
    "reopen": ("ui-only", "none"),
    "setup_boot": ("runtime-mutation", "op.one-click"),
    "setup_config_load": ("startup-mutation", "op.startup-config-migration"),
    "single_instance_boot": ("runtime-mutation", "op.one-click"),
    "window_close_requested": ("ui-only", "none"),
}
TEST_MODULE_SOURCES = {
    "codex::tests": [ROOT / "desktop/src-tauri/src/commands/codex.rs"],
    "codex_auth::lifecycle_tests": [ROOT / "desktop/gateway/src/codex_auth/mod.rs"],
    "codex_auth::storage::tests": [ROOT / "desktop/gateway/src/codex_auth/storage.rs"],
    "codex_auth_supervisor::tests": [ROOT / "desktop/src-tauri/src/codex_auth_supervisor.rs"],
    "config::tests": [ROOT / "desktop/src-tauri/src/config.rs"],
    "diagnostics::tests": [ROOT / "desktop/src-tauri/src/commands/diagnostics.rs"],
    "lib::tests": [ROOT / "desktop/src-tauri/src/lib.rs"],
    "profile::tests": [ROOT / "desktop/src-tauri/src/runtime/profile.rs"],
    "profiles::tests": [ROOT / "desktop/src-tauri/src/commands/profiles.rs"],
    "proxy_lifecycle::tests": [ROOT / "desktop/src-tauri/src/runtime/proxy_lifecycle/tests.rs"],
    "runtime::tests": [ROOT / "desktop/src-tauri/src/commands/runtime/tests.rs"],
    "sandbox_session::transaction_tests": list(
        (ROOT / "desktop/src-tauri/src/runtime/sandbox_session/transaction_tests").rglob("*.rs")
    ),
    "skill_install::tests": [ROOT / "desktop/src-tauri/src/commands/skill_install.rs"],
}
RUST_SOURCE_PATHS = tuple((ROOT / "desktop/src-tauri/src").rglob("*.rs")) + tuple(
    (ROOT / "desktop/gateway/src").rglob("*.rs")
)


def load_inventory():
    return json.loads(INVENTORY_PATH.read_text(encoding="utf-8"))


def registered_commands():
    return set(registered_command_sources())


def registered_command_sources():
    source = (ROOT / "desktop/src-tauri/src/lib.rs").read_text(encoding="utf-8")
    block = source.split(".invoke_handler(tauri::generate_handler![", 1)[1].split("])\n", 1)[0]
    result = {}
    for module, name in re.findall(r"commands::([a-z_]+)::([a-z_]+)", block):
        result[name] = f"desktop/src-tauri/src/commands/{module}.rs"
    return result


def assert_unique_ids(testcase, rows, label):
    ids = [row["id"] if "id" in row else row["name"] for row in rows]
    testcase.assertEqual(len(ids), len(set(ids)), f"duplicate {label} identity")
    return set(ids)


def source_anchor_resolves(anchor):
    path = ROOT / anchor["path"]
    if not path.is_file():
        return False
    source = path.read_text(encoding="utf-8")
    locator = anchor["symbol"]
    return bool(locator) and locator in source


def bundled_caller_resolves(caller, command_name=None):
    path_text, separator, locator = caller.partition("::")
    path = ROOT / path_text
    if not path.is_file():
        return False
    source = path.read_text(encoding="utf-8")
    if separator:
        if not locator:
            return False
        if path.suffix == ".js":
            declaration = re.search(
                rf"^(?:export\s+)?(?:async\s+)?function\s+{re.escape(locator)}\s*\(",
                source,
                re.MULTILINE,
            )
            if declaration is None:
                return False
            next_declaration = re.search(
                r"^(?:export\s+)?(?:async\s+)?function\s+[A-Za-z_][A-Za-z0-9_]*\s*\(",
                source[declaration.end() :],
                re.MULTILINE,
            )
            function_end = (
                declaration.end() + next_declaration.start()
                if next_declaration is not None
                else len(source)
            )
            source = source[declaration.start() : function_end]
        if path.suffix != ".js" and locator not in source:
            return False
    if command_name is not None and path.suffix == ".js":
        return f'call("{command_name}"' in source
    return True


def test_identity_resolves(identity):
    module, separator, function = identity.rpartition("::")
    if not separator or module not in TEST_MODULE_SOURCES:
        return False
    pattern = re.compile(rf"\b(?:async\s+)?fn\s+{re.escape(function)}\s*\(")
    return any(
        path.is_file() and pattern.search(path.read_text(encoding="utf-8"))
        for path in TEST_MODULE_SOURCES[module]
    )


class RuntimeMutationInventoryTests(unittest.TestCase):
    def test_schema_and_fail_closed_references(self):
        inventory = load_inventory()
        validator = Validator(ROOT)
        validator.load_schemas()
        schema = validator.schemas[SCHEMA_NAME]
        validator.validate_instance(inventory, schema, schema, str(INVENTORY_PATH.relative_to(ROOT)))
        self.assertEqual(validator.errors, [], "\n".join(validator.errors))

        owner_ids = assert_unique_ids(self, inventory["state_owners"], "state owner")
        record_ids = assert_unique_ids(self, inventory["durable_records"], "durable record")
        operation_ids = assert_unique_ids(self, inventory["operations"], "operation")
        surface_ids = assert_unique_ids(self, inventory["registered_surface"], "registered command")
        native_ids = assert_unique_ids(self, inventory["native_hooks"], "native hook")

        self.assertEqual(owner_ids, EXPECTED_STATE_OWNERS)
        self.assertEqual(record_ids, EXPECTED_DURABLE_RECORDS)
        self.assertEqual(operation_ids, EXPECTED_OPERATIONS)
        self.assertEqual(surface_ids, registered_commands())
        self.assertEqual(surface_ids, set(EXPECTED_SURFACE_CONTRACT))
        self.assertEqual(native_ids, EXPECTED_NATIVE_HOOKS)
        self.assertEqual(native_ids, set(EXPECTED_NATIVE_CONTRACT))
        record_owner = {
            record["id"]: record["authority_owner"]
            for record in inventory["durable_records"]
        }
        for record in inventory["durable_records"]:
            self.assertIn(record["authority_owner"], owner_ids, record["id"])
            for provenance in record["writers"] + record["readers"]:
                self.assertTrue(
                    source_anchor_resolves(provenance),
                    f"{record['id']}: {provenance}",
                )
        for operation in inventory["operations"]:
            self.assertTrue(set(operation["state_owners"]).issubset(owner_ids), operation["id"])
            access = operation["durable_records"]
            referenced = set(access["reads"] + access["writes"] + access["clears"])
            self.assertTrue(referenced.issubset(record_ids), operation["id"])
            required_owners = {record_owner[record_id] for record_id in referenced}
            self.assertTrue(
                required_owners.issubset(operation["state_owners"]),
                operation["id"],
            )
        for entry in inventory["registered_surface"]:
            self.assertEqual(
                (entry["classification"], entry["operation_id"]),
                EXPECTED_SURFACE_CONTRACT[entry["name"]],
                entry["name"],
            )
            self.assertEqual(
                entry["source_anchor"],
                {
                    "path": registered_command_sources()[entry["name"]],
                    "symbol": entry["name"],
                },
                entry["name"],
            )
            if entry["classification"] in MUTATION_CLASSES:
                self.assertIn(entry["operation_id"], operation_ids, entry["name"])
            else:
                self.assertEqual(entry["operation_id"], "none", entry["name"])
            unresolved_callers = [
                caller
                for caller in entry["bundled_callers"]
                if not bundled_caller_resolves(caller, entry["name"])
            ]
            self.assertEqual(unresolved_callers, [], entry["name"])
        for hook in inventory["native_hooks"]:
            self.assertEqual(
                (hook["classification"], hook["operation_id"]),
                EXPECTED_NATIVE_CONTRACT[hook["name"]],
                hook["name"],
            )
            if hook["classification"] in NATIVE_MUTATION_CLASSES:
                self.assertIn(hook["operation_id"], operation_ids, hook["name"])
            else:
                self.assertEqual(hook["operation_id"], "none", hook["name"])

        exposed_operation_ids = {
            entry["operation_id"]
            for entry in inventory["registered_surface"] + inventory["native_hooks"]
            if entry["operation_id"] != "none"
        }
        internal_only = {
            "op.codex-auth-refresh",
            "op.healthy-reopen",
            "op.interrupted-gateway-recovery",
        }
        self.assertEqual(operation_ids, exposed_operation_ids | internal_only)

    def test_every_machine_anchor_resolves_in_current_source(self):
        inventory = load_inventory()
        anchors = []
        anchors.extend(item["source_anchor"] for item in inventory["state_owners"])
        anchors.extend(item["source_anchor"] for item in inventory["durable_records"])
        for record in inventory["durable_records"]:
            anchors.extend(record["writers"])
            anchors.extend(record["readers"])
        anchors.extend(item["source_anchor"] for item in inventory["registered_surface"])
        anchors.extend(item["source_anchor"] for item in inventory["native_hooks"])
        for operation in inventory["operations"]:
            anchors.extend(entry["source_anchor"] for entry in operation["entrypoints"])
            anchors.append(operation["compensation"]["source_anchor"])
            for entrypoint in operation["entrypoints"]:
                unresolved_callers = [
                    caller
                    for caller in entrypoint["bundled_callers"]
                    if not bundled_caller_resolves(
                        caller,
                        entrypoint["name"]
                        if entrypoint["kind"] == "tauri-command"
                        else None,
                    )
                ]
                self.assertEqual(unresolved_callers, [], operation["id"])
        unresolved = [anchor for anchor in anchors if not source_anchor_resolves(anchor)]
        self.assertEqual(unresolved, [])

    def test_r0_scope_and_critical_failure_boundaries_are_explicit(self):
        inventory = load_inventory()
        operations = {item["id"]: item for item in inventory["operations"]}
        surface = {item["name"]: item for item in inventory["registered_surface"]}

        self.assertEqual(
            {
                name: (surface[name]["classification"], surface[name]["operation_id"])
                for name in (
                    "boot_attention",
                    "create_profile",
                    "get_config",
                    "update_profile_metadata",
                )
            },
            {
                "boot_attention": ("ui-one-shot", "none"),
                "create_profile": ("config-nonruntime", "none"),
                "get_config": ("config-nonruntime", "none"),
                "update_profile_metadata": ("config-nonruntime", "none"),
            },
        )

        one_click = operations["op.one-click"]
        snapshot_index = one_click["ordered_effects"].index("capture authority snapshot")
        journal_index = one_click["ordered_effects"].index(
            "attempt first durable runtime-transaction journal write"
        )
        self.assertEqual(journal_index, snapshot_index + 1)
        one_click_failures = {item["id"]: item for item in one_click["failure_points"]}
        self.assertIn("one-click.post-stop-pre-snapshot", one_click_failures)
        self.assertEqual(
            one_click_failures["one-click.post-snapshot-pre-journal"]["after_effects"],
            ["authority snapshot captured", "runtime transaction journal not yet committed"],
        )

        self.assertEqual(
            operations["op.select-profile"]["durable_records"]["clears"],
            [],
        )
        self.assertEqual(
            operations["op.revoke-profile"]["durable_records"]["clears"],
            ["record.runtime-binding-v1"],
        )

        downgrade = operations["op.codex-downgrade"]
        export_index = downgrade["ordered_effects"].index(
            "atomically publish the user-selected export"
        )
        backup_index = downgrade["ordered_effects"].index(
            "write the rolling config backup"
        )
        v2_index = downgrade["ordered_effects"].index(
            "attempt v2 config publication and latch committed or uncertain outcomes"
        )
        self.assertLess(export_index, backup_index)
        self.assertLess(backup_index, v2_index)
        downgrade_failures = {item["id"]: item for item in downgrade["failure_points"]}
        self.assertEqual(
            downgrade_failures["codex-downgrade.post-export-pre-v2"]["compensation"],
            "no export rollback and no runtime restart",
        )
        self.assertIn(
            "config::tests::completed_export_survives_later_config_precommit_failure",
            downgrade["characterization_tests"],
        )

        migration = operations["op.startup-config-migration"]
        backup_index = migration["ordered_effects"].index(
            "publish one or more non-overwriting version backups for legacy input"
        )
        v4_index = migration["ordered_effects"].index(
            "atomically publish the normalized current v4 config"
        )
        self.assertLess(backup_index, v4_index)

    def test_completion_state_cannot_claim_missing_characterization_tests(self):
        inventory = load_inventory()
        required = {
            test_id
            for operation in inventory["operations"]
            for test_id in operation["characterization_tests"]
        }
        function_locations = {}
        declaration = re.compile(r"\b(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(")
        for path in RUST_SOURCE_PATHS:
            for function in declaration.findall(path.read_text(encoding="utf-8")):
                function_locations.setdefault(function, []).append(path)
        for test_id in required:
            module, separator, function = test_id.rpartition("::")
            self.assertTrue(separator, test_id)
            self.assertIn(module, TEST_MODULE_SOURCES, test_id)
            if function_locations.get(function):
                self.assertTrue(test_identity_resolves(test_id), test_id)
        missing = sorted(test_id for test_id in required if not test_identity_resolves(test_id))
        status = inventory["scope"]["characterization_status"]
        if status == "complete":
            self.assertEqual(missing, [])
        else:
            self.assertEqual(status, "requirements-open")
            self.assertTrue(missing, "open status must not hide a fully resolved inventory")

    def test_integrity_checks_reject_representative_bad_inventory(self):
        inventory = load_inventory()
        bad = copy.deepcopy(inventory)
        bad["operations"][1]["id"] = bad["operations"][0]["id"]
        with self.assertRaises(AssertionError):
            assert_unique_ids(self, bad["operations"], "operation")

        bad = copy.deepcopy(inventory)
        bad["registered_surface"].pop()
        self.assertNotEqual(
            {entry["name"] for entry in bad["registered_surface"]},
            registered_commands(),
        )

        bad = copy.deepcopy(inventory)
        bad["operations"][0]["state_owners"].append("owner.does-not-exist")
        owner_ids = {item["id"] for item in bad["state_owners"]}
        self.assertFalse(set(bad["operations"][0]["state_owners"]).issubset(owner_ids))

        bad = copy.deepcopy(inventory)
        bad["state_owners"][0]["source_anchor"]["path"] = "missing.rs"
        self.assertFalse(source_anchor_resolves(bad["state_owners"][0]["source_anchor"]))

        bad_anchor = {
            "path": "desktop/src-tauri/src/commands/diagnostics.rs",
            "symbol": "run_doctor open_logs",
        }
        self.assertFalse(source_anchor_resolves(bad_anchor))

        bad = copy.deepcopy(inventory)
        bad["durable_records"][0]["writers"][0]["path"] = "missing-writer.rs"
        self.assertFalse(
            source_anchor_resolves(bad["durable_records"][0]["writers"][0])
        )

        self.assertFalse(
            bundled_caller_resolves(
                "desktop/src/runtime-controller.js::recoverHistoryChoice",
                "restore_history_choice",
            )
        )

        self.assertFalse(
            bundled_caller_resolves(
                "desktop/src/runtime-controller.js::runOneClick",
                "restore_history_choice",
            )
        )

        bad = copy.deepcopy(inventory)
        set_mode = next(entry for entry in bad["registered_surface"] if entry["name"] == "set_mode")
        set_mode["classification"] = "read-only"
        set_mode["operation_id"] = "none"
        self.assertNotEqual(
            (set_mode["classification"], set_mode["operation_id"]),
            EXPECTED_SURFACE_CONTRACT["set_mode"],
        )

        self.assertFalse(
            test_identity_resolves(
                "does_not_exist::tests::login_finalization_requires_profile_ready_before_succeeded"
            )
        )


if __name__ == "__main__":
    unittest.main()
