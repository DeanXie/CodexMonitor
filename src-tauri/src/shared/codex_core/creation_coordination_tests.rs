use super::creation_coordination::*;
use crate::backend::app_server::write_message_to;
use crate::shared::global_sources_core::rollout_identity::CodexThreadKey;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};

const THREAD: &str = "01a05de4-4098-7833-9deb-e3763e15f397";
const TURN: &str = "01a05de4-4098-7833-9deb-e3763e15f398";

#[tokio::test]
async fn real_transport_write_failure_is_possibly_dispatched_not_safe_failure() {
    let c = CreationCoordinator::default();
    let i = id(&c, 1);
    let (writer, reader) = tokio::io::duplex(64);
    drop(reader);
    let writer = tokio::sync::Mutex::new(writer);
    let error = c
        .create(&i, "workspace", |b| async move {
            write_message_to(&writer, json!({"id":1,"method":"thread/start"}), Some(&b)).await?;
            Ok(("home:test".into(), thread_response()))
        })
        .await
        .unwrap_err();
    assert!(error.contains("CREATION_OUTCOME_UNKNOWN"));
}

#[tokio::test]
async fn transport_lock_wait_cancellation_is_definitely_not_dispatched() {
    let c = CreationCoordinator::default();
    let i = id(&c, 1);
    let (writer, _reader) = tokio::io::duplex(64);
    let writer = std::sync::Arc::new(tokio::sync::Mutex::new(writer));
    let locked = writer.lock().await;
    let owner = c.clone();
    let token = i.clone();
    let pending = writer.clone();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        owner
            .create(&token, "workspace", |b| async move {
                tx.send(()).unwrap();
                write_message_to(&pending, json!({"method":"thread/start"}), Some(&b)).await?;
                Ok(("home:test".into(), thread_response()))
            })
            .await
    });
    rx.await.unwrap();
    task.abort();
    let _ = task.await;
    drop(locked);
    assert_eq!(c.creation_status(&i).unwrap()["state"], "CREATION_FAILED");
}

#[tokio::test]
async fn exact_outcome_before_ack_is_applied_only_after_response_correlates_turn() {
    let c = CreationCoordinator::default();
    let i = id(&c, 1);
    create(&c, &i).await.unwrap();
    let t = turn_intent(&c, Some(i), 2);
    c.observe_known_turn_outcome(&key(), TURN, "interrupted");
    turn(&c, &t).await.unwrap();
    assert_eq!(
        c.turn_status(&t.intent).unwrap()["state"],
        "FIRST_TURN_INTERRUPTED"
    );
}
fn id(c: &CreationCoordinator, n: u8) -> IntentId {
    IntentId {
        process_epoch: c.context()["processEpoch"].as_str().unwrap().into(),
        id: format!("01a05de4-4098-7833-9deb-e3763e15f{n:03}"),
    }
}
fn thread_response() -> Value {
    json!({"result":{"thread":{"id":THREAD}}})
}
fn key() -> CodexThreadKey {
    CodexThreadKey::new("home:test", THREAD)
}

#[tokio::test]
async fn cross_home_first_turn_is_rejected_before_transport_dispatch() {
    let c = CreationCoordinator::default();
    let creation = id(&c, 1);
    create(&c, &creation).await.unwrap();
    let t = turn_intent(&c, Some(creation.clone()), 2);
    let calls = &AtomicUsize::new(0);
    let wrong = CodexThreadKey::new("home:other", THREAD);
    let result = c
        .turn(&t, "workspace", THREAD, |boundary| {
            let c = &c;
            let t = &t;
            let wrong = &wrong;
            async move {
                c.validate_turn_target(&t, &wrong)?;
                calls.fetch_add(1, Ordering::SeqCst);
                boundary.mark_dispatched();
                Ok((wrong.clone(), json!({"result":{"turn":{"id":TURN}}})))
            }
        })
        .await;
    assert!(result.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        c.turn_status(&t.intent).unwrap()["state"],
        "FIRST_TURN_FAILED"
    );
}

