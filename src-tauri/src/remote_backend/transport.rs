use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot, Mutex};

use super::protocol::{parse_incoming_line, IncomingMessage, RemoteCallError};
use crate::shared::remote_host_availability::{
    AvailabilityAttempt, AvailabilityEvent, RemoteHostAvailabilityRuntime,
};
use crate::shared::remote_request_provenance::RemoteTransportGeneration;

pub(crate) type PendingMap = HashMap<u64, oneshot::Sender<Result<Value, RemoteCallError>>>;
const OUTBOUND_QUEUE_CAPACITY: usize = 512;

#[derive(Clone, Default)]
pub(crate) struct RemoteNotificationDeliveryAuthority {
    current_generation: Arc<StdMutex<Option<RemoteTransportGeneration>>>,
}

impl RemoteNotificationDeliveryAuthority {
    pub(crate) fn new_connection_gate(&self) -> RemoteNotificationDeliveryGate {
        RemoteNotificationDeliveryGate {
            authority: self.clone(),
            authenticated_generation: Arc::new(StdMutex::new(None)),
        }
    }

    pub(crate) fn publish_current(&self, generation: RemoteTransportGeneration) {
        *self
            .current_generation
            .lock()
            .expect("remote notification delivery authority lock") = Some(generation);
    }

    pub(crate) fn clear_current(&self) {
        *self
            .current_generation
            .lock()
            .expect("remote notification delivery authority lock") = None;
    }

    pub(crate) fn clear_if_current(&self, generation: &RemoteTransportGeneration) {
        let mut current = self
            .current_generation
            .lock()
            .expect("remote notification delivery authority lock");
        if current.as_ref() == Some(generation) {
            *current = None;
        }
    }
}

#[derive(Clone)]
pub(crate) struct RemoteNotificationDeliveryGate {
    authority: RemoteNotificationDeliveryAuthority,
    authenticated_generation: Arc<StdMutex<Option<RemoteTransportGeneration>>>,
}

impl RemoteNotificationDeliveryGate {
    pub(crate) fn bind_authenticated_generation(
        &self,
        generation: RemoteTransportGeneration,
    ) -> Result<(), String> {
        let mut authenticated = self
            .authenticated_generation
            .lock()
            .expect("remote notification generation lock");
        match authenticated.as_ref() {
            None => {
                *authenticated = Some(generation);
                Ok(())
            }
            Some(existing) if existing == &generation => Ok(()),
            Some(_) => Err(
                "remote notification delivery gate is already bound to another generation"
                    .to_string(),
            ),
        }
    }

    pub(crate) fn authenticated_generation(&self) -> Option<RemoteTransportGeneration> {
        self.authenticated_generation
            .lock()
            .expect("remote notification generation lock")
            .clone()
    }
}

pub(crate) fn dispatch_notification_if_current<F>(
    delivery: &RemoteNotificationDeliveryGate,
    method: &str,
    params: Value,
    mut emit: F,
) where
    F: FnMut(&str, Value),
{
    let Some(generation) = delivery.authenticated_generation() else {
        return;
    };
    let current = delivery
        .authority
        .current_generation
        .lock()
        .expect("remote notification delivery authority lock");
    if current.as_ref() != Some(&generation) {
        return;
    }

    match method {
        "app-server-event" => {
            let transport_matches = params
                .get("remoteTransportGeneration")
                .and_then(Value::as_str)
                == Some(generation.as_str());
            let has_required_session_generations = params
                .get("workspaceSessionGeneration")
                .and_then(Value::as_str)
                .is_some_and(|value| !value.is_empty())
                && params
                    .get("appServerConnectionGeneration")
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.is_empty());
            let has_daemon_generation = params
                .get("daemonProcessGeneration")
                .and_then(Value::as_str)
                .is_some_and(|value| !value.is_empty());
            if transport_matches && has_required_session_generations && has_daemon_generation {
                emit(method, params);
            }
        }
        "app-server-event-gap" => {
            let transport_matches = params
                .get("remoteTransportGeneration")
                .and_then(Value::as_str)
                == Some(generation.as_str());
            let has_daemon_generation = params
                .get("daemonProcessGeneration")
                .and_then(Value::as_str)
                .is_some_and(|value| !value.is_empty());
            let has_gap = params
                .get("skipped")
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0);
            let has_coverages = params
                .get("affectedCoverages")
                .and_then(Value::as_array)
                .is_some_and(|values| !values.is_empty());
            if transport_matches && has_daemon_generation && has_gap && has_coverages {
                emit(method, params);
            }
        }
        "terminal-output" | "terminal-exit" => emit(method, params),
        _ => {}
    }
}

