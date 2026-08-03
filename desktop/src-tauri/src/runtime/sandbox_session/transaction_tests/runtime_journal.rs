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

#[test]
fn gateway_terminal_handoff_prior_stop_and_finalize_are_exact_replayable_transitions() {
    let dir = isolated_tmpdir("gateway-prior-finalize-protocol");
    let previous = RuntimeBindingCommit {
        profile_id: "prior".into(),
        route_fp: "prior-route".into(),
        catalog_fp: "prior-catalog".into(),
        binding_fp: "prior-binding".into(),
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
        launch_receipt_digest: "b".repeat(64),
    };
    let replacement = RuntimeBindingCommit {
        profile_id: "replacement".into(),
        route_fp: "replacement-route".into(),
        catalog_fp: "replacement-catalog".into(),
        binding_fp: "replacement-binding".into(),
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
            None,
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
        None,
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
        profile_switch_handoff: None,
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
    };
    begin_one_click_finalize(
        &dir,
        &identity,
        &mut progress,
        config::RuntimeFinalizeAction::CommitBinding {
            binding: committed.clone(),
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
            action: config::RuntimeFinalizeAction::CommitBinding { binding },
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
        profile_switch_handoff: None,
        gateway_terminal_handoff: None,
        prior_stop: config::RuntimePriorStopState::NotRequired,
    };
    let mut progress = OneClickJournalProgress::PreJournalAbort {
        registered_ticket: snapshot_ticket.clone(),
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

    let profile_switch_transaction_id = "profile-switch-handoff".to_string();
    let profile_switch_record =
        config::RuntimeTransactionRecord::V2(config::RuntimeTransactionV2 {
            schema_version: config::RUNTIME_TRANSACTION_SCHEMA_VERSION_V2,
            transaction_id: profile_switch_transaction_id.clone(),
            operation: config::RuntimeTransactionOperation::ProfileSwitch,
            target_profile_id: identity.target_profile_id.clone(),
            phase: config::RuntimeTransactionPhase::StartFormalGateway,
            runtime_fingerprint: None,
            environment_exposure: config::RuntimeEnvironmentExposure::NotExposed,
            snapshot_ticket: None,
            previous_binding: identity.previous_binding.clone(),
            previous_gateway: None,
            compensation: config::RuntimeCompensationState::NotStarted,
            gateway_stop_outcome: config::RuntimeGatewayStopOutcome::NotAttempted,
            prior_stop: config::RuntimePriorStopState::NotRequired,
            finalize: config::RuntimeFinalizeState::NotStarted,
        });
    config::update(&dir, |current| {
        current.runtime_transaction = Some(profile_switch_record.clone());
    })
    .unwrap();
    let before_retarget_rejection = std::fs::read(dir.join("config.json")).unwrap();
    let mut retargeted_handoff_identity = identity.clone();
    let mut retargeted_profile_switch = profile_switch_record.as_v2().unwrap().clone();
    retargeted_profile_switch.transaction_id = "different-profile-switch-transaction".into();
    retargeted_handoff_identity.profile_switch_handoff = Some(retargeted_profile_switch);
    let mut retargeted_handoff_progress = OneClickJournalProgress::PreJournalAbort {
        registered_ticket: snapshot_ticket.clone(),
    };
    let handoff_error = write_one_click_checkpoint(
        &dir,
        &retargeted_handoff_identity,
        &mut retargeted_handoff_progress,
        config::RuntimeTransactionPhase::StartGateway,
    )
    .expect_err("a replaced profile-switch transaction must not be handed off");
    assert!(handoff_error.contains("identity changed"));
    assert_eq!(
        std::fs::read(dir.join("config.json")).unwrap(),
        before_retarget_rejection
    );
    let mut handoff_identity = identity.clone();
    handoff_identity.profile_switch_handoff = Some(profile_switch_record.as_v2().unwrap().clone());

    config::update(&dir, |current| {
        current
            .runtime_transaction
            .as_mut()
            .and_then(config::RuntimeTransactionRecord::as_v2_mut)
            .unwrap()
            .previous_gateway = Some(config::GatewayRuntimeJournalIdentity {
            provider: "retargeted".into(),
            shim: "off".into(),
            launch_id: "retargeted-launch".into(),
            provider_contract_id: "retargeted-contract".into(),
            provider_contract_digest: "retargeted-digest".into(),
            catalog_fp: "retargeted-catalog".into(),
        });
    })
    .unwrap();
    let before_field_drift_rejection = std::fs::read(dir.join("config.json")).unwrap();
    let mut field_drift_progress = OneClickJournalProgress::PreJournalAbort {
        registered_ticket: snapshot_ticket.clone(),
    };
    let field_drift_error = write_one_click_checkpoint(
        &dir,
        &handoff_identity,
        &mut field_drift_progress,
        config::RuntimeTransactionPhase::StartGateway,
    )
    .expect_err("same-id previous Gateway drift must fail closed");
    assert!(field_drift_error.contains("identity changed"));
    assert_eq!(
        std::fs::read(dir.join("config.json")).unwrap(),
        before_field_drift_rejection
    );

    for (label, replacement) in [
        ("missing", None),
        (
            "v1-regression",
            Some(
                config::RuntimeTransactionJournal {
                    transaction_id: profile_switch_transaction_id.clone(),
                    target_profile_id: identity.target_profile_id.clone(),
                    stage: "start_formal_gateway".into(),
                    previous_binding: identity.previous_binding.clone(),
                    previous_gateway: None,
                }
                .into(),
            ),
        ),
    ] {
        config::update(&dir, |current| {
            current.runtime_transaction = replacement.clone();
        })
        .unwrap();
        let before = std::fs::read(dir.join("config.json")).unwrap();
        let mut rejected_progress = OneClickJournalProgress::PreJournalAbort {
            registered_ticket: snapshot_ticket.clone(),
        };
        let error = write_one_click_checkpoint(
            &dir,
            &handoff_identity,
            &mut rejected_progress,
            config::RuntimeTransactionPhase::StartGateway,
        )
        .expect_err("a disappeared or regressed profile-switch handoff must fail closed");
        assert!(
            error.contains("handoff journal disappeared or regressed"),
            "{label}"
        );
        assert_eq!(std::fs::read(dir.join("config.json")).unwrap(), before);
    }

    config::update(&dir, |current| {
        current.runtime_transaction = Some(profile_switch_record.clone());
    })
    .unwrap();
    let typed_profile_switch = profile_switch_record.as_v2().unwrap();
    let mut mismatched_profile_switch = typed_profile_switch.clone();
    mismatched_profile_switch.compensation = config::RuntimeCompensationState::InProgress;
    assert!(healthy_reopen_transaction_matches(
        Some(&profile_switch_record),
        Some(typed_profile_switch),
        None,
        &identity.target_profile_id,
        identity.previous_binding.as_ref(),
    ));
    assert!(!healthy_reopen_transaction_matches(
        Some(&profile_switch_record),
        Some(&mismatched_profile_switch),
        None,
        &identity.target_profile_id,
        identity.previous_binding.as_ref(),
    ));
    assert!(!healthy_reopen_transaction_matches(
        None,
        Some(typed_profile_switch),
        None,
        &identity.target_profile_id,
        identity.previous_binding.as_ref(),
    ));
    assert!(!healthy_reopen_transaction_matches(
        Some(&profile_switch_record),
        None,
        None,
        &identity.target_profile_id,
        identity.previous_binding.as_ref(),
    ));
    let legacy_profile_switch: config::RuntimeTransactionRecord =
        config::RuntimeTransactionJournal {
            transaction_id: profile_switch_transaction_id.clone(),
            target_profile_id: identity.target_profile_id.clone(),
            stage: "start_formal_gateway".into(),
            previous_binding: identity.previous_binding.clone(),
            previous_gateway: None,
        }
        .into();
    assert_eq!(
        resolve_profile_switch_handoff(
            Some(&profile_switch_record),
            Some(typed_profile_switch),
            true,
            &identity.target_profile_id,
            identity.previous_binding.as_ref(),
        )
        .unwrap()
        .as_ref(),
        Some(typed_profile_switch)
    );
    for regressed in [None, Some(&legacy_profile_switch)] {
        assert!(resolve_profile_switch_handoff(
            regressed,
            Some(typed_profile_switch),
            true,
            &identity.target_profile_id,
            identity.previous_binding.as_ref(),
        )
        .is_err());
    }
    assert!(resolve_profile_switch_handoff(
        Some(&profile_switch_record),
        Some(&mismatched_profile_switch),
        true,
        &identity.target_profile_id,
        identity.previous_binding.as_ref(),
    )
    .is_err());
    assert!(resolve_profile_switch_handoff(
        Some(&profile_switch_record),
        Some(typed_profile_switch),
        false,
        &identity.target_profile_id,
        identity.previous_binding.as_ref(),
    )
    .is_err());
    assert!(resolve_profile_switch_handoff(
        Some(&profile_switch_record),
        None,
        false,
        &identity.target_profile_id,
        identity.previous_binding.as_ref(),
    )
    .is_err());
    assert_eq!(
        resolve_profile_switch_handoff(
            Some(&legacy_profile_switch),
            None,
            false,
            &identity.target_profile_id,
            identity.previous_binding.as_ref(),
        )
        .unwrap(),
        None
    );
    assert!(healthy_reopen_transaction_matches(
        Some(&legacy_profile_switch),
        None,
        None,
        &identity.target_profile_id,
        identity.previous_binding.as_ref(),
    ));
    assert!(healthy_reopen_transaction_matches(
        None,
        None,
        None,
        &identity.target_profile_id,
        identity.previous_binding.as_ref(),
    ));
    assert_eq!(
        typed_profile_switch.previous_binding.as_ref(),
        identity.previous_binding.as_ref()
    );

    config::update(&dir, |current| {
        current
            .runtime_transaction
            .as_mut()
            .and_then(config::RuntimeTransactionRecord::as_v2_mut)
            .unwrap()
            .compensation = config::RuntimeCompensationState::InProgress;
    })
    .unwrap();
    let before_commit_drift_rejection = std::fs::read(dir.join("config.json")).unwrap();
    let committed_binding = RuntimeBindingCommit {
        profile_id: identity.target_profile_id.clone(),
        route_fp: "committed-route-fp".into(),
        catalog_fp: "committed-catalog-fp".into(),
        binding_fp: "committed-binding-fp".into(),
    };
    let commit_error =
        commit_healthy_reopen_binding(&dir, Some(typed_profile_switch), None, &committed_binding)
            .expect_err("same-id drift between healthy read and binding commit must fail closed");
    assert!(commit_error.contains("retargeted healthy reopen"));
    assert_eq!(
        std::fs::read(dir.join("config.json")).unwrap(),
        before_commit_drift_rejection
    );

    config::update(&dir, |current| {
        current.runtime_transaction = Some(profile_switch_record.clone());
    })
    .unwrap();
    commit_healthy_reopen_binding(&dir, Some(typed_profile_switch), None, &committed_binding)
        .unwrap();
    let committed_config = config::load_from(&dir).unwrap();
    assert_eq!(
        committed_config.runtime_binding.as_ref(),
        Some(&committed_binding)
    );
    assert!(committed_config.runtime_transaction.is_none());

    config::update(&dir, |current| {
        current.runtime_binding = identity.previous_binding.clone();
        current.runtime_transaction = Some(profile_switch_record.clone());
    })
    .unwrap();
    let mut handoff_progress = OneClickJournalProgress::PreJournalAbort {
        registered_ticket: snapshot_ticket.clone(),
    };
    write_one_click_checkpoint(
        &dir,
        &handoff_identity,
        &mut handoff_progress,
        config::RuntimeTransactionPhase::StartGateway,
    )
    .unwrap();
    let handed_off = config::load_from(&dir)
        .unwrap()
        .runtime_transaction
        .unwrap();
    let handed_off = handed_off.as_v2().unwrap();
    assert_eq!(
        handed_off.operation,
        config::RuntimeTransactionOperation::OneClick
    );
    assert_ne!(handed_off.transaction_id, profile_switch_transaction_id);
    assert_eq!(
        handoff_progress.transaction_id(),
        Some(handed_off.transaction_id.as_str())
    );
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
        profile_switch_handoff: None,
        gateway_terminal_handoff: None,
        prior_stop: config::RuntimePriorStopState::NotRequired,
    };
    let mut compensation_progress = OneClickJournalProgress::PreJournalAbort {
        registered_ticket: compensation_ticket,
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
        &compensation_progress,
        launch_runtime,
    )
    .expect_err("production compensation must fail closed after journal drift");
    assert!(error.to_string().contains("compensation_restore_failed"));
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
