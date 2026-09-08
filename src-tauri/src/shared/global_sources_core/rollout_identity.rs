use super::rollout_record::SessionMetaRecord;
use super::source_envelope::CodexHomeIdentity;
pub(crate) use crate::shared::codex_identity::{CodexThreadKey, CodexTurnKey};

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
