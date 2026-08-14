use super::*;

fn runtime_journal_test_config(
    binding: Option<RuntimeBindingCommit>,
    transaction: Option<config::RuntimeTransactionRecord>,
) -> Config {
    let (model_catalog, default_model_route_id, role_bindings) =
        crate::model_catalog::new_profile_catalog(
            "deepseek",
            "anthropic",
            Some("deepseek-v4-flash"),
        )
        .unwrap();
    Config {
        profiles: vec![config::Profile {
            id: "target".into(),
            template_id: "deepseek".into(),
            api_format: "anthropic".into(),
            model: "deepseek-v4-flash".into(),
            model_catalog,
            default_model_route_id,
            role_bindings,
            model_policy: crate::provider_contracts::ModelPolicy::SavedCatalog,
            ..Default::default()
        }],
        active_id: "target".into(),
        runtime_binding: binding,
        runtime_transaction: transaction,
        ..Default::default()
    }
}

fn terminal_gateway_record(binding: Option<RuntimeBindingCommit>) -> config::RuntimeTransactionV2 {
    config::RuntimeTransactionV2 {
        schema_version: config::RUNTIME_TRANSACTION_SCHEMA_VERSION_V2,
        transaction_id: "terminal-gateway-handoff".into(),
        operation: config::RuntimeTransactionOperation::ProfileSwitch,
        target_profile_id: "target".into(),
        phase: config::RuntimeTransactionPhase::RecoverInterruptedGateway,
        runtime_fingerprint: None,
        environment_exposure: config::RuntimeEnvironmentExposure::NotExposed,
        snapshot_ticket: None,
        previous_binding: binding,
        previous_gateway: None,
        compensation: config::RuntimeCompensationState::NotStarted,
        gateway_stop_outcome: config::RuntimeGatewayStopOutcome::Stopped,
        prior_stop: config::RuntimePriorStopState::NotRequired,
        finalize: config::RuntimeFinalizeState::NotStarted,
    }
}

fn record_compensation_step(
    dir: &Path,
    progress: &mut OneClickJournalProgress,
    step: config::RuntimeCompensationStep,
    outcome: config::RuntimeCompensationStepState,
) {
    begin_one_click_compensation_step(dir, progress, step).unwrap();
    if step == config::RuntimeCompensationStep::AuthorityRestore {
        let restored_runtime_transaction = match progress {
            OneClickJournalProgress::Compensating {
                restored_runtime_transaction,
                ..
            } => restored_runtime_transaction.as_ref().clone(),
            _ => panic!("authority restore test step requires compensation progress"),
        };
        config::update(dir, |current| {
            current.runtime_transaction = restored_runtime_transaction.clone()
        })
        .unwrap();
        finish_one_click_authority_restore_step(dir, progress, outcome).unwrap();
    } else {
        finish_one_click_compensation_step(dir, progress, step, outcome).unwrap();
    }
}

fn record_successful_test_compensation(dir: &Path, progress: &mut OneClickJournalProgress) {
    record_compensation_step(
        dir,
        progress,
        config::RuntimeCompensationStep::ScienceCleanup,
        config::RuntimeCompensationStepState::Skipped {
            cause: config::RuntimeCompensationSkipCause::NoScienceCandidate,
        },
    );
    record_compensation_step(
        dir,
        progress,
        config::RuntimeCompensationStep::SshCleanup,
        config::RuntimeCompensationStepState::Succeeded,
    );
    record_compensation_step(
        dir,
        progress,
        config::RuntimeCompensationStep::AuthorityRestore,
        config::RuntimeCompensationStepState::Succeeded,
    );
    record_compensation_step(
        dir,
        progress,
        config::RuntimeCompensationStep::PriorScienceRestart,
        config::RuntimeCompensationStepState::Skipped {
            cause: config::RuntimeCompensationSkipCause::NoPriorScience,
        },
    );
    record_compensation_step(
        dir,
        progress,
        config::RuntimeCompensationStep::SnapshotCleanup,
        config::RuntimeCompensationStepState::Succeeded,
    );
}

fn write_prior_restart_replay_science(path: &Path) {
    std::fs::write(
        path,
        r#"#!/bin/sh
set -eu
cmd="${1:-}"
if [ "$#" -gt 0 ]; then shift; fi
if [ "$cmd" = "--version" ]; then
  echo "claude-science prior-restart-replay-test"
  exit 0
fi
data_dir=""
port=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --data-dir) data_dir="$2"; shift 2 ;;
    --port) port="$2"; shift 2 ;;
    *) shift ;;
  esac
done
state="$data_dir/fake-science"
mkdir -p "$state"
case "$cmd" in
  serve)
    printf '%s' "$port" > "$state/port"
    /usr/bin/python3 - "$port" "$state/pid" >/dev/null 2>&1 <<'PY' &
import http.server
import os
import socketserver
import sys
port = int(sys.argv[1])
pidfile = sys.argv[2]
class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass
    def do_GET(self):
        if self.path.startswith("/health"):
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b'{"status":"ok"}')
        else:
            self.send_response(404)
            self.end_headers()
socketserver.TCPServer.allow_reuse_address = True
with open(pidfile, "w", encoding="utf-8") as f:
    f.write(str(os.getpid()))
with socketserver.TCPServer(("127.0.0.1", port), Handler) as server:
    server.serve_forever()
PY
    ;;
  status)
    pid="$(cat "$state/pid" 2>/dev/null || true)"
    if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
      echo '{"running":true}'
    else
      echo '{"running":false}'
      exit 1
    fi
    ;;
  url)
    port="$(cat "$state/port")"
    printf 'http://127.0.0.1:%s/\n' "$port"
    ;;
  stop)
    pid="$(cat "$state/pid" 2>/dev/null || true)"
    if [ -n "$pid" ]; then kill "$pid" 2>/dev/null || true; fi
    rm -f "$state/pid"
    ;;
  *) exit 2 ;;
esac
"#,
    )
    .unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
}

fn reserve_prior_restart_replay_port() -> u16 {
    for _ in 0..128 {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        if matches!(port, 8764 | 8765 | 65535) {
            continue;
        }
        if let Ok(preview) = TcpListener::bind(("127.0.0.1", port + 1)) {
            drop(preview);
            drop(listener);
            return port;
        }
    }
    panic!("could not reserve a prior-restart replay port pair");
}

