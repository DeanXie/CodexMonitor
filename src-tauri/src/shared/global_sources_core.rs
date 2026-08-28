pub(crate) mod rollout_checkpoint;
pub(crate) mod rollout_discovery;
pub(crate) mod rollout_identity;
pub(crate) mod rollout_record;
pub(crate) mod rollout_tail;
pub(crate) mod rollout_watch_service;
pub(crate) mod rollout_watcher;
pub(crate) mod runtime_config;
pub(crate) mod source_envelope;
pub(crate) mod source_registry;

#[cfg(test)]
#[path = "global_sources_core/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "global_sources_core/watcher_tests.rs"]
mod watcher_tests;
