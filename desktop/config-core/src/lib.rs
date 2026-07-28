//! App-independent, atomic CSSwitch configuration store.
//!
//! The store preserves unknown fields and never exposes credential values in
//! its public projections. It intentionally accepts an explicit data root so
//! the external bridge can be upgraded independently of CSSwitch.app.

use csswitch_control_core::{config_fingerprint, schema_supported};
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

const CONFIG_FILE: &str = "config.json";

#[derive(Debug)]
pub enum StoreError { Io(String), Invalid(String), Schema(u32), Conflict }

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self { Self::Io(v) => write!(f, "I/O error: {v}"), Self::Invalid(v) => write!(f, "invalid config: {v}"), Self::Schema(v) => write!(f, "unsupported config schema: {v}"), Self::Conflict => write!(f, "configuration fingerprint changed") }
    }
}
impl std::error::Error for StoreError {}

#[derive(Clone, Debug)]
pub struct ConfigStore { root: PathBuf }

impl ConfigStore {
    pub fn new(root: impl Into<PathBuf>) -> Self { Self { root: root.into() } }
    pub fn path(&self) -> PathBuf { self.root.join(CONFIG_FILE) }

    pub fn load(&self) -> Result<Value, StoreError> {
        let path = self.path();
        let value = match fs::symlink_metadata(&path) {
            Ok(meta) => {
                if !meta.file_type().is_file() { return Err(StoreError::Invalid("config.json is not a regular file".into())); }
                serde_json::from_slice(&fs::read(&path).map_err(|e| StoreError::Io(e.to_string()))?).map_err(|e| StoreError::Invalid(e.to_string()))?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => json!({"schema_version": 4, "profiles": [], "active_id": ""}),
            Err(error) => return Err(StoreError::Io(error.to_string())),
        };
        let schema = value.get("schema_version").and_then(Value::as_u64).unwrap_or(4) as u32;
        if !schema_supported(schema) { return Err(StoreError::Schema(schema)); }
        Ok(value)
    }

    pub fn fingerprint(&self) -> Result<String, StoreError> {
        let value = self.load()?;
        config_fingerprint(&value).map_err(|e| StoreError::Invalid(e.to_string()))
    }

    pub fn save(&self, expected_fingerprint: Option<&str>, value: &Value) -> Result<String, StoreError> {
        let current = self.fingerprint()?;
        if let Some(expected) = expected_fingerprint { if expected != current { return Err(StoreError::Conflict); } }
        let schema = value.get("schema_version").and_then(Value::as_u64).unwrap_or(4) as u32;
        if !schema_supported(schema) { return Err(StoreError::Schema(schema)); }
        fs::create_dir_all(&self.root).map_err(|e| StoreError::Io(e.to_string()))?;
        let tmp = self.root.join(format!(".config-{}.tmp", std::process::id()));
        let bytes = serde_json::to_vec_pretty(value).map_err(|e| StoreError::Invalid(e.to_string()))?;
        let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp).map_err(|e| StoreError::Io(e.to_string()))?;
        file.write_all(&bytes).map_err(|e| StoreError::Io(e.to_string()))?;
        file.sync_all().map_err(|e| StoreError::Io(e.to_string()))?;
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600)).map_err(|e| StoreError::Io(e.to_string()))?; }
        fs::rename(&tmp, self.path()).map_err(|e| StoreError::Io(e.to_string()))?;
        config_fingerprint(value).map_err(|e| StoreError::Invalid(e.to_string()))
    }

    pub fn public_projection(&self) -> Result<Value, StoreError> {
        let mut value = self.load()?;
        if let Some(profiles) = value.get_mut("profiles").and_then(Value::as_array_mut) {
            for profile in profiles {
                if let Some(obj) = profile.as_object_mut() {
                    for key in ["api_key", "key", "secret", "token", "credential_ref", "oauth_token"] { obj.remove(key); }
                }
            }
        }
        Ok(value)
    }

    pub fn update_profile_metadata(&self, expected: &str, id: &str, name: &str, notes: Option<&str>) -> Result<String, StoreError> {
        let mut value = self.load()?;
        let profiles = value.get_mut("profiles").and_then(Value::as_array_mut).ok_or_else(|| StoreError::Invalid("profiles must be an array".into()))?;
        let profile = profiles.iter_mut().find(|profile| profile.get("id").and_then(Value::as_str) == Some(id)).ok_or_else(|| StoreError::Invalid("profile not found".into()))?;
        let obj = profile.as_object_mut().ok_or_else(|| StoreError::Invalid("profile must be an object".into()))?;
        obj.insert("name".into(), Value::String(name.to_string()));
        if let Some(notes) = notes { obj.insert("notes".into(), Value::String(notes.to_string())); }
        self.save(Some(expected), &value)
    }
}
