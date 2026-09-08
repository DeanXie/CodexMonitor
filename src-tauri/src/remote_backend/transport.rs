use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot, Mutex};

use super::protocol::{parse_incoming_line, IncomingMessage, RemoteCallError};
use crate::shared::remote_host_availability::{
    AvailabilityAttempt, AvailabilityEvent, RemoteHostAvailabilityRuntime,
};

pub(crate) type PendingMap = HashMap<u64, oneshot::Sender<Result<Value, RemoteCallError>>>;
const OUTBOUND_QUEUE_CAPACITY: usize = 512;

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
    ) -> TransportFuture;
}

pub(crate) fn spawn_transport_io<R, W>(
    app: AppHandle,
    reader: R,
    mut writer: W,
    availability: TransportAvailabilityObserver,
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
) where
    R: AsyncRead + Unpin + Send + 'static,
{
    let mut lines = BufReader::new(reader).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        dispatch_incoming_line(&app, &pending, trimmed).await;
    }

    mark_disconnected(&pending, &connected, &availability, "transport read ended").await;
}

pub(crate) async fn dispatch_incoming_line(
    app: &AppHandle,
    pending: &Arc<Mutex<PendingMap>>,
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
        IncomingMessage::Notification { method, params } => match method.as_str() {
            "app-server-event" => {
                let _ = app.emit("app-server-event", params);
            }
            "terminal-output" => {
                let _ = app.emit("terminal-output", params);
            }
            "terminal-exit" => {
                let _ = app.emit("terminal-exit", params);
            }
            _ => {}
        },
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
