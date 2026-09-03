use super::creation_acknowledgement::*;
use crate::shared::global_sources_core::rollout_identity::{CodexThreadKey, CodexTurnKey};
use crate::shared::global_sources_core::rollout_record::{
    ParsedRolloutRecord, RolloutRecordParser,
};
use crate::shared::global_sources_core::source_envelope::CodexHomeIdentity;
use serde_json::{json, Value};

const ID: &str = "01a05de4-4098-7833-9deb-e3763e15f397";
const TURN: &str = "01a05de4-59d9-7533-8479-c44425d0f851";
const HOME: &str = "codex-home:test";

fn response() -> Value {
    json!({"id": 7, "result": {"thread": {"id": ID}, "model": "requested-is-not-identity"}})
}

fn ack() -> CreationAcknowledgement {
    acknowledge_thread_start(HOME, None, &response()).unwrap()
}

#[test]
fn successful_thread_start_acknowledges_exact_server_thread_id() {
    let facts = ack();
    assert_eq!(facts.thread_key(), &CodexThreadKey::new(HOME, ID));
    assert_eq!(facts.state(), CreationState::ThreadAcknowledged);
    let mut upper = response();
    upper["result"]["thread"]["id"] = json!(ID.to_uppercase());
    assert_eq!(
        acknowledge_thread_start(HOME, None, &upper)
            .unwrap()
            .thread_key()
            .thread_id,
        ID.to_uppercase()
    );
}

#[test]
fn missing_thread_id_is_not_acknowledged() {
    for value in [
        json!({}),
        json!({"result":{"thread":{}}}),
        Value::Null,
        json!("unparsed response"),
    ] {
        assert_eq!(
            acknowledge_thread_start(HOME, None, &value).unwrap_err(),
            CreationFailure::InvalidResponse
        );
    }
}

#[test]
fn invalid_thread_id_is_not_acknowledged() {
    for id in [
        json!(""),
        Value::Null,
        json!(27),
        json!({"id":ID}),
        json!("thread-1"),
        json!(format!(" {ID}")),
        json!(ID.replace('-', "")),
        json!("00000000-0000-0000-0000-000000000000"),
    ] {
        let value = json!({"result":{"thread":{"id":id}}});
        assert_eq!(
            acknowledge_thread_start(HOME, None, &value).unwrap_err(),
            CreationFailure::InvalidResponse
        );
    }
}

#[test]
fn conflicting_expected_thread_identity_fails_closed() {
    for key in [
        CodexThreadKey::new("another-home", ID),
        CodexThreadKey::new(HOME, TURN),
    ] {
        assert_eq!(
            acknowledge_thread_start(HOME, Some(&key), &response()).unwrap_err(),
            CreationFailure::IdentityConflict
        );
    }
    let mut value = response();
    value["thread"] = json!({"id": TURN});
    assert_eq!(
        acknowledge_thread_start(HOME, None, &value).unwrap_err(),
        CreationFailure::IdentityConflict
    );
}

#[test]
fn server_error_never_acknowledges_even_with_a_thread_id() {
    let mut value = response();
    value["error"] = json!({"code": -32600, "message":"rejected"});
    assert_eq!(
        acknowledge_thread_start(HOME, None, &value).unwrap_err(),
        CreationFailure::ServerRejected
    );
}

#[test]
fn acknowledgement_does_not_imply_persistence() {
    let mut value = response();
    value["result"]["thread"]["ephemeral"] = json!(false);
    value["result"]["thread"]["path"] = json!("/sessions/not-proof.jsonl");
    let facts = acknowledge_thread_start(HOME, None, &value).unwrap();
    assert_eq!(facts.persistence(), PersistenceState::NotYetConfirmed);
    assert_eq!(facts.first_turn_id(), None);
    assert_eq!(facts.first_turn_outcome(), FirstTurnOutcome::Unknown);
    assert!(!facts.is_standard_persisted_session());
}

