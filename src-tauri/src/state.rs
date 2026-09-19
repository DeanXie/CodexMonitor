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
    pub(crate) fn load_activated(data_dir: PathBuf) -> Result<Self, String> {
        let metadata = crate::shared::startup_activation::validate_activated_profile(&data_dir)?;
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
            #[cfg(desktop)]
            remote_host_identity,
            #[cfg(desktop)]
            global_rollout_runtime: crate::global_sources::runtime::GlobalRolloutRuntime::default(),
        })
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
