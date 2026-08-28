use super::source_envelope::{CodexHomeIdentity, SourceFileIdentity};
use sha2::{Digest, Sha256};
use std::fs::{self, Metadata};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CodexHomeSource {
    pub codex_home: CodexHomeIdentity,
    pub root: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiscoveredRolloutSource {
    pub codex_home: CodexHomeIdentity,
    pub path: PathBuf,
    pub file_identity: SourceFileIdentity,
}

pub(crate) fn discover_rollout_sources(
    homes: &[CodexHomeSource],
) -> io::Result<Vec<DiscoveredRolloutSource>> {
    let mut sources = Vec::new();
    for home in homes {
        let sessions = home.root.join("sessions");
        if !sessions.exists() {
            continue;
        }
        visit_directory(&sessions, &mut |path, metadata| {
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                return Ok(());
            };
            if !name.starts_with("rollout-") || !name.ends_with(".jsonl") {
                return Ok(());
            }
            let normalized_path = normalize_path(path);
            let filesystem_id = stable_filesystem_id(path, metadata);
            let generation = initial_generation(
                &home.codex_home.identity,
                &normalized_path,
                filesystem_id.as_deref(),
            );
            sources.push(DiscoveredRolloutSource {
                codex_home: home.codex_home.clone(),
                path: path.to_path_buf(),
                file_identity: SourceFileIdentity {
                    normalized_path,
                    filesystem_id,
                    generation,
                    session_meta_id: None,
                },
            });
            Ok(())
        })?;
    }
    sources.sort_by(|left, right| {
        left.codex_home
            .identity
            .cmp(&right.codex_home.identity)
            .then_with(|| {
                left.file_identity
                    .normalized_path
                    .cmp(&right.file_identity.normalized_path)
            })
    });
    sources.dedup_by(|left, right| {
        left.codex_home.identity == right.codex_home.identity
            && left.file_identity.normalized_path == right.file_identity.normalized_path
    });
    Ok(sources)
}

fn visit_directory(
    directory: &Path,
    visitor: &mut impl FnMut(&Path, &Metadata) -> io::Result<()>,
) -> io::Result<()> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            visit_directory(&entry.path(), visitor)?;
        } else if file_type.is_file() {
            let metadata = entry.metadata()?;
            visitor(&entry.path(), &metadata)?;
        }
    }
    Ok(())
}

fn normalize_path(path: &Path) -> String {
    let value = path.to_string_lossy().replace('/', "\\");
    if cfg!(windows) {
        value.to_lowercase()
    } else {
        value
    }
}

fn initial_generation(home_identity: &str, path: &str, filesystem_id: Option<&str>) -> String {
    let mut digest = Sha256::new();
    digest.update(home_identity.as_bytes());
    digest.update([0]);
    digest.update(path.as_bytes());
    digest.update([0]);
    digest.update(
        filesystem_id
            .unwrap_or("filesystem-id-unavailable")
            .as_bytes(),
    );
    format!("file:{:x}", digest.finalize())
}

#[cfg(windows)]
fn stable_filesystem_id(_path: &Path, metadata: &Metadata) -> Option<String> {
    fallback_filesystem_id(metadata).map(|identity| format!("win:{identity}"))
}

#[cfg(unix)]
fn stable_filesystem_id(_path: &Path, metadata: &Metadata) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    Some(format!(
        "unix:{:016x}:{:016x}",
        metadata.dev(),
        metadata.ino()
    ))
}

#[cfg(not(any(windows, unix)))]
fn stable_filesystem_id(_path: &Path, metadata: &Metadata) -> Option<String> {
    fallback_filesystem_id(metadata)
}

fn fallback_filesystem_id(metadata: &Metadata) -> Option<String> {
    let created = metadata
        .created()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    Some(format!(
        "created:{}:{}",
        created.as_secs(),
        created.subsec_nanos()
    ))
}
