#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const HOST_IDENTITY_SCHEMA_V2: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HostIdentityV2State {
    Active,
    Retired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct HostIdentityV2Store {
    pub(crate) schema_version: u32,
    pub(crate) state: HostIdentityV2State,
    pub(crate) remote_host_identity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) transaction_id: Option<String>,
}

pub(crate) fn write_v2_identity(
    path: &Path,
    identity: &str,
    state: HostIdentityV2State,
    transaction_id: Option<&str>,
) -> Result<(), String> {
    validate_identity(identity)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("create identity root: {error}"))?;
    }
    let store = HostIdentityV2Store {
        schema_version: HOST_IDENTITY_SCHEMA_V2,
        state,
        remote_host_identity: identity.to_string(),
        transaction_id: transaction_id.map(str::to_string),
    };
    let bytes = serde_json::to_vec_pretty(&store)
        .map_err(|error| format!("serialize identity v2: {error}"))?;
    fs::write(path, bytes).map_err(|error| format!("write identity v2: {error}"))
}

pub(crate) fn load_v2_identity(path: &Path) -> Result<HostIdentityV2Store, String> {
    let bytes = fs::read(path).map_err(|error| format!("read identity v2: {error}"))?;
    let store: HostIdentityV2Store =
        serde_json::from_slice(&bytes).map_err(|error| format!("parse identity v2: {error}"))?;
    if store.schema_version != HOST_IDENTITY_SCHEMA_V2 {
        return Err("unsupported identity schema".to_string());
    }
    validate_identity(&store.remote_host_identity)?;
    if store.state != HostIdentityV2State::Active {
        return Err("remote host identity is retired".to_string());
    }
    Ok(store)
}

fn validate_identity(identity: &str) -> Result<(), String> {
    let parsed = Uuid::parse_str(identity).map_err(|_| "invalid remote host identity")?;
    if parsed.is_nil() {
        return Err("invalid remote host identity".to_string());
    }
    Ok(())
}

#[cfg(windows)]
pub(crate) struct ProtectedLegacyIdentity {
    path: PathBuf,
    _guard: fs::File,
}

#[cfg(windows)]
impl ProtectedLegacyIdentity {
    pub(crate) fn acquire(path: &Path) -> io::Result<Self> {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_DELETE;

        let guard = fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_DELETE)
            .open(path)?;
        Ok(Self {
            path: path.to_path_buf(),
            _guard: guard,
        })
    }

    pub(crate) fn replace_with(&self, replacement: &Path) -> io::Result<()> {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;

        fn wide(path: &Path) -> Vec<u16> {
            path.as_os_str().encode_wide().chain(Some(0)).collect()
        }

        let replaced = wide(&self.path);
        let replacement = wide(replacement);
        let result = unsafe {
            ReplaceFileW(
                replaced.as_ptr(),
                replacement.as_ptr(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if result == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

#[cfg(not(windows))]
pub(crate) struct ProtectedLegacyIdentity;

#[cfg(not(windows))]
impl ProtectedLegacyIdentity {
    pub(crate) fn acquire(_path: &Path) -> io::Result<Self> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "legacy identity retirement is Windows-gated",
        ))
    }
}