#[tokio::test]
async fn existing_thread_exact_outcome_before_ack_uses_preflight_bound_key() {
    let c = CreationCoordinator::default();
    let t = turn_intent(&c, None, 2);
    let result = c
        .turn(&t, "workspace", THREAD, |boundary| {
            let c = &c;
            let t = &t;
            async move {
                c.validate_turn_target(t, &key())?;
                boundary.mark_dispatched();
                c.observe_known_turn_outcome(&key(), TURN, "failed");
                Ok((
                    key(),
                    json!({"result":{"turn":{"id":TURN,"status":"inProgress"}}}),
                ))
            }
        })
        .await
        .unwrap();
    assert_eq!(
        result["result"]["firstTurnCoordination"]["state"],
        "FIRST_TURN_FAILED"
    );
}
async fn create(c: &CreationCoordinator, i: &IntentId) -> Result<Value, String> {
    c.create(i, "workspace", |b| async move {
        b.mark_dispatched();
        Ok(("home:test".into(), thread_response()))
    })
    .await
}
fn turn_intent(c: &CreationCoordinator, creation: Option<IntentId>, n: u8) -> TurnIntent {
    TurnIntent {
        intent: id(c, n),
        creation_intent: creation,
    }
}
async fn turn(c: &CreationCoordinator, i: &TurnIntent) -> Result<Value, String> {
    c.turn(i, "workspace", THREAD, |b| async move {
        b.mark_dispatched();
        Ok((key(), json!({"result":{"turn":{"id":TURN}}})))
    })
    .await
}

#[tokio::test]
async fn same_creation_intent_dispatches_thread_start_once() {
    let c = CreationCoordinator::default();
    let i = id(&c, 1);
    let calls = &AtomicUsize::new(0);
    for _ in 0..3 {
        c.create(&i, "workspace", |b| async move {
            calls.fetch_add(1, Ordering::SeqCst);
            b.mark_dispatched();
            Ok(("home:test".into(), thread_response()))
        })
        .await
        .unwrap();
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        c.creation_status(&i).unwrap()["state"],
        "THREAD_ACKNOWLEDGED"
    );
}

