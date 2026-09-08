//! Remote-only Thread operations used by mobile adapters.
//!
//! These functions intentionally accept no local app-server, `CODEX_HOME`,
//! Global Source, or deletion-tombstone fallback.

use std::future::Future;

use serde_json::{json, Value};

pub(crate) async fn read_thread<F, Fut>(
    workspace_id: &str,
    thread_id: &str,
    remote_dispatch: F,
) -> Result<Value, String>
where
    F: FnOnce(&'static str, Value) -> Fut,
    Fut: Future<Output = Result<Value, String>>,
{
    remote_dispatch(
        "read_thread",
        json!({ "workspaceId": workspace_id, "threadId": thread_id }),
    )
    .await
}

pub(crate) async fn delete_thread<F, Fut>(
    workspace_id: &str,
    thread_id: &str,
    remote_dispatch: F,
) -> Result<Value, String>
where
    F: FnOnce(&'static str, Value) -> Fut,
    Fut: Future<Output = Result<Value, String>>,
{
    remote_dispatch(
        "delete_thread",
        json!({ "workspaceId": workspace_id, "threadId": thread_id }),
    )
    .await
}
