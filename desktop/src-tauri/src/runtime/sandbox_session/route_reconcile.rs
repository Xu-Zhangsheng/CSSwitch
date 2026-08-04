//! Third-party Skill/route reconcile after Science is running.
use serde::Serialize;
use tauri::Runtime;

use crate::runtime::proxy_lifecycle::{current_skill_install_bridge_key, skill_install_bridge_dir};
use crate::runtime::science::{
    runtime_identity_is_current, sandbox_data_dir, SandboxScienceState, ScienceHostAdapter,
    ScienceRuntimeIdentity,
};
use crate::runtime::skill_install_bridge::{
    configure_third_party_after_science_start, inspect_while_science_running,
    invalidate_route_configuration, mark_route_configuration_current,
    route_configuration_is_current, RegistrationStatus,
};
use crate::{config, lock, SharedAppState};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SkillRouteRepairStatus {
    Synchronized,
    Deferred,
    RestartRequired,
    NotRequired,
    Warning,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct SkillRouteRepairOutcome {
    pub(crate) status: SkillRouteRepairStatus,
    pub(crate) message: String,
}

impl SkillRouteRepairOutcome {
    fn new(status: SkillRouteRepairStatus, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

pub(super) fn configure_third_party_best_effort<R: Runtime>(
    app: &tauri::AppHandle<R>,
    status: RegistrationStatus,
    data_dir: &std::path::Path,
    port: u16,
    runtime: &ScienceRuntimeIdentity,
    force: bool,
) -> RegistrationStatus {
    configure_third_party_best_effort_with(
        status,
        data_dir,
        runtime.version.as_deref(),
        force,
        || {
            let control_url = ScienceHostAdapter::url(port, runtime);
            configure_third_party_after_science_start(app, &control_url)
        },
    )
}

fn configure_third_party_best_effort_with<F>(
    status: RegistrationStatus,
    data_dir: &std::path::Path,
    science_version: Option<&str>,
    force: bool,
    configure_host: F,
) -> RegistrationStatus
where
    F: FnOnce() -> Result<(), String>,
{
    if !matches!(
        status,
        RegistrationStatus::Registered | RegistrationStatus::AlreadyRegistered
    ) {
        let _ = invalidate_route_configuration(data_dir);
        return status;
    }
    let Some(science_version) = science_version else {
        let _ = invalidate_route_configuration(data_dir);
        return RegistrationStatus::Warning(
            "Science 版本无法确认，未记录第三方能力配置状态".into(),
        );
    };
    let needs_configuration = force
        || matches!(status, RegistrationStatus::Registered)
        || match route_configuration_is_current(data_dir, science_version) {
            Ok(current) => !current,
            Err(error) => return RegistrationStatus::Warning(error),
        };
    if !needs_configuration {
        return status;
    }
    if let Err(error) = invalidate_route_configuration(data_dir) {
        return RegistrationStatus::Warning(error);
    }
    if let Err(error) = configure_host() {
        return RegistrationStatus::Warning(error);
    }
    match mark_route_configuration_current(data_dir, science_version) {
        Ok(()) => status,
        Err(error) => RegistrationStatus::Warning(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::path::PathBuf::from("/private/tmp").join(format!(
            "csswitch-r0-g-doctor-reconcile-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn r0_doctor_reconcile_freezes_marker_and_host_mutation_outcomes() {
        let data_dir = temp_dir();
        let fake_science = data_dir.join("fake-science");
        let url_call = data_dir.join("url-command-called");
        fs::write(
            &fake_science,
            format!(
                "#!/bin/sh\nprintf called > '{}'\nexit 0\n",
                url_call.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&fake_science, fs::Permissions::from_mode(0o700)).unwrap();
        let runtime = crate::runtime::science::test_runtime_identity(fake_science);
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        mark_route_configuration_current(&data_dir, "science-v1").unwrap();
        let skipped = configure_third_party_best_effort(
            app.handle(),
            RegistrationStatus::Warning("inspect failed".into()),
            &data_dir,
            19_951,
            &runtime,
            true,
        );
        assert!(matches!(skipped, RegistrationStatus::Warning(_)));
        assert!(!route_configuration_is_current(&data_dir, "test-only").unwrap());
        assert!(
            !url_call.exists(),
            "early status return must not execute Science url command"
        );

        mark_route_configuration_current(&data_dir, "test-only").unwrap();
        let retained_host_effect = data_dir.join("host-effect-retained");
        let partial_guard =
            crate::runtime::skill_install_bridge::test_arm_third_party_partial_failure(
                retained_host_effect.clone(),
            );
        let failed = configure_third_party_best_effort(
            app.handle(),
            RegistrationStatus::Registered,
            &data_dir,
            19_951,
            &runtime,
            true,
        );
        assert!(matches!(failed, RegistrationStatus::Warning(_)));
        assert!(retained_host_effect.is_file());
        assert!(url_call.is_file());
        assert!(!route_configuration_is_current(&data_dir, "test-only").unwrap());
        drop(partial_guard);

        let succeeded = configure_third_party_best_effort_with(
            RegistrationStatus::AlreadyRegistered,
            &data_dir,
            Some("test-only"),
            true,
            || Ok(()),
        );
        assert_eq!(succeeded, RegistrationStatus::AlreadyRegistered);
        assert!(route_configuration_is_current(&data_dir, "test-only").unwrap());
        drop(app);
        fs::remove_dir_all(data_dir).unwrap();
    }
}

/// Explicit Skill route repair: bypass the version cache and route marker
/// without starting Science or the proxy solely for this mutation intent.
pub(crate) fn force_third_party_reconcile<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SharedAppState,
) -> Result<SkillRouteRepairOutcome, String> {
    let cfg = config::load_from(&config::default_dir()).map_err(|error| error.to_string())?;
    let data_dir = sandbox_data_dir();
    let (remembered_runtime, version_cache) = {
        let st = lock(state);
        (st.science_runtime.clone(), st.science_version_cache.clone())
    };

    let (science_state, running_runtime) = match remembered_runtime {
        Some(mut runtime) => {
            if !runtime_identity_is_current(&runtime) {
                invalidate_route_configuration(&data_dir)?;
                return Ok(SkillRouteRepairOutcome::new(
                    SkillRouteRepairStatus::Deferred,
                    "Science 二进制文件已变化；已安排下次停止并启动后重新选择 runtime。",
                ));
            }
            let previous_version = runtime.version.clone();
            let refreshed = version_cache
                .force_refresh(&runtime.path)
                .ok_or("Science 版本强制复检失败")?;
            if previous_version
                .as_deref()
                .is_some_and(|version| version != refreshed)
            {
                invalidate_route_configuration(&data_dir)?;
                return Ok(SkillRouteRepairOutcome::new(
                    SkillRouteRepairStatus::Deferred,
                    "Science 二进制版本已变化；已安排下次停止并启动后重新配置 Skill 路由。",
                ));
            }
            runtime.version = Some(refreshed);
            let science_state = ScienceHostAdapter::probe_known(cfg.sandbox_port, &runtime);
            let running = (science_state == SandboxScienceState::RunningHealthy).then_some(runtime);
            (science_state, running)
        }
        None => {
            version_cache.clear();
            ScienceHostAdapter::probe_cached(cfg.sandbox_port, &version_cache)?
        }
    };

    if cfg.mode == "official" {
        return Ok(SkillRouteRepairOutcome::new(
            SkillRouteRepairStatus::NotRequired,
            "官方模式无需核验 CSSwitch 第三方 Skill 路由。",
        ));
    }
    match science_state {
        SandboxScienceState::Stopped => {
            invalidate_route_configuration(&data_dir)?;
            Ok(SkillRouteRepairOutcome::new(
                SkillRouteRepairStatus::Deferred,
                "Science 未运行；已安排下次一键开始重新核验 Skill 路由。",
            ))
        }
        SandboxScienceState::Unknown => {
            invalidate_route_configuration(&data_dir)?;
            Ok(SkillRouteRepairOutcome::new(
                SkillRouteRepairStatus::Warning,
                "无法确认 Science 实例身份；已使路由标记失效，未执行修复",
            ))
        }
        SandboxScienceState::RunningHealthy => {
            let runtime = running_runtime.ok_or("Science 运行身份缺失")?;
            let secret = { lock(state).secret.clone() };
            if secret.is_empty() {
                invalidate_route_configuration(&data_dir)?;
                return Ok(SkillRouteRepairOutcome::new(
                    SkillRouteRepairStatus::Deferred,
                    "当前代理身份不可用；已安排下次一键开始重新核验 Skill 路由。",
                ));
            }
            let bridge_dir = skill_install_bridge_dir(&secret)?;
            let bridge_key = match current_skill_install_bridge_key() {
                Ok(path) => path,
                Err(error) => {
                    invalidate_route_configuration(&data_dir)?;
                    return Ok(SkillRouteRepairOutcome::new(
                        SkillRouteRepairStatus::Deferred,
                        format!("Skill bridge 尚未就绪；已安排下次一键开始重新核验：{error}"),
                    ));
                }
            };
            let status = inspect_while_science_running(app, &data_dir, &bridge_dir, &bridge_key);
            let status = configure_third_party_best_effort(
                app,
                status,
                &data_dir,
                cfg.sandbox_port,
                &runtime,
                true,
            );
            {
                let mut st = lock(state);
                st.science_runtime = Some(runtime);
                st.science_confirmed_stopped = None;
            }
            match status {
                RegistrationStatus::AlreadyRegistered | RegistrationStatus::Registered => {
                    Ok(SkillRouteRepairOutcome::new(
                        SkillRouteRepairStatus::Synchronized,
                        "Skill 路由已强制核验并同步。",
                    ))
                }
                RegistrationStatus::RestartRequired => Ok(SkillRouteRepairOutcome::new(
                    SkillRouteRepairStatus::RestartRequired,
                    "Skill 路由文件需要重启 Science 后加载；状态标记已失效。",
                )),
                RegistrationStatus::Warning(message) => Ok(SkillRouteRepairOutcome::new(
                    SkillRouteRepairStatus::Warning,
                    format!("Skill 路由核验未完成：{message}"),
                )),
            }
        }
    }
}
