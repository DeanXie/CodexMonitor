use super::deletion_tombstone::{
    DeletionReconciliationState, DeletionTombstone, DeletionTombstoneDocument,
    DeletionTombstoneStore,
};
use super::rollout_checkpoint::{
    RolloutReplayGuardState, RolloutSourceCheckpoint, RolloutWatcherCheckpoint,
};
use super::rollout_discovery::{discover_rollout_sources, CodexHomeSource};
use super::rollout_identity::CodexThreadKey;
use super::rollout_tail::{read_rollout_delta, RolloutDelta, RolloutTailState};
use super::rollout_watcher::{
    RolloutDeltaReader, RolloutTailWatcher, RolloutWatcherConfig, WatcherRetryPolicy,
};
use super::source_envelope::{
    CodexHomeIdentity, FreshnessState, SourceFileIdentity, SourceKind, SourceTemporalClass,
};
use super::source_registry::{ExternalLifecycle, SourceLaneUpdate, TokenSnapshot};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use uuid::Uuid;

fn legacy_completed_fixture() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository root")
        .join("docs")
        .join("fixtures")
        .join("cli-rollout")
        .join("legacy-completed-sessions.json");
    serde_json::from_slice(&fs::read(path).expect("legacy fixture")).expect("valid fixture")
}

fn desktop_compacted_child_fixture_lines() -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository root")
        .join("docs")
        .join("fixtures")
        .join("desktop-rollout")
        .join("desktop-subagent-compacted-prefix.jsonl");
    fs::read_to_string(path)
        .expect("desktop compacted child fixture")
        .lines()
        .map(str::to_string)
        .collect()
}

fn temp_dir() -> PathBuf {
    let path = std::env::temp_dir().join(format!("codex-monitor-watcher-{}", Uuid::new_v4()));
    fs::create_dir_all(&path).expect("create test directory");
    path
}

fn home(root: &Path, identity: &str) -> CodexHomeSource {
    CodexHomeSource {
        codex_home: CodexHomeIdentity {
            normalized_path: root.to_string_lossy().into_owned(),
            identity: identity.to_string(),
        },
        root: root.to_path_buf(),
    }
}

fn rollout_path(root: &Path, name: &str) -> PathBuf {
    let directory = root.join("sessions").join("2026").join("08").join("25");
    fs::create_dir_all(&directory).expect("session directory");
    directory.join(format!("rollout-{name}.jsonl"))
}

fn session_meta(thread_id: &str, cwd: &str) -> String {
    serde_json::json!({
        "timestamp": "2026-08-25T13:57:09.608Z",
        "type": "session_meta",
        "payload": {
            "session_id": thread_id,
            "id": thread_id,
            "cwd": cwd,
            "cli_version": "0.147.0",
            "source": "exec",
            "thread_source": "user",
            "model_provider": "openai"
        }
    })
    .to_string()
}

fn subagent_session_meta(thread_id: &str, parent_thread_id: &str, agent_path: &str) -> String {
    serde_json::json!({
        "timestamp": "2026-08-25T13:57:09.608Z",
        "type": "session_meta",
        "payload": {
            "session_id": parent_thread_id,
            "id": thread_id,
            "cwd": "C:\\fixture",
            "cli_version": "0.147.0",
            "source": { "subagent": { "thread_spawn": {
                "parent_thread_id": parent_thread_id,
                "depth": 1,
                "agent_path": agent_path,
                "agent_nickname": "fixture",
                "agent_role": "explorer"
            }}},
            "thread_source": "subagent",
            "model_provider": "openai"
        }
    })
    .to_string()
}

fn task_started(turn_id: &str) -> String {
    task_started_at(turn_id, "2026-08-25T13:57:10.000Z", 1787666230)
}

fn thread_settings_applied() -> String {
    serde_json::json!({
        "timestamp": "2026-08-25T13:57:10.500Z",
        "type": "event_msg",
        "payload": {"type": "thread_settings_applied"}
    })
    .to_string()
}

fn task_started_at(turn_id: &str, timestamp: &str, started_at: i64) -> String {
    serde_json::json!({
        "timestamp": timestamp,
        "type": "event_msg",
        "payload": {"type": "task_started", "turn_id": turn_id, "started_at": started_at}
    })
    .to_string()
}

fn turn_context(turn_id: &str, model: &str) -> String {
    serde_json::json!({
        "timestamp": "2026-08-25T13:57:11.000Z",
        "type": "turn_context",
        "payload": {"turn_id": turn_id, "cwd": "C:\\fixture", "model": model, "effort": "medium"}
    })
    .to_string()
}

fn token_count(total: u64) -> String {
    let usage = serde_json::json!({
        "input_tokens": total - 10,
        "cached_input_tokens": 0,
        "cache_write_input_tokens": 0,
        "output_tokens": 10,
        "reasoning_output_tokens": 0,
        "total_tokens": total
    });
    serde_json::json!({
        "timestamp": "2026-08-25T13:57:12.000Z",
        "type": "event_msg",
        "payload": {"type": "token_count", "info": {
            "total_token_usage": usage,
            "last_token_usage": usage
        }}
    })
    .to_string()
}

fn task_complete(turn_id: &str) -> String {
    serde_json::json!({
        "timestamp": "2026-08-25T13:57:13.000Z",
        "type": "event_msg",
        "payload": {
            "type": "task_complete",
            "turn_id": turn_id,
            "started_at": 1787666230i64,
            "completed_at": 1787666233i64,
            "duration_ms": 3000
        }
    })
    .to_string()
}

fn write_lines(path: &Path, lines: &[String]) {
    let content = lines
        .iter()
        .map(|line| format!("{line}\n"))
        .collect::<String>();
    fs::write(path, content.as_bytes()).expect("write rollout");
}

fn append_lines(path: &Path, lines: &[String]) {
    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .expect("append rollout");
    for line in lines {
        writeln!(file, "{line}").expect("append record");
    }
}

fn config(root: &Path, identity: &str, checkpoint: &Path) -> RolloutWatcherConfig {
    RolloutWatcherConfig {
        homes: vec![home(root, identity)],
        checkpoint_path: checkpoint.to_path_buf(),
        deletion_tombstones_path: checkpoint.with_file_name("deletion-tombstones.json"),
        retry: WatcherRetryPolicy {
            max_attempts: 3,
            initial_backoff_ms: 0,
        },
        fresh_window_ms: 5_000,
        settled_after_ms: 10_000,
        reconciliation_interval_ms: 1_000,
    }
}

