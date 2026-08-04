import copy
import json
import re
import unittest
from pathlib import Path

from test.quality.validate_quality_metadata import Validator


ROOT = Path(__file__).resolve().parents[1]
INVENTORY_PATH = ROOT / "quality/runtime-mutation-inventory.v1.json"
SOURCE_IDENTITIES_PATH = (
    ROOT / "test/quality/fixtures/source_gate/expected_test_ids.v1.json"
)
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
    "codex.catalog-cache",
    "codex.supervisor",
    "config.access-gate",
    "config.binding",
    "config.desired",
    "config.migration-backups",
    "config.transaction",
    "gateway.bridge-key",
    "lifecycle.global",
    "process.gateway",
    "process.science",
    "science.route",
    "skill.bridge-mailbox",
    "skill.host",
}
EXPECTED_DURABLE_RECORDS = {
    "record.authority-snapshot-v1",
    "record.codex-auth-state-v1",
    "record.codex-model-cache-v3",
    "record.codex-oauth-v1",
    "record.codex-thinking-v1",
    "record.config-v4",
    "record.config-migration-backups-v1",
    "record.gateway-bridge-key-v1",
    "record.history-marker-v1",
    "record.pending-cleanup-v1",
    "record.operon-skill-attachment-v1",
    "record.route-configuration-v1",
    "record.runtime-binding-v1",
    "record.runtime-transaction-v1",
    "record.runtime-transaction-v2",
    "record.sandbox-ssh-stub-v2",
    "record.science-credential-v1",
    "record.science-receipt-v1",
    "record.science-ssh-bridge-v1",
    "record.skill-package-v1",
    "record.skill-bridge-mailbox-v1",
}
EXPECTED_OPERATIONS = {
    "op.codex-auth-cancel",
    "op.codex-auth-refresh",
    "op.codex-auth-start",
    "op.codex-downgrade",
    "op.codex-enable",
    "op.codex-ensure-profile",
    "op.codex-catalog-mutation",
    "op.codex-logout",
    "op.codex-network",
    "op.skill-route-repair",
    "op.healthy-reopen",
    "op.history-restore",
    "op.install-local-skill",
    "op.gateway-skill-bridge",
    "op.interrupted-gateway-recovery",
    "op.native-exit",
    "op.one-click",
    "op.quit-command",
    "op.revoke-profile",
    "op.select-profile",
    "op.set-mode",
    "op.set-settings",
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
    "fetch_models": ("runtime-mutation", "op.codex-catalog-mutation"),
    "finalize_consumer_state": ("read-only", "none"),
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
    "repair_skill_route": ("host-bridge-mutation", "op.skill-route-repair"),
    "run_doctor_read_only": ("read-only", "none"),
    "science_runtime_preflight": ("transient-probe", "none"),
    "set_active_profile": ("intent-mutation", "op.select-profile"),
    "set_codex_network": ("runtime-mutation", "op.codex-network"),
    "set_experimental_codex_enabled": ("runtime-mutation", "op.codex-enable"),
    "set_mode": ("runtime-mutation", "op.set-mode"),
    "set_settings": ("runtime-mutation", "op.set-settings"),
    "status": ("read-only", "none"),
    "stop_all": ("runtime-mutation", "op.stop-all"),
    "update_profile_connection": ("intent-mutation", "op.update-connection"),
    "update_profile_metadata": ("config-nonruntime", "none"),
    "validate_profile_catalog_model": ("transient-probe", "none"),
}
EXPECTED_IMPLICIT_OPERATION_CONTRACT = {
    "codex_auth_status": {"op.startup-config-migration"},
    "codex_downgrade_preview": {"op.startup-config-migration"},
    "create_profile": {"op.startup-config-migration"},
    "get_config": {"op.startup-config-migration"},
    "list_templates": {"op.startup-config-migration"},
    "preview_profile_preset_sync": {"op.startup-config-migration"},
    "science_runtime_preflight": {"op.startup-config-migration"},
    "status": {"op.startup-config-migration"},
    "update_profile_metadata": {"op.startup-config-migration"},
    "validate_profile_catalog_model": {"op.startup-config-migration"},
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
def load_inventory():
    return json.loads(INVENTORY_PATH.read_text(encoding="utf-8"))


def load_source_identities():
    return json.loads(SOURCE_IDENTITIES_PATH.read_text(encoding="utf-8"))["suites"]


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


def source_test_identities():
    executable = set()
    ignored = set()
    skipped = set()
    for suite in load_source_identities().values():
        discovered = set(suite["discovered_test_ids"])
        suite_ignored = set(suite["approved_ignored_test_ids"])
        suite_skipped = set(suite["approved_skipped_test_ids"])
        executable.update(discovered - suite_ignored - suite_skipped)
        ignored.update(suite_ignored)
        skipped.update(suite_skipped)
    return executable, ignored, skipped


def test_identity_resolves(identity):
    executable, _, _ = source_test_identities()
    return identity in executable


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
            expected_implicit = (
                {"op.startup-config-migration"}
                if operation["id"] != "op.startup-config-migration"
                and "record.config-v4" in access["reads"]
                else set()
            )
            self.assertEqual(
                set(operation.get("implicit_operation_ids", [])),
                expected_implicit,
                operation["id"],
            )
            self.assertTrue(expected_implicit.issubset(operation_ids), operation["id"])
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
            implicit_operation_ids = set(entry.get("implicit_operation_ids", []))
            self.assertEqual(
                implicit_operation_ids,
                EXPECTED_IMPLICIT_OPERATION_CONTRACT.get(entry["name"], set()),
                entry["name"],
            )
            self.assertTrue(implicit_operation_ids.issubset(operation_ids), entry["name"])
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
        exposed_operation_ids.update(
            operation_id
            for entry in inventory["registered_surface"]
            for operation_id in entry.get("implicit_operation_ids", [])
        )
        internal_only = {
            "op.codex-auth-refresh",
            "op.gateway-skill-bridge",
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
        records = {item["id"]: item for item in inventory["durable_records"]}
        surface = {item["name"]: item for item in inventory["registered_surface"]}

        config_writers = {
            (item["path"], item["symbol"])
            for item in records["record.config-v4"]["writers"]
        }
        self.assertTrue(
            {
                ("desktop/src-tauri/src/config.rs", "pub fn load_from("),
                ("desktop/src-tauri/src/config.rs", "fn commit_migrated_config("),
            }.issubset(config_writers)
        )
        self.assertTrue(
            {
                "pub(crate) fn commit_package(",
                "fn read_and_validate_marker(",
                "fn scan_installed_payload(",
            }.issubset(
                {item["symbol"] for item in records["record.skill-package-v1"]["readers"]}
            )
        )

        cache_record = records["record.codex-model-cache-v3"]
        self.assertEqual(cache_record["authority_owner"], "codex.catalog-cache")
        self.assertTrue(
            {
                "fn commit_cache(",
                "fn commit_cache_epoch(",
                "fn persist_invalidation(",
                "fn remove_cache(",
            }.issubset({item["symbol"] for item in cache_record["writers"]})
        )

        bridge_record = records["record.skill-bridge-mailbox-v1"]
        self.assertEqual(bridge_record["authority_owner"], "skill.bridge-mailbox")
        self.assertTrue(
            {
                "fn host_access_request(",
                "pub(super) fn start_skill_install_bridge(",
                "pub(super) fn write_bridge_status(",
                "pub(super) fn finalize_bridge_processing(",
            }.issubset({item["symbol"] for item in bridge_record["writers"]})
        )
        backup_record = records["record.config-migration-backups-v1"]
        self.assertEqual(backup_record["authority_owner"], "config.migration-backups")
        self.assertEqual(
            {item["symbol"] for item in backup_record["writers"]},
            {"fn write_versioned_backup_bytes_in("},
        )

        operon_record = records["record.operon-skill-attachment-v1"]
        self.assertEqual(operon_record["authority_owner"], "skill.host")
        self.assertEqual(
            {item["symbol"] for item in operon_record["writers"]},
            {"pub fn attach_skill(", "pub fn update_agent_skills("},
        )
        self.assertTrue(
            {
                "pub fn install_github_package_with_progress(",
                "pub(crate) fn install_validated_bundle(",
                "pub fn quarantine_bundle(",
                "fn uninstall_external_skill(",
            }.issubset(
                {item["symbol"] for item in records["record.skill-package-v1"]["writers"]}
            )
        )

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
        prior_intent_index = one_click["ordered_effects"].index(
            "persist prior Science stop intent"
        )
        stop_outcome_index = one_click["ordered_effects"].index(
            "stop prior managed Science and persist typed outcome"
        )
        snapshot_index = one_click["ordered_effects"].index(
            "capture authority snapshot and attach its ticket"
        )
        finalize_index = one_click["ordered_effects"].index("persist finalize intent")
        cleanup_only_index = one_click["ordered_effects"].index(
            "convert authority manifest to cleanup-only"
        )
        binding_index = one_click["ordered_effects"].index(
            "atomically commit binding and clear journal"
        )
        self.assertLess(prior_intent_index, stop_outcome_index)
        self.assertLess(stop_outcome_index, snapshot_index)
        self.assertLess(finalize_index, cleanup_only_index)
        self.assertLess(cleanup_only_index, binding_index)
        one_click_failures = {item["id"]: item for item in one_click["failure_points"]}
        self.assertIn("one-click.post-stop-pre-snapshot", one_click_failures)
        self.assertEqual(
            one_click_failures["one-click.post-snapshot-pre-journal"]["after_effects"],
            [
                "authority snapshot captured",
                "snapshot ticket not yet attached to the V2 record",
            ],
        )
        self.assertIn(
            "same-process PreJournalAbort",
            one_click_failures["one-click.post-snapshot-pre-journal"]["observed_outcome"],
        )
        self.assertIn("one-click.success-finalize", one_click_failures)
        self.assertFalse(any("F5" in gap for gap in one_click["known_gaps"]))
        self.assertTrue(
            {
                "desktop/src-tauri/Cargo.toml::lib::commands::runtime::tests::h3_finalize_failures_preserve_replayable_intent",
                "desktop/src-tauri/Cargo.toml::lib::commands::runtime::tests::o0_gateway_terminal_handoff_reaches_production_one_click",
                "desktop/src-tauri/Cargo.toml::lib::runtime::sandbox_session::transaction_tests::cleanup_recovery::success_finalize_replays_both_active_recovery_and_cleanup_only_crash_windows",
                "desktop/src-tauri/Cargo.toml::lib::runtime::sandbox_session::transaction_tests::runtime_journal::gateway_terminal_handoff_prior_stop_and_finalize_are_exact_replayable_transitions",
            }.issubset(one_click["characterization_tests"])
        )
        self.assertIn(
            "record.runtime-transaction-v2", one_click["durable_records"]["writes"]
        )
        self.assertNotIn(
            "record.runtime-transaction-v1", one_click["durable_records"]["writes"]
        )

        self.assertEqual(
            operations["op.select-profile"]["durable_records"]["clears"],
            [],
        )
        self.assertEqual(
            operations["op.revoke-profile"]["durable_records"]["clears"],
            ["record.runtime-binding-v1"],
        )

        fetch_models = operations["op.codex-catalog-mutation"]
        self.assertEqual(
            set(fetch_models["state_owners"]),
            {"codex.auth", "codex.catalog-cache", "config.desired", "process.gateway"},
        )
        self.assertIn(
            "record.codex-model-cache-v3",
            fetch_models["durable_records"]["writes"],
        )
        self.assertTrue(
            {
                "record.codex-auth-state-v1",
                "record.codex-oauth-v1",
                "record.codex-thinking-v1",
            }.issubset(fetch_models["durable_records"]["writes"])
        )
        self.assertEqual(
            {item["name"] for item in fetch_models["entrypoints"]},
            {
                "fetch_models",
                "formal_gateway_models_route",
                "formal_inference_catalog_invalidation",
            },
        )

        gateway_bridge = operations["op.gateway-skill-bridge"]
        self.assertEqual(
            {item["name"] for item in gateway_bridge["entrypoints"]},
            {"gateway_skill_bridge_host", "skill_bridge_mcp_tool"},
        )
        self.assertEqual(
            set(gateway_bridge["durable_records"]["writes"]),
            {
                "record.operon-skill-attachment-v1",
                "record.skill-bridge-mailbox-v1",
                "record.skill-package-v1",
            },
        )
        self.assertIn(
            "record.operon-skill-attachment-v1",
            operations["op.install-local-skill"]["durable_records"]["writes"],
        )
        self.assertIn(
            "record.skill-package-v1",
            operations["op.install-local-skill"]["durable_records"]["reads"],
        )
        self.assertEqual(
            gateway_bridge["durable_records"]["clears"],
            ["record.skill-bridge-mailbox-v1"],
        )

        self.assertEqual(
            {item["name"] for item in operations["op.codex-auth-refresh"]["entrypoints"]},
            {
                "codex_auth_refresh",
                "expired_token_auth_refresh",
                "inference_401_auth_refresh",
                "models_401_auth_refresh",
            },
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
            "desktop/src-tauri/Cargo.toml::lib::config::tests::completed_export_survives_later_config_precommit_failure",
            downgrade["characterization_tests"],
        )

        migration = operations["op.startup-config-migration"]
        self.assertEqual(
            {item["name"] for item in migration["entrypoints"]},
            {
                "config_load_migration",
                "startup_config_load",
                "boot_coordinator_config_load",
                "single_instance_boot_reentry",
            },
        )
        self.assertEqual(
            set(migration["durable_records"]["writes"]),
            {"record.config-migration-backups-v1", "record.config-v4"},
        )
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
        executable, ignored, skipped = source_test_identities()
        missing = sorted(required - executable)
        status = inventory["scope"]["characterization_status"]
        review_status = inventory["scope"]["completion_review_status"]
        if status == "complete":
            self.assertEqual(missing, [])
            self.assertEqual(sorted(required & ignored), [])
            self.assertEqual(sorted(required & skipped), [])
            self.assertEqual(review_status, "complete")
        else:
            self.assertEqual(status, "requirements-open")
            self.assertEqual(review_status, "pending")

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
            "symbol": "run_doctor_read_only open_logs",
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
                "desktop/src-tauri/Cargo.toml::lib::commands::codex::tests::does_not_exist"
            )
        )
        _, ignored, _ = source_test_identities()
        self.assertTrue(ignored)
        self.assertFalse(test_identity_resolves(next(iter(ignored))))


if __name__ == "__main__":
    unittest.main()
