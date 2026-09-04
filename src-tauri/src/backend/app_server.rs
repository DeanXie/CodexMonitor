use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::env;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::time::timeout;

use crate::backend::events::{AppServerEvent, EventSink};
use crate::codex::args::parse_codex_args;
use crate::shared::codex_core::creation_coordination::{CreationCoordinator, DispatchBoundary};
use crate::shared::process_core::{kill_child_process_tree, tokio_command};
use crate::shared::workspace_interop_core::{
    ExecutionEnvironmentKey, RootLocatorPlatform, RuntimeOriginWorkspaceObservation,
    RuntimeTurnWorkspaceObservation, RuntimeWorkspaceReconciler, RuntimeWorkspaceRoute,
};
use crate::types::WorkspaceEntry;

pub(crate) async fn write_message_to<W: tokio::io::AsyncWrite + Unpin>(
    stdin: &Mutex<W>,
    value: Value,
    boundary: Option<&DispatchBoundary>,
) -> Result<(), String> {
    let mut line = serde_json::to_string(&value).map_err(|e| e.to_string())?;
    line.push('\n');
    let mut stdin = stdin.lock().await;
    if let Some(boundary) = boundary {
        boundary.mark_dispatched();
    }
    stdin
        .write_all(line.as_bytes())
        .await
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
use crate::shared::process_core::{build_cmd_c_command, resolve_windows_executable};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

fn extract_thread_id(value: &Value) -> Option<String> {
    fn extract_from_container(container: Option<&Value>) -> Option<String> {
        let container = container?;
        container
            .get("threadId")
            .or_else(|| container.get("thread_id"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                container
                    .get("thread")
                    .and_then(|thread| thread.get("id"))
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string())
            })
    }

    extract_from_container(value.get("params"))
        .or_else(|| extract_from_container(value.get("result")))
}

#[cfg(test)]
fn push_thread_id(out: &mut Vec<String>, value: Option<&Value>) {
    let Some(value) = value else {
        return;
    };
    if let Some(thread_id) = value.as_str().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        out.push(thread_id.to_string());
        return;
    }
    if let Some(values) = value.as_array() {
        for entry in values {
            push_thread_id(out, Some(entry));
        }
    }
}

#[cfg(test)]
fn extract_related_thread_ids(value: &Value) -> Vec<String> {
    fn collect_agent_thread_ids(value: Option<&Value>, out: &mut Vec<String>) {
        let Some(value) = value else {
            return;
        };
        if let Some(values) = value.as_array() {
            for entry in values {
                collect_agent_thread_ids(Some(entry), out);
            }
            return;
        }
        let Some(record) = value.as_object() else {
            return;
        };
        push_thread_id(
            out,
            record.get("threadId").or_else(|| record.get("thread_id")),
        );
        push_thread_id(out, record.get("id"));
        push_thread_id(
            out,
            record.get("thread").and_then(|thread| {
                thread
                    .get("id")
                    .or_else(|| thread.get("threadId"))
                    .or_else(|| thread.get("thread_id"))
            }),
        );
    }

    fn collect_from_container(container: Option<&Value>, out: &mut Vec<String>) {
        let Some(container) = container.and_then(|value| value.as_object()) else {
            return;
        };
        push_thread_id(
            out,
            container
                .get("threadId")
                .or_else(|| container.get("thread_id")),
        );
        push_thread_id(
            out,
            container.get("thread").and_then(|thread| thread.get("id")),
        );
        push_thread_id(
            out,
            container
                .get("params")
                .and_then(|params| params.get("threadId").or_else(|| params.get("thread_id"))),
        );
        push_thread_id(
            out,
            container
                .get("result")
                .and_then(|result| result.get("threadId").or_else(|| result.get("thread_id"))),
        );
        push_thread_id(
            out,
            container
                .get("newThreadId")
                .or_else(|| container.get("new_thread_id")),
        );
        push_thread_id(
            out,
            container
                .get("receiverThreadId")
                .or_else(|| container.get("receiver_thread_id")),
        );
        push_thread_id(
            out,
            container
                .get("receiverThreadIds")
                .or_else(|| container.get("receiver_thread_ids")),
        );
        collect_agent_thread_ids(
            container
                .get("receiverAgents")
                .or_else(|| container.get("receiver_agents")),
            out,
        );
        collect_agent_thread_ids(
            container
                .get("receiverAgent")
                .or_else(|| container.get("receiver_agent")),
            out,
        );
        collect_agent_thread_ids(
            container
                .get("agentStatuses")
                .or_else(|| container.get("agent_statuses")),
            out,
        );
        if let Some(status_map) = container
            .get("statuses")
            .and_then(|value| value.as_object())
        {
            out.extend(
                status_map
                    .keys()
                    .map(|key| key.trim().to_string())
                    .filter(|key| !key.is_empty()),
            );
        }
        if let Some(item) = container.get("item") {
            collect_from_container(Some(item), out);
        }
    }

    let mut out = Vec::new();
    collect_from_container(value.get("params"), &mut out);
    collect_from_container(value.get("result"), &mut out);
    collect_from_container(Some(value), &mut out);

    let mut seen = HashSet::new();
    out.into_iter()
        .filter(|thread_id| seen.insert(thread_id.clone()))
        .collect()
}

#[derive(Debug, Clone)]
struct ThreadListEntry {
    thread_id: String,
    cwd: Option<String>,
    confirmed_parent_thread_id: Option<String>,
    is_memory_consolidation: bool,
}

fn extract_thread_entries_from_thread_list_result(value: &Value) -> Vec<ThreadListEntry> {
    fn collect_entries(input: &Value, out: &mut Vec<ThreadListEntry>) {
        if let Some(values) = input.as_array() {
            for value in values {
                collect_entries(value, out);
            }
            return;
        }
        let Some(object) = input.as_object() else {
            return;
        };

        let cwd = object
            .get("cwd")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string())
            .or_else(|| {
                object
                    .get("thread")
                    .and_then(|thread| thread.get("cwd"))
                    .and_then(|value| value.as_str())
                    .map(|value| value.to_string())
            });

        let thread_id = object
            .get("threadId")
            .or_else(|| object.get("thread_id"))
            .or_else(|| object.get("id"))
            .and_then(|value| value.as_str())
            .map(|value| value.to_string())
            .or_else(|| {
                object
                    .get("thread")
                    .and_then(|thread| thread.get("id"))
                    .and_then(|value| value.as_str())
                    .map(|value| value.to_string())
            });
        if let Some(thread_id) = thread_id {
            let source = object
                .get("source")
                .or_else(|| object.get("thread").and_then(|thread| thread.get("source")));
            let is_memory_consolidation = source
                .and_then(source_subagent_kind)
                .is_some_and(|kind| kind == "memory_consolidation");
            out.push(ThreadListEntry {
                thread_id,
                cwd,
                confirmed_parent_thread_id: confirmed_parent_thread_id(input),
                is_memory_consolidation,
            });
        }

        for key in ["threads", "items", "results", "data"] {
            if let Some(values) = object.get(key).and_then(|value| value.as_array()) {
                for value in values {
                    collect_entries(value, out);
                }
            }
        }
    }

    let mut out = Vec::new();
    if let Some(result) = value.get("result") {
        collect_entries(result, &mut out);
    }
    out
}