#[test]
fn desktop_compacted_child_pins_file_owner_and_publishes_only_child_execution() {
    let root = temp_dir();
    let checkpoint = root.join("checkpoint.json");
    let path = rollout_path(&root, "desktop-compacted-child");
    write_lines(&path, &desktop_compacted_child_fixture_lines());
    let mut watcher = RolloutTailWatcher::new(config(&root, "desktop-home", &checkpoint));

    watcher
        .reconcile(1_787_793_004_000)
        .expect("reconcile fixture");

    let snapshot = watcher.registry().snapshot();
    assert_eq!(snapshot.threads.len(), 1);
    let child = &snapshot.threads[0];
    assert_eq!(child.key.thread_id, "thread-child-0001");
    assert_eq!(
        child
            .parent_thread_key
            .as_ref()
            .map(|value| value.value.thread_id.as_str()),
        Some("thread-main-0001")
    );
    assert_eq!(
        child.agent_path.as_ref().map(|value| value.value.as_str()),
        Some("/root/desktop_probe")
    );
    assert_eq!(
        child
            .current_turn
            .as_ref()
            .map(|turn| turn.key.turn_id.as_str()),
        Some("turn-child-0001")
    );
    assert_eq!(
        child.lifecycle.as_ref().map(|value| value.value),
        Some(ExternalLifecycle::Completed)
    );
    assert_eq!(
        child
            .observed_model
            .as_ref()
            .map(|value| value.value.as_str()),
        Some("gpt-observed-child")
    );
    assert_eq!(
        child
            .token_snapshot
            .as_ref()
            .map(|value| value.value.total_tokens),
        Some(1_680)
    );
    assert_eq!(
        watcher
            .source_file_for_path(&path)
            .and_then(|source| source.session_meta_id.as_deref()),
        Some("thread-child-0001")
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn main_rollout_treats_thread_settings_applied_as_internal_evidence() {
    let root = temp_dir();
    let checkpoint = root.join("checkpoint.json");
    let path = rollout_path(&root, "main-with-thread-settings");
    write_lines(
        &path,
        &[
            session_meta("thread-main-settings", r"C:\fixture"),
            thread_settings_applied(),
            task_started("turn-main-settings"),
            turn_context("turn-main-settings", "gpt-main-settings"),
            token_count(420),
            task_complete("turn-main-settings"),
        ],
    );
    let mut watcher = RolloutTailWatcher::new(config(&root, "desktop-home", &checkpoint));

    watcher
        .reconcile(1_787_793_004_000)
        .expect("main rollout boundary marker must not panic");

    let snapshot = watcher.registry().snapshot();
    assert_eq!(snapshot.threads.len(), 1);
    let main = &snapshot.threads[0];
    assert_eq!(main.key.thread_id, "thread-main-settings");
    assert!(main.parent_thread_key.is_none());
    assert_eq!(
        main.lifecycle.as_ref().map(|value| value.value),
        Some(ExternalLifecycle::Completed)
    );
    assert_eq!(
        main.observed_model
            .as_ref()
            .map(|value| value.value.as_str()),
        Some("gpt-main-settings")
    );
    assert_eq!(
        main.token_snapshot
            .as_ref()
            .map(|value| value.value.total_tokens),
        Some(420)
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn desktop_compacted_child_replay_prefix_stays_non_authoritative_while_pending() {
    let root = temp_dir();
    let checkpoint = root.join("checkpoint.json");
    let path = rollout_path(&root, "desktop-compacted-prefix");
    let lines = desktop_compacted_child_fixture_lines();
    write_lines(&path, &lines[..6]);
    let mut watcher = RolloutTailWatcher::new(config(&root, "desktop-home", &checkpoint));

    watcher
        .reconcile(1_787_793_001_000)
        .expect("reconcile prefix");

    let snapshot = watcher.registry().snapshot();
    assert_eq!(snapshot.threads.len(), 1);
    let child = &snapshot.threads[0];
    assert_eq!(child.key.thread_id, "thread-child-0001");
    assert_eq!(
        child
            .parent_thread_key
            .as_ref()
            .map(|value| value.value.thread_id.as_str()),
        Some("thread-main-0001")
    );
    assert_eq!(
        child.agent_path.as_ref().map(|value| value.value.as_str()),
        Some("/root/desktop_probe")
    );
    assert!(child.current_turn.is_none());
    assert!(child.lifecycle.is_none());
    assert!(child.observed_model.is_none());
    assert!(child.token_snapshot.is_none());
    assert_eq!(
        watcher
            .source_file_for_path(&path)
            .and_then(|source| source.session_meta_id.as_deref()),
        Some("thread-child-0001")
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn desktop_compacted_child_requires_complete_boundary_evidence() {
    let root = temp_dir();
    let checkpoint = root.join("checkpoint.json");
    let path = rollout_path(&root, "desktop-ambiguous-boundary");
    let lines = desktop_compacted_child_fixture_lines();
    let without_marker = lines
        .into_iter()
        .enumerate()
        .filter_map(|(index, line)| (index != 5).then_some(line))
        .collect::<Vec<_>>();
    write_lines(&path, &without_marker);
    let mut watcher = RolloutTailWatcher::new(config(&root, "desktop-home", &checkpoint));

    watcher
        .reconcile(1_787_793_004_000)
        .expect("reconcile ambiguous fixture");

    let snapshot = watcher.registry().snapshot();
    assert_eq!(snapshot.threads.len(), 1);
    let child = &snapshot.threads[0];
    assert_eq!(child.key.thread_id, "thread-child-0001");
    assert!(child.current_turn.is_none());
    assert!(child.lifecycle.is_none());
    assert!(child.observed_model.is_none());
    assert!(child.token_snapshot.is_none());
    fs::remove_dir_all(root).ok();
}

#[test]
fn desktop_compacted_child_checkpoint_restores_pending_guard_before_boundary() {
    let root = temp_dir();
    let checkpoint = root.join("checkpoint.json");
    let path = rollout_path(&root, "desktop-checkpoint-boundary");
    let lines = desktop_compacted_child_fixture_lines();
    write_lines(&path, &lines[..6]);
    let watcher_config = config(&root, "desktop-home", &checkpoint);
    let mut initial = RolloutTailWatcher::new(watcher_config.clone());
    initial.reconcile(1_787_793_001_000).expect("prefix");
    drop(initial);

    let saved: RolloutWatcherCheckpoint =
        serde_json::from_slice(&fs::read(&checkpoint).expect("checkpoint"))
            .expect("valid checkpoint");
    assert!(matches!(
        &saved.sources[0].adapter.replay_guard,
        RolloutReplayGuardState::AwaitingChildBoundary {
            replay_parent_thread_id,
            replay_parent_identity_seen: true,
            boundary_marker_seen: true,
            replay_turn_ids,
        } if replay_parent_thread_id == "thread-main-0001"
            && replay_turn_ids.contains("turn-parent-inherited")
    ));

    append_lines(&path, &lines[6..]);
    let mut restarted = RolloutTailWatcher::new(watcher_config);
    restarted
        .reconcile(1_787_793_004_000)
        .expect("resume after boundary");
    let snapshot = restarted.registry().snapshot();
    assert_eq!(snapshot.threads.len(), 1);
    let child = &snapshot.threads[0];
    assert_eq!(child.key.thread_id, "thread-child-0001");
    assert_eq!(
        child.lifecycle.as_ref().map(|value| value.value),
        Some(ExternalLifecycle::Completed)
    );
    assert_eq!(
        child
            .observed_model
            .as_ref()
            .map(|value| value.value.as_str()),
        Some("gpt-observed-child")
    );
    assert_eq!(
        child
            .token_snapshot
            .as_ref()
            .map(|value| value.value.total_tokens),
        Some(1_680)
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn desktop_compacted_child_checkpoint_restores_identity_without_new_records() {
    let root = temp_dir();
    let checkpoint = root.join("checkpoint.json");
    let path = rollout_path(&root, "desktop-checkpoint-identity");
    write_lines(&path, &desktop_compacted_child_fixture_lines());
    let watcher_config = config(&root, "desktop-home", &checkpoint);
    let mut initial = RolloutTailWatcher::new(watcher_config.clone());
    initial
        .reconcile(1_787_793_004_000)
        .expect("initial child observation");
    drop(initial);

    let mut restarted = RolloutTailWatcher::new(watcher_config);
    restarted
        .reconcile(1_787_793_014_000)
        .expect("checkpoint-only reconstruction");

    let snapshot = restarted.registry().snapshot();
    assert_eq!(snapshot.threads.len(), 1);
    let child = &snapshot.threads[0];
    assert_eq!(child.key.thread_id, "thread-child-0001");
    assert_eq!(
        child
            .parent_thread_key
            .as_ref()
            .map(|value| value.value.thread_id.as_str()),
        Some("thread-main-0001")
    );
    assert_eq!(
        child.agent_path.as_ref().map(|value| value.value.as_str()),
        Some("/root/desktop_probe")
    );
    assert_eq!(
        child.lifecycle.as_ref().map(|value| value.value),
        Some(ExternalLifecycle::Completed)
    );
    assert_eq!(
        child
            .observed_model
            .as_ref()
            .map(|value| value.value.as_str()),
        Some("gpt-observed-child")
    );
    assert_eq!(
        child
            .token_snapshot
            .as_ref()
            .map(|value| value.value.total_tokens),
        Some(1_680)
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn desktop_compacted_child_duplicate_replay_prefix_is_idempotent() {
    let root = temp_dir();
    let checkpoint = root.join("checkpoint.json");
    let path = rollout_path(&root, "desktop-duplicate-prefix");
    let lines = desktop_compacted_child_fixture_lines();
    let mut duplicated = lines[..4].to_vec();
    duplicated.extend_from_slice(&lines[1..4]);
    duplicated.extend_from_slice(&lines[4..]);
    write_lines(&path, &duplicated);
    let mut watcher = RolloutTailWatcher::new(config(&root, "desktop-home", &checkpoint));

    watcher
        .reconcile(1_787_793_004_000)
        .expect("reconcile duplicate prefix");

    let snapshot = watcher.registry().snapshot();
    assert_eq!(snapshot.threads.len(), 1);
    let child = &snapshot.threads[0];
    assert_eq!(child.key.thread_id, "thread-child-0001");
    assert_eq!(
        child
            .current_turn
            .as_ref()
            .map(|turn| turn.key.turn_id.as_str()),
        Some("turn-child-0001")
    );
    assert_eq!(
        child
            .token_snapshot
            .as_ref()
            .map(|value| value.value.total_tokens),
        Some(1_680)
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn desktop_compacted_child_generation_reset_accepts_a_new_file_owner() {
    let root = temp_dir();
    let checkpoint = root.join("checkpoint.json");
    let path = rollout_path(&root, "desktop-generation-reset");
    write_lines(&path, &desktop_compacted_child_fixture_lines());
    let mut watcher = RolloutTailWatcher::new(config(&root, "desktop-home", &checkpoint));
    watcher.reconcile(1_787_793_004_000).expect("initial child");
    let old_generation = watcher
        .source_file_for_path(&path)
        .expect("source")
        .generation
        .clone();

    write_lines(&path, &[session_meta("thread-new-owner", "C:\\fixture")]);
    watcher
        .reconcile(1_787_793_005_000)
        .expect("generation reset");

    let source = watcher.source_file_for_path(&path).expect("reset source");
    assert_ne!(source.generation, old_generation);
    assert_eq!(source.session_meta_id.as_deref(), Some("thread-new-owner"));
    assert!(watcher
        .registry()
        .snapshot()
        .threads
        .iter()
        .any(|thread| thread.key.thread_id == "thread-new-owner"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn confirmed_deletion_retires_root_descendant_checkpoint_and_stale_missing_path() {
    let root = temp_dir();
    let checkpoint = root.join("checkpoint.json");
    let root_path = rollout_path(&root, "delete-root");
    let child_path = rollout_path(&root, "delete-child");
    let unrelated_path = rollout_path(&root, "unrelated");
    write_lines(&root_path, &[session_meta("thread-root", "C:\\fixture")]);
    write_lines(
        &child_path,
        &[subagent_session_meta(
            "thread-child",
            "thread-root",
            "/root/child",
        )],
    );
    write_lines(
        &unrelated_path,
        &[session_meta("thread-other", "C:\\fixture")],
    );
    let mut watcher = RolloutTailWatcher::new(config(&root, "home-a", &checkpoint));
    watcher.reconcile(1_000).expect("initial reconcile");

    let child_key = CodexThreadKey::new("home-a", "thread-child");
    assert!(!watcher
        .record_thread_deleted_confirmation(&child_key)
        .expect("pre-response confirmation"));
    assert!(!watcher
        .record_thread_deleted_confirmation(&child_key)
        .expect("duplicate pre-response confirmation"));

    fs::remove_file(&root_path).expect("remove root rollout");
    fs::remove_file(&child_path).expect("remove child rollout");
    let report = watcher
        .reconcile_deletion(DeletionTombstone::confirmed(
            "15e1cc50-9afd-4fbb-a8a7-8df5f5657407",
            CodexThreadKey::new("home-a", "thread-root"),
            vec![CodexThreadKey::new("home-a", "thread-child")],
            2_000,
        ))
        .expect("reconcile deletion");
    assert_eq!(report.registry_retirement_count, 2);
    assert_eq!(report.watcher_source_retirement_count, 2);
    assert_eq!(
        watcher
            .registry()
            .snapshot()
            .threads
            .iter()
            .map(|thread| thread.key.thread_id.as_str())
            .collect::<Vec<_>>(),
        vec!["thread-other"]
    );

    watcher.record_filesystem_signal([root_path.clone(), child_path.clone()], 2_100);
    let first = watcher.reconcile(2_200).expect("stale signal reconcile");
    let second = watcher.reconcile(2_300).expect("periodic reconcile");
    assert!(first.read_failures.is_empty());
    assert!(second.read_failures.is_empty());
    assert!(watcher.health_for_path(&root_path).is_none());
    assert!(watcher.health_for_path(&child_path).is_none());

    let saved: RolloutWatcherCheckpoint =
        serde_json::from_slice(&fs::read(&checkpoint).expect("checkpoint"))
            .expect("valid checkpoint");
    assert_eq!(saved.sources.len(), 1);
    assert_eq!(
        saved.sources[0].source_file.normalized_path.to_lowercase(),
        unrelated_path.to_string_lossy().to_lowercase()
    );
    let tombstones = DeletionTombstoneStore::new(root.join("deletion-tombstones.json"))
        .load()
        .expect("tombstones");
    assert_eq!(
        tombstones.operations[0].reconciliation_state,
        DeletionReconciliationState::Completed
    );
    assert_eq!(tombstones.operations[0].retired_source_files.len(), 2);
    assert_eq!(
        tombstones.operations[0].thread_deleted_confirmations,
        vec!["thread-child"]
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn pending_tombstone_recovers_before_checkpoint_source_can_be_read() {
    let root = temp_dir();
    let checkpoint = root.join("checkpoint.json");
    let path = rollout_path(&root, "pending-recovery");
    write_lines(&path, &[session_meta("thread-pending", "C:\\fixture")]);
    let watcher_config = config(&root, "home-a", &checkpoint);
    let mut initial = RolloutTailWatcher::new(watcher_config.clone());
    initial.reconcile(1_000).expect("initial checkpoint");
    drop(initial);

    let store = DeletionTombstoneStore::new(watcher_config.deletion_tombstones_path.clone());
    store
        .save(&DeletionTombstoneDocument {
            version: 1,
            operations: vec![DeletionTombstone::confirmed(
                "3318d616-f282-47e7-b8ea-736e23987672",
                CodexThreadKey::new("home-a", "thread-pending"),
                Vec::new(),
                2_000,
            )],
        })
        .expect("persist pending tombstone");
    fs::remove_file(&path).expect("delete rollout before restart");

    let mut restarted = RolloutTailWatcher::new(watcher_config);
    let report = restarted.reconcile(3_000).expect("startup recovery");
    assert!(report.read_failures.is_empty());
    assert!(restarted.health_for_path(&path).is_none());
    assert!(restarted
        .registry()
        .is_tombstoned(&CodexThreadKey::new("home-a", "thread-pending")));
    let recovered = store.load().expect("reloaded tombstone");
    assert_eq!(
        recovered.operations[0].reconciliation_state,
        DeletionReconciliationState::Completed
    );
    let saved: RolloutWatcherCheckpoint =
        serde_json::from_slice(&fs::read(&checkpoint).expect("checkpoint"))
            .expect("valid checkpoint");
    assert!(saved.sources.is_empty());
    fs::remove_dir_all(root).ok();
}

#[test]
fn checkpoint_rewrite_failure_keeps_persisted_tombstone_pending() {
    let root = temp_dir();
    let checkpoint = root.join("checkpoint-target");
    let watcher_config = config(&root, "home-a", &checkpoint);
    let tombstone_path = watcher_config.deletion_tombstones_path.clone();
    let mut watcher = RolloutTailWatcher::new(watcher_config);
    fs::create_dir_all(&checkpoint).expect("make checkpoint target unwritable as a file");

    let error = watcher
        .reconcile_deletion(DeletionTombstone::confirmed(
            "38a6e11e-3bc5-4874-8866-2219536264b8",
            CodexThreadKey::new("home-a", "thread-pending"),
            Vec::new(),
            1_000,
        ))
        .expect_err("checkpoint rewrite must fail");
    assert!(!error.to_string().is_empty());
    let persisted = DeletionTombstoneStore::new(tombstone_path)
        .load()
        .expect("pending tombstone remains readable");
    assert_eq!(
        persisted.operations[0].reconciliation_state,
        DeletionReconciliationState::Pending
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn tombstone_persistence_failure_rolls_back_in_memory_without_retirement() {
    let root = temp_dir();
    let checkpoint = root.join("checkpoint.json");
    let watcher_config = config(&root, "home-a", &checkpoint);
    let tombstone_path = watcher_config.deletion_tombstones_path.clone();
    let mut watcher = RolloutTailWatcher::new(watcher_config);
    fs::create_dir_all(&tombstone_path).expect("block tombstone file creation");
    let key = CodexThreadKey::new("home-a", "thread-not-persisted");

    let failure = watcher
        .reconcile_deletion(DeletionTombstone::confirmed(
            "8b8dd7f0-a04b-4fcb-9733-a4f19ea39331",
            key.clone(),
            Vec::new(),
            1_000,
        ))
        .expect_err("tombstone persistence must fail");

    assert!(!failure.tombstone_persisted);
    assert!(!watcher.registry().is_tombstoned(&key));
    fs::remove_dir_all(root).ok();
}

#[test]
fn retired_deleted_rollout_os_error_2_count_stops_increasing() {
    let root = temp_dir();
    let checkpoint = root.join("checkpoint.json");
    let path = rollout_path(&root, "os-error-2-retirement");
    write_lines(&path, &[session_meta("thread-retired-path", "C:\\fixture")]);
    let mut watcher = RolloutTailWatcher::new(config(&root, "home-a", &checkpoint));
    watcher.reconcile(1_000).expect("initial reconcile");
    fs::remove_file(&path).expect("delete rollout");
    watcher
        .reconcile_deletion(DeletionTombstone::confirmed(
            "8d461812-df22-42aa-bf4f-167175159f3f",
            CodexThreadKey::new("home-a", "thread-retired-path"),
            Vec::new(),
            2_000,
        ))
        .expect("retire deleted source");

    let mut os_error_2_count = 0;
    for observed_at in [2_100, 2_200, 2_300, 2_400, 2_500] {
        watcher.record_filesystem_signal([path.clone()], observed_at);
        let report = watcher
            .reconcile(observed_at)
            .expect("post-retirement reconcile");
        os_error_2_count += report
            .read_failures
            .iter()
            .filter(|failure| failure.message.contains("os error 2"))
            .count();
    }
    assert_eq!(os_error_2_count, 0);
    assert!(watcher.health_for_path(&path).is_none());
    fs::remove_dir_all(root).ok();
}

#[test]
fn operating_system_watch_roots_are_limited_to_sessions_directories() {
    let root = temp_dir();
    let checkpoint = root.join("checkpoint.json");
    let watcher = RolloutTailWatcher::new(config(&root, "home-watch-root", &checkpoint));

    assert_eq!(watcher.watched_roots(), vec![root.join("sessions")]);

    fs::remove_dir_all(root).ok();
}

#[test]
fn discovery_finds_new_rollouts_and_keeps_file_identity_stable() {
    let root = temp_dir();
    let path = rollout_path(&root, "new");
    write_lines(&path, &[session_meta("thread-new", "C:\\fixture")]);

    let first = discover_rollout_sources(&[home(&root, "home-a")]).expect("first discovery");
    let second = discover_rollout_sources(&[home(&root, "home-a")]).expect("second discovery");
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].file_identity, second[0].file_identity);
    assert!(first[0].file_identity.filesystem_id.is_some());
    fs::remove_dir_all(root).ok();
}

#[test]
fn discovery_supports_multiple_rollouts_and_multiple_codex_homes() {
    let first_home = temp_dir();
    let second_home = temp_dir();
    write_lines(
        &rollout_path(&first_home, "one"),
        &[session_meta("thread-1", "C:\\one")],
    );
    write_lines(
        &rollout_path(&first_home, "two"),
        &[session_meta("thread-2", "C:\\two")],
    );
    write_lines(
        &rollout_path(&second_home, "three"),
        &[session_meta("thread-3", "C:\\three")],
    );

    let discovered =
        discover_rollout_sources(&[home(&first_home, "home-a"), home(&second_home, "home-b")])
            .expect("discover homes");
    assert_eq!(discovered.len(), 3);
    assert_eq!(
        discovered
            .iter()
            .map(|source| source.codex_home.identity.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len(),
        2
    );
    fs::remove_dir_all(first_home).ok();
    fs::remove_dir_all(second_home).ok();
}

#[test]
fn watcher_publishes_only_confirmed_subagent_parent_and_agent_path_in_snapshot() {
    let root = temp_dir();
    let path = rollout_path(&root, "child-identity");
    let checkpoint = root.join("checkpoint.json");
    write_lines(
        &path,
        &[
            subagent_session_meta("thread-child", "thread-main", "/root/reader"),
            task_started("turn-child"),
        ],
    );
    let mut watcher = RolloutTailWatcher::new(config(&root, "home-a", &checkpoint));

    watcher.reconcile(2_000).expect("reconcile child");

    let snapshot = watcher.registry().snapshot();
    let child = snapshot
        .threads
        .iter()
        .find(|thread| thread.key.thread_id == "thread-child")
        .expect("child snapshot");
    assert_eq!(
        child
            .parent_thread_key
            .as_ref()
            .map(|parent| parent.value.thread_id.as_str()),
        Some("thread-main")
    );
    assert_eq!(
        child.agent_path.as_ref().map(|path| path.value.as_str()),
        Some("/root/reader")
    );
    assert_eq!(
        child
            .current_turn
            .as_ref()
            .map(|turn| turn.key.turn_id.as_str()),
        Some("turn-child")
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn watcher_reads_append_and_resume_from_the_same_file() {
    let root = temp_dir();
    let path = rollout_path(&root, "resume");
    let checkpoint = root.join("checkpoint.json");
    write_lines(
        &path,
        &[
            session_meta("thread-resume", "C:\\fixture"),
            task_started("turn-1"),
        ],
    );
    let mut watcher = RolloutTailWatcher::new(config(&root, "home-a", &checkpoint));

    let initial = watcher.reconcile(1_000).expect("initial reconciliation");
    assert_eq!(initial.envelopes.len(), 2);
    append_lines(
        &path,
        &[
            task_complete("turn-1"),
            task_started_at("turn-2", "2026-08-25T14:03:28.110Z", 1787666608),
            turn_context("turn-2", "gpt-5.6-terra"),
        ],
    );
    let resumed = watcher.reconcile(2_000).expect("resume reconciliation");
    assert_eq!(resumed.envelopes.len(), 3);
    let view = watcher
        .registry()
        .resolve(&CodexThreadKey::new("home-a", "thread-resume"))
        .expect("resolved thread");
    assert_eq!(view.lifecycle.unwrap().value, ExternalLifecycle::Running);
    assert_eq!(view.observed_model.unwrap().value, "gpt-5.6-terra");
    fs::remove_dir_all(root).ok();
}

#[test]
fn duplicate_signal_does_not_replay_committed_records() {
    let root = temp_dir();
    let path = rollout_path(&root, "duplicate");
    let checkpoint = root.join("checkpoint.json");
    write_lines(&path, &[session_meta("thread-duplicate", "C:\\fixture")]);
    let mut watcher = RolloutTailWatcher::new(config(&root, "home-a", &checkpoint));
    assert_eq!(watcher.reconcile(1_000).unwrap().envelopes.len(), 1);

    watcher.record_filesystem_signal([path.clone()], 1_100);
    watcher.record_filesystem_signal([path.clone()], 1_101);
    assert!(watcher.reconcile(1_200).unwrap().envelopes.is_empty());
    assert_eq!(
        watcher
            .health_for_path(&path)
            .unwrap()
            .last_filesystem_signal_at_ms,
        Some(1_101)
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn periodic_reconciliation_recovers_a_missed_notification() {
    let root = temp_dir();
    let path = rollout_path(&root, "missed");
    let checkpoint = root.join("checkpoint.json");
    write_lines(&path, &[session_meta("thread-missed", "C:\\fixture")]);
    let mut watcher = RolloutTailWatcher::new(config(&root, "home-a", &checkpoint));
    watcher.reconcile(1_000).unwrap();

    append_lines(&path, &[task_started("turn-missed")]);
    let recovered = watcher.reconcile(2_000).expect("periodic reconciliation");
    assert_eq!(recovered.envelopes.len(), 1);
    assert_eq!(recovered.processed_sources, 1);
    fs::remove_dir_all(root).ok();
}

#[test]
fn failed_delta_does_not_partially_commit_running_lifecycle_or_cursor() {
    let root = temp_dir();
    let path = rollout_path(&root, "transaction-rollback");
    let checkpoint = root.join("checkpoint.json");
    write_lines(&path, &[session_meta("thread-transaction", "C:\\fixture")]);
    let mut watcher = RolloutTailWatcher::new(config(&root, "home-a", &checkpoint));
    watcher.reconcile(1_000).expect("baseline");
    let baseline = fs::read(&checkpoint).expect("baseline checkpoint");

    append_lines(
        &path,
        &[
            task_started("turn-transaction"),
            serde_json::json!({
                "timestamp": "2026-08-25T13:57:11.000Z",
                "type": "turn_context",
                "payload": {"model": "gpt-fixture"}
            })
            .to_string(),
        ],
    );
    let first = watcher.reconcile(2_000).expect("reported read failure");
    assert_eq!(first.read_failures.len(), 1);
    assert!(first.read_failures[0]
        .message
        .starts_with("unsupported rollout schema:"));
    assert!(watcher
        .registry()
        .resolve(&CodexThreadKey::new("home-a", "thread-transaction"))
        .expect("baseline identity")
        .lifecycle
        .is_none());
    assert_eq!(
        fs::read(&checkpoint).expect("unchanged checkpoint"),
        baseline
    );

    let retry = watcher.reconcile(3_000).expect("retry remains recoverable");
    assert_eq!(retry.read_failures.len(), 1);
    assert!(watcher
        .registry()
        .resolve(&CodexThreadKey::new("home-a", "thread-transaction"))
        .expect("baseline identity")
        .lifecycle
        .is_none());
    fs::remove_dir_all(root).ok();
}

#[test]
fn legacy_rollouts_reach_real_task_complete_and_are_settled_on_cold_scan() {
    let root = temp_dir();
    let checkpoint = root.join("checkpoint.json");
    let fixture = legacy_completed_fixture();
    let files = fixture["files"].as_array().expect("fixture files");
    for file in files {
        let thread_id = file["threadId"].as_str().expect("thread id");
        let path = rollout_path(&root, thread_id);
        let records = file["records"]
            .as_array()
            .expect("records")
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>();
        write_lines(&path, &records);
    }
    let mut watcher = RolloutTailWatcher::new(config(&root, "home-legacy", &checkpoint));
    let observed_at = 1_787_785_200_000i64;
    let report = watcher.reconcile(observed_at).expect("legacy cold scan");
    assert!(report.read_failures.is_empty());

    for file in files {
        let thread_id = file["threadId"].as_str().expect("thread id");
        let thread = watcher
            .registry()
            .snapshot()
            .threads
            .into_iter()
            .find(|thread| thread.key.thread_id == thread_id)
            .expect("canonical legacy thread");
        assert_eq!(
            thread.lifecycle.expect("lifecycle").value,
            ExternalLifecycle::Completed
        );
        assert_eq!(
            thread
                .authority_provenance
                .expect("authority")
                .freshness
                .state,
            FreshnessState::Settled
        );
        assert_eq!(
            thread
                .token_snapshot
                .expect("legacy token snapshot")
                .value
                .cache_write_input_tokens,
            None
        );
    }
    fs::remove_dir_all(root).ok();
}

#[test]
fn old_noncritical_missing_fields_do_not_block_later_completion() {
    let root = temp_dir();
    let path = rollout_path(&root, "old-optional-fields");
    let checkpoint = root.join("checkpoint.json");
    let meta = serde_json::json!({
        "timestamp": "2026-07-01T00:00:00.000Z",
        "type": "session_meta",
        "payload": {
            "id": "thread-old-optional",
            "source": {"subagent": {"thread_spawn": {
                "parent_thread_id": "thread-parent",
                "depth": 1
            }}}
        }
    })
    .to_string();
    let token_without_info = serde_json::json!({
        "timestamp": "2026-07-01T00:00:02.000Z",
        "type": "event_msg",
        "payload": {"type": "token_count"}
    })
    .to_string();
    write_lines(
        &path,
        &[
            meta,
            task_started_at("turn-old", "2026-07-01T00:00:01.000Z", 1_788_307_201),
            token_without_info,
            serde_json::json!({
                "timestamp": "2026-07-01T00:00:03.000Z",
                "type": "event_msg",
                "payload": {
                    "type": "task_complete",
                    "turn_id": "turn-old",
                    "completed_at": 1_788_307_203i64,
                    "duration_ms": 2_000
                }
            })
            .to_string(),
        ],
    );
    let mut watcher = RolloutTailWatcher::new(config(&root, "home-old", &checkpoint));
    let report = watcher
        .reconcile(1_788_393_600_000)
        .expect("old schema scan");
    assert!(report.read_failures.is_empty());
    let thread = watcher
        .registry()
        .snapshot()
        .threads
        .into_iter()
        .next()
        .expect("thread");
    assert_eq!(
        thread.lifecycle.expect("lifecycle").value,
        ExternalLifecycle::Completed
    );
    assert_eq!(
        thread.parent_thread_key.expect("parent").value.thread_id,
        "thread-parent"
    );
    assert!(thread.agent_path.is_none());
    assert!(thread.token_snapshot.is_none());
    fs::remove_dir_all(root).ok();
}

#[test]
#[ignore = "manual replay of a copied real legacy rollout corpus"]
fn real_legacy_rollout_corpus_replays_when_requested() {
    let root = std::env::var_os("CODEX_MONITOR_LEGACY_ROLLOUT_CORPUS")
        .map(PathBuf::from)
        .expect("set CODEX_MONITOR_LEGACY_ROLLOUT_CORPUS to a copied corpus root");
    let checkpoint = root.join("forensic-checkpoint.json");
    let mut watcher = RolloutTailWatcher::new(config(&root, "legacy-real-corpus", &checkpoint));
    let report = watcher
        .reconcile(chrono::Utc::now().timestamp_millis())
        .expect("real legacy corpus replay");
    println!(
        "discovered={} processed={} envelopes={} failures={}",
        report.discovered_sources,
        report.processed_sources,
        report.envelopes.len(),
        report.read_failures.len()
    );
    for failure in &report.read_failures {
        println!(
            "failure={} path={}",
            failure.message,
            failure.source_path.display()
        );
    }
    assert!(
        report.read_failures.is_empty(),
        "legacy corpus must parse without retry failures"
    );
    for thread_id in [
        "019f3861-43c1-7862-b34a-6e2e9dc93c3f",
        "019f6beb-d8a6-7831-b1cc-beefd8c8a490",
        "019f7196-8b64-7732-a821-f65682588cd7",
    ] {
        let thread = watcher
            .registry()
            .snapshot()
            .threads
            .into_iter()
            .find(|thread| thread.key.thread_id == thread_id)
            .expect("target legacy thread");
        assert_eq!(
            thread.lifecycle.expect("target lifecycle").value,
            ExternalLifecycle::Completed
        );
    }
}

#[test]
fn unsupported_complete_record_advances_checkpoint_and_health_without_an_envelope() {
    let root = temp_dir();
    let path = rollout_path(&root, "unsupported");
    let checkpoint = root.join("checkpoint.json");
    write_lines(&path, &[session_meta("thread-unsupported", "C:\\fixture")]);
    let mut watcher = RolloutTailWatcher::new(config(&root, "home-a", &checkpoint));
    watcher.reconcile(1_000).unwrap();

    append_lines(
        &path,
        &[serde_json::json!({
            "timestamp": "2026-08-25T13:57:14.000Z",
            "type": "response_item",
            "payload": {"type": "message", "role": "assistant", "content": []}
        })
        .to_string()],
    );
    let report = watcher.reconcile(2_000).unwrap();
    assert!(report.envelopes.is_empty());
    assert_eq!(report.processed_sources, 1);
    assert_eq!(
        watcher
            .health_for_path(&path)
            .unwrap()
            .last_complete_record_observed_at_ms,
        Some(2_000)
    );
    let saved: RolloutWatcherCheckpoint =
        serde_json::from_slice(&fs::read(&checkpoint).unwrap()).unwrap();
    assert_eq!(saved.sources[0].tail.record_ordinal, 2);
    fs::remove_dir_all(root).ok();
}

#[test]
fn watcher_preserves_utf8_partial_line_until_newline_arrives() {
    let root = temp_dir();
    let path = rollout_path(&root, "partial");
    let checkpoint = root.join("checkpoint.json");
    let first = format!("{}\n", session_meta("thread-partial", "C:\\你好"));
    let partial = turn_context("turn-utf8", "模型-测试");
    fs::write(
        &path,
        format!("{first}{}", &partial[..partial.len() - 1]).as_bytes(),
    )
    .unwrap();
    let mut watcher = RolloutTailWatcher::new(config(&root, "home-a", &checkpoint));
    assert_eq!(watcher.reconcile(1_000).unwrap().envelopes.len(), 1);

    let mut file = OpenOptions::new().append(true).open(&path).unwrap();
    file.write_all(b"}\n").unwrap();
    drop(file);
    let completed = watcher.reconcile(2_000).unwrap();
    assert_eq!(completed.envelopes.len(), 1);
    assert_eq!(
        completed.envelopes[0].record["payload"]["model"],
        "模型-测试"
    );
    fs::remove_dir_all(root).ok();
}

#[derive(Clone)]
struct FlakyReader {
    remaining_failures: Arc<AtomicUsize>,
}

impl RolloutDeltaReader for FlakyReader {
    fn read_delta(
        &self,
        path: &Path,
        source_file: &mut SourceFileIdentity,
        state: &mut RolloutTailState,
        observed_timestamp_ms: i64,
    ) -> io::Result<RolloutDelta> {
        if self
            .remaining_failures
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                value.checked_sub(1)
            })
            .is_ok()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "writer lock",
            ));
        }
        read_rollout_delta(path, source_file, state, observed_timestamp_ms)
    }
}

#[derive(Clone)]
struct DelayedReader {
    delay_ms: u64,
}

impl RolloutDeltaReader for DelayedReader {
    fn read_delta(
        &self,
        path: &Path,
        source_file: &mut SourceFileIdentity,
        state: &mut RolloutTailState,
        observed_timestamp_ms: i64,
    ) -> io::Result<RolloutDelta> {
        std::thread::sleep(std::time::Duration::from_millis(self.delay_ms));
        read_rollout_delta(path, source_file, state, observed_timestamp_ms)
    }
}

#[test]
fn reconcile_now_records_observation_after_the_file_read_completes() {
    let root = temp_dir();
    let path = rollout_path(&root, "observed-after-read");
    let checkpoint = root.join("checkpoint.json");
    let source_timestamp = chrono::Utc::now() + chrono::Duration::milliseconds(100);
    let record = serde_json::json!({
        "timestamp": source_timestamp.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        "type": "session_meta",
        "payload": {
            "session_id": "thread-observed-after-read",
            "id": "thread-observed-after-read",
            "cwd": "C:\\fixture",
            "cli_version": "0.147.0",
            "source": "exec",
            "thread_source": "user",
            "model_provider": "openai"
        }
    })
    .to_string();
    write_lines(&path, &[record]);
    let mut watcher = RolloutTailWatcher::with_reader(
        config(&root, "home-observed-after-read", &checkpoint),
        DelayedReader { delay_ms: 200 },
    );

    let report = watcher.reconcile_now().expect("reconcile after read");
    assert_eq!(report.envelopes.len(), 1);
    let timestamps = &report.envelopes[0].timestamps;
    assert!(
        timestamps.observed_timestamp_ms >= timestamps.source_timestamp_ms.expect("source time")
    );
    assert!(timestamps.lag_ms.expect("lag") >= 0);
    fs::remove_dir_all(root).ok();
}

#[test]
fn transient_file_lock_is_retried_without_abandoning_the_source() {
    let root = temp_dir();
    let path = rollout_path(&root, "locked");
    let checkpoint = root.join("checkpoint.json");
    write_lines(&path, &[session_meta("thread-locked", "C:\\fixture")]);
    let reader = FlakyReader {
        remaining_failures: Arc::new(AtomicUsize::new(2)),
    };
    let mut watcher = RolloutTailWatcher::with_reader(config(&root, "home-a", &checkpoint), reader);

    let report = watcher.reconcile(1_000).expect("retry succeeds");
    assert_eq!(report.envelopes.len(), 1);
    let health = watcher.health_for_path(&path).expect("health");
    assert_eq!(health.consecutive_read_failures, 0);
    assert_eq!(health.last_successful_read_at_ms, Some(1_000));
    fs::remove_dir_all(root).ok();
}

#[cfg(windows)]
#[test]
fn windows_exclusive_file_lock_is_retried_until_the_writer_releases_it() {
    use std::os::windows::fs::OpenOptionsExt;

    let root = temp_dir();
    let path = rollout_path(&root, "windows-locked");
    let checkpoint = root.join("checkpoint.json");
    write_lines(
        &path,
        &[session_meta("thread-windows-locked", "C:\\fixture")],
    );
    let lock = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .expect("exclusive Windows lock");
    let release = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(120));
        drop(lock);
    });
    let mut locked_config = config(&root, "home-windows-lock", &checkpoint);
    locked_config.retry = WatcherRetryPolicy {
        max_attempts: 5,
        initial_backoff_ms: 50,
    };
    let mut watcher = RolloutTailWatcher::new(locked_config);

    let report = watcher.reconcile(1_000).expect("retry after lock release");
    release.join().expect("lock release thread");
    assert_eq!(report.envelopes.len(), 1);
    assert!(report.read_failures.is_empty());
    assert_eq!(
        report.envelopes[0].record["payload"]["id"],
        "thread-windows-locked"
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn atomic_checkpoint_restart_resumes_without_replay() {
    let root = temp_dir();
    let path = rollout_path(&root, "restart");
    let checkpoint = root.join("checkpoint.json");
    write_lines(
        &path,
        &[
            session_meta("thread-restart", "C:\\fixture"),
            task_started("turn-1"),
        ],
    );
    let mut first = RolloutTailWatcher::new(config(&root, "home-a", &checkpoint));
    assert_eq!(first.reconcile(1_000).unwrap().envelopes.len(), 2);
    drop(first);

    append_lines(
        &path,
        &[turn_context("turn-1", "gpt-5.6-terra"), token_count(120)],
    );
    let mut restarted = RolloutTailWatcher::new(config(&root, "home-a", &checkpoint));
    let resumed = restarted.reconcile(2_000).expect("checkpoint restart");
    assert_eq!(resumed.envelopes.len(), 2);
    let saved: RolloutWatcherCheckpoint =
        serde_json::from_slice(&fs::read(&checkpoint).unwrap()).unwrap();
    assert_eq!(saved.sources.len(), 1);
    assert_eq!(
        saved.sources[0].source_file.session_meta_id.as_deref(),
        Some("thread-restart")
    );
    assert_eq!(saved.sources[0].tail.record_ordinal, 4);
    assert!(!checkpoint.with_extension("json.tmp").exists());
    fs::remove_dir_all(root).ok();
}

#[test]
fn truncate_rotates_generation_and_rereads_session_meta() {
    let root = temp_dir();
    let path = rollout_path(&root, "reset");
    let checkpoint = root.join("checkpoint.json");
    write_lines(
        &path,
        &[
            session_meta("thread-old", "C:\\fixture"),
            task_started("turn-old"),
            turn_context("turn-old", "gpt-old"),
        ],
    );
    let mut watcher = RolloutTailWatcher::new(config(&root, "home-a", &checkpoint));
    watcher.reconcile(1_000).unwrap();
    let old_generation = watcher
        .source_file_for_path(&path)
        .unwrap()
        .generation
        .clone();

    write_lines(&path, &[session_meta("thread-new", "C:\\fixture")]);
    let reset = watcher.reconcile(2_000).expect("truncate reset");
    assert_eq!(reset.envelopes.len(), 1);
    assert_ne!(
        watcher.source_file_for_path(&path).unwrap().generation,
        old_generation
    );
    assert_eq!(
        watcher
            .source_file_for_path(&path)
            .unwrap()
            .session_meta_id
            .as_deref(),
        Some("thread-new")
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn freshness_uses_complete_records_and_completion_not_file_existence() {
    let root = temp_dir();
    let path = rollout_path(&root, "health");
    let checkpoint = root.join("checkpoint.json");
    write_lines(
        &path,
        &[
            session_meta("thread-health", "C:\\fixture"),
            task_started("turn-health"),
            task_complete("turn-health"),
        ],
    );
    let mut watcher = RolloutTailWatcher::new(config(&root, "home-a", &checkpoint));
    const COMPLETED_SOURCE_TIME_MS: i64 = 1_787_666_233_000;
    watcher.reconcile(COMPLETED_SOURCE_TIME_MS + 1_000).unwrap();
    assert_eq!(
        watcher.refresh_health(COMPLETED_SOURCE_TIME_MS + 2_000)[0]
            .freshness
            .state,
        FreshnessState::Fresh
    );
    assert_eq!(
        watcher.refresh_health(COMPLETED_SOURCE_TIME_MS + 12_000)[0]
            .freshness
            .state,
        FreshnessState::Settled
    );
    assert_eq!(
        watcher
            .registry()
            .resolve(&CodexThreadKey::new("home-a", "thread-health"))
            .unwrap()
            .lifecycle
            .unwrap()
            .provenance
            .freshness
            .state,
        FreshnessState::Settled
    );

    let unknown_path = rollout_path(&root, "unknown");
    fs::write(&unknown_path, b"partial").unwrap();
    watcher
        .reconcile(COMPLETED_SOURCE_TIME_MS + 13_000)
        .unwrap();
    assert_eq!(
        watcher
            .health_for_path(&unknown_path)
            .unwrap()
            .freshness
            .state,
        FreshnessState::Unknown
    );
    fs::remove_dir_all(root).ok();
}

fn live_update(thread_key: CodexThreadKey) -> SourceLaneUpdate {
    SourceLaneUpdate {
        observation_id: "live-observation".to_string(),
        thread_key,
        turn_key: None,
        source_kind: SourceKind::MonitorAppServer,
        temporal_class: SourceTemporalClass::Live,
        source_instance_id: "app-server".to_string(),
        source_generation: "live-generation".to_string(),
        source_timestamp_ms: Some(2_000),
        observed_timestamp_ms: 2_001,
        freshness: super::source_envelope::FreshnessEvidence {
            state: FreshnessState::Fresh,
            last_complete_record_observed_at_ms: Some(2_001),
            reason: "connected".to_string(),
        },
        lifecycle: Some(ExternalLifecycle::Running),
        observed_model: Some("gpt-live".to_string()),
        token_snapshot: Some(TokenSnapshot {
            input_tokens: 290,
            cached_input_tokens: 0,
            cache_write_input_tokens: Some(0),
            output_tokens: 10,
            reasoning_output_tokens: 0,
            total_tokens: 300,
        }),
    }
}

#[test]
fn rollout_enters_near_live_lane_without_overriding_or_adding_to_live() {
    let root = temp_dir();
    let path = rollout_path(&root, "authority");
    let checkpoint = root.join("checkpoint.json");
    write_lines(
        &path,
        &[
            session_meta("thread-authority", "C:\\fixture"),
            task_started("turn-rollout"),
            turn_context("turn-rollout", "gpt-rollout"),
            token_count(250),
            task_complete("turn-rollout"),
        ],
    );
    let mut watcher = RolloutTailWatcher::new(config(&root, "home-a", &checkpoint));
    let key = CodexThreadKey::new("home-a", "thread-authority");
    watcher
        .registry_mut()
        .ingest(live_update(key.clone()))
        .unwrap();

    let report = watcher.reconcile(3_000).unwrap();
    assert!(report
        .envelopes
        .iter()
        .all(|envelope| envelope.temporal_class == SourceTemporalClass::NearLive));
    let lanes = watcher.registry().lanes(&key).unwrap();
    assert_eq!(lanes.live_count(), 1);
    assert_eq!(lanes.near_live_count(), 1);
    let view = watcher.registry().resolve(&key).unwrap();
    assert_eq!(view.lifecycle.unwrap().value, ExternalLifecycle::Running);
    assert_eq!(view.observed_model.unwrap().value, "gpt-live");
    assert_eq!(view.token_snapshot.unwrap().value.total_tokens, 300);
    fs::remove_dir_all(root).ok();
}

#[test]
fn checkpoint_document_round_trips_each_source_independently() {
    let source = SourceFileIdentity {
        normalized_path: "C:\\fixture\\rollout.jsonl".to_string(),
        filesystem_id: Some("volume:file".to_string()),
        generation: "generation-1".to_string(),
        session_meta_id: Some("thread-1".to_string()),
    };
    let checkpoint = RolloutWatcherCheckpoint {
        version: 1,
        sources: vec![RolloutSourceCheckpoint::new(
            "home-a".to_string(),
            source,
            super::rollout_tail::RolloutCheckpoint {
                generation: "generation-1".to_string(),
                committed_byte_offset: 42,
                record_ordinal: 3,
            },
        )],
    };
    let restored: RolloutWatcherCheckpoint =
        serde_json::from_str(&serde_json::to_string(&checkpoint).unwrap()).unwrap();
    assert_eq!(restored, checkpoint);
}
