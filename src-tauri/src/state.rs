use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::process::Child;
use tokio::sync::Mutex;

use crate::dictation::DictationState;
use crate::shared::codex_core::CodexLoginCancelState;
use crate::storage::{read_settings, read_workspaces};
use crate::types::{AppSettings, TcpDaemonState, TcpDaemonStatus, WorkspaceEntry};

pub(crate) struct TcpDaemonRuntime {
    pub(crate) child: Option<Child>,
    pub(crate) status: TcpDaemonStatus,
}

impl Default for TcpDaemonRuntime {
    fn default() -> Self {
        Self {
            child: None,
            status: TcpDaemonStatus {
                state: TcpDaemonState::Stopped,
                pid: None,
                started_at_ms: None,
                last_error: None,
                listen_addr: None,
            },
        }
    }
}

pub(crate) struct AppState {
    pub(crate) creation_coordinator:
        crate::shared::codex_core::creation_coordination::CreationCoordinator,
    pub(crate) execution_settings_evidence:
        crate::shared::execution_settings_ingestion::ExecutionSettingsEvidenceRuntime,
    pub(crate) projection_freshness:
        crate::shared::projection_freshness::ProjectionFreshnessRuntime,
    pub(crate) workspaces: Mutex<HashMap<String, WorkspaceEntry>>,
    pub(crate) sessions: Mutex<HashMap<String, Arc<crate::codex::WorkspaceSession>>>,
    pub(crate) terminal_sessions: Mutex<HashMap<String, Arc<crate::terminal::TerminalSession>>>,
    pub(crate) remote_backend: crate::remote_backend::RemoteBackendCache,
    pub(crate) remote_host_availability:
        Arc<crate::shared::remote_host_availability::RemoteHostAvailabilityRuntime>,
    pub(crate) storage_path: PathBuf,
    pub(crate) settings_path: PathBuf,
    pub(crate) app_settings: Mutex<AppSettings>,
    pub(crate) dictation: Mutex<DictationState>,
    pub(crate) codex_login_cancels: Mutex<HashMap<String, CodexLoginCancelState>>,
    pub(crate) tcp_daemon: Mutex<TcpDaemonRuntime>,
    #[cfg(desktop)]
    pub(crate) remote_host_identity:
        Result<crate::shared::remote_host_identity::RemoteHostIdentity, String>,
    #[cfg(desktop)]
    pub(crate) global_rollout_runtime: crate::global_sources::runtime::GlobalRolloutRuntime,
}

impl AppState {
    pub(crate) fn load_for_runtime_validation(
        data_dir: PathBuf,
    ) -> Result<
        (
            Self,
            crate::shared::startup_activation::ActivatedProfileMetadata,
        ),
        String,
    > {
        let metadata = crate::shared::startup_activation::validate_committed_profile(&data_dir)?;
        let storage_path = data_dir.join("workspaces.json");
        let settings_path = data_dir.join("settings.json");
        let workspaces = read_workspaces(&storage_path)
            .map_err(|error| format!("read activated workspaces: {error}"))?;
        let app_settings = read_settings(&settings_path)
            .map_err(|error| format!("read activated settings: {error}"))?;
        #[cfg(desktop)]
        let remote_host_identity = crate::shared::remote_host_identity::RemoteHostIdentity::parse(
            &metadata.remote_host_identity,
        );
        let state = Self {
            creation_coordinator: Default::default(),
            execution_settings_evidence: Default::default(),
            projection_freshness: Default::default(),
            workspaces: Mutex::new(workspaces),
            sessions: Mutex::new(HashMap::new()),
            terminal_sessions: Mutex::new(HashMap::new()),
            remote_backend: Default::default(),
            remote_host_availability: Arc::new(Default::default()),
            storage_path,
            settings_path,
            app_settings: Mutex::new(app_settings),
            dictation: Mutex::new(DictationState::default()),
            codex_login_cancels: Mutex::new(HashMap::new()),
            tcp_daemon: Mutex::new(TcpDaemonRuntime::default()),
            #[cfg(desktop)]
            remote_host_identity,
            #[cfg(desktop)]
            global_rollout_runtime: crate::global_sources::runtime::GlobalRolloutRuntime::default(),
        };
        Ok((state, metadata))
    }

