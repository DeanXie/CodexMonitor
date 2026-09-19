use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

const DAEMON: &str = env!("CARGO_BIN_EXE_codex_monitor_daemon");
const DAEMONCTL: &str = env!("CARGO_BIN_EXE_codex_monitor_daemonctl");

fn temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("codex-monitor-p4-1d3-{label}-{}", Uuid::new_v4()))
}

fn write_json(path: &Path, value: Value) {
    fs::create_dir_all(path.parent().expect("fixture parent")).unwrap();
    fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
}

fn journal_path(root: &Path) -> PathBuf {
    let name = root.file_name().unwrap().to_string_lossy();
    root.parent()
        .unwrap()
        .join(format!(".{name}.codexmonitor-activation-journal.json"))
}

fn write_committed_profile(root: &Path, workspaces: Value, journal_state: &str) -> PathBuf {
    let transaction = format!("tx-{}", Uuid::new_v4());
    fs::create_dir_all(root).unwrap();
    write_json(&root.join("settings.json"), json!({}));
    write_json(&root.join("workspaces.json"), workspaces);
    write_json(
        &root.join("activation-manifest.json"),
        json!({
            "schemaVersion": 1,
            "transactionId": transaction,
            "profileState": "activated"
        }),
    );
    write_json(
        &root.join("remote-host-identity.json"),
        json!({
            "schemaVersion": 2,
            "state": "active",
            "remoteHostIdentity": "014383f2-41f8-4b13-b9d7-30c511e47cec",
            "transactionId": transaction
        }),
    );
    let canonical = fs::canonicalize(root).unwrap();
    let journal = journal_path(root);
    write_json(
        &journal,
        json!({
            "schemaVersion": 1,
            "transactionId": transaction,
            "sourceRoot": "",
            "targetRoot": canonical,
            "stagingRoot": root.parent().unwrap().join("prepared-no-longer-present"),
            "state": journal_state
        }),
    );
    journal
}

fn journal_state(path: &Path) -> String {
    serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap()["state"]
        .as_str()
        .unwrap()
        .to_string()
}

fn free_loopback_addr() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    address.to_string()
}

fn terminate_controlled_child(pid: u64) {
    #[cfg(windows)]
    let status = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status()
        .unwrap();
    #[cfg(unix)]
    let status = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .unwrap();
    assert!(
        status.success(),
        "failed to terminate controlled test child"
    );
}

fn authenticated_daemon_info(listen: &str, token: &str) -> Value {
    let mut stream = TcpStream::connect(listen).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(2)))
        .unwrap();
    let reader_stream = stream.try_clone().unwrap();
    let mut lines = BufReader::new(reader_stream).lines();
    writeln!(
        stream,
        "{}",
        json!({"id": 1, "method": "auth", "params": {"token": token}})
    )
    .unwrap();
    let auth: Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
    assert!(
        auth.get("result").is_some(),
        "auth response was not successful"
    );
    writeln!(
        stream,
        "{}",
        json!({"id": 2, "method": "daemon_info", "params": {}})
    )
    .unwrap();
    let info: Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
    info["result"].clone()
}

#[test]
fn daemonctl_start_waits_for_authenticated_child_readiness() {
    let base = temp_root("daemonctl-ready");
    let root = base.join("profile");
    let journal = write_committed_profile(&root, json!([]), "target_committed");
    let listen = free_loopback_addr();
    let token = "p4-1d3-test-token";

    let started = Command::new(DAEMONCTL)
        .args([
            "start",
            "--listen",
            &listen,
            "--data-dir",
            root.to_str().unwrap(),
            "--daemon-path",
            DAEMON,
            "--token",
            token,
            "--json",
        ])
        .status()
        .unwrap();
    assert!(started.success(), "daemonctl start failed");
    let info = authenticated_daemon_info(&listen, token);
    assert_eq!(info["name"], "codex-monitor-daemon");
    assert_eq!(info["mode"], "tcp");
    let pid = info["pid"].as_u64().expect("ready child pid");
    assert_eq!(journal_state(&journal), "runtime_validated");
    terminate_controlled_child(pid);
    let _ = fs::remove_dir_all(base);
}

