#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub(crate) const LEGACY_DESKTOP_IDENTIFIER: &str = "com.dimillian.codexmonitor";
pub(crate) const TARGET_DESKTOP_IDENTIFIER: &str = "io.github.deanxie.codexmonitor";
pub(crate) const TARGET_PRODUCT_NAME: &str = "CodexMonitor DeanX";

const ACTIVATION_SCHEMA_VERSION: u32 = 1;
const IDENTITY_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StartupDisposition {
    Ready,
    FreshActivationRequired,
    LegacyMigrationRequired,
    RecoveryRequired,
    BlockedConflict,
    BlockedCorrupt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StartupInspection {
    pub(crate) disposition: StartupDisposition,
    pub(crate) normal_load_allowed: bool,
    pub(crate) target_root: PathBuf,
    pub(crate) legacy_root: PathBuf,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActivatedProfileMetadata {
    pub(crate) transaction_id: String,
    pub(crate) remote_host_identity: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ActivationManifest {
    schema_version: u32,
    transaction_id: String,
    profile_state: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ActiveIdentityStore {
    schema_version: u32,
    state: String,
    remote_host_identity: String,
    transaction_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuntimeActivationJournal {
    schema_version: u32,
    transaction_id: String,
    source_root: PathBuf,
    target_root: PathBuf,
    staging_root: PathBuf,
    state: String,
}

pub(crate) fn inspect_startup_roots(
    target_root: &Path,
    legacy_root: &Path,
) -> Result<StartupInspection, String> {
    let target_root = absolute_path(target_root)?;
    let legacy_root = absolute_path(legacy_root)?;
    if target_root == legacy_root {
        return Ok(inspection(
            StartupDisposition::BlockedConflict,
            target_root,
            legacy_root,
            "legacy and target roots resolve to the same path",
        ));
    }

    if target_root.exists() {
        if !target_root.is_dir() {
            return Ok(inspection(
                StartupDisposition::BlockedConflict,
                target_root,
                legacy_root,
                "target root is not a directory",
            ));
        }
        if target_root.join("activation-manifest.json").exists() {
            return match validate_activated_profile(&target_root) {
                Ok(_) => Ok(inspection(
                    StartupDisposition::Ready,
                    target_root,
                    legacy_root,
                    "activated profile validated",
                )),
                Err(error) => Ok(inspection(
                    StartupDisposition::BlockedCorrupt,
                    target_root,
                    legacy_root,
                    &error,
                )),
            };
        }
        if external_journal_path(&target_root).exists() {
            return Ok(inspection(
                StartupDisposition::RecoveryRequired,
                target_root,
                legacy_root,
                "activation recovery evidence exists",
            ));
        }
        if fs::read_dir(&target_root)
            .map_err(|error| format!("inspect target root: {error}"))?
            .next()
            .is_some()
        {
            return Ok(inspection(
                StartupDisposition::BlockedConflict,
                target_root,
                legacy_root,
                "target root contains unactivated data",
            ));
        }
    }

    if external_journal_path(&target_root).exists() {
        return Ok(inspection(
            StartupDisposition::RecoveryRequired,
            target_root,
            legacy_root,
            "activation recovery evidence exists",
        ));
    }

    if legacy_root.exists() {
        if !legacy_root.is_dir() {
            return Ok(inspection(
                StartupDisposition::BlockedConflict,
                target_root,
                legacy_root,
                "legacy root is not a directory",
            ));
        }
        let settings = legacy_root.join("settings.json");
        let workspaces = legacy_root.join("workspaces.json");
        if settings.exists()
            || workspaces.exists()
            || legacy_root.join("remote-host-identity.json").exists()
        {
            if valid_json(&settings) && valid_json(&workspaces) {
                return Ok(inspection(
                    StartupDisposition::LegacyMigrationRequired,
                    target_root,
                    legacy_root,
                    "legacy profile requires explicit migration activation",
                ));
            }
            return Ok(inspection(
                StartupDisposition::BlockedCorrupt,
                target_root,
                legacy_root,
                "legacy profile is incomplete or corrupt",
            ));
        }
    }

    Ok(inspection(
        StartupDisposition::FreshActivationRequired,
        target_root,
        legacy_root,
        "fresh profile requires explicit activation",
    ))
}

pub(crate) fn validate_activated_profile(root: &Path) -> Result<ActivatedProfileMetadata, String> {
    if !root.is_dir() {
        return Err("activated profile root is unavailable".to_string());
    }
    let manifest: ActivationManifest = read_json(
        &root.join("activation-manifest.json"),
        "activation manifest",
    )?;
    if manifest.schema_version != ACTIVATION_SCHEMA_VERSION
        || manifest.profile_state != "activated"
        || manifest.transaction_id.trim().is_empty()
    {
        return Err("activation manifest is unsupported or incomplete".to_string());
    }
    let identity: ActiveIdentityStore = read_json(
        &root.join("remote-host-identity.json"),
        "remote host identity",
    )?;
    if identity.schema_version != IDENTITY_SCHEMA_VERSION || identity.state != "active" {
        return Err("remote host identity is not an active v2 store".to_string());
    }
    if identity.transaction_id.as_deref() != Some(manifest.transaction_id.as_str()) {
        return Err("activation and identity transaction bindings differ".to_string());
    }
    let journal: RuntimeActivationJournal =
        read_json(&external_journal_path(root), "activation journal")?;
    let canonical_root = fs::canonicalize(root)
        .map_err(|error| format!("canonicalize activated profile root: {error}"))?;
    if journal.schema_version != ACTIVATION_SCHEMA_VERSION
        || journal.state != "runtime_validated"
        || journal.transaction_id != manifest.transaction_id
        || journal.target_root != canonical_root
    {
        return Err("activation journal is incomplete or has a mismatched binding".to_string());
    }
    let _ = (&journal.source_root, &journal.staging_root);
    let parsed = Uuid::parse_str(&identity.remote_host_identity)
        .map_err(|_| "remote host identity is invalid".to_string())?;
    if parsed.is_nil()
        || parsed.get_version_num() != 4
        || parsed.hyphenated().to_string() != identity.remote_host_identity
    {
        return Err("remote host identity is invalid".to_string());
    }
    let _: Value = read_json(&root.join("settings.json"), "settings")?;
    let workspaces: Value = read_json(&root.join("workspaces.json"), "workspaces")?;
    if !workspaces.is_array() {
        return Err("workspaces must be a JSON array".to_string());
    }
    Ok(ActivatedProfileMetadata {
        transaction_id: manifest.transaction_id,
        remote_host_identity: identity.remote_host_identity,
    })
}

pub(crate) fn default_target_root() -> Result<PathBuf, String> {
    Ok(platform_data_base()?.join(TARGET_DESKTOP_IDENTIFIER))
}

pub(crate) fn legacy_root_for_target(target_root: &Path) -> Result<PathBuf, String> {
    let parent = target_root
        .parent()
        .ok_or("target data root has no parent")?;
    Ok(parent.join(LEGACY_DESKTOP_IDENTIFIER))
}

fn platform_data_base() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        let value = std::env::var_os("APPDATA").ok_or("APPDATA is unavailable")?;
        if value.is_empty() {
            return Err("APPDATA is empty".to_string());
        }
        return Ok(PathBuf::from(value));
    }
    #[cfg(target_os = "macos")]
    {
        let value = std::env::var_os("HOME").ok_or("HOME is unavailable")?;
        if value.is_empty() {
            return Err("HOME is empty".to_string());
        }
        return Ok(PathBuf::from(value).join("Library/Application Support"));
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Some(value) = std::env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
            return Ok(PathBuf::from(value));
        }
        let value = std::env::var_os("HOME").ok_or("HOME is unavailable")?;
        if value.is_empty() {
            return Err("HOME is empty".to_string());
        }
        Ok(PathBuf::from(value).join(".local/share"))
    }
}

fn inspection(
    disposition: StartupDisposition,
    target_root: PathBuf,
    legacy_root: PathBuf,
    reason: &str,
) -> StartupInspection {
    StartupInspection {
        normal_load_allowed: disposition == StartupDisposition::Ready,
        disposition,
        target_root,
        legacy_root,
        reason: Some(reason.to_string()),
    }
}

fn valid_json(path: &Path) -> bool {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .is_some()
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path, label: &str) -> Result<T, String> {
    let bytes = fs::read(path).map_err(|error| format!("read {label}: {error}"))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("parse {label}: {error}"))
}

fn external_journal_path(target_root: &Path) -> PathBuf {
    let name = target_root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("target");
    target_root
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(".{name}.codexmonitor-activation-journal.json"))
}

fn absolute_path(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .map_err(|error| format!("resolve data root: {error}"))
    }
}
