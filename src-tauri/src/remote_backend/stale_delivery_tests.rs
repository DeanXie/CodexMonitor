use super::transport::{
    dispatch_notification_if_current, mark_disconnected, PendingMap,
    RemoteNotificationDeliveryAuthority, RemoteNotificationDeliveryGate,
    TransportAvailabilityObserver,
};
use crate::remote_backend::protocol::RemoteCallError;
use crate::shared::remote_host_availability::{
    AvailabilityEvent, RemoteHostAvailabilityRuntime, TransportState,
};
use crate::shared::remote_request_provenance::RemoteTransportGeneration;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};

fn generation(value: &str) -> RemoteTransportGeneration {
    RemoteTransportGeneration::new(value).expect("valid transport generation")
}

fn authenticated_gate(
    authority: &RemoteNotificationDeliveryAuthority,
    value: &str,
) -> RemoteNotificationDeliveryGate {
    let gate = authority.new_connection_gate();
    gate.bind_authenticated_generation(generation(value))
        .expect("bind authenticated generation");
    gate
}

fn dispatch(gate: &RemoteNotificationDeliveryGate, marker: &str) -> Vec<(String, String)> {
    let mut events = Vec::new();
    let generation = gate
        .authenticated_generation()
        .map(|generation| generation.as_str().to_string())
        .unwrap_or_else(|| "transport-generation-unbound".to_string());
    dispatch_notification_if_current(
        gate,
        "app-server-event",
        serde_json::json!({
            "marker": marker,
            "daemonProcessGeneration": "daemon-generation-current",
            "remoteTransportGeneration": generation,
            "workspaceSessionGeneration": "workspace-generation-current",
            "appServerConnectionGeneration": "connection-generation-current"
        }),
        |event, payload| events.push((event.to_string(), payload.to_string())),
    );
    events
}

#[test]
fn remote_event_binds_current_transport_generation() {
    let authority = RemoteNotificationDeliveryAuthority::default();
    let current = authenticated_gate(&authority, "transport-generation-current");
    authority.publish_current(generation("transport-generation-current"));

    let events = dispatch(&current, "current-generation");

    assert_eq!(events.len(), 1);
    assert!(events[0]
        .1
        .contains("\"remoteTransportGeneration\":\"transport-generation-current\""));
}

#[test]
fn transport_envelope_generation_must_match_delivery_gate() {
    let authority = RemoteNotificationDeliveryAuthority::default();
    let current = authenticated_gate(&authority, "transport-generation-current");
    authority.publish_current(generation("transport-generation-current"));
    let mut events = Vec::new();

    dispatch_notification_if_current(
        &current,
        "app-server-event",
        serde_json::json!({
            "daemonProcessGeneration": "daemon-generation-current",
            "remoteTransportGeneration": "transport-generation-stale",
            "workspaceSessionGeneration": "workspace-generation-current",
            "appServerConnectionGeneration": "connection-generation-current"
        }),
        |event, payload| events.push((event.to_string(), payload)),
    );

    assert!(events.is_empty());
}

#[test]
fn reconnect_old_transport_event_cannot_mutate_new_transport_projection() {
    let authority = RemoteNotificationDeliveryAuthority::default();
    let old = authenticated_gate(&authority, "transport-generation-old");
    let current = authenticated_gate(&authority, "transport-generation-current");
    authority.publish_current(generation("transport-generation-current"));

    assert!(dispatch(&old, "old").is_empty());
    assert_eq!(dispatch(&current, "current").len(), 1);
}

#[test]
fn remote_clients_bind_same_shared_event_to_independent_transport_generations() {
    let first_authority = RemoteNotificationDeliveryAuthority::default();
    let second_authority = RemoteNotificationDeliveryAuthority::default();
    let first = authenticated_gate(&first_authority, "transport-generation-first");
    let second = authenticated_gate(&second_authority, "transport-generation-second");
    first_authority.publish_current(generation("transport-generation-first"));
    second_authority.publish_current(generation("transport-generation-second"));

    let first_events = dispatch(&first, "same-shared-event");
    let second_events = dispatch(&second, "same-shared-event");

    assert_eq!(first_events.len(), 1);
    assert_eq!(second_events.len(), 1);
    assert!(first_events[0].1.contains("transport-generation-first"));
    assert!(second_events[0].1.contains("transport-generation-second"));
}

#[test]
fn generation_event_delivery_does_not_trigger_mutation_replay() {
    let authority = RemoteNotificationDeliveryAuthority::default();
    let current = authenticated_gate(&authority, "transport-generation-current");
    authority.publish_current(generation("transport-generation-current"));

    let delivered = dispatch(&current, "one-delivery");

    assert_eq!(delivered.len(), 1);
}

