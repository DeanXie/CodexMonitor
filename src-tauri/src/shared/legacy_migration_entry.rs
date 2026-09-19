use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StopEvidenceState {
    Running,
    Unknown,
    VerifiedQuiescentWithinSupportedScope,
}

impl StopEvidenceState {
    fn diagnostic_code(self) -> &'static str {
        match self {
            Self::Running => "RUNNING",
            Self::Unknown => "UNKNOWN",
            Self::VerifiedQuiescentWithinSupportedScope => {
                "VERIFIED_QUIESCENT_WITHIN_SUPPORTED_SCOPE"
            }
        }
    }
}

pub(crate) trait LegacyProcessStopGuard: Send {
    fn revalidate(&mut self, source_root: &Path) -> Result<StopEvidenceState, String>;
}

pub(crate) enum StopEvidenceAcquisition {
    Blocked(StopEvidenceState),
    Verified(Box<dyn LegacyProcessStopGuard>),
}

pub(crate) trait LegacyProcessStopProvider: Send + Sync {
    fn acquire(&self, source_root: &Path) -> Result<StopEvidenceAcquisition, String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LegacyMigrationPreview {
    pub(crate) preview_id: String,
    pub(crate) source_schema_version: u32,
    pub(crate) target_schema_version: u32,
    pub(crate) migratable_categories: Vec<String>,
    pub(crate) excluded_categories: Vec<String>,
    pub(crate) deferred_categories: Vec<String>,
    pub(crate) warnings: Vec<String>,
    pub(crate) conflicts: Vec<String>,
    pub(crate) restart_required: bool,
}

#[derive(Debug)]
struct BoundPreview {
    created_at: Instant,
    source_snapshot: String,
    migration_snapshot: String,
}

pub(crate) struct LegacyMigrationCoordinator {
    source_root: PathBuf,
    target_root: PathBuf,
    provider: Arc<dyn LegacyProcessStopProvider>,
    ttl: Duration,
    previews: Mutex<HashMap<String, BoundPreview>>,
}

impl LegacyMigrationCoordinator {
    pub(crate) fn new(
        source_root: PathBuf,
        target_root: PathBuf,
        provider: Arc<dyn LegacyProcessStopProvider>,
        ttl: Duration,
    ) -> Self {
        Self {
            source_root,
            target_root,
            provider,
            ttl,
            previews: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn preview(&self) -> Result<LegacyMigrationPreview, String> {
        let inspected =
            super::migration_core::inspect_migration(&self.source_root, &self.target_root)
                .map_err(|error| error.to_string())?;
        let preview_id = Uuid::new_v4().to_string();
        let source_snapshot = source_snapshot(&self.source_root)?;
        let migration_snapshot = hash_serializable(&inspected)?;
        let public = LegacyMigrationPreview {
            preview_id: preview_id.clone(),
            source_schema_version: inspected.source_schema_version,
            target_schema_version: inspected.target_schema_version,
            migratable_categories: inspected.migratable_categories,
            excluded_categories: inspected.excluded_categories,
            deferred_categories: inspected.deferred_categories,
            warnings: inspected.warnings,
            conflicts: inspected.conflicts,
            restart_required: true,
        };
        self.previews
            .lock()
            .map_err(|_| "legacy migration preview registry is poisoned".to_string())?
            .insert(
                preview_id,
                BoundPreview {
                    created_at: Instant::now(),
                    source_snapshot,
                    migration_snapshot,
                },
            );
        Ok(public)
    }

    pub(crate) fn confirm(&self, preview_id: &str, intent: &str) -> Result<(), String> {
        if intent != "confirm_legacy_migration" {
            return Err("explicit legacy migration intent is required".to_string());
        }
        let preview = self
            .previews
            .lock()
            .map_err(|_| "legacy migration preview registry is poisoned".to_string())?
            .remove(preview_id)
            .ok_or("legacy migration preview is absent or was already consumed")?;
        if preview.created_at.elapsed() > self.ttl {
            return Err("legacy migration preview expired".to_string());
        }
        if source_snapshot(&self.source_root)? != preview.source_snapshot {
            return Err("legacy migration source changed after preview".to_string());
        }
        let current =
            super::migration_core::inspect_migration(&self.source_root, &self.target_root)
                .map_err(|error| error.to_string())?;
        if hash_serializable(&current)? != preview.migration_snapshot {
            return Err("legacy migration bindings changed after preview".to_string());
        }
        match self.provider.acquire(&self.source_root)? {
            StopEvidenceAcquisition::Blocked(state) => Err(format!(
                "legacy process stop evidence blocked migration: {}",
                state.diagnostic_code()
            )),
            StopEvidenceAcquisition::Verified(mut stop_guard) => {
                let journal_exists =
                    super::activation_foundation::activation_journal_path(&self.target_root)
                        .is_file();
                let resumes_prepared = journal_exists
                    && super::activation_foundation::recover_activation(&self.target_root)?
                        == super::activation_foundation::RecoveryDisposition::ContinueBeforeRetirement;
                if !resumes_prepared {
                    super::migration_core::stage_migration(&self.source_root, &self.target_root)
                        .map_err(|error| error.to_string())?;
                    super::activation_foundation::prepare_migration_activation(
                        &self.source_root,
                        &self.target_root,
                    )?;
                }
                super::activation_foundation::commit_migration_activation_with_stop_guard(
                    &self.target_root,
                    None,
                    stop_guard.as_mut(),
                )
            }
        }
    }
}

fn hash_serializable(value: &impl Serialize) -> Result<String, String> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| format!("serialize migration snapshot: {error}"))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn source_snapshot(root: &Path) -> Result<String, String> {
    let mut names = fs::read_dir(root)
        .map_err(|error| format!("enumerate legacy migration source: {error}"))?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("enumerate legacy migration source entry: {error}"))?;
    names.sort();
    let mut digest = Sha256::new();
    for name in names {
        digest.update(name.to_string_lossy().as_bytes());
        let path = root.join(&name);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("inspect legacy migration source entry: {error}"))?;
        digest.update(metadata.len().to_le_bytes());
        if metadata.is_file() {
            digest.update(
                fs::read(&path)
                    .map_err(|error| format!("read legacy migration source entry: {error}"))?,
            );
        }
    }
    Ok(format!("{:x}", digest.finalize()))
}