#[tokio::test]
async fn concurrent_same_creation_intent_does_not_duplicate_thread() {
    let c = CreationCoordinator::default();
    let i = id(&c, 1);
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let owner = c.clone();
    let token = i.clone();
    let (started, ready) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        owner
            .create(&token, "workspace", |b| async move {
                b.mark_dispatched();
                started.send(()).unwrap();
                rx.await.unwrap();
                Ok(("home:test".into(), thread_response()))
            })
            .await
    });
    ready.await.unwrap();
    let duplicate = create(&c, &i).await;
    assert!(duplicate.unwrap_err().contains("ALREADY_IN_FLIGHT"));
    assert_eq!(c.creation_status(&i).unwrap()["state"], "START_IN_FLIGHT");
    tx.send(()).unwrap();
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn different_creation_intents_can_create_multiple_threads_in_same_workspace() {
    let c = CreationCoordinator::default();
    let calls = &AtomicUsize::new(0);
    for n in 1..=2 {
        c.create(&id(&c, n), "workspace", |b| async move {
            calls.fetch_add(1, Ordering::SeqCst);
            b.mark_dispatched();
            Ok(("home:test".into(), thread_response()))
        })
        .await
        .unwrap();
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn creation_timeout_after_dispatch_becomes_outcome_unknown_and_never_retries() {
    for reason in [
        "timeout",
        "disconnect",
        "partial write failure",
        "request canceled",
    ] {
        let c = CreationCoordinator::default();
        let i = id(&c, 1);
        let error = c
            .create(&i, "workspace", |b| async move {
                b.mark_dispatched();
                Err(reason.into())
            })
            .await
            .unwrap_err();
        assert!(error.contains("CREATION_OUTCOME_UNKNOWN"));
        assert_eq!(
            c.creation_status(&i).unwrap()["state"],
            "CREATION_OUTCOME_UNKNOWN"
        );
        assert!(create(&c, &i)
            .await
            .unwrap_err()
            .contains("CREATION_OUTCOME_UNKNOWN"));
    }
}

#[tokio::test]
async fn failure_before_transport_boundary_is_failed_not_unknown() {
    let c = CreationCoordinator::default();
    let i = id(&c, 1);
    assert!(c
        .create(&i, "workspace", |_| async {
            Err("workspace not found".into())
        })
        .await
        .unwrap_err()
        .contains("CREATION_FAILED"));
    assert_eq!(c.creation_status(&i).unwrap()["state"], "CREATION_FAILED");
    assert!(create(&c, &i).await.is_err());
}

#[tokio::test]
async fn canceled_future_retains_dispatch_certainty_and_cannot_reacquire() {
    for dispatched in [false, true] {
        let c = CreationCoordinator::default();
        let i = id(&c, 1);
        let owner = c.clone();
        let token = i.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            owner
                .create(&token, "workspace", |b| async move {
                    if dispatched {
                        b.mark_dispatched();
                    }
                    tx.send(()).unwrap();
                    std::future::pending::<Result<(String, Value), String>>().await
                })
                .await
        });
        rx.await.unwrap();
        task.abort();
        let _ = task.await;
        assert_eq!(
            c.creation_status(&i).unwrap()["state"],
            if dispatched {
                "CREATION_OUTCOME_UNKNOWN"
            } else {
                "CREATION_FAILED"
            }
        );
        assert!(create(&c, &i).await.is_err());
    }
}

#[tokio::test]
async fn missing_or_invalid_acknowledgement_fails_closed_without_replacement() {
    for response in [
        json!({"result":{}}),
        json!({"error":{"message":"rejected"}}),
    ] {
        let c = CreationCoordinator::default();
        let i = id(&c, 1);
        assert!(c
            .create(&i, "workspace", |b| async move {
                b.mark_dispatched();
                Ok(("home:test".into(), response))
            })
            .await
            .unwrap_err()
            .contains("CREATION_FAILED"));
        assert!(create(&c, &i).await.is_err());
    }
}

#[tokio::test]
async fn late_response_without_retained_correlation_cannot_acknowledge_unknown() {
    let c = CreationCoordinator::default();
    let i = id(&c, 1);
    let _ = c
        .create(&i, "workspace", |b| async move {
            b.mark_dispatched();
            Err("timeout".into())
        })
        .await;
    // A subsequent call offering a valid response is not original request correlation.
    assert!(create(&c, &i).await.is_err());
    assert_eq!(
        c.creation_status(&i).unwrap()["state"],
        "CREATION_OUTCOME_UNKNOWN"
    );
}

#[tokio::test]
async fn first_turn_waits_for_thread_acknowledgement() {
    let c = CreationCoordinator::default();
    let creation = id(&c, 1);
    let t = turn_intent(&c, Some(creation.clone()), 2);
    assert!(turn(&c, &t)
        .await
        .unwrap_err()
        .contains("FIRST_TURN_PENDING"));
    assert_eq!(
        c.turn_status(&t.intent).unwrap()["state"],
        "FIRST_TURN_PENDING"
    );
    create(&c, &creation).await.unwrap();
    turn(&c, &t).await.unwrap();
    assert_eq!(
        c.turn_status(&t.intent).unwrap()["state"],
        "FIRST_TURN_ACCEPTED"
    );
}

#[tokio::test]
async fn acknowledged_thread_requires_first_turn_token_until_acceptance() {
    let c = CreationCoordinator::default();
    let i = id(&c, 1);
    create(&c, &i).await.unwrap();
    assert!(c.requires_first_turn_intent(&key()));
    assert!(!c.requires_first_turn_intent(&CodexThreadKey::new("external", THREAD)));
    turn(&c, &turn_intent(&c, Some(i), 2)).await.unwrap();
    assert!(!c.requires_first_turn_intent(&key()));
}

