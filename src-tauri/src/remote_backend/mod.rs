mod protocol;
mod tcp_transport;
mod transport;

use serde_json::{json, Value};
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::AppHandle;
use tokio::sync::Mutex;
use tokio::time::timeout;

use crate::shared::remote_host_availability::{AvailabilityEvent, RemoteHostAvailabilitySnapshot};
use crate::shared::remote_host_identity::{
    validate_daemon_info, RemoteDaemonInfo, RemoteHostIdentity,
};
use crate::state::AppState;
use crate::storage::write_settings_atomic;
use crate::types::{BackendMode, RemoteBackendProvider, RemoteBackendTarget};

use self::protocol::{build_request_line, RemoteCallError, DEFAULT_REMOTE_HOST};
use self::tcp_transport::TcpTransport;
use self::transport::{
    PendingMap, RemoteTransport, RemoteTransportConfig, RemoteTransportKind,
    TransportAvailabilityObserver,
};

const REMOTE_REQUEST_TIMEOUT: Duration = Duration::from_secs(300);
const REMOTE_SEND_TIMEOUT: Duration = Duration::from_secs(15);
const REMOTE_ENDPOINT_FAILURE_WINDOW: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Eq, PartialEq)]
enum RemoteBackendInitializationErrorKind {
    EndpointUnreachable,
    Other,
}

#[derive(Clone, Debug)]
struct RemoteBackendInitializationError {
    message: String,
    kind: RemoteBackendInitializationErrorKind,
}

impl RemoteBackendInitializationError {
    fn endpoint_unreachable(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: RemoteBackendInitializationErrorKind::EndpointUnreachable,
        }
    }

    #[cfg(test)]
    fn message(&self) -> &str {
        &self.message
    }
}

impl From<String> for RemoteBackendInitializationError {
    fn from(message: String) -> Self {
        Self {
            message,
            kind: RemoteBackendInitializationErrorKind::Other,
        }
    }
}

#[derive(Clone, Debug)]
struct RemoteBackendNegativeResult {
    target_id: String,
    generation: u64,
    error: RemoteBackendInitializationError,
    retry_not_before: Instant,
}

#[derive(Default)]
pub(crate) struct RemoteBackendCache {
    state: Mutex<RemoteBackendCacheState>,
    initialization: Mutex<()>,
}

#[derive(Default)]
struct RemoteBackendCacheState {
    generation: u64,
    current: Option<RemoteBackend>,
    negative: Option<RemoteBackendNegativeResult>,
}

impl RemoteBackendCache {
    #[cfg(test)]
    async fn get_or_try_initialize<F, Fut>(&self, initialize: F) -> Result<RemoteBackend, String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<RemoteBackend, String>>,
    {
        self.get_or_try_initialize_for_target("", Duration::ZERO, || async {
            initialize()
                .await
                .map_err(RemoteBackendInitializationError::from)
        })
        .await
        .map_err(|error| error.message)
    }

    async fn get_or_try_initialize_for_target<F, Fut>(
        &self,
        target_id: &str,
        negative_window: Duration,
        initialize: F,
    ) -> Result<RemoteBackend, RemoteBackendInitializationError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<RemoteBackend, RemoteBackendInitializationError>>,
    {
        if let Some(result) = self.current_or_negative(target_id).await {
            return result;
        }

        let _initialization = self.initialization.lock().await;
        if let Some(result) = self.current_or_negative(target_id).await {
            return result;
        }

        let generation = self.state.lock().await.generation;
        match initialize().await {
            Ok(client) => {
                let mut state = self.state.lock().await;
                if state.generation != generation {
                    return Err(RemoteBackendInitializationError::from(
                        "remote backend initialization was superseded".to_string(),
                    ));
                }
                state.negative = None;
                state.current = Some(client.clone());
                Ok(client)
            }
            Err(error) => {
                if error.kind == RemoteBackendInitializationErrorKind::EndpointUnreachable {
                    let mut state = self.state.lock().await;
                    if state.generation == generation {
                        state.negative = Some(RemoteBackendNegativeResult {
                            target_id: target_id.to_string(),
                            generation,
                            error: error.clone(),
                            retry_not_before: Instant::now() + negative_window,
                        });
                    }
                }
                Err(error)
            }
        }
    }

    async fn current_or_negative(
        &self,
        target_id: &str,
    ) -> Option<Result<RemoteBackend, RemoteBackendInitializationError>> {
        let mut state = self.state.lock().await;
        if let Some(client) = state.current.clone() {
            return Some(Ok(client));
        }
        let reusable = state.negative.as_ref().is_some_and(|negative| {
            negative.target_id == target_id
                && negative.generation == state.generation
                && Instant::now() < negative.retry_not_before
        });
        if reusable {
            return state
                .negative
                .as_ref()
                .map(|negative| Err(negative.error.clone()));
        }
        state.negative = None;
        None
    }

    #[cfg(test)]
    async fn current(&self) -> Option<RemoteBackend> {
        self.state.lock().await.current.clone()
    }

    async fn clear_if_current(&self, client: &RemoteBackend) -> bool {
        let mut state = self.state.lock().await;
        if state
            .current
            .as_ref()
            .is_some_and(|candidate| candidate.same_connection(client))
        {
            state.current = None;
            state.negative = None;
            state.generation = state.generation.wrapping_add(1);
            return true;
        }
        false
    }

    pub(crate) async fn clear(&self) {
        let mut state = self.state.lock().await;
        state.current = None;
        state.negative = None;
        state.generation = state.generation.wrapping_add(1);
    }
}

pub(crate) fn normalize_path_for_remote(path: String) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return path;
    }

    if let Some(normalized) = normalize_wsl_unc_path(trimmed) {
        return normalized;
    }

    path
}

