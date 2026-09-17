use super::*;
use crate::shared::remote_request_provenance::{
    RemoteRequestDispatchContext, RemoteRequestDispatchState, RemoteRequestTransitionError,
    RemoteTransportGeneration, SessionAttemptKind,
};
use crate::shared::codex_core::thread_lifecycle_observation::ThreadSubscriptionObservationState;
use crate::shared::codex_core::writer_admission_observation::WriterAdmissionObservationState;
use crate::shared::codex_core::approval_decision_provenance::ApprovalDecisionState;
use crate::shared::codex_core::delete_mutation_observation::DeleteMutationState;
use std::sync::atomic::Ordering;

async fn respond_to_resume(session: &WorkspaceSession, thread_id: &str) {
    for _ in 0..100 {
        let request_id = session.pending.lock().await.keys().next().copied();
        if let Some(request_id) = request_id {
            session
                .pending
                .lock()
                .await
                .remove(&request_id)
                .expect("pending resume request")
                .send(json!({
                    "id": request_id,
                    "result": { "thread": { "id": thread_id } },
                }))
                .expect("deliver resume response");
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("resume request was not dispatched");
}

async fn respond_to_delete(session: &WorkspaceSession) {
    for _ in 0..100 {
        let request_id = session.pending.lock().await.keys().next().copied();
        if let Some(request_id) = request_id {
            session
                .pending
                .lock()
                .await
                .remove(&request_id)
                .expect("pending delete request")
                .send(json!({
                    "id": request_id,
                    "result": {},
                }))
                .expect("deliver delete response");
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("delete request was not dispatched");
}

fn remote_context(
    generation: &str,
    request_id: u64,
    method: &str,
) -> (
    Arc<RemoteRequestProvenanceRuntime>,
    RemoteRequestKey,
    RemoteRequestDispatchContext,
) {
    let provenance = Arc::new(RemoteRequestProvenanceRuntime::new(
        RemoteTransportGeneration::new(generation).unwrap(),
    ));
    let key = provenance
        .record_received(request_id, method, 10)
        .expect("record request");
    provenance
        .record_dispatch_started(&key, 20)
        .expect("dispatch starts");
    let context = RemoteRequestDispatchContext::new(Arc::clone(&provenance), key.clone());
    (provenance, key, context)
}

#[test]
fn remote_delete_binds_transport_to_exact_session_attempt() {
    run_async_test(async {
        let tmp = make_temp_dir("remote-delete-correlation");
        let workspace_id = "remote-delete-correlation-workspace";
        let thread_id = "0199a8c0-1111-7222-8333-444455556666";
        let state = test_state(&tmp);
        insert_workspace(&state, workspace_id, &tmp.to_string_lossy()).await;
        let session = make_session(make_workspace_entry(workspace_id, &tmp.to_string_lossy()));
        state
            .sessions
            .lock()
            .await
            .insert(workspace_id.to_string(), Arc::clone(&session));
        let (provenance, key, context) =
            remote_context("transport-delete-a", 61, "delete_thread");

        let call = rpc::handle_rpc_request_with_context(
            &state,
            "delete_thread",
            json!({ "workspaceId": workspace_id, "threadId": thread_id }),
            "daemon-test".to_string(),
            Some(&context),
        );
        let response = respond_to_delete(&session);
        let (result, ()) = tokio::join!(call, response);
        assert_eq!(result.expect("delete succeeds")["result"], json!({}));

        let attempt = provenance
            .snapshot(&key)
            .expect("transport provenance")
            .session_attempt
            .expect("delete attempt correlation");
        assert_eq!(attempt.kind, SessionAttemptKind::DeleteMutation);
        assert_eq!(attempt.workspace_id, workspace_id);
        assert_eq!(attempt.thread_key.thread_id, thread_id);
        assert_eq!(
            session
                .delete_mutation_observations
                .latest_for_thread(&attempt.thread_key)
                .expect("delete observation")
                .state,
            DeleteMutationState::DeleteConfirmed
        );

        stop_session(&session).await;
        let _ = std::fs::remove_dir_all(tmp);
    });
}

async fn stop_session(session: &WorkspaceSession) {
    let mut child = session.child.lock().await;
    let _ = child.kill().await;
    let _ = child.wait().await;
}

fn seed_command_approval(session: &WorkspaceSession, request_id: u64) {
    session
        .approval_observations
        .observe_request(
            &json!({
                "id": request_id,
                "method": "item/commandExecution/requestApproval",
                "params": {
                    "threadId": "thread-approval",
                    "turnId": "turn-approval",
                    "itemId": "item-approval",
                    "approvalId": "approval-callback"
                }
            }),
            10,
        )
        .expect("seed approval");
}

#[test]
fn remote_approval_decision_binds_transport_to_exact_session_attempt() {
    run_async_test(async {
        let tmp = make_temp_dir("remote-approval-correlation-red");
        let workspace_id = "remote-approval-correlation-workspace";
        let state = test_state(&tmp);
        insert_workspace(&state, workspace_id, &tmp.to_string_lossy()).await;
        let session = make_session(make_workspace_entry(workspace_id, &tmp.to_string_lossy()));
        seed_command_approval(&session, 7);
        state.sessions.lock().await.insert(workspace_id.to_string(), Arc::clone(&session));
        let (provenance, key, context) =
            remote_context("transport-approval-a", 41, "respond_to_server_request");

        let result = rpc::handle_rpc_request_with_context(
            &state,
            "respond_to_server_request",
            json!({"workspaceId":workspace_id,"requestId":7,"result":{"decision":"accept"}}),
            "daemon-test".to_string(),
            Some(&context),
        )
        .await;
        assert!(result.is_ok());
        let attempts = session.approval_observations.decision_attempts();
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].state, ApprovalDecisionState::DecisionDispatched);
        assert_eq!(attempts[0].dispatch_count, 1);
        let correlation = provenance
            .snapshot(&key)
            .unwrap()
            .session_attempt
            .expect("decision attempt correlation");
        assert_eq!(correlation.kind, SessionAttemptKind::ApprovalDecision);
        assert_eq!(correlation.workspace_id, workspace_id);
        assert_eq!(correlation.thread_key.thread_id, "thread-approval");
        assert_eq!(
            correlation.app_server_connection_generation.as_deref(),
            Some(session.approval_observations.app_server_connection_generation().as_str())
        );
        stop_session(&session).await;
        let _ = std::fs::remove_dir_all(tmp);
    });
}

#[test]
fn simultaneous_remote_approval_decisions_write_at_most_once() {
    run_async_test(async {
        let tmp = make_temp_dir("remote-approval-simultaneous");
        let workspace_id = "remote-approval-simultaneous-workspace";
        let state = test_state(&tmp);
        insert_workspace(&state, workspace_id, &tmp.to_string_lossy()).await;
        let session = make_session(make_workspace_entry(workspace_id, &tmp.to_string_lossy()));
        seed_command_approval(&session, 7);
        state.sessions.lock().await.insert(workspace_id.to_string(), Arc::clone(&session));
        let (_, _, first_context) =
            remote_context("transport-approval-a", 1, "respond_to_server_request");
        let (_, _, second_context) =
            remote_context("transport-approval-b", 1, "respond_to_server_request");
        let first = rpc::handle_rpc_request_with_context(
            &state,
            "respond_to_server_request",
            json!({"workspaceId":workspace_id,"requestId":7,"result":{"decision":"accept"}}),
            "daemon-test".to_string(),
            Some(&first_context),
        );
        let second = rpc::handle_rpc_request_with_context(
            &state,
            "respond_to_server_request",
            json!({"workspaceId":workspace_id,"requestId":7,"result":{"decision":"decline"}}),
            "daemon-test".to_string(),
            Some(&second_context),
        );
        let (first, second) = tokio::join!(first, second);
        assert_eq!([first.is_ok(), second.is_ok()].into_iter().filter(|ok| *ok).count(), 1);
        let attempts = session.approval_observations.decision_attempts();
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts.iter().map(|attempt| attempt.dispatch_count).sum::<u32>(), 1);
        stop_session(&session).await;
        let _ = std::fs::remove_dir_all(tmp);
    });
}

#[test]
fn wrong_approval_response_schema_dispatches_zero_messages() {
    run_async_test(async {
        let tmp = make_temp_dir("remote-approval-wrong-schema");
        let workspace_id = "remote-approval-wrong-schema-workspace";
        let state = test_state(&tmp);
        insert_workspace(&state, workspace_id, &tmp.to_string_lossy()).await;
        let session = make_session(make_workspace_entry(workspace_id, &tmp.to_string_lossy()));
        seed_command_approval(&session, 7);
        state
            .sessions
            .lock()
            .await
            .insert(workspace_id.to_string(), Arc::clone(&session));
        let (_, _, context) =
            remote_context("transport-approval-invalid", 1, "respond_to_server_request");

        let result = rpc::handle_rpc_request_with_context(
            &state,
            "respond_to_server_request",
            json!({"workspaceId":workspace_id,"requestId":7,"result":{"decision":"acceptAlways"}}),
            "daemon-test".to_string(),
            Some(&context),
        )
        .await;
        assert!(result.is_err());
        let attempt = session.approval_observations.decision_attempts().remove(0);
        assert_eq!(attempt.state, ApprovalDecisionState::DecisionNotDispatched);
        assert_eq!(attempt.dispatch_count, 0);
        assert_eq!(attempt.retry_count, 0);
        stop_session(&session).await;
        let _ = std::fs::remove_dir_all(tmp);
    });
}

#[test]
fn approval_transport_loss_after_write_records_outcome_unknown() {
    run_async_test(async {
        let tmp = make_temp_dir("remote-approval-post-write-loss");
        let workspace_id = "remote-approval-post-write-loss-workspace";
        let state = test_state(&tmp);
        insert_workspace(&state, workspace_id, &tmp.to_string_lossy()).await;
        let session = make_session(make_workspace_entry(workspace_id, &tmp.to_string_lossy()));
        seed_command_approval(&session, 7);
        state.sessions.lock().await.insert(workspace_id.to_string(), Arc::clone(&session));
        let (provenance, _, context) =
            remote_context("transport-approval-loss", 1, "respond_to_server_request");
        rpc::handle_rpc_request_with_context(
            &state,
            "respond_to_server_request",
            json!({"workspaceId":workspace_id,"requestId":7,"result":{"decision":"accept"}}),
            "daemon-test".to_string(),
            Some(&context),
        )
        .await
        .expect("response write");
        provenance.record_transport_lost(30);
        let attempt = session.approval_observations.decision_attempts().remove(0);
        assert_eq!(attempt.state, ApprovalDecisionState::DecisionOutcomeUnknown);
        assert_eq!(attempt.dispatch_count, 1);
        assert_eq!(attempt.retry_count, 0);
        stop_session(&session).await;
        let _ = std::fs::remove_dir_all(tmp);
    });
}

#[test]
fn stale_old_transport_cannot_start_approval_decision_but_new_transport_can() {
    run_async_test(async {
        let tmp = make_temp_dir("remote-approval-reconnect");
        let workspace_id = "remote-approval-reconnect-workspace";
        let state = test_state(&tmp);
        insert_workspace(&state, workspace_id, &tmp.to_string_lossy()).await;
        let session = make_session(make_workspace_entry(workspace_id, &tmp.to_string_lossy()));
        seed_command_approval(&session, 7);
        state.sessions.lock().await.insert(workspace_id.to_string(), Arc::clone(&session));
        let (old_runtime, _, old_context) =
            remote_context("transport-approval-old", 1, "respond_to_server_request");
        old_runtime.record_transport_lost(25);
        let old = rpc::handle_rpc_request_with_context(
            &state,
            "respond_to_server_request",
            json!({"workspaceId":workspace_id,"requestId":7,"result":{"decision":"accept"}}),
            "daemon-test".to_string(),
            Some(&old_context),
        )
        .await;
        assert!(old.is_err());
        assert!(session.approval_observations.decision_attempts().is_empty());

        let (_, _, new_context) =
            remote_context("transport-approval-new", 1, "respond_to_server_request");
        let new = rpc::handle_rpc_request_with_context(
            &state,
            "respond_to_server_request",
            json!({"workspaceId":workspace_id,"requestId":7,"result":{"decision":"accept"}}),
            "daemon-test".to_string(),
            Some(&new_context),
        )
        .await;
        assert!(new.is_ok());
        assert_eq!(session.approval_observations.decision_attempts().len(), 1);
        stop_session(&session).await;
        let _ = std::fs::remove_dir_all(tmp);
    });
}

#[test]
fn resume_request_binds_transport_to_existing_writer_attempt() {
    run_async_test(async {
        let tmp = make_temp_dir("remote-resume-correlation-red");
        let workspace_id = "remote-resume-correlation-workspace";
        let thread_id = "01a08c05-7880-75a2-976c-2a5895b58723";
        let state = test_state(&tmp);
        insert_workspace(&state, workspace_id, &tmp.to_string_lossy()).await;
        let session = make_session_with_generation(
            make_workspace_entry(workspace_id, &tmp.to_string_lossy()),
            "session-generation-1",
        );
        state
            .sessions
            .lock()
            .await
            .insert(workspace_id.to_string(), Arc::clone(&session));

        let (provenance, key, context) =
            remote_context("transport-generation-a", 7, "resume_thread");

        let call = rpc::handle_rpc_request_with_context(
            &state,
            "resume_thread",
            json!({ "workspaceId": workspace_id, "threadId": thread_id }),
            "daemon-test".to_string(),
            Some(&context),
        );
        let response = respond_to_resume(&session, thread_id);
        let (result, ()) = tokio::join!(call, response);
        assert_eq!(
            result.expect("resume succeeds")["result"]["thread"]["id"],
            thread_id
        );

        let observation = provenance.snapshot(&key).expect("provenance");
        let attempt = observation.session_attempt.expect("attempt correlation");
        assert_eq!(attempt.kind, SessionAttemptKind::WriterAdmission);
        assert_eq!(attempt.workspace_id, workspace_id);
        assert_eq!(attempt.workspace_session_generation, "session-generation-1");
        assert_eq!(attempt.thread_key.thread_id, thread_id);
        assert!(!attempt.attempt_id.is_empty());
        assert_eq!(observation.dispatch_state, RemoteRequestDispatchState::SessionAttemptBound);
        assert_eq!(session.next_id.load(Ordering::SeqCst), 1);

        stop_session(&session).await;
        let _ = std::fs::remove_dir_all(tmp);
    });
}

#[test]
fn transport_lost_before_dispatch_is_rejected() {
    let provenance = RemoteRequestProvenanceRuntime::new(
        RemoteTransportGeneration::new("transport-generation-lost").unwrap(),
    );
    let key = provenance
        .record_received(11, "resume_thread", 10)
        .expect("record request");
    provenance.record_transport_lost(20);

    assert_eq!(
        provenance.record_dispatch_started(&key, 30),
        Err(RemoteRequestTransitionError::TransportLostBeforeDispatch)
    );
    let observation = provenance.snapshot(&key).expect("provenance");
    assert_eq!(observation.dispatch_started_at, None);
    assert_eq!(observation.session_attempt, None);
    assert_eq!(observation.dispatch_state, RemoteRequestDispatchState::TransportLost);
}

#[test]
fn pre_dispatch_transport_loss_spawns_zero_session_attempts_and_mutations() {
    run_async_test(async {
        let tmp = make_temp_dir("remote-pre-dispatch-loss");
        let workspace_id = "remote-pre-dispatch-loss-workspace";
        let thread_id = "thread-pre-dispatch-loss";
        let state = Arc::new(test_state(&tmp));
        insert_workspace(&state, workspace_id, &tmp.to_string_lossy()).await;
        let session = make_session(make_workspace_entry(workspace_id, &tmp.to_string_lossy()));
        state
            .sessions
            .lock()
            .await
            .insert(workspace_id.to_string(), Arc::clone(&session));
        let provenance = Arc::new(RemoteRequestProvenanceRuntime::new(
            RemoteTransportGeneration::new("transport-pre-dispatch-loss").unwrap(),
        ));
        let key = provenance
            .record_received(1, "resume_thread", 10)
            .expect("record request");
        let limiter = Arc::new(Semaphore::new(0));
        let (out_tx, mut out_rx) = mpsc::unbounded_channel();

        rpc::spawn_rpc_response_task(
            Arc::clone(&state),
            out_tx,
            Some(1),
            "resume_thread".to_string(),
            json!({ "workspaceId": workspace_id, "threadId": thread_id }),
            "daemon-test".to_string(),
            Arc::clone(&limiter),
            Arc::clone(&provenance),
            Some(key.clone()),
        );
        provenance.record_transport_lost(20);
        limiter.add_permits(1);
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert_eq!(session.next_id.load(Ordering::SeqCst), 0);
        assert!(session.pending.lock().await.is_empty());
        assert!(out_rx.try_recv().is_err());
        let observation = provenance.snapshot(&key).expect("provenance");
        assert_eq!(observation.dispatch_started_at, None);
        assert_eq!(observation.session_attempt, None);
        assert_eq!(observation.dispatch_state, RemoteRequestDispatchState::TransportLost);

        stop_session(&session).await;
        let _ = std::fs::remove_dir_all(tmp);
    });
}

#[test]
fn disconnect_after_attempt_binding_preserves_direct_session_evidence() {
    run_async_test(async {
        let tmp = make_temp_dir("remote-post-dispatch-loss");
        let workspace_id = "remote-post-dispatch-loss-workspace";
        let thread_id = "thread-post-dispatch-loss";
        let state = test_state(&tmp);
        insert_workspace(&state, workspace_id, &tmp.to_string_lossy()).await;
        let session = make_session(make_workspace_entry(workspace_id, &tmp.to_string_lossy()));
        state
            .sessions
            .lock()
            .await
            .insert(workspace_id.to_string(), Arc::clone(&session));
        let (provenance, key, context) =
            remote_context("transport-post-dispatch-loss", 1, "resume_thread");

        let call = rpc::handle_rpc_request_with_context(
            &state,
            "resume_thread",
            json!({ "workspaceId": workspace_id, "threadId": thread_id }),
            "daemon-test".to_string(),
            Some(&context),
        );
        let disconnect_then_respond = async {
            for _ in 0..100 {
                if !session.pending.lock().await.is_empty() {
                    provenance.record_transport_lost(30);
                    respond_to_resume(&session, thread_id).await;
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            panic!("resume dispatch not observed");
        };
        let (result, ()) = tokio::join!(call, disconnect_then_respond);
        assert!(result.is_ok());
        let observation = provenance.snapshot(&key).expect("provenance");
        assert_eq!(observation.dispatch_state, RemoteRequestDispatchState::TransportLost);
        assert!(observation.session_attempt.is_some());
        let thread_key = observation.session_attempt.unwrap().thread_key;
        assert_eq!(
            session.writer_admission_observations.state(&thread_key),
            WriterAdmissionObservationState::AdmittedForSession,
            "direct upstream success remains authoritative after transport loss"
        );
        assert_eq!(session.next_id.load(Ordering::SeqCst), 1);

        stop_session(&session).await;
        let _ = std::fs::remove_dir_all(tmp);
    });
}

#[test]
fn remote_unsubscribe_binds_transport_request_to_existing_attempt() {
    run_async_test(async {
        let tmp = make_temp_dir("remote-unsubscribe-correlation");
        let workspace_id = "remote-unsubscribe-correlation-workspace";
        let thread_id = "thread-unsubscribe-correlation";
        let state = test_state(&tmp);
        insert_workspace(&state, workspace_id, &tmp.to_string_lossy()).await;
        let session = make_session(make_workspace_entry(workspace_id, &tmp.to_string_lossy()));
        seed_thread_subscription(&session, thread_id).await;
        state
            .sessions
            .lock()
            .await
            .insert(workspace_id.to_string(), Arc::clone(&session));
        let (provenance, key, context) = remote_context(
            "transport-unsubscribe-single",
            5,
            "thread_upstream_unsubscribe",
        );

        let call = rpc::handle_rpc_request_with_context(
            &state,
            "thread_upstream_unsubscribe",
            json!({ "workspaceId": workspace_id, "threadId": thread_id }),
            "daemon-test".to_string(),
            Some(&context),
        );
        let response = respond_to_next_app_server_request(&session, "unsubscribed");
        let (result, ()) = tokio::join!(call, response);
        assert!(result.is_ok());
        let attempt = provenance
            .snapshot(&key)
            .unwrap()
            .session_attempt
            .expect("unsubscribe attempt correlation");
        assert_eq!(attempt.kind, SessionAttemptKind::UpstreamUnsubscribe);
        assert_eq!(attempt.workspace_id, workspace_id);
        assert_eq!(attempt.thread_key.thread_id, thread_id);
        assert!(attempt.app_server_connection_generation.is_some());
        assert_eq!(session.next_id.load(Ordering::SeqCst), 1);

        stop_session(&session).await;
        let _ = std::fs::remove_dir_all(tmp);
    });
}

#[test]
fn disconnect_after_unsubscribe_dispatch_preserves_direct_session_result() {
    run_async_test(async {
        let tmp = make_temp_dir("remote-unsubscribe-post-dispatch-loss");
        let workspace_id = "remote-unsubscribe-post-dispatch-loss-workspace";
        let thread_id = "thread-unsubscribe-post-dispatch-loss";
        let state = test_state(&tmp);
        insert_workspace(&state, workspace_id, &tmp.to_string_lossy()).await;
        let session = make_session(make_workspace_entry(workspace_id, &tmp.to_string_lossy()));
        seed_thread_subscription(&session, thread_id).await;
        state
            .sessions
            .lock()
            .await
            .insert(workspace_id.to_string(), Arc::clone(&session));
        let (provenance, key, context) = remote_context(
            "transport-unsubscribe-post-dispatch-loss",
            6,
            "thread_upstream_unsubscribe",
        );

        let call = rpc::handle_rpc_request_with_context(
            &state,
            "thread_upstream_unsubscribe",
            json!({ "workspaceId": workspace_id, "threadId": thread_id }),
            "daemon-test".to_string(),
            Some(&context),
        );
        let disconnect_then_respond = async {
            for _ in 0..100 {
                if !session.pending.lock().await.is_empty() {
                    provenance.record_transport_lost(30);
                    respond_to_next_app_server_request(&session, "unsubscribed").await;
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            panic!("unsubscribe dispatch not observed");
        };
        let (result, ()) = tokio::join!(call, disconnect_then_respond);
        assert!(result.is_ok());
        let observation = provenance.snapshot(&key).unwrap();
        assert_eq!(observation.dispatch_state, RemoteRequestDispatchState::TransportLost);
        let attempt = observation.session_attempt.expect("attempt remains bound");
        assert_eq!(
            session
                .thread_lifecycle_observations
                .subscription_snapshot(&attempt.thread_key)
                .state,
            ThreadSubscriptionObservationState::UnsubscribedForAppServerConnection,
            "direct upstream result remains authoritative after transport loss"
        );
        assert_eq!(session.next_id.load(Ordering::SeqCst), 1);

        stop_session(&session).await;
        let _ = std::fs::remove_dir_all(tmp);
    });
}

#[test]
fn same_request_id_across_transports_binds_distinct_resume_attempts() {
    run_async_test(async {
        let tmp = make_temp_dir("remote-two-resume-transports");
        let workspace_id = "remote-two-resume-transports-workspace";
        let thread_id = "thread-two-resume-transports";
        let state = test_state(&tmp);
        insert_workspace(&state, workspace_id, &tmp.to_string_lossy()).await;
        let session = make_session(make_workspace_entry(workspace_id, &tmp.to_string_lossy()));
        state
            .sessions
            .lock()
            .await
            .insert(workspace_id.to_string(), Arc::clone(&session));
        let (first_runtime, first_key, first_context) =
            remote_context("transport-resume-a", 1, "resume_thread");
        let (second_runtime, second_key, second_context) =
            remote_context("transport-resume-b", 1, "resume_thread");

        let first = rpc::handle_rpc_request_with_context(
            &state,
            "resume_thread",
            json!({ "workspaceId": workspace_id, "threadId": thread_id }),
            "daemon-test".to_string(),
            Some(&first_context),
        );
        let second = rpc::handle_rpc_request_with_context(
            &state,
            "resume_thread",
            json!({ "workspaceId": workspace_id, "threadId": thread_id }),
            "daemon-test".to_string(),
            Some(&second_context),
        );
        let respond = async {
            for _ in 0..100 {
                let ids = session.pending.lock().await.keys().copied().collect::<Vec<_>>();
                if ids.len() == 2 {
                    for request_id in ids {
                        session
                            .pending
                            .lock()
                            .await
                            .remove(&request_id)
                            .unwrap()
                            .send(json!({
                                "id": request_id,
                                "error": { "code": -32600, "message": "already has an active writer" },
                            }))
                            .unwrap();
                    }
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            panic!("two resume dispatches not observed");
        };
        let (first, second, ()) = tokio::join!(first, second, respond);
        assert!(first.is_ok());
        assert!(second.is_ok());
        let first_attempt = first_runtime
            .snapshot(&first_key)
            .unwrap()
            .session_attempt
            .unwrap();
        let second_attempt = second_runtime
            .snapshot(&second_key)
            .unwrap()
            .session_attempt
            .unwrap();
        assert_ne!(first_attempt.attempt_id, second_attempt.attempt_id);
        assert_eq!(first_key.transport_request_id, second_key.transport_request_id);
        assert_ne!(first_key.transport_generation, second_key.transport_generation);
        assert_eq!(session.next_id.load(Ordering::SeqCst), 2);

        stop_session(&session).await;
        let _ = std::fs::remove_dir_all(tmp);
    });
}

#[test]
fn duplicate_same_transport_resume_creates_no_second_task_or_attempt() {
    run_async_test(async {
        let tmp = make_temp_dir("remote-duplicate-resume");
        let workspace_id = "remote-duplicate-resume-workspace";
        let thread_id = "thread-duplicate-resume";
        let state = Arc::new(test_state(&tmp));
        insert_workspace(&state, workspace_id, &tmp.to_string_lossy()).await;
        let session = make_session(make_workspace_entry(workspace_id, &tmp.to_string_lossy()));
        state
            .sessions
            .lock()
            .await
            .insert(workspace_id.to_string(), Arc::clone(&session));

        let (events, _) = broadcast::channel(16);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let config = Arc::new(DaemonConfig {
            listen: address,
            token: Some("secret-token".to_string()),
            data_dir: tmp.clone(),
        });
        let server_state = Arc::clone(&state);
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            transport::handle_client(socket, config, server_state, events).await;
        });

        let stream = TcpStream::connect(address).await.unwrap();
        let (reader, mut writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();
        writer
            .write_all(
                b"{\"id\":99,\"method\":\"auth\",\"params\":{\"token\":\"secret-token\"}}\n",
            )
            .await
            .unwrap();
        let _: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        let request = format!(
            "{{\"id\":1,\"method\":\"resume_thread\",\"params\":{{\"workspaceId\":\"{workspace_id}\",\"threadId\":\"{thread_id}\"}}}}\n"
        );
        writer.write_all(request.as_bytes()).await.unwrap();
        writer.write_all(request.as_bytes()).await.unwrap();

        let respond = respond_to_resume(&session, thread_id);
        let read_responses = async {
            let first: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            let second: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            [first, second]
        };
        let ((), responses) = tokio::join!(respond, read_responses);
        assert_eq!(
            responses
                .iter()
                .filter(|response| {
                    response.pointer("/error/message")
                        == Some(&json!("duplicate request id for current transport"))
                })
                .count(),
            1
        );
        assert_eq!(
            responses
                .iter()
                .filter(|response| {
                    response.pointer("/result/result/thread/id") == Some(&json!(thread_id))
                })
                .count(),
            1
        );
        assert_eq!(session.next_id.load(Ordering::SeqCst), 1);

        drop(writer);
        server.await.unwrap();
        stop_session(&session).await;
        let _ = std::fs::remove_dir_all(tmp);
    });
}

#[test]
fn simultaneous_unsubscribe_preserves_shared_pending_gate_and_provenance() {
    run_async_test(async {
        let tmp = make_temp_dir("remote-concurrent-unsubscribe");
        let workspace_id = "remote-concurrent-unsubscribe-workspace";
        let thread_id = "thread-concurrent-unsubscribe";
        let state = test_state(&tmp);
        insert_workspace(&state, workspace_id, &tmp.to_string_lossy()).await;
        let session = make_session(make_workspace_entry(workspace_id, &tmp.to_string_lossy()));
        seed_thread_subscription(&session, thread_id).await;
        state
            .sessions
            .lock()
            .await
            .insert(workspace_id.to_string(), Arc::clone(&session));
        let (first_runtime, first_key, first_context) =
            remote_context("transport-unsubscribe-a", 1, "thread_upstream_unsubscribe");
        let (second_runtime, second_key, second_context) =
            remote_context("transport-unsubscribe-b", 1, "thread_upstream_unsubscribe");

        let first = rpc::handle_rpc_request_with_context(
            &state,
            "thread_upstream_unsubscribe",
            json!({ "workspaceId": workspace_id, "threadId": thread_id }),
            "daemon-test".to_string(),
            Some(&first_context),
        );
        let second_then_respond = async {
            for _ in 0..100 {
                if !session.pending.lock().await.is_empty() {
                    let second = rpc::handle_rpc_request_with_context(
                        &state,
                        "thread_upstream_unsubscribe",
                        json!({ "workspaceId": workspace_id, "threadId": thread_id }),
                        "daemon-test".to_string(),
                        Some(&second_context),
                    )
                    .await;
                    respond_to_next_app_server_request(&session, "unsubscribed").await;
                    return second;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            panic!("first unsubscribe dispatch not observed");
        };
        let (first, second) = tokio::join!(first, second_then_respond);
        assert!(first.is_ok());
        assert!(second.is_err(), "shared pending gate rejects second attempt");
        let first_attempt = first_runtime
            .snapshot(&first_key)
            .unwrap()
            .session_attempt
            .expect("first unsubscribe attempt correlated");
        assert_eq!(first_attempt.kind, SessionAttemptKind::UpstreamUnsubscribe);
        assert!(first_attempt.app_server_connection_generation.is_some());
        assert!(second_runtime
            .snapshot(&second_key)
            .unwrap()
            .session_attempt
            .is_none());
        assert_eq!(session.next_id.load(Ordering::SeqCst), 1);

        stop_session(&session).await;
        let _ = std::fs::remove_dir_all(tmp);
    });
}