#[test]
fn old_generation_notification_delivers_while_still_current() {
    let authority = RemoteNotificationDeliveryAuthority::default();
    let old = authenticated_gate(&authority, "transport-generation-old");
    authority.publish_current(generation("transport-generation-old"));
    let events = dispatch(&old, "old-current");

    assert_eq!(events.len(), 1);
    assert!(events[0].1.contains("old-current"));
}

#[test]
fn old_generation_notification_is_dropped_after_replacement() {
    let authority = RemoteNotificationDeliveryAuthority::default();
    let old = authenticated_gate(&authority, "transport-generation-old");
    let current = authenticated_gate(&authority, "transport-generation-current");
    authority.publish_current(generation("transport-generation-old"));
    authority.publish_current(generation("transport-generation-current"));
    let old_events = dispatch(&old, "old-stale");
    let current_events = dispatch(&current, "current");

    assert!(old_events.is_empty());
    assert_eq!(current_events.len(), 1);
    assert!(current_events[0].1.contains("current"));
}

#[test]
fn new_generation_notification_waits_for_ready_publication() {
    let authority = RemoteNotificationDeliveryAuthority::default();
    let old = authenticated_gate(&authority, "transport-generation-old");
    let initializing = authenticated_gate(&authority, "transport-generation-initializing");
    authority.publish_current(generation("transport-generation-old"));
    let initializing_events = dispatch(&initializing, "too-early");
    let old_events = dispatch(&old, "old-still-current");

    assert!(initializing_events.is_empty());
    assert_eq!(old_events.len(), 1);
    assert!(old_events[0].1.contains("old-still-current"));
}

#[test]
fn old_and_new_readers_may_overlap_safely() {
    let authority = RemoteNotificationDeliveryAuthority::default();
    let old = authenticated_gate(&authority, "transport-generation-old");
    let current = authenticated_gate(&authority, "transport-generation-current");
    authority.publish_current(generation("transport-generation-current"));
    let old_events = dispatch(&old, "old-overlap");
    let current_events = dispatch(&current, "new-overlap");

    assert!(old_events.is_empty());
    assert_eq!(current_events.len(), 1);
    assert!(current_events[0].1.contains("new-overlap"));
}

#[test]
fn unauthenticated_or_cleared_generation_cannot_publish_notifications() {
    let authority = RemoteNotificationDeliveryAuthority::default();
    let unauthenticated = authority.new_connection_gate();
    let authenticated = authenticated_gate(&authority, "transport-generation-current");
    authority.publish_current(generation("transport-generation-current"));

    assert!(dispatch(&unauthenticated, "unauthenticated").is_empty());
    authority.clear_current();
    assert!(dispatch(&authenticated, "cleared").is_empty());
}

#[tokio::test]
async fn stale_disconnect_eof_and_read_error_leave_current_attempt_untouched() {
    for diagnostic in [
        "transport disconnected",
        "transport read ended",
        "transport read error",
    ] {
        let availability = Arc::new(RemoteHostAvailabilityRuntime::default());
        let stale = availability.begin_attempt("remote-a", None, 10);
        let current = availability.begin_attempt("remote-a", None, 20);
        availability.observe(current.clone(), 21, AvailabilityEvent::TransportConnected);
        let observer = TransportAvailabilityObserver {
            runtime: Arc::clone(&availability),
            attempt: stale,
        };
        let stale_pending = Arc::new(Mutex::new(PendingMap::new()));
        let (stale_sender, stale_receiver) = oneshot::channel();
        stale_pending.lock().await.insert(1, stale_sender);
        let current_pending = Arc::new(Mutex::new(PendingMap::new()));
        let (current_sender, mut current_receiver) = oneshot::channel();
        current_pending.lock().await.insert(1, current_sender);
        let stale_connected = Arc::new(AtomicBool::new(true));

        mark_disconnected(&stale_pending, &stale_connected, &observer, diagnostic).await;

        assert_eq!(
            stale_receiver.await.expect("stale pending result"),
            Err(RemoteCallError::Disconnected)
        );
        assert!(matches!(
            current_receiver.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ));
        let snapshot = availability.snapshot("remote-a").expect("current snapshot");
        assert_eq!(snapshot.attempt_id, current.attempt_id);
        assert_eq!(snapshot.transport, TransportState::Connected);
        assert!(snapshot.diagnostics.is_empty());

        current_pending
            .lock()
            .await
            .remove(&1)
            .expect("current pending sender")
            .send(Ok(serde_json::json!({ "current": true })))
            .expect("complete current pending call");
        assert_eq!(
            current_receiver.await.expect("current pending result"),
            Ok(serde_json::json!({ "current": true }))
        );
    }
}