fn normalize_subagent_kind(value: &str) -> String {
    let mut normalized = value.trim().to_ascii_lowercase().replace([' ', '-'], "_");
    if let Some(stripped) = normalized.strip_prefix("subagent_") {
        normalized = stripped.to_string();
    } else if let Some(stripped) = normalized.strip_prefix("sub_agent_") {
        normalized = stripped.to_string();
    }
    normalized
}

fn source_subagent_kind(source: &Value) -> Option<String> {
    if let Some(raw) = source.as_str() {
        let normalized = normalize_subagent_kind(raw);
        return if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        };
    }
    let source_obj = source.as_object()?;
    let sub_agent = source_obj
        .get("subAgent")
        .or_else(|| source_obj.get("sub_agent"))
        .or_else(|| source_obj.get("subagent"))?;

    if let Some(raw) = sub_agent.as_str() {
        let normalized = normalize_subagent_kind(raw);
        return if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        };
    }
    let sub_agent_obj = sub_agent.as_object()?;
    if let Some(explicit) = sub_agent_obj
        .get("kind")
        .or_else(|| sub_agent_obj.get("type"))
        .or_else(|| sub_agent_obj.get("name"))
        .or_else(|| sub_agent_obj.get("id"))
        .and_then(Value::as_str)
    {
        let normalized = normalize_subagent_kind(explicit);
        return if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        };
    }

    let candidate_keys: Vec<&String> = sub_agent_obj
        .keys()
        .filter(|key| key.as_str() != "thread_spawn" && key.as_str() != "threadSpawn")
        .collect();
    if candidate_keys.len() != 1 {
        return None;
    }
    let normalized = normalize_subagent_kind(candidate_keys[0]);
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn thread_started_is_memory_consolidation(value: &Value) -> bool {
    value
        .get("params")
        .and_then(|params| {
            params
                .get("thread")
                .and_then(|thread| thread.get("source"))
                .or_else(|| params.get("source"))
        })
        .and_then(source_subagent_kind)
        .is_some_and(|kind| kind == "memory_consolidation")
}

fn should_suppress_hidden_thread_event(
    method_name: Option<&str>,
    has_result_or_error: bool,
) -> bool {
    !has_result_or_error
        && !matches!(
            method_name,
            Some("thread/archived") | Some("thread/deleted") | Some("codex/backgroundThread")
        )
}

fn is_global_workspace_notification(method: &str) -> bool {
    matches!(
        method,
        "account/updated" | "account/rateLimits/updated" | "account/login/completed"
    )
}

fn should_broadcast_global_workspace_notification(
    method_name: Option<&str>,
    thread_id: Option<&String>,
    request_workspace: Option<&str>,
) -> bool {
    method_name.is_some_and(is_global_workspace_notification)
        && thread_id.is_none()
        && request_workspace.is_none()
}

#[derive(Clone)]
pub(crate) struct RequestContext {
    workspace_id: String,
    method: String,
    params: Value,
}

#[derive(Debug)]
struct RuntimeRouteUpdate {
    thread_id: String,
    route: RuntimeWorkspaceRoute,
}