fn confirm_rollout(
    facts: &mut CreationAcknowledgement,
    home: &str,
    id: &str,
) -> Result<(), CreationFailure> {
    let line = json!({"timestamp":"2026-09-03T00:00:00Z", "type":"session_meta", "payload":{"id":id,"cwd":"/project"}}).to_string();
    let ParsedRolloutRecord::SessionMeta(meta) = RolloutRecordParser::default()
        .parse_line(&line)
        .unwrap()
        .unwrap()
    else {
        panic!("session meta")
    };
    facts.observe_persisted_session_meta(
        &CodexHomeIdentity {
            normalized_path: "/codex-home".into(),
            identity: home.into(),
        },
        &meta,
    )
}

#[test]
fn persistence_requires_authoritative_evidence() {
    let mut facts = ack();
    assert_eq!(facts.persistence(), PersistenceState::NotYetConfirmed);
    confirm_rollout(&mut facts, HOME, ID).unwrap();
    assert_eq!(facts.persistence(), PersistenceState::PersistenceConfirmed);
    assert_eq!(facts.ephemeral(), EphemeralState::Unknown);
    assert_eq!(facts.first_turn_id(), None);
    assert!(facts.is_standard_persisted_session());
}

#[test]
fn mismatched_persistence_evidence_does_not_change_facts() {
    let mut facts = ack();
    let before = facts.clone();
    assert_eq!(
        confirm_rollout(&mut facts, "other-home", ID),
        Err(CreationFailure::IdentityConflict)
    );
    assert_eq!(
        confirm_rollout(&mut facts, HOME, TURN),
        Err(CreationFailure::IdentityConflict)
    );
    assert_eq!(facts, before);
}

#[test]
fn unknown_ephemeral_state_is_not_guessed() {
    for field in [Value::Null, json!("false"), json!(0), json!({})] {
        let mut value = response();
        value["result"]["thread"]["ephemeral"] = field;
        assert_eq!(
            acknowledge_thread_start(HOME, None, &value)
                .unwrap()
                .ephemeral(),
            EphemeralState::Unknown
        );
    }
    assert_eq!(ack().ephemeral(), EphemeralState::Unknown);
    for (flag, state) in [
        (true, EphemeralState::Ephemeral),
        (false, EphemeralState::NonEphemeral),
    ] {
        let mut value = response();
        value["result"]["thread"]["ephemeral"] = json!(flag);
        assert_eq!(
            acknowledge_thread_start(HOME, None, &value)
                .unwrap()
                .ephemeral(),
            state
        );
    }
}

#[test]
fn confirmed_ephemeral_thread_is_not_standard_persisted_session() {
    let mut value = response();
    value["result"]["thread"]["ephemeral"] = json!(true);
    let mut facts = acknowledge_thread_start(HOME, None, &value).unwrap();
    assert!(!facts.is_standard_persisted_session());
    assert_eq!(
        confirm_rollout(&mut facts, HOME, ID),
        Err(CreationFailure::EvidenceConflict)
    );
    assert_eq!(facts.persistence(), PersistenceState::NotYetConfirmed);
}

#[test]
fn first_turn_failure_keeps_acknowledged_thread_identity() {
    for outcome in [
        FirstTurnOutcome::Failed,
        FirstTurnOutcome::Interrupted,
        FirstTurnOutcome::Completed,
    ] {
        let mut facts = ack();
        let key = CodexTurnKey::new(CodexThreadKey::new(HOME, ID), TURN);
        facts.observe_first_turn_accepted(&key).unwrap();
        facts
            .observe_first_turn_outcome(Some(&key), outcome)
            .unwrap();
        assert_eq!(facts.thread_key(), &CodexThreadKey::new(HOME, ID));
        assert_eq!(facts.first_turn_id(), Some(TURN));
        assert_eq!(facts.first_turn_outcome(), outcome);
        assert_eq!(facts.state(), CreationState::ThreadAcknowledged);
        assert_eq!(facts.persistence(), PersistenceState::NotYetConfirmed);
    }
    let mut facts = ack();
    facts
        .observe_first_turn_outcome(None, FirstTurnOutcome::Rejected)
        .unwrap();
    assert_eq!(facts.first_turn_id(), None);
    assert_eq!(facts.thread_key().thread_id, ID);
    assert_eq!(facts.state(), CreationState::ThreadAcknowledged);
}

