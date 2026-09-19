#![allow(dead_code)]

use crate::types::{AppSettings, WorkspaceEntry};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const SETTINGS_FILE: &str = "settings.json";
const WORKSPACES_FILE: &str = "workspaces.json";
const MARKER_FILE: &str = ".codexmonitor-migration-owned.json";
const STATE_FILE: &str = "migration-state.json";
const MANIFEST_FILE: &str = "migration-manifest.json";
const REPORT_FILE: &str = "migration-report.json";
const SOURCE_MANIFEST_FILE: &str = "profile-manifest.json";
const ENGINE_ID: &str = "codex-monitor-p4-1c";
const LEGACY_V0_SCHEMA: u32 = 0;
const CLEANUP_ORDINARY: &str = "ordinary_staging";
const CLEANUP_PROTECTED: &str = "activation_recovery_material";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MigrationState {
    NotStarted,
    Preflighted,
    Staging,
    Validated,
    ReadyForActivation,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MigrationFailPoint {
    AfterPreflight,
    MidStage,
    AfterStage,
    BeforeValidation,
    AfterValidation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MigrationPreview {
    pub(crate) state: MigrationState,
    pub(crate) source_root: PathBuf,
    pub(crate) target_root: PathBuf,
    pub(crate) source_detected: bool,
    pub(crate) source_schema_version: u32,
    pub(crate) target_schema_version: u32,
    pub(crate) migratable_categories: Vec<String>,
    pub(crate) excluded_categories: Vec<String>,
    pub(crate) deferred_categories: Vec<String>,
    pub(crate) unknown_categories: Vec<String>,
    pub(crate) excluded_field_paths: Vec<String>,
    pub(crate) conflicts: Vec<String>,
    pub(crate) warnings: Vec<String>,
    pub(crate) credential_fields_excluded: usize,
    pub(crate) legacy_remote_host_identity_present: bool,
    pub(crate) estimated_file_count: usize,
    pub(crate) estimated_action_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MigrationResult {
    pub(crate) state: MigrationState,
    pub(crate) staging_root: PathBuf,
    pub(crate) config_schema_version: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct MigrationError {
    code: &'static str,
    message: String,
}

impl MigrationError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn io(action: &'static str, error: io::Error) -> Self {
        Self::new("IO_FAILED", format!("{action}: {}", error.kind()))
    }
}

impl fmt::Display for MigrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for MigrationError {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReleaseIdentityAuthority {
    config_schema_version: u32,
    target: ReleaseTargetIdentity,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReleaseTargetIdentity {
    product_name: String,
    desktop_identifier: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StagingMarker {
    engine: String,
    target_root: PathBuf,
    source_root: PathBuf,
    source_fingerprint: String,
    config_schema_version: u32,
    cleanup_policy: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StagingState {
    state: MigrationState,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MigrationManifest {
    config_schema_version: u32,
    target_product_name: String,
    target_desktop_identifier: String,
    activation_phase: String,
    credentials_migrated: bool,
    remote_host_identity_migrated: bool,
    prepared_content_sha256: String,
}

struct PreparedInput {
    settings: Value,
    workspaces: Value,
    excluded_field_paths: Vec<String>,
    credential_fields_excluded: usize,
}

pub(crate) fn config_schema_version() -> Result<u32, MigrationError> {
    Ok(release_identity_authority()?.config_schema_version)
}

pub(crate) fn staging_path(target_root: &Path) -> PathBuf {
    let name = target_root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("target");
    target_root
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(".{name}.codexmonitor-migration-staging"))
}

pub(crate) fn inspect_migration(
    source_root: &Path,
    target_root: &Path,
) -> Result<MigrationPreview, MigrationError> {
    validate_root_relationships(source_root, target_root)?;
    if !source_root.is_dir() {
        return Err(MigrationError::new(
            "PRECHECK_BLOCKED",
            "explicit sourceRoot is not a directory",
        ));
    }

    let prepared = prepare_input(source_root)?;
    let mut migratable_categories = vec!["settings".to_string(), "workspaces".to_string()];
    let mut excluded_categories = Vec::new();
    let mut deferred_categories = Vec::new();
    let mut unknown_categories = Vec::new();
    let mut root_entries = fs::read_dir(source_root)
        .map_err(|error| MigrationError::io("enumerate sourceRoot", error))?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().to_string()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| MigrationError::io("enumerate sourceRoot entry", error))?;
    root_entries.sort();
    for name in root_entries {
        let lower = name.to_ascii_lowercase();
        match lower.as_str() {
            SETTINGS_FILE | WORKSPACES_FILE | SOURCE_MANIFEST_FILE => {}
            "remote-host-identity.json" | "remote-host-identity.lock" => {
                push_unique(&mut deferred_categories, "remote_host_identity")
            }
            _ if is_credential_name(&lower) => push_unique(&mut excluded_categories, "credentials"),
            "codex_home" | ".codex" => push_unique(&mut excluded_categories, "codex_home"),
            "sessions" | "threads" => {
                push_unique(&mut excluded_categories, "canonical_thread_data")
            }
            _ if lower.starts_with("rollout-") || lower.ends_with(".jsonl") => {
                push_unique(&mut excluded_categories, "canonical_thread_data")
            }
            _ if is_runtime_or_transient_name(&lower) => {
                push_unique(&mut excluded_categories, "runtime_transient_state")
            }
            _ => push_unique(&mut unknown_categories, "unrecognized_root_artifact"),
        }
    }
    if prepared.credential_fields_excluded > 0 {
        push_unique(&mut excluded_categories, "credentials");
    }
    migratable_categories.sort();
    excluded_categories.sort();
    deferred_categories.sort();
    unknown_categories.sort();

    let mut conflicts = Vec::new();
    if directory_has_entries(target_root)? {
        conflicts.push("target_root_non_empty".to_string());
    }
    let staging = staging_path(target_root);
    if staging.exists() && !owned_staging_matches(&staging, target_root).unwrap_or(false) {
        conflicts.push("unowned_staging_exists".to_string());
    }

    let mut warnings = Vec::new();
    if prepared.credential_fields_excluded > 0 {
        warnings.push("remote_connection_reauthentication_required".to_string());
    }
    if !unknown_categories.is_empty() || !prepared.excluded_field_paths.is_empty() {
        warnings.push("unknown_data_excluded".to_string());
    }

    let source_schema = detect_source_schema(source_root)?;
    let schema = config_schema_version()?;
    Ok(MigrationPreview {
        state: MigrationState::Preflighted,
        source_root: source_root.to_path_buf(),
        target_root: target_root.to_path_buf(),
        source_detected: true,
        source_schema_version: source_schema,
        target_schema_version: schema,
        migratable_categories,
        excluded_categories,
        deferred_categories,
        unknown_categories,
        excluded_field_paths: prepared.excluded_field_paths,
        conflicts,
        warnings,
        credential_fields_excluded: prepared.credential_fields_excluded,
        legacy_remote_host_identity_present: source_root
            .join("remote-host-identity.json")
            .is_file(),
        estimated_file_count: 7,
        estimated_action_count: 7,
    })
}

pub(crate) fn stage_migration(
    source_root: &Path,
    target_root: &Path,
) -> Result<MigrationResult, MigrationError> {
    stage_migration_impl(source_root, target_root, None)
}

pub(crate) fn stage_migration_with_failpoint(
    source_root: &Path,
    target_root: &Path,
    failpoint: MigrationFailPoint,
) -> Result<MigrationResult, MigrationError> {
    stage_migration_impl(source_root, target_root, Some(failpoint))
}

fn stage_migration_impl(
    source_root: &Path,
    target_root: &Path,
    failpoint: Option<MigrationFailPoint>,
) -> Result<MigrationResult, MigrationError> {
    let preview = inspect_migration(source_root, target_root)?;
    if !preview.conflicts.is_empty() {
        return Err(MigrationError::new(
            "TARGET_CONFLICT",
            preview.conflicts.join(","),
        ));
    }
    maybe_interrupt(failpoint, MigrationFailPoint::AfterPreflight)?;

    let prepared = prepare_input(source_root)?;
    let staging = staging_path(target_root);
    if staging.exists() {
        if !owned_staging_matches(&staging, target_root)? {
            return Err(MigrationError::new(
                "STAGING_CONFLICT",
                "existing staging is not owned by this migration target",
            ));
        }
        fs::remove_dir_all(&staging)
            .map_err(|error| MigrationError::io("discard incomplete owned staging", error))?;
    }
    fs::create_dir_all(staging.join("backup"))
        .map_err(|error| MigrationError::io("create staging", error))?;
    let schema = config_schema_version()?;
    let source_root_binding = canonical_binding(source_root)?;
    let target_root_binding = canonical_binding(target_root)?;
    let source_fingerprint = source_fingerprint(source_root)?;
    write_json(
        &staging.join(MARKER_FILE),
        &StagingMarker {
            engine: ENGINE_ID.to_string(),
            target_root: target_root_binding,
            source_root: source_root_binding,
            source_fingerprint,
            config_schema_version: schema,
            cleanup_policy: CLEANUP_ORDINARY.to_string(),
        },
    )?;
    write_state(&staging, MigrationState::Staging)?;

    write_json_value(&staging.join(SETTINGS_FILE), &prepared.settings)?;
    maybe_interrupt(failpoint, MigrationFailPoint::MidStage)?;
    write_json_value(&staging.join(WORKSPACES_FILE), &prepared.workspaces)?;
    write_json_value(
        &staging.join("backup").join(SETTINGS_FILE),
        &prepared.settings,
    )?;
    write_json_value(
        &staging.join("backup").join(WORKSPACES_FILE),
        &prepared.workspaces,
    )?;
    let authority = release_identity_authority()?;
    let prepared_content_sha256 =
        prepared_content_fingerprint(&staging.join(SETTINGS_FILE), &staging.join(WORKSPACES_FILE))?;
    write_json(
        &staging.join(MANIFEST_FILE),
        &MigrationManifest {
            config_schema_version: schema,
            target_product_name: authority.target.product_name,
            target_desktop_identifier: authority.target.desktop_identifier,
            activation_phase: "P4.1d".to_string(),
            credentials_migrated: false,
            remote_host_identity_migrated: false,
            prepared_content_sha256,
        },
    )?;
    write_json(&staging.join(REPORT_FILE), &preview)?;
    maybe_interrupt(failpoint, MigrationFailPoint::AfterStage)?;
    maybe_interrupt(failpoint, MigrationFailPoint::BeforeValidation)?;

    if let Err(error) = validate_staging_inner(target_root) {
        let _ = write_state(&staging, MigrationState::Failed);
        return Err(error);
    }
    write_state(&staging, MigrationState::Validated)?;
    maybe_interrupt(failpoint, MigrationFailPoint::AfterValidation)?;
    write_state(&staging, MigrationState::ReadyForActivation)?;
    Ok(MigrationResult {
        state: MigrationState::ReadyForActivation,
        staging_root: staging,
        config_schema_version: schema,
    })
}

pub(crate) fn validate_staging(target_root: &Path) -> Result<MigrationResult, MigrationError> {
    let staging = staging_path(target_root);
    match validate_staging_inner(target_root) {
        Ok(result) => Ok(result),
        Err(error) => {
            if owned_staging_matches(&staging, target_root).unwrap_or(false) {
                let _ = write_state(&staging, MigrationState::Failed);
            }
            Err(error)
        }
    }
}

fn validate_staging_inner(target_root: &Path) -> Result<MigrationResult, MigrationError> {
    let staging = staging_path(target_root);
    if !owned_staging_matches(&staging, target_root)? {
        return Err(MigrationError::new(
            "VALIDATION_FAILED",
            "staging ownership marker mismatch",
        ));
    }
    let marker: StagingMarker = read_json(&staging.join(MARKER_FILE), "staging marker")?;
    if marker.source_fingerprint != source_fingerprint(&marker.source_root)? {
        return Err(MigrationError::new(
            "VALIDATION_FAILED",
            "source changed after migration preview",
        ));
    }
    let settings_value = read_json_value(&staging.join(SETTINGS_FILE), "staged settings")?;
    let _: AppSettings = serde_json::from_value(settings_value.clone()).map_err(|_| {
        MigrationError::new("VALIDATION_FAILED", "staged settings schema is invalid")
    })?;
    let workspaces_value = read_json_value(&staging.join(WORKSPACES_FILE), "staged workspaces")?;
    let workspaces: Vec<WorkspaceEntry> = serde_json::from_value(workspaces_value.clone())
        .map_err(|_| {
            MigrationError::new("VALIDATION_FAILED", "staged workspaces schema is invalid")
        })?;
    validate_workspaces(&workspaces)?;
    validate_value_keys(&settings_value, "settings")?;
    validate_value_keys(&workspaces_value, "workspaces")?;

    let manifest: MigrationManifest =
        read_json(&staging.join(MANIFEST_FILE), "migration manifest")?;
    let authority = release_identity_authority()?;
    if manifest.config_schema_version != authority.config_schema_version
        || manifest.target_product_name != authority.target.product_name
        || manifest.target_desktop_identifier != authority.target.desktop_identifier
        || manifest.activation_phase != "P4.1d"
        || manifest.credentials_migrated
        || manifest.remote_host_identity_migrated
        || manifest.prepared_content_sha256
            != prepared_content_fingerprint(
                &staging.join(SETTINGS_FILE),
                &staging.join(WORKSPACES_FILE),
            )?
    {
        return Err(MigrationError::new(
            "VALIDATION_FAILED",
            "migration manifest is incompatible with target authority",
        ));
    }

    validate_tree_names(&staging)?;
    Ok(MigrationResult {
        state: read_state(&staging).unwrap_or(MigrationState::Staging),
        staging_root: staging,
        config_schema_version: authority.config_schema_version,
    })
}

pub(crate) fn rollback_staging(target_root: &Path) -> Result<bool, MigrationError> {
    let staging = staging_path(target_root);
    if !staging.exists() {
        return Ok(false);
    }
    if !owned_staging_matches(&staging, target_root)? {
        return Err(MigrationError::new(
            "ROLLBACK_REFUSED",
            "staging is not owned by this migration target",
        ));
    }
    let marker: StagingMarker = read_json(&staging.join(MARKER_FILE), "staging marker")?;
    if marker.source_fingerprint != source_fingerprint(&marker.source_root)? {
        return Err(MigrationError::new(
            "VALIDATION_FAILED",
            "source changed after migration preview",
        ));
    }
    if marker.cleanup_policy == CLEANUP_PROTECTED {
        return Err(MigrationError::new(
            "ROLLBACK_REFUSED",
            "staging contains protected activation recovery material",
        ));
    }
    fs::remove_dir_all(&staging)
        .map_err(|error| MigrationError::io("remove owned staging", error))?;
    Ok(true)
}

pub(crate) fn protect_staging_for_activation(target_root: &Path) -> Result<(), MigrationError> {
    validate_staging_inner(target_root)?;
    let staging = staging_path(target_root);
    let marker_path = staging.join(MARKER_FILE);
    let mut marker: StagingMarker = read_json(&marker_path, "staging marker")?;
    marker.cleanup_policy = CLEANUP_PROTECTED.to_string();
    write_json(&marker_path, &marker)
}

fn prepare_input(source_root: &Path) -> Result<PreparedInput, MigrationError> {
    detect_source_schema(source_root)?;
    let raw_settings = read_json_value(&source_root.join(SETTINGS_FILE), "legacy settings")?;
    let required = raw_settings
        .get("requiredMigrationFields")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .map(|value| value.as_str().map(str::to_string))
                .collect::<Option<Vec<_>>>()
        })
        .flatten()
        .unwrap_or_default();
    let settings_value = project_settings(&raw_settings)?;

    let raw_workspaces = read_json_value(&source_root.join(WORKSPACES_FILE), "legacy workspaces")?;
    let workspaces_value = project_workspaces(&raw_workspaces)?;
    let workspaces: Vec<WorkspaceEntry> = serde_json::from_value(workspaces_value.clone())
        .map_err(|_| MigrationError::new("PRECHECK_BLOCKED", "workspace projection invalid"))?;
    validate_workspaces(&workspaces)?;

    let mut excluded = Vec::new();
    collect_unknown_paths(&raw_settings, &settings_value, "settings", &mut excluded);
    collect_unknown_paths(
        &raw_workspaces,
        &workspaces_value,
        "workspaces",
        &mut excluded,
    );
    excluded.retain(|path| {
        !is_intentionally_sensitive_path(path) && path != "settings.requiredMigrationFields"
    });
    excluded.sort();
    excluded.dedup();

    let allowed_top_level = settings_value
        .as_object()
        .map(|object| object.keys().cloned().collect::<HashSet<_>>())
        .unwrap_or_default();
    let missing_required = required
        .into_iter()
        .filter(|field| !allowed_top_level.contains(field))
        .collect::<Vec<_>>();
    if !missing_required.is_empty() {
        return Err(MigrationError::new(
            "PRECHECK_BLOCKED",
            format!(
                "unknown required migration field(s): {}",
                missing_required.join(",")
            ),
        ));
    }

    Ok(PreparedInput {
        settings: settings_value,
        workspaces: workspaces_value,
        excluded_field_paths: excluded,
        credential_fields_excluded: count_credential_fields(&raw_settings),
    })
}

fn detect_source_schema(source_root: &Path) -> Result<u32, MigrationError> {
    let manifest_path = source_root.join(SOURCE_MANIFEST_FILE);
    if !manifest_path.exists() {
        return Ok(LEGACY_V0_SCHEMA);
    }
    let manifest = read_json_value(&manifest_path, "source profile manifest")?;
    let version = manifest
        .get("configSchemaVersion")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| {
            MigrationError::new("PRECHECK_BLOCKED", "source schema version is missing")
        })?;
    if version != LEGACY_V0_SCHEMA {
        return Err(MigrationError::new(
            "PRECHECK_BLOCKED",
            format!("unsupported source schema version: {version}"),
        ));
    }
    Ok(version)
}

fn project_settings(raw: &Value) -> Result<Value, MigrationError> {
    let raw_object = raw.as_object().ok_or_else(|| {
        MigrationError::new("PRECHECK_BLOCKED", "legacy settings must be an object")
    })?;
    let mut projected = Map::new();
    const SAFE_FIELDS: &[&str] = &[
        "defaultAccessMode",
        "reviewDeliveryMode",
        "composerModelShortcut",
        "composerAccessShortcut",
        "composerReasoningShortcut",
        "interruptShortcut",
        "composerCollaborationShortcut",
        "newAgentShortcut",
        "newWorktreeAgentShortcut",
        "newCloneAgentShortcut",
        "archiveThreadShortcut",
        "toggleProjectsSidebarShortcut",
        "toggleGitSidebarShortcut",
        "toggleDebugPanelShortcut",
        "toggleTerminalShortcut",
        "cycleAgentNextShortcut",
        "cycleAgentPrevShortcut",
        "cycleWorkspaceNextShortcut",
        "cycleWorkspacePrevShortcut",
        "lastComposerModelId",
        "lastComposerReasoningEffort",
        "uiScale",
        "theme",
        "usageShowRemaining",
        "showMessageFilePath",
        "chatHistoryScrollbackItems",
        "threadTitleAutogenerationEnabled",
        "uiFontFamily",
        "codeFontFamily",
        "codeFontSize",
        "notificationSoundsEnabled",
        "splitChatDiffView",
        "preloadGitDiffs",
        "gitDiffIgnoreWhitespaceChanges",
        "commitMessageModelId",
        "systemNotificationsEnabled",
        "subagentSystemNotificationsEnabled",
        "collaborationModesEnabled",
        "steerEnabled",
        "followUpMessageBehavior",
        "composerFollowUpHintEnabled",
        "pauseQueuedMessagesWhenResponseRequired",
        "unifiedExecEnabled",
        "experimentalAppsEnabled",
        "personality",
        "dictationEnabled",
        "dictationModelId",
        "dictationPreferredLanguage",
        "dictationHoldKey",
        "composerEditorPreset",
        "composerFenceExpandOnSpace",
        "composerFenceExpandOnEnter",
        "composerFenceLanguageTags",
        "composerFenceWrapSelection",
        "composerFenceAutoWrapPasteMultiline",
        "composerFenceAutoWrapPasteCodeLike",
        "composerListContinuation",
        "composerCodeBlockCopyUseModifier",
        "workspaceGroups",
        "globalWorktreesFolder",
    ];
    for field in SAFE_FIELDS {
        if let Some(value) = raw_object.get(*field) {
            projected.insert((*field).to_string(), value.clone());
        }
    }

    let remote_backends = raw_object
        .get("remoteBackends")
        .and_then(Value::as_array)
        .map(|targets| {
            targets
                .iter()
                .filter_map(Value::as_object)
                .map(|target| {
                    let mut safe = Map::new();
                    for field in ["id", "name", "provider", "host"] {
                        if let Some(value) = target.get(field) {
                            safe.insert(field.to_string(), value.clone());
                        }
                    }
                    safe.insert("lastConnectedAtMs".to_string(), Value::Null);
                    Value::Object(safe)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    projected.insert("remoteBackends".to_string(), Value::Array(remote_backends));
    projected.insert(
        "backendMode".to_string(),
        Value::String("local".to_string()),
    );
    projected.insert("activeRemoteBackendId".to_string(), Value::Null);
    projected.insert(
        "automaticAppUpdateChecksEnabled".to_string(),
        Value::Bool(false),
    );
    projected.insert(
        "keepDaemonRunningAfterAppClose".to_string(),
        Value::Bool(false),
    );
    let value = Value::Object(projected);
    let _: AppSettings = serde_json::from_value(value.clone()).map_err(|_| {
        MigrationError::new("PRECHECK_BLOCKED", "allowlisted settings schema is invalid")
    })?;
    Ok(value)
}

fn project_workspaces(raw: &Value) -> Result<Value, MigrationError> {
    let workspaces = raw.as_array().ok_or_else(|| {
        MigrationError::new("PRECHECK_BLOCKED", "legacy workspaces must be an array")
    })?;
    let projected = workspaces
        .iter()
        .map(|workspace| {
            let raw = workspace.as_object().ok_or_else(|| {
                MigrationError::new("PRECHECK_BLOCKED", "workspace must be an object")
            })?;
            let mut output = Map::new();
            for field in ["id", "name", "path", "kind", "parentId", "worktree"] {
                if let Some(value) = raw.get(field) {
                    output.insert(field.to_string(), value.clone());
                }
            }
            let mut settings = Map::new();
            if let Some(raw_settings) = raw.get("settings").and_then(Value::as_object) {
                for field in [
                    "sidebarCollapsed",
                    "sortOrder",
                    "groupId",
                    "cloneSourceWorkspaceId",
                    "gitRoot",
                    "worktreesFolder",
                ] {
                    if let Some(value) = raw_settings.get(field) {
                        settings.insert(field.to_string(), value.clone());
                    }
                }
            }
            output.insert("settings".to_string(), Value::Object(settings));
            Ok(Value::Object(output))
        })
        .collect::<Result<Vec<_>, MigrationError>>()?;
    Ok(Value::Array(projected))
}

fn validate_workspaces(workspaces: &[WorkspaceEntry]) -> Result<(), MigrationError> {
    let mut ids = HashSet::new();
    for workspace in workspaces {
        if workspace.id.trim().is_empty() || !ids.insert(workspace.id.as_str()) {
            return Err(MigrationError::new(
                "PRECHECK_BLOCKED",
                "workspace identifiers must be non-empty and unique",
            ));
        }
        if workspace.path.trim().is_empty()
            || workspace.path.contains('\0')
            || !Path::new(&workspace.path).is_absolute()
        {
            return Err(MigrationError::new(
                "PRECHECK_BLOCKED",
                "workspace path must be a valid absolute path",
            ));
        }
    }
    Ok(())
}

fn release_identity_authority() -> Result<ReleaseIdentityAuthority, MigrationError> {
    serde_json::from_str(include_str!("../../../release-identity.json")).map_err(|_| {
        MigrationError::new(
            "AUTHORITY_INVALID",
            "release identity authority cannot be parsed",
        )
    })
}

fn read_json_value(path: &Path, label: &'static str) -> Result<Value, MigrationError> {
    let bytes = fs::read(path).map_err(|error| MigrationError::io(label, error))?;
    serde_json::from_slice(&bytes)
        .map_err(|_| MigrationError::new("PRECHECK_BLOCKED", format!("{label} JSON is invalid")))
}

fn read_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
    label: &'static str,
) -> Result<T, MigrationError> {
    let bytes = fs::read(path).map_err(|error| MigrationError::io(label, error))?;
    serde_json::from_slice(&bytes)
        .map_err(|_| MigrationError::new("VALIDATION_FAILED", format!("{label} is invalid")))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), MigrationError> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|_| MigrationError::new("STAGING_FAILED", "serialize sanitized artifact"))?;
    fs::write(path, bytes).map_err(|error| MigrationError::io("write staging artifact", error))
}

fn write_json_value(path: &Path, value: &Value) -> Result<(), MigrationError> {
    write_json(path, value)
}

fn write_state(staging: &Path, state: MigrationState) -> Result<(), MigrationError> {
    write_json(&staging.join(STATE_FILE), &StagingState { state })
}

fn read_state(staging: &Path) -> Result<MigrationState, MigrationError> {
    let state: StagingState = read_json(&staging.join(STATE_FILE), "migration state")?;
    Ok(state.state)
}

fn owned_staging_matches(staging: &Path, target_root: &Path) -> Result<bool, MigrationError> {
    if !staging.is_dir() {
        return Ok(false);
    }
    let marker_path = staging.join(MARKER_FILE);
    if !marker_path.is_file() {
        return Ok(false);
    }
    let marker: StagingMarker = read_json(&marker_path, "staging marker")?;
    Ok(marker.engine == ENGINE_ID
        && marker.target_root == canonical_binding(target_root)?
        && marker.config_schema_version == config_schema_version()?)
}

fn validate_root_relationships(
    source_root: &Path,
    target_root: &Path,
) -> Result<(), MigrationError> {
    if !source_root.is_dir() {
        return Err(MigrationError::new(
            "PRECHECK_BLOCKED",
            "explicit sourceRoot is not a directory",
        ));
    }
    reject_reparse_components(source_root)?;
    reject_reparse_components(target_root)?;
    let source = canonical_binding(source_root)?;
    let target = canonical_binding(target_root)?;
    let staging = canonical_binding(&staging_path(target_root))?;
    if source == target
        || source.starts_with(&target)
        || target.starts_with(&source)
        || source == staging
        || staging.starts_with(&source)
    {
        return Err(MigrationError::new(
            "PRECHECK_BLOCKED",
            "source, target, and staging roots must be distinct and non-nested",
        ));
    }
    Ok(())
}

fn canonical_binding(path: &Path) -> Result<PathBuf, MigrationError> {
    if path.exists() {
        return fs::canonicalize(path)
            .map_err(|error| MigrationError::io("resolve path identity", error));
    }
    let mut missing = Vec::new();
    let mut cursor = path;
    while !cursor.exists() {
        let name = cursor.file_name().ok_or_else(|| {
            MigrationError::new("PRECHECK_BLOCKED", "path has no resolvable parent")
        })?;
        missing.push(name.to_os_string());
        cursor = cursor.parent().ok_or_else(|| {
            MigrationError::new("PRECHECK_BLOCKED", "path has no resolvable parent")
        })?;
    }
    let mut resolved = fs::canonicalize(cursor)
        .map_err(|error| MigrationError::io("resolve path parent identity", error))?;
    for name in missing.into_iter().rev() {
        resolved.push(name);
    }
    Ok(resolved)
}

#[cfg(windows)]
fn reject_reparse_components(path: &Path) -> Result<(), MigrationError> {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    let mut cursor = Some(path);
    while let Some(component) = cursor {
        if component.exists() {
            let attributes = fs::symlink_metadata(component)
                .map_err(|error| MigrationError::io("inspect path attributes", error))?
                .file_attributes();
            if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                return Err(MigrationError::new(
                    "PRECHECK_BLOCKED",
                    "reparse-point path components are not accepted",
                ));
            }
        }
        cursor = component.parent();
    }
    Ok(())
}

#[cfg(not(windows))]
fn reject_reparse_components(path: &Path) -> Result<(), MigrationError> {
    let mut cursor = Some(path);
    while let Some(component) = cursor {
        if component.exists()
            && fs::symlink_metadata(component)
                .map_err(|error| MigrationError::io("inspect path attributes", error))?
                .file_type()
                .is_symlink()
        {
            return Err(MigrationError::new(
                "PRECHECK_BLOCKED",
                "symbolic-link path components are not accepted",
            ));
        }
        cursor = component.parent();
    }
    Ok(())
}

fn source_fingerprint(source_root: &Path) -> Result<String, MigrationError> {
    let mut hasher = Sha256::new();
    for name in [SOURCE_MANIFEST_FILE, SETTINGS_FILE, WORKSPACES_FILE] {
        let path = source_root.join(name);
        if path.exists() {
            hasher.update(name.as_bytes());
            hasher.update(
                fs::read(path).map_err(|error| MigrationError::io("fingerprint source", error))?,
            );
        }
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn prepared_content_fingerprint(
    settings_path: &Path,
    workspaces_path: &Path,
) -> Result<String, MigrationError> {
    let mut hasher = Sha256::new();
    for path in [settings_path, workspaces_path] {
        hasher.update(
            fs::read(path)
                .map_err(|error| MigrationError::io("fingerprint prepared content", error))?,
        );
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn directory_has_entries(path: &Path) -> Result<bool, MigrationError> {
    if !path.exists() {
        return Ok(false);
    }
    if !path.is_dir() {
        return Ok(true);
    }
    Ok(fs::read_dir(path)
        .map_err(|error| MigrationError::io("inspect targetRoot", error))?
        .next()
        .is_some())
}

fn remove_object_key(value: &mut Value, key: &str) {
    if let Some(object) = value.as_object_mut() {
        object.remove(key);
    }
}

fn collect_unknown_paths(raw: &Value, normalized: &Value, base: &str, output: &mut Vec<String>) {
    match (raw, normalized) {
        (Value::Object(raw), Value::Object(normalized)) => {
            for (key, raw_value) in raw {
                let path = format!("{base}.{key}");
                if let Some(normalized_value) = normalized.get(key) {
                    collect_unknown_paths(raw_value, normalized_value, &path, output);
                } else {
                    output.push(path);
                }
            }
        }
        (Value::Array(raw), Value::Array(normalized)) => {
            for (index, raw_value) in raw.iter().enumerate() {
                if let Some(normalized_value) = normalized.get(index) {
                    collect_unknown_paths(
                        raw_value,
                        normalized_value,
                        &format!("{base}[{index}]"),
                        output,
                    );
                }
            }
        }
        _ => {}
    }
}

fn is_intentionally_sensitive_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".token")
        || lower.ends_with("backendtoken")
        || lower.ends_with("remotehostidentity")
        || lower.contains(".auth")
        || lower.contains(".credential")
        || lower.contains(".secret")
        || lower.contains("apikey")
}

fn count_credential_fields(value: &Value) -> usize {
    match value {
        Value::Object(object) => object
            .iter()
            .map(|(key, value)| {
                let lower = key.to_ascii_lowercase();
                usize::from(
                    lower.contains("token")
                        || lower.contains("credential")
                        || lower.contains("secret")
                        || lower.contains("apikey")
                        || lower == "auth",
                ) + count_credential_fields(value)
            })
            .sum(),
        Value::Array(array) => array.iter().map(count_credential_fields).sum(),
        _ => 0,
    }
}

fn validate_value_keys(value: &Value, base: &str) -> Result<(), MigrationError> {
    match value {
        Value::Object(object) => {
            for (key, nested) in object {
                let lower = key.to_ascii_lowercase();
                if matches!(
                    lower.as_str(),
                    "token"
                        | "remotebackendtoken"
                        | "authtoken"
                        | "auth"
                        | "secret"
                        | "apikey"
                        | "remotehostidentity"
                        | "remotetransportgeneration"
                        | "workspacesessiongeneration"
                        | "appserverconnectiongeneration"
                        | "daemonprocessgeneration"
                        | "pendingrequest"
                        | "pendingapprovaldecision"
                        | "pendingdeleteattempt"
                ) {
                    return Err(MigrationError::new(
                        "VALIDATION_FAILED",
                        format!("forbidden staged field: {base}.{key}"),
                    ));
                }
                validate_value_keys(nested, &format!("{base}.{key}"))?;
            }
        }
        Value::Array(array) => {
            for (index, nested) in array.iter().enumerate() {
                validate_value_keys(nested, &format!("{base}[{index}]"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_tree_names(root: &Path) -> Result<(), MigrationError> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let entries = fs::read_dir(&directory)
            .map_err(|error| MigrationError::io("validate staged files", error))?;
        for entry in entries {
            let path = entry
                .map_err(|error| MigrationError::io("validate staged entry", error))?
                .path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if name == "auth.json"
                || name == "remote-host-identity.json"
                || name.contains("generation")
                || name.starts_with("rollout-")
                || name.ends_with(".jsonl")
            {
                return Err(MigrationError::new(
                    "VALIDATION_FAILED",
                    "forbidden artifact exists in staging",
                ));
            }
            if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                let value = read_json_value(&path, "staged JSON")?;
                validate_value_keys(&value, "artifact")?;
            }
        }
    }
    Ok(())
}

fn is_runtime_or_transient_name(name: &str) -> bool {
    name.ends_with(".pid")
        || name.ends_with(".lock")
        || name.ends_with(".tmp")
        || name.contains("generation")
        || name.contains("pending")
        || name.contains("cache")
        || name.contains("socket")
}

fn is_credential_name(name: &str) -> bool {
    name == "auth.json"
        || name.contains("credential")
        || name.contains("secret")
        || name.contains("api-key")
        || name.contains("apikey")
        || name.ends_with(".token")
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

fn maybe_interrupt(
    configured: Option<MigrationFailPoint>,
    current: MigrationFailPoint,
) -> Result<(), MigrationError> {
    if configured == Some(current) {
        return Err(MigrationError::new(
            "INTERRUPTED",
            "synthetic migration interruption",
        ));
    }
    Ok(())
}