fn normalize_wsl_unc_path(path: &str) -> Option<String> {
    let lower = path.to_ascii_lowercase();
    let (prefix_len, raw) = if lower.starts_with("\\\\wsl$\\") {
        (7, path)
    } else if lower.starts_with("\\\\wsl.localhost\\") {
        (16, path)
    } else {
        return None;
    };

    let remainder = raw.get(prefix_len..)?;
    let mut segments = remainder.split('\\').filter(|segment| !segment.is_empty());
    segments.next()?;
    let joined = segments.collect::<Vec<_>>().join("/");
    Some(if joined.is_empty() {
        "/".to_string()
    } else {
        format!("/{joined}")
    })
}

#[derive(Clone)]
pub(crate) struct RemoteBackend {
    inner: Arc<RemoteBackendInner>,
}

#[cfg(test)]
impl std::fmt::Debug for RemoteBackend {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RemoteBackend")
            .finish_non_exhaustive()
    }
}

struct RemoteBackendInner {
    out_tx: tokio::sync::mpsc::Sender<String>,
    pending: Arc<Mutex<PendingMap>>,
    next_id: AtomicU64,
    connected: Arc<std::sync::atomic::AtomicBool>,
    ready: AtomicBool,
    availability: TransportAvailabilityObserver,
}

