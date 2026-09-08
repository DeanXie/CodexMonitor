use std::collections::HashSet;

use serde_json::json;

use super::codex_identity::{CodexHomeIdentity, CodexThreadKey, CodexTurnKey};
use super::mobile_thread_routing;

#[test]
fn neutral_identity_preserves_serde_equality_and_hash_contract() {
    let home = CodexHomeIdentity {
        normalized_path: r"C:\host\.codex".to_string(),
        identity: "codex-home:host-provided".to_string(),
    };
    let thread = CodexThreadKey::new(
        home.identity.clone(),
        "01a080bd-66b1-7e82-8c02-3759c32c4283",
    );
    let turn = CodexTurnKey::new(thread.clone(), "turn-1");

    assert_eq!(
        serde_json::to_value(&home).unwrap(),
        json!({
            "normalizedPath": r"C:\host\.codex",
            "identity": "codex-home:host-provided"
        })
    );
    assert_eq!(
        serde_json::to_value(&thread).unwrap(),
        json!({
            "codexHomeIdentity": "codex-home:host-provided",
            "threadId": "01a080bd-66b1-7e82-8c02-3759c32c4283"
        })
    );
    assert_eq!(
        serde_json::to_value(&turn).unwrap(),
        json!({
            "threadKey": {
                "codexHomeIdentity": "codex-home:host-provided",
                "threadId": "01a080bd-66b1-7e82-8c02-3759c32c4283"
            },
            "turnId": "turn-1"
        })
    );

    let decoded: CodexTurnKey =
        serde_json::from_value(serde_json::to_value(&turn).unwrap()).unwrap();
    assert_eq!(decoded, turn);
    assert_eq!(HashSet::from([thread.clone(), thread]).len(), 1);

    let _: super::global_sources_core::source_envelope::CodexHomeIdentity = home;
    let _: super::global_sources_core::rollout_identity::CodexThreadKey = decoded.thread_key;
}

#[tokio::test]
async fn mobile_read_routes_only_through_remote_backend() {
    let response =
        mobile_thread_routing::read_thread("workspace-1", "thread-1", |method, params| {
            assert_eq!(method, "read_thread");
            assert_eq!(
                params,
                json!({ "workspaceId": "workspace-1", "threadId": "thread-1" })
            );
            async { Ok(json!({ "thread": { "id": "thread-1" } })) }
        })
        .await
        .unwrap();

    assert_eq!(response["thread"]["id"], "thread-1");
}

#[tokio::test]
async fn mobile_delete_routes_only_through_remote_backend() {
    let response =
        mobile_thread_routing::delete_thread("workspace-1", "thread-1", |method, params| {
            assert_eq!(method, "delete_thread");
            assert_eq!(
                params,
                json!({ "workspaceId": "workspace-1", "threadId": "thread-1" })
            );
            async { Ok(json!({ "ok": true })) }
        })
        .await
        .unwrap();

    assert_eq!(response, json!({ "ok": true }));
}

#[tokio::test]
async fn mobile_remote_unavailable_is_returned_without_local_fallback() {
    let error = mobile_thread_routing::read_thread("workspace-1", "thread-1", |_, _| async {
        Err("remote host unavailable".to_string())
    })
    .await
    .unwrap_err();

    assert_eq!(error, "remote host unavailable");
}