    #[cfg(not(desktop))]
    pub(crate) fn load_mobile(app: &tauri::AppHandle) -> Result<Self, String> {
        use tauri::Manager;
        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|error| format!("failed to resolve mobile app data root: {error}"))?;
        let storage_path = data_dir.join("workspaces.json");
        let settings_path = data_dir.join("settings.json");
        let workspaces = read_workspaces(&storage_path).unwrap_or_default();
        let app_settings = read_settings(&settings_path).unwrap_or_default();
        Ok(Self {
            creation_coordinator: Default::default(),
            execution_settings_evidence: Default::default(),
            projection_freshness: Default::default(),
            workspaces: Mutex::new(workspaces),
            sessions: Mutex::new(HashMap::new()),
            terminal_sessions: Mutex::new(HashMap::new()),
            remote_backend: Default::default(),
            remote_host_availability: Arc::new(Default::default()),
            storage_path,
            settings_path,
            app_settings: Mutex::new(app_settings),
            dictation: Mutex::new(DictationState::default()),
            codex_login_cancels: Mutex::new(HashMap::new()),
            tcp_daemon: Mutex::new(TcpDaemonRuntime::default()),
        })
    }
}

#[cfg(all(test, desktop))]
mod tests {
    use super::AppState;
    use crate::bootstrap::BootstrapState;
    use crate::shared::activation_foundation::{
        activation_journal_path, commit_fresh_activation, prepare_fresh_activation,
    };
    use serde_json::{json, Value};
    use std::fs;
    use uuid::Uuid;

    fn committed_profile(label: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "codex-monitor-p4-1d3-app-{label}-{}",
            Uuid::new_v4()
        ));
        prepare_fresh_activation(&root, "014383f2-41f8-4b13-b9d7-30c511e47cec").unwrap();
        commit_fresh_activation(&root).unwrap();
        root
    }

    fn journal_state(root: &std::path::Path) -> String {
        serde_json::from_slice::<Value>(&fs::read(activation_journal_path(root)).unwrap()).unwrap()
            ["state"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn remove_fixture(root: &std::path::Path) {
        let journal = activation_journal_path(root);
        if root.exists() {
            fs::remove_dir_all(root).unwrap();
        }
        if journal.exists() {
            fs::remove_file(journal).unwrap();
        }
    }

    #[test]
    fn app_candidate_load_is_restricted_and_does_not_publish_runtime_success() {
        let root = committed_profile("candidate-success");
        let (_, metadata) = AppState::load_for_runtime_validation(root.clone()).unwrap();
        assert!(!metadata.transaction_id.is_empty());
        assert_eq!(journal_state(&root), "target_committed");
        remove_fixture(&root);
    }

    #[test]
    fn app_production_handshake_opens_business_access_only_after_candidate_validation() {
        let root = committed_profile("production-handshake");
        let bootstrap = BootstrapState::inspect(root.clone());
        assert!(!bootstrap.business_access_allowed().unwrap());

        bootstrap.begin_runtime_validation().unwrap();
        let (_candidate, metadata) = AppState::load_for_runtime_validation(root.clone()).unwrap();
        assert_eq!(journal_state(&root), "target_committed");
        assert!(!bootstrap.business_access_allowed().unwrap());

        bootstrap
            .complete_runtime_validation(&metadata.transaction_id)
            .unwrap();
        assert_eq!(journal_state(&root), "runtime_validated");
        assert!(bootstrap.business_access_allowed().unwrap());
        assert!(bootstrap.begin_runtime_validation().is_err());
        remove_fixture(&root);
    }

    #[test]
    fn app_candidate_failure_preserves_committed_identity_and_journal() {
        let root = committed_profile("candidate-failure");
        let identity_before = fs::read(root.join("remote-host-identity.json")).unwrap();
        fs::write(
            root.join("workspaces.json"),
            serde_json::to_vec_pretty(&json!([{
                "id": 42,
                "name": false,
                "path": [],
                "kind": "main"
            }]))
            .unwrap(),
        )
        .unwrap();

        assert!(AppState::load_for_runtime_validation(root.clone()).is_err());
        assert_eq!(journal_state(&root), "target_committed");
        assert_eq!(
            fs::read(root.join("remote-host-identity.json")).unwrap(),
            identity_before
        );
        remove_fixture(&root);
    }
}