#[test]
fn later_or_foreign_turn_cannot_replace_first_turn() {
    let mut facts = ack();
    let key = CodexTurnKey::new(CodexThreadKey::new(HOME, ID), TURN);
    facts.observe_first_turn_accepted(&key).unwrap();
    let before = facts.clone();
    let other = CodexTurnKey::new(key.thread_key.clone(), ID);
    assert!(facts.observe_first_turn_accepted(&other).is_err());
    assert!(facts
        .observe_first_turn_outcome(Some(&other), FirstTurnOutcome::Failed)
        .is_err());
    let foreign = CodexTurnKey::new(CodexThreadKey::new("other-home", ID), TURN);
    assert!(facts.observe_first_turn_accepted(&foreign).is_err());
    assert_eq!(facts, before);
}

#[test]
fn thread_name_is_not_required_for_creation_success() {
    assert_eq!(ack().state(), CreationState::ThreadAcknowledged);
}

#[test]
fn desktop_project_or_sidebar_state_is_not_required_for_creation_success() {
    let mut value = response();
    value["result"]["thread"]["projectId"] = Value::Null;
    value["result"]["thread"]["sidebarVisible"] = json!(false);
    assert_eq!(acknowledge_thread_start(HOME, None, &value).unwrap(), ack());
}

#[test]
fn workspace_entry_id_does_not_enter_canonical_thread_identity() {
    let mut value = response();
    value["result"]["thread"]["workspaceId"] = json!("workspace-projection-only");
    value["result"]["model"] = json!("other-model");
    value["result"]["approvalPolicy"] = json!("never");
    assert_eq!(acknowledge_thread_start(HOME, None, &value).unwrap(), ack());
}

#[tokio::test]
async fn thread_start_does_not_start_first_turn() {
    let mut calls = Vec::new();
    let result = start_thread_with_acknowledgement(HOME, "/project", |method, params| {
        calls.push((method.to_string(), params));
        std::future::ready(Ok(response()))
    })
    .await
    .unwrap();
    assert_eq!(
        calls,
        vec![(
            "thread/start".into(),
            json!({"cwd":"/project", "approvalPolicy":"on-request"})
        )]
    );
    assert_eq!(result["result"]["thread"]["id"], ID);
    assert_eq!(
        result["result"]["creationAcknowledgement"]["state"],
        "THREAD_ACKNOWLEDGED"
    );
    assert_eq!(
        result["result"]["creationAcknowledgement"]["persistence"],
        "NOT_YET_CONFIRMED"
    );
    assert_eq!(
        result["result"]["creationAcknowledgement"]["firstTurnAcceptance"],
        "NOT_YET_ACCEPTED"
    );
}

#[tokio::test]
async fn creation_failure_does_not_fallback_to_second_thread_start() {
    for reply in [
        Ok(json!({"result":{"thread":{"id":"bad"}}})),
        Ok(json!({"error":{"code":-32600}})),
        Err("request timed out".to_string()),
    ] {
        let mut calls = 0;
        let result = start_thread_with_acknowledgement(HOME, "/project", |method, _| {
            assert_eq!(method, "thread/start");
            calls += 1;
            std::future::ready(reply)
        })
        .await;
        assert!(result.is_err());
        assert_eq!(calls, 1);
    }
}

#[test]
fn phase_3_1_exact_id_resume_contract_does_not_regress() {
    let facts = ack();
    let request = super::build_exact_thread_request(
        super::ExactThreadMethod::Resume,
        &facts.thread_key().thread_id,
    )
    .unwrap();
    assert_eq!(request.method, "thread/resume");
    assert_eq!(request.params, json!({"threadId":ID}));
    assert_eq!(
        super::validate_exact_thread_response(ID, response()).unwrap(),
        response()
    );
}
