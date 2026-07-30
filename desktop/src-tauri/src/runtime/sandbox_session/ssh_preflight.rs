//! SSH preflight for one-click / running-bridge validation.
use crate::config;
use crate::runtime::system::asset_root;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tauri::Runtime;

pub(super) fn validate_system_ssh_wrapper_path<R: Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<PathBuf, String> {
    #[cfg(test)]
    let wrapper_override =
        std::env::var_os("CSSWITCH_TEST_SSH_WRAPPER_OVERRIDE").map(PathBuf::from);
    #[cfg(not(test))]
    let wrapper_override: Option<PathBuf> = None;
    let wrapper = match wrapper_override {
        Some(wrapper) => wrapper,
        None => {
            let root = asset_root(app).ok_or("打包的 CSSwitch SSH bridge 缺失")?;
            let scripts = root.join("scripts");
            let wrapper_dir = scripts.join("ssh-bridge");
            let wrapper = wrapper_dir.join("ssh");
            for path in [&root, &scripts, &wrapper_dir] {
                let metadata = std::fs::symlink_metadata(path)
                    .map_err(|_| "打包的 CSSwitch SSH bridge 缺失".to_string())?;
                if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
                    return Err("打包的 CSSwitch SSH bridge 不是安全的可执行文件".into());
                }
            }
            wrapper
        }
    };
    let metadata = std::fs::symlink_metadata(&wrapper)
        .map_err(|_| "打包的 CSSwitch SSH bridge 缺失".to_string())?;
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_file()
        || metadata.len() > 128 * 1024
        || metadata.permissions().mode() & 0o111 == 0
    {
        return Err("打包的 CSSwitch SSH bridge 不是安全的可执行文件".into());
    }
    Ok(wrapper)
}

pub(super) fn validate_running_system_ssh_bridge<R: Runtime>(
    app: &tauri::AppHandle<R>,
    sandbox_home: &Path,
) -> Result<(), String> {
    let _validated_wrapper =
        crate::runtime::sandbox_session::validate_system_ssh_wrapper_path(app)?;
    let expected_hosts = crate::runtime::ssh_bridge::validate_science_ssh_bridge(sandbox_home)?;
    crate::runtime::settings::validate_managed_sandbox_ssh_stub(sandbox_home, &expected_hosts)?;
    Ok(())
}

pub(super) fn prevalidate_one_click_system_ssh<R: Runtime>(
    app: &tauri::AppHandle<R>,
    cfg: &config::Config,
    sandbox_home: &Path,
) -> Result<Vec<String>, String> {
    let expected_hosts = crate::runtime::ssh_bridge::prevalidate_science_ssh_bridge(
        sandbox_home,
        cfg.reuse_system_ssh,
    )?;
    crate::runtime::settings::prevalidate_sandbox_ssh_stub(
        sandbox_home,
        &expected_hosts,
        cfg.reuse_system_ssh,
    )?;
    if cfg.reuse_system_ssh {
        let _validated_wrapper =
            crate::runtime::sandbox_session::validate_system_ssh_wrapper_path(app)?;
    }
    Ok(expected_hosts)
}
