use super::*;

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

    clear_one_click_transaction(&dir, &identity, &progress).unwrap();
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
        &identity.target_profile_id,
        identity.previous_binding.as_ref(),
    ));
    assert!(!healthy_reopen_transaction_matches(
        Some(&profile_switch_record),
        Some(&mismatched_profile_switch),
        &identity.target_profile_id,
        identity.previous_binding.as_ref(),
    ));
    assert!(!healthy_reopen_transaction_matches(
        None,
        Some(typed_profile_switch),
        &identity.target_profile_id,
        identity.previous_binding.as_ref(),
    ));
    assert!(!healthy_reopen_transaction_matches(
        Some(&profile_switch_record),
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
        &identity.target_profile_id,
        identity.previous_binding.as_ref(),
    ));
    assert!(healthy_reopen_transaction_matches(
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
        commit_healthy_reopen_binding(&dir, Some(typed_profile_switch), &committed_binding)
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
    commit_healthy_reopen_binding(&dir, Some(typed_profile_switch), &committed_binding).unwrap();
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
}
