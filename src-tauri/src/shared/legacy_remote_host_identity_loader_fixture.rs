//! Frozen compatibility target copied from `remote_host_identity.rs` at
//! commit ea79bce9d86b1b91c0afea6ff726d94685a07986. Do not evolve this parser
//! with the v2 activation implementation.

use serde::Deserialize;
use std::fs;
use std::path::Path;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredRemoteHostIdentityV1 {
    schema_version: u32,
    remote_host_identity: String,
}

pub(super) fn load_v1(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| format!("read failed: {}", error.kind()))?;
    let stored: StoredRemoteHostIdentityV1 =
        serde_json::from_slice(&bytes).map_err(|_| "invalid JSON".to_string())?;
    if stored.schema_version != 1 {
        return Err("unsupported schema".to_string());
    }
    let parsed = Uuid::parse_str(&stored.remote_host_identity)
        .map_err(|_| "invalid remote host identity".to_string())?;
    if parsed.is_nil() {
        return Err("invalid remote host identity".to_string());
    }
    Ok(parsed.to_string())
}
