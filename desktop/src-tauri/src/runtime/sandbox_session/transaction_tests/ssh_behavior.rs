use super::*;

#[test]
#[ignore = "explicit Acceptance-boundary optional SSH feature-off smoke; temp HOME only"]
fn reuse_system_ssh_false_does_not_require_packaged_wrapper_or_system_home() {
    let _env_lock = TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut env = ScopedEnv::new();
    let tmp = isolated_tmpdir("ssh-feature-off");
    let missing_wrapper = tmp.join("missing-wrapper");
    env.remove("HOME");
    env.set("CSSWITCH_TEST_SSH_WRAPPER_OVERRIDE", &missing_wrapper);
    let cfg = Config {
        reuse_system_ssh: false,
        ..Default::default()
    };
    let sandbox_home = tmp.join("sandbox/home");
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    let result = prevalidate_one_click_system_ssh(&app.handle().clone(), &cfg, &sandbox_home);
    assert!(
            result.is_ok(),
            "disabled SSH must not require HOME, system config, host parsing, or packaged wrapper: {result:?}"
        );
    let _ = fs::remove_dir_all(tmp);
}

#[test]
#[ignore = "explicit Acceptance-boundary SSH write-authority prevalidation; temp HOME only"]
fn enabled_ssh_prevalidation_rejects_unwritable_science_authority() {
    let _env_lock = TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut env = ScopedEnv::new();
    let tmp = isolated_tmpdir("ssh-unwritable-authority");
    let home = tmp.join("home");
    let sandbox_home = tmp.join("sandbox/home");
    let science_data = sandbox_home.join(".claude-science");
    fs::create_dir_all(home.join(".ssh")).unwrap();
    fs::write(home.join(".ssh/config"), b"Host isolated-test-host\n").unwrap();
    fs::set_permissions(home.join(".ssh/config"), fs::Permissions::from_mode(0o600)).unwrap();
    fs::create_dir_all(&science_data).unwrap();
    fs::write(science_data.join("config.toml"), b"quiet_logs = true\n").unwrap();
    fs::set_permissions(
        science_data.join("config.toml"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    fs::set_permissions(&science_data, fs::Permissions::from_mode(0o500)).unwrap();
    let wrapper = tmp.join("ssh-wrapper");
    fs::write(&wrapper, b"#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
    env.set("HOME", &home);
    env.set("CSSWITCH_TEST_SSH_WRAPPER_OVERRIDE", &wrapper);
    let cfg = Config {
        reuse_system_ssh: true,
        ..Default::default()
    };
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    let system_before = tree(&home.join(".ssh"));
    let science_before = tree(&science_data);
    let sandbox_stub_before = tree(&sandbox_home.join(".ssh"));
    let result = prevalidate_one_click_system_ssh(&app.handle().clone(), &cfg, &sandbox_home);
    let system_after = tree(&home.join(".ssh"));
    let science_after = tree(&science_data);
    let sandbox_stub_after = tree(&sandbox_home.join(".ssh"));
    let probe_residue = fs::read_dir(&science_data)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .filter(|name| {
            let name = name.to_string_lossy();
            name.starts_with('.') && name.contains("tmp")
        })
        .collect::<Vec<_>>();
    fs::set_permissions(&science_data, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(
            result
                .as_ref()
                .is_err_and(|error| error == "隔离 Science SSH authority 不可写"),
            "enabled SSH must reject a statically unwritable bridge authority before OAuth or journal mutation"
        );
    assert_eq!(system_after, system_before);
    assert_eq!(science_after, science_before);
    assert_eq!(sandbox_stub_after, sandbox_stub_before);
    assert!(
        probe_residue.is_empty(),
        "read-only prevalidation must not create a write probe or temp residue"
    );
    let _ = fs::remove_dir_all(tmp);
}
