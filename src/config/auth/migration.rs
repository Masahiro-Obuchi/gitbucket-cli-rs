//! Explicit whole-file migration, with an exact, guarded rollback under the
//! same OS lock as ordinary settings writes. Unknown TOML fields are retained.
use super::{
    instance::{instance_key, unique_key},
    store, AuthConfig, HostConfig,
};
use crate::error::{GbError, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use toml::Value;

#[derive(Serialize)]
pub struct MigrationReport {
    changed: bool,
    applied: bool,
    restored: bool,
    backup: Option<PathBuf>,
    key_changes: Vec<KeyChange>,
}
#[derive(Serialize)]
struct KeyChange {
    scope: String,
    from: String,
    to: String,
}
#[derive(Serialize, Deserialize)]
struct Backup {
    version: u8,
    original: String,
    migrated_sha256: String,
}

fn digest(content: &str) -> String {
    ring::digest::digest(&ring::digest::SHA256, content.as_bytes())
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn read(path: &Path) -> Result<String> {
    let mut file = store::open_regular(path, false)?;
    let mut text = String::new();
    file.read_to_string(&mut text)?;
    Ok(text)
}
fn host_table(node: &Value) -> Result<HashMap<String, HostConfig>> {
    match node.get("hosts") {
        Some(hosts) => Ok(hosts.clone().try_into()?),
        None => Ok(HashMap::new()),
    }
}
fn normalize_hosts(node: &mut Value, scope: &str, changes: &mut Vec<KeyChange>) -> Result<()> {
    let Some(hosts) = node.get_mut("hosts") else {
        return Ok(());
    };
    let mut normalized = toml::map::Map::new();
    for (key, value) in hosts
        .as_table()
        .ok_or_else(|| GbError::Config("Expected a hosts table.".into()))?
    {
        let host: HostConfig = value.clone().try_into()?;
        let canonical = instance_key(key, &host.protocol)?;
        let mut value = value.clone();
        let protocol = url::Url::parse(&canonical)?.scheme().to_string();
        value
            .as_table_mut()
            .expect("validated host table")
            .insert("protocol".into(), Value::String(protocol));
        if let Some(previous) = normalized.get(&canonical) {
            if previous != &value {
                return Err(GbError::Config(format!(
                    "Credential-key collision in {scope} for {canonical}. No configuration was changed; resolve conflicting aliases explicitly."
                )));
            }
        }
        if key != &canonical || value != hosts[key] {
            changes.push(KeyChange {
                scope: scope.to_string(),
                from: key.clone(),
                to: canonical.clone(),
            });
        }
        normalized.insert(canonical, value);
    }
    *hosts = Value::Table(normalized);
    Ok(())
}
fn normalize_default(node: &mut Value, global: &HashMap<String, HostConfig>) -> Result<()> {
    let Some(value) = node.get("default_host") else {
        return Ok(());
    };
    let target = value
        .as_str()
        .ok_or_else(|| GbError::Config("default_host must be a string.".into()))?;
    let own = host_table(node)?;
    let normalized = if let Some(key) = unique_key(&own, target, None)? {
        key
    } else if let Some(key) = unique_key(global, target, None)? {
        key
    } else if target.contains("://") {
        instance_key(target, "https")?
    } else {
        return Err(GbError::Config("Cannot infer the scheme of a bare default_host without matching saved credentials. Set an explicit HTTP(S) default URL before migration.".into()));
    };
    node["default_host"] = Value::String(normalized);
    Ok(())
}
fn plan(original: &str) -> Result<(String, bool, Vec<KeyChange>)> {
    // Validate known fields, then transform the original document so future
    // fields are not silently erased by typed serialization.
    let _: AuthConfig = toml::from_str(original)?;
    let mut document: Value = toml::from_str(original)?;
    let before = document.clone();
    let mut changes = Vec::new();
    normalize_hosts(&mut document, "global", &mut changes)?;
    let global = host_table(&document)?;
    normalize_default(&mut document, &global)?;
    if let Some(profiles) = document.get_mut("profiles").and_then(Value::as_table_mut) {
        for (name, profile) in profiles {
            normalize_hosts(profile, &format!("profile:{name}"), &mut changes)?;
            normalize_default(profile, &global)?;
        }
    }
    Ok((
        toml::to_string_pretty(&document)?,
        document != before,
        changes,
    ))
}
impl AuthConfig {
    pub fn migrate(apply: bool, restore: Option<&Path>) -> Result<MigrationReport> {
        let dir = crate::config::ensure_config_dir()?;
        let path = dir.join("config.toml");
        let operation = || {
            let original = match read(&path) {
                Ok(text) => text,
                Err(GbError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(e) => return Err(e),
            };
            if let Some(backup_path) = restore {
                let backup: Backup = serde_json::from_str(&read(backup_path)?)
                    .map_err(|_| GbError::Config("Invalid migration backup.".into()))?;
                if backup.version != 1 {
                    return Err(GbError::Config(
                        "Unsupported migration backup version.".into(),
                    ));
                }
                let _: AuthConfig = toml::from_str(&backup.original).map_err(|_| {
                    GbError::Config("Invalid original configuration in migration backup.".into())
                })?;
                if original != backup.original {
                    if digest(&original) != backup.migrated_sha256 {
                        return Err(GbError::Config("Configuration changed since migration; automatic restore refused. Inspect the current configuration and backup before a manual recovery.".into()));
                    }
                    store::write_config_file(&path, &backup.original)?;
                }
                return Ok(MigrationReport {
                    changed: original != backup.original,
                    applied: false,
                    restored: true,
                    backup: Some(backup_path.to_path_buf()),
                    key_changes: Vec::new(),
                });
            }
            let (content, changed, key_changes) = plan(&original)?;
            let mut backup = None;
            if apply && changed {
                let mut file = tempfile::Builder::new()
                    .prefix("config.toml.migration-")
                    .suffix(".json")
                    .tempfile_in(&dir)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    file.as_file()
                        .set_permissions(std::fs::Permissions::from_mode(0o600))?;
                }
                file.write_all(&serde_json::to_vec(&Backup {
                    version: 1,
                    original,
                    migrated_sha256: digest(&content),
                })?)?;
                file.as_file().sync_all()?;
                let (_, backup_path) = file.keep().map_err(|e| GbError::Io(e.error))?;
                #[cfg(unix)]
                std::fs::File::open(&dir)?.sync_all()?;
                backup = Some(backup_path);
                store::write_config_file(&path, &content)?;
            }
            Ok(MigrationReport {
                changed,
                applied: apply && changed,
                restored: false,
                backup,
                key_changes,
            })
        };
        if apply || restore.is_some() {
            store::with_config_lock(&path, operation)
        } else {
            operation()
        }
    }
}
