use super::*;
use crate::remote_backend::transport::{
    RemoteNotificationDeliveryAuthority, TransportAvailabilityObserver,
};
use crate::shared::remote_host_availability::RemoteHostAvailabilityRuntime;
use crate::shared::remote_host_identity::{DaemonProcessContinuity, DaemonProcessGeneration};
use crate::shared::remote_request_provenance::RemoteRequestProvenanceRuntime;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

fn test_backend(
    availability: Arc<RemoteHostAvailabilityRuntime>,
    process_generation: Option<&str>,
    observed_at: i64,
) -> RemoteBackend {
    let (out_tx, _out_rx) = mpsc::channel(8);
    let attempt = availability.begin_attempt("remote-a", None, observed_at);
    let request_provenance =
        Arc::new(RemoteRequestProvenanceRuntime::new_authenticated_transport());
    let notification_delivery =
        RemoteNotificationDeliveryAuthority::default().new_connection_gate();
    notification_delivery
        .bind_authenticated_generation(request_provenance.transport_generation().clone())
        .expect("bind notification generation");
    let backend = RemoteBackend {
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
            request_provenance: StdMutex::new(Some(request_provenance)),
            notification_delivery,
            daemon_process_generation: StdMutex::new(None),
            daemon_process_continuity: StdMutex::new(DaemonProcessContinuity::Unknown),
        }),
    };
    if let Some(generation) = process_generation {
        backend
            .bind_daemon_process_generation(Some(
                DaemonProcessGeneration::new(generation).expect("process generation"),
            ))
            .expect("bind daemon process generation");
    }
    backend
}

#[tokio::test]
async fn reconnect_to_same_daemon_process_is_distinct_from_restart() {
    let cache = RemoteBackendCache::default();
    let availability = Arc::new(RemoteHostAvailabilityRuntime::default());
    let first = cache
        .get_or_try_initialize(|| async {
            Ok(test_backend(
                Arc::clone(&availability),
                Some("daemon-process-a"),
                10,
            ))
        })
        .await
        .expect("first connection");
    assert_eq!(
        first.daemon_process_continuity(),
        DaemonProcessContinuity::Unknown
    );
    assert!(cache.clear_if_current(&first).await);

    let reconnected = cache
        .get_or_try_initialize(|| async {
            Ok(test_backend(
                Arc::clone(&availability),
                Some("daemon-process-a"),
                20,
            ))
        })
        .await
        .expect("same-process reconnect");

    assert_eq!(
        reconnected.daemon_process_continuity(),
        DaemonProcessContinuity::SameProcess
    );
}

#[tokio::test]
async fn reconnect_after_restart_observes_new_daemon_process_generation() {
    let cache = RemoteBackendCache::default();
    let availability = Arc::new(RemoteHostAvailabilityRuntime::default());
    let first = cache
        .get_or_try_initialize(|| async {
            Ok(test_backend(
                Arc::clone(&availability),
                Some("daemon-process-a"),
                10,
            ))
        })
        .await
        .expect("first connection");
    assert!(cache.clear_if_current(&first).await);

    let restarted = cache
        .get_or_try_initialize(|| async {
            Ok(test_backend(
                Arc::clone(&availability),
                Some("daemon-process-b"),
                20,
            ))
        })
        .await
        .expect("restart reconnect");

    assert_eq!(
        restarted.daemon_process_continuity(),
        DaemonProcessContinuity::RestartedProcess
    );
    assert_eq!(
        restarted
            .daemon_process_generation()
            .expect("current process generation")
            .as_str(),
        "daemon-process-b"
    );
}

#[tokio::test]
async fn missing_process_generation_never_proves_process_continuity() {
    let cache = RemoteBackendCache::default();
    let availability = Arc::new(RemoteHostAvailabilityRuntime::default());
    let first = cache
        .get_or_try_initialize(|| async {
            Ok(test_backend(
                Arc::clone(&availability),
                Some("daemon-process-a"),
                10,
            ))
        })
        .await
        .expect("first connection");
    assert!(cache.clear_if_current(&first).await);

    let unknown = cache
        .get_or_try_initialize(|| async { Ok(test_backend(availability, None, 20)) })
        .await
        .expect("legacy daemon connection");

    assert_eq!(
        unknown.daemon_process_continuity(),
        DaemonProcessContinuity::Unknown
    );
}
