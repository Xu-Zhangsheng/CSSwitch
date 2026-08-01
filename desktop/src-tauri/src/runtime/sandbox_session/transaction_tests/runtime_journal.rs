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
    let first = config::load_from(&dir)
        .unwrap()
        .runtime_transaction
        .unwrap();
    assert_eq!(first.target_profile_id, "new");
    assert_eq!(first.stage, "start_gateway");
    assert_eq!(first.previous_binding, Some(previous.clone()));

    let runtime_id = "a".repeat(64);
    let environment_stage = format!("{SCIENCE_ENVIRONMENT_PENDING_STAGE_PREFIX}{runtime_id}");
    advance_runtime_transaction(&dir, "new", Some(previous.clone()), &environment_stage).unwrap();
    let second = config::load_from(&dir)
        .unwrap()
        .runtime_transaction
        .unwrap();
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
    let retargeted = config::load_from(&dir)
        .unwrap()
        .runtime_transaction
        .unwrap();
    assert_ne!(retargeted.transaction_id, second.transaction_id);
    assert_eq!(retargeted.target_profile_id, "newer");
    let encoded = serde_json::to_string(&retargeted).unwrap();
    assert!(!encoded.contains("api_key"));
    assert!(!encoded.contains("base_url"));
    let _ = std::fs::remove_dir_all(dir);
}
