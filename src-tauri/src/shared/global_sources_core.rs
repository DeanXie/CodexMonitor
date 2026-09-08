#[cfg(desktop)]
pub(crate) mod deletion_tombstone;
#[cfg(desktop)]
pub(crate) mod desktop_metadata;
#[cfg(desktop)]
pub(crate) mod desktop_projection;
#[cfg(desktop)]
pub(crate) mod rollout_checkpoint;
#[cfg(desktop)]
pub(crate) mod rollout_discovery;
#[cfg(desktop)]
pub(crate) mod rollout_identity;
#[cfg(not(desktop))]
pub(crate) mod rollout_identity {
    pub(crate) use crate::shared::codex_identity::{CodexThreadKey, CodexTurnKey};
}
#[cfg(desktop)]
pub(crate) mod rollout_record;
#[cfg(desktop)]
pub(crate) mod rollout_tail;
#[cfg(desktop)]
pub(crate) mod rollout_watch_service;
#[cfg(desktop)]
pub(crate) mod rollout_watcher;
#[cfg(desktop)]
pub(crate) mod runtime_config;
#[cfg(desktop)]
pub(crate) mod source_envelope;
#[cfg(not(desktop))]
pub(crate) mod source_envelope {
    pub(crate) use crate::shared::codex_identity::CodexHomeIdentity;
}
#[cfg(desktop)]
pub(crate) mod source_registry;

#[cfg(all(test, desktop))]
#[path = "global_sources_core/tests.rs"]
mod tests;

#[cfg(all(test, desktop))]
#[path = "global_sources_core/watcher_tests.rs"]
mod watcher_tests;

#[cfg(all(test, desktop))]
#[path = "global_sources_core/deletion_tests.rs"]
mod deletion_tests;

#[cfg(all(test, desktop))]
#[path = "global_sources_core/desktop_projection_tests.rs"]
mod desktop_projection_tests;
