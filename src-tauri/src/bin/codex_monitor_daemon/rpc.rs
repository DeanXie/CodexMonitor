use super::*;

#[path = "rpc/codex.rs"]
mod codex;
#[path = "rpc/daemon.rs"]
mod daemon;
#[path = "rpc/dispatcher.rs"]
mod dispatcher;
#[path = "rpc/git.rs"]
mod git;
#[path = "rpc/prompts.rs"]
mod prompts;
#[path = "rpc/workspace.rs"]
mod workspace;

pub(super) fn build_error_response(id: Option<u64>, message: &str) -> Option<String> {
    let id = id?;
    Some(
        serde_json::to_string(&json!({
            "id": id,
            "error": { "message": message }
        }))
        .unwrap_or_else(|_| {
            "{\"id\":0,\"error\":{\"message\":\"serialization failed\"}}".to_string()
        }),
    )
}

pub(super) fn build_result_response(id: Option<u64>, result: Value) -> Option<String> {
    let id = id?;
    Some(
        serde_json::to_string(&json!({ "id": id, "result": result })).unwrap_or_else(|_| {
            "{\"id\":0,\"error\":{\"message\":\"serialization failed\"}}".to_string()
        }),
    )
}

fn build_event_notification(
    event: DaemonEvent,
    daemon_process_generation: &DaemonProcessGeneration,
    remote_transport_generation: &RemoteTransportGeneration,
) -> Option<String> {
    let payload = match event {
        DaemonEvent::AppServer(payload) => json!({
            "method": "app-server-event",
            "params": payload.for_remote_delivery(
                daemon_process_generation.clone(),
                remote_transport_generation.clone(),
            ),
        }),
        DaemonEvent::TerminalOutput(payload) => json!({
            "method": "terminal-output",
            "params": payload,
        }),
        DaemonEvent::TerminalExit(payload) => json!({
            "method": "terminal-exit",
            "params": payload,
        }),
    };
    serde_json::to_string(&payload).ok()
}

pub(super) fn parse_auth_token(params: &Value) -> Option<String> {
    match params {
        Value::String(value) => Some(value.clone()),
        Value::Object(map) => map
            .get("token")
            .and_then(|value| value.as_str())
            .map(|v| v.to_string()),
        _ => None,
    }
}

pub(super) fn parse_string(value: &Value, key: &str) -> Result<String, String> {
    match value {
        Value::Object(map) => map
            .get(key)
            .and_then(|value| value.as_str())
            .map(|value| value.to_string())
            .ok_or_else(|| format!("missing or invalid `{key}`")),
        _ => Err(format!("missing `{key}`")),
    }
}

pub(super) fn parse_optional_string(value: &Value, key: &str) -> Option<String> {
    match value {
        Value::Object(map) => map
            .get(key)
            .and_then(|value| value.as_str())
            .map(|v| v.to_string()),
        _ => None,
    }
}

pub(super) fn parse_optional_nullable_string(value: &Value, key: &str) -> Option<Option<String>> {
    match value {
        Value::Object(map) => match map.get(key) {
            Some(Value::Null) => Some(None),
            Some(Value::String(value)) => Some(Some(value.to_string())),
            Some(_) => None,
            None => None,
        },
        _ => None,
    }
}

pub(super) fn parse_optional_u32(value: &Value, key: &str) -> Option<u32> {
    match value {
        Value::Object(map) => map.get(key).and_then(|value| value.as_u64()).and_then(|v| {
            if v > u32::MAX as u64 {
                None
            } else {
                Some(v as u32)
            }
        }),
        _ => None,
    }
}

pub(super) fn parse_optional_bool(value: &Value, key: &str) -> Option<bool> {
    match value {
        Value::Object(map) => map.get(key).and_then(|value| value.as_bool()),
        _ => None,
    }
}

pub(super) fn parse_optional_string_array(value: &Value, key: &str) -> Option<Vec<String>> {
    match value {
        Value::Object(map) => map
            .get(key)
            .and_then(|value| value.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(|value| value.to_string()))
                    .collect::<Vec<_>>()
            }),
        _ => None,
    }
}

pub(super) fn parse_string_array(value: &Value, key: &str) -> Result<Vec<String>, String> {
    parse_optional_string_array(value, key).ok_or_else(|| format!("missing `{key}`"))
}

