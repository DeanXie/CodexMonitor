use super::rollout_discovery::CodexHomeSource;
use super::source_envelope::CodexHomeIdentity;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GlobalSourceRuntimePaths {
    pub checkpoint_path: PathBuf,
    pub deletion_tombstones_path: PathBuf,
    pub diagnostics_path: PathBuf,
}

impl GlobalSourceRuntimePaths {
    pub(crate) fn new(app_data_root: &Path) -> Self {
        let root = app_data_root.join("global-sources");
        Self {
            checkpoint_path: root.join("rollout-watcher-checkpoint.json"),
            deletion_tombstones_path: root.join("deletion-tombstones.json"),
            diagnostics_path: root.join("rollout-watcher-diagnostics.jsonl"),
        }
    }
}

pub(crate) fn discover_runtime_codex_homes(
    default_home: Option<PathBuf>,
    workspace_homes: impl IntoIterator<Item = PathBuf>,
) -> Vec<CodexHomeSource> {
    let mut seen = HashSet::new();
    let mut homes = default_home
        .into_iter()
        .chain(workspace_homes)
        .filter_map(|path| {
            let normalized_path = normalize_path(&path);
            let key = if cfg!(windows) {
                normalized_path.to_lowercase()
            } else {
                normalized_path.clone()
            };
            if !seen.insert(key) {
                return None;
            }
            Some(CodexHomeSource {
                codex_home: CodexHomeIdentity {
                    identity: codex_home_identity(&normalized_path),
                    normalized_path,
                },
                root: path,
            })
        })
        .collect::<Vec<_>>();
    homes.sort_by(|left, right| left.codex_home.identity.cmp(&right.codex_home.identity));
    homes
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\")
}

fn codex_home_identity(normalized_path: &str) -> String {
    let normalized = if cfg!(windows) {
        normalized_path.to_lowercase()
    } else {
        normalized_path.to_string()
    };
    let digest = Sha256::digest(normalized.as_bytes());
    format!("codex-home:{digest:x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn runtime_codex_homes_are_stable_deduplicated_and_support_multiple_roots() {
        let default = PathBuf::from(r"C:\Users\DeanX\.codex");
        let workspace_home = PathBuf::from(r"D:\codex-homes\workspace-a");

        let first =
            discover_runtime_codex_homes(Some(default.clone()), [default, workspace_home.clone()]);
        let second = discover_runtime_codex_homes(None, [workspace_home]);

        assert_eq!(first.len(), 2);
        assert_eq!(second.len(), 1);
        let first_workspace = first
            .iter()
            .find(|home| home.root == second[0].root)
            .expect("workspace-specific home should be present");
        assert_eq!(first_workspace.codex_home, second[0].codex_home);
        assert!(!first_workspace.codex_home.identity.contains("workspace-a"));
    }

    #[test]
    fn runtime_paths_live_under_stable_global_sources_app_data_directory() {
        let root = PathBuf::from(r"C:\app-data\com.dimillian.codexmonitor");

        let paths = GlobalSourceRuntimePaths::new(&root);

        assert_eq!(
            paths.checkpoint_path,
            root.join("global-sources")
                .join("rollout-watcher-checkpoint.json")
        );
        assert_eq!(
            paths.deletion_tombstones_path,
            root.join("global-sources").join("deletion-tombstones.json")
        );
        assert_eq!(
            paths.diagnostics_path,
            root.join("global-sources")
                .join("rollout-watcher-diagnostics.jsonl")
        );
    }
}
