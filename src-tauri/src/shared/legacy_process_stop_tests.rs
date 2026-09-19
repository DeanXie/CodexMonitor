#![cfg(windows)]

use super::legacy_migration_entry::{
    LegacyProcessStopProvider, StopEvidenceAcquisition, StopEvidenceState,
};
use super::legacy_process_stop::WindowsLegacyProcessStopProvider;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use uuid::Uuid;

fn fixture() -> (PathBuf, PathBuf) {
    let root =
        std::env::temp_dir().join(format!("codex-monitor-p4-1d4b-native-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let identity = root.join("remote-host-identity.json");
    fs::write(
        &identity,
        serde_json::to_vec(&json!({"remoteHostIdentity": Uuid::new_v4().to_string()})).unwrap(),
    )
    .unwrap();
    (root, identity)
}

#[test]
#[ignore]
fn controlled_stop_child() {
    let identity = PathBuf::from(std::env::var_os("P4_1D4B_IDENTITY").unwrap());
    let _bytes = fs::read(identity).unwrap();
    thread::sleep(Duration::from_secs(30));
}

#[test]
fn cached_identity_then_closed_handle_still_reports_running_until_process_exits() {
    let (root, identity) = fixture();
    let helper = std::env::current_exe().unwrap();
    let mut child = Command::new(&helper)
        .args([
            "--ignored",
            "--exact",
            "shared::legacy_process_stop_tests::controlled_stop_child",
        ])
        .env("P4_1D4B_IDENTITY", &identity)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    thread::sleep(Duration::from_millis(400));
    let provider = WindowsLegacyProcessStopProvider::new(helper, std::process::id()).unwrap();

    assert!(matches!(
        provider.acquire(&root).unwrap(),
        StopEvidenceAcquisition::Blocked(StopEvidenceState::Running)
    ));

    child.kill().unwrap();
    child.wait().unwrap();
    let acquired = provider.acquire(&root).unwrap();
    let StopEvidenceAcquisition::Verified(mut guard) = acquired else {
        panic!("stopped controlled process must produce verified quiescence")
    };
    assert_eq!(
        guard.revalidate(&root).unwrap(),
        StopEvidenceState::VerifiedQuiescentWithinSupportedScope
    );
}

#[test]
fn missing_expected_executable_is_unknown_not_quiescent() {
    let (root, _) = fixture();
    let provider = WindowsLegacyProcessStopProvider::new(
        root.join("missing-codex-monitor.exe"),
        std::process::id(),
    )
    .unwrap();
    assert!(matches!(
        provider.acquire(&root).unwrap(),
        StopEvidenceAcquisition::Blocked(StopEvidenceState::Unknown)
    ));
}

#[test]
fn verified_guard_rejects_source_root_rebinding() {
    let (root, _) = fixture();
    let other =
        std::env::temp_dir().join(format!("codex-monitor-p4-1d4b-other-{}", Uuid::new_v4()));
    fs::create_dir_all(&other).unwrap();
    let inactive_executable = root.join("inactive-legacy-loader.exe");
    fs::copy(std::env::current_exe().unwrap(), &inactive_executable).unwrap();
    let provider =
        WindowsLegacyProcessStopProvider::new(inactive_executable, std::process::id()).unwrap();
    let StopEvidenceAcquisition::Verified(mut guard) = provider.acquire(&root).unwrap() else {
        panic!("excluded current process should permit a verified test guard")
    };

    assert_eq!(
        guard.revalidate(&other).unwrap(),
        StopEvidenceState::Unknown
    );

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(other);
}