impl RemoteBackend {
    fn same_connection(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    pub(crate) async fn call(&self, method: &str, params: Value) -> Result<Value, RemoteCallError> {
        if !self.inner.ready.load(Ordering::SeqCst) {
            return Err(RemoteCallError::Protocol {
                message: "remote backend is not routable before authenticated host handshake"
                    .to_string(),
            });
        }
        let runtime_workspace = (method == "connect_workspace")
            .then(|| extract_workspace_id(&params))
            .flatten();
        if let Some(workspace_id) = runtime_workspace.clone() {
            self.observe(AvailabilityEvent::RuntimeStarted { workspace_id });
        }
        let result = self.call_inner(method, params).await;
        match &result {
            Err(RemoteCallError::Disconnected) => {
                self.observe(AvailabilityEvent::Disconnected {
                    diagnostic: RemoteCallError::Disconnected.to_string(),
                });
            }
            Err(
                error @ (RemoteCallError::DispatchTimeout { .. }
                | RemoteCallError::ResponseTimeout { .. }),
            ) => {
                self.observe(AvailabilityEvent::TransportUnknown {
                    diagnostic: error.to_string(),
                });
            }
            _ => {}
        }
        if let Some(workspace_id) = runtime_workspace {
            match &result {
                Ok(_) => self.observe(AvailabilityEvent::RuntimeReady { workspace_id }),
                Err(RemoteCallError::RpcRejected { message }) => {
                    self.observe(AvailabilityEvent::RuntimeUnavailable {
                        workspace_id,
                        diagnostic: message.clone(),
                    });
                }
                Err(error) => self.observe(AvailabilityEvent::RuntimeUnknown {
                    workspace_id,
                    diagnostic: error.to_string(),
                }),
            }
        }
        result
    }

    async fn call_during_handshake(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, RemoteCallError> {
        self.call_inner(method, params).await
    }

    async fn call_inner(&self, method: &str, params: Value) -> Result<Value, RemoteCallError> {
        if !self.inner.connected.load(Ordering::SeqCst) {
            return Err(RemoteCallError::Disconnected);
        }

        let id = self.inner.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.inner.pending.lock().await.insert(id, tx);

        let message = build_request_line(id, method, params)?;
        match timeout(REMOTE_SEND_TIMEOUT, self.inner.out_tx.send(message)).await {
            Ok(Ok(())) => {}
            Ok(Err(_)) => {
                self.inner.pending.lock().await.remove(&id);
                return Err(RemoteCallError::Disconnected);
            }
            Err(_) => {
                self.inner.pending.lock().await.remove(&id);
                return Err(RemoteCallError::DispatchTimeout {
                    seconds: REMOTE_SEND_TIMEOUT.as_secs(),
                });
            }
        }

        match timeout(REMOTE_REQUEST_TIMEOUT, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(RemoteCallError::Disconnected),
            Err(_) => {
                self.inner.pending.lock().await.remove(&id);
                Err(RemoteCallError::ResponseTimeout {
                    seconds: REMOTE_REQUEST_TIMEOUT.as_secs(),
                })
            }
        }
    }

    fn mark_ready(&self) {
        self.inner.ready.store(true, Ordering::SeqCst);
    }

    fn observe(&self, event: AvailabilityEvent) {
        self.inner.availability.runtime.observe(
            self.inner.availability.attempt.clone(),
            chrono::Utc::now().timestamp_millis(),
            event,
        );
    }
}

fn extract_workspace_id(params: &Value) -> Option<String> {
    params
        .get("workspaceId")
        .or_else(|| params.get("id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) async fn is_remote_mode(state: &AppState) -> bool {
    let settings = state.app_settings.lock().await;
    matches!(settings.backend_mode, BackendMode::Remote)
}

pub(crate) async fn call_remote(
    state: &AppState,
    app: AppHandle,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    let client = ensure_remote_backend(state, app.clone()).await?;
    match client.call(method, params.clone()).await {
        Ok(value) => Ok(value),
        Err(RemoteCallError::Disconnected) => {
            state.remote_backend.clear_if_current(&client).await;
            if !can_retry_after_disconnect(method) {
                return Err(RemoteCallError::Disconnected.to_string());
            }
            let retry_client = ensure_remote_backend(state, app).await?;
            match retry_client.call(method, params).await {
                Ok(value) => Ok(value),
                Err(retry_err) => {
                    state.remote_backend.clear_if_current(&retry_client).await;
                    Err(retry_err.to_string())
                }
            }
        }
        Err(err) => {
            state.remote_backend.clear_if_current(&client).await;
            Err(err.to_string())
        }
    }
}

pub(crate) fn invalidate_availability_for_settings(
    state: &AppState,
    previous: &crate::types::AppSettings,
    updated: &crate::types::AppSettings,
) {
    let observed_at = chrono::Utc::now().timestamp_millis();
    let previous_target_id = active_remote_target_id(previous);
    state
        .remote_host_availability
        .invalidate_for_settings_change(
            previous_target_id.clone(),
            active_remote_host_identity(previous).ok().flatten(),
            observed_at,
        );

    let updated_target_id = active_remote_target_id(updated);
    if updated_target_id != previous_target_id {
        state
            .remote_host_availability
            .invalidate_for_settings_change(
                updated_target_id,
                active_remote_host_identity(updated).ok().flatten(),
                observed_at,
            );
    }
}

fn can_retry_after_disconnect(method: &str) -> bool {
    matches!(
        method,
        "account_rate_limits"
            | "account_read"
            | "apps_list"
            | "collaboration_mode_list"
            | "connect_workspace"
            | "experimental_feature_list"
            | "set_workspace_runtime_codex_args"
            | "file_read"
            | "get_agents_settings"
            | "get_config_model"
            | "get_git_commit_diff"
            | "get_git_diffs"
            | "get_git_log"
            | "get_git_remote"
            | "get_git_status"
            | "get_github_issues"
            | "get_github_pull_request_comments"
            | "get_github_pull_request_diff"
            | "get_github_pull_requests"
            | "is_workspace_path_dir"
            | "list_git_branches"
            | "list_git_roots"
            | "list_mcp_server_status"
            | "list_threads"
            | "local_usage_snapshot"
            | "list_workspace_files"
            | "list_workspaces"
            | "model_list"
            | "read_thread"
            | "read_agent_config_toml"
            | "read_workspace_file"
            | "resume_thread"
            | "thread_live_subscribe"
            | "thread_live_unsubscribe"
            | "skills_list"
            | "worktree_setup_status"
    )
}

async fn ensure_remote_backend(state: &AppState, app: AppHandle) -> Result<RemoteBackend, String> {
    let target_id = {
        let settings = state.app_settings.lock().await;
        active_remote_target_id(&settings)
    };
    state
        .remote_backend
        .get_or_try_initialize_for_target(&target_id, REMOTE_ENDPOINT_FAILURE_WINDOW, || {
            initialize_remote_backend(state, app)
        })
        .await
        .map_err(|error| error.message)
}

async fn initialize_remote_backend(
    state: &AppState,
    app: AppHandle,
) -> Result<RemoteBackend, RemoteBackendInitializationError> {
    let (target_id, expected_identity, transport_config) = {
        let settings = state.app_settings.lock().await;
        let target_id = active_remote_target_id(&settings);
        let expected_identity = match active_remote_host_identity(&settings) {
            Ok(identity) => identity,
            Err(error) => {
                state.remote_host_availability.observe_unconfigured(
                    target_id,
                    chrono::Utc::now().timestamp_millis(),
                    error.clone(),
                );
                return Err(error.into());
            }
        };
        (
            target_id,
            expected_identity,
            resolve_transport_config(&settings)?,
        )
    };
    let transport_kind = transport_config.kind();
    let auth_token = match transport_config
        .auth_token()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
    {
        Some(token) => token,
        None => {
            let error =
                "remote backend requires token authentication before host identity handshake"
                    .to_string();
            state.remote_host_availability.observe_unconfigured(
                target_id,
                chrono::Utc::now().timestamp_millis(),
                error.clone(),
            );
            return Err(error.into());
        }
    };

    let attempt = state.remote_host_availability.begin_attempt(
        target_id,
        expected_identity,
        chrono::Utc::now().timestamp_millis(),
    );
    let availability = TransportAvailabilityObserver {
        runtime: Arc::clone(&state.remote_host_availability),
        attempt,
    };

    let transport: Box<dyn RemoteTransport> = match transport_config.kind() {
        RemoteTransportKind::Tcp => Box::new(TcpTransport),
    };
    let connection = match transport
        .connect(app, transport_config, availability.clone())
        .await
    {
        Ok(connection) => connection,
        Err(error) => {
            availability.runtime.observe(
                availability.attempt.clone(),
                chrono::Utc::now().timestamp_millis(),
                AvailabilityEvent::EndpointUnreachable {
                    diagnostic: error.message.clone(),
                },
            );
            return Err(RemoteBackendInitializationError::endpoint_unreachable(
                error.message,
            ));
        }
    };
    availability.runtime.observe(
        availability.attempt.clone(),
        chrono::Utc::now().timestamp_millis(),
        AvailabilityEvent::TransportConnected,
    );

    let client = RemoteBackend {
        inner: Arc::new(RemoteBackendInner {
            out_tx: connection.out_tx,
            pending: connection.pending,
            next_id: AtomicU64::new(1),
            connected: connection.connected,
            ready: AtomicBool::new(false),
            availability,
        }),
    };

    if matches!(transport_kind, RemoteTransportKind::Tcp) {
        client.observe(AvailabilityEvent::AuthStarted);
        match client
            .call_during_handshake("auth", json!({ "token": auth_token }))
            .await
        {
            Ok(_) => client.observe(AvailabilityEvent::AuthSucceeded),
            Err(RemoteCallError::RpcRejected { message }) => {
                client.observe(AvailabilityEvent::AuthRejected {
                    diagnostic: message.clone(),
                });
                return Err(message.into());
            }
            Err(error) => {
                client.observe(AvailabilityEvent::AuthUnknown {
                    diagnostic: error.to_string(),
                });
                return Err(error.to_string().into());
            }
        }
    }

    client.observe(AvailabilityEvent::DaemonValidationStarted);
    let daemon_info_value = match client.call_during_handshake("daemon_info", json!({})).await {
        Ok(value) => value,
        Err(error) => {
            client.observe(AvailabilityEvent::DaemonUnknown {
                diagnostic: error.to_string(),
            });
            return Err(error.to_string().into());
        }
    };
    let daemon_info: RemoteDaemonInfo = match serde_json::from_value(daemon_info_value) {
        Ok(info) => info,
        Err(error) => {
            let error = format!("invalid remote daemon identity response: {error}");
            client.observe(AvailabilityEvent::DaemonInvalidResponse {
                diagnostic: error.clone(),
            });
            return Err(error.into());
        }
    };
    if daemon_info.name != "codex-monitor-daemon" || daemon_info.mode != "tcp" {
        let error = validate_daemon_info(&daemon_info).unwrap_err();
        client.observe(AvailabilityEvent::ServiceMismatch {
            diagnostic: error.clone(),
        });
        return Err(error.into());
    }
    if let Err(error) = validate_daemon_info(&daemon_info) {
        client.observe(AvailabilityEvent::ProtocolUnsupported {
            diagnostic: error.clone(),
        });
        return Err(error.into());
    }
    if let Err(error) =
        persist_or_validate_remote_host_pin(state, &daemon_info.remote_host_identity, true).await
    {
        client.observe(AvailabilityEvent::IdentityMismatch {
            observed_identity: daemon_info.remote_host_identity.clone(),
            diagnostic: error.clone(),
        });
        return Err(error.into());
    }
    client.observe(AvailabilityEvent::DaemonAvailable {
        identity: daemon_info.remote_host_identity.clone(),
    });
    client.mark_ready();

    Ok(client)
}

#[tauri::command]
pub(crate) async fn get_remote_host_availability(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<RemoteHostAvailabilitySnapshot>, String> {
    Ok(state.remote_host_availability.snapshots())
}

async fn persist_or_validate_remote_host_pin(
    state: &AppState,
    remote_host_identity: &RemoteHostIdentity,
    authenticated: bool,
) -> Result<(), String> {
    let mut current = state.app_settings.lock().await;
    let mut next = current.clone();
    let changed = reconcile_remote_host_pin(&mut next, remote_host_identity, authenticated)?;
    if changed {
        write_settings_atomic(&state.settings_path, &next)?;
        *current = next;
    }
    Ok(())
}

fn reconcile_remote_host_pin(
    settings: &mut crate::types::AppSettings,
    remote_host_identity: &RemoteHostIdentity,
    authenticated: bool,
) -> Result<bool, String> {
    let active_index = settings
        .active_remote_backend_id
        .as_deref()
        .and_then(|id| {
            settings
                .remote_backends
                .iter()
                .position(|target| target.id == id)
        })
        .or_else(|| (!settings.remote_backends.is_empty()).then_some(0));

    if let Some(index) = active_index {
        if let Some(expected) = settings.remote_backends[index]
            .remote_host_identity
            .as_deref()
        {
            let expected = RemoteHostIdentity::parse(expected)
                .map_err(|error| format!("configured remote host identity is invalid: {error}"))?;
            if expected != *remote_host_identity {
                return Err(format!(
                    "remote host identity mismatch: expected {}, received {}",
                    expected.as_str(),
                    remote_host_identity.as_str()
                ));
            }
            return Ok(false);
        }
        if !authenticated {
            return Err(
                "cannot learn remote host identity from an unauthenticated connection".to_string(),
            );
        }
        settings.remote_backends[index].remote_host_identity =
            Some(remote_host_identity.as_str().to_string());
        return Ok(true);
    }

    if !authenticated {
        return Err(
            "cannot learn remote host identity from an unauthenticated connection".to_string(),
        );
    }
    let id = settings
        .active_remote_backend_id
        .clone()
        .unwrap_or_else(|| "remote-default".to_string());
    settings.remote_backends.push(RemoteBackendTarget {
        id: id.clone(),
        name: "Primary remote".to_string(),
        provider: RemoteBackendProvider::Tcp,
        host: settings.remote_backend_host.clone(),
        token: settings.remote_backend_token.clone(),
        remote_host_identity: Some(remote_host_identity.as_str().to_string()),
        last_connected_at_ms: None,
    });
    settings.active_remote_backend_id = Some(id);
    Ok(true)
}

fn resolve_transport_config(
    settings: &crate::types::AppSettings,
) -> Result<RemoteTransportConfig, String> {
    let host = if settings.remote_backend_host.trim().is_empty() {
        DEFAULT_REMOTE_HOST.to_string()
    } else {
        settings.remote_backend_host.clone()
    };
    Ok(RemoteTransportConfig::Tcp {
        host,
        auth_token: settings.remote_backend_token.clone(),
    })
}

fn active_remote_target_id(settings: &crate::types::AppSettings) -> String {
    settings
        .active_remote_backend_id
        .clone()
        .or_else(|| {
            settings
                .remote_backends
                .first()
                .map(|target| target.id.clone())
        })
        .unwrap_or_else(|| "remote-default".to_string())
}

fn active_remote_host_identity(
    settings: &crate::types::AppSettings,
) -> Result<Option<RemoteHostIdentity>, String> {
    let active_id = settings.active_remote_backend_id.as_deref();
    let target = active_id
        .and_then(|id| {
            settings
                .remote_backends
                .iter()
                .find(|target| target.id == id)
        })
        .or_else(|| settings.remote_backends.first());
    target
        .and_then(|target| target.remote_host_identity.as_deref())
        .map(RemoteHostIdentity::parse)
        .transpose()
        .map_err(|error| format!("configured remote host identity is invalid: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{
        can_retry_after_disconnect, reconcile_remote_host_pin, resolve_transport_config,
        RemoteBackend, RemoteBackendCache, RemoteBackendInitializationError, RemoteBackendInner,
    };
    use crate::remote_backend::transport::{RemoteTransportConfig, TransportAvailabilityObserver};
    use crate::shared::remote_host_availability::RemoteHostAvailabilityRuntime;
    use crate::shared::remote_host_identity::RemoteHostIdentity;
    use crate::types::AppSettings;
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::{Barrier, Mutex};

    fn test_backend_with_outbound(
        availability: Arc<RemoteHostAvailabilityRuntime>,
        target: &str,
        observed_at: i64,
    ) -> (RemoteBackend, tokio::sync::mpsc::Receiver<String>) {
        let (out_tx, out_rx) = tokio::sync::mpsc::channel(8);
        let attempt = availability.begin_attempt(target, None, observed_at);
        (
            RemoteBackend {
                inner: Arc::new(RemoteBackendInner {
                    out_tx,
                    pending: Arc::new(Mutex::new(Default::default())),
                    next_id: AtomicU64::new(1),
                    connected: Arc::new(AtomicBool::new(true)),
                    ready: AtomicBool::new(true),
                    availability: TransportAvailabilityObserver {
                        runtime: availability,
                        attempt,
                    },
                }),
            },
            out_rx,
        )
    }

    fn test_backend(
        availability: Arc<RemoteHostAvailabilityRuntime>,
        target: &str,
        observed_at: i64,
    ) -> RemoteBackend {
        test_backend_with_outbound(availability, target, observed_at).0
    }

    async fn complete_next_call(
        backend: &RemoteBackend,
        outbound: &mut tokio::sync::mpsc::Receiver<String>,
        expected_method: &str,
        response: serde_json::Value,
    ) -> u64 {
        let request: serde_json::Value =
            serde_json::from_str(&outbound.recv().await.expect("outbound request"))
                .expect("valid request JSON");
        assert_eq!(request["method"], expected_method);
        let id = request["id"].as_u64().expect("request id");
        backend
            .inner
            .pending
            .lock()
            .await
            .remove(&id)
            .expect("pending request")
            .send(Ok(response))
            .expect("call receiver");
        id
    }

    #[tokio::test]
    async fn concurrent_connect_and_list_share_single_current_backend() {
        let cache = Arc::new(RemoteBackendCache::default());
        let availability = Arc::new(RemoteHostAvailabilityRuntime::default());
        let initializer_calls = Arc::new(AtomicUsize::new(0));
        let initializer_started = Arc::new(Barrier::new(2));
        let release_initializer = Arc::new(Barrier::new(2));

        let first_cache = Arc::clone(&cache);
        let first_availability = Arc::clone(&availability);
        let first_calls = Arc::clone(&initializer_calls);
        let first_started = Arc::clone(&initializer_started);
        let first_release = Arc::clone(&release_initializer);
        let first = tokio::spawn(async move {
            first_cache
                .get_or_try_initialize(|| async move {
                    first_calls.fetch_add(1, Ordering::SeqCst);
                    first_started.wait().await;
                    first_release.wait().await;
                    Ok(test_backend(first_availability, "remote-a", 10))
                })
                .await
        });

        initializer_started.wait().await;
        let second_cache = Arc::clone(&cache);
        let second_availability = Arc::clone(&availability);
        let second_calls = Arc::clone(&initializer_calls);
        let second = tokio::spawn(async move {
            second_cache
                .get_or_try_initialize(|| async move {
                    second_calls.fetch_add(1, Ordering::SeqCst);
                    Ok(test_backend(second_availability, "remote-a", 20))
                })
                .await
        });
        release_initializer.wait().await;

        let first = first.await.unwrap().unwrap();
        let second = second.await.unwrap().unwrap();
        assert_eq!(initializer_calls.load(Ordering::SeqCst), 1);
        assert!(first.same_connection(&second));
        assert!(cache
            .current()
            .await
            .is_some_and(|current| current.same_connection(&first)));
    }

    #[tokio::test]
    async fn endpoint_failure_is_coalesced_for_waiting_callers() {
        let cache = Arc::new(RemoteBackendCache::default());
        let initializer_calls = Arc::new(AtomicUsize::new(0));
        let initializer_started = Arc::new(Barrier::new(2));
        let release_initializer = Arc::new(Barrier::new(2));

        let first_cache = Arc::clone(&cache);
        let first_calls = Arc::clone(&initializer_calls);
        let first_started = Arc::clone(&initializer_started);
        let first_release = Arc::clone(&release_initializer);
        let first = tokio::spawn(async move {
            first_cache
                .get_or_try_initialize_for_target(
                    "remote-a",
                    Duration::from_secs(5),
                    || async move {
                        first_calls.fetch_add(1, Ordering::SeqCst);
                        first_started.wait().await;
                        first_release.wait().await;
                        Err(RemoteBackendInitializationError::endpoint_unreachable(
                            "endpoint unavailable",
                        ))
                    },
                )
                .await
        });

        initializer_started.wait().await;
        let second_cache = Arc::clone(&cache);
        let second_calls = Arc::clone(&initializer_calls);
        let second = tokio::spawn(async move {
            second_cache
                .get_or_try_initialize_for_target(
                    "remote-a",
                    Duration::from_secs(5),
                    || async move {
                        second_calls.fetch_add(1, Ordering::SeqCst);
                        Err(RemoteBackendInitializationError::endpoint_unreachable(
                            "second attempt must not run",
                        ))
                    },
                )
                .await
        });

        release_initializer.wait().await;
        assert_eq!(
            first.await.unwrap().unwrap_err().message(),
            "endpoint unavailable"
        );
        assert_eq!(
            second.await.unwrap().unwrap_err().message(),
            "endpoint unavailable"
        );
        assert_eq!(initializer_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn coalesced_callers_do_not_increment_attempt_id() {
        let cache = RemoteBackendCache::default();
        let availability = Arc::new(RemoteHostAvailabilityRuntime::default());
        let first_availability = Arc::clone(&availability);
        cache
            .get_or_try_initialize_for_target(
                "remote-a",
                Duration::from_secs(5),
                || async move {
                    let attempt = first_availability.begin_attempt("remote-a", None, 10);
                    first_availability.observe(
                        attempt,
                        11,
                        crate::shared::remote_host_availability::AvailabilityEvent::EndpointUnreachable {
                            diagnostic: "endpoint unavailable".to_string(),
                        },
                    );
                    Err(RemoteBackendInitializationError::endpoint_unreachable(
                        "endpoint unavailable",
                    ))
                },
            )
            .await
            .unwrap_err();
        let attempt_id = availability.snapshot("remote-a").unwrap().attempt_id;

        cache
            .get_or_try_initialize_for_target("remote-a", Duration::from_secs(5), || async {
                panic!("coalesced caller must not allocate a new attempt");
            })
            .await
            .unwrap_err();

        assert_eq!(
            availability.snapshot("remote-a").unwrap().attempt_id,
            attempt_id
        );
    }

    #[tokio::test]
    async fn endpoint_failure_remains_observable_during_negative_window() {
        use crate::shared::remote_host_availability::TransportState;

        let cache = RemoteBackendCache::default();
        let availability = Arc::new(RemoteHostAvailabilityRuntime::default());
        let observed = Arc::clone(&availability);
        cache
            .get_or_try_initialize_for_target(
                "remote-a",
                Duration::from_secs(5),
                || async move {
                    let attempt = observed.begin_attempt("remote-a", None, 10);
                    observed.observe(
                        attempt,
                        11,
                        crate::shared::remote_host_availability::AvailabilityEvent::EndpointUnreachable {
                            diagnostic: "endpoint unavailable".to_string(),
                        },
                    );
                    Err(RemoteBackendInitializationError::endpoint_unreachable(
                        "endpoint unavailable",
                    ))
                },
            )
            .await
            .unwrap_err();

        cache
            .get_or_try_initialize_for_target("remote-a", Duration::from_secs(5), || async {
                panic!("negative result must be reused")
            })
            .await
            .unwrap_err();

        assert_eq!(
            availability.snapshot("remote-a").unwrap().transport,
            TransportState::EndpointUnreachable
        );
    }

    #[tokio::test]
    async fn only_one_retry_attempt_is_admitted_after_window_expires() {
        let cache = RemoteBackendCache::default();
        let calls = AtomicUsize::new(0);
        cache
            .get_or_try_initialize_for_target("remote-a", Duration::from_millis(10), || async {
                calls.fetch_add(1, Ordering::SeqCst);
                Err(RemoteBackendInitializationError::endpoint_unreachable(
                    "first",
                ))
            })
            .await
            .unwrap_err();
        tokio::time::sleep(Duration::from_millis(20)).await;
        cache
            .get_or_try_initialize_for_target("remote-a", Duration::from_secs(5), || async {
                calls.fetch_add(1, Ordering::SeqCst);
                Err(RemoteBackendInitializationError::endpoint_unreachable(
                    "retry",
                ))
            })
            .await
            .unwrap_err();
        cache
            .get_or_try_initialize_for_target("remote-a", Duration::from_secs(5), || async {
                calls.fetch_add(1, Ordering::SeqCst);
                Err(RemoteBackendInitializationError::endpoint_unreachable(
                    "extra",
                ))
            })
            .await
            .unwrap_err();

        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn repeated_failure_reestablishes_negative_window() {
        let cache = RemoteBackendCache::default();
        cache
            .get_or_try_initialize_for_target("remote-a", Duration::from_millis(10), || async {
                Err(RemoteBackendInitializationError::endpoint_unreachable(
                    "first",
                ))
            })
            .await
            .unwrap_err();
        tokio::time::sleep(Duration::from_millis(20)).await;
        cache
            .get_or_try_initialize_for_target("remote-a", Duration::from_secs(5), || async {
                Err(RemoteBackendInitializationError::endpoint_unreachable(
                    "second",
                ))
            })
            .await
            .unwrap_err();

        let error = cache
            .get_or_try_initialize_for_target("remote-a", Duration::from_secs(5), || async {
                panic!("second failure window must be active")
            })
            .await
            .unwrap_err();
        assert_eq!(error.message(), "second");
    }

    async fn assert_clear_invalidates_negative_result() {
        let cache = RemoteBackendCache::default();
        let calls = AtomicUsize::new(0);
        cache
            .get_or_try_initialize_for_target("remote-a", Duration::from_secs(5), || async {
                calls.fetch_add(1, Ordering::SeqCst);
                Err(RemoteBackendInitializationError::endpoint_unreachable(
                    "old config",
                ))
            })
            .await
            .unwrap_err();
        cache.clear().await;
        cache
            .get_or_try_initialize_for_target("remote-a", Duration::from_secs(5), || async {
                calls.fetch_add(1, Ordering::SeqCst);
                Err(RemoteBackendInitializationError::endpoint_unreachable(
                    "new config",
                ))
            })
            .await
            .unwrap_err();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn endpoint_change_invalidates_negative_result() {
        assert_clear_invalidates_negative_result().await;
    }

    #[tokio::test]
    async fn token_change_invalidates_negative_result_without_clearing_host_pin() {
        assert_clear_invalidates_negative_result().await;
    }

    #[tokio::test]
    async fn active_target_change_invalidates_negative_result() {
        let cache = RemoteBackendCache::default();
        let calls = AtomicUsize::new(0);
        cache
            .get_or_try_initialize_for_target("remote-a", Duration::from_secs(5), || async {
                calls.fetch_add(1, Ordering::SeqCst);
                Err(RemoteBackendInitializationError::endpoint_unreachable(
                    "remote-a unavailable",
                ))
            })
            .await
            .unwrap_err();
        cache
            .get_or_try_initialize_for_target("remote-b", Duration::from_secs(5), || async {
                calls.fetch_add(1, Ordering::SeqCst);
                Err(RemoteBackendInitializationError::endpoint_unreachable(
                    "remote-b unavailable",
                ))
            })
            .await
            .unwrap_err();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn non_endpoint_initialization_failures_are_not_coalesced() {
        let cache = RemoteBackendCache::default();
        let calls = AtomicUsize::new(0);
        for message in ["authentication failed", "identity mismatch"] {
            let error = cache
                .get_or_try_initialize_for_target("remote-a", Duration::from_secs(5), || async {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Err(RemoteBackendInitializationError::from(message.to_string()))
                })
                .await
                .unwrap_err();
            assert_eq!(error.message(), message);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn successful_reconnect_clears_negative_result() {
        let cache = RemoteBackendCache::default();
        let availability = Arc::new(RemoteHostAvailabilityRuntime::default());
        cache
            .get_or_try_initialize_for_target("remote-a", Duration::from_millis(10), || async {
                Err(RemoteBackendInitializationError::endpoint_unreachable(
                    "first",
                ))
            })
            .await
            .unwrap_err();
        tokio::time::sleep(Duration::from_millis(20)).await;
        let connected = cache
            .get_or_try_initialize_for_target("remote-a", Duration::from_secs(5), || async {
                Ok(test_backend(availability, "remote-a", 20))
            })
            .await
            .unwrap();
        let reused = cache
            .get_or_try_initialize_for_target("remote-a", Duration::from_secs(5), || async {
                panic!("successful backend must be reused")
            })
            .await
            .unwrap();
        assert!(connected.same_connection(&reused));
    }

    #[tokio::test]
    async fn queued_git_workspace_thread_reads_share_same_failed_initialization() {
        let cache = Arc::new(RemoteBackendCache::default());
        let calls = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();
        for _method in ["get_git_status", "list_workspaces", "read_thread"] {
            let cache = Arc::clone(&cache);
            let calls = Arc::clone(&calls);
            tasks.push(tokio::spawn(async move {
                cache
                    .get_or_try_initialize_for_target(
                        "remote-a",
                        Duration::from_secs(5),
                        || async move {
                            calls.fetch_add(1, Ordering::SeqCst);
                            tokio::time::sleep(Duration::from_millis(10)).await;
                            Err(RemoteBackendInitializationError::endpoint_unreachable(
                                "endpoint unavailable",
                            ))
                        },
                    )
                    .await
            }));
        }
        for task in tasks {
            assert_eq!(
                task.await.unwrap().unwrap_err().message(),
                "endpoint unavailable"
            );
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn stale_client_disconnect_cannot_clear_current_backend() {
        let cache = RemoteBackendCache::default();
        let availability = Arc::new(RemoteHostAvailabilityRuntime::default());
        let stale = cache
            .get_or_try_initialize(|| async {
                Ok(test_backend(Arc::clone(&availability), "remote-a", 10))
            })
            .await
            .unwrap();
        assert!(cache.clear_if_current(&stale).await);
        let current = cache
            .get_or_try_initialize(|| async {
                Ok(test_backend(Arc::clone(&availability), "remote-a", 20))
            })
            .await
            .unwrap();

        assert!(!cache.clear_if_current(&stale).await);
        assert!(cache
            .current()
            .await
            .is_some_and(|cached| cached.same_connection(&current)));
    }

    #[tokio::test]
    async fn older_handshake_cannot_publish_after_generation_invalidation() {
        let cache = Arc::new(RemoteBackendCache::default());
        let availability = Arc::new(RemoteHostAvailabilityRuntime::default());
        let initializer_started = Arc::new(Barrier::new(2));
        let release_initializer = Arc::new(Barrier::new(2));

        let initializing_cache = Arc::clone(&cache);
        let initializing_availability = Arc::clone(&availability);
        let started = Arc::clone(&initializer_started);
        let release = Arc::clone(&release_initializer);
        let initializing = tokio::spawn(async move {
            initializing_cache
                .get_or_try_initialize(|| async move {
                    started.wait().await;
                    release.wait().await;
                    Ok(test_backend(initializing_availability, "remote-a", 10))
                })
                .await
        });

        initializer_started.wait().await;
        cache.clear().await;
        release_initializer.wait().await;

        assert!(initializing.await.unwrap().is_err());
        assert!(cache.current().await.is_none());
    }

    #[tokio::test]
    async fn current_client_disconnect_does_invalidate_current_backend() {
        let cache = RemoteBackendCache::default();
        let availability = Arc::new(RemoteHostAvailabilityRuntime::default());
        let current = cache
            .get_or_try_initialize(|| async { Ok(test_backend(availability, "remote-a", 10)) })
            .await
            .unwrap();

        assert!(cache.clear_if_current(&current).await);
        assert!(cache.current().await.is_none());
    }

    #[test]
    fn discarded_stale_connection_eof_does_not_disconnect_current_attempt() {
        use crate::shared::remote_host_availability::{AvailabilityEvent, TransportState};

        let availability = RemoteHostAvailabilityRuntime::default();
        let stale = availability.begin_attempt("remote-a", None, 10);
        let current = availability.begin_attempt("remote-a", None, 20);
        availability.observe(current.clone(), 21, AvailabilityEvent::TransportConnected);
        availability.observe(
            stale,
            22,
            AvailabilityEvent::Disconnected {
                diagnostic: "transport read ended".to_string(),
            },
        );

        let snapshot = availability.snapshot("remote-a").unwrap();
        assert_eq!(snapshot.attempt_id, current.attempt_id);
        assert_eq!(snapshot.transport, TransportState::Connected);
        assert!(snapshot.diagnostics.is_empty());
    }

    #[tokio::test]
    async fn connect_workspace_success_records_runtime_ready_on_current_attempt() {
        use crate::shared::remote_host_availability::RuntimeState;

        let availability = Arc::new(RemoteHostAvailabilityRuntime::default());
        let (backend, mut outbound) =
            test_backend_with_outbound(Arc::clone(&availability), "remote-a", 10);
        let caller = backend.clone();
        let call = tokio::spawn(async move {
            caller
                .call(
                    "connect_workspace",
                    serde_json::json!({ "id": "workspace-a" }),
                )
                .await
        });

        complete_next_call(
            &backend,
            &mut outbound,
            "connect_workspace",
            serde_json::json!({ "ok": true }),
        )
        .await;
        call.await.unwrap().unwrap();

        let snapshot = availability.snapshot("remote-a").unwrap();
        assert_eq!(snapshot.runtime.state, RuntimeState::Ready);
        assert_eq!(
            snapshot.runtime.workspace_id.as_deref(),
            Some("workspace-a")
        );
        assert!(snapshot.last_runtime_ready_at.is_some());
    }

    #[tokio::test]
    async fn reconnect_normal_rpc_reuses_same_authenticated_long_connection() {
        let availability = Arc::new(RemoteHostAvailabilityRuntime::default());
        let (backend, mut outbound) = test_backend_with_outbound(availability, "remote-a", 10);

        let first_caller = backend.clone();
        let first = tokio::spawn(async move {
            first_caller
                .call("list_workspaces", serde_json::json!({}))
                .await
        });
        let first_id = complete_next_call(
            &backend,
            &mut outbound,
            "list_workspaces",
            serde_json::json!([]),
        )
        .await;
        first.await.unwrap().unwrap();

        let second_caller = backend.clone();
        let second = tokio::spawn(async move {
            second_caller
                .call("list_workspaces", serde_json::json!({}))
                .await
        });
        let second_id = complete_next_call(
            &backend,
            &mut outbound,
            "list_workspaces",
            serde_json::json!([]),
        )
        .await;
        second.await.unwrap().unwrap();

        assert_eq!(first_id, 1);
        assert_eq!(second_id, 2);
        assert!(backend.inner.connected.load(Ordering::SeqCst));
    }

    #[test]
    fn resolve_tcp_transport_uses_remote_host() {
        let mut settings = AppSettings::default();
        settings.remote_backend_host = "tcp.example:4732".to_string();

        let config = resolve_transport_config(&settings).expect("transport config");
        let RemoteTransportConfig::Tcp { host, .. } = config else {
            panic!("expected tcp transport config");
        };
        assert_eq!(host, "tcp.example:4732");
    }

    #[test]
    fn retries_only_retry_safe_methods_after_disconnect() {
        assert!(can_retry_after_disconnect("resume_thread"));
        assert!(can_retry_after_disconnect("list_threads"));
        assert!(can_retry_after_disconnect("local_usage_snapshot"));
        assert!(!can_retry_after_disconnect("send_user_message"));
        assert!(!can_retry_after_disconnect("start_thread"));
        assert!(!can_retry_after_disconnect("remove_workspace"));
    }

    fn host(value: &str) -> RemoteHostIdentity {
        RemoteHostIdentity::parse(value).unwrap()
    }

    #[test]
    fn legacy_unpinned_target_tofu_pins_once() {
        let mut settings = AppSettings::default();
        assert!(reconcile_remote_host_pin(
            &mut settings,
            &host("6ba7b810-9dad-41d1-80b4-00c04fd430c8"),
            true,
        )
        .unwrap());
        assert_eq!(
            settings.remote_backends[0].remote_host_identity.as_deref(),
            Some("6ba7b810-9dad-41d1-80b4-00c04fd430c8")
        );
        assert!(!reconcile_remote_host_pin(
            &mut settings,
            &host("6ba7b810-9dad-41d1-80b4-00c04fd430c8"),
            true,
        )
        .unwrap());
    }

    #[test]
    fn pinned_identity_mismatch_fails_closed_and_never_overwrites_pin() {
        let mut settings = AppSettings::default();
        reconcile_remote_host_pin(
            &mut settings,
            &host("6ba7b810-9dad-41d1-80b4-00c04fd430c8"),
            true,
        )
        .unwrap();
        let result = reconcile_remote_host_pin(
            &mut settings,
            &host("6ba7b811-9dad-41d1-80b4-00c04fd430c8"),
            true,
        );
        assert!(result.is_err());
        assert_eq!(
            settings.remote_backends[0].remote_host_identity.as_deref(),
            Some("6ba7b810-9dad-41d1-80b4-00c04fd430c8")
        );
    }

    #[test]
    fn pinned_identity_match_succeeds() {
        let mut settings = AppSettings::default();
        let identity = host("6ba7b810-9dad-41d1-80b4-00c04fd430c8");
        reconcile_remote_host_pin(&mut settings, &identity, true).unwrap();
        assert!(!reconcile_remote_host_pin(&mut settings, &identity, true).unwrap());
    }

    #[test]
    fn endpoint_change_with_same_identity_succeeds() {
        let mut settings = AppSettings::default();
        reconcile_remote_host_pin(
            &mut settings,
            &host("6ba7b810-9dad-41d1-80b4-00c04fd430c8"),
            true,
        )
        .unwrap();
        settings.remote_backend_host = "new-endpoint.example:4732".to_string();
        settings.remote_backends[0].host = settings.remote_backend_host.clone();
        assert!(!reconcile_remote_host_pin(
            &mut settings,
            &host("6ba7b810-9dad-41d1-80b4-00c04fd430c8"),
            true,
        )
        .unwrap());
    }

    #[test]
    fn display_name_change_does_not_change_identity() {
        let mut settings = AppSettings::default();
        let identity = host("baf44dc3-9d14-43fd-a2a8-2f3fc58c8d37");
        reconcile_remote_host_pin(&mut settings, &identity, true).unwrap();
        settings.remote_backends[0].name = "Office Desktop".to_string();
        assert!(!reconcile_remote_host_pin(&mut settings, &identity, true).unwrap());
    }

    #[test]
    fn unpinned_identity_requires_authenticated_tofu() {
        let mut settings = AppSettings::default();
        assert!(reconcile_remote_host_pin(
            &mut settings,
            &host("6ba7b810-9dad-41d1-80b4-00c04fd430c8"),
            false,
        )
        .is_err());
        assert!(settings.remote_backends.is_empty());
    }

    #[tokio::test]
    async fn remote_backend_is_not_routable_before_handshake() {
        let (out_tx, _out_rx) = tokio::sync::mpsc::channel(1);
        let availability = Arc::new(RemoteHostAvailabilityRuntime::default());
        let attempt = availability.begin_attempt("test", None, 1);
        let client = RemoteBackend {
            inner: Arc::new(RemoteBackendInner {
                out_tx,
                pending: Arc::new(Mutex::new(Default::default())),
                next_id: AtomicU64::new(1),
                connected: Arc::new(AtomicBool::new(true)),
                ready: AtomicBool::new(false),
                availability: TransportAvailabilityObserver {
                    runtime: availability,
                    attempt,
                },
            }),
        };
        let error = client
            .call("list_workspaces", serde_json::json!({}))
            .await
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("not routable before authenticated host handshake"));
    }
}