fn string_at<'a>(value: &'a Value, pointers: &[&str]) -> Option<&'a str> {
    pointers
        .iter()
        .find_map(|pointer| value.pointer(pointer).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn message_cwd(value: &Value) -> Option<&str> {
    string_at(
        value,
        &[
            "/params/thread/cwd",
            "/params/turn/cwd",
            "/params/cwd",
            "/result/thread/cwd",
            "/result/turn/cwd",
            "/result/cwd",
            "/cwd",
        ],
    )
}

fn message_turn_id(value: &Value) -> Option<&str> {
    string_at(
        value,
        &[
            "/params/turn/id",
            "/params/turnId",
            "/params/turn_id",
            "/result/turn/id",
            "/result/turnId",
            "/result/turn_id",
        ],
    )
}

fn confirmed_parent_thread_id(value: &Value) -> Option<String> {
    string_at(
        value,
        &[
            "/params/thread/source/subagent/thread_spawn/parent_thread_id",
            "/params/thread/source/subAgent/threadSpawn/parentThreadId",
            "/params/source/subagent/thread_spawn/parent_thread_id",
            "/params/source/subAgent/threadSpawn/parentThreadId",
            "/thread/source/subagent/thread_spawn/parent_thread_id",
            "/thread/source/subAgent/threadSpawn/parentThreadId",
            "/source/subagent/thread_spawn/parent_thread_id",
            "/source/subAgent/threadSpawn/parentThreadId",
        ],
    )
    .map(str::to_string)
}

fn reconcile_runtime_message(
    runtime: &mut RuntimeWorkspaceReconciler,
    request: Option<&RequestContext>,
    value: &Value,
    observed_at: u64,
) -> Vec<RuntimeRouteUpdate> {
    let mut updates = Vec::new();
    if let Some(request) = request {
        match request.method.as_str() {
            "thread/start" => {
                if let Some(thread_id) = extract_thread_id(value) {
                    let route = runtime.observe_origin(RuntimeOriginWorkspaceObservation {
                        thread_id: &thread_id,
                        thread_start_cwd: message_cwd(&request.params),
                        session_meta_cwd: message_cwd(value),
                        confirmed_parent_thread_id: confirmed_parent_thread_id(value).as_deref(),
                        observed_at,
                    });
                    updates.push(RuntimeRouteUpdate { thread_id, route });
                }
            }
            "thread/list" => {
                for entry in extract_thread_entries_from_thread_list_result(value)
                    .into_iter()
                    .filter(|entry| !entry.is_memory_consolidation)
                {
                    let route = runtime.observe_origin(RuntimeOriginWorkspaceObservation {
                        thread_id: &entry.thread_id,
                        thread_start_cwd: None,
                        session_meta_cwd: entry.cwd.as_deref(),
                        confirmed_parent_thread_id: entry.confirmed_parent_thread_id.as_deref(),
                        observed_at,
                    });
                    updates.push(RuntimeRouteUpdate {
                        thread_id: entry.thread_id,
                        route,
                    });
                }
            }
            "turn/start" => {
                let thread_id = extract_thread_id(&json!({ "params": request.params.clone() }));
                if let (Some(thread_id), Some(turn_id)) = (thread_id, message_turn_id(value)) {
                    let route = runtime.observe_turn(RuntimeTurnWorkspaceObservation {
                        thread_id: &thread_id,
                        turn_id,
                        explicit_turn_cwd: message_cwd(&request.params),
                        turn_context_cwd: message_cwd(value),
                        confirmed_parent_thread_id: None,
                        observed_at,
                    });
                    updates.push(RuntimeRouteUpdate { thread_id, route });
                }
            }
            "thread/read" | "thread/resume" => {
                if let (Some(thread_id), Some(cwd)) = (extract_thread_id(value), message_cwd(value))
                {
                    let route = runtime.observe_origin(RuntimeOriginWorkspaceObservation {
                        thread_id: &thread_id,
                        thread_start_cwd: None,
                        session_meta_cwd: Some(cwd),
                        confirmed_parent_thread_id: confirmed_parent_thread_id(value).as_deref(),
                        observed_at,
                    });
                    updates.push(RuntimeRouteUpdate { thread_id, route });
                }
            }
            _ => {}
        }
        return updates;
    }

    match value.get("method").and_then(Value::as_str) {
        Some("thread/started") => {
            if let Some(thread_id) = extract_thread_id(value) {
                let parent_id = confirmed_parent_thread_id(value);
                let route = runtime.observe_origin(RuntimeOriginWorkspaceObservation {
                    thread_id: &thread_id,
                    thread_start_cwd: message_cwd(value),
                    session_meta_cwd: None,
                    confirmed_parent_thread_id: parent_id.as_deref(),
                    observed_at,
                });
                updates.push(RuntimeRouteUpdate { thread_id, route });
            }
        }
        Some("turn/started") => {
            if let (Some(thread_id), Some(turn_id)) =
                (extract_thread_id(value), message_turn_id(value))
            {
                let parent_id = confirmed_parent_thread_id(value);
                let route = runtime.observe_turn(RuntimeTurnWorkspaceObservation {
                    thread_id: &thread_id,
                    turn_id,
                    explicit_turn_cwd: None,
                    turn_context_cwd: message_cwd(value),
                    confirmed_parent_thread_id: parent_id.as_deref(),
                    observed_at,
                });
                updates.push(RuntimeRouteUpdate { thread_id, route });
            }
        }
        _ => {}
    }
    updates
}

fn apply_runtime_route_updates(
    cache: &mut HashMap<String, String>,
    updates: Vec<RuntimeRouteUpdate>,
) {
    for update in updates {
        if let Some(workspace_id) = update.route.workspace_id {
            cache.insert(update.thread_id, workspace_id);
        } else {
            cache.remove(&update.thread_id);
        }
    }
}

fn runtime_message_observation_key(value: &Value) -> String {
    value.to_string()
}

pub(crate) fn runtime_reconciler_for_home(codex_home: Option<&Path>) -> RuntimeWorkspaceReconciler {
    let codex_home_identity =
        crate::shared::global_sources_core::runtime_config::discover_runtime_codex_homes(
            codex_home.map(Path::to_path_buf),
            std::iter::empty(),
        )
        .into_iter()
        .next()
        .map(|source| source.codex_home.identity)
        .unwrap_or_else(|| "codex-home:runtime-default".to_string());
    let platform = if cfg!(windows) {
        RootLocatorPlatform::Windows
    } else {
        RootLocatorPlatform::Posix
    };
    let execution_environment_key = ExecutionEnvironmentKey::new(match platform {
        RootLocatorPlatform::Windows => "monitor-local-windows",
        RootLocatorPlatform::Posix => "monitor-local-posix",
    })
    .expect("runtime execution environment key is non-empty");
    RuntimeWorkspaceReconciler::new(codex_home_identity, execution_environment_key, platform)
}

fn build_initialize_params(client_version: &str) -> Value {
    json!({
        "clientInfo": {
            "name": "codex_monitor",
            "title": "Codex Monitor",
            "version": client_version
        },
        "capabilities": {
            "experimentalApi": true
        }
    })
}

const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

pub(crate) struct WorkspaceSession {
    pub(crate) codex_args: Option<String>,
    pub(crate) child: Mutex<Child>,
    pub(crate) stdin: Mutex<ChildStdin>,
    pub(crate) pending: Mutex<HashMap<u64, oneshot::Sender<Value>>>,
    pub(crate) request_context: Mutex<HashMap<u64, RequestContext>>,
    pub(crate) thread_workspace: Mutex<HashMap<String, String>>,
    pub(crate) workspace_reconciler: Mutex<RuntimeWorkspaceReconciler>,
    // Shared process owner survives session reconnect; this is only an observer.
    pub(crate) creation_coordinator: Mutex<Option<CreationCoordinator>>,
    pub(crate) runtime_observation_keys: Mutex<HashSet<String>>,
    pub(crate) runtime_observation_clock: AtomicU64,
    pub(crate) hidden_thread_ids: Mutex<HashSet<String>>,
    pub(crate) next_id: AtomicU64,
    /// Callbacks for background threads - events for these threadIds are sent through the channel
    pub(crate) background_thread_callbacks: Mutex<HashMap<String, mpsc::UnboundedSender<Value>>>,
    pub(crate) owner_workspace_id: String,
    pub(crate) workspace_ids: Mutex<HashSet<String>>,
}

impl WorkspaceSession {
    pub(crate) async fn register_workspace(&self, workspace_id: &str) {
        self.register_workspace_with_path(workspace_id, None).await;
    }

    pub(crate) async fn register_workspace_with_path(
        &self,
        workspace_id: &str,
        workspace_path: Option<&str>,
    ) {
        self.workspace_ids
            .lock()
            .await
            .insert(workspace_id.to_string());
        if let Some(path) = workspace_path {
            self.workspace_reconciler
                .lock()
                .await
                .register_workspace(workspace_id, path);
        }
    }

    pub(crate) async fn unregister_workspace(&self, workspace_id: &str) {
        self.workspace_ids.lock().await.remove(workspace_id);
        self.workspace_reconciler
            .lock()
            .await
            .unregister_workspace(workspace_id);
        self.thread_workspace
            .lock()
            .await
            .retain(|_, routed_workspace_id| routed_workspace_id != workspace_id);
    }

    pub(crate) async fn workspace_ids_snapshot(&self) -> Vec<String> {
        self.workspace_ids.lock().await.iter().cloned().collect()
    }

    async fn write_message(&self, value: Value) -> Result<(), String> {
        let mut stdin = self.stdin.lock().await;
        let mut line = serde_json::to_string(&value).map_err(|e| e.to_string())?;
        line.push('\n');
        stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| e.to_string())
    }

    pub(crate) async fn send_request(&self, method: &str, params: Value) -> Result<Value, String> {
        self.send_request_for_workspace(self.owner_workspace_id.as_str(), method, params)
            .await
    }

    pub(crate) async fn send_request_for_workspace(
        &self,
        workspace_id: &str,
        method: &str,
        params: Value,
    ) -> Result<Value, String> {
        self.send_request_for_workspace_observed(workspace_id, method, params, None)
            .await
    }

    pub(crate) async fn send_request_for_workspace_observed(
        &self,
        workspace_id: &str,
        method: &str,
        params: Value,
        boundary: Option<&DispatchBoundary>,
    ) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.register_workspace(workspace_id).await;
        self.pending.lock().await.insert(id, tx);
        self.request_context.lock().await.insert(
            id,
            RequestContext {
                workspace_id: workspace_id.to_string(),
                method: method.to_string(),
                params: params.clone(),
            },
        );
        if let Err(error) = write_message_to(
            &self.stdin,
            json!({ "id": id, "method": method, "params": params }),
            boundary,
        )
        .await
        {
            self.pending.lock().await.remove(&id);
            self.request_context.lock().await.remove(&id);
            return Err(error);
        }
        match timeout(REQUEST_TIMEOUT, rx).await {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(_)) => Err("request canceled".to_string()),
            Err(_) => {
                self.pending.lock().await.remove(&id);
                self.request_context.lock().await.remove(&id);
                Err(format!(
                    "request timed out after {} seconds",
                    REQUEST_TIMEOUT.as_secs()
                ))
            }
        }
    }

    pub(crate) async fn send_notification(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<(), String> {
        let value = if let Some(params) = params {
            json!({ "method": method, "params": params })
        } else {
            json!({ "method": method })
        };
        self.write_message(value).await
    }

    pub(crate) async fn send_response(&self, id: Value, result: Value) -> Result<(), String> {
        self.write_message(json!({ "id": id, "result": result }))
            .await
    }
}

