import hashlib
import json
import os
import plistlib
import socket
import stat
import subprocess
import sys
import tempfile
import threading
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock as mocklib

import test.installed_provider_matrix as controller_module
from test.installed_provider_matrix import (
    CASE_DEFINITIONS,
    CONTROLLER_SCHEMA,
    EXPECTED_BUNDLE_ID,
    EXPECTED_EXECUTABLE,
    FAKE_API_KEY,
    FIXED_PATH_SECRET,
    NETWORK_PROBE_SOURCE,
    NETWORK_SANDBOX_PROFILE,
    ControllerError,
    InProcessScenarioControl,
    InstalledProviderSession,
    NetworkIsolationGuard,
    ProcessInspector,
    ProcessRecord,
    SubprocessScenarioControl,
    UnixScenarioControlClient,
    _dispatch,
    _canonical_regular_file_list_digest,
    _safe_json_write,
    _safe_write,
    _scrub_error,
    _tree_manifest,
    build_case_scenario,
)
from test.model_catalog_coverage_acceptance import (
    require_acceptance_bundle,
    strict_route_checks,
    v3_fixture,
)


class FakeInspector:
    def __init__(self):
        self.records = []
        self.executables = {}
        self.listeners = set()
        self.alive = set()
        self.accept_all_listeners = False
        self.root_pids = []
        self.process_groups = {}
        self.start_markers = {}

    def process_table(self):
        return list(self.records)

    def executable_for_pid(self, pid):
        return self.executables.get(pid)

    def executable_identity_for_pid(self, pid):
        path = self.executables.get(pid)
        if path is None:
            return None
        info = path.stat()
        return {"path": str(path.resolve()), "device": info.st_dev, "inode": info.st_ino}

    def listener_owned(self, pid, port):
        return self.accept_all_listeners or (pid, port) in self.listeners

    def pid_alive(self, pid):
        return pid in self.alive

    def process_group(self, pid):
        return self.process_groups.get(pid)

    def pids_in_process_group(self, pgid):
        return sorted(pid for pid, value in self.process_groups.items() if value == pgid)

    def process_start_marker(self, pid):
        return self.start_markers.get(pid, f"fake-start-{pid}")

    def children(self, parent_pid):
        return [record for record in self.records if record.ppid == parent_pid]

    def descendants(self, parent_pid):
        pending = [parent_pid]
        found = []
        seen = {parent_pid}
        while pending:
            current = pending.pop()
            for record in self.records:
                if record.ppid == current and record.pid not in seen:
                    seen.add(record.pid)
                    found.append(record)
                    pending.append(record.pid)
        return found

    def pids_using_path(self, _root):
        return list(self.root_pids)


class FakePortReservation:
    next_port = 43100

    def __init__(self, port=None):
        if port is None:
            self.port = type(self).next_port
            type(self).next_port += 1
        else:
            self.port = port
            type(self).next_port = max(type(self).next_port, port + 1)

    def release(self):
        return None


class PreviewValueErrorOnceReservation(FakePortReservation):
    raised = False

    def __init__(self, port=None):
        if port is not None and not type(self).raised:
            type(self).raised = True
            raise ValueError("reserved preview port")
        super().__init__(port)


class InstalledProviderMatrixTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="installed-provider-controller-test.")
        self.base = Path(self.temp.name).resolve(strict=True)
        self.app_bundle = self._make_fake_bundle()

    def tearDown(self):
        self.temp.cleanup()

    def _make_fake_bundle(self):
        bundle = self.base / "CSSwitch.app"
        macos = bundle / "Contents/MacOS"
        macos.mkdir(parents=True)
        info = {
            "CFBundleIdentifier": EXPECTED_BUNDLE_ID,
            "CFBundleExecutable": EXPECTED_EXECUTABLE,
            "CFBundleShortVersionString": "0.4.0-test",
        }
        with (bundle / "Contents/Info.plist").open("wb") as handle:
            plistlib.dump(info, handle)
        for name in ("desktop", "csswitch-gateway"):
            path = macos / name
            path.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            path.chmod(0o700)
        return bundle

    def _session(self, case="deepseek-off", inspector=None):
        root = self.base / f"root-{case}-{len(list(self.base.glob('root-*')))}"
        return InstalledProviderSession(
            case,
            root=root,
            app_bundle=self.app_bundle,
            allow_test_bundle=True,
            inspector=inspector or FakeInspector(),
            scenario_control=InProcessScenarioControl(
                build_case_scenario(CASE_DEFINITIONS[case])
            ),
        )

    def _bind_test_g1_receipt(self, session):
        receipt_path = session.root / "test-g1-binding-receipt.json"
        identity_path = session.root / "identity-hashes.json"
        hashes_path = session.root / "hashes.sha256"
        artifact_record_path = session.root / "test-artifact-observation.md"
        source_gate_path = session.root / "completion-seal.json"
        repo_root = Path(controller_module.__file__).resolve(strict=True).parents[1]
        commit = subprocess.run(
            ["/usr/bin/git", "-C", str(repo_root), "rev-parse", "HEAD"],
            check=True,
            stdout=subprocess.PIPE,
            text=True,
        ).stdout.strip()
        branch = subprocess.run(
            ["/usr/bin/git", "-C", str(repo_root), "rev-parse", "--abbrev-ref", "HEAD"],
            check=True,
            stdout=subprocess.PIPE,
            text=True,
        ).stdout.strip()
        desktop_sha256 = hashlib.sha256(session.app_bin.read_bytes()).hexdigest()
        gateway_sha256 = hashlib.sha256(session.gateway_bin.read_bytes()).hexdigest()
        science_sha256 = hashlib.sha256(session.science_bin.read_bytes()).hexdigest()
        bundle_digest = _canonical_regular_file_list_digest(session.app_bundle)
        science_package_digest = _canonical_regular_file_list_digest(session.bin_dir)
        _safe_write(source_gate_path, b'{"unit_test_only":true}\n', 0o600)
        source_authority = {
            "path": str(source_gate_path),
            "sha256": hashlib.sha256(source_gate_path.read_bytes()).hexdigest(),
            "run_id": "unit-test-authority",
            "head_sha": commit,
            "suite_count": 15,
            "aggregate_decision": "PASS",
            "runner_exit": 0,
        }
        artifact_record = "\n".join(
            (
                f"source_commit: {commit}",
                f"canonical_bundle_digest: {bundle_digest}",
                f"desktop_sha256: {desktop_sha256}",
                f"gateway_sha256: {gateway_sha256}",
                "Source gate: `15/15 PASS`, aggregate `PASS`, runner exit `0`",
                "CSSwitch exact-artifact scope: `PASS`",
                "",
            )
        )
        _safe_write(artifact_record_path, artifact_record.encode("utf-8"), 0o600)
        artifact_record_sha256 = hashlib.sha256(
            artifact_record_path.read_bytes()
        ).hexdigest()
        receipt = {
            "schema": "csswitch-g1-binding-receipt.v1",
            "current_g1_result": "PASS",
            "source": {
                "branch": branch,
                "commit": commit,
                "completion_seal_path": str(source_gate_path),
                "completion_seal_sha256": source_authority["sha256"],
            },
            "csswitch": {
                "artifact_path": str(session.app_bundle),
                "artifact_record_sha256": artifact_record_sha256,
                "canonical_bundle_digest": bundle_digest,
                "desktop_sha256": desktop_sha256,
                "gateway_sha256": gateway_sha256,
                "bundle_id": session.expected_bundle_id,
            },
            "science": {
                "executable_path": str(session.science_bin),
                "executable_sha256": science_sha256,
                "package_path": str(session.bin_dir),
                "package_canonical_digest": science_package_digest,
            },
        }
        _safe_json_write(receipt_path, receipt)
        identity = {
            "schema": "csswitch-g1-binding-observation.v1",
            "source_commit": commit,
            "source_gate": source_authority,
            "current_recomputable_receipt": receipt_path.name,
            "artifact_record": {
                "path": str(artifact_record_path),
                "sha256": artifact_record_sha256,
            },
            "csswitch_bundle": {"canonical_digest": bundle_digest},
            "desktop_sha256": desktop_sha256,
            "gateway_sha256": gateway_sha256,
            "science": {
                "executable_sha256": science_sha256,
                "package_canonical_digest": science_package_digest,
            },
        }
        _safe_json_write(identity_path, identity)
        closure = "\n".join(
            f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}"
            for path in (receipt_path, identity_path)
        ) + "\n"
        _safe_write(hashes_path, closure.encode("utf-8"), 0o600)
        session.g1_binding_receipt_path = receipt_path
        session._test_source_authority = source_authority
        return receipt_path

    @staticmethod
    def _fake_network_receipt(session):
        denied = {
            field: 1
            for field in (
                "ipv4_tcp_errno",
                "ipv4_udp_errno",
                "ipv4_dns_tcp_errno",
                "ipv4_dns_udp_errno",
                "ipv6_tcp_errno",
                "ipv6_udp_errno",
                "ipv6_dns_tcp_errno",
                "ipv6_dns_udp_errno",
                "system_resolver_ipc_stream_errno",
                "system_resolver_ipc_datagram_errno",
            )
        }
        return {
            "schema": "csswitch.network-isolation-receipt.v1",
            "profile_path": str(session.network_profile),
            "profile_sha256": hashlib.sha256(
                session.network_profile.read_bytes()
            ).hexdigest(),
            "sandbox_exec": {"path": "/usr/bin/sandbox-exec"},
            "probe": {
                **denied,
                "ipv4_loopback_allowed": True,
                "ipv6_loopback_allowed": True,
                "dns_transport_blocked": True,
                "unique_system_resolver_lookup_failed_observation": True,
                "exit_code": 0,
            },
            "policy": "deny-all-outbound-except-loopback",
            "ok": True,
        }

    def test_coverage_fixture_is_legal_v3_and_bundle_id_is_run_scoped(self):
        fixture = v3_fixture(SimpleNamespace(proxy_port=43191, sandbox_port=43192))
        self.assertEqual(fixture["schema_version"], 3)
        self.assertEqual(fixture["profiles"][0]["model_policy"], "optional_fixed")
        self.assertEqual(
            require_acceptance_bundle(self.app_bundle, EXPECTED_BUNDLE_ID)[
                "CFBundleIdentifier"
            ],
            EXPECTED_BUNDLE_ID,
        )
        coverage_source = Path(controller_module.__file__).with_name(
            "model_catalog_coverage_acceptance.py"
        ).read_text(encoding="utf-8")
        self.assertIn("stopped_science = session.stop_fake_science()", coverage_source)

    def test_required_case_catalog_and_composed_phase_contract(self):
        self.assertTrue(
            {
                "deepseek-off",
                "deepseek-detect",
                "deepseek-rewrite",
                "qwen-chat",
                "custom-chat",
                "responses",
                "relay-force",
                "kimi",
                "siliconflow",
            }.issubset(CASE_DEFINITIONS)
        )
        for case in CASE_DEFINITIONS.values():
            scenario = build_case_scenario(case)
            phases = [step.phase for step in scenario.steps]
            self.assertEqual(phases.count("scratch"), 1, case.case_id)
            self.assertEqual(phases.count("formal"), 1, case.case_id)
            self.assertEqual(phases.count("reuse"), 2, case.case_id)
            self.assertEqual(phases.count("restart"), 1, case.case_id)
            self.assertEqual(phases.count("discovery"), 0 if case.base_kind == "native" else 1)
            self.assertTrue(all("8765" not in step.path for step in scenario.steps))
        silicon = build_case_scenario(CASE_DEFINITIONS["siliconflow"])
        self.assertEqual(silicon.steps[0].path, "http://api.siliconflow.cn/v1/models")
        self.assertTrue(
            all(
                step.path == "http://api.siliconflow.cn/v1/messages"
                for step in silicon.steps[1:]
            )
        )
        kimi = build_case_scenario(CASE_DEFINITIONS["kimi"])
        scratch = next(step for step in kimi.steps if step.phase == "scratch")
        scratch_equals = scratch.checks["body"]["equals"]
        self.assertEqual(scratch_equals["/max_tokens"], 1025)
        self.assertEqual(scratch_equals["/thinking/budget_tokens"], 1024)

    def test_acceptance_bundle_and_data_root_can_be_selected_explicitly(self):
        info_path = self.app_bundle / "Contents/Info.plist"
        with info_path.open("rb") as handle:
            info = plistlib.load(handle)
        info["CFBundleIdentifier"] = "com.csswitch.test"
        with info_path.open("wb") as handle:
            plistlib.dump(info, handle)
        with mocklib.patch.object(controller_module, "LoopbackPortReservation", FakePortReservation):
            session = InstalledProviderSession(
                "qwen-chat",
                root=self.base / "acceptance-root",
                app_bundle=self.app_bundle,
                allow_test_bundle=True,
                expected_bundle_id="com.csswitch.test",
                config_dir_name=".csswitch-acceptance",
                inspector=FakeInspector(),
                scenario_control=InProcessScenarioControl(
                    build_case_scenario(CASE_DEFINITIONS["qwen-chat"])
                ),
            )
        self.addCleanup(session.close)
        self.assertEqual(session.csswitch_dir, session.home / ".csswitch-acceptance")

    def test_preview_value_error_retries_adjacent_port_reservation(self):
        PreviewValueErrorOnceReservation.raised = False
        with mocklib.patch.object(
            controller_module,
            "LoopbackPortReservation",
            PreviewValueErrorOnceReservation,
        ):
            session = self._session("qwen-chat")
        self.addCleanup(session.close)
        self.assertTrue(PreviewValueErrorOnceReservation.raised)
        self.assertEqual(session.preview_port, session.sandbox_port + 1)

    def test_controller_rejects_arbitrary_data_root_names(self):
        with self.assertRaisesRegex(ControllerError, "unsupported config data root"):
            InstalledProviderSession(
                "qwen-chat",
                root=self.base / "unsafe-root",
                app_bundle=self.app_bundle,
                allow_test_bundle=True,
                config_dir_name=".somewhere-else",
                inspector=FakeInspector(),
            )

    def test_workspace_config_and_wrappers_are_private_and_redacted_from_plan(self):
        with self._session("custom-chat") as session:
            plan = session.prepare_dry_run()
            config = json.loads(session.config_path.read_text(encoding="utf-8"))
            self.assertEqual(config["schema_version"], 2)
            self.assertEqual(config["active_id"], "")
            self.assertEqual(config["secret"], FIXED_PATH_SECRET)
            self.assertEqual(config["profiles"][0]["api_key"], FAKE_API_KEY)
            self.assertNotEqual(config["proxy_port"], 8765)
            self.assertNotEqual(config["sandbox_port"], 8765)
            self.assertNotEqual(config["proxy_port"], config["sandbox_port"])
            self.assertEqual(session.preview_port, session.sandbox_port + 1)
            dry_run_mock_port = int(plan["mock_base_url"].rsplit(":", 1)[1])
            self.assertNotIn(
                session.preview_port,
                {session.proxy_port, dry_run_mock_port},
            )
            for directory in (
                session.root,
                session.home,
                session.csswitch_dir,
                session.evidence,
                session.tmp,
                session.bin_dir,
            ):
                self.assertFalse(directory.is_symlink())
                self.assertEqual(stat.S_IMODE(directory.stat().st_mode), 0o700)
            self.assertEqual(stat.S_IMODE(session.config_path.stat().st_mode), 0o600)
            for wrapper in (
                session.fake_science,
                session.bin_dir / "open",
                session.bin_dir / "security",
                session.bin_dir / "python3",
            ):
                self.assertFalse(wrapper.is_symlink())
                self.assertEqual(stat.S_IMODE(wrapper.stat().st_mode), 0o700)
            version = subprocess.run(
                [session.fake_science, "--version"],
                check=False,
                env={"HOME": str(session.home), "PATH": "/usr/bin:/bin"},
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                # A newly materialized executable can spend several seconds in
                # macOS launch/security checks while the full loopback suite is
                # under load. Keep a bounded hang detector without making that
                # cold-start latency a release-gate flake.
                timeout=10,
            )
            self.assertEqual(version.returncode, 0)
            self.assertEqual(version.stdout.strip(), "claude-science acceptance-fixture-1")
            self.assertEqual(version.stderr, "")
            encoded = json.dumps(plan, sort_keys=True)
            self.assertNotIn(FIXED_PATH_SECRET, encoded)
            self.assertNotIn(FAKE_API_KEY, encoded)
            self.assertTrue(plan["dry_run"])
            self.assertTrue(plan["do_not_execute"])

    def test_launchservices_plan_is_exact_and_controller_never_executes_it(self):
        with self._session("relay-force") as session:
            plan = session.prepare_dry_run()
            argv = plan["launch_argv"]
            self.assertEqual(
                argv[:4],
                [
                    "/usr/bin/sandbox-exec",
                    "-p",
                    NETWORK_SANDBOX_PROFILE,
                    "/usr/bin/env",
                ],
            )
            self.assertEqual(argv[4], "-i")
            self.assertEqual(argv[-1], str(self.app_bundle / "Contents/MacOS/desktop"))
            self.assertNotIn("--env", argv)
            self.assertNotIn("--args", argv)
            self.assertEqual(
                session.network_profile.read_text(encoding="utf-8"),
                NETWORK_SANDBOX_PROFILE,
            )
            self.assertIn("(deny network-outbound)", NETWORK_SANDBOX_PROFILE)
            self.assertIn(
                '(allow network-outbound (remote ip "localhost:*"))',
                NETWORK_SANDBOX_PROFILE,
            )
            for field in (
                "ipv4_dns_tcp_errno",
                "ipv4_dns_udp_errno",
                "ipv6_tcp_errno",
                "ipv6_udp_errno",
                "ipv6_dns_tcp_errno",
                "ipv6_dns_udp_errno",
                "system_resolver_ipc_stream_errno",
                "system_resolver_ipc_datagram_errno",
            ):
                self.assertIn(field, NETWORK_PROBE_SOURCE)
            self.assertFalse(plan["controller_launches_app"])
            self.assertFalse(plan["launch_allowed"])
            self.assertTrue(plan["preflight_would_allow_launch"])
            self.assertIsNone(plan["pre_run_manifest"])
            self.assertEqual(
                plan["network_policy"], "deny-all-outbound-except-loopback"
            )
            self.assertEqual(
                session._launch_environment(plan["mock_base_url"])["CSSWITCH_UPSTREAM_URL"],
                plan["mock_base_url"],
            )
            self.assertEqual(
                session._launch_environment(plan["mock_base_url"])[
                    "CFFIXED_USER_HOME"
                ],
                str(session.home),
            )
            self.assertEqual(
                session._launch_environment(plan["mock_base_url"])[
                    "CSSWITCH_ACCEPTANCE_OUTER_SANDBOX"
                ],
                "1",
            )
            encoded = json.dumps(argv)
            self.assertNotIn(FIXED_PATH_SECRET, encoded)
            self.assertNotIn(FAKE_API_KEY, encoded)
            with self.assertRaisesRegex(ControllerError, "unknown controller operation"):
                _dispatch(session, {"op": "launch"})

    def test_siliconflow_proxy_mapping_is_present_but_launch_is_fail_closed(self):
        with self._session("siliconflow") as session:
            plan = session.prepare_dry_run()
            self.assertFalse(plan["launch_allowed"])
            self.assertTrue(plan["preflight_would_allow_launch"])
            self.assertIsNotNone(plan["launch_argv"])
            self.assertEqual(plan["blockers"], [])
            env = session._launch_environment(plan["mock_base_url"])
            self.assertEqual(env["HTTP_PROXY"], plan["mock_base_url"])
            self.assertEqual(env["http_proxy"], plan["mock_base_url"])
            self.assertEqual(env["NO_PROXY"], "")
            self.assertEqual(env["no_proxy"], "")
            self.assertEqual(env["CSSWITCH_UPSTREAM_URL"], plan["mock_base_url"])

    def test_same_bundle_and_unknown_same_named_process_block_launch_without_signal(self):
        inspector = FakeInspector()
        inspector.records = [ProcessRecord(101, 1, "desktop")]
        inspector.executables[101] = self.app_bundle / "Contents/MacOS/desktop"
        with self._session("deepseek-off", inspector) as session:
            plan = session.prepare_dry_run()
            self.assertFalse(plan["launch_allowed"])
            self.assertIn("same_bundle_process_running", plan["blockers"])
            self.assertNotIn(101, inspector.alive)

        inspector = FakeInspector()
        inspector.records = [ProcessRecord(202, 1, "desktop")]
        with self._session("deepseek-off", inspector) as session:
            matches = session.same_bundle_processes()
            self.assertEqual(matches[0]["identity"], "unknown")
            self.assertIn("same_bundle_process_running", session.preflight_blockers())

        bad_bundle = self.base / "Unreadable.app"
        bad_macos = bad_bundle / "Contents/MacOS"
        bad_macos.mkdir(parents=True)
        bad_executable = bad_macos / "desktop"
        bad_executable.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
        bad_executable.chmod(0o700)
        (bad_bundle / "Contents/Info.plist").write_bytes(b"not-a-plist")
        inspector = FakeInspector()
        inspector.records = [ProcessRecord(303, 1, "desktop")]
        inspector.executables[303] = bad_executable
        with self._session("deepseek-off", inspector) as session:
            matches = session.same_bundle_processes()
            self.assertEqual(matches[0]["identity"], "unknown_bundle")
            self.assertIn("same_bundle_process_running", session.preflight_blockers())

    def test_mock_start_uses_dynamic_owned_listener_and_native_discovery_is_no_hit(self):
        inspector = FakeInspector()
        inspector.accept_all_listeners = True
        inspector.executables[os.getpid()] = Path(sys.executable)
        inspector.alive.add(os.getpid())
        with self._session("deepseek-off", inspector) as session:
            session.preselect_profile = True
            ready = session.start_mock()
            self.assertNotIn(ready["port"], {8765, session.proxy_port, session.sandbox_port})
            self.assertTrue(ready["listener_verified"])
            provider_receipt = json.loads(
                (session.evidence / "provider-launch-receipt.json").read_text(
                    encoding="utf-8"
                )
            )
            fixture_receipt = json.loads(
                (session.evidence / "fixture-receipt.json").read_text(encoding="utf-8")
            )
            self.assertTrue(provider_receipt["issued_before_app_launch"])
            self.assertEqual(provider_receipt["listener"]["host"], "127.0.0.1")
            self.assertEqual(provider_receipt["request_count"], 0)
            self.assertEqual(fixture_receipt["credential_class"], "fixed-fake-only")
            self.assertFalse(fixture_receipt["real_provider_credentials_present"])
            self.assertTrue(fixture_receipt["preselected_active_profile"])
            self.assertEqual(len(set(fixture_receipt["dynamic_ports"].values())), 4)
            with self.assertRaisesRegex(ControllerError, "G1 binding receipt"):
                session.safe_plan(auto_boot=True)
            self._bind_test_g1_receipt(session)
            source_authority_patch = mocklib.patch.object(
                controller_module,
                "_validated_source_gate_seal",
                return_value=session._test_source_authority,
            )
            source_authority_patch.start()
            self.addCleanup(source_authority_patch.stop)
            with mocklib.patch.object(
                NetworkIsolationGuard,
                "verify",
                return_value=self._fake_network_receipt(session),
            ):
                plan = session.safe_plan(auto_boot=True)
            self.assertIsNotNone(plan["pre_run_manifest"])
            self.assertIsNone(plan["launch_argv"])
            self.assertEqual(plan["launch_operation"], "launch_guarded")
            self.assertTrue(plan["controller_launches_app"])
            frozen = session.validate_pre_run()
            self.assertTrue(frozen["ok"])
            pre_run = json.loads(
                (session.evidence / "pre-run-manifest.json").read_text(encoding="utf-8")
            )
            self.assertTrue(all(pre_run["preconditions"].values()))
            self.assertEqual(
                pre_run["network_policy"], "deny-all-outbound-except-loopback"
            )
            self.assertTrue(
                pre_run["network_isolation_receipt"]["sha256"]
            )
            encoded = json.dumps(pre_run, sort_keys=True)
            self.assertNotIn(FIXED_PATH_SECRET, encoded)
            self.assertNotIn(FAKE_API_KEY, encoded)
            with mocklib.patch.object(
                session._mock,
                "status",
                return_value={
                    "active_phase": None,
                    "requests": [{"unexpected": True}],
                    "failures": [],
                },
            ):
                with self.assertRaisesRegex(ControllerError, "provider live identity drift"):
                    session.launch_guarded()
            config = json.loads(session.config_path.read_text(encoding="utf-8"))
            original_config = dict(config)
            config["mode"] = "official"
            _safe_json_write(session.config_path, config)
            with self.assertRaisesRegex(ControllerError, "fixture config drift"):
                session.launch_guarded()
            _safe_json_write(session.config_path, original_config)
            with mocklib.patch.object(
                session,
                "_validate_provider_live",
                side_effect=[None, None],
            ), mocklib.patch.object(session, "_port_closed", return_value=False):
                with self.assertRaisesRegex(ControllerError, "acquired before launch"):
                    session.launch_guarded()
            self.assertTrue(session._launch_consumed)
            with self.assertRaisesRegex(ControllerError, "one-shot"):
                session.launch_guarded()
            started = session.enter_phase("discovery")
            self.assertFalse(started["mock_request_expected"])
            finished = session.finish_phase("discovery")
            self.assertTrue(finished["ok"])
            result = session.stop_mock()
            self.assertTrue(result["stopped"])
            self.assertFalse(result["server_thread_alive"])
            original_hash_tree_at = controller_module._hash_tree_at
            rebound_once = {"done": False}

            def rebind_after_hash(directory_fd, prefix=""):
                value = original_hash_tree_at(directory_fd, prefix)
                if prefix == "" and not rebound_once["done"]:
                    rebound_once["done"] = True
                    moved = session.evidence.with_name(session.evidence.name + "-held")
                    session.evidence.rename(moved)
                    session.evidence.mkdir(mode=0o700)
                return value

            with mocklib.patch.object(
                controller_module, "_hash_tree_at", side_effect=rebind_after_hash
            ):
                with self.assertRaisesRegex(ControllerError, "binding changed"):
                    session.finalize_evidence()
            replacement = session.evidence
            moved = session.evidence.with_name(session.evidence.name + "-held")
            replacement.rmdir()
            moved.rename(session.evidence)
            finalized = session.finalize_evidence()
            self.assertTrue(finalized["finalized"])
            self.assertEqual(
                finalized["decision"],
                "INCONCLUSIVE(reason=controller-does-not-aggregate-runtime-observations)",
            )
            for name in (
                "manifest.json",
                "events.ndjson",
                "observations.json",
                "inventory-before.json",
                "inventory-after.json",
                "cleanup.json",
                "hashes.sha256",
            ):
                self.assertTrue((session.evidence / name).is_file())

    def test_config_diff_reports_paths_only_and_enforces_allowlist(self):
        with self._session("responses") as session:
            session.prepare_dry_run()
            config = json.loads(session.config_path.read_text(encoding="utf-8"))
            config["active_id"] = config["profiles"][0]["id"]
            _safe_json_write(session.config_path, config)
            accepted = session.inspect_config(["/active_id"])
            self.assertTrue(accepted["ok"])
            self.assertEqual(accepted["changed_paths"], ["/active_id"])
            config["secret"] = "unexpected-test-value"
            _safe_json_write(session.config_path, config)
            rejected = session.inspect_config(["/active_id"])
            self.assertFalse(rejected["ok"])
            self.assertEqual(rejected["unexpected_paths"], ["/secret"])
            encoded = json.dumps(rejected)
            self.assertNotIn("unexpected-test-value", encoded)
            self.assertNotIn(FIXED_PATH_SECRET, encoded)

    def test_log_scan_outputs_counts_only_and_python_tripwire_is_a_hard_failure(self):
        with self._session("qwen-chat") as session:
            session.prepare_dry_run()
            logs = session.csswitch_dir / "logs"
            logs.mkdir(mode=0o700)
            (logs / "proxy.log").write_text("safe gateway log\n", encoding="utf-8")
            clean = session.scan_logs()
            self.assertTrue(clean["ok"])
            self.assertEqual(clean["sensitive_log_match_count"], 0)
            (logs / "proxy.log").write_text(FAKE_API_KEY, encoding="utf-8")
            session._python_tripwire.write_text("python3-invoked\n", encoding="utf-8")
            dirty = session.scan_logs()
            self.assertFalse(dirty["ok"])
            self.assertGreater(dirty["sensitive_log_match_count"], 0)
            encoded = json.dumps(dirty)
            self.assertNotIn(FAKE_API_KEY, encoded)

    def test_health_and_formal_observations_never_return_raw_secret_key_or_body(self):
        with self._session("responses") as session:
            session.prepare_dry_run()
            captured = {}

            def fake_request(method, path, **kwargs):
                if method == "GET":
                    if path == "/health" or path == "/wrong-installed-secret/health":
                        return 403, {"content-type": "application/json"}, b"{}"
                    return (
                        200,
                        {"content-type": "application/json"},
                        json.dumps(
                            {
                                "gateway": "rust",
                                "provider": "openai-responses",
                                "shim": "off",
                                "launch_id": "launch-safe-1",
                            }
                        ).encode(),
                    )
                captured["body"] = kwargs["body"]
                return (
                    200,
                    {"content-type": "application/json"},
                    b'{"type":"message","content":[]}',
                )

            session._http_request = fake_request
            health = session.inspect_health()
            self.assertTrue(health["ok"])
            formal = session.send_formal()
            self.assertEqual(formal["status"], 200)
            self.assertIn(b'"tools"', captured["body"])
            self.assertNotIn("body", formal)
            for value in (health, formal):
                encoded = json.dumps(value)
                self.assertNotIn(FIXED_PATH_SECRET, encoded)
                self.assertNotIn(FAKE_API_KEY, encoded)

    def test_exact_app_sidecar_listener_and_reuse_restart_records(self):
        inspector = FakeInspector()
        app_exe = self.app_bundle / "Contents/MacOS/desktop"
        gateway_exe = self.app_bundle / "Contents/MacOS/csswitch-gateway"
        inspector.records = [
            ProcessRecord(301, 1, "desktop"),
            ProcessRecord(302, 301, "csswitch-gateway"),
        ]
        inspector.executables = {301: app_exe, 302: gateway_exe}
        inspector.alive = {301, 302}
        with self._session("relay-force", inspector) as session:
            inspector.listeners.add((302, session.proxy_port))
            observed = session.observe_app(timeout_seconds=0.05)
            self.assertEqual(observed["pid"], 301)
            self.assertEqual(session.inspect_sidecar()["pid"], 302)
            self.assertEqual(session.inspect_children()["python_direct_children"], 0)
            session._runtime_records["first"] = {"pid": 302, "launch_id": "launch-a"}
            session._runtime_records["reused"] = {"pid": 302, "launch_id": "launch-a"}
            session._runtime_records["restarted"] = {"pid": 303, "launch_id": "launch-b"}
            self.assertTrue(session.compare_runtime("first", "reused", "reuse")["ok"])
            self.assertTrue(session.compare_runtime("first", "restarted", "restart")["ok"])

    def test_guarded_reopen_preserves_one_exact_primary_process(self):
        inspector = FakeInspector()
        with self._session("deepseek-off", inspector) as session:
            primary_pid = 4101
            helper_pid = 4102
            inspector.records = [ProcessRecord(primary_pid, 1, "desktop")]
            inspector.executables[primary_pid] = session.app_bin
            inspector.alive.add(primary_pid)
            inspector.start_markers[primary_pid] = "primary-start"
            inspector.process_groups[helper_pid] = helper_pid
            session._app_pid = primary_pid
            primary = mocklib.Mock()
            primary.pid = primary_pid
            primary.poll.return_value = None
            session._launcher_process = primary
            session._fixture_receipt = {
                "science_executable": controller_module._file_identity(session.science_bin)
            }
            g1_binding = {"schema": "test-g1-binding"}
            artifact_tree = controller_module._tree_manifest(session.app_bundle)
            manifest = {
                "launch": {"argv": [str(session.app_bin)]},
                "g1_binding_receipt": g1_binding,
                "artifact": {
                    "desktop": controller_module._file_identity(session.app_bin),
                    "gateway": controller_module._file_identity(session.gateway_bin),
                    "entries_sha256": artifact_tree["entries_sha256"],
                },
            }
            encoded = (
                json.dumps(manifest, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
            ).encode("utf-8")
            session._write_evidence_json("pre-run-manifest.json", manifest, once=True)
            session._write_evidence_json(
                "guarded-launch-receipt.json",
                {
                    "schema": "csswitch.guarded-launch-receipt.v1",
                    "pid": primary_pid,
                    "process_start_marker": "primary-start",
                    "controller_owned": True,
                },
                once=True,
            )
            session._pre_run_manifest = manifest
            session._pre_run_manifest_sha256 = hashlib.sha256(encoded).hexdigest()
            helper = mocklib.Mock()
            helper.pid = helper_pid
            helper.poll.return_value = 0
            helper.wait.side_effect = lambda timeout: (
                inspector.process_groups.pop(helper_pid, None),
                0,
            )[1]
            with mocklib.patch.object(
                session, "_validated_g1_binding", return_value=g1_binding
            ), mocklib.patch.object(
                session, "_validate_provider_live", return_value=None
            ), mocklib.patch.object(
                controller_module.subprocess, "Popen", return_value=helper
            ):
                receipt = session.reopen_guarded()
            self.assertTrue(receipt["primary_identity_preserved"])
            self.assertTrue(receipt["second_persistent_app_absent"])
            self.assertEqual(receipt["exact_app_pids_before"], [primary_pid])
            self.assertEqual(receipt["exact_app_pids_after"], [primary_pid])
            self.assertEqual(receipt["helper_exit_code"], 0)
            self.assertTrue(receipt["helper_process_group_empty"])
            self.assertEqual(session._reopen_helper_members, {})
            with self.assertRaisesRegex(ControllerError, "one-shot"):
                session.reopen_guarded()

    def test_guarded_reopen_rejects_a_dead_or_replaced_primary(self):
        inspector = FakeInspector()
        with self._session("deepseek-off", inspector) as session:
            primary_pid = 4201
            inspector.records = [ProcessRecord(primary_pid, 1, "desktop")]
            inspector.executables[primary_pid] = session.app_bin
            inspector.alive.add(primary_pid)
            inspector.start_markers[primary_pid] = "primary-start"
            session._app_pid = primary_pid
            primary = mocklib.Mock()
            primary.pid = primary_pid
            primary.poll.return_value = 0
            session._launcher_process = primary
            session._write_evidence_json(
                "guarded-launch-receipt.json",
                {
                    "schema": "csswitch.guarded-launch-receipt.v1",
                    "pid": primary_pid,
                    "process_start_marker": "primary-start",
                    "controller_owned": True,
                },
                once=True,
            )
            with self.assertRaisesRegex(ControllerError, "primary app is not alive"):
                session._live_guarded_launch_receipt()
            primary.poll.return_value = None
            inspector.start_markers[primary_pid] = "replacement-start"
            with self.assertRaisesRegex(ControllerError, "identity drift"):
                session._live_guarded_launch_receipt()

    def test_reopen_helper_group_cleanup_tracks_and_stops_exact_members(self):
        inspector = FakeInspector()
        with self._session("deepseek-off", inspector) as session:
            pgid = 4301
            child_pid = 4302
            inspector.process_groups[child_pid] = pgid
            inspector.executables[child_pid] = Path(sys.executable)
            inspector.start_markers[child_pid] = "helper-child-start"
            inspector.alive.add(child_pid)
            session._reopen_helper_members[child_pid] = (
                Path(sys.executable),
                "helper-child-start",
            )

            def stop_exact_child(pid, _signal):
                self.assertEqual(pid, child_pid)
                inspector.alive.discard(pid)
                inspector.process_groups.pop(pid, None)

            with mocklib.patch.object(controller_module.os, "kill", side_effect=stop_exact_child):
                self.assertEqual(session._cleanup_reopen_helper_group(), [])
            self.assertEqual(session._reopen_helper_members, {})

    def test_reopen_cleanup_never_adopts_a_reused_process_group(self):
        inspector = FakeInspector()
        with self._session("deepseek-off", inspector) as session:
            old_pid = 4351
            unrelated_pid = 4352
            session._reopen_helper_members[old_pid] = (
                Path(sys.executable),
                "old-helper-start",
            )
            inspector.process_groups[unrelated_pid] = old_pid
            inspector.executables[unrelated_pid] = Path(sys.executable)
            inspector.start_markers[unrelated_pid] = "unrelated-start"
            inspector.alive.add(unrelated_pid)

            with mocklib.patch.object(controller_module.os, "kill") as kill:
                self.assertEqual(session._cleanup_reopen_helper_group(), [])

            kill.assert_not_called()
            self.assertEqual(session._reopen_helper_members, {})

    def test_reopen_freeze_never_overwrites_a_replaced_member_identity(self):
        inspector = FakeInspector()
        with self._session("deepseek-off", inspector) as session:
            helper_pid = 4371
            member_pid = 4372
            helper = mocklib.Mock()
            helper.pid = helper_pid
            helper.poll.return_value = None
            for pid, executable, start in (
                (helper_pid, session.app_bin, "helper-start"),
                (member_pid, Path(sys.executable), "old-member-start"),
            ):
                inspector.process_groups[pid] = helper_pid
                inspector.executables[pid] = executable
                inspector.start_markers[pid] = start
                inspector.alive.add(pid)

            self.assertTrue(session._freeze_live_reopen_helper_group(helper, helper_pid))
            frozen = dict(session._reopen_helper_members)
            inspector.executables[member_pid] = session.gateway_bin
            inspector.start_markers[member_pid] = "replacement-start"

            with self.assertRaisesRegex(ControllerError, "frozen member identity drift"):
                session._freeze_live_reopen_helper_group(helper, helper_pid)

            self.assertEqual(session._reopen_helper_members, frozen)

    def test_reopen_freeze_error_stops_the_controller_owned_helper(self):
        inspector = FakeInspector()
        with self._session("deepseek-off", inspector) as session:
            helper = mocklib.Mock()
            helper.poll.side_effect = [None, None]
            helper.wait.return_value = 0

            session._stop_reopen_helper_process(helper)

            helper.terminate.assert_called_once_with()
            helper.wait.assert_called_once_with(timeout=3.0)
            helper.kill.assert_not_called()

    def test_close_does_not_adopt_a_reused_reopen_helper_process_group(self):
        inspector = FakeInspector()
        with self._session("deepseek-off", inspector) as session:
            old_pid = 4401
            reused_pgid = 4401
            unrelated_pid = 4402
            session._reopen_helper_members[old_pid] = (
                Path(sys.executable),
                "old-helper-start",
            )
            inspector.process_groups[unrelated_pid] = reused_pgid
            inspector.executables[unrelated_pid] = Path(sys.executable)
            inspector.start_markers[unrelated_pid] = "unrelated-start"
            inspector.alive.add(unrelated_pid)

            with mocklib.patch.object(controller_module.os, "kill") as kill:
                session.close()

            kill.assert_not_called()

    def test_cleanup_probes_only_owned_dynamic_ports_and_does_not_signal(self):
        inspector = FakeInspector()
        with self._session("custom-chat", inspector) as session:
            session.prepare_dry_run()
            session._launcher_process = SimpleNamespace(pid=808)
            session._launcher_pgid = 808
            inspector.records = [ProcessRecord(909, 1, "reparented-early-sidecar")]
            inspector.process_groups[909] = 808
            inspector.alive.add(909)
            inspector.executables[909] = Path(sys.executable)
            result = session.verify_cleanup()
            self.assertFalse(result["ok"])
            self.assertEqual(
                next(item for item in result["owned_pids"] if item["pid"] == 909)[
                    "source"
                ],
                "guarded-process-group",
            )
            session._launcher_process = None
            session._launcher_pgid = None
            inspector.records = []
            inspector.process_groups = {}
            inspector.alive.clear()
            inspector.root_pids = [909]
            inspector.alive.add(909)
            inspector.executables[909] = Path(sys.executable)
            result = session.verify_cleanup()
            self.assertFalse(result["ok"])
            self.assertEqual(result["runtime_root_open_pids"], [909])
            inspector.root_pids = []
            inspector.alive.clear()
            result = session.verify_cleanup()
            self.assertTrue(result["ok"])
            self.assertEqual(
                set(result["ports_closed"]), {"proxy", "sandbox", "preview"}
            )
            self.assertNotIn(8765, (session.proxy_port, session.sandbox_port))

    def test_cleanup_accepts_stopped_fake_science_empty_state_but_rejects_partial_state(self):
        inspector = FakeInspector()
        with self._session("custom-chat", inspector) as session:
            session.prepare_dry_run()
            state = (
                session.csswitch_dir
                / "sandbox/home/.claude-science/csswitch-installed-fake-science"
            )
            state.mkdir(parents=True, mode=0o700)
            self.assertTrue(session.verify_cleanup()["fake_science_state_valid"])

            (state / "pid").write_text("701\n", encoding="utf-8")
            result = session.verify_cleanup()
            self.assertFalse(result["fake_science_state_valid"])
            self.assertFalse(result["ok"])

    def test_destroy_workspace_requires_closed_owned_state_and_rejects_symlinks(self):
        inspector = FakeInspector()
        session = self._session("custom-chat", inspector)
        session.prepare_dry_run()
        root = session.root
        inspector.alive.add(701)
        inspector.executables[701] = self.app_bundle / "Contents/MacOS/desktop"
        session._app_pid = 701
        with self.assertRaisesRegex(ControllerError, "owned cleanup"):
            session.destroy_workspace()
        self.assertTrue(root.exists())
        inspector.alive.clear()
        outside = self.base / "outside-root"
        outside.mkdir()
        marker = outside / "preserved.txt"
        marker.write_text("keep", encoding="utf-8")
        (root / "external-link").symlink_to(outside, target_is_directory=True)
        self.assertEqual(session.destroy_workspace(), {"root_removed": True})
        self.assertFalse(root.exists())
        self.assertEqual(marker.read_text(encoding="utf-8"), "keep")
        session.close()

        external_evidence = self.base / "preserved-evidence"
        external = InstalledProviderSession(
            "custom-chat",
            root=self.base / "external-evidence-root",
            evidence_dir=external_evidence,
            app_bundle=self.app_bundle,
            allow_test_bundle=True,
            inspector=FakeInspector(),
            scenario_control=InProcessScenarioControl(
                build_case_scenario(CASE_DEFINITIONS["custom-chat"])
            ),
        )
        external.prepare_dry_run()
        self.assertTrue(external.destroy_workspace()["root_removed"])
        self.assertTrue(external_evidence.is_dir())
        self.assertEqual(stat.S_IMODE(external_evidence.stat().st_mode), 0o700)
        external.close()

        symlink_parent = self.base / "evidence-parent-link"
        symlink_parent.symlink_to(self.base, target_is_directory=True)
        refused_evidence = symlink_parent / "must-not-be-created"
        with self.assertRaisesRegex(ControllerError, "symlink"):
            InstalledProviderSession(
                "custom-chat",
                root=self.base / "refused-evidence-root",
                evidence_dir=refused_evidence,
                app_bundle=self.app_bundle,
                allow_test_bundle=True,
                inspector=FakeInspector(),
            )
        self.assertFalse(refused_evidence.exists())

    def test_destroy_workspace_refuses_a_live_owned_port(self):
        session = self._session("deepseek-off")
        session.prepare_dry_run()
        session._proxy_reservation.release()
        listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        try:
            listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            listener.bind(("127.0.0.1", session.proxy_port))
            listener.listen(1)
            with self.assertRaisesRegex(ControllerError, "owned cleanup"):
                session.destroy_workspace()
        finally:
            listener.close()
        self.assertTrue(session.destroy_workspace()["root_removed"])
        session.close()

    def test_sanitized_summary_exports_outside_root_without_sensitive_values(self):
        export_temp = tempfile.TemporaryDirectory(prefix="csim-export.", dir="/private/tmp")
        self.addCleanup(export_temp.cleanup)
        export_parent = Path(export_temp.name)
        export_parent.chmod(0o700)
        destination = export_parent / "summary.json"
        session = self._session("responses")
        try:
            session.prepare_dry_run()
            session.scan_logs()
            exported = session.export_sanitized_summary(destination)
            self.assertEqual(exported["destination"], str(destination))
            raw = destination.read_text(encoding="utf-8")
            self.assertNotIn(FIXED_PATH_SECRET, raw)
            self.assertNotIn(FAKE_API_KEY, raw)
            value = json.loads(raw)
            self.assertEqual(value["case"], "responses")
            self.assertEqual(stat.S_IMODE(destination.stat().st_mode), 0o600)
            with self.assertRaisesRegex(ControllerError, "outside repo"):
                session.export_sanitized_summary(session.root / "summary.json")
            self.assertTrue(session.destroy_workspace()["root_removed"])
        finally:
            session.close()

    def test_external_cli_control_uses_private_evidence_and_exits_via_socket(self):
        external_temp = tempfile.TemporaryDirectory(prefix="csim-ext.", dir="/private/tmp")
        self.addCleanup(external_temp.cleanup)
        parent = Path(external_temp.name)
        parent.chmod(0o700)
        control = SubprocessScenarioControl(
            build_case_scenario(CASE_DEFINITIONS["deepseek-off"]),
            parent,
        )
        ready = control.start()
        self.assertEqual(ready["phase"], None)
        self.assertNotIn("token", json.dumps(ready))
        control.enter_phase("discovery")
        self.assertEqual(control.status()["active_phase"], "discovery")
        result = control.stop()
        self.assertTrue(result["stopped"])
        self.assertFalse(result["final_ok"])
        self.assertEqual(result["owned_process_exit_code"], 1)
        self.assertFalse(control.process_alive)
        for evidence_file in control.evidence_dir.glob("*.json"):
            self.assertNotIn("token", evidence_file.read_text(encoding="utf-8"))

    def test_external_cli_ready_failure_terminates_only_owned_subprocess(self):
        external_temp = tempfile.TemporaryDirectory(prefix="csim-start-fail.", dir="/private/tmp")
        self.addCleanup(external_temp.cleanup)
        parent = Path(external_temp.name)
        parent.chmod(0o700)
        control = SubprocessScenarioControl(
            build_case_scenario(CASE_DEFINITIONS["deepseek-off"]),
            parent,
        )
        with mocklib.patch.object(
            control,
            "_read_ready",
            side_effect=ControllerError("injected ready failure"),
        ):
            with self.assertRaisesRegex(ControllerError, "injected ready failure"):
                control.start()
        self.assertIsNotNone(control.process_pid)
        self.assertFalse(control.process_alive)

    def test_installed_bundle_identity_rejects_symlinked_components(self):
        real_contents = self.base / "real-contents"
        real_macos = real_contents / "MacOS"
        real_macos.mkdir(parents=True)
        info = {
            "CFBundleIdentifier": EXPECTED_BUNDLE_ID,
            "CFBundleExecutable": EXPECTED_EXECUTABLE,
        }
        with (real_contents / "Info.plist").open("wb") as handle:
            plistlib.dump(info, handle)
        for name in ("desktop", "csswitch-gateway"):
            path = real_macos / name
            path.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            path.chmod(0o700)
        linked_bundle = self.base / "Linked.app"
        linked_bundle.mkdir()
        (linked_bundle / "Contents").symlink_to(real_contents, target_is_directory=True)
        with self.assertRaisesRegex(ControllerError, "traverses a symlink"):
            InstalledProviderSession(
                "deepseek-off",
                root=self.base / "linked-root",
                app_bundle=linked_bundle,
                allow_test_bundle=True,
                inspector=FakeInspector(),
            )

        manifest_root = self.base / "manifest-root"
        manifest_root.mkdir()
        target = manifest_root / "target.txt"
        target.write_text("bound", encoding="utf-8")
        (manifest_root / "internal-link").symlink_to(target)
        manifest = _tree_manifest(manifest_root)
        link_entry = next(
            entry for entry in manifest["entries"] if entry["path"] == "internal-link"
        )
        self.assertEqual(link_entry["target_identity"]["sha256"], hashlib.sha256(b"bound").hexdigest())
        outside_target = self.base / "outside-target.txt"
        outside_target.write_text("escape", encoding="utf-8")
        (manifest_root / "external-link").symlink_to(outside_target)
        with self.assertRaisesRegex(ControllerError, "escapes the bundle"):
            _tree_manifest(manifest_root)

    def test_default_session_mock_is_external_and_identity_checked(self):
        external_temp = tempfile.TemporaryDirectory(prefix="csim-session.", dir="/private/tmp")
        self.addCleanup(external_temp.cleanup)
        root = Path(external_temp.name) / "owned"
        session = InstalledProviderSession(
            "deepseek-off",
            root=root,
            app_bundle=self.app_bundle,
            allow_test_bundle=True,
            inspector=ProcessInspector(),
        )
        try:
            self.assertIsInstance(session._mock, SubprocessScenarioControl)
            ready = session.start_mock()
            self.assertTrue(ready["listener_verified"])
            expected = ProcessInspector().executable_for_pid(os.getpid())
            self.assertEqual(ready["owned_executable"], str(expected))
            session.enter_phase("discovery")
            session.finish_phase("discovery")
            stopped = session.stop_mock()
            self.assertTrue(stopped["stopped"])
            self.assertTrue(session.verify_cleanup()["ok"])
            self.assertTrue(session.destroy_workspace()["root_removed"])
        finally:
            session.close()

    def test_control_error_scrubbing_and_json_controller_schema(self):
        message = f"bad {FIXED_PATH_SECRET} and {FAKE_API_KEY}"
        scrubbed = _scrub_error(message)
        self.assertNotIn(FIXED_PATH_SECRET, scrubbed)
        self.assertNotIn(FAKE_API_KEY, scrubbed)
        self.assertIn("<redacted>", scrubbed)
        self.assertEqual(CONTROLLER_SCHEMA, "csswitch.installed-provider-controller.v1")

    def test_unix_control_client_keeps_token_in_memory_and_uses_reviewed_commands(self):
        socket_temp = tempfile.TemporaryDirectory(prefix="csim-sock.", dir="/private/tmp")
        self.addCleanup(socket_temp.cleanup)
        evidence = Path(socket_temp.name) / "e"
        evidence.mkdir(mode=0o700)
        evidence.chmod(0o700)
        socket_path = evidence / "control.sock"
        token = "controller-token-never-persist"
        received = []
        server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        server.bind(str(socket_path))
        socket_path.chmod(0o600)
        server.listen(4)

        def serve():
            while True:
                conn, _ = server.accept()
                with conn:
                    raw = b""
                    while not raw.endswith(b"\n"):
                        raw += conn.recv(4096)
                    request = json.loads(raw)
                    received.append({key: value for key, value in request.items() if key != "token"})
                    self.assertEqual(request["token"], token)
                    command = request["command"]
                    if command == "status":
                        response = {"ok": True, "status": {"requests": [], "failures": []}}
                    elif command == "enter_phase":
                        response = {"ok": True, "status": {"requests": [], "failures": []}}
                    elif command == "wait":
                        response = {"ok": True, "completed": False, "status": {}}
                    else:
                        response = {"ok": True, "accepted": True}
                    conn.sendall(json.dumps(response).encode() + b"\n")
                    if command == "stop":
                        _safe_json_write(
                            evidence / "result.json",
                            {"final_ok": True, "stopped": True, "requests": [], "failures": []},
                        )
                        break
            server.close()

        thread = threading.Thread(target=serve)
        thread.start()
        ready = {
            "control_socket": "control.sock",
            "base_url": "http://127.0.0.1:32123",
            "port": 32123,
            "owned_pid": 999,
        }
        client = UnixScenarioControlClient(evidence, ready, token)
        self.assertNotIn(token, json.dumps(client.start()))
        client.enter_phase("scratch")
        self.assertEqual(client.status()["requests"], [])
        self.assertFalse(client.wait(0.01))
        self.assertTrue(client.stop()["final_ok"])
        thread.join(timeout=2)
        self.assertFalse(thread.is_alive())
        self.assertEqual(
            [request["command"] for request in received],
            ["enter_phase", "status", "wait", "stop"],
        )
        self.assertFalse((evidence / "ready-token.json").exists())

    def test_coverage_strict_routes_consume_phases_and_send_exact_selector(self):
        class FakeMock:
            def __init__(self):
                self.requests = []

            def status(self):
                return {"requests": list(self.requests)}

        class FakeCoverageSession:
            def __init__(self):
                self._mock = FakeMock()
                self.phases = []
                self.models = []

            def enter_phase(self, phase):
                self.phases.append(phase)

            def finish_phase(self, phase):
                return {"phase": phase, "ok": True}

            def send_formal(self):
                self._mock.requests.append({"model": "qwen3.7-max"})
                return {"status": 200}

            def _http_request(self, method, path, *, body=None, headers=None, timeout=4.0):
                del method, path, headers, timeout
                payload = json.loads(body or b"{}")
                model = payload.get("model")
                self.models.append(model)
                if model == "claude-csswitch-codex-stale-should-fail":
                    return 400, {}, b'{"error":{"type":"route_unknown"}}'
                self._mock.requests.append({"model": "qwen3.7-max"})
                return 200, {}, b"{}"

        session = FakeCoverageSession()
        selector = "claude-csswitch-qwen-qwen3-7-max-0123456789ab"
        result = strict_route_checks(
            session,
            exact_selector=selector,
            expected_upstream="qwen3.7-max",
        )
        self.assertEqual(
            session.phases,
            ["discovery", "scratch", "formal", "reuse", "restart"],
        )
        self.assertEqual(session.models.count(selector), 4)
        self.assertEqual(result["unknown_status"], 400)
        self.assertEqual(result["unknown_upstream_requests"], 0)


if __name__ == "__main__":
    unittest.main()
