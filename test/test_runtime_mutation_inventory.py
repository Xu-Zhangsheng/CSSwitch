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
    "science_runtime_update_scheduler",
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
    "config.codex-disable-receipt",
    "config.mutation-operation",
    "config.rolling-backup",
    "config.desired",
    "config.migration-backups",
    "config.transaction",
    "config.writer-fence",
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
    "record.codex-disable-operation-v1",
    "record.config-mutation-operation-v1",
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
    "record.runtime-compensation-v1",
    "record.runtime-compensation-v2",
    "record.runtime-transaction-v1",
    "record.runtime-transaction-v2",
    "record.sandbox-ssh-stub-v2",
    "record.science-adoption-v1",
    "record.science-credential-v1",
    "record.science-receipt-v1",
    "record.science-runtime-selection-v1",
    "record.science-runtime-snapshot-v1",
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
    "op.science-runtime-preflight",
    "op.science-runtime-update-action",
    "op.science-runtime-update-check",
    "op.select-profile",
    "op.set-mode",
    "op.set-settings",
    "op.startup-config-migration",
    "op.stop-all",
    "op.update-connection",
}
EXPECTED_SURFACE_CONTRACT = {
    "acknowledge_pending_notice": ("config-nonruntime", "none"),
    "app_version": ("read-only", "none"),
    "boot_snapshot": ("read-only", "none"),
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
    "get_config": ("read-only", "none"),
    "install_local_skill_package": ("host-bridge-mutation", "op.install-local-skill"),
    "list_installed_skills": ("read-only", "none"),
    "one_click_login": ("runtime-mutation", "op.one-click"),
    "open_logs": ("external-side-effect", "none"),
    "open_official": ("external-side-effect", "none"),
    "open_release_page": ("external-side-effect", "none"),
    "open_science_download_page": ("external-side-effect", "none"),
    "open_url": ("external-side-effect", "none"),
    "quit_app": ("terminal-mutation", "op.quit-command"),
    "report_bug": ("external-side-effect", "none"),
    "restore_history_choice": ("runtime-mutation", "op.history-restore"),
    "repair_skill_route": ("host-bridge-mutation", "op.skill-route-repair"),
    "run_doctor_read_only": ("read-only", "none"),
    "science_runtime_preflight": ("runtime-mutation", "op.science-runtime-preflight"),
    "science_runtime_update_action": ("intent-mutation", "op.science-runtime-update-action"),
    "science_runtime_update_status": ("read-only", "none"),
    "set_active_profile": ("intent-mutation", "op.select-profile"),
    "set_codex_network": ("runtime-mutation", "op.codex-network"),
    "set_experimental_codex_enabled": ("runtime-mutation", "op.codex-enable"),
    "set_mode": ("runtime-mutation", "op.set-mode"),
    "set_settings": ("runtime-mutation", "op.set-settings"),
    "status": ("read-only", "none"),
    "stop_all": ("runtime-mutation", "op.stop-all"),
    "update_profile_connection": ("intent-mutation", "op.update-connection"),
    "update_profile_metadata": ("config-nonruntime", "none"),
}
EXPECTED_IMPLICIT_OPERATION_CONTRACT = {
    "acknowledge_pending_notice": {"op.startup-config-migration"},
    "codex_auth_status": {"op.startup-config-migration"},
    "codex_downgrade_preview": {"op.startup-config-migration"},
    "create_profile": {"op.startup-config-migration"},
    "science_runtime_preflight": {"op.startup-config-migration"},
    "status": {"op.startup-config-migration"},
    "update_profile_metadata": {"op.startup-config-migration"},
}
EXPECTED_OPERATION_IMPLICIT_CONTRACT = {
    "op.history-restore": {"op.one-click"},
}
EXPECTED_NATIVE_CONTRACT = {
    "app_state_drop": ("terminal-mutation", "op.native-exit"),
    "exit_cleanup": ("terminal-mutation", "op.native-exit"),
    "exit_requested_cleanup": ("terminal-mutation", "op.native-exit"),
    "reopen": ("ui-only", "none"),
    "science_runtime_update_scheduler": ("runtime-mutation", "op.science-runtime-update-check"),
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


def assert_science_inventory_contract(testcase, inventory):
    owners = {item["id"]: item for item in inventory["state_owners"]}
    records = {item["id"]: item for item in inventory["durable_records"]}
    operations = {item["id"]: item for item in inventory["operations"]}

    testcase.assertTrue(
        {
            "science_adoption_ledger",
            "science_runtime_selection",
            "science_runtime_snapshot",
        }.issubset(
            set(owners["authority.private"]["fields"])
        )
    )

    adoption = records["record.science-adoption-v1"]
    testcase.assertEqual(adoption["authority_owner"], "authority.private")
    testcase.assertEqual(
        set(adoption["fields"]),
        {
            "attempt_id",
            "attempts",
            "candidate",
            "decision",
            "embedded_identity",
            "milestone",
            "normalized_diff",
            "observed_at_ms",
            "predecessor",
            "rejected_source",
            "rejection_code",
            "schema_version",
            "sha256",
            "size",
            "snapshot_id",
            "source",
            "version",
        },
    )
    testcase.assertEqual(
        {item["symbol"] for item in adoption["writers"]},
        {
            "bind_selected_science_runtime_attempt",
            "ensure_selected_science_runtime_attempt",
            "mark_science_runtime_adoption_finalized",
            "mark_science_runtime_adoption_launch_committed",
            "reconcile_current_science_runtime_adoption",
            "record_deferred_science_runtime_observation",
            "record_rejected_science_runtime_attempt",
        },
    )
    testcase.assertEqual(
        {item["symbol"] for item in adoption["readers"]},
        {
            "bind_selected_science_runtime_attempt",
            "reconcile_current_science_runtime_adoption",
        },
    )
    testcase.assertEqual(
        adoption["source_anchor"],
        {
            "path": "desktop/src-tauri/src/runtime/science/adoption.rs",
            "symbol": "ScienceAdoptionLedger",
        },
    )

    snapshot = records["record.science-runtime-snapshot-v1"]
    testcase.assertEqual(snapshot["authority_owner"], "authority.private")
    testcase.assertEqual(
        set(snapshot["fields"]),
        {"content_sha256", "executable_bytes", "executable_mode"},
    )
    testcase.assertEqual(
        {item["symbol"] for item in snapshot["writers"]},
        {"fn snapshot_science_executable("},
    )
    testcase.assertEqual(
        {item["symbol"] for item in snapshot["readers"]},
        {
            "fn official_updated_snapshot_for_home(",
            "fn official_updated_snapshot_from_process_paths(",
            "fn runtime_identity(",
            "pub(crate) fn runtime_identity_from_durable_parts(",
            "runtime_from_pinned_selection",
        },
    )

    selection = records["record.science-runtime-selection-v1"]
    testcase.assertEqual(selection["authority_owner"], "authority.private")
    testcase.assertEqual(
        set(selection["fields"]),
        {
            "active",
            "dismissed_sha256s",
            "last_checked_at_ms",
            "pending",
            "schema_version",
            "sha256",
            "size",
            "source",
            "update_check_id",
            "version",
        },
    )
    testcase.assertEqual(
        {item["symbol"] for item in selection["writers"]},
        {
            "mutate_science_runtime_selection",
            "resolve_active_or_bootstrap_science_runtime",
            "science_runtime_update_action",
            "check_science_runtime_update",
        },
    )
    testcase.assertEqual(
        {item["symbol"] for item in selection["readers"]},
        {
            "active_science_runtime",
            "resolve_active_or_bootstrap_science_runtime",
            "science_runtime_update_status",
        },
    )
    testcase.assertEqual(
        snapshot["source_anchor"],
        {
            "path": "desktop/src-tauri/src/runtime/science/contracts.rs",
            "symbol": "const OFFICIAL_UPDATED_SNAPSHOT_DIR",
        },
    )

    preflight = operations["op.science-runtime-preflight"]
    testcase.assertEqual(preflight["mutation_kind"], "runtime")
    testcase.assertEqual(preflight["serialization"], "none")
    testcase.assertEqual(
        set(preflight["state_owners"]),
        {
            "app.science",
            "authority.private",
            "config.binding",
            "config.desired",
            "process.science",
        },
    )
    testcase.assertEqual(
        {key: set(value) for key, value in preflight["durable_records"].items()},
        {
            "reads": {
                "record.config-v4",
                "record.runtime-binding-v1",
                "record.science-adoption-v1",
                "record.science-receipt-v1",
                "record.science-runtime-selection-v1",
                "record.science-runtime-snapshot-v1",
            },
            "writes": {
                "record.science-adoption-v1",
                "record.science-runtime-selection-v1",
                "record.science-runtime-snapshot-v1",
            },
            "clears": set(),
        },
    )

    update_check = operations["op.science-runtime-update-check"]
    testcase.assertEqual(update_check["serialization"], "lifecycle-observation-only")
    testcase.assertIn("lifecycle.global", update_check["state_owners"])
    testcase.assertIn(
        "record.science-receipt-v1", update_check["durable_records"]["reads"]
    )
    update_effects = " ".join(update_check["ordered_effects"])
    testcase.assertIn("generation plus full-owner CAS", update_effects)
    testcase.assertIn("same exact pending choice", update_effects)
    testcase.assertIn("typed managed-health proof", update_effects)
    testcase.assertIn(
        "science-runtime-update-check.managed-observation",
        {failure["id"] for failure in update_check["failure_points"]},
    )
    testcase.assertTrue(
        {
            "desktop/src-tauri/Cargo.toml::lib::runtime::science::tests::background_science_update_check_is_due_once_per_day",
            "desktop/src-tauri/Cargo.toml::lib::runtime::science::tests::fresh_restart_rejects_listener_without_managed_launch_identity",
            "desktop/src-tauri/Cargo.toml::lib::commands::runtime::tests::r0_one_click_cold_start_commits_runtime_and_receipts",
        }.issubset(update_check["characterization_tests"])
    )
    testcase.assertEqual(
        preflight["entrypoints"],
        [
            {
                "kind": "tauri-command",
                "name": "science_runtime_preflight",
                "source_anchor": {
                    "path": "desktop/src-tauri/src/commands/runtime.rs",
                    "symbol": "science_runtime_preflight",
                },
                "bundled_callers": ["desktop/src/runtime-controller.js"],
            }
        ],
    )
    testcase.assertIn(
        "for stopped startup resolve and validate only the fixed active content-addressed snapshot",
        preflight["ordered_effects"],
    )
    testcase.assertIn(
        "only when no selection exists, perform one bootstrap discovery, content-address the updater or installed-App executable, and atomically publish active selection",
        preflight["ordered_effects"],
    )
    testcase.assertEqual(
        {item["id"] for item in preflight["failure_points"]},
        {
            "science-runtime-preflight.adoption-record",
            "science-runtime-preflight.runtime-snapshot",
        },
    )
    testcase.assertEqual(preflight["compensation"]["kind"], "partial")
    testcase.assertEqual(
        preflight["compensation"]["source_anchor"],
        {
            "path": "desktop/src-tauri/src/runtime/science/executable.rs",
            "symbol": "science_runtime_preflight",
        },
    )
    testcase.assertEqual(
        preflight["compensation"]["contract"],
        "The command never starts, stops, or replaces Science. Normal startup validates only fixed active state; one-time bootstrap temporary names are removed best effort, verified snapshots are retained, and selection/adoption writes share the private cross-process writer lock.",
    )

    update_action = operations["op.science-runtime-update-action"]
    testcase.assertEqual(
        set(update_action["durable_records"]["reads"]),
        {
            "record.science-runtime-selection-v1",
            "record.science-runtime-snapshot-v1",
        },
    )
    testcase.assertEqual(
        set(update_action["durable_records"]["writes"]),
        {"record.science-runtime-selection-v1"},
    )

    one_click = operations["op.one-click"]
    testcase.assertIn(
        "record.science-runtime-snapshot-v1",
        one_click["durable_records"]["reads"],
    )
    testcase.assertIn(
        "record.science-runtime-selection-v1",
        one_click["durable_records"]["reads"],
    )
    testcase.assertIn(
        "record.science-runtime-snapshot-v1",
        one_click["durable_records"]["writes"],
    )
    testcase.assertIn(
        "record.science-runtime-selection-v1",
        one_click["durable_records"]["writes"],
    )
    testcase.assertIn(
        "recapture entry facts, resolve only the fixed active Science snapshot or perform one initial bootstrap when selection is absent, and purely decide one recovery step",
        one_click["ordered_effects"],
    )
    testcase.assertIn(
        "one-click.runtime-snapshot",
        {item["id"] for item in one_click["failure_points"]},
    )
    testcase.assertIn(
        "Verified content-addressed Science runtime snapshots are immutable selection evidence outside rollback and are retained.",
        one_click["compensation"]["contract"],
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
            expected_implicit = (
                {"op.startup-config-migration"}
                if operation["id"] != "op.startup-config-migration"
                and "record.config-v4" in access["reads"]
                else set()
            )
            expected_implicit.update(
                EXPECTED_OPERATION_IMPLICIT_CONTRACT.get(operation["id"], set())
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
        assert_science_inventory_contract(self, inventory)

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
                    "acknowledge_pending_notice",
                    "boot_snapshot",
                    "create_profile",
                    "get_config",
                    "update_profile_metadata",
                )
            },
            {
                "acknowledge_pending_notice": ("config-nonruntime", "none"),
                "boot_snapshot": ("read-only", "none"),
                "create_profile": ("config-nonruntime", "none"),
                "get_config": ("read-only", "none"),
                "update_profile_metadata": ("config-nonruntime", "none"),
            },
        )

        one_click = operations["op.one-click"]
        prior_intent_index = one_click["ordered_effects"].index(
            "persist prior Science stop intent"
        )
        stop_outcome_index = one_click["ordered_effects"].index(
            "claim the exact prior Science generation and full process-local owner under AppState, execute the existing stop/TERM/KILL/wait policy outside AppState, publish only by generation plus full-owner CAS, and persist the typed outcome"
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
        self.assertEqual(
            one_click_failures["one-click.prior-stop-owner-cas"]["compensation"],
            "manual recovery from the retained durable intent; stale publication cannot clear or restart replacement Science",
        )
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
        self.assertIn(
            "desktop/src-tauri/Cargo.toml::lib::runtime::sandbox_session::one_click::cold::tests::cold_prior_science_stop_wait_releases_read_model_and_stale_result_preserves_replacement",
            one_click["characterization_tests"],
        )
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
            ["record.config-mutation-operation-v1", "record.runtime-binding-v1"],
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
        claim_index = downgrade["ordered_effects"].index(
            "bump generation and claim the exact process-local Science owner and typed stop request under AppState"
        )
        execute_index = downgrade["ordered_effects"].index(
            "release AppState while executing the existing Science stop policy"
        )
        publish_index = downgrade["ordered_effects"].index(
            "CAS the Science result against generation and complete owner identity, then always stop the tracked Gateway"
        )
        effect_index = downgrade["ordered_effects"].index(
            "only a current confirmed stop proceeds to the secure writer, which rechecks both journals"
        )
        self.assertLess(claim_index, execute_index)
        self.assertLess(execute_index, publish_index)
        self.assertLess(publish_index, effect_index)
        self.assertLess(effect_index, export_index)
        downgrade_failures = {item["id"]: item for item in downgrade["failure_points"]}
        self.assertEqual(
            downgrade_failures["codex-downgrade.owner-cas"]["compensation"],
            "no runtime restart; stale Science ownership cannot be cleared and downgrade effects do not begin",
        )
        self.assertEqual(
            downgrade_failures["codex-downgrade.post-export-pre-v2"]["compensation"],
            "no export rollback and no runtime restart",
        )
        self.assertIn(
            "commands::codex::tests::downgrade_cleanup_wait_releases_read_model_and_stale_result_preserves_replacement",
            downgrade["characterization_tests"],
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

    def test_p2a_codex_disable_receipt_contract_is_explicit(self):
        inventory = load_inventory()
        records = {item["id"]: item for item in inventory["durable_records"]}
        operations = {item["id"]: item for item in inventory["operations"]}
        receipt = records["record.codex-disable-operation-v1"]
        config_record = records["record.config-v4"]
        operation = operations["op.codex-enable"]

        self.assertEqual(receipt["authority_owner"], "config.codex-disable-receipt")
        self.assertNotIn("secret", receipt["fields"])
        self.assertNotIn("credential", " ".join(receipt["fields"]))
        self.assertIn("codex_disable_operation", config_record["fields"])
        self.assertEqual(operation["compensation"]["kind"], "prior-runtime-restart")
        for access in ("reads", "writes", "clears"):
            self.assertIn(
                "record.codex-disable-operation-v1",
                operation["durable_records"][access],
            )
        self.assertEqual(
            {entry["name"] for entry in operation["entrypoints"]},
            {"set_experimental_codex_enabled", "codex_disable_replay"},
        )
        self.assertEqual(
            {point["id"] for point in operation["failure_points"]},
            {
                "codex-enable.intent",
                "codex-enable.stop",
                "codex-enable.commit",
                "codex-enable.post-commit-clear",
                "codex-enable.replay-drift",
            },
        )
        effects = " ".join(operation["ordered_effects"])
        self.assertIn("durably publish intent before", effects)
        self.assertIn("typed stopping component phase", effects)
        self.assertIn("stopping as pre-effect WAL only", effects)
        self.assertIn("start-stop ABA", effects)
        self.assertIn("Config authority fence", effects)
        self.assertIn("each fresh process adopts exact deterministic restores", effects)
        self.assertIn("fresh boot replays", effects)
        self.assertIn("never stop or restart another provider", effects)
        stop_failure = next(
            point
            for point in operation["failure_points"]
            if point["id"] == "codex-enable.stop"
        )
        self.assertIn("phase-publication failure", stop_failure["observed_outcome"])
        commit_failure = next(
            point
            for point in operation["failure_points"]
            if point["id"] == "codex-enable.commit"
        )
        self.assertIn("unproven after-spawn Science candidate", commit_failure["observed_outcome"])
        self.assertIn(
            "Recovery attention never erases durable inverse progress",
            operation["compensation"]["contract"],
        )
        self.assertNotIn(
            "record.codex-disable-operation-v1",
            operations["op.codex-network"]["durable_records"]["writes"],
            "P2-A must not silently expand the receipt to op.codex-network",
        )

    def test_p2b_non_one_click_receipt_contract_is_explicit(self):
        inventory = load_inventory()
        records = {item["id"]: item for item in inventory["durable_records"]}
        operations = {item["id"]: item for item in inventory["operations"]}
        receipt = records["record.config-mutation-operation-v1"]

        self.assertEqual(receipt["authority_owner"], "config.mutation-operation")
        self.assertNotIn("secret", receipt["fields"])
        self.assertNotIn("credential", " ".join(receipt["fields"]))
        self.assertIn("config_mutation_operation", records["record.config-v4"]["fields"])
        self.assertEqual(
            {"op.set-mode", "op.set-settings", "op.codex-auth-start", "op.codex-logout", "op.codex-network", "op.revoke-profile"},
            {
                operation["id"]
                for operation in inventory["operations"]
                if "record.config-mutation-operation-v1" in operation["durable_records"]["writes"]
            },
        )
        for operation_id in (
            "op.set-mode",
            "op.set-settings",
            "op.codex-auth-start",
            "op.codex-logout",
            "op.codex-network",
            "op.revoke-profile",
        ):
            operation = operations[operation_id]
            self.assertEqual(
                operation["serialization"],
                "lifecycle-plus-domain-lease-plus-cross-process-effect-fence",
            )
            for access in ("reads", "writes", "clears"):
                self.assertIn("record.config-mutation-operation-v1", operation["durable_records"][access])
            self.assertTrue(any("fence" in effect.lower() for effect in operation["ordered_effects"]))

        controller = (ROOT / "desktop/src/codex-controller.js").read_text(encoding="utf-8")
        self.assertIn("const destructiveCompleted = mutationOutcome", controller)
        self.assertIn('mutationOutcome.operation === "set_codex_network"', controller)
        self.assertIn('typeof mutationOutcome.operation_id === "string"', controller)
        self.assertIn("const intentCommitted = intentOutcome", controller)
        self.assertIn('intentOutcome.operation === "set_codex_network"', controller)
        self.assertIn('intentOutcome.config_state === "committed"', controller)
        self.assertIn('intentOutcome.validation === "not_run"', controller)
        self.assertIn("intentOutcome.science_running === false", controller)
        self.assertIn("(!destructiveCompleted && !intentCommitted)", controller)

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

        science_mutations = []

        def remove_adoption_field(candidate):
            record = next(
                item
                for item in candidate["durable_records"]
                if item["id"] == "record.science-adoption-v1"
            )
            record["fields"].remove("schema_version")

        science_mutations.append(remove_adoption_field)

        def remove_adoption_writer(candidate):
            record = next(
                item
                for item in candidate["durable_records"]
                if item["id"] == "record.science-adoption-v1"
            )
            record["writers"] = [
                writer
                for writer in record["writers"]
                if writer["symbol"] != "ensure_selected_science_runtime_attempt"
            ]

        science_mutations.append(remove_adoption_writer)

        def remove_private_snapshot_owner_field(candidate):
            owner = next(
                item for item in candidate["state_owners"] if item["id"] == "authority.private"
            )
            owner["fields"].remove("science_runtime_snapshot")

        science_mutations.append(remove_private_snapshot_owner_field)

        def remove_preflight_snapshot_write(candidate):
            operation = next(
                item
                for item in candidate["operations"]
                if item["id"] == "op.science-runtime-preflight"
            )
            operation["durable_records"]["writes"].remove(
                "record.science-runtime-snapshot-v1"
            )

        science_mutations.append(remove_preflight_snapshot_write)

        def remove_preflight_snapshot_effect(candidate):
            operation = next(
                item
                for item in candidate["operations"]
                if item["id"] == "op.science-runtime-preflight"
            )
            operation["ordered_effects"] = [
                effect
                for effect in operation["ordered_effects"]
                if "content-address the updater or installed-App executable" not in effect
            ]

        science_mutations.append(remove_preflight_snapshot_effect)

        def remove_preflight_snapshot_failure(candidate):
            operation = next(
                item
                for item in candidate["operations"]
                if item["id"] == "op.science-runtime-preflight"
            )
            operation["failure_points"] = [
                failure
                for failure in operation["failure_points"]
                if failure["id"] != "science-runtime-preflight.runtime-snapshot"
            ]

        science_mutations.append(remove_preflight_snapshot_failure)

        def remove_one_click_snapshot_write(candidate):
            operation = next(
                item for item in candidate["operations"] if item["id"] == "op.one-click"
            )
            operation["durable_records"]["writes"].remove(
                "record.science-runtime-snapshot-v1"
            )

        science_mutations.append(remove_one_click_snapshot_write)

        def remove_one_click_snapshot_effect(candidate):
            operation = next(
                item for item in candidate["operations"] if item["id"] == "op.one-click"
            )
            operation["ordered_effects"] = [
                effect
                for effect in operation["ordered_effects"]
                if "fixed active Science snapshot" not in effect
            ]

        science_mutations.append(remove_one_click_snapshot_effect)

        for mutate in science_mutations:
            bad = copy.deepcopy(inventory)
            mutate(bad)
            with self.assertRaises((AssertionError, KeyError)):
                assert_science_inventory_contract(self, bad)

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
