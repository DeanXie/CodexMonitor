//! Platform-neutral canonical Codex identity values.
//!
//! Mobile clients may receive, deserialize, compare, and carry these values.
//! Host identity construction from a local `CODEX_HOME` remains in the
//! desktop-only Global Source runtime.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexHomeIdentity {
    pub normalized_path: String,
    pub identity: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexThreadKey {
    pub codex_home_identity: String,
    pub thread_id: String,
}

impl CodexThreadKey {
    pub(crate) fn new(
        codex_home_identity: impl Into<String>,
        thread_id: impl Into<String>,
    ) -> Self {
        Self {
            codex_home_identity: codex_home_identity.into(),
            thread_id: thread_id.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexTurnKey {
    pub thread_key: CodexThreadKey,
    pub turn_id: String,
}

impl CodexTurnKey {
    pub(crate) fn new(thread_key: CodexThreadKey, turn_id: impl Into<String>) -> Self {
        Self {
            thread_key,
            turn_id: turn_id.into(),
        }
    }
}