pub(super) fn parse_optional_value(value: &Value, key: &str) -> Option<Value> {
    match value {
        Value::Object(map) => map.get(key).cloned(),
        _ => None,
    }
}

pub(super) async fn handle_rpc_request(
    state: &DaemonState,
    method: &str,
    params: Value,
    client_version: String,
) -> Result<Value, String> {
    handle_rpc_request_with_context(state, method, params, client_version, None).await
}

pub(super) async fn handle_rpc_request_with_context(
    state: &DaemonState,
    method: &str,
    params: Value,
    client_version: String,
    remote_context: Option<&RemoteRequestDispatchContext>,
) -> Result<Value, String> {
    dispatcher::dispatch_rpc_request(state, method, &params, &client_version, remote_context).await
}

pub(super) async fn forward_events(
    mut rx: broadcast::Receiver<DaemonEvent>,
    out_tx_events: mpsc::UnboundedSender<String>,
    daemon_process_generation: DaemonProcessGeneration,
    remote_transport_generation: RemoteTransportGeneration,
) {
    loop {
        let event = match rx.recv().await {
            Ok(event) => event,
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => break,
        };

        let Some(payload) = build_event_notification(
            event,
            &daemon_process_generation,
            &remote_transport_generation,
        ) else {
            continue;
        };

        if out_tx_events.send(payload).is_err() {
            break;
        }
    }
}

pub(super) fn spawn_rpc_response_task(
    state: Arc<DaemonState>,
    out_tx: mpsc::UnboundedSender<String>,
    id: Option<u64>,
    method: String,
    params: Value,
    client_version: String,
    request_limiter: Arc<Semaphore>,
    request_provenance: Arc<RemoteRequestProvenanceRuntime>,
    request_key: Option<RemoteRequestKey>,
) {
    tokio::spawn(async move {
        let Ok(_permit) = request_limiter.acquire_owned().await else {
            return;
        };
        let remote_context = if let Some(request_key) = request_key.as_ref() {
            if request_provenance
                .record_dispatch_started(request_key, chrono::Utc::now().timestamp_millis())
                .is_err()
            {
                return;
            }
            Some(RemoteRequestDispatchContext::new(
                Arc::clone(&request_provenance),
                request_key.clone(),
            ))
        } else {
            None
        };
        let result = handle_rpc_request_with_context(
            &state,
            &method,
            params,
            client_version,
            remote_context.as_ref(),
        )
        .await;
        if let Some(request_key) = request_key.as_ref() {
            let _ = request_provenance
                .record_response_observed(request_key, chrono::Utc::now().timestamp_millis());
        }
        let response = match result {
            Ok(result) => build_result_response(id, result),
            Err(message) => build_error_response(id, &message),
        };
        if let Some(response) = response {
            let _ = out_tx.send(response);
        }
    });
}

#[cfg(test)]
mod generation_event_delivery_tests {
    use super::*;
    use crate::shared::codex_core::thread_lifecycle_observation::AppServerConnectionGeneration;
    use crate::shared::codex_core::writer_admission_observation::WorkspaceSessionGeneration;

    #[test]
    fn daemon_delivery_binds_process_and_transport_generations() {
        let event = AppServerEvent::for_session(
            "workspace-a",
            json!({ "method": "thread/updated" }),
            WorkspaceSessionGeneration::new("workspace-generation-a").unwrap(),
            AppServerConnectionGeneration::new("connection-generation-a").unwrap(),
        );
        let daemon = DaemonProcessGeneration::new("daemon-generation-a").unwrap();
        let transport = RemoteTransportGeneration::new("transport-generation-a").unwrap();

        let wire = build_event_notification(DaemonEvent::AppServer(event), &daemon, &transport)
            .expect("event notification");
        let value: Value = serde_json::from_str(&wire).expect("valid json-rpc notification");

        assert_eq!(
            value.pointer("/params/daemonProcessGeneration"),
            Some(&json!("daemon-generation-a"))
        );
        assert_eq!(
            value.pointer("/params/remoteTransportGeneration"),
            Some(&json!("transport-generation-a"))
        );
        assert_eq!(
            value.pointer("/params/workspaceSessionGeneration"),
            Some(&json!("workspace-generation-a"))
        );
        assert_eq!(
            value.pointer("/params/appServerConnectionGeneration"),
            Some(&json!("connection-generation-a"))
        );
    }
}