pub(crate) fn build_codex_path_env(codex_bin: Option<&str>) -> Option<String> {
    let mut paths: Vec<PathBuf> = env::var_os("PATH")
        .map(|value| env::split_paths(&value).collect())
        .unwrap_or_default();

    let mut extras: Vec<PathBuf> = Vec::new();

    #[cfg(not(target_os = "windows"))]
    {
        extras.extend(
            [
                "/opt/homebrew/bin",
                "/usr/local/bin",
                "/usr/bin",
                "/bin",
                "/usr/sbin",
                "/sbin",
            ]
            .into_iter()
            .map(PathBuf::from),
        );

        if let Ok(home) = env::var("HOME") {
            let home_path = Path::new(&home);
            extras.push(home_path.join(".local/bin"));
            extras.push(home_path.join(".local/share/mise/shims"));
            extras.push(home_path.join(".cargo/bin"));
            extras.push(home_path.join(".bun/bin"));
            let nvm_root = home_path.join(".nvm/versions/node");
            if let Ok(entries) = std::fs::read_dir(nvm_root) {
                for entry in entries.flatten() {
                    let bin_path = entry.path().join("bin");
                    if bin_path.is_dir() {
                        extras.push(bin_path);
                    }
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = env::var("APPDATA") {
            extras.push(Path::new(&appdata).join("npm"));
        }
        if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
            extras.push(
                Path::new(&local_app_data)
                    .join("Microsoft")
                    .join("WindowsApps"),
            );
        }
        if let Ok(home) = env::var("USERPROFILE").or_else(|_| env::var("HOME")) {
            let home_path = Path::new(&home);
            extras.push(home_path.join(".cargo").join("bin"));
            extras.push(home_path.join("scoop").join("shims"));
        }
        if let Ok(program_data) = env::var("PROGRAMDATA") {
            extras.push(Path::new(&program_data).join("chocolatey").join("bin"));
        }
    }

    if let Some(bin_path) = codex_bin.filter(|value| !value.trim().is_empty()) {
        if let Some(parent) = Path::new(bin_path).parent() {
            extras.push(parent.to_path_buf());
        }
    }

    for extra in extras {
        if !paths.iter().any(|path| path == &extra) {
            paths.push(extra);
        }
    }

    if paths.is_empty() {
        return None;
    }

    env::join_paths(paths)
        .ok()
        .map(|joined| joined.to_string_lossy().to_string())
}

pub(crate) fn build_codex_command_with_bin(
    codex_bin: Option<String>,
    codex_args: Option<&str>,
    args: Vec<String>,
) -> Result<Command, String> {
    let bin = codex_bin
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "codex".into());

    let path_env = build_codex_path_env(codex_bin.as_deref());
    let mut command_args = parse_codex_args(codex_args)?;
    command_args.extend(args);

    #[cfg(target_os = "windows")]
    let mut command = {
        let bin_trimmed = bin.trim();
        let resolved = resolve_windows_executable(bin_trimmed, path_env.as_deref());
        let resolved_path = resolved
            .as_deref()
            .unwrap_or_else(|| Path::new(bin_trimmed));
        let ext = resolved_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase());

        if matches!(ext.as_deref(), Some("cmd") | Some("bat")) {
            let mut command = tokio_command("cmd");
            let command_line = build_cmd_c_command(resolved_path, &command_args)?;
            command.arg("/D");
            command.arg("/S");
            command.arg("/C");
            command.raw_arg(command_line);
            command
        } else {
            let mut command = tokio_command(resolved_path);
            command.args(command_args);
            command
        }
    };

    #[cfg(not(target_os = "windows"))]
    let mut command = {
        let mut command = tokio_command(bin.trim());
        command.args(command_args);
        command
    };

    if let Some(path_env) = path_env {
        command.env("PATH", path_env);
    }
    Ok(command)
}