#[tokio::test]
async fn real_core_missing_session_is_failed_and_not_dispatched() {
    let c = CreationCoordinator::default();
    let i = id(&c, 1);
    let sessions = tokio::sync::Mutex::new(std::collections::HashMap::new());
    let workspaces = tokio::sync::Mutex::new(std::collections::HashMap::new());
    let error = super::start_thread_core(&sessions, &workspaces, "missing".into(), &c, i.clone())
        .await
        .unwrap_err();
    assert!(error.contains("CREATION_FAILED"));
    assert_eq!(c.creation_status(&i).unwrap()["state"], "CREATION_FAILED");
}

#[tokio::test]
async fn acknowledged_thread_can_start_first_turn_without_persistence_confirmation() {
    let c = CreationCoordinator::default();
    let i = id(&c, 1);
    let response = create(&c, &i).await.unwrap();
    assert_eq!(
        response["result"]["creationAcknowledgement"]["persistence"],
        "NOT_YET_CONFIRMED"
    );
    turn(&c, &turn_intent(&c, Some(i), 2)).await.unwrap();
}

#[tokio::test]
async fn same_first_turn_intent_and_duplicate_acknowledgement_dispatch_once() {
    let c = CreationCoordinator::default();
    let i = id(&c, 1);
    create(&c, &i).await.unwrap();
    let t = turn_intent(&c, Some(i.clone()), 2);
    let calls = &AtomicUsize::new(0);
    for _ in 0..3 {
        create(&c, &i).await.unwrap();
        c.turn(&t, "workspace", THREAD, |b| async move {
            calls.fetch_add(1, Ordering::SeqCst);
            b.mark_dispatched();
            Ok((key(), json!({"result":{"turn":{"id":TURN}}})))
        })
        .await
        .unwrap();
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn first_turn_timeout_becomes_unknown_and_does_not_retry() {
    let c = CreationCoordinator::default();
    let i = id(&c, 1);
    create(&c, &i).await.unwrap();
    let t = turn_intent(&c, Some(i.clone()), 2);
    assert!(c
        .turn(&t, "workspace", THREAD, |b| async move {
            b.mark_dispatched();
            Err("timeout".into())
        })
        .await
        .unwrap_err()
        .contains("FIRST_TURN_OUTCOME_UNKNOWN"));
    assert!(turn(&c, &t).await.is_err());
    assert_eq!(
        c.creation_status(&i).unwrap()["threadKey"]["threadId"],
        THREAD
    );
}

#[tokio::test]
async fn first_turn_preflight_failure_is_safe_failure_but_same_intent_never_retries() {
    let c = CreationCoordinator::default();
    let t = turn_intent(&c, None, 2);
    assert!(c
        .turn(&t, "workspace", THREAD, |_| async {
            Err("invalid image".into())
        })
        .await
        .unwrap_err()
        .contains("FIRST_TURN_FAILED"));
    assert!(turn(&c, &t).await.is_err());
}

#[tokio::test]
async fn first_turn_failure_and_interruption_keep_original_thread_id() {
    for outcome in ["failed", "interrupted", "completed"] {
        let c = CreationCoordinator::default();
        let i = id(&c, 1);
        create(&c, &i).await.unwrap();
        let t = turn_intent(&c, Some(i.clone()), 2);
        turn(&c, &t).await.unwrap();
        c.observe_turn_outcome(&t.intent, &key(), TURN, outcome)
            .unwrap();
        assert_eq!(
            c.turn_status(&t.intent).unwrap()["state"],
            match outcome {
                "failed" => "FIRST_TURN_FAILED",
                "interrupted" => "FIRST_TURN_INTERRUPTED",
                _ => "FIRST_TURN_COMPLETED",
            }
        );
        assert_eq!(
            c.creation_status(&i).unwrap()["threadKey"]["threadId"],
            THREAD
        );
        let retry = turn_intent(&c, Some(i.clone()), 3);
        turn(&c, &retry).await.unwrap();
        assert_eq!(
            c.turn_status(&retry.intent).unwrap()["threadKey"]["threadId"],
            THREAD
        );
    }
}

#[tokio::test]
async fn rejected_first_turn_explicit_retry_uses_new_turn_intent_and_same_thread() {
    let c = CreationCoordinator::default();
    let i = id(&c, 1);
    create(&c, &i).await.unwrap();
    let t = turn_intent(&c, Some(i.clone()), 2);
    assert!(c
        .turn(&t, "workspace", THREAD, |b| async move {
            b.mark_dispatched();
            Ok((key(), json!({"error":{"message":"rejected"}})))
        })
        .await
        .is_err());
    let rejected = c.turn_status(&t.intent).unwrap();
    assert_eq!(rejected["state"], "FIRST_TURN_FAILED");
    assert_eq!(rejected["failureReason"], "REJECTED");
    assert!(turn(&c, &t).await.is_err());
    turn(&c, &turn_intent(&c, Some(i.clone()), 3))
        .await
        .unwrap();
    assert_eq!(
        c.creation_status(&i).unwrap()["threadKey"]["threadId"],
        THREAD
    );
}

#[tokio::test]
async fn mismatched_thread_or_turn_evidence_does_not_rewrite_identity() {
    let c = CreationCoordinator::default();
    let i = id(&c, 1);
    create(&c, &i).await.unwrap();
    let t = turn_intent(&c, Some(i), 2);
    turn(&c, &t).await.unwrap();
    assert!(c
        .observe_turn_outcome(
            &t.intent,
            &CodexThreadKey::new("other", THREAD),
            TURN,
            "failed"
        )
        .is_err());
    assert!(c
        .observe_turn_outcome(&t.intent, &key(), "different", "failed")
        .is_err());
    assert_eq!(
        c.turn_status(&t.intent).unwrap()["state"],
        "FIRST_TURN_ACCEPTED"
    );
}

#[tokio::test]
async fn restart_rejects_old_intent_but_known_thread_keeps_exact_id_resume_contract() {
    let old = CreationCoordinator::default();
    let i = id(&old, 1);
    create(&old, &i).await.unwrap();
    let new = CreationCoordinator::default();
    assert!(create(&new, &i)
        .await
        .unwrap_err()
        .contains("STALE_PROCESS_EPOCH"));
    let request =
        super::build_exact_thread_request(super::ExactThreadMethod::Resume, THREAD).unwrap();
    assert_eq!(request.method, "thread/resume");
    assert_eq!(request.params, json!({"threadId":THREAD}));
}

#[tokio::test]
async fn workspace_is_not_dedup_key_but_same_intent_cannot_change_binding() {
    let c = CreationCoordinator::default();
    let i = id(&c, 1);
    create(&c, &i).await.unwrap();
    assert!(c
        .create(&i, "other-workspace", |_| async {
            Ok(("home:test".into(), thread_response()))
        })
        .await
        .unwrap_err()
        .contains("INTENT_BINDING_CONFLICT"));
}

#[tokio::test]
async fn unknown_creation_is_not_claimed_by_similar_cwd_title_prompt_or_sidebar() {
    let c = CreationCoordinator::default();
    let i = id(&c, 1);
    let _ = c
        .create(&i, "workspace", |b| async move {
            b.mark_dispatched();
            Err("lost".into())
        })
        .await;
    let t = turn_intent(&c, Some(i.clone()), 2);
    assert!(turn(&c, &t).await.is_err());
    assert_eq!(
        c.creation_status(&i).unwrap()["state"],
        "CREATION_OUTCOME_UNKNOWN"
    );
}