fn assert_v1_prior_restart_replay_hydrates_v2_receipt(env: &mut ScopedEnv) {
    let tmp = isolated_tmpdir("o1-e3-v1-prior-v2-replay");
    let home = tmp.join("home");
    let bin_dir = tmp.join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let science_bin = bin_dir.join("claude-science");
    write_prior_restart_replay_science(&science_bin);
    let science_bin = science_bin.canonicalize().unwrap();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    env.set("HOME", &home);
    env.set("CSSWITCH_REPO", &repo);
    env.set("SCIENCE_BIN", &science_bin);
    env.set("CSSWITCH_TEST_FAKE_SCIENCE_IDENTITY", "1");

    let port = reserve_prior_restart_replay_port();
    let dir = config::default_dir();
    let sandbox_home = dir.join("sandbox/home");
    let auth_dir = sandbox_home.join(".claude-science");
    std::fs::create_dir_all(&auth_dir).unwrap();
    let mut initial = runtime_journal_test_config(None, None);
    initial.sandbox_port = port;
    initial.reuse_system_ssh = false;
    config::save_to(&dir, &initial).unwrap();

    let runtime = crate::runtime::science::test_runtime_identity(science_bin);
    let mut launch_runtime = runtime.clone();
    crate::runtime::science::bind_selected_science_runtime_attempt(&mut launch_runtime).unwrap();
    assert_ne!(runtime, launch_runtime);
    assert!(runtime.adoption_attempt_id().is_none());
    assert!(launch_runtime.adoption_attempt_id().is_some());
    assert!(
        !runtime_environment_fingerprint_changed(
            Some(&runtime.environment_transaction_id()),
            &launch_runtime.environment_transaction_id(),
        ),
        "adoption provenance alone must not split one executable environment"
    );
    let recipe = config::RuntimePriorScienceRecipe {
        port,
        runtime_path: runtime.path.clone(),
        runtime_source: runtime.source.code().to_string(),
        runtime_version: runtime.version.clone(),
        runtime_fingerprint: runtime.environment_transaction_id(),
        runtime_adoption_attempt_id: None,
        launch_receipt_digest: "a".repeat(64),
    };
    let intent = begin_prior_stop_intent(
        &dir,
        "target",
        &runtime.environment_transaction_id(),
        None,
        None,
        recipe.clone(),
    )
    .unwrap();
    let stopped =
        publish_prior_stop_outcome(&dir, &intent, config::RuntimePriorStopOutcome::ExactStopped)
            .unwrap();
    let state: SharedAppState = Arc::new(Mutex::new(AppState::default()));
    let stopped_cfg = config::load_from(&dir).unwrap();
    let authority =
        AuthorityTransaction::capture(&dir, &sandbox_home, &auth_dir, &stopped_cfg, &state)
            .unwrap();
    let ticket = authority.registered_snapshot_ticket().unwrap();
    let identity = OneClickTransactionIdentity {
        target_profile_id: "target".into(),
        runtime_fingerprint: runtime.environment_transaction_id(),
        snapshot_ticket: ticket.clone(),
        previous_binding: None,
        gateway_terminal_handoff: None,
        prior_stop: stopped.prior_stop.clone(),
    };
    let mut progress = OneClickJournalProgress::Journaled {
        record: stopped,
        registered_ticket: ticket,
    };
    write_one_click_checkpoint(
        &dir,
        &identity,
        &mut progress,
        config::RuntimeTransactionPhase::AuthoritySnapshotActive,
    )
    .unwrap();
    test_begin_replayable_compensation(
        &dir,
        &authority,
        &state,
        &identity,
        &mut progress,
        launch_runtime,
        None,
        true,
    )
    .unwrap();
    record_compensation_step(
        &dir,
        &mut progress,
        config::RuntimeCompensationStep::ScienceCleanup,
        config::RuntimeCompensationStepState::Skipped {
            cause: config::RuntimeCompensationSkipCause::NoScienceCandidate,
        },
    );
    record_compensation_step(
        &dir,
        &mut progress,
        config::RuntimeCompensationStep::SshCleanup,
        config::RuntimeCompensationStepState::Succeeded,
    );
    record_compensation_step(
        &dir,
        &mut progress,
        config::RuntimeCompensationStep::AuthorityRestore,
        config::RuntimeCompensationStepState::Succeeded,
    );
    begin_one_click_compensation_step(
        &dir,
        &mut progress,
        config::RuntimeCompensationStep::PriorScienceRestart,
    )
    .unwrap();

    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    let lifecycle = crate::lifecycle::Lifecycle::default();
    assert_eq!(
        test_replay_prior_restart_effect_without_outcome(app.handle(), &state, &lifecycle).unwrap(),
        config::RuntimeCompensationStepState::Succeeded,
        "fixture must execute the restart effect without publishing its durable outcome"
    );
    let receipt_path = dir.join("science-managed-launch.v1.json");
    let receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
    let attempt_id = receipt["adoption_attempt_id"]
        .as_str()
        .expect("restart must emit managed launch V2 adoption provenance")
        .to_string();
    assert_eq!(receipt["schema_version"], 2);
    assert!(recipe.runtime_adoption_attempt_id.is_none());
    assert_eq!(
        lock(&state)
            .science_runtime
            .as_ref()
            .and_then(|runtime| runtime.adoption_attempt_id()),
        Some(attempt_id.as_str())
    );
    assert_eq!(
        config::load_from(&dir)
            .unwrap()
            .runtime_compensation
            .as_ref()
            .unwrap()
            .steps
            .iter()
            .find(|step| step.step == config::RuntimeCompensationStep::PriorScienceRestart)
            .map(|step| step.outcome),
        Some(config::RuntimeCompensationStepState::InProgress),
        "fixture boundary must retain the pre-effect durable intent after the restart effect"
    );

    let fresh_state: SharedAppState = Arc::new(Mutex::new(AppState::default()));
    let interrupted = config::load_from(&dir).unwrap();
    assert!(replay_interrupted_one_click_compensation(
        app.handle(),
        &fresh_state,
        &lifecycle,
        None,
        &interrupted,
    )
    .unwrap());
    let recovered_runtime = lock(&fresh_state)
        .science_runtime
        .clone()
        .expect("fresh replay must publish the already-restarted runtime");
    assert_eq!(
        recovered_runtime.adoption_attempt_id(),
        Some(attempt_id.as_str()),
        "fresh replay must hydrate the V2 receipt before publishing AppState"
    );
    assert_eq!(
        config::load_from(&dir)
            .unwrap()
            .runtime_compensation
            .as_ref()
            .unwrap()
            .steps
            .iter()
            .find(|step| step.step == config::RuntimeCompensationStep::PriorScienceRestart)
            .map(|step| step.outcome),
        Some(config::RuntimeCompensationStepState::Succeeded)
    );

    for _ in 0..4 {
        let current = config::load_from(&dir).unwrap();
        if current.runtime_compensation.is_none() {
            break;
        }
        assert!(replay_interrupted_one_click_compensation(
            app.handle(),
            &fresh_state,
            &lifecycle,
            None,
            &current,
        )
        .unwrap());
    }
    assert!(config::load_from(&dir)
        .unwrap()
        .runtime_compensation
        .is_none());
    let stop_result = {
        let mut current = lock(&fresh_state);
        let current = &mut *current;
        crate::runtime::science::stop_sandbox(
            app.handle(),
            &mut current.sandbox,
            &mut current.sandbox_url,
            crate::runtime::science::ScienceStopRequest::recover(Some(&recovered_runtime)),
        )
    };
    assert!(stop_result.is_ok());
    assert!(!crate::proc::loopback_port_in_use(
        port,
        crate::runtime::operation::LOCAL_HEALTH_TIMEOUT_MS,
    ));
    assert!(!receipt_path.exists());
    drop(authority);
    let _ = std::fs::remove_dir_all(tmp);
}

#[test]
fn legacy_v1_prior_restart_replay_hydrates_v2_receipt_regression() {
    let _env_lock = TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut env = ScopedEnv::new();
    assert_v1_prior_restart_replay_hydrates_v2_receipt(&mut env);
}