pub(crate) async fn check_codex_installation(
    codex_bin: Option<String>,
) -> Result<Option<String>, String> {
    let mut command = build_codex_command_with_bin(codex_bin, None, vec!["--version".to_string()])?;
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    let output = match timeout(Duration::from_secs(5), command.output()).await {
        Ok(result) => result.map_err(|e| {
            if e.kind() == ErrorKind::NotFound {
                "Codex CLI not found. Install Codex and ensure `codex` is on your PATH.".to_string()
            } else {
                e.to_string()
            }
        })?,
        Err(_) => {
            return Err(
                "Timed out while checking Codex CLI. Make sure `codex --version` runs in Terminal."
                    .to_string(),
            );
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        if detail.is_empty() {
            return Err(
                "Codex CLI failed to start. Try running `codex --version` in Terminal.".to_string(),
            );
        }
        return Err(format!(
            "Codex CLI failed to start: {detail}. Try running `codex --version` in Terminal."
        ));
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(if version.is_empty() {
        None
    } else {
        Some(version)
    })
}

pub(crate) async fn spawn_workspace_session<E: EventSink>(
    entry: WorkspaceEntry,
    default_codex_bin: Option<String>,
    codex_args: Option<String>,
    codex_home: Option<PathBuf>,
    client_version: String,
    event_sink: E,
) -> Result<Arc<WorkspaceSession>, String> {
    let codex_bin = default_codex_bin;
    let _ = check_codex_installation(codex_bin.clone()).await?;

    let mut command = build_codex_command_with_bin(
        codex_bin,
        codex_args.as_deref(),
        vec!["app-server".to_string()],
    )?;
    command.current_dir(&entry.path);
    if let Some(path) = codex_home.as_ref() {
        command.env("CODEX_HOME", path);
    }
    command.stdin(std::process::Stdio::piped());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let stdin = child.stdin.take().ok_or("missing stdin")?;
    let stdout = child.stdout.take().ok_or("missing stdout")?;
    let stderr = child.stderr.take().ok_or("missing stderr")?;

    let resolved_codex_home = codex_home
        .clone()
        .or_else(crate::codex::home::resolve_default_codex_home);
    let mut workspace_reconciler = runtime_reconciler_for_home(resolved_codex_home.as_deref());
    workspace_reconciler.register_workspace(&entry.id, &entry.path);

    let session = Arc::new(WorkspaceSession {
        codex_args,
        child: Mutex::new(child),
        stdin: Mutex::new(stdin),
        pending: Mutex::new(HashMap::new()),
        request_context: Mutex::new(HashMap::new()),
        thread_workspace: Mutex::new(HashMap::new()),
        workspace_reconciler: Mutex::new(workspace_reconciler),
        creation_coordinator: Mutex::new(None),
        runtime_observation_keys: Mutex::new(HashSet::new()),
        runtime_observation_clock: AtomicU64::new(0),
        hidden_thread_ids: Mutex::new(HashSet::new()),
        next_id: AtomicU64::new(1),
        background_thread_callbacks: Mutex::new(HashMap::new()),
        owner_workspace_id: entry.id.clone(),
        workspace_ids: Mutex::new(HashSet::from([entry.id.clone()])),
    });

    let session_clone = Arc::clone(&session);
    let fallback_workspace_id = entry.id.clone();
    let event_sink_clone = event_sink.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if line.trim().is_empty() {
                continue;
            }
            let value: Value = match serde_json::from_str(&line) {
                Ok(value) => value,
                Err(err) => {
                    let payload = AppServerEvent {
                        workspace_id: fallback_workspace_id.clone(),
                        message: json!({
                            "method": "codex/parseError",
                            "params": { "error": err.to_string(), "raw": line },
                        }),
                    };
                    event_sink_clone.emit_app_server_event(payload);
                    continue;
                }
            };

            let maybe_id = value.get("id").and_then(|id| id.as_u64());
            let has_method = value.get("method").is_some();
            let has_result_or_error = value.get("result").is_some() || value.get("error").is_some();
            let method_name = value.get("method").and_then(|method| method.as_str());

            if method_name == Some("turn/completed") {
                if let (Some(thread), Some(turn), Some(outcome)) = (
                    value.pointer("/params/threadId").and_then(Value::as_str),
                    value.pointer("/params/turn/id").and_then(Value::as_str),
                    value.pointer("/params/turn/status").and_then(Value::as_str),
                ) {
                    let observer = session_clone.creation_coordinator.lock().await.clone();
                    if let Some(observer) = observer {
                        let home = session_clone
                            .workspace_reconciler
                            .lock()
                            .await
                            .codex_home_identity()
                            .to_string();
                        observer.observe_known_turn_outcome(
                            &crate::shared::global_sources_core::rollout_identity::CodexThreadKey::new(home,thread),
                            turn,outcome,
                        );
                    }
                }
            }

            // Check if this event is for a background thread
            let thread_id = extract_thread_id(&value);
            let mut request_workspace: Option<String> = None;
            let mut request_method: Option<String> = None;
            let mut completed_request: Option<RequestContext> = None;
            if let Some(id) = maybe_id {
                if has_result_or_error {
                    if let Some(context) = session_clone.request_context.lock().await.remove(&id) {
                        request_workspace = Some(context.workspace_id.clone());
                        request_method = Some(context.method.clone());
                        completed_request = Some(context);
                    }
                }
            }

            let observation_key = runtime_message_observation_key(&value);
            let should_reconcile = session_clone
                .runtime_observation_keys
                .lock()
                .await
                .insert(observation_key);
            if should_reconcile {
                let observed_at = session_clone
                    .runtime_observation_clock
                    .fetch_add(1, Ordering::SeqCst);
                let updates = {
                    let mut runtime = session_clone.workspace_reconciler.lock().await;
                    reconcile_runtime_message(
                        &mut *runtime,
                        completed_request.as_ref(),
                        &value,
                        observed_at,
                    )
                };
                let mut cache = session_clone.thread_workspace.lock().await;
                apply_runtime_route_updates(&mut *cache, updates);
            }
            if matches!(request_method.as_deref(), Some("thread/list")) {
                let thread_entries = extract_thread_entries_from_thread_list_result(&value);
                if !thread_entries.is_empty() {
                    let mut hidden_thread_ids = Vec::new();
                    let mut thread_workspace = session_clone.thread_workspace.lock().await;
                    for entry in thread_entries {
                        if entry.is_memory_consolidation {
                            thread_workspace.remove(&entry.thread_id);
                            hidden_thread_ids.push(entry.thread_id);
                        }
                    }
                    drop(thread_workspace);
                    if !hidden_thread_ids.is_empty() {
                        let mut hidden = session_clone.hidden_thread_ids.lock().await;
                        for thread_id in hidden_thread_ids {
                            hidden.insert(thread_id);
                        }
                    }
                }
            }

            let mapped_thread_workspace = if let Some(ref tid) = thread_id {
                session_clone
                    .thread_workspace
                    .lock()
                    .await
                    .get(tid)
                    .cloned()
            } else {
                None
            };

            let routed_workspace_id = mapped_thread_workspace
                .clone()
                .or_else(|| request_workspace.clone())
                .unwrap_or_else(|| fallback_workspace_id.clone());
            let should_broadcast_unresolved_thread = thread_id.is_some()
                && mapped_thread_workspace.is_none()
                && request_workspace.is_none();

            if let Some(ref tid) = thread_id {
                if method_name == Some("codex/backgroundThread") {
                    let action = value
                        .get("params")
                        .and_then(|params| params.get("action"))
                        .and_then(Value::as_str)
                        .unwrap_or("hide");
                    if action.eq_ignore_ascii_case("hide") {
                        session_clone
                            .hidden_thread_ids
                            .lock()
                            .await
                            .insert(tid.clone());
                    }
                } else if method_name == Some("thread/started")
                    && thread_started_is_memory_consolidation(&value)
                {
                    session_clone
                        .hidden_thread_ids
                        .lock()
                        .await
                        .insert(tid.clone());
                    let payload = AppServerEvent {
                        workspace_id: routed_workspace_id.clone(),
                        message: json!({
                            "method": "codex/backgroundThread",
                            "params": {
                                "threadId": tid,
                                "action": "hide"
                            }
                        }),
                    };
                    event_sink_clone.emit_app_server_event(payload);
                    continue;
                }

                let should_suppress_hidden_thread = {
                    let hidden = session_clone.hidden_thread_ids.lock().await;
                    hidden.contains(tid)
                };
                if should_suppress_hidden_thread
                    && should_suppress_hidden_thread_event(method_name, has_result_or_error)
                {
                    continue;
                }
            }

            if matches!(
                method_name,
                Some("thread/archived") | Some("thread/deleted")
            ) {
                if let Some(ref tid) = thread_id {
                    session_clone.thread_workspace.lock().await.remove(tid);
                    session_clone.hidden_thread_ids.lock().await.remove(tid);
                }
            }

            if let Some(id) = maybe_id {
                if has_result_or_error {
                    if let Some(tx) = session_clone.pending.lock().await.remove(&id) {
                        let _ = tx.send(value);
                    }
                } else if has_method {
                    // Check for background thread callback
                    let mut sent_to_background = false;
                    if let Some(ref tid) = thread_id {
                        let callbacks = session_clone.background_thread_callbacks.lock().await;
                        if let Some(tx) = callbacks.get(tid) {
                            let _ = tx.send(value.clone());
                            sent_to_background = true;
                        }
                    }
                    // Don't emit to frontend if this is a background thread event
                    if !sent_to_background {
                        if should_broadcast_unresolved_thread
                            || should_broadcast_global_workspace_notification(
                                method_name,
                                thread_id.as_ref(),
                                request_workspace.as_deref(),
                            )
                        {
                            let workspace_ids = session_clone.workspace_ids_snapshot().await;
                            if workspace_ids.is_empty() {
                                let payload = AppServerEvent {
                                    workspace_id: routed_workspace_id.clone(),
                                    message: value,
                                };
                                event_sink_clone.emit_app_server_event(payload);
                            } else {
                                for workspace_id in workspace_ids {
                                    let payload = AppServerEvent {
                                        workspace_id,
                                        message: value.clone(),
                                    };
                                    event_sink_clone.emit_app_server_event(payload);
                                }
                            }
                        } else {
                            let payload = AppServerEvent {
                                workspace_id: routed_workspace_id.clone(),
                                message: value,
                            };
                            event_sink_clone.emit_app_server_event(payload);
                        }
                    }
                } else if let Some(tx) = session_clone.pending.lock().await.remove(&id) {
                    let _ = tx.send(value);
                }
            } else if has_method {
                // Check for background thread callback
                let mut sent_to_background = false;
                if let Some(ref tid) = thread_id {
                    let callbacks = session_clone.background_thread_callbacks.lock().await;
                    if let Some(tx) = callbacks.get(tid) {
                        let _ = tx.send(value.clone());
                        sent_to_background = true;
                    }
                }
                // Don't emit to frontend if this is a background thread event
                if !sent_to_background {
                    if should_broadcast_unresolved_thread
                        || should_broadcast_global_workspace_notification(
                            method_name,
                            thread_id.as_ref(),
                            request_workspace.as_deref(),
                        )
                    {
                        let workspace_ids = session_clone.workspace_ids_snapshot().await;
                        if workspace_ids.is_empty() {
                            let payload = AppServerEvent {
                                workspace_id: routed_workspace_id,
                                message: value,
                            };
                            event_sink_clone.emit_app_server_event(payload);
                        } else {
                            for workspace_id in workspace_ids {
                                let payload = AppServerEvent {
                                    workspace_id,
                                    message: value.clone(),
                                };
                                event_sink_clone.emit_app_server_event(payload);
                            }
                        }
                    } else {
                        let payload = AppServerEvent {
                            workspace_id: routed_workspace_id,
                            message: value,
                        };
                        event_sink_clone.emit_app_server_event(payload);
                    }
                }
            }
        }

        // Ensure pending foreground requests cannot accumulate after process output ends.
        session_clone.pending.lock().await.clear();
        session_clone.request_context.lock().await.clear();
    });

    let workspace_id = entry.id.clone();
    let event_sink_clone = event_sink.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if line.trim().is_empty() {
                continue;
            }
            let payload = AppServerEvent {
                workspace_id: workspace_id.clone(),
                message: json!({
                    "method": "codex/stderr",
                    "params": { "message": line },
                }),
            };
            event_sink_clone.emit_app_server_event(payload);
        }
    });

    let init_params = build_initialize_params(&client_version);
    let init_result = timeout(
        Duration::from_secs(15),
        session.send_request("initialize", init_params),
    )
    .await;
    let init_response = match init_result {
        Ok(response) => response,
        Err(_) => {
            let mut child = session.child.lock().await;
            kill_child_process_tree(&mut child).await;
            return Err(
                "Codex app-server did not respond to initialize. Check that `codex app-server` works in Terminal."
                    .to_string(),
            );
        }
    };
    init_response?;
    session.send_notification("initialized", None).await?;

    let payload = AppServerEvent {
        workspace_id: entry.id.clone(),
        message: json!({
            "method": "codex/connected",
            "params": { "workspaceId": entry.id.clone() }
        }),
    };
    event_sink.emit_app_server_event(payload);

    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::{
        apply_runtime_route_updates, build_initialize_params, extract_related_thread_ids,
        extract_thread_entries_from_thread_list_result, extract_thread_id,
        reconcile_runtime_message, should_suppress_hidden_thread_event, source_subagent_kind,
        thread_started_is_memory_consolidation, RequestContext,
    };
    use crate::shared::workspace_interop_core::{
        ExecutionEnvironmentKey, RootLocatorPlatform, RuntimeWorkspaceReconciler,
    };
    use serde_json::json;
    use std::collections::HashMap;

    fn runtime_reconciler() -> RuntimeWorkspaceReconciler {
        let mut runtime = RuntimeWorkspaceReconciler::new(
            "codex-home-fixture",
            ExecutionEnvironmentKey::new("monitor-runtime-fixture").unwrap(),
            RootLocatorPlatform::Windows,
        );
        runtime.register_workspace("workspace-a", r"C:\origin");
        runtime.register_workspace("workspace-b", r"F:\turn");
        runtime
    }

    fn request_context(
        workspace_id: &str,
        method: &str,
        params: serde_json::Value,
    ) -> RequestContext {
        RequestContext {
            workspace_id: workspace_id.to_string(),
            method: method.to_string(),
            params,
        }
    }

    #[test]
    fn live_thread_start_and_thread_list_reconstruction_share_runtime_contract() {
        let mut live = runtime_reconciler();
        let start_context = request_context(
            "workspace-a",
            "thread/start",
            json!({ "cwd": r"C:\origin" }),
        );
        let live_updates = reconcile_runtime_message(
            &mut live,
            Some(&start_context),
            &json!({ "result": { "thread": { "id": "thread-a" } } }),
            1,
        );
        let mut reconstructed = runtime_reconciler();
        let list_context = request_context("workspace-a", "thread/list", json!({}));
        let reconstructed_updates = reconcile_runtime_message(
            &mut reconstructed,
            Some(&list_context),
            &json!({ "result": { "data": [{ "id": "thread-a", "cwd": r"C:\origin" }] } }),
            1,
        );

        assert_eq!(
            live_updates[0].route.workspace_id.as_deref(),
            Some("workspace-a")
        );
        assert_eq!(
            reconstructed_updates[0].route.workspace_id.as_deref(),
            Some("workspace-a")
        );
        assert_eq!(
            live.route_for_origin("thread-a").unwrap().workspace_key,
            reconstructed
                .route_for_origin("thread-a")
                .unwrap()
                .workspace_key
        );
    }

    #[test]
    fn runtime_turn_start_uses_explicit_cwd_without_rewriting_origin() {
        let mut runtime = runtime_reconciler();
        let start_context = request_context(
            "workspace-a",
            "thread/start",
            json!({ "cwd": r"C:\origin" }),
        );
        reconcile_runtime_message(
            &mut runtime,
            Some(&start_context),
            &json!({ "result": { "thread": { "id": "thread-a" } } }),
            1,
        );
        let turn_context = request_context(
            "workspace-b",
            "turn/start",
            json!({ "threadId": "thread-a", "cwd": r"F:\turn" }),
        );
        let updates = reconcile_runtime_message(
            &mut runtime,
            Some(&turn_context),
            &json!({ "result": { "turn": { "id": "turn-b" } } }),
            2,
        );

        assert_eq!(
            updates[0].route.workspace_id.as_deref(),
            Some("workspace-b")
        );
        assert_eq!(
            runtime
                .route_for_origin("thread-a")
                .unwrap()
                .workspace_id
                .as_deref(),
            Some("workspace-a")
        );
        assert_eq!(
            runtime
                .route_for_turn("thread-a", "turn-b")
                .unwrap()
                .workspace_id
                .as_deref(),
            Some("workspace-b")
        );
    }

    #[test]
    fn exact_read_without_cwd_does_not_forge_workspace_relation_or_cache() {
        let mut runtime = runtime_reconciler();
        let context = request_context(
            "workspace-b",
            "thread/read",
            json!({ "threadId": "external-thread" }),
        );
        let updates = reconcile_runtime_message(
            &mut runtime,
            Some(&context),
            &json!({ "result": { "thread": { "id": "external-thread" } } }),
            1,
        );
        let mut cache = HashMap::new();
        apply_runtime_route_updates(&mut cache, updates);

        assert!(runtime.route_for_origin("external-thread").is_none());
        assert!(!cache.contains_key("external-thread"));
    }

    #[test]
    fn confirmed_child_notification_uses_parent_fallback_only_without_direct_cwd() {
        let mut runtime = runtime_reconciler();
        let parent_context = request_context(
            "workspace-a",
            "thread/start",
            json!({ "cwd": r"C:\origin" }),
        );
        reconcile_runtime_message(
            &mut runtime,
            Some(&parent_context),
            &json!({ "result": { "thread": { "id": "parent" } } }),
            1,
        );
        let updates = reconcile_runtime_message(
            &mut runtime,
            None,
            &json!({
                "method": "thread/started",
                "params": {
                    "thread": {
                        "id": "child",
                        "source": {
                            "subagent": {
                                "thread_spawn": { "parent_thread_id": "parent" }
                            }
                        }
                    }
                }
            }),
            2,
        );

        assert_eq!(
            updates[0].route.workspace_id.as_deref(),
            Some("workspace-a")
        );
        assert_eq!(
            updates[0].route.basis,
            crate::shared::workspace_interop_core::ThreadWorkspaceRelationBasis::ParentFallback
        );
    }

    #[test]
    fn extract_thread_id_reads_camel_case() {
        let value = json!({ "params": { "threadId": "thread-123" } });
        assert_eq!(extract_thread_id(&value), Some("thread-123".to_string()));
    }

    #[test]
    fn extract_thread_id_reads_snake_case() {
        let value = json!({ "params": { "thread_id": "thread-456" } });
        assert_eq!(extract_thread_id(&value), Some("thread-456".to_string()));
    }

    #[test]
    fn extract_thread_id_reads_hook_notification_thread_id() {
        let value = json!({
            "method": "hook/started",
            "params": {
                "threadId": "thread-hook-1",
                "run": { "id": "hook-1" }
            }
        });
        assert_eq!(extract_thread_id(&value), Some("thread-hook-1".to_string()));
    }

    #[test]
    fn extract_thread_id_returns_none_when_missing() {
        let value = json!({ "params": {} });
        assert_eq!(extract_thread_id(&value), None);
    }

    #[test]
    fn build_initialize_params_enables_experimental_api() {
        let params = build_initialize_params("1.2.3");
        assert_eq!(
            params
                .get("capabilities")
                .and_then(|caps| caps.get("experimentalApi"))
                .and_then(|value| value.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn extract_thread_entries_reads_result_data_items() {
        let value = json!({
            "result": {
                "data": [
                    { "id": "thread-a", "cwd": "/tmp/a" },
                    {
                        "threadId": "thread-b",
                        "cwd": "/tmp/b",
                        "source": { "subAgent": "memory_consolidation" }
                    }
                ]
            }
        });
        let entries = extract_thread_entries_from_thread_list_result(&value);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].thread_id, "thread-a");
        assert_eq!(entries[0].cwd.as_deref(), Some("/tmp/a"));
        assert!(!entries[0].is_memory_consolidation);
        assert_eq!(entries[1].thread_id, "thread-b");
        assert_eq!(entries[1].cwd.as_deref(), Some("/tmp/b"));
        assert!(entries[1].is_memory_consolidation);
    }

    #[test]
    fn extract_related_thread_ids_reads_spawn_hints_from_item_payloads() {
        let value = json!({
            "method": "item/completed",
            "params": {
                "threadId": "thread-parent",
                "item": {
                    "type": "mcpToolCall",
                    "new_thread_id": "thread-child"
                }
            }
        });
        let ids = extract_related_thread_ids(&value);
        assert!(ids.contains(&"thread-parent".to_string()));
        assert!(ids.contains(&"thread-child".to_string()));
    }

    #[test]
    fn extract_related_thread_ids_reads_receiver_agent_references() {
        let value = json!({
            "method": "item/completed",
            "params": {
                "threadId": "thread-parent",
                "item": {
                    "type": "collabToolCall",
                    "receiver_agents": [
                        { "thread_id": "thread-child-a" },
                        { "thread": { "id": "thread-child-b" } }
                    ],
                    "statuses": {
                        "thread-child-c": { "status": "running" }
                    }
                }
            }
        });
        let ids = extract_related_thread_ids(&value);
        assert!(ids.contains(&"thread-parent".to_string()));
        assert!(ids.contains(&"thread-child-a".to_string()));
        assert!(ids.contains(&"thread-child-b".to_string()));
        assert!(ids.contains(&"thread-child-c".to_string()));
    }

    #[test]
    fn extract_related_thread_ids_reads_singular_receiver_agent_reference() {
        let value = json!({
            "method": "item/completed",
            "params": {
                "threadId": "thread-parent",
                "item": {
                    "type": "mcpToolCall",
                    "receiver_agent": { "thread_id": "thread-child-single" }
                }
            }
        });
        let ids = extract_related_thread_ids(&value);
        assert!(ids.contains(&"thread-parent".to_string()));
        assert!(ids.contains(&"thread-child-single".to_string()));
    }

    #[test]
    fn source_subagent_kind_reads_string_variants() {
        assert_eq!(
            source_subagent_kind(&json!("subagent-memory-consolidation")),
            Some("memory_consolidation".to_string())
        );
        assert_eq!(
            source_subagent_kind(&json!("sub_agent_memory_consolidation")),
            Some("memory_consolidation".to_string())
        );
    }

    #[test]
    fn source_subagent_kind_reads_nested_subagent_object_keys() {
        let source = json!({
            "subAgent": {
                "memory_consolidation": {
                    "thread_spawn": { "parent_thread_id": "thread-parent" }
                }
            }
        });
        assert_eq!(
            source_subagent_kind(&source),
            Some("memory_consolidation".to_string())
        );
    }

    #[test]
    fn thread_started_memory_consolidation_detects_thread_source() {
        let value = json!({
            "method": "thread/started",
            "params": {
                "thread": {
                    "id": "thread-1",
                    "source": {
                        "subagent": "memory_consolidation"
                    }
                }
            }
        });
        assert!(thread_started_is_memory_consolidation(&value));
    }

    #[test]
    fn thread_started_memory_consolidation_detects_params_source_fallback() {
        let value = json!({
            "method": "thread/started",
            "params": {
                "threadId": "thread-1",
                "source": {
                    "subAgent": "memory_consolidation"
                }
            }
        });
        assert!(thread_started_is_memory_consolidation(&value));
    }

    #[test]
    fn thread_started_memory_consolidation_rejects_non_memory_subagent() {
        let value = json!({
            "method": "thread/started",
            "params": {
                "thread": {
                    "id": "thread-1",
                    "source": {
                        "subAgent": "review"
                    }
                }
            }
        });
        assert!(!thread_started_is_memory_consolidation(&value));
    }

    #[test]
    fn hidden_thread_suppression_allows_rpc_responses() {
        assert!(!should_suppress_hidden_thread_event(
            Some("thread/archived"),
            true
        ));
        assert!(!should_suppress_hidden_thread_event(
            Some("thread/updated"),
            true
        ));
        assert!(!should_suppress_hidden_thread_event(None, true));
    }

    #[test]
    fn hidden_thread_suppression_still_blocks_non_exempt_notifications() {
        assert!(should_suppress_hidden_thread_event(
            Some("thread/updated"),
            false
        ));
        assert!(!should_suppress_hidden_thread_event(
            Some("thread/archived"),
            false
        ));
        assert!(!should_suppress_hidden_thread_event(
            Some("thread/deleted"),
            false
        ));
        assert!(!should_suppress_hidden_thread_event(
            Some("codex/backgroundThread"),
            false
        ));
    }
}
