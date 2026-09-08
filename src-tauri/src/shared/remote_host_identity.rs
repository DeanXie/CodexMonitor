//! Stable identity and routing values for a CodexMonitor remote execution host.
//!
//! The identity is public routing metadata, not an authentication credential.
//! Dynamic availability is intentionally not represented here.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::codex_identity::CodexThreadKey;

pub(crate) const REMOTE_DAEMON_PROTOCOL_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub(crate) struct RemoteHostIdentity(String);

impl<'de> Deserialize<'de> for RemoteHostIdentity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

impl RemoteHostIdentity {
    pub(crate) fn parse(value: impl AsRef<str>) -> Result<Self, String> {
        let value = value.as_ref();
        let parsed = Uuid::parse_str(value)
            .map_err(|_| "remote host identity must be a canonical UUID v4".to_string())?;
        if parsed.is_nil()
            || parsed.get_version_num() != 4
            || parsed.hyphenated().to_string() != value
        {
            return Err(
                "remote host identity must be a canonical lowercase non-nil UUID v4".to_string(),
            );
        }
        Ok(Self(value.to_string()))
    }

    #[cfg(desktop)]
    fn generate() -> Self {
        Self(Uuid::new_v4().hyphenated().to_string())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteThreadLocator {
    pub remote_host_identity: RemoteHostIdentity,
    pub thread_key: CodexThreadKey,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteDaemonCapabilities {
    pub thread_read: bool,
    pub thread_delete: bool,
}

impl Default for RemoteDaemonCapabilities {
    fn default() -> Self {
        Self {
            thread_read: true,
            thread_delete: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteDaemonInfo {
    pub name: String,
    pub remote_host_identity: RemoteHostIdentity,
    pub version: String,
    pub protocol_version: u32,
    pub mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub capabilities: RemoteDaemonCapabilities,
}

pub(crate) fn validate_daemon_info(info: &RemoteDaemonInfo) -> Result<(), String> {
    if info.name != "codex-monitor-daemon" {
        return Err("remote daemon identity response has an unexpected service name".to_string());
    }
    if info.mode != "tcp" {
        return Err("remote daemon identity response has an unexpected mode".to_string());
    }
    if info.protocol_version != REMOTE_DAEMON_PROTOCOL_VERSION {
        return Err(format!(
            "unsupported remote daemon protocol version: {}",
            info.protocol_version
        ));
    }
    Ok(())
}

#[cfg(desktop)]
mod store {
    use super::RemoteHostIdentity;
    use serde::{Deserialize, Serialize};
    use std::fs::{File, OpenOptions};
    use std::io::{ErrorKind, Write};
    use std::path::{Path, PathBuf};
    use std::thread;
    use std::time::Duration;
    use uuid::Uuid;

    const STORE_FILE: &str = "remote-host-identity.json";
    const LOCK_FILE: &str = "remote-host-identity.lock";
    const SCHEMA_VERSION: u32 = 1;
    const LOCK_RETRIES: usize = 100;
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct StoredIdentity {
        schema_version: u32,
        remote_host_identity: String,
    }

    struct InitLock(PathBuf);

    impl Drop for InitLock {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    pub(crate) fn load_or_initialize_remote_host_identity(
        data_dir: &Path,
    ) -> Result<RemoteHostIdentity, String> {
        let store_path = data_dir.join(STORE_FILE);
        match load_existing(&store_path)? {
            Some(identity) => return Ok(identity),
            None => {}
        }

        std::fs::create_dir_all(data_dir).map_err(|error| {
            format!("failed to create daemon data directory for host identity: {error}")
        })?;
        let lock_path = data_dir.join(LOCK_FILE);
        let mut remaining_attempts = LOCK_RETRIES;
        let _lock = loop {
            match try_acquire_init_lock(&lock_path)? {
                Some(lock) => break lock,
                None => {
                    if let Some(identity) = load_existing(&store_path)? {
                        return Ok(identity);
                    }
                    if remaining_attempts == 0 {
                        return Err(
                            "remote host identity initialization is already in progress or was interrupted"
                                .to_string(),
                        );
                    }
                    remaining_attempts -= 1;
                    thread::sleep(Duration::from_millis(10));
                }
            }
        };
        if let Some(identity) = load_existing(&store_path)? {
            return Ok(identity);
        }

        let identity = RemoteHostIdentity::generate();
        write_atomically(&store_path, &identity)?;
        Ok(identity)
    }

    fn load_existing(path: &Path) -> Result<Option<RemoteHostIdentity>, String> {
        let data = match std::fs::read_to_string(path) {
            Ok(data) => data,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(format!(
                    "failed to read remote host identity store {}: {error}",
                    path.display()
                ))
            }
        };
        let stored: StoredIdentity = serde_json::from_str(&data).map_err(|error| {
            format!(
                "remote host identity store {} is malformed: {error}",
                path.display()
            )
        })?;
        if stored.schema_version != SCHEMA_VERSION {
            return Err(format!(
                "unsupported remote host identity schema version: {}",
                stored.schema_version
            ));
        }
        RemoteHostIdentity::parse(stored.remote_host_identity).map(Some)
    }

    fn try_acquire_init_lock(lock_path: &Path) -> Result<Option<InitLock>, String> {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(lock_path)
        {
            Ok(mut file) => {
                if let Err(error) = file
                    .write_all(b"initializing\n")
                    .and_then(|_| file.sync_all())
                {
                    let _ = std::fs::remove_file(lock_path);
                    return Err(format!("failed to initialize host identity lock: {error}"));
                }
                Ok(Some(InitLock(lock_path.to_path_buf())))
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => Ok(None),
            Err(error) => Err(format!(
                "failed to acquire remote host identity lock: {error}"
            )),
        }
    }

    fn write_atomically(path: &Path, identity: &RemoteHostIdentity) -> Result<(), String> {
        let parent = path
            .parent()
            .ok_or_else(|| "remote host identity store has no parent directory".to_string())?;
        let temp = parent.join(format!(".{STORE_FILE}.{}.tmp", Uuid::new_v4()));
        let result = (|| {
            let payload = serde_json::to_vec_pretty(&StoredIdentity {
                schema_version: SCHEMA_VERSION,
                remote_host_identity: identity.as_str().to_string(),
            })
            .map_err(|error| error.to_string())?;
            let mut file = File::create(&temp).map_err(|error| error.to_string())?;
            file.write_all(&payload)
                .map_err(|error| error.to_string())?;
            file.sync_all().map_err(|error| error.to_string())?;
            std::fs::rename(&temp, path).map_err(|error| error.to_string())?;
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .or_else(|error| if cfg!(windows) { Ok(()) } else { Err(error) })
                .map_err(|error| error.to_string())?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        result.map_err(|error: String| format!("failed to persist remote host identity: {error}"))
    }

    #[cfg(test)]
    pub(super) fn store_path(data_dir: &Path) -> PathBuf {
        data_dir.join(STORE_FILE)
    }
}

#[cfg(desktop)]
pub(crate) use store::load_or_initialize_remote_host_identity;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Arc, Barrier};
    use std::thread;

    fn temp_dir(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("codex-monitor-{label}-{}", Uuid::new_v4()))
    }

    #[test]
    fn missing_identity_store_generates_and_persists_once() {
        let dir = temp_dir("host-id-first-run");
        let first = load_or_initialize_remote_host_identity(&dir).unwrap();
        let second = load_or_initialize_remote_host_identity(&dir).unwrap();
        assert_eq!(first, second);
        assert!(store::store_path(&dir).is_file());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn same_data_dir_restart_preserves_identity() {
        let dir = temp_dir("host-id-restart");
        let before = load_or_initialize_remote_host_identity(&dir).unwrap();
        let after = load_or_initialize_remote_host_identity(&dir).unwrap();
        assert_eq!(before, after);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn different_data_dirs_generate_different_identities() {
        let a = temp_dir("host-id-a");
        let b = temp_dir("host-id-b");
        let a_id = load_or_initialize_remote_host_identity(&a).unwrap();
        let b_id = load_or_initialize_remote_host_identity(&b).unwrap();
        assert_ne!(a_id, b_id);
        let _ = fs::remove_dir_all(a);
        let _ = fs::remove_dir_all(b);
    }

    #[test]
    fn malformed_identity_store_fails_closed() {
        let dir = temp_dir("host-id-malformed");
        fs::create_dir_all(&dir).unwrap();
        fs::write(store::store_path(&dir), "not-json").unwrap();
        assert!(load_or_initialize_remote_host_identity(&dir).is_err());
        assert_eq!(
            fs::read_to_string(store::store_path(&dir)).unwrap(),
            "not-json"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn unsupported_schema_fails_closed() {
        let dir = temp_dir("host-id-schema");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            store::store_path(&dir),
            r#"{"schemaVersion":2,"remoteHostIdentity":"6ba7b810-9dad-41d1-80b4-00c04fd430c8"}"#,
        )
        .unwrap();
        assert!(load_or_initialize_remote_host_identity(&dir).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn nil_uuid_fails_closed() {
        let dir = temp_dir("host-id-nil");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            store::store_path(&dir),
            r#"{"schemaVersion":1,"remoteHostIdentity":"00000000-0000-0000-0000-000000000000"}"#,
        )
        .unwrap();
        assert!(load_or_initialize_remote_host_identity(&dir).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn concurrent_initialization_creates_one_identity() {
        let dir = Arc::new(temp_dir("host-id-concurrent"));
        let barrier = Arc::new(Barrier::new(8));
        let handles = (0..8)
            .map(|_| {
                let dir = Arc::clone(&dir);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    load_or_initialize_remote_host_identity(&dir)
                        .expect("concurrent identity initialization")
                })
            })
            .collect::<Vec<_>>();
        let identities = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        assert!(identities.windows(2).all(|pair| pair[0] == pair[1]));
        let _ = fs::remove_dir_all(&*dir);
    }

    #[test]
    fn remote_thread_locator_keeps_codex_thread_key_unchanged() {
        let thread_key = CodexThreadKey::new("codex-home:test", "thread-1");
        let locator = RemoteThreadLocator {
            remote_host_identity: RemoteHostIdentity::parse("6ba7b810-9dad-41d1-80b4-00c04fd430c8")
                .unwrap(),
            thread_key: thread_key.clone(),
        };
        assert_eq!(locator.thread_key, thread_key);
    }

    #[test]
    fn same_thread_key_on_two_hosts_is_distinguishable() {
        let thread_key = CodexThreadKey::new("codex-home:test", "thread-1");
        let a = RemoteThreadLocator {
            remote_host_identity: RemoteHostIdentity::parse("6ba7b810-9dad-41d1-80b4-00c04fd430c8")
                .unwrap(),
            thread_key: thread_key.clone(),
        };
        let b = RemoteThreadLocator {
            remote_host_identity: RemoteHostIdentity::parse("6ba7b811-9dad-41d1-80b4-00c04fd430c8")
                .unwrap(),
            thread_key,
        };
        assert_ne!(a, b);
    }

    #[test]
    fn same_host_multiple_codex_homes_are_distinguishable() {
        let host = RemoteHostIdentity::parse("6ba7b810-9dad-41d1-80b4-00c04fd430c8").unwrap();
        let a = RemoteThreadLocator {
            remote_host_identity: host.clone(),
            thread_key: CodexThreadKey::new("codex-home:a", "thread-1"),
        };
        let b = RemoteThreadLocator {
            remote_host_identity: host,
            thread_key: CodexThreadKey::new("codex-home:b", "thread-1"),
        };
        assert_ne!(a, b);
    }

    #[test]
    fn identity_is_not_auth_token_or_availability() {
        let serialized = serde_json::to_value(
            RemoteHostIdentity::parse("6ba7b810-9dad-41d1-80b4-00c04fd430c8").unwrap(),
        )
        .unwrap();
        assert_eq!(
            serialized,
            serde_json::json!("6ba7b810-9dad-41d1-80b4-00c04fd430c8")
        );
        assert!(serialized.get("token").is_none());
        assert!(serialized.get("online").is_none());
    }

    #[test]
    fn unsupported_protocol_version_fails_closed() {
        let info = RemoteDaemonInfo {
            name: "codex-monitor-daemon".to_string(),
            remote_host_identity: RemoteHostIdentity::parse("6ba7b810-9dad-41d1-80b4-00c04fd430c8")
                .unwrap(),
            version: "1.0.0".to_string(),
            protocol_version: 2,
            mode: "tcp".to_string(),
            display_name: None,
            capabilities: RemoteDaemonCapabilities::default(),
        };
        assert!(validate_daemon_info(&info).is_err());
    }
}