#[test]
fn one_click_compensation_journal_begin_and_finish_use_complete_record_cas() {
    let dir = isolated_tmpdir("durable-compensation-journal");
    let ticket = config::RuntimeSnapshotTicket::verified(
        ".one-click-rollback-0123456789abcdef0123456789abcdef".into(),
    )
    .unwrap();
    let identity = OneClickTransactionIdentity {
        target_profile_id: "target".into(),
        runtime_fingerprint: "d".repeat(64),
        snapshot_ticket: ticket.clone(),
        previous_binding: None,
        gateway_terminal_handoff: None,
        prior_stop: config::RuntimePriorStopState::NotRequired,
    };
    config::save_to(&dir, &runtime_journal_test_config(None, None)).unwrap();
    let mut pre_journal = OneClickJournalProgress::PreJournalAbort {
        registered_ticket: ticket.clone(),
        runtime_transaction: Box::new(None),
    };
    begin_one_click_compensation(&dir, &identity, &mut pre_journal, None).unwrap();
    let pre_journal_marker = config::load_from(&dir)
        .unwrap()
        .runtime_compensation
        .unwrap();
    assert_eq!(
        pre_journal_marker.state,
        config::RuntimeCompensationState::InProgress
    );
    assert_eq!(
        pre_journal_marker.steps,
        config::pending_one_click_compensation_steps()
    );
    assert!(matches!(
        &pre_journal,
        OneClickJournalProgress::Compensating {
            active_runtime_transaction,
            compensation,
            ..
        } if active_runtime_transaction.as_ref().is_none()
            && compensation.as_ref() == &pre_journal_marker
    ));
    begin_one_click_compensation_step(
        &dir,
        &mut pre_journal,
        config::RuntimeCompensationStep::ScienceCleanup,
    )
    .unwrap();
    let science_intent = config::load_from(&dir)
        .unwrap()
        .runtime_compensation
        .unwrap();
    assert_eq!(
        science_intent.steps[0].outcome,
        config::RuntimeCompensationStepState::InProgress
    );
    finish_one_click_compensation_step(
        &dir,
        &mut pre_journal,
        config::RuntimeCompensationStep::ScienceCleanup,
        config::RuntimeCompensationStepState::Failed,
    )
    .unwrap();
    finish_one_click_compensation(&dir, &mut pre_journal).unwrap();

    config::save_to(&dir, &runtime_journal_test_config(None, None)).unwrap();
    let mut progress = OneClickJournalProgress::PreJournalAbort {
        registered_ticket: ticket.clone(),
        runtime_transaction: Box::new(None),
    };
    write_one_click_checkpoint(
        &dir,
        &identity,
        &mut progress,
        config::RuntimeTransactionPhase::AuthoritySnapshotActive,
    )
    .unwrap();
    begin_one_click_compensation(&dir, &identity, &mut progress, None).unwrap();
    let in_progress = config::load_from(&dir)
        .unwrap()
        .runtime_compensation
        .unwrap()
        .clone();
    assert_eq!(
        in_progress.state,
        config::RuntimeCompensationState::InProgress
    );
    assert!(matches!(
        &progress,
        OneClickJournalProgress::Compensating { compensation, .. }
            if compensation.as_ref() == &in_progress
    ));
    let active_business_record = config::load_from(&dir).unwrap().runtime_transaction;
    config::update(&dir, |current| current.runtime_transaction = None).unwrap();
    assert!(
        begin_one_click_compensation_step(
            &dir,
            &mut progress,
            config::RuntimeCompensationStep::ScienceCleanup,
        )
        .unwrap_err()
        .contains("drifted runtime journal"),
        "pre-authority steps must reject a premature restored business record"
    );
    config::update(&dir, |current| {
        current.runtime_transaction = active_business_record.clone()
    })
    .unwrap();

    record_compensation_step(
        &dir,
        &mut progress,
        config::RuntimeCompensationStep::ScienceCleanup,
        config::RuntimeCompensationStepState::Succeeded,
    );
    record_compensation_step(
        &dir,
        &mut progress,
        config::RuntimeCompensationStep::SshCleanup,
        config::RuntimeCompensationStepState::Failed,
    );
    record_compensation_step(
        &dir,
        &mut progress,
        config::RuntimeCompensationStep::AuthorityRestore,
        config::RuntimeCompensationStepState::Failed,
    );
    config::update(&dir, |current| {
        current.runtime_transaction = active_business_record.clone()
    })
    .unwrap();
    assert!(
        begin_one_click_compensation_step(
            &dir,
            &mut progress,
            config::RuntimeCompensationStep::PriorScienceRestart,
        )
        .unwrap_err()
        .contains("drifted runtime journal"),
        "post-authority steps must reject drift back to the active business record"
    );
    config::update(&dir, |current| current.runtime_transaction = None).unwrap();
    finish_one_click_compensation(&dir, &mut progress).unwrap();
    let incomplete = config::load_from(&dir)
        .unwrap()
        .runtime_compensation
        .unwrap()
        .clone();
    assert_eq!(
        incomplete.state,
        config::RuntimeCompensationState::Incomplete {
            failed_steps: vec![
                config::RuntimeCompensationStep::SshCleanup,
                config::RuntimeCompensationStep::AuthorityRestore,
            ],
        }
    );
    assert!(matches!(
        &progress,
        OneClickJournalProgress::Compensating { compensation, .. }
            if compensation.as_ref() == &incomplete
    ));

    config::save_to(&dir, &runtime_journal_test_config(None, None)).unwrap();
    let mut completed = OneClickJournalProgress::PreJournalAbort {
        registered_ticket: ticket.clone(),
        runtime_transaction: Box::new(None),
    };
    write_one_click_checkpoint(
        &dir,
        &identity,
        &mut completed,
        config::RuntimeTransactionPhase::AuthoritySnapshotActive,
    )
    .unwrap();
    begin_one_click_compensation(&dir, &identity, &mut completed, None).unwrap();
    record_successful_test_compensation(&dir, &mut completed);
    finish_one_click_compensation(&dir, &mut completed).unwrap();
    let completed_config = config::load_from(&dir).unwrap();
    assert!(completed_config.runtime_compensation.is_none());
    assert!(completed_config.runtime_transaction.is_none());
    assert!(matches!(
        completed,
        OneClickJournalProgress::CompensationFinished { .. }
    ));

    let prior_transaction = config::RuntimeTransactionRecord::V2(terminal_gateway_record(None));
    config::save_to(
        &dir,
        &runtime_journal_test_config(None, Some(prior_transaction.clone())),
    )
    .unwrap();
    let mut preserves_prior = OneClickJournalProgress::PreJournalAbort {
        registered_ticket: ticket.clone(),
        runtime_transaction: Box::new(Some(prior_transaction.clone())),
    };
    begin_one_click_compensation(
        &dir,
        &identity,
        &mut preserves_prior,
        Some(prior_transaction.clone()),
    )
    .unwrap();
    record_successful_test_compensation(&dir, &mut preserves_prior);
    finish_one_click_compensation(&dir, &mut preserves_prior).unwrap();
    let prior_preserved = config::load_from(&dir).unwrap();
    assert_eq!(
        prior_preserved.runtime_transaction.as_ref(),
        Some(&prior_transaction)
    );
    assert!(prior_preserved.runtime_compensation.is_none());

    config::save_to(&dir, &runtime_journal_test_config(None, None)).unwrap();
    let mut drifted = OneClickJournalProgress::PreJournalAbort {
        registered_ticket: ticket,
        runtime_transaction: Box::new(None),
    };
    write_one_click_checkpoint(
        &dir,
        &identity,
        &mut drifted,
        config::RuntimeTransactionPhase::AuthoritySnapshotActive,
    )
    .unwrap();
    config::update(&dir, |current| {
        current
            .runtime_transaction
            .as_mut()
            .and_then(config::RuntimeTransactionRecord::as_v2_mut)
            .unwrap()
            .phase = config::RuntimeTransactionPhase::StartGateway;
    })
    .unwrap();
    let before_rejection = std::fs::read(dir.join("config.json")).unwrap();
    assert!(
        begin_one_click_compensation(&dir, &identity, &mut drifted, None)
            .unwrap_err()
            .contains("drifted runtime journal")
    );
    assert_eq!(
        std::fs::read(dir.join("config.json")).unwrap(),
        before_rejection
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn gateway_terminal_handoff_prior_stop_and_finalize_are_exact_replayable_transitions() {
    let dir = isolated_tmpdir("gateway-prior-finalize-protocol");
    let previous = RuntimeBindingCommit {
        profile_id: "prior".into(),
        route_fp: "prior-route".into(),
        catalog_fp: "prior-catalog".into(),
        binding_fp: "prior-binding".into(),
        science_adoption_attempt_id: None,
    };
    let terminal = terminal_gateway_record(Some(previous.clone()));
    config::save_to(
        &dir,
        &runtime_journal_test_config(
            Some(previous.clone()),
            Some(config::RuntimeTransactionRecord::V2(terminal.clone())),
        ),
    )
    .unwrap();
    let accepted = resolve_gateway_terminal_handoff(
        config::load_from(&dir)
            .unwrap()
            .runtime_transaction
            .as_ref(),
        Some(&terminal),
        "target",
        Some(&previous),
    )
    .unwrap()
    .expect("exact terminal record must produce a one-shot handoff");

    let recipe = config::RuntimePriorScienceRecipe {
        port: 8990,
        runtime_path: PathBuf::from(
            "/Applications/Claude Science.app/Contents/Resources/bin/claude-science",
        ),
        runtime_source: "installed_app".into(),
        runtime_version: Some("test-only".into()),
        runtime_fingerprint: "a".repeat(64),
        runtime_adoption_attempt_id: None,
        launch_receipt_digest: "b".repeat(64),
    };
    let replacement = RuntimeBindingCommit {
        profile_id: "replacement".into(),
        route_fp: "replacement-route".into(),
        catalog_fp: "replacement-catalog".into(),
        binding_fp: "replacement-binding".into(),
        science_adoption_attempt_id: None,
    };
    assert!(resolve_gateway_terminal_handoff(
        config::load_from(&dir)
            .unwrap()
            .runtime_transaction
            .as_ref(),
        Some(&terminal),
        "target",
        Some(&replacement),
    )
    .is_err());
    for drift in ["active", "binding"] {
        config::update(&dir, |current| {
            if drift == "active" {
                current.active_id.clear();
            } else {
                current.runtime_binding = Some(replacement.clone());
            }
        })
        .unwrap();
        let drifted = fs::read(dir.join("config.json")).unwrap();
        assert!(begin_prior_stop_intent(
            &dir,
            "target",
            &"c".repeat(64),
            Some(&previous),
            Some(&accepted),
            recipe.clone(),
        )
        .is_err());
        assert_eq!(fs::read(dir.join("config.json")).unwrap(), drifted);
        config::update(&dir, |current| {
            current.active_id = "target".into();
            current.runtime_binding = Some(previous.clone());
        })
        .unwrap();
    }
    let intent = begin_prior_stop_intent(
        &dir,
        "target",
        &"c".repeat(64),
        Some(&previous),
        Some(&accepted),
        recipe.clone(),
    )
    .unwrap();
    assert_eq!(
        config::load_from(&dir)
            .unwrap()
            .runtime_transaction
            .unwrap()
            .as_v2()
            .unwrap()
            .prior_stop,
        config::RuntimePriorStopState::Intent {
            recipe: recipe.clone()
        },
        "durable intent must exist before the external stop effect"
    );
    let stopped =
        publish_prior_stop_outcome(&dir, &intent, config::RuntimePriorStopOutcome::ExactStopped)
            .unwrap();
    let ticket =
        config::RuntimeSnapshotTicket::verified(format!(".one-click-rollback-{}", "d".repeat(32)))
            .unwrap();
    let identity = OneClickTransactionIdentity {
        target_profile_id: "target".into(),
        runtime_fingerprint: "c".repeat(64),
        snapshot_ticket: ticket.clone(),
        previous_binding: Some(previous.clone()),
        gateway_terminal_handoff: Some(accepted),
        prior_stop: stopped.prior_stop.clone(),
    };
    let mut progress = OneClickJournalProgress::Journaled {
        record: stopped,
        registered_ticket: ticket,
    };
    write_one_click_checkpoint(
        &dir,
        &identity,
        &mut progress,
        config::RuntimeTransactionPhase::StartGateway,
    )
    .unwrap();
    let committed = RuntimeBindingCommit {
        profile_id: "target".into(),
        route_fp: "new-route".into(),
        catalog_fp: "new-catalog".into(),
        binding_fp: "new-binding".into(),
        science_adoption_attempt_id: None,
    };
    begin_one_click_finalize(
        &dir,
        &identity,
        &mut progress,
        config::RuntimeFinalizeAction::CommitBinding {
            binding: committed.clone(),
            science_adoption_attempt_id: None,
        },
    )
    .unwrap();
    let finalize_cfg = config::load_from(&dir).unwrap();
    assert_eq!(finalize_cfg.runtime_binding.as_ref(), Some(&previous));
    assert!(matches!(
        finalize_cfg
            .runtime_transaction
            .as_ref()
            .and_then(config::RuntimeTransactionRecord::as_v2)
            .map(|record| &record.finalize),
        Some(config::RuntimeFinalizeState::Intent { .. })
    ));
    let expected_finalize = progress.journaled_record().unwrap().clone();
    config::update(&dir, |current| {
        let record = current
            .runtime_transaction
            .as_mut()
            .and_then(config::RuntimeTransactionRecord::as_v2_mut)
            .unwrap();
        if let config::RuntimeFinalizeState::Intent {
            action:
                config::RuntimeFinalizeAction::CommitBinding {
                    binding,
                    science_adoption_attempt_id: None,
                },
        } = &mut record.finalize
        {
            binding.binding_fp = "drifted-finalize-binding".into();
        }
    })
    .unwrap();
    let drifted_finalize_bytes = fs::read(dir.join("config.json")).unwrap();
    assert!(complete_one_click_finalize(&dir, &mut progress).is_err());
    assert_eq!(
        fs::read(dir.join("config.json")).unwrap(),
        drifted_finalize_bytes,
        "finalize completion must preserve a drifted complete record"
    );
    config::update(&dir, |current| {
        current.runtime_transaction = Some(config::RuntimeTransactionRecord::V2(
            expected_finalize.clone(),
        ));
    })
    .unwrap();
    for drift in ["active", "binding"] {
        config::update(&dir, |current| {
            if drift == "active" {
                current.active_id.clear();
            } else {
                current.runtime_binding = Some(replacement.clone());
            }
        })
        .unwrap();
        let drifted = fs::read(dir.join("config.json")).unwrap();
        assert!(complete_one_click_finalize(&dir, &mut progress).is_err());
        assert_eq!(
            fs::read(dir.join("config.json")).unwrap(),
            drifted,
            "finalize completion must preserve companion config-authority drift"
        );
        config::update(&dir, |current| {
            current.active_id = "target".into();
            current.runtime_binding = Some(previous.clone());
            current.runtime_transaction = Some(config::RuntimeTransactionRecord::V2(
                expected_finalize.clone(),
            ));
        })
        .unwrap();
    }
    complete_one_click_finalize(&dir, &mut progress).unwrap();
    let finalized = config::load_from(&dir).unwrap();
    assert_eq!(finalized.runtime_binding.as_ref(), Some(&committed));
    assert!(finalized.runtime_transaction.is_none());

    config::save_to(
        &dir,
        &runtime_journal_test_config(
            Some(previous.clone()),
            Some(config::RuntimeTransactionRecord::V2(terminal.clone())),
        ),
    )
    .unwrap();
    let mut drifts = Vec::new();
    let mut transaction_drift = terminal.clone();
    transaction_drift.transaction_id = "drifted-terminal".into();
    drifts.push(transaction_drift);
    let mut target_drift = terminal.clone();
    target_drift.target_profile_id = "other-target".into();
    drifts.push(target_drift);
    let mut outcome_drift = terminal.clone();
    outcome_drift.gateway_stop_outcome = config::RuntimeGatewayStopOutcome::ExitUnconfirmed;
    drifts.push(outcome_drift);
    let mut binding_drift = terminal.clone();
    binding_drift.previous_binding.as_mut().unwrap().binding_fp = "drifted-binding".into();
    drifts.push(binding_drift);
    for drifted in &drifts {
        assert!(resolve_gateway_terminal_handoff(
            config::load_from(&dir)
                .unwrap()
                .runtime_transaction
                .as_ref(),
            Some(drifted),
            "target",
            Some(&previous),
        )
        .is_err());
    }
    assert_eq!(
        config::load_from(&dir).unwrap().runtime_transaction,
        Some(config::RuntimeTransactionRecord::V2(terminal))
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn one_click_v2_checkpoints_freeze_candidate_identity_and_ticket() {
    let dir = std::env::temp_dir().join(format!(
        "csswitch-runtime-journal-{}-{}",
        std::process::id(),
        config::new_id()
    ));
    let previous = RuntimeBindingCommit {
        profile_id: "old".into(),
        route_fp: "route-fp".into(),
        catalog_fp: "catalog-fp".into(),
        binding_fp: "binding-fp".into(),
        science_adoption_attempt_id: None,
    };
    let (model_catalog, default_model_route_id, role_bindings) =
        crate::model_catalog::new_profile_catalog(
            "deepseek",
            "anthropic",
            Some("deepseek-v4-flash"),
        )
        .unwrap();
    config::save_to(
        &dir,
        &Config {
            profiles: vec![config::Profile {
                id: "new".into(),
                template_id: "deepseek".into(),
                api_format: "anthropic".into(),
                model: "deepseek-v4-flash".into(),
                model_catalog,
                default_model_route_id,
                role_bindings,
                model_policy: crate::provider_contracts::ModelPolicy::SavedCatalog,
                ..Default::default()
            }],
            active_id: "new".into(),
            runtime_binding: Some(previous.clone()),
            runtime_transaction: Some(
                config::RuntimeTransactionJournal {
                    transaction_id: "legacy-interrupted".into(),
                    target_profile_id: "old-target".into(),
                    stage: "start_gateway".into(),
                    previous_binding: None,
                    previous_gateway: None,
                }
                .into(),
            ),
            ..Default::default()
        },
    )
    .unwrap();

    let snapshot_ticket = config::RuntimeSnapshotTicket::verified(
        ".one-click-rollback-fedcba9876543210fedcba9876543210".into(),
    )
    .unwrap();
    let identity = OneClickTransactionIdentity {
        target_profile_id: "new".into(),
        runtime_fingerprint: "a".repeat(64),
        snapshot_ticket: snapshot_ticket.clone(),
        previous_binding: Some(previous.clone()),
        gateway_terminal_handoff: None,
        prior_stop: config::RuntimePriorStopState::NotRequired,
    };
    let mut progress = OneClickJournalProgress::PreJournalAbort {
        registered_ticket: snapshot_ticket.clone(),
        runtime_transaction: Box::new(config::load_from(&dir).unwrap().runtime_transaction),
    };
    write_one_click_checkpoint(
        &dir,
        &identity,
        &mut progress,
        config::RuntimeTransactionPhase::StopOldScience,
    )
    .unwrap();
    let first_record = config::load_from(&dir)
        .unwrap()
        .runtime_transaction
        .unwrap();
    let first = first_record.as_v2().unwrap();
    assert_eq!(first.schema_version, 2);
    assert_eq!(
        first.operation,
        config::RuntimeTransactionOperation::OneClick
    );
    assert_eq!(first.target_profile_id, "new");
    assert_eq!(first.phase, config::RuntimeTransactionPhase::StopOldScience);
    assert_eq!(first.runtime_fingerprint, Some("a".repeat(64)));
    assert_eq!(first.snapshot_ticket, Some(snapshot_ticket.clone()));
    assert_eq!(first.previous_binding, Some(previous.clone()));
    assert_eq!(
        progress.transaction_id(),
        Some(first.transaction_id.as_str())
    );

    let phases = [
        config::RuntimeTransactionPhase::StartGateway,
        config::RuntimeTransactionPhase::AuthoritySnapshotActive,
        config::RuntimeTransactionPhase::StartScienceEnvironmentPending,
        config::RuntimeTransactionPhase::WaitScienceDbReverify,
        config::RuntimeTransactionPhase::RestartScienceAfterDbHeal,
        config::RuntimeTransactionPhase::VerifyScienceDbAfterRestart,
        config::RuntimeTransactionPhase::VerifyScienceCatalog,
    ];
    for phase in phases {
        write_one_click_checkpoint(&dir, &identity, &mut progress, phase).unwrap();
        let current = config::load_from(&dir)
            .unwrap()
            .runtime_transaction
            .unwrap();
        let current = current.as_v2().unwrap();
        assert_eq!(current.transaction_id, first.transaction_id);
        assert_eq!(current.phase, phase);
        assert_eq!(
            current.runtime_fingerprint.as_deref(),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert_eq!(current.snapshot_ticket.as_ref(), Some(&snapshot_ticket));
        assert_eq!(
            current.environment_exposure,
            one_click_phase_exposure(phase)
        );
    }

    let encoded = serde_json::to_string(
        &config::load_from(&dir)
            .unwrap()
            .runtime_transaction
            .unwrap(),
    )
    .unwrap();
    assert!(!encoded.contains("api_key"));
    assert!(!encoded.contains("base_url"));

    let expected_before_drift = config::load_from(&dir)
        .unwrap()
        .runtime_transaction
        .unwrap();
    for (label, drift) in [
        ("previous-binding", "binding"),
        ("compensation", "compensation"),
        ("phase", "phase"),
    ] {
        config::update(&dir, |current| {
            let typed = current
                .runtime_transaction
                .as_mut()
                .and_then(config::RuntimeTransactionRecord::as_v2_mut)
                .unwrap();
            match drift {
                "binding" => typed.previous_binding.as_mut().unwrap().binding_fp = "drift".into(),
                "compensation" => typed.compensation = config::RuntimeCompensationState::InProgress,
                "phase" => {
                    typed.phase = config::RuntimeTransactionPhase::StartGateway;
                    typed.environment_exposure = config::RuntimeEnvironmentExposure::NotExposed;
                }
                _ => unreachable!(),
            }
        })
        .unwrap();
        let drifted_bytes = std::fs::read(dir.join("config.json")).unwrap();
        let error = write_one_click_checkpoint(
            &dir,
            &identity,
            &mut progress,
            config::RuntimeTransactionPhase::VerifyScienceCatalog,
        )
        .expect_err("same-id complete-record drift must fail closed");
        assert!(error.contains("identity changed"), "{label}");
        assert_eq!(
            std::fs::read(dir.join("config.json")).unwrap(),
            drifted_bytes
        );
        config::update(&dir, |current| {
            current.runtime_transaction = Some(expected_before_drift.clone());
        })
        .unwrap();
    }

    config::update(&dir, |current| {
        let typed = current
            .runtime_transaction
            .as_mut()
            .and_then(config::RuntimeTransactionRecord::as_v2_mut)
            .unwrap();
        typed.phase = config::RuntimeTransactionPhase::StartGateway;
        typed.environment_exposure = config::RuntimeEnvironmentExposure::NotExposed;
    })
    .unwrap();
    let before_terminal_drift_rejection = std::fs::read(dir.join("config.json")).unwrap();
    let clear_error = clear_one_click_transaction(&dir, &identity, &mut progress)
        .expect_err("clear must compare the complete last checkpoint");
    assert!(clear_error.contains("identity changed"));
    let commit_error = commit_runtime_binding(
        &dir,
        &identity,
        &mut progress,
        RuntimeBindingCommit {
            profile_id: "new".into(),
            route_fp: "new-route".into(),
            catalog_fp: "new-catalog".into(),
            binding_fp: "new-binding".into(),
            science_adoption_attempt_id: None,
        },
    )
    .expect_err("binding commit must compare the complete last checkpoint");
    assert!(commit_error.contains("identity changed"));
    assert_eq!(
        std::fs::read(dir.join("config.json")).unwrap(),
        before_terminal_drift_rejection
    );
    config::update(&dir, |current| {
        current.runtime_transaction = Some(expected_before_drift.clone());
    })
    .unwrap();

    let before_typed_advance = std::fs::read(dir.join("config.json")).unwrap();
    let replacement = OneClickTransactionIdentity {
        runtime_fingerprint: "b".repeat(64),
        ..identity.clone()
    };
    let error = write_one_click_checkpoint(
        &dir,
        &replacement,
        &mut progress,
        config::RuntimeTransactionPhase::VerifyScienceCatalog,
    )
    .expect_err("a later phase must not replace the first candidate fingerprint");
    assert!(error.contains("identity changed"));
    assert_eq!(
        std::fs::read(dir.join("config.json")).unwrap(),
        before_typed_advance
    );

    clear_one_click_transaction(&dir, &identity, &mut progress).unwrap();
    assert!(config::load_from(&dir)
        .unwrap()
        .runtime_transaction
        .is_none());

    let _ = std::fs::remove_dir_all(dir);

    let _env_lock = TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut env = ScopedEnv::new();
    let compensation_tmp = isolated_tmpdir("runtime-journal-compensation-cas");
    let home = compensation_tmp.join("home");
    env.set("HOME", &home);
    let compensation_dir = config::default_dir();
    let sandbox_home = compensation_dir.join("sandbox/home");
    let auth_dir = sandbox_home.join(".claude-science");
    std::fs::create_dir_all(&auth_dir).unwrap();
    std::fs::write(auth_dir.join("active-org.json"), b"prior-authority\n").unwrap();
    std::fs::set_permissions(
        auth_dir.join("active-org.json"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let compensation_config = runtime_journal_test_config(None, None);
    config::save_to(&compensation_dir, &compensation_config).unwrap();
    let state: SharedAppState = Arc::new(Mutex::new(AppState::default()));
    let mut authority_transaction = AuthorityTransaction::capture(
        &compensation_dir,
        &sandbox_home,
        &auth_dir,
        &compensation_config,
        &state,
    )
    .unwrap();
    let compensation_ticket = authority_transaction.registered_snapshot_ticket().unwrap();
    let compensation_identity = OneClickTransactionIdentity {
        target_profile_id: "target".into(),
        runtime_fingerprint: "c".repeat(64),
        snapshot_ticket: compensation_ticket.clone(),
        previous_binding: None,
        gateway_terminal_handoff: None,
        prior_stop: config::RuntimePriorStopState::NotRequired,
    };
    let mut compensation_progress = OneClickJournalProgress::PreJournalAbort {
        registered_ticket: compensation_ticket,
        runtime_transaction: Box::new(None),
    };
    write_one_click_checkpoint(
        &compensation_dir,
        &compensation_identity,
        &mut compensation_progress,
        config::RuntimeTransactionPhase::AuthoritySnapshotActive,
    )
    .unwrap();
    config::update(&compensation_dir, |current| {
        let typed = current
            .runtime_transaction
            .as_mut()
            .and_then(config::RuntimeTransactionRecord::as_v2_mut)
            .unwrap();
        typed.phase = config::RuntimeTransactionPhase::StartGateway;
        typed.environment_exposure = config::RuntimeEnvironmentExposure::NotExposed;
    })
    .unwrap();
    let drifted_bytes = std::fs::read(compensation_dir.join("config.json")).unwrap();
    let checkpoint_error = write_one_click_checkpoint(
        &compensation_dir,
        &compensation_identity,
        &mut compensation_progress,
        config::RuntimeTransactionPhase::AuthoritySnapshotActive,
    )
    .expect_err("same-id phase drift must reject the production checkpoint");
    assert!(checkpoint_error.contains("identity changed"));
    assert_eq!(
        std::fs::read(compensation_dir.join("config.json")).unwrap(),
        drifted_bytes
    );
    std::fs::write(auth_dir.join("active-org.json"), b"retargeted-authority\n").unwrap();
    {
        let mut current = crate::lock(&state);
        current.proxy_port = 4242;
    }
    let backup_root = authority_transaction.recovery_path().to_path_buf();
    let science_bin = compensation_tmp.join("fake-science");
    std::fs::write(&science_bin, b"#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&science_bin, std::fs::Permissions::from_mode(0o700)).unwrap();
    let launch_runtime = crate::runtime::science::test_runtime_identity(science_bin);
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    let error = test_compensate_one_click_failure(
        app.handle(),
        &state,
        &crate::lifecycle::Lifecycle::default(),
        &compensation_dir,
        &mut authority_transaction,
        &compensation_identity,
        &mut compensation_progress,
        launch_runtime,
    )
    .expect_err("production compensation must fail closed after journal drift");
    assert!(error
        .to_string()
        .contains("无法持久化 exact runtime journal"));
    assert_eq!(
        std::fs::read(compensation_dir.join("config.json")).unwrap(),
        drifted_bytes,
        "production compensation must not overwrite or clear the drifted journal"
    );
    assert_eq!(
        std::fs::read(auth_dir.join("active-org.json")).unwrap(),
        b"retargeted-authority\n",
        "journal mismatch must reject compensation before restoring protected authority"
    );
    assert_eq!(
        crate::lock(&state).proxy_port,
        4242,
        "journal mismatch must reject compensation before restoring captured AppState"
    );
    assert!(backup_root.is_dir());
    drop(authority_transaction);
    let _ = std::fs::remove_dir_all(compensation_tmp);
}

#[test]
fn durable_authority_v2_preflight_preserves_pending_and_in_progress_journal() {
    let _env_lock = TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut env = ScopedEnv::new();
    let tmp = isolated_tmpdir("o1-e3-fresh-compensation-replay");
    let home = tmp.join("home");
    env.set("HOME", &home);
    let dir = config::default_dir();
    let sandbox_home = dir.join("sandbox/home");
    let auth_dir = sandbox_home.join(".claude-science");
    std::fs::create_dir_all(&auth_dir).unwrap();
    std::fs::create_dir_all(home.join(".ssh")).unwrap();
    let system_ssh_config = home.join(".ssh/config");
    std::fs::write(&system_ssh_config, b"Host alpha\n").unwrap();
    let sandbox_ssh_dir = sandbox_home.join(".ssh");
    std::fs::create_dir_all(&sandbox_ssh_dir).unwrap();
    std::fs::set_permissions(&sandbox_ssh_dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let sandbox_ssh_config = sandbox_ssh_dir.join("config");
    let preexisting_stub = format!(
        "# CSSwitch managed system SSH config bridge v2\nHost alpha\nInclude \"{}\"\n",
        system_ssh_config.display()
    );
    std::fs::write(&sandbox_ssh_config, preexisting_stub.as_bytes()).unwrap();
    std::fs::set_permissions(&sandbox_ssh_config, std::fs::Permissions::from_mode(0o600)).unwrap();
    let ssh_stub_transaction = crate::runtime::settings::ManagedSshStubTransaction::capture(
        &sandbox_home,
        &["alpha".to_string()],
    )
    .unwrap();
    let authority_file = auth_dir.join("active-org.json");
    std::fs::write(&authority_file, b"before\n").unwrap();
    std::fs::set_permissions(&authority_file, std::fs::Permissions::from_mode(0o600)).unwrap();
    let authority_toml = auth_dir.join("config.toml");
    std::fs::write(&authority_toml, b"candidate = true\n").unwrap();
    std::fs::set_permissions(&authority_toml, std::fs::Permissions::from_mode(0o600)).unwrap();
    let port_probe = TcpListener::bind("127.0.0.1:0").unwrap();
    let sandbox_port = port_probe.local_addr().unwrap().port();
    drop(port_probe);
    let mut initial = runtime_journal_test_config(None, None);
    initial.sandbox_port = sandbox_port;
    config::save_to(&dir, &initial).unwrap();
    let state: SharedAppState = Arc::new(Mutex::new(AppState::default()));
    let authority =
        AuthorityTransaction::capture(&dir, &sandbox_home, &auth_dir, &initial, &state).unwrap();
    let ticket = authority.registered_snapshot_ticket().unwrap();
    let mut identity = OneClickTransactionIdentity {
        target_profile_id: "target".into(),
        runtime_fingerprint: "d".repeat(64),
        snapshot_ticket: ticket.clone(),
        previous_binding: None,
        gateway_terminal_handoff: None,
        prior_stop: config::RuntimePriorStopState::NotRequired,
    };
    let mut progress = OneClickJournalProgress::PreJournalAbort {
        registered_ticket: ticket,
        runtime_transaction: Box::new(None),
    };
    let science_bin = tmp.join("fake-science");
    std::fs::write(&science_bin, b"#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&science_bin, std::fs::Permissions::from_mode(0o700)).unwrap();
    let launch_runtime = crate::runtime::science::test_runtime_identity(science_bin);
    identity.runtime_fingerprint = launch_runtime.environment_transaction_id();
    test_begin_replayable_compensation(
        &dir,
        &authority,
        &state,
        &identity,
        &mut progress,
        launch_runtime,
        Some(ssh_stub_transaction),
        false,
    )
    .unwrap();
    std::fs::write(&authority_file, b"candidate\n").unwrap();
    let exact_compensation = config::load_from(&dir)
        .unwrap()
        .runtime_compensation
        .unwrap();
    config::update(&dir, |current| {
        current
            .runtime_compensation
            .as_mut()
            .unwrap()
            .compensation_id = "retargeted-compensation".into();
    })
    .unwrap();
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    let lifecycle = crate::lifecycle::Lifecycle::default();
    let retargeted = config::load_from(&dir).unwrap();
    assert!(replay_interrupted_one_click_compensation(
        app.handle(),
        &state,
        &lifecycle,
        None,
        &retargeted,
    )
    .unwrap_err()
    .contains("identity drifted or retargeted"));
    assert_eq!(std::fs::read(&authority_file).unwrap(), b"candidate\n");
    config::update(&dir, |current| {
        current.runtime_compensation = Some(exact_compensation.clone());
    })
    .unwrap();
    begin_one_click_compensation_step(
        &dir,
        &mut progress,
        config::RuntimeCompensationStep::ScienceCleanup,
    )
    .unwrap();
    for expected_step in [
        config::RuntimeCompensationStep::ScienceCleanup,
        config::RuntimeCompensationStep::SshCleanup,
    ] {
        let current = config::load_from(&dir).unwrap();
        assert_eq!(
            current
                .runtime_compensation
                .as_ref()
                .unwrap()
                .steps
                .iter()
                .find(|step| !step.outcome.is_terminal())
                .map(|step| step.step),
            Some(expected_step)
        );
        assert!(replay_interrupted_one_click_compensation(
            app.handle(),
            &state,
            &lifecycle,
            None,
            &current,
        )
        .unwrap());
    }
    assert_eq!(
        std::fs::read(&sandbox_ssh_config).unwrap(),
        preexisting_stub.as_bytes(),
        "durable SSH compensation must preserve the exact pre-one-click stub"
    );
    let recovery_root = authority.recovery_path().to_path_buf();
    drop(authority);
    let authority_v2 = recovery_root.join("authority-replay.v2.json");
    let authority_v1 = recovery_root.join("authority-replay.v1.json");
    std::fs::rename(&authority_v2, &authority_v1).unwrap();
    let legacy_before = std::fs::read(config::default_dir().join("config.json")).unwrap();
    assert!(replay_interrupted_one_click_compensation(
        app.handle(),
        &state,
        &lifecycle,
        None,
        &config::load_from(&dir).unwrap(),
    )
    .is_err());
    assert_eq!(
        std::fs::read(config::default_dir().join("config.json")).unwrap(),
        legacy_before
    );
    assert_eq!(std::fs::read(&authority_file).unwrap(), b"candidate\n");
    assert!(recovery_root.exists());
    std::fs::rename(&authority_v1, &authority_v2).unwrap();
    assert!(
        authority_v2.is_file(),
        "strict v2 authority manifest is restored"
    );
    assert_eq!(std::fs::read(&authority_file).unwrap(), b"candidate\n");
    assert_eq!(
        std::fs::read(&sandbox_ssh_config).unwrap(),
        preexisting_stub.as_bytes()
    );
    assert!(recovery_root.exists());
    let mismatch_before = std::fs::read(config::default_dir().join("config.json")).unwrap();
    let mismatch_journal = config::load_from(&dir)
        .unwrap()
        .runtime_compensation
        .unwrap();
    let durable =
        OneClickAuthoritySnapshot::load_durable(&state, &identity.snapshot_ticket).unwrap();
    assert!(durable
        .preflight_durable_authority_restore(
            &dir,
            &None,
            &mismatch_journal,
            &crate::runtime::sandbox_session::recovery::AuthorityRestoreReplayInputs {
                sandbox_port: 0,
                compensation_id: mismatch_journal.compensation_id.clone(),
                snapshot_ticket: identity.snapshot_ticket.clone(),
            },
        )
        .is_err());
    assert_eq!(
        std::fs::read(config::default_dir().join("config.json")).unwrap(),
        mismatch_before
    );
    drop(durable);
    assert_eq!(
        crate::lock(&state).sandbox_port,
        0,
        "durable authority replay must exercise a genuinely fresh AppState"
    );
    let exact_journal = config::load_from(&dir)
        .unwrap()
        .runtime_compensation
        .unwrap();
    let inputs = crate::runtime::sandbox_session::recovery::AuthorityRestoreReplayInputs {
        sandbox_port: initial.sandbox_port,
        compensation_id: exact_journal.compensation_id.clone(),
        snapshot_ticket: identity.snapshot_ticket.clone(),
    };
    let durable =
        OneClickAuthoritySnapshot::load_durable(&state, &identity.snapshot_ticket).unwrap();
    durable
        .preflight_durable_authority_restore(&dir, &None, &exact_journal, &inputs)
        .unwrap();
    {
        let _listener = TcpListener::bind(("127.0.0.1", initial.sandbox_port)).unwrap();
        assert!(durable
            .prepare_durable_authority_restore(&state, &inputs)
            .expect_err("fresh AppState with a live durable port must fail closed")
            .contains("port is not quiescent"));
    }
    let receipt = dir.join("science-managed-launch.v1.json");
    std::fs::write(&receipt, b"foreign receipt").unwrap();
    assert!(durable
        .prepare_durable_authority_restore(&state, &inputs)
        .expect_err("fresh AppState with a managed receipt must fail closed")
        .contains("receipt is present"));
    std::fs::remove_file(&receipt).unwrap();
    drop(durable);
    let reservation = recovery_root.join("authority-restore-reservation.v1.json");
    let stage = auth_dir.join(format!(
        ".csswitch-authority-{}-2-stage",
        identity.snapshot_ticket.managed_id
    ));
    let stage_observation = tmp.join("authority-stage-journal-observation");
    {
        let _crash =
            test_arm_durable_authority_stage_crash_after_copy(2, stage_observation.clone());
        assert!(replay_interrupted_one_click_compensation(
            app.handle(),
            &state,
            &lifecycle,
            None,
            &config::load_from(&dir).unwrap(),
        )
        .unwrap());
    }
    assert!(
        reservation.is_file()
            && stage.exists()
            && std::fs::read(&stage_observation).unwrap() == b"in_progress"
            && !recovery_root
                .join("authority-restore-effect.v1.2.staged.json")
                .exists(),
        "fresh production replay must reserve, advance AuthorityRestore, and crash after stage effect"
    );
    assert_eq!(
        crate::lock(&state).sandbox_port,
        0,
        "production fresh replay must not adopt the durable port as process-local ownership"
    );
    let exact_journal = config::load_from(&dir)
        .unwrap()
        .runtime_compensation
        .unwrap();
    let mut durable =
        OneClickAuthoritySnapshot::load_durable(&state, &identity.snapshot_ticket).unwrap();
    let tombstone = auth_dir.join(format!(
        ".csswitch-authority-{}-2-tombstone",
        identity.snapshot_ticket.managed_id
    ));
    {
        let _crash = test_arm_durable_authority_tombstone_crash_after_rename(2);
        assert!(durable
            .restore_durable_authority(&dir, &state, &None, &exact_journal, &inputs)
            .is_err());
    }
    assert!(
        recovery_root
            .join("authority-restore-effect.v1.2.tombstone-intent.json")
            .is_file()
            && !recovery_root
                .join("authority-restore-effect.v1.2.tombstoned.json")
                .exists()
            && tombstone.is_file()
            && std::fs::read(&tombstone).unwrap() == b"candidate\n"
            && !authority_file.exists(),
        "post-rename crash must retain intent and exact tombstone without a terminal marker"
    );
    drop(durable);
    let mut durable =
        OneClickAuthoritySnapshot::load_durable(&state, &identity.snapshot_ticket).unwrap();
    {
        let _crash = test_arm_durable_authority_outcome_crash_after_promotion(5);
        assert!(durable
            .restore_durable_authority(&dir, &state, &None, &exact_journal, &inputs)
            .is_err());
    }
    let toml_tombstone = auth_dir.join(format!(
        ".csswitch-authority-{}-5-tombstone",
        identity.snapshot_ticket.managed_id
    ));
    assert!(
        recovery_root
            .join("authority-restore-effect.v1.5.outcome-intent.json")
            .is_file()
            && !recovery_root
                .join("authority-restore-effect.v1.5.outcome.json")
                .exists()
            && authority_toml.is_file()
            && toml_tombstone.is_file()
            && std::fs::read(&toml_tombstone).unwrap() == b"candidate = true\n"
            && recovery_root
                .join("authority-restore-effect.v1.2.outcome.json")
                .is_file()
            && !tombstone.exists(),
        "post-promotion crash must retain OutcomeIntent, exact tombstone, and no Outcome"
    );
    drop(durable);
    let mut durable =
        OneClickAuthoritySnapshot::load_durable(&state, &identity.snapshot_ticket).unwrap();
    durable
        .restore_durable_authority(&dir, &state, &None, &exact_journal, &inputs)
        .unwrap();
    assert_eq!(
        std::fs::read(&authority_file).unwrap(),
        b"before\n",
        "durable staged/tombstone replay must restore the captured authority"
    );
    assert_eq!(
        crate::lock(&state).sandbox_port,
        0,
        "fresh replay must not silently publish a durable port as process-local ownership"
    );
    assert!(
        recovery_root
            .join("authority-restore-effect.v1.2.staged.json")
            .is_file()
            && recovery_root
                .join("authority-restore-effect.v1.2.tombstoned.json")
                .is_file()
            && recovery_root
                .join("authority-restore-effect.v1.2.outcome.json")
                .is_file(),
        "each per-target staged/tombstone/outcome boundary must be durable"
    );
    drop(durable);
    let mut durable =
        OneClickAuthoritySnapshot::load_durable(&state, &identity.snapshot_ticket).unwrap();
    durable
        .restore_durable_authority(&dir, &state, &None, &exact_journal, &inputs)
        .unwrap();
    assert_eq!(
        std::fs::read(&authority_file).unwrap(),
        b"before\n",
        "post-outcome cleanup may leave no tombstone and must fresh-replay idempotently"
    );
    let outcome_path = recovery_root.join("authority-restore-effect.v1.2.outcome.json");
    let outcome_bytes = std::fs::read(&outcome_path).unwrap();
    std::fs::remove_file(&outcome_path).unwrap();
    let tombstone_drift = durable
        .restore_durable_authority(&dir, &state, &None, &exact_journal, &inputs)
        .expect_err("missing tombstone before a durable outcome must fail closed");
    assert!(tombstone_drift.contains("tombstone drifted before outcome"));
    assert_eq!(
        std::fs::read(&authority_file).unwrap(),
        b"before\n",
        "tombstone drift must not overwrite the completed target"
    );
    std::fs::write(&outcome_path, outcome_bytes).unwrap();
    drop(durable);
    let mut durable =
        OneClickAuthoritySnapshot::load_durable(&state, &identity.snapshot_ticket).unwrap();
    let foreign_authority = auth_dir.join("foreign-authority");
    std::fs::write(&foreign_authority, b"cooperating-drift\n").unwrap();
    std::fs::rename(&foreign_authority, &authority_file).unwrap();
    let drift = durable
        .restore_durable_authority(&dir, &state, &None, &exact_journal, &inputs)
        .expect_err("unknown completed target identity must fail closed");
    assert!(drift.contains("identity drifted"));
    assert_eq!(
        std::fs::read(&authority_file).unwrap(),
        b"cooperating-drift\n",
        "durable replay must not overwrite a drifted cooperating writer"
    );
    assert!(recovery_root.exists());
    let _ = std::fs::remove_dir_all(tmp);
}
