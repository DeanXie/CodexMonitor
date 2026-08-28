use super::rollout_identity::CodexThreadKey;
use super::rollout_tail::RolloutCheckpoint;
use super::source_envelope::SourceFileIdentity;
use super::source_registry::{ExternalLifecycle, TokenSnapshot};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RolloutAdapterCheckpoint {
    pub thread_key: Option<CodexThreadKey>,
    pub root_session_id: Option<String>,
    pub parent_thread_key: Option<CodexThreadKey>,
    pub agent_path: Option<String>,
    pub active_turn_id: Option<String>,
    pub lifecycle: Option<ExternalLifecycle>,
    pub observed_model: Option<String>,
    pub token_snapshot: Option<TokenSnapshot>,
    pub source_timestamp_ms: Option<i64>,
    pub producer_version: Option<String>,
    pub completed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RolloutSourceCheckpoint {
    pub codex_home_identity: String,
    pub source_file: SourceFileIdentity,
    pub tail: RolloutCheckpoint,
    #[serde(default)]
    pub adapter: RolloutAdapterCheckpoint,
    pub last_complete_record_observed_at_ms: Option<i64>,
    pub last_successful_read_at_ms: Option<i64>,
    pub last_filesystem_signal_at_ms: Option<i64>,
}

impl RolloutSourceCheckpoint {
    pub(crate) fn new(
        codex_home_identity: String,
        source_file: SourceFileIdentity,
        tail: RolloutCheckpoint,
    ) -> Self {
        Self {
            codex_home_identity,
            source_file,
            tail,
            adapter: RolloutAdapterCheckpoint::default(),
            last_complete_record_observed_at_ms: None,
            last_successful_read_at_ms: None,
            last_filesystem_signal_at_ms: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RolloutWatcherCheckpoint {
    pub version: u32,
    pub sources: Vec<RolloutSourceCheckpoint>,
}

impl Default for RolloutWatcherCheckpoint {
    fn default() -> Self {
        Self {
            version: 1,
            sources: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RolloutCheckpointStore {
    path: PathBuf,
}

impl RolloutCheckpointStore {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub(crate) fn load(&self) -> io::Result<RolloutWatcherCheckpoint> {
        match fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid rollout checkpoint: {error}"),
                )
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Ok(RolloutWatcherCheckpoint::default())
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) fn save(&self, checkpoint: &RolloutWatcherCheckpoint) -> io::Result<()> {
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let file_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("rollout-watcher-checkpoint.json");
        let temporary = parent.join(format!(".{file_name}.{}.tmp", Uuid::new_v4()));
        let result = (|| {
            let bytes = serde_json::to_vec_pretty(checkpoint).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("serialize checkpoint: {error}"),
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
