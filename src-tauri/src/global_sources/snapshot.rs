use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::shared::global_sources_core::source_registry::CanonicalSourceThread;
use crate::state::AppState;

pub(crate) const GLOBAL_SOURCE_SNAPSHOT_UPDATED_EVENT: &str = "global-source-snapshot-updated";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GlobalSourceSnapshot {
    pub revision: u64,
    pub generated_at_ms: i64,
    pub workspace_codex_home_identities: HashMap<String, String>,
    pub threads: Vec<CanonicalSourceThread>,
}

#[tauri::command]
pub(crate) fn global_source_snapshot(state: State<'_, AppState>) -> GlobalSourceSnapshot {
    state.global_rollout_runtime.snapshot()
}
