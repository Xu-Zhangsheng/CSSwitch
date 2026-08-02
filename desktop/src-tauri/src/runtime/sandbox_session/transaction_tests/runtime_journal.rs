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
    config::save_to(
        &dir,
        &Config {
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
    let _ = std::fs::remove_dir_all(dir);
}
