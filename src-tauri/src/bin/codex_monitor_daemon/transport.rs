use super::rpc::{
    build_error_response, build_result_response, forward_events, parse_auth_token,
    spawn_rpc_response_task,
};
use super::*;

pub(super) async fn handle_client(
    socket: TcpStream,
    config: Arc<DaemonConfig>,
    state: Arc<DaemonState>,
    events: broadcast::Sender<DaemonEvent>,
) {
    let (reader, mut writer) = socket.into_split();
    let mut lines = BufReader::new(reader).lines();

    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();
    let write_task = tokio::spawn(async move {
        while let Some(message) = out_rx.recv().await {
            if writer.write_all(message.as_bytes()).await.is_err() {
                break;
            }
            if writer.write_all(b"\n").await.is_err() {
                break;
            }
        }
    });

    let mut authenticated = config.token.is_none();
    let mut events_task: Option<tokio::task::JoinHandle<()>> = None;
    let request_limiter = Arc::new(Semaphore::new(MAX_IN_FLIGHT_RPC_PER_CONNECTION));
    let client_version = format!("daemon-{}", env!("CARGO_PKG_VERSION"));
    let mut request_provenance = authenticated
        .then(|| Arc::new(RemoteRequestProvenanceRuntime::new_authenticated_transport()));

    if authenticated {
        let rx = events.subscribe();
        let out_tx_events = out_tx.clone();
        let transport_generation = request_provenance
            .as_ref()
            .expect("authenticated transport provenance")
            .transport_generation()
            .clone();
        events_task = Some(tokio::spawn(forward_events(
            rx,
            out_tx_events,
            state.daemon_process_generation.clone(),
            transport_generation,
        )));
    }

    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let message: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(_) => continue,
        };

        let id = message.get("id").and_then(|value| value.as_u64());
        let method = message
            .get("method")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();
        let params = message.get("params").cloned().unwrap_or(Value::Null);

        if !authenticated {
            if method != "auth" {
                if let Some(response) = build_error_response(id, "unauthorized") {
                    let _ = out_tx.send(response);
                }
                continue;
            }

            let expected = config.token.clone().unwrap_or_default();
            let provided = parse_auth_token(&params).unwrap_or_default();
            if expected != provided {
                if let Some(response) = build_error_response(id, "invalid token") {
                    let _ = out_tx.send(response);
                }
                continue;
            }

            authenticated = true;
            request_provenance = Some(Arc::new(
                RemoteRequestProvenanceRuntime::new_authenticated_transport(),
            ));
            if let Some(response) = build_result_response(id, json!({ "ok": true })) {
                let _ = out_tx.send(response);
            }

            let rx = events.subscribe();
            let out_tx_events = out_tx.clone();
            let transport_generation = request_provenance
                .as_ref()
                .expect("authenticated transport provenance")
                .transport_generation()
                .clone();
            events_task = Some(tokio::spawn(forward_events(
                rx,
                out_tx_events,
                state.daemon_process_generation.clone(),
                transport_generation,
            )));

            continue;
        }

        let request_provenance = request_provenance
            .as_ref()
            .expect("authenticated transport provenance")
            .clone();
        let request_key = if let Some(id) = id {
            match request_provenance.record_received(
                id,
                method.clone(),
                chrono::Utc::now().timestamp_millis(),
            ) {
                Ok(key) => Some(key),
                Err(_) => {
                    if let Some(response) =
                        build_error_response(Some(id), "duplicate request id for current transport")
                    {
                        let _ = out_tx.send(response);
                    }
                    continue;
                }
            }
        } else {
            None
        };

        spawn_rpc_response_task(
            Arc::clone(&state),
            out_tx.clone(),
            id,
            method,
            params,
            client_version.clone(),
            Arc::clone(&request_limiter),
            Arc::clone(&request_provenance),
            request_key,
        );
    }

    if let Some(request_provenance) = request_provenance {
        request_provenance.record_transport_lost(chrono::Utc::now().timestamp_millis());
    }

    drop(out_tx);
    if let Some(task) = events_task {
        task.abort();
    }
    write_task.abort();
}
