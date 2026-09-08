pub(crate) mod account;
pub(crate) mod agents_config_core;
pub(crate) mod codex_aux_core;
pub(crate) mod codex_core;
pub(crate) mod codex_identity;
pub(crate) mod codex_update_core;
pub(crate) mod config_toml_core;
#[cfg(desktop)]
pub(crate) mod desktop_projection_handling;
pub(crate) mod execution_settings_evidence;
pub(crate) mod execution_settings_ingestion;
pub(crate) mod files_core;
pub(crate) mod git_core;
pub(crate) mod git_rpc;
pub(crate) mod git_ui_core;
pub(crate) mod global_sources_core;
pub(crate) mod local_usage_core;
#[cfg(any(not(desktop), test))]
pub(crate) mod mobile_thread_routing;
pub(crate) mod process_core;
pub(crate) mod prompts_core;
pub(crate) mod remote_host_identity;
pub(crate) mod settings_core;
pub(crate) mod surface_projection_core;
pub(crate) mod surface_projection_engine;
pub(crate) mod workspace_interop_core;
pub(crate) mod workspace_rpc;
pub(crate) mod workspaces_core;
pub(crate) mod worktree_core;

#[cfg(test)]
#[path = "mobile_compile_boundary_tests.rs"]
mod mobile_compile_boundary_tests;

#[cfg(test)]
#[path = "execution_settings_evidence_tests.rs"]
mod execution_settings_evidence_tests;

#[cfg(test)]
#[path = "execution_settings_ingestion_tests.rs"]
mod execution_settings_ingestion_tests;

#[cfg(test)]
#[path = "execution_settings_acceptance_tests.rs"]
mod execution_settings_acceptance_tests;

#[cfg(test)]
#[path = "surface_projection_core_tests.rs"]
mod surface_projection_core_tests;

#[cfg(test)]
#[path = "surface_projection_engine_tests.rs"]
mod surface_projection_engine_tests;

#[cfg(all(test, desktop))]
#[path = "desktop_projection_handling_tests.rs"]
mod desktop_projection_handling_tests;
