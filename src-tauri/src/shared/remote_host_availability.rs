//! Process-local, layered evidence for a configured remote execution host.
//!
//! This module deliberately has no dependency on Thread, projection, deletion,
//! workspace identity, project, or token-accounting state.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Mutex;

use super::remote_host_identity::RemoteHostIdentity;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum TransportState {
    NotConfigured,
    Connecting,
    Connected,
    EndpointUnreachable,
    Disconnected,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum AuthState {
    NotAttempted,
    Authenticating,
    Authenticated,
    Failed,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum DaemonState {
    NotObserved,
    Validating,
    Available,
    ProtocolUnsupported,
    IdentityMismatch,
    ServiceMismatch,
    InvalidResponse,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum RuntimeState {
    NotObserved,
    Starting,
    Ready,
    Unavailable,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeAvailability {
    pub workspace_id: Option<String>,
    pub state: RuntimeState,
}

impl Default for RuntimeAvailability {
    fn default() -> Self {
        Self {
            workspace_id: None,
            state: RuntimeState::NotObserved,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum AvailabilityStage {
    Configuration,
    Transport,
    Authentication,
    Daemon,
    Runtime,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AvailabilityDiagnostic {
    pub stage: AvailabilityStage,
    pub message: String,
    pub observed_at: i64,
    pub attempt_id: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteHostAvailabilitySnapshot {
    pub target_id: String,
    pub expected_remote_host_identity: Option<RemoteHostIdentity>,
    pub observed_remote_host_identity: Option<RemoteHostIdentity>,
    pub attempt_id: u64,
    pub transport: TransportState,
    pub auth: AuthState,
    pub daemon: DaemonState,
    pub runtime: RuntimeAvailability,
    pub observed_at: i64,
    pub last_successful_handshake_at: Option<i64>,
    pub last_runtime_ready_at: Option<i64>,
    pub diagnostics: Vec<AvailabilityDiagnostic>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum AvailabilitySummary {
    NotConfigured,
    Connecting,
    EndpointUnreachable,
    Disconnected,
    AuthenticationFailed,
    IdentityMismatch,
    ProtocolUnsupported,
    DaemonVerificationFailed,
    CodexRuntimeUnavailable,
    Ready,
    DaemonAvailable,
    Unknown,
}

pub(crate) fn availability_summary(
    snapshot: &RemoteHostAvailabilitySnapshot,
) -> AvailabilitySummary {
    if snapshot.transport == TransportState::NotConfigured {
        return AvailabilitySummary::NotConfigured;
    }
    if snapshot.transport == TransportState::Connecting
        || snapshot.auth == AuthState::Authenticating
        || snapshot.daemon == DaemonState::Validating
        || snapshot.runtime.state == RuntimeState::Starting
    {
        return AvailabilitySummary::Connecting;
    }
    if snapshot.transport == TransportState::EndpointUnreachable {
        return AvailabilitySummary::EndpointUnreachable;
    }
    if snapshot.transport == TransportState::Disconnected {
        return AvailabilitySummary::Disconnected;
    }
    if snapshot.auth == AuthState::Failed {
        return AvailabilitySummary::AuthenticationFailed;
    }
    if snapshot.daemon == DaemonState::IdentityMismatch {
        return AvailabilitySummary::IdentityMismatch;
    }
    if snapshot.daemon == DaemonState::ProtocolUnsupported {
        return AvailabilitySummary::ProtocolUnsupported;
    }
    if matches!(
        snapshot.daemon,
        DaemonState::ServiceMismatch | DaemonState::InvalidResponse
    ) {
        return AvailabilitySummary::DaemonVerificationFailed;
    }
    if snapshot.runtime.state == RuntimeState::Unavailable {
        return AvailabilitySummary::CodexRuntimeUnavailable;
    }
    if snapshot.transport == TransportState::Connected
        && snapshot.auth == AuthState::Authenticated
        && snapshot.daemon == DaemonState::Available
        && snapshot.runtime.state == RuntimeState::Ready
    {
        return AvailabilitySummary::Ready;
    }
    if snapshot.transport == TransportState::Connected
        && snapshot.auth == AuthState::Authenticated
        && snapshot.daemon == DaemonState::Available
    {
        return AvailabilitySummary::DaemonAvailable;
    }
    AvailabilitySummary::Unknown
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AvailabilityAttempt {
    pub target_id: String,
    pub attempt_id: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AvailabilityEvent {
    TransportConnected,
    EndpointUnreachable {
        diagnostic: String,
    },
    Disconnected {
        diagnostic: String,
    },
    TransportUnknown {
        diagnostic: String,
    },
    AuthStarted,
    AuthSucceeded,
    AuthRejected {
        diagnostic: String,
    },
    AuthUnknown {
        diagnostic: String,
    },
    DaemonValidationStarted,
    DaemonAvailable {
        identity: RemoteHostIdentity,
    },
    ProtocolUnsupported {
        diagnostic: String,
    },
    IdentityMismatch {
        observed_identity: RemoteHostIdentity,
        diagnostic: String,
    },
    ServiceMismatch {
        diagnostic: String,
    },
    DaemonInvalidResponse {
        diagnostic: String,
    },
    DaemonUnknown {
        diagnostic: String,
    },
    RuntimeStarted {
        workspace_id: String,
    },
    RuntimeReady {
        workspace_id: String,
    },
    RuntimeUnavailable {
        workspace_id: String,
        diagnostic: String,
    },
    RuntimeUnknown {
        workspace_id: String,
        diagnostic: String,
    },
}

#[derive(Default)]
struct AvailabilityState {
    next_attempt_id: u64,
    snapshots: BTreeMap<String, RemoteHostAvailabilitySnapshot>,
}

#[derive(Default)]
pub(crate) struct RemoteHostAvailabilityRuntime {
    state: Mutex<AvailabilityState>,
}

impl RemoteHostAvailabilityRuntime {
    pub(crate) fn begin_attempt(
        &self,
        target_id: impl Into<String>,
        expected_identity: Option<RemoteHostIdentity>,
        observed_at: i64,
    ) -> AvailabilityAttempt {
        let target_id = target_id.into();
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.next_attempt_id += 1;
        let attempt_id = state.next_attempt_id;
        let (last_successful_handshake_at, last_runtime_ready_at) = state
            .snapshots
            .get(&target_id)
            .map(|snapshot| {
                (
                    snapshot.last_successful_handshake_at,
                    snapshot.last_runtime_ready_at,
                )
            })
            .unwrap_or((None, None));
        state.snapshots.insert(
            target_id.clone(),
            RemoteHostAvailabilitySnapshot {
                target_id: target_id.clone(),
                expected_remote_host_identity: expected_identity,
                observed_remote_host_identity: None,
                attempt_id,
                transport: TransportState::Connecting,
                auth: AuthState::NotAttempted,
                daemon: DaemonState::NotObserved,
                runtime: RuntimeAvailability::default(),
                observed_at,
                last_successful_handshake_at,
                last_runtime_ready_at,
                diagnostics: Vec::new(),
            },
        );
        AvailabilityAttempt {
            target_id,
            attempt_id,
        }
    }

    pub(crate) fn observe_unconfigured(
        &self,
        target_id: impl Into<String>,
        observed_at: i64,
        diagnostic: impl Into<String>,
    ) -> AvailabilityAttempt {
        let target_id = target_id.into();
        let attempt = self.begin_attempt(target_id.clone(), None, observed_at);
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let snapshot = state.snapshots.get_mut(&target_id).expect("attempt exists");
        snapshot.transport = TransportState::NotConfigured;
        snapshot.diagnostics.push(AvailabilityDiagnostic {
            stage: AvailabilityStage::Configuration,
            message: diagnostic.into(),
            observed_at,
            attempt_id: attempt.attempt_id,
        });
        attempt
    }

    pub(crate) fn invalidate_for_settings_change(
        &self,
        target_id: impl Into<String>,
        expected_identity: Option<RemoteHostIdentity>,
        observed_at: i64,
    ) -> AvailabilityAttempt {
        let target_id = target_id.into();
        let attempt = self.begin_attempt(target_id.clone(), expected_identity, observed_at);
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let snapshot = state.snapshots.get_mut(&target_id).expect("attempt exists");
        snapshot.transport = TransportState::Unknown;
        attempt
    }

    pub(crate) fn current_attempt(&self, target_id: &str) -> Option<AvailabilityAttempt> {
        self.snapshot(target_id)
            .map(|snapshot| AvailabilityAttempt {
                target_id: snapshot.target_id,
                attempt_id: snapshot.attempt_id,
            })
    }

    pub(crate) fn snapshot(&self, target_id: &str) -> Option<RemoteHostAvailabilitySnapshot> {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .snapshots
            .get(target_id)
            .cloned()
    }

    pub(crate) fn snapshots(&self) -> Vec<RemoteHostAvailabilitySnapshot> {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .snapshots
            .values()
            .cloned()
            .collect()
    }

    pub(crate) fn observe(
        &self,
        attempt: AvailabilityAttempt,
        observed_at: i64,
        event: AvailabilityEvent,
    ) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let Some(snapshot) = state.snapshots.get_mut(&attempt.target_id) else {
            return;
        };
        if snapshot.attempt_id != attempt.attempt_id {
            return;
        }
        snapshot.observed_at = observed_at;
        match event {
            AvailabilityEvent::TransportConnected => {
                snapshot.transport = TransportState::Connected;
            }
            AvailabilityEvent::EndpointUnreachable { diagnostic } => {
                snapshot.transport = TransportState::EndpointUnreachable;
                push_diagnostic(
                    snapshot,
                    AvailabilityStage::Transport,
                    diagnostic,
                    observed_at,
                );
            }
            AvailabilityEvent::Disconnected { diagnostic } => {
                snapshot.transport = TransportState::Disconnected;
                snapshot.auth = AuthState::Unknown;
                snapshot.daemon = DaemonState::Unknown;
                snapshot.runtime.state = RuntimeState::Unknown;
                push_diagnostic(
                    snapshot,
                    AvailabilityStage::Transport,
                    diagnostic,
                    observed_at,
                );
            }
            AvailabilityEvent::TransportUnknown { diagnostic } => {
                snapshot.transport = TransportState::Unknown;
                snapshot.auth = AuthState::Unknown;
                snapshot.daemon = DaemonState::Unknown;
                snapshot.runtime.state = RuntimeState::Unknown;
                push_diagnostic(
                    snapshot,
                    AvailabilityStage::Transport,
                    diagnostic,
                    observed_at,
                );
            }
            AvailabilityEvent::AuthStarted => snapshot.auth = AuthState::Authenticating,
            AvailabilityEvent::AuthSucceeded => snapshot.auth = AuthState::Authenticated,
            AvailabilityEvent::AuthRejected { diagnostic } => {
                snapshot.auth = AuthState::Failed;
                push_diagnostic(
                    snapshot,
                    AvailabilityStage::Authentication,
                    diagnostic,
                    observed_at,
                );
            }
            AvailabilityEvent::AuthUnknown { diagnostic } => {
                snapshot.auth = AuthState::Unknown;
                push_diagnostic(
                    snapshot,
                    AvailabilityStage::Authentication,
                    diagnostic,
                    observed_at,
                );
            }
            AvailabilityEvent::DaemonValidationStarted => {
                snapshot.daemon = DaemonState::Validating;
            }
            AvailabilityEvent::DaemonAvailable { identity } => {
                snapshot.daemon = DaemonState::Available;
                snapshot.observed_remote_host_identity = Some(identity);
                snapshot.last_successful_handshake_at = Some(observed_at);
            }
            AvailabilityEvent::ProtocolUnsupported { diagnostic } => {
                snapshot.daemon = DaemonState::ProtocolUnsupported;
                push_diagnostic(snapshot, AvailabilityStage::Daemon, diagnostic, observed_at);
            }
            AvailabilityEvent::IdentityMismatch {
                observed_identity,
                diagnostic,
            } => {
                snapshot.daemon = DaemonState::IdentityMismatch;
                snapshot.observed_remote_host_identity = Some(observed_identity);
                push_diagnostic(snapshot, AvailabilityStage::Daemon, diagnostic, observed_at);
            }
            AvailabilityEvent::ServiceMismatch { diagnostic } => {
                snapshot.daemon = DaemonState::ServiceMismatch;
                push_diagnostic(snapshot, AvailabilityStage::Daemon, diagnostic, observed_at);
            }
            AvailabilityEvent::DaemonInvalidResponse { diagnostic } => {
                snapshot.daemon = DaemonState::InvalidResponse;
                push_diagnostic(snapshot, AvailabilityStage::Daemon, diagnostic, observed_at);
            }
            AvailabilityEvent::DaemonUnknown { diagnostic } => {
                snapshot.daemon = DaemonState::Unknown;
                push_diagnostic(snapshot, AvailabilityStage::Daemon, diagnostic, observed_at);
            }
            AvailabilityEvent::RuntimeStarted { workspace_id } => {
                snapshot.runtime = RuntimeAvailability {
                    workspace_id: Some(workspace_id),
                    state: RuntimeState::Starting,
                };
            }
            AvailabilityEvent::RuntimeReady { workspace_id } => {
                snapshot.runtime = RuntimeAvailability {
                    workspace_id: Some(workspace_id),
                    state: RuntimeState::Ready,
                };
                snapshot.last_runtime_ready_at = Some(observed_at);
            }
            AvailabilityEvent::RuntimeUnavailable {
                workspace_id,
                diagnostic,
            } => {
                snapshot.runtime = RuntimeAvailability {
                    workspace_id: Some(workspace_id),
                    state: RuntimeState::Unavailable,
                };
                push_diagnostic(
                    snapshot,
                    AvailabilityStage::Runtime,
                    diagnostic,
                    observed_at,
                );
            }
            AvailabilityEvent::RuntimeUnknown {
                workspace_id,
                diagnostic,
            } => {
                snapshot.runtime = RuntimeAvailability {
                    workspace_id: Some(workspace_id),
                    state: RuntimeState::Unknown,
                };
                push_diagnostic(
                    snapshot,
                    AvailabilityStage::Runtime,
                    diagnostic,
                    observed_at,
                );
            }
        }
    }
}

fn push_diagnostic(
    snapshot: &mut RemoteHostAvailabilitySnapshot,
    stage: AvailabilityStage,
    message: String,
    observed_at: i64,
) {
    let diagnostic = AvailabilityDiagnostic {
        stage,
        message,
        observed_at,
        attempt_id: snapshot.attempt_id,
    };
    if snapshot.diagnostics.last() != Some(&diagnostic) {
        snapshot.diagnostics.push(diagnostic);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::remote_host_identity::RemoteHostIdentity;

    const TARGET: &str = "remote-a";
    const HOST: &str = "6ba7b810-9dad-41d1-80b4-00c04fd430c8";

    fn host() -> RemoteHostIdentity {
        RemoteHostIdentity::parse(HOST).unwrap()
    }

    fn ready_runtime() -> RemoteHostAvailabilityRuntime {
        let runtime = RemoteHostAvailabilityRuntime::default();
        let attempt = runtime.begin_attempt(TARGET, Some(host()), 100);
        runtime.observe(attempt.clone(), 101, AvailabilityEvent::TransportConnected);
        runtime.observe(attempt.clone(), 102, AvailabilityEvent::AuthStarted);
        runtime.observe(attempt.clone(), 103, AvailabilityEvent::AuthSucceeded);
        runtime.observe(
            attempt.clone(),
            104,
            AvailabilityEvent::DaemonValidationStarted,
        );
        runtime.observe(
            attempt.clone(),
            105,
            AvailabilityEvent::DaemonAvailable { identity: host() },
        );
        runtime.observe(
            attempt.clone(),
            106,
            AvailabilityEvent::RuntimeStarted {
                workspace_id: "workspace-a".to_string(),
            },
        );
        runtime.observe(
            attempt,
            107,
            AvailabilityEvent::RuntimeReady {
                workspace_id: "workspace-a".to_string(),
            },
        );
        runtime
    }

    #[test]
    fn unconfigured_target_is_not_configured() {
        let runtime = RemoteHostAvailabilityRuntime::default();
        runtime.observe_unconfigured(TARGET, 10, "token is missing");
        let snapshot = runtime.snapshot(TARGET).unwrap();
        assert_eq!(snapshot.transport, TransportState::NotConfigured);
        assert_eq!(
            availability_summary(&snapshot),
            AvailabilitySummary::NotConfigured
        );
    }

    #[test]
    fn tcp_failure_is_endpoint_unreachable() {
        let runtime = RemoteHostAvailabilityRuntime::default();
        let attempt = runtime.begin_attempt(TARGET, None, 10);
        runtime.observe(
            attempt,
            11,
            AvailabilityEvent::EndpointUnreachable {
                diagnostic: "connection refused".to_string(),
            },
        );
        let snapshot = runtime.snapshot(TARGET).unwrap();
        assert_eq!(snapshot.transport, TransportState::EndpointUnreachable);
        assert_eq!(snapshot.auth, AuthState::NotAttempted);
        assert_eq!(
            availability_summary(&snapshot),
            AvailabilitySummary::EndpointUnreachable
        );
    }

    #[test]
    fn auth_rejection_is_failed_not_unreachable() {
        let runtime = RemoteHostAvailabilityRuntime::default();
        let attempt = runtime.begin_attempt(TARGET, None, 10);
        runtime.observe(attempt.clone(), 11, AvailabilityEvent::TransportConnected);
        runtime.observe(attempt.clone(), 12, AvailabilityEvent::AuthStarted);
        runtime.observe(
            attempt,
            13,
            AvailabilityEvent::AuthRejected {
                diagnostic: "server rejected credentials".to_string(),
            },
        );
        let snapshot = runtime.snapshot(TARGET).unwrap();
        assert_eq!(snapshot.transport, TransportState::Connected);
        assert_eq!(snapshot.auth, AuthState::Failed);
        assert_eq!(
            availability_summary(&snapshot),
            AvailabilitySummary::AuthenticationFailed
        );
    }

    #[test]
    fn auth_timeout_is_unknown() {
        let runtime = RemoteHostAvailabilityRuntime::default();
        let attempt = runtime.begin_attempt(TARGET, None, 10);
        runtime.observe(attempt.clone(), 11, AvailabilityEvent::TransportConnected);
        runtime.observe(attempt.clone(), 12, AvailabilityEvent::AuthStarted);
        runtime.observe(
            attempt,
            13,
            AvailabilityEvent::AuthUnknown {
                diagnostic: "response timeout".to_string(),
            },
        );
        let snapshot = runtime.snapshot(TARGET).unwrap();
        assert_eq!(snapshot.auth, AuthState::Unknown);
        assert_eq!(
            availability_summary(&snapshot),
            AvailabilitySummary::Unknown
        );
    }

    #[test]
    fn daemon_timeout_is_unknown() {
        let runtime = RemoteHostAvailabilityRuntime::default();
        let attempt = runtime.begin_attempt(TARGET, None, 10);
        runtime.observe(attempt.clone(), 11, AvailabilityEvent::TransportConnected);
        runtime.observe(attempt.clone(), 12, AvailabilityEvent::AuthSucceeded);
        runtime.observe(
            attempt.clone(),
            13,
            AvailabilityEvent::DaemonValidationStarted,
        );
        runtime.observe(
            attempt,
            14,
            AvailabilityEvent::DaemonUnknown {
                diagnostic: "response timeout".to_string(),
            },
        );
        assert_eq!(
            runtime.snapshot(TARGET).unwrap().daemon,
            DaemonState::Unknown
        );
    }

    #[test]
    fn identity_mismatch_is_distinct_from_auth_failure() {
        let runtime = RemoteHostAvailabilityRuntime::default();
        let attempt = runtime.begin_attempt(TARGET, Some(host()), 10);
        runtime.observe(attempt.clone(), 11, AvailabilityEvent::TransportConnected);
        runtime.observe(attempt.clone(), 12, AvailabilityEvent::AuthSucceeded);
        runtime.observe(
            attempt,
            13,
            AvailabilityEvent::IdentityMismatch {
                observed_identity: RemoteHostIdentity::parse(
                    "db7a6abc-cfad-4e6b-a93c-6a6918c2e112",
                )
                .unwrap(),
                diagnostic: "pin mismatch".to_string(),
            },
        );
        let snapshot = runtime.snapshot(TARGET).unwrap();
        assert_eq!(snapshot.auth, AuthState::Authenticated);
        assert_eq!(snapshot.daemon, DaemonState::IdentityMismatch);
        assert_eq!(
            snapshot
                .observed_remote_host_identity
                .as_ref()
                .map(|value| value.as_str()),
            Some("db7a6abc-cfad-4e6b-a93c-6a6918c2e112")
        );
        assert_eq!(
            availability_summary(&snapshot),
            AvailabilitySummary::IdentityMismatch
        );
    }

    #[test]
    fn unsupported_protocol_is_distinct_from_unreachable() {
        let runtime = RemoteHostAvailabilityRuntime::default();
        let attempt = runtime.begin_attempt(TARGET, None, 10);
        runtime.observe(attempt.clone(), 11, AvailabilityEvent::TransportConnected);
        runtime.observe(attempt.clone(), 12, AvailabilityEvent::AuthSucceeded);
        runtime.observe(
            attempt,
            13,
            AvailabilityEvent::ProtocolUnsupported {
                diagnostic: "protocol 2".to_string(),
            },
        );
        let snapshot = runtime.snapshot(TARGET).unwrap();
        assert_eq!(snapshot.daemon, DaemonState::ProtocolUnsupported);
        assert_eq!(
            availability_summary(&snapshot),
            AvailabilitySummary::ProtocolUnsupported
        );
    }

    #[test]
    fn daemon_available_without_runtime_probe_is_not_ready() {
        let runtime = RemoteHostAvailabilityRuntime::default();
        let attempt = runtime.begin_attempt(TARGET, None, 10);
        runtime.observe(attempt.clone(), 11, AvailabilityEvent::TransportConnected);
        runtime.observe(attempt.clone(), 12, AvailabilityEvent::AuthSucceeded);
        runtime.observe(
            attempt,
            13,
            AvailabilityEvent::DaemonAvailable { identity: host() },
        );
        let snapshot = runtime.snapshot(TARGET).unwrap();
        assert_eq!(snapshot.runtime.state, RuntimeState::NotObserved);
        assert_eq!(
            availability_summary(&snapshot),
            AvailabilitySummary::DaemonAvailable
        );
    }

    #[test]
    fn daemon_available_runtime_unavailable_is_distinguishable() {
        let runtime = RemoteHostAvailabilityRuntime::default();
        let attempt = runtime.begin_attempt(TARGET, None, 10);
        runtime.observe(attempt.clone(), 11, AvailabilityEvent::TransportConnected);
        runtime.observe(attempt.clone(), 12, AvailabilityEvent::AuthSucceeded);
        runtime.observe(
            attempt.clone(),
            13,
            AvailabilityEvent::DaemonAvailable { identity: host() },
        );
        runtime.observe(
            attempt.clone(),
            14,
            AvailabilityEvent::RuntimeStarted {
                workspace_id: "workspace-a".to_string(),
            },
        );
        runtime.observe(
            attempt,
            15,
            AvailabilityEvent::RuntimeUnavailable {
                workspace_id: "workspace-a".to_string(),
                diagnostic: "initialize rejected".to_string(),
            },
        );
        let snapshot = runtime.snapshot(TARGET).unwrap();
        assert_eq!(snapshot.daemon, DaemonState::Available);
        assert_eq!(snapshot.runtime.state, RuntimeState::Unavailable);
        assert_eq!(
            availability_summary(&snapshot),
            AvailabilitySummary::CodexRuntimeUnavailable
        );
    }

    #[test]
    fn successful_runtime_gate_is_ready() {
        let runtime = ready_runtime();
        let snapshot = runtime.snapshot(TARGET).unwrap();
        assert_eq!(
            snapshot.runtime.workspace_id.as_deref(),
            Some("workspace-a")
        );
        assert_eq!(snapshot.last_successful_handshake_at, Some(105));
        assert_eq!(snapshot.last_runtime_ready_at, Some(107));
        assert_eq!(availability_summary(&snapshot), AvailabilitySummary::Ready);
    }

    #[test]
    fn disconnect_invalidates_current_ready_without_erasing_history() {
        let runtime = ready_runtime();
        let attempt = runtime.current_attempt(TARGET).unwrap();
        runtime.observe(
            attempt,
            200,
            AvailabilityEvent::Disconnected {
                diagnostic: "eof".to_string(),
            },
        );
        let snapshot = runtime.snapshot(TARGET).unwrap();
        assert_eq!(snapshot.transport, TransportState::Disconnected);
        assert_eq!(snapshot.auth, AuthState::Unknown);
        assert_eq!(snapshot.daemon, DaemonState::Unknown);
        assert_eq!(snapshot.runtime.state, RuntimeState::Unknown);
        assert_eq!(snapshot.last_successful_handshake_at, Some(105));
        assert_eq!(snapshot.last_runtime_ready_at, Some(107));
        assert_eq!(
            availability_summary(&snapshot),
            AvailabilitySummary::Disconnected
        );
    }

    #[test]
    fn reconnect_creates_new_attempt_and_old_attempt_cannot_overwrite_it() {
        let runtime = RemoteHostAvailabilityRuntime::default();
        let old = runtime.begin_attempt(TARGET, None, 10);
        let current = runtime.begin_attempt(TARGET, None, 20);
        assert!(current.attempt_id > old.attempt_id);
        runtime.observe(current.clone(), 21, AvailabilityEvent::TransportConnected);
        runtime.observe(
            old,
            99,
            AvailabilityEvent::EndpointUnreachable {
                diagnostic: "late failure".to_string(),
            },
        );
        let snapshot = runtime.snapshot(TARGET).unwrap();
        assert_eq!(snapshot.attempt_id, current.attempt_id);
        assert_eq!(snapshot.transport, TransportState::Connected);
        assert_eq!(snapshot.observed_at, 21);
    }

    #[test]
    fn settings_change_invalidates_attempt_without_claiming_failure_and_preserves_pin() {
        let runtime = ready_runtime();
        let next = runtime.invalidate_for_settings_change(TARGET, Some(host()), 300);
        let snapshot = runtime.snapshot(TARGET).unwrap();
        assert_eq!(snapshot.attempt_id, next.attempt_id);
        assert_eq!(snapshot.expected_remote_host_identity, Some(host()));
        assert_eq!(snapshot.transport, TransportState::Unknown);
        assert_eq!(snapshot.auth, AuthState::NotAttempted);
        assert_eq!(snapshot.daemon, DaemonState::NotObserved);
        assert_eq!(snapshot.runtime.state, RuntimeState::NotObserved);
        assert_eq!(
            availability_summary(&snapshot),
            AvailabilitySummary::Unknown
        );
    }

    #[test]
    fn historical_ready_timestamp_does_not_make_current_ready() {
        let runtime = ready_runtime();
        runtime.invalidate_for_settings_change(TARGET, Some(host()), 300);
        let snapshot = runtime.snapshot(TARGET).unwrap();
        assert_eq!(snapshot.last_runtime_ready_at, Some(107));
        assert_ne!(availability_summary(&snapshot), AvailabilitySummary::Ready);
    }

    #[test]
    fn runtime_disconnect_is_unknown() {
        let runtime = ready_runtime();
        runtime.observe(
            runtime.current_attempt(TARGET).unwrap(),
            400,
            AvailabilityEvent::Disconnected {
                diagnostic: "request channel closed".to_string(),
            },
        );
        assert_eq!(
            runtime.snapshot(TARGET).unwrap().runtime.state,
            RuntimeState::Unknown
        );
    }

    #[test]
    fn last_connected_at_is_not_current_truth() {
        let runtime = ready_runtime();
        runtime.observe(
            runtime.current_attempt(TARGET).unwrap(),
            400,
            AvailabilityEvent::Disconnected {
                diagnostic: "eof".to_string(),
            },
        );
        let snapshot = runtime.snapshot(TARGET).unwrap();
        assert_eq!(snapshot.last_successful_handshake_at, Some(105));
        assert_eq!(
            availability_summary(&snapshot),
            AvailabilitySummary::Disconnected
        );
    }

    #[test]
    fn connecting_never_implies_thread_absence() {
        let runtime = RemoteHostAvailabilityRuntime::default();
        runtime.begin_attempt(TARGET, None, 10);
        let value = serde_json::to_value(runtime.snapshot(TARGET).unwrap()).unwrap();
        assert_eq!(value["transport"], "CONNECTING");
        assert!(value.get("threadState").is_none());
        assert!(value.get("threadAbsent").is_none());
    }

    #[test]
    fn availability_failure_does_not_emit_thread_absent() {
        let runtime = RemoteHostAvailabilityRuntime::default();
        let attempt = runtime.begin_attempt(TARGET, None, 10);
        runtime.observe(
            attempt,
            11,
            AvailabilityEvent::EndpointUnreachable {
                diagnostic: "connection refused".to_string(),
            },
        );
        let value = serde_json::to_value(runtime.snapshot(TARGET).unwrap()).unwrap();
        assert!(value.get("threadState").is_none());
        assert!(value.get("projectionState").is_none());
    }

    #[test]
    fn availability_failure_does_not_emit_tombstone() {
        let runtime = RemoteHostAvailabilityRuntime::default();
        runtime.observe_unconfigured(TARGET, 10, "missing token");
        let value = serde_json::to_value(runtime.snapshot(TARGET).unwrap()).unwrap();
        assert!(value.get("tombstone").is_none());
        assert!(value.get("deletionState").is_none());
    }

    #[test]
    fn availability_does_not_change_workspace_project_or_token_accounting() {
        let runtime = ready_runtime();
        let value = serde_json::to_value(runtime.snapshot(TARGET).unwrap()).unwrap();
        assert!(value.get("workspaceKey").is_none());
        assert!(value.get("desktopProject").is_none());
        assert!(value.get("tokenUsage").is_none());
    }

    #[test]
    fn silent_partition_without_new_io_evidence_does_not_invent_a_failure() {
        let runtime = ready_runtime();
        let before = runtime.snapshot(TARGET).unwrap();
        let after = runtime.snapshot(TARGET).unwrap();

        assert_eq!(after, before);
        assert_eq!(availability_summary(&after), AvailabilitySummary::Ready);
    }
}