#[test]
fn daemonctl_read_only_commands_do_not_advance_activation() {
    let base = temp_root("daemonctl-read-only");
    let root = base.join("profile");
    let journal = write_committed_profile(&root, json!([]), "target_committed");
    let listen = free_loopback_addr();

    for command in ["status", "command-preview"] {
        let mut process = Command::new(DAEMONCTL);
        process.args([
            command,
            "--listen",
            &listen,
            "--data-dir",
            root.to_str().unwrap(),
            "--insecure-no-auth",
            "--json",
        ]);
        if command == "command-preview" {
            process.args(["--daemon-path", DAEMON]);
        }
        let output = process.output().unwrap();
        assert!(
            output.status.success(),
            "{command} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(journal_state(&journal), "target_committed");
    }
    let _ = fs::remove_dir_all(base);
}

#[test]
fn daemon_candidate_failure_never_claims_runtime_success() {
    let base = temp_root("daemon-candidate-failure");
    let root = base.join("profile");
    let journal = write_committed_profile(
        &root,
        json!([{"id": 42, "name": false, "path": [], "kind": "main"}]),
        "target_committed",
    );
    let listen = free_loopback_addr();

    let output = Command::new(DAEMON)
        .args([
            "--listen",
            &listen,
            "--data-dir",
            root.to_str().unwrap(),
            "--insecure-no-auth",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(journal_state(&journal), "target_committed");
    assert!(
        TcpListener::bind(&listen).is_ok(),
        "failed daemon leaked its socket"
    );
    let _ = fs::remove_dir_all(base);
}

#[test]
fn daemon_bind_failure_releases_candidate_resources_and_keeps_gate_closed() {
    let base = temp_root("daemon-bind-failure");
    let root = base.join("profile");
    let journal = write_committed_profile(&root, json!([]), "target_committed");
    let occupied = TcpListener::bind("127.0.0.1:0").unwrap();
    let listen = occupied.local_addr().unwrap().to_string();

    let output = Command::new(DAEMON)
        .args([
            "--listen",
            &listen,
            "--data-dir",
            root.to_str().unwrap(),
            "--insecure-no-auth",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(journal_state(&journal), "target_committed");
    drop(occupied);
    assert!(TcpListener::bind(&listen).is_ok());
    let _ = fs::remove_dir_all(base);
}

#[test]
fn daemonctl_start_does_not_report_success_before_child_is_ready() {
    let base = temp_root("daemonctl-child-not-ready");
    let root = base.join("profile");
    let journal = write_committed_profile(
        &root,
        json!([{"id": 42, "name": false, "path": [], "kind": "main"}]),
        "target_committed",
    );
    let listen = free_loopback_addr();

    let output = Command::new(DAEMONCTL)
        .args([
            "start",
            "--listen",
            &listen,
            "--data-dir",
            root.to_str().unwrap(),
            "--daemon-path",
            DAEMON,
            "--insecure-no-auth",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("before readiness"));
    assert_eq!(journal_state(&journal), "target_committed");
    assert!(TcpListener::bind(&listen).is_ok());
    let _ = fs::remove_dir_all(base);
}

#[test]
fn historical_runtime_record_does_not_skip_current_daemon_validation() {
    let base = temp_root("historical-runtime");
    let root = base.join("profile");
    let journal = write_committed_profile(
        &root,
        json!([{"id": 42, "name": false, "path": [], "kind": "main"}]),
        "runtime_validated",
    );
    let listen = free_loopback_addr();

    let output = Command::new(DAEMON)
        .args([
            "--listen",
            &listen,
            "--data-dir",
            root.to_str().unwrap(),
            "--insecure-no-auth",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(journal_state(&journal), "runtime_validated");
    assert!(TcpListener::bind(&listen).is_ok());
    let _ = fs::remove_dir_all(base);
}