#[derive(Clone, Debug)]
pub(crate) enum RemoteTransportConfig {
    Tcp {
        host: String,
        auth_token: Option<String>,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum RemoteTransportKind {
    Tcp,
}

impl RemoteTransportConfig {
    pub(crate) fn kind(&self) -> RemoteTransportKind {
        match self {
            RemoteTransportConfig::Tcp { .. } => RemoteTransportKind::Tcp,
        }
    }

    pub(crate) fn auth_token(&self) -> Option<&str> {
        match self {
            RemoteTransportConfig::Tcp { auth_token, .. } => auth_token.as_deref(),
        }
    }
}

pub(crate) struct TransportConnection {
    pub(crate) out_tx: mpsc::Sender<String>,
    pub(crate) pending: Arc<Mutex<PendingMap>>,
    pub(crate) connected: Arc<AtomicBool>,
}

#[derive(Clone)]
pub(crate) struct TransportAvailabilityObserver {
    pub(crate) runtime: Arc<RemoteHostAvailabilityRuntime>,
    pub(crate) attempt: AvailabilityAttempt,
}

impl TransportAvailabilityObserver {
    fn disconnected(&self, diagnostic: impl Into<String>) {
        self.runtime.observe(
            self.attempt.clone(),
            chrono::Utc::now().timestamp_millis(),
            AvailabilityEvent::Disconnected {
                diagnostic: diagnostic.into(),
            },
        );
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RemoteTransportError {
    pub(crate) message: String,
}

pub(crate) type TransportFuture =
    Pin<Box<dyn Future<Output = Result<TransportConnection, RemoteTransportError>> + Send>>;

pub(crate) trait RemoteTransport: Send + Sync {
    fn connect(
        &self,
        app: AppHandle,
        config: RemoteTransportConfig,
        availability: TransportAvailabilityObserver,
        notification_delivery: RemoteNotificationDeliveryGate,
    ) -> TransportFuture;
}

pub(crate) fn spawn_transport_io<R, W>(
    app: AppHandle,
    reader: R,
    mut writer: W,
    availability: TransportAvailabilityObserver,
    notification_delivery: RemoteNotificationDeliveryGate,
) -> TransportConnection
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let (out_tx, mut out_rx) = mpsc::channel::<String>(OUTBOUND_QUEUE_CAPACITY);
    let pending = Arc::new(Mutex::new(PendingMap::new()));
    let pending_for_writer = Arc::clone(&pending);
    let pending_for_reader = Arc::clone(&pending);

    let connected = Arc::new(AtomicBool::new(true));
    let connected_for_writer = Arc::clone(&connected);
    let connected_for_reader = Arc::clone(&connected);
    let availability_for_writer = availability.clone();

    tokio::spawn(async move {
        while let Some(message) = out_rx.recv().await {
            if writer.write_all(message.as_bytes()).await.is_err()
                || writer.write_all(b"\n").await.is_err()
            {
                mark_disconnected(
                    &pending_for_writer,
                    &connected_for_writer,
                    &availability_for_writer,
                    "transport write failed",
                )
                .await;
                break;
            }
        }
    });

    tokio::spawn(async move {
        read_loop(
            app,
            reader,
            pending_for_reader,
            connected_for_reader,
            availability,
            notification_delivery,
        )
        .await;
    });

    TransportConnection {
        out_tx,
        pending,
        connected,
    }
}

async fn read_loop<R>(
    app: AppHandle,
    reader: R,
    pending: Arc<Mutex<PendingMap>>,
    connected: Arc<AtomicBool>,
    availability: TransportAvailabilityObserver,
    notification_delivery: RemoteNotificationDeliveryGate,
) where
    R: AsyncRead + Unpin + Send + 'static,
{
    let mut lines = BufReader::new(reader).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        dispatch_incoming_line(&app, &pending, &notification_delivery, trimmed).await;
    }

    mark_disconnected(&pending, &connected, &availability, "transport read ended").await;
}

pub(crate) async fn dispatch_incoming_line(
    app: &AppHandle,
    pending: &Arc<Mutex<PendingMap>>,
    notification_delivery: &RemoteNotificationDeliveryGate,
    line: &str,
) {
    let Some(message) = parse_incoming_line(line) else {
        return;
    };

    match message {
        IncomingMessage::Response { id, payload } => {
            let sender = pending.lock().await.remove(&id);
            if let Some(sender) = sender {
                let _ = sender.send(payload);
            }
        }
        IncomingMessage::Notification { method, params } => {
            dispatch_notification_if_current(
                notification_delivery,
                &method,
                params,
                |event, payload| {
                    let _ = app.emit(event, payload);
                },
            );
        }
    }
}

pub(crate) async fn mark_disconnected(
    pending: &Arc<Mutex<PendingMap>>,
    connected: &Arc<AtomicBool>,
    availability: &TransportAvailabilityObserver,
    diagnostic: &str,
) {
    if connected.swap(false, Ordering::SeqCst) {
        availability.disconnected(diagnostic);
    }
    let mut pending = pending.lock().await;
    for (_, sender) in pending.drain() {
        let _ = sender.send(Err(RemoteCallError::Disconnected));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::remote_host_availability::{
        AuthState, DaemonState, RuntimeState, TransportState,
    };

    #[tokio::test]
    async fn transport_disconnect_invalidates_current_availability_and_pending_calls() {
        let runtime = Arc::new(RemoteHostAvailabilityRuntime::default());
        let attempt = runtime.begin_attempt("remote-a", None, 1);
        runtime.observe(attempt.clone(), 2, AvailabilityEvent::TransportConnected);
        runtime.observe(attempt.clone(), 3, AvailabilityEvent::AuthSucceeded);
        let observer = TransportAvailabilityObserver {
            runtime: Arc::clone(&runtime),
            attempt,
        };
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let (sender, receiver) = oneshot::channel();
        pending.lock().await.insert(7, sender);
        let connected = Arc::new(AtomicBool::new(true));

        mark_disconnected(&pending, &connected, &observer, "transport eof").await;

        let snapshot = runtime.snapshot("remote-a").unwrap();
        assert_eq!(snapshot.transport, TransportState::Disconnected);
        assert_eq!(snapshot.auth, AuthState::Unknown);
        assert_eq!(snapshot.daemon, DaemonState::Unknown);
        assert_eq!(snapshot.runtime.state, RuntimeState::Unknown);
        assert_eq!(receiver.await.unwrap(), Err(RemoteCallError::Disconnected));
    }
}
