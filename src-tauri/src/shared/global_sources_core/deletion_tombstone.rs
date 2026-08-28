use super::rollout_identity::CodexThreadKey;
use super::source_envelope::SourceFileIdentity;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DeletionReconciliationState {
    Pending,
    Completed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DesktopReconciliationState {
    Unknown,
    RefreshPending,
    Reconciled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeletionTombstone {
    pub monitor_delete_operation_id: String,
    pub upstream_request_id: Option<String>,
    pub root_thread_key: CodexThreadKey,
    pub descendant_thread_keys: Vec<CodexThreadKey>,
    pub deleted_at_ms: i64,
    pub delete_response_confirmed: bool,
    pub thread_deleted_confirmations: Vec<String>,
    pub post_delete_thread_list_absent: Vec<String>,
    pub retired_source_files: Vec<SourceFileIdentity>,
    pub reconciliation_state: DeletionReconciliationState,
    pub desktop_reconciliation: DesktopReconciliationState,
}

impl DeletionTombstone {
    pub(crate) fn confirmed(
        monitor_delete_operation_id: impl Into<String>,
        root_thread_key: CodexThreadKey,
        descendant_thread_keys: Vec<CodexThreadKey>,
        deleted_at_ms: i64,
    ) -> Self {
        Self {
            monitor_delete_operation_id: monitor_delete_operation_id.into(),
            upstream_request_id: None,
            root_thread_key,
            descendant_thread_keys,
            deleted_at_ms,
            delete_response_confirmed: true,
            thread_deleted_confirmations: Vec::new(),
            post_delete_thread_list_absent: Vec::new(),
            retired_source_files: Vec::new(),
            reconciliation_state: DeletionReconciliationState::Pending,
            desktop_reconciliation: DesktopReconciliationState::Unknown,
        }
    }

    pub(crate) fn thread_keys(&self) -> impl Iterator<Item = &CodexThreadKey> {
        std::iter::once(&self.root_thread_key).chain(self.descendant_thread_keys.iter())
    }

    pub(crate) fn contains_thread_key(&self, key: &CodexThreadKey) -> bool {
        self.thread_keys().any(|candidate| candidate == key)
    }

    pub(crate) fn record_thread_deleted_confirmation(&mut self, thread_id: &str) -> bool {
        if !self.thread_keys().any(|key| key.thread_id == thread_id)
            || self
                .thread_deleted_confirmations
                .iter()
                .any(|candidate| candidate == thread_id)
        {
            return false;
        }
        self.thread_deleted_confirmations
            .push(thread_id.to_string());
        self.thread_deleted_confirmations.sort();
        true
    }

    pub(crate) fn record_retired_source_file(&mut self, source_file: SourceFileIdentity) -> bool {
        if self.retired_source_files.contains(&source_file) {
            return false;
        }
        self.retired_source_files.push(source_file);
        self.retired_source_files.sort_by(|left, right| {
            left.normalized_path
                .cmp(&right.normalized_path)
                .then_with(|| left.generation.cmp(&right.generation))
        });
        true
    }

    pub(crate) fn mark_local_reconciliation_completed(&mut self) {
        self.reconciliation_state = DeletionReconciliationState::Completed;
        if self.desktop_reconciliation == DesktopReconciliationState::Unknown {
            self.desktop_reconciliation = DesktopReconciliationState::RefreshPending;
        }
    }

    pub(crate) fn record_desktop_absence<'a>(
        &mut self,
        absent_thread_ids: impl IntoIterator<Item = &'a str>,
    ) -> bool {
        let mut changed = false;
        for thread_id in absent_thread_ids {
            if self.thread_keys().any(|key| key.thread_id == thread_id)
                && !self
                    .post_delete_thread_list_absent
                    .iter()
                    .any(|candidate| candidate == thread_id)
            {
                self.post_delete_thread_list_absent
                    .push(thread_id.to_string());
                changed = true;
            }
        }
        self.post_delete_thread_list_absent.sort();
        let all_absent = self.thread_keys().all(|key| {
            self.post_delete_thread_list_absent
                .iter()
                .any(|candidate| candidate == &key.thread_id)
        });
        if all_absent && self.desktop_reconciliation != DesktopReconciliationState::Reconciled {
            self.desktop_reconciliation = DesktopReconciliationState::Reconciled;
            changed = true;
        }
        changed
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeletionTombstoneDocument {
    pub version: u32,
    pub operations: Vec<DeletionTombstone>,
}

impl Default for DeletionTombstoneDocument {
    fn default() -> Self {
        Self {
            version: 1,
            operations: Vec::new(),
        }
    }
}

impl DeletionTombstoneDocument {
    pub(crate) fn upsert(&mut self, tombstone: DeletionTombstone) -> &mut DeletionTombstone {
        if let Some(index) = self.operations.iter().position(|operation| {
            operation.monitor_delete_operation_id == tombstone.monitor_delete_operation_id
        }) {
            return &mut self.operations[index];
        }
        self.operations.push(tombstone);
        self.operations.last_mut().expect("inserted tombstone")
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DeletionTombstoneStore {
    path: PathBuf,
}

impl DeletionTombstoneStore {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub(crate) fn load(&self) -> io::Result<DeletionTombstoneDocument> {
        match fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid deletion tombstone document: {error}"),
                )
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Ok(DeletionTombstoneDocument::default())
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) fn save(&self, document: &DeletionTombstoneDocument) -> io::Result<()> {
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let file_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("deletion-tombstones.json");
        let temporary = parent.join(format!(".{file_name}.{}.tmp", Uuid::new_v4()));
        let result = (|| {
            let bytes = serde_json::to_vec_pretty(document).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("serialize deletion tombstones: {error}"),
                )
            })?;
            let mut file = File::create(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temporary, &self.path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}
