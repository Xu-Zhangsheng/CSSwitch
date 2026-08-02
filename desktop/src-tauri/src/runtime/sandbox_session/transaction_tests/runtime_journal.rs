use super::*;

#[test]
fn runtime_journal_advances_in_place_and_retargets_without_secrets() {
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
            ..Default::default()
        },
    )
    .unwrap();

    advance_runtime_transaction(&dir, "new", Some(previous.clone()), "start_gateway").unwrap();
    let first_record = config::load_from(&dir)
        .unwrap()
        .runtime_transaction
        .unwrap();
    let first = first_record.as_v1().unwrap();
    assert_eq!(first.target_profile_id, "new");
    assert_eq!(first.stage, "start_gateway");
    assert_eq!(first.previous_binding, Some(previous.clone()));

    let runtime_id = "a".repeat(64);
    let environment_stage = format!("{SCIENCE_ENVIRONMENT_PENDING_STAGE_PREFIX}{runtime_id}");
    advance_runtime_transaction(&dir, "new", Some(previous.clone()), &environment_stage).unwrap();
    let second_record = config::load_from(&dir)
        .unwrap()
        .runtime_transaction
        .unwrap();
    let second = second_record.as_v1().unwrap();
    assert_eq!(second.transaction_id, first.transaction_id);
    assert_eq!(second.stage, environment_stage);
    assert_eq!(
        interrupted_science_environment_runtime_id(&second.stage),
        Some(runtime_id.as_str())
    );
    assert!(interrupted_science_environment_runtime_id(
        "start_science_environment_pending:not-a-fingerprint"
    )
    .is_none());
    assert!(
        runtime_transaction_requires_snapshot_preservation("start_science")
            && runtime_transaction_requires_snapshot_preservation(
                "start_science_environment_pending"
            )
            && runtime_transaction_requires_snapshot_preservation(&environment_stage)
            && !runtime_transaction_requires_snapshot_preservation("recover_interrupted_gateway"),
        "legacy and fingerprinted environment-exposure stages must fail closed"
    );
    for listener_state in [
        "stopped-no-gateway",
        "running-no-gateway",
        "stopped-managed-gateway",
        "running-managed-gateway",
    ] {
        let legacy = validate_interrupted_science_transaction_entry(Some("start_science"), None)
            .expect_err("legacy 0.8.3 start_science must never authorize an automatic spawn");
        assert_eq!(
            legacy.projected_recovery(),
            crate::runtime::failure::ProjectedRecovery::environment_uncertain_manual()
        );
        assert!(
            legacy.to_string().contains("environment_uncertain")
                && legacy.to_string().contains("newer_runtime_required")
                && legacy.to_string().contains("manual_recovery_required"),
            "legacy oracle {listener_state} must fail closed: {legacy}"
        );
    }
    let authority_stage = format!("{AUTHORITY_SNAPSHOT_ACTIVE_STAGE_PREFIX}{runtime_id}");
    assert_eq!(
        interrupted_science_environment_runtime_id(&authority_stage),
        Some(runtime_id.as_str())
    );
    let authority =
        validate_interrupted_science_transaction_entry(Some(&authority_stage), Some(&runtime_id))
            .expect_err("active authority snapshot must require explicit recovery");
    assert_eq!(
        authority.projected_recovery(),
        crate::runtime::failure::ProjectedRecovery::MANUAL_RECOVERY_REQUIRED
    );

    advance_runtime_transaction(&dir, "newer", Some(previous), "start_gateway").unwrap();
    let retargeted_record = config::load_from(&dir)
        .unwrap()
        .runtime_transaction
        .unwrap();
    let retargeted = retargeted_record.as_v1().unwrap();
    assert_ne!(retargeted.transaction_id, second.transaction_id);
    assert_eq!(retargeted.target_profile_id, "newer");
    let encoded = serde_json::to_string(&retargeted).unwrap();
    assert!(!encoded.contains("api_key"));
    assert!(!encoded.contains("base_url"));

    let typed = config::RuntimeTransactionRecord::V2(config::RuntimeTransactionV2 {
        schema_version: config::RUNTIME_TRANSACTION_SCHEMA_VERSION_V2,
        transaction_id: "typed-retarget".into(),
        operation: config::RuntimeTransactionOperation::OneClick,
        target_profile_id: "other-target".into(),
        phase: config::RuntimeTransactionPhase::StartGateway,
        runtime_fingerprint: Some("b".repeat(64)),
        environment_exposure: config::RuntimeEnvironmentExposure::NotExposed,
        snapshot_ticket: Some(config::RuntimeSnapshotTicket {
            managed_id: ".one-click-rollback-fedcba9876543210fedcba9876543210".into(),
        }),
        previous_binding: None,
        previous_gateway: None,
        compensation: config::RuntimeCompensationState::NotStarted,
        gateway_stop_outcome: config::RuntimeGatewayStopOutcome::NotAttempted,
    });
    config::update(&dir, |cfg| cfg.runtime_transaction = Some(typed.clone())).unwrap();
    let before_typed_advance = std::fs::read(dir.join("config.json")).unwrap();
    let error = advance_runtime_transaction(&dir, "newer", None, "verify_science_catalog")
        .expect_err("a V1 writer must not overwrite a differently targeted V2 journal");
    assert!(error.contains("preserved the typed transaction"));
    assert_eq!(
        std::fs::read(dir.join("config.json")).unwrap(),
        before_typed_advance
    );
    assert_eq!(
        config::load_from(&dir).unwrap().runtime_transaction,
        Some(typed.clone())
    );
    for guarded_writer in [clear_runtime_transaction(&dir)] {
        assert!(guarded_writer
            .expect_err("every remaining V1 writer must preserve V2")
            .contains("preserved the typed transaction"));
        assert_eq!(
            std::fs::read(dir.join("config.json")).unwrap(),
            before_typed_advance
        );
        assert_eq!(
            config::load_from(&dir).unwrap().runtime_transaction,
            Some(typed.clone())
        );
    }
    let _ = std::fs::remove_dir_all(dir);
}
