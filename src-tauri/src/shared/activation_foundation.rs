#![allow(dead_code)]

use super::remote_host_identity_activation::load_v2_identity;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub(crate) const ACTIVATION_MANIFEST_FILE: &str = "activation-manifest.json";
pub(crate) const ACTIVATION_JOURNAL_FILE: &str = ".codexmonitor-activation-journal.json";
const ACTIVATION_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BootstrapProfileClassification {
    FreshProfile,
    ActivatedValid,
    LegacyMigrationRequired,
    ActivationRecoveryRequired,
    TargetConflict,
    CorruptProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LoadPermission {
    Denied,
    ActivationOnly,
    Normal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BootstrapInspection {
    pub(crate) classification: BootstrapProfileClassification,
    pub(crate) load_permission: LoadPermission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ActivationJournalState {
    Prepared,
    LegacyIdentityRetired,
    TargetCommitted,
    RuntimeValidated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ActivationJournal {
    pub(crate) schema_version: u32,
    pub(crate) transaction_id: String,
    pub(crate) source_root: PathBuf,
    pub(crate) target_root: PathBuf,
    pub(crate) staging_root: PathBuf,
    pub(crate) state: ActivationJournalState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ActivationManifest {
    schema_version: u32,
    transaction_id: String,
    profile_state: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecoveryDisposition {
    ContinueBeforeRetirement,
    ContinueAfterRetirement,
    ValidateCommittedTarget,
    Complete,
    BlockedInconsistent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActivationFailPoint {
    AfterPreparedJournal,
    AfterIdentityReplaceBeforeJournal,
    AfterIdentityJournal,
    AfterTargetMoveBeforeJournal,
    AfterTargetJournal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LegacyProcessStopEvidence {
    ConfirmedStopped,
    Running,
    Unknown,
}

pub(crate) fn inspect_bootstrap_profile(root: &Path) -> Result<BootstrapInspection, String> {
    if !root.exists() {
        return Ok(BootstrapInspection {
            classification: BootstrapProfileClassification::FreshProfile,
            load_permission: LoadPermission::ActivationOnly,
        });
    }
    if !root.is_dir() {
        return Ok(denied(BootstrapProfileClassification::TargetConflict));
    }
    let external_journal = activation_journal_path(root);
    let manifest = root.join(ACTIVATION_MANIFEST_FILE);
    let settings = root.join("settings.json");
    let workspaces = root.join("workspaces.json");
    let identity = root.join("remote-host-identity.json");
    if manifest.exists() {
        let bound_activation = read_activation_manifest(&manifest)
            .ok()
            .and_then(|manifest| {
                load_v2_identity(&identity).ok().map(|identity| {
                    identity.transaction_id.as_deref() == Some(&manifest.transaction_id)
                })
            })
            .unwrap_or(false);
        if bound_activation && valid_json(&settings) && valid_json(&workspaces) {
            return Ok(BootstrapInspection {
                classification: BootstrapProfileClassification::ActivatedValid,
                load_permission: LoadPermission::Normal,
            });
        }
        return Ok(denied(BootstrapProfileClassification::CorruptProfile));
    }
    if external_journal.exists() {
        return Ok(denied(
            BootstrapProfileClassification::ActivationRecoveryRequired,
        ));
    }
    if settings.exists() || workspaces.exists() || identity.exists() {
        if valid_json(&settings) && valid_json(&workspaces) {
            return Ok(BootstrapInspection {
                classification: BootstrapProfileClassification::LegacyMigrationRequired,
                load_permission: LoadPermission::ActivationOnly,
            });
        }
        return Ok(denied(BootstrapProfileClassification::CorruptProfile));
    }
    let has_entries = fs::read_dir(root)
        .map_err(|error| format!("inspect bootstrap root: {error}"))?
        .next()
        .is_some();
    Ok(if has_entries {
        denied(BootstrapProfileClassification::TargetConflict)
    } else {
        BootstrapInspection {
            classification: BootstrapProfileClassification::FreshProfile,
            load_permission: LoadPermission::ActivationOnly,
        }
    })
}

pub(crate) fn activation_staging_path(target_root: &Path) -> PathBuf {
    let name = target_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("target");
    target_root
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(".{name}.codexmonitor-activation-prepared"))
}

pub(crate) fn activation_journal_path(target_root: &Path) -> PathBuf {
    let name = target_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("target");
    target_root
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(".{name}{ACTIVATION_JOURNAL_FILE}"))
}

pub(crate) fn prepare_fresh_activation(
    target_root: &Path,
    identity: &str,
) -> Result<ActivationJournal, String> {
    if target_root.exists()
        && fs::read_dir(target_root)
            .map_err(|error| format!("inspect fresh target: {error}"))?
            .next()
            .is_some()
    {
        return Err("fresh activation target is not empty".to_string());
    }
    let staging = activation_staging_path(target_root);
    if staging.exists() {
        return Err("fresh activation staging already exists".to_string());
    }
    fs::create_dir_all(&staging).map_err(|error| format!("create fresh staging: {error}"))?;
    let transaction_id = Uuid::new_v4().to_string();
    write_json_atomic(
        &staging.join("settings.json"),
        &crate::types::AppSettings::default(),
    )?;
    write_json_atomic(&staging.join("workspaces.json"), &Vec::<Value>::new())?;
    super::remote_host_identity_activation::write_v2_identity(
        &staging.join("remote-host-identity.json"),
        identity,
        super::remote_host_identity_activation::HostIdentityV2State::Active,
        Some(&transaction_id),
    )?;
    write_json_atomic(
        &staging.join(ACTIVATION_MANIFEST_FILE),
        &ActivationManifest {
            schema_version: ACTIVATION_SCHEMA_VERSION,
            transaction_id: transaction_id.clone(),
            profile_state: "activated".to_string(),
        },
    )?;
    validate_prepared_profile_for_transaction(&staging, &transaction_id)?;
    let journal = ActivationJournal {
        schema_version: ACTIVATION_SCHEMA_VERSION,
        transaction_id,
        source_root: PathBuf::new(),
        target_root: canonical_future_path(target_root)?,
        staging_root: canonical_future_path(&staging)?,
        state: ActivationJournalState::Prepared,
    };
    write_json_atomic(&activation_journal_path(target_root), &journal)?;
    Ok(journal)
}

pub(crate) fn commit_fresh_activation(target_root: &Path) -> Result<(), String> {
    let _lock = ActivationCommitLock::acquire(target_root)?;
    let journal = read_journal(target_root)?;
    if journal.state != ActivationJournalState::Prepared
        || journal.target_root != canonical_future_path(target_root)?
        || journal.staging_root != canonical_future_path(&activation_staging_path(target_root))?
    {
        return Err("fresh activation journal binding mismatch".to_string());
    }
    validate_prepared_profile_for_transaction(
        &activation_staging_path(target_root),
        &journal.transaction_id,
    )?;
    if target_root.exists()
        && fs::read_dir(target_root)
            .map_err(|error| format!("inspect target before commit: {error}"))?
            .next()
            .is_some()
    {
        return Err("fresh activation target changed".to_string());
    }
    if target_root.exists() {
        fs::remove_dir(target_root).map_err(|error| format!("remove empty target: {error}"))?;
    }
    fs::rename(activation_staging_path(target_root), target_root)
        .map_err(|error| format!("commit prepared profile: {error}"))?;
    let mut committed = journal;
    committed.state = ActivationJournalState::TargetCommitted;
    write_json_atomic(&activation_journal_path(target_root), &committed)
}

pub(crate) fn write_activation_journal(
    target_root: &Path,
    journal: &ActivationJournal,
) -> Result<(), String> {
    write_json_atomic(&activation_journal_path(target_root), journal)
}

pub(crate) fn prepare_migration_activation(
    source_root: &Path,
    target_root: &Path,
    legacy_process: LegacyProcessStopEvidence,
) -> Result<ActivationJournal, String> {
    if legacy_process != LegacyProcessStopEvidence::ConfirmedStopped {
        return Err("legacy process stop is not confirmed".to_string());
    }
    super::migration_core::validate_staging(target_root).map_err(|error| error.to_string())?;
    super::migration_core::protect_staging_for_activation(target_root)
        .map_err(|error| error.to_string())?;
    let staging = super::migration_core::staging_path(target_root);
    let source_identity = source_root.join("remote-host-identity.json");
    let identity = read_v1_identity(&source_identity)?;
    let transaction_id = Uuid::new_v4().to_string();
    let recovery = staging.join("identity-recovery");
    fs::create_dir_all(&recovery).map_err(|error| format!("create identity recovery: {error}"))?;
    super::remote_host_identity_activation::write_v2_identity(
        &recovery.join("remote-host-identity.v2.active.json"),
        &identity,
        super::remote_host_identity_activation::HostIdentityV2State::Active,
        Some(&transaction_id),
    )?;
    super::remote_host_identity_activation::write_v2_identity(
        &recovery.join("remote-host-identity.v2.retired.json"),
        &identity,
        super::remote_host_identity_activation::HostIdentityV2State::Retired,
        Some(&transaction_id),
    )?;
    fs::copy(
        recovery.join("remote-host-identity.v2.active.json"),
        staging.join("remote-host-identity.json"),
    )
    .map_err(|error| format!("prepare active identity: {error}"))?;
    write_json_atomic(
        &staging.join(ACTIVATION_MANIFEST_FILE),
        &ActivationManifest {
            schema_version: ACTIVATION_SCHEMA_VERSION,
            transaction_id: transaction_id.clone(),
            profile_state: "activated".to_string(),
        },
    )?;
    validate_prepared_profile_for_transaction(&staging, &transaction_id)?;
    let journal = ActivationJournal {
        schema_version: ACTIVATION_SCHEMA_VERSION,
        transaction_id,
        source_root: fs::canonicalize(source_root)
            .map_err(|error| format!("bind source root: {error}"))?,
        target_root: canonical_future_path(target_root)?,
        staging_root: fs::canonicalize(&staging)
            .map_err(|error| format!("bind staging root: {error}"))?,
        state: ActivationJournalState::Prepared,
    };
    write_activation_journal(target_root, &journal)?;
    Ok(journal)
}

pub(crate) fn commit_migration_activation(
    target_root: &Path,
    failpoint: Option<ActivationFailPoint>,
) -> Result<(), String> {
    let _lock = ActivationCommitLock::acquire(target_root)?;
    let mut journal = read_journal(target_root)?;
    if journal.schema_version != ACTIVATION_SCHEMA_VERSION
        || journal.target_root != canonical_future_path(target_root)?
    {
        return Err("activation journal binding mismatch".to_string());
    }
    if journal.state == ActivationJournalState::RuntimeValidated {
        return Err("activation already completed".to_string());
    }
    if journal.staging_root.is_dir() {
        validate_prepared_profile_for_transaction(&journal.staging_root, &journal.transaction_id)?;
    }
    maybe_fail(failpoint, ActivationFailPoint::AfterPreparedJournal)?;
    let source_identity = journal.source_root.join("remote-host-identity.json");
    let source_state = read_identity_state(&source_identity)?;
    if source_state == "active_v1" {
        #[cfg(windows)]
        {
            let replacement = journal
                .staging_root
                .join("identity-recovery/remote-host-identity.v2.retired.json");
            if !replacement.is_file() {
                return Err("identity recovery material missing".to_string());
            }
            let guard = super::remote_host_identity_activation::ProtectedLegacyIdentity::acquire(
                &source_identity,
            )
            .map_err(|error| format!("protect legacy identity: {error}"))?;
            guard
                .replace_with(&replacement)
                .map_err(|error| format!("retire legacy identity: {error}"))?;
        }
        #[cfg(not(windows))]
        return Err("legacy identity retirement is Windows-gated".to_string());
    } else if source_state == "retired_v2" {
        if !retired_identity_matches_transaction(&source_identity, &journal.transaction_id) {
            return Err("retired identity transaction binding mismatch".to_string());
        }
    } else {
        return Err("legacy identity state is inconsistent".to_string());
    }
    maybe_fail(
        failpoint,
        ActivationFailPoint::AfterIdentityReplaceBeforeJournal,
    )?;
    journal.state = ActivationJournalState::LegacyIdentityRetired;
    write_activation_journal(target_root, &journal)?;
    maybe_fail(failpoint, ActivationFailPoint::AfterIdentityJournal)?;

    if journal.staging_root.is_dir() {
        if target_root.exists()
            && fs::read_dir(target_root)
                .map_err(|error| format!("inspect activation target: {error}"))?
                .next()
                .is_some()
        {
            return Err("activation target changed".to_string());
        }
        if target_root.exists() {
            fs::remove_dir(target_root).map_err(|error| format!("remove empty target: {error}"))?;
        }
        fs::rename(&journal.staging_root, target_root)
            .map_err(|error| format!("commit activation target: {error}"))?;
    } else if validate_prepared_profile_for_transaction(target_root, &journal.transaction_id)
        .is_err()
    {
        return Err("neither prepared staging nor committed target is valid".to_string());
    }
    maybe_fail(failpoint, ActivationFailPoint::AfterTargetMoveBeforeJournal)?;
    journal.state = ActivationJournalState::TargetCommitted;
    write_activation_journal(target_root, &journal)?;
    maybe_fail(failpoint, ActivationFailPoint::AfterTargetJournal)?;
    validate_prepared_profile_for_transaction(target_root, &journal.transaction_id)?;
    journal.state = ActivationJournalState::RuntimeValidated;
    write_activation_journal(target_root, &journal)
}

pub(crate) fn recover_activation(target_root: &Path) -> Result<RecoveryDisposition, String> {
    let journal = read_journal(target_root)?;
    if journal.schema_version != ACTIVATION_SCHEMA_VERSION
        || journal.target_root != canonical_future_path(target_root)?
    {
        return Ok(RecoveryDisposition::BlockedInconsistent);
    }
    let staging_exists = journal.staging_root.is_dir();
    let staging_valid = staging_exists
        && validate_prepared_profile_for_transaction(
            &journal.staging_root,
            &journal.transaction_id,
        )
        .is_ok();
    let target_valid =
        validate_prepared_profile_for_transaction(target_root, &journal.transaction_id).is_ok();
    let is_migration = !journal.source_root.as_os_str().is_empty();
    let source_identity = journal.source_root.join("remote-host-identity.json");
    let source_active = is_migration && read_v1_identity(&source_identity).is_ok();
    let source_retired = is_migration
        && retired_identity_matches_transaction(&source_identity, &journal.transaction_id);
    let source_consistent_after_retirement = !is_migration || source_retired;
    Ok(match journal.state {
        ActivationJournalState::Prepared if source_retired && staging_valid => {
            RecoveryDisposition::ContinueAfterRetirement
        }
        ActivationJournalState::Prepared if (source_active || !is_migration) && staging_valid => {
            RecoveryDisposition::ContinueBeforeRetirement
        }
        ActivationJournalState::LegacyIdentityRetired if source_retired && staging_valid => {
            RecoveryDisposition::ContinueAfterRetirement
        }
        ActivationJournalState::Prepared | ActivationJournalState::LegacyIdentityRetired
            if target_valid && !staging_exists && source_consistent_after_retirement =>
        {
            RecoveryDisposition::ValidateCommittedTarget
        }
        ActivationJournalState::TargetCommitted
            if target_valid && source_consistent_after_retirement =>
        {
            RecoveryDisposition::ValidateCommittedTarget
        }
        ActivationJournalState::RuntimeValidated
            if target_valid && source_consistent_after_retirement =>
        {
            RecoveryDisposition::Complete
        }
        _ => RecoveryDisposition::BlockedInconsistent,
    })
}

fn retired_identity_matches_transaction(path: &Path, transaction_id: &str) -> bool {
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    let Ok(store) = serde_json::from_slice::<
        super::remote_host_identity_activation::HostIdentityV2Store,
    >(&bytes) else {
        return false;
    };
    store.schema_version == 2
        && store.state == super::remote_host_identity_activation::HostIdentityV2State::Retired
        && store.transaction_id.as_deref() == Some(transaction_id)
        && Uuid::parse_str(&store.remote_host_identity)
            .map(|identity| !identity.is_nil())
            .unwrap_or(false)
}

fn read_journal(target_root: &Path) -> Result<ActivationJournal, String> {
    let bytes = fs::read(activation_journal_path(target_root))
        .map_err(|error| format!("read activation journal: {error}"))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("parse activation journal: {error}"))
}

fn validate_prepared_profile(root: &Path) -> Result<(), String> {
    validate_prepared_profile_inner(root, None)
}

fn validate_prepared_profile_for_transaction(
    root: &Path,
    transaction_id: &str,
) -> Result<(), String> {
    validate_prepared_profile_inner(root, Some(transaction_id))
}

fn validate_prepared_profile_inner(
    root: &Path,
    expected_transaction_id: Option<&str>,
) -> Result<(), String> {
    for name in [
        "settings.json",
        "workspaces.json",
        "remote-host-identity.json",
    ] {
        if !valid_json(&root.join(name)) {
            return Err(format!("prepared profile artifact invalid: {name}"));
        }
    }
    let manifest = read_activation_manifest(&root.join(ACTIVATION_MANIFEST_FILE))?;
    if let Some(expected) = expected_transaction_id {
        if manifest.transaction_id != expected {
            return Err("prepared profile transaction binding mismatch".to_string());
        }
    }
    let identity = load_v2_identity(&root.join("remote-host-identity.json"))?;
    if let Some(expected) = expected_transaction_id {
        if identity.transaction_id.as_deref() != Some(expected) {
            return Err("prepared identity transaction binding mismatch".to_string());
        }
    }
    Ok(())
}

fn read_activation_manifest(path: &Path) -> Result<ActivationManifest, String> {
    let bytes = fs::read(path).map_err(|error| format!("read activation manifest: {error}"))?;
    let manifest: ActivationManifest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse activation manifest: {error}"))?;
    if manifest.schema_version != ACTIVATION_SCHEMA_VERSION
        || manifest.profile_state != "activated"
        || manifest.transaction_id.trim().is_empty()
    {
        return Err("prepared profile activation manifest invalid".to_string());
    }
    Ok(manifest)
}

fn read_v1_identity(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| format!("read legacy identity: {error}"))?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse legacy identity: {error}"))?;
    if value.get("schemaVersion").and_then(Value::as_u64) != Some(1) {
        return Err("legacy identity is not schema v1".to_string());
    }
    let identity = value
        .get("remoteHostIdentity")
        .and_then(Value::as_str)
        .ok_or_else(|| "legacy identity value missing".to_string())?;
    let parsed = Uuid::parse_str(identity).map_err(|_| "legacy identity invalid".to_string())?;
    if parsed.is_nil() {
        return Err("legacy identity invalid".to_string());
    }
    Ok(parsed.to_string())
}

fn read_identity_state(path: &Path) -> Result<&'static str, String> {
    let bytes = fs::read(path).map_err(|error| format!("read identity state: {error}"))?;
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|error| format!("parse identity state: {error}"))?;
    match (
        value.get("schemaVersion").and_then(Value::as_u64),
        value.get("state").and_then(Value::as_str),
    ) {
        (Some(1), _) => {
            read_v1_identity(path)?;
            Ok("active_v1")
        }
        (Some(2), Some("retired")) => Ok("retired_v2"),
        (Some(2), Some("active")) => Ok("active_v2"),
        _ => Err("identity state unsupported".to_string()),
    }
}

fn maybe_fail(
    selected: Option<ActivationFailPoint>,
    current: ActivationFailPoint,
) -> Result<(), String> {
    if selected == Some(current) {
        Err(format!("simulated interruption at {current:?}"))
    } else {
        Ok(())
    }
}

#[cfg(windows)]
struct ActivationCommitLock {
    _file: fs::File,
}

#[cfg(windows)]
impl ActivationCommitLock {
    fn acquire(target_root: &Path) -> Result<Self, String> {
        use std::os::windows::fs::OpenOptionsExt;
        let path = target_root
            .parent()
            .ok_or_else(|| "target lacks parent".to_string())?
            .join(format!(
                ".{}.activation.lock",
                target_root
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("target")
            ));
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .share_mode(0)
            .open(path)
            .map_err(|error| format!("activation commit already in progress: {error}"))?;
        Ok(Self { _file: file })
    }
}

#[cfg(not(windows))]
struct ActivationCommitLock;

#[cfg(not(windows))]
impl ActivationCommitLock {
    fn acquire(_target_root: &Path) -> Result<Self, String> {
        Err("activation commit lock is not yet platform-validated".to_string())
    }
}

fn canonical_future_path(path: &Path) -> Result<PathBuf, String> {
    if path.exists() {
        return fs::canonicalize(path).map_err(|error| format!("canonicalize path: {error}"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| "path lacks a parent".to_string())?;
    let parent =
        fs::canonicalize(parent).map_err(|error| format!("canonicalize parent: {error}"))?;
    Ok(parent.join(
        path.file_name()
            .ok_or_else(|| "path lacks a name".to_string())?,
    ))
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "artifact lacks parent".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("create artifact parent: {error}"))?;
    let temp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("artifact"),
        Uuid::new_v4()
    ));
    let bytes =
        serde_json::to_vec_pretty(value).map_err(|error| format!("serialize artifact: {error}"))?;
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)
        .map_err(|error| format!("create temp artifact: {error}"))?;
    file.write_all(&bytes)
        .map_err(|error| format!("write temp artifact: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("sync temp artifact: {error}"))?;
    fs::rename(&temp, path).map_err(|error| format!("commit artifact: {error}"))
}

fn valid_json(path: &Path) -> bool {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .is_some()
}

fn denied(classification: BootstrapProfileClassification) -> BootstrapInspection {
    BootstrapInspection {
        classification,
        load_permission: LoadPermission::Denied,
    }
}

#[cfg(windows)]
pub(crate) struct ServiceLifetimeLock {
    _file: fs::File,
    pub(crate) path: PathBuf,
}

#[cfg(windows)]
impl ServiceLifetimeLock {
    pub(crate) fn acquire(root: &Path) -> Result<Self, String> {
        use std::os::windows::fs::OpenOptionsExt;
        fs::create_dir_all(root).map_err(|error| format!("create lock root: {error}"))?;
        let path = root.join("remote-service-lifetime.lock");
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .share_mode(0)
            .open(&path)
            .map_err(|error| format!("service lifetime lock unavailable: {error}"))?;
        Ok(Self { _file: file, path })
    }
}

#[cfg(not(windows))]
pub(crate) struct ServiceLifetimeLock;

#[cfg(not(windows))]
impl ServiceLifetimeLock {
    pub(crate) fn acquire(_root: &Path) -> Result<Self, String> {
        Err("service lifetime lock is not yet platform-validated".to_string())
    }
}
