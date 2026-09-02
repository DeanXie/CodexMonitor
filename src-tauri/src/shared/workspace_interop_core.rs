#![allow(dead_code, unused_imports)]

mod locator;
mod resolver;
mod value_types;

pub(crate) use locator::{NormalizedRootLocator, RootLocatorPlatform};
pub(crate) use resolver::{
    resolve_workspace_root, ConfiguredWorkspaceRoot, PhysicalIdentityStatus, WorkspaceResolution,
    WorkspaceResolutionInput, WorkspaceResolutionState,
};
pub(crate) use value_types::{ExecutionEnvironmentKey, WorkspaceKey};

#[cfg(test)]
#[path = "workspace_interop_core/tests.rs"]
mod tests;
