use super::rollout_record::SessionMetaRecord;
use super::source_envelope::CodexHomeIdentity;
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RolloutIdentity {
    pub thread_key: CodexThreadKey,
    pub root_session_id: Option<String>,
    pub parent_thread_key: Option<CodexThreadKey>,
    pub agent_path: Option<String>,
}

pub(crate) fn identity_from_session_meta(
    codex_home: &CodexHomeIdentity,
    meta: &SessionMetaRecord,
) -> RolloutIdentity {
    let parent_thread_key = meta.subagent_spawn.as_ref().map(|spawn| {
        CodexThreadKey::new(codex_home.identity.clone(), spawn.parent_thread_id.clone())
    });
    RolloutIdentity {
        thread_key: CodexThreadKey::new(codex_home.identity.clone(), meta.id.clone()),
        root_session_id: meta.session_id.clone(),
        parent_thread_key,
        agent_path: meta
            .subagent_spawn
            .as_ref()
            .and_then(|spawn| spawn.agent_path.clone()),
    }
}
