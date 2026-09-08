mod protocol;
mod tcp_transport;
mod transport;

use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

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

struct RemoteBackendInner {
    out_tx: tokio::sync::mpsc::Sender<String>,
    pending: Arc<Mutex<PendingMap>>,
    next_id: AtomicU64,
    connected: Arc<std::sync::atomic::AtomicBool>,
    ready: AtomicBool,
    availability: TransportAvailabilityObserver,
}

impl RemoteBackend {
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
            *state.remote_backend.lock().await = None;
            if !can_retry_after_disconnect(method) {
                return Err(RemoteCallError::Disconnected.to_string());
            }
            let retry_client = ensure_remote_backend(state, app).await?;
            match retry_client.call(method, params).await {
                Ok(value) => Ok(value),
                Err(retry_err) => {
                    *state.remote_backend.lock().await = None;
                    Err(retry_err.to_string())
                }
            }
        }
        Err(err) => {
            *state.remote_backend.lock().await = None;
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
    {
        let guard = state.remote_backend.lock().await;
        if let Some(client) = guard.as_ref() {
            return Ok(client.clone());
        }
    }

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
                return Err(error);
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
            return Err(error);
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
            return Err(error.message);
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
                return Err(message);
            }
            Err(error) => {
                client.observe(AvailabilityEvent::AuthUnknown {
                    diagnostic: error.to_string(),
                });
                return Err(error.to_string());
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
            return Err(error.to_string());
        }
    };
    let daemon_info: RemoteDaemonInfo = match serde_json::from_value(daemon_info_value) {
        Ok(info) => info,
        Err(error) => {
            let error = format!("invalid remote daemon identity response: {error}");
            client.observe(AvailabilityEvent::DaemonInvalidResponse {
                diagnostic: error.clone(),
            });
            return Err(error);
        }
    };
    if daemon_info.name != "codex-monitor-daemon" || daemon_info.mode != "tcp" {
        let error = validate_daemon_info(&daemon_info).unwrap_err();
        client.observe(AvailabilityEvent::ServiceMismatch {
            diagnostic: error.clone(),
        });
        return Err(error);
    }
    if let Err(error) = validate_daemon_info(&daemon_info) {
        client.observe(AvailabilityEvent::ProtocolUnsupported {
            diagnostic: error.clone(),
        });
        return Err(error);
    }
    if let Err(error) =
        persist_or_validate_remote_host_pin(state, &daemon_info.remote_host_identity, true).await
    {
        client.observe(AvailabilityEvent::IdentityMismatch {
            observed_identity: daemon_info.remote_host_identity.clone(),
            diagnostic: error.clone(),
        });
        return Err(error);
    }
    client.observe(AvailabilityEvent::DaemonAvailable {
        identity: daemon_info.remote_host_identity.clone(),
    });
    client.mark_ready();

    {
        let mut guard = state.remote_backend.lock().await;
        *guard = Some(client.clone());
    }

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
        RemoteBackend, RemoteBackendInner,
    };
    use crate::remote_backend::transport::{RemoteTransportConfig, TransportAvailabilityObserver};
    use crate::shared::remote_host_availability::RemoteHostAvailabilityRuntime;
    use crate::shared::remote_host_identity::RemoteHostIdentity;
    use crate::types::AppSettings;
    use std::sync::atomic::{AtomicBool, AtomicU64};
    use std::sync::Arc;
    use tokio::sync::Mutex;

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
