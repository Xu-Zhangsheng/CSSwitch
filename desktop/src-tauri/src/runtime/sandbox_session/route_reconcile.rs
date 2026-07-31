//! Third-party Skill/route reconcile after Science is running.
use tauri::Runtime;

use crate::runtime::proxy_lifecycle::{current_skill_install_bridge_key, skill_install_bridge_dir};
use crate::runtime::science::{
    probe_known_runtime, probe_sandbox_runtime_cached, runtime_identity_is_current,
    sandbox_data_dir, sandbox_url, SandboxScienceState, ScienceRuntimeIdentity,
};
use crate::runtime::skill_install_bridge::{
    configure_third_party_after_science_start, inspect_while_science_running,
    invalidate_route_configuration, mark_route_configuration_current,
    route_configuration_is_current, RegistrationStatus,
};
use crate::{config, lock, SharedAppState};

pub(super) fn configure_third_party_best_effort<R: Runtime>(
    app: &tauri::AppHandle<R>,
    status: RegistrationStatus,
    data_dir: &std::path::Path,
    port: u16,
    runtime: &ScienceRuntimeIdentity,
    force: bool,
) -> RegistrationStatus {
    let control_url = sandbox_url(port, runtime);
    configure_third_party_best_effort_with(
        status,
        data_dir,
        runtime.version.as_deref(),
        force,
        || configure_third_party_after_science_start(app, &control_url),
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
        mark_route_configuration_current(&data_dir, "science-v1").unwrap();
        let skipped = configure_third_party_best_effort_with(
            RegistrationStatus::Warning("inspect failed".into()),
            &data_dir,
            Some("science-v1"),
            true,
            || panic!("untrusted inspect result must not mutate the host"),
        );
        assert!(matches!(skipped, RegistrationStatus::Warning(_)));
        assert!(!route_configuration_is_current(&data_dir, "science-v1").unwrap());

        mark_route_configuration_current(&data_dir, "science-v1").unwrap();
        let retained_host_effect = data_dir.join("host-effect-retained");
        let failed = configure_third_party_best_effort_with(
            RegistrationStatus::Registered,
            &data_dir,
            Some("science-v1"),
            true,
            || {
                fs::write(&retained_host_effect, b"partial host mutation").unwrap();
                Err("host configure failed after mutation".into())
            },
        );
        assert!(matches!(failed, RegistrationStatus::Warning(_)));
        assert!(retained_host_effect.is_file());
        assert!(!route_configuration_is_current(&data_dir, "science-v1").unwrap());

        let succeeded = configure_third_party_best_effort_with(
            RegistrationStatus::AlreadyRegistered,
            &data_dir,
            Some("science-v1"),
            true,
            || Ok(()),
        );
        assert_eq!(succeeded, RegistrationStatus::AlreadyRegistered);
        assert!(route_configuration_is_current(&data_dir, "science-v1").unwrap());
        fs::remove_dir_all(data_dir).unwrap();
    }
}

/// Explicit doctor action: bypass the version cache and route marker without
/// starting Science or the proxy solely for diagnostics.
pub(crate) fn force_third_party_reconcile<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SharedAppState,
) -> Result<String, String> {
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
                return Ok(
                    "Science 二进制文件已变化；已安排下次停止并启动后重新选择 runtime。".into(),
                );
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
                return Ok(
                    "Science 二进制版本已变化；已安排下次停止并启动后重新配置 Skill 路由。".into(),
                );
            }
            runtime.version = Some(refreshed);
            let science_state = probe_known_runtime(cfg.sandbox_port, &runtime);
            let running = (science_state == SandboxScienceState::RunningHealthy).then_some(runtime);
            (science_state, running)
        }
        None => {
            version_cache.clear();
            probe_sandbox_runtime_cached(cfg.sandbox_port, &version_cache)?
        }
    };

    if cfg.mode == "official" {
        return Ok("官方模式无需核验 CSSwitch 第三方 Skill 路由。".into());
    }
    match science_state {
        SandboxScienceState::Stopped => {
            invalidate_route_configuration(&data_dir)?;
            Ok("Science 未运行；已安排下次一键开始重新核验 Skill 路由。".into())
        }
        SandboxScienceState::Unknown => {
            invalidate_route_configuration(&data_dir)?;
            Err("无法确认 Science 实例身份；已使路由标记失效，未执行修复".into())
        }
        SandboxScienceState::RunningHealthy => {
            let runtime = running_runtime.ok_or("Science 运行身份缺失")?;
            let secret = { lock(state).secret.clone() };
            if secret.is_empty() {
                invalidate_route_configuration(&data_dir)?;
                return Ok("当前代理身份不可用；已安排下次一键开始重新核验 Skill 路由。".into());
            }
            let bridge_dir = skill_install_bridge_dir(&secret)?;
            let bridge_key = match current_skill_install_bridge_key() {
                Ok(path) => path,
                Err(error) => {
                    invalidate_route_configuration(&data_dir)?;
                    return Ok(format!(
                        "Skill bridge 尚未就绪；已安排下次一键开始重新核验：{error}"
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
                    Ok("Skill 路由已强制核验并同步。".into())
                }
                RegistrationStatus::RestartRequired => {
                    Ok("Skill 路由文件需要重启 Science 后加载；状态标记已失效。".into())
                }
                RegistrationStatus::Warning(message) => {
                    Ok(format!("Skill 路由核验未完成：{message}"))
                }
            }
        }
    }
}
