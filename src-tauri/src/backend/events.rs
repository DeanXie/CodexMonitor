use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::shared::codex_core::thread_lifecycle_observation::AppServerConnectionGeneration;
use crate::shared::codex_core::writer_admission_observation::WorkspaceSessionGeneration;
use crate::shared::remote_host_identity::DaemonProcessGeneration;
use crate::shared::remote_request_provenance::RemoteTransportGeneration;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct AppServerEvent {
    pub(crate) workspace_id: String,
    pub(crate) message: Value,
    #[serde(rename = "daemonProcessGeneration")]
    pub(crate) daemon_process_generation: Option<DaemonProcessGeneration>,
    #[serde(rename = "remoteTransportGeneration")]
    pub(crate) remote_transport_generation: Option<RemoteTransportGeneration>,
    #[serde(rename = "workspaceSessionGeneration")]
    pub(crate) workspace_session_generation: WorkspaceSessionGeneration,
    #[serde(rename = "appServerConnectionGeneration")]
    pub(crate) app_server_connection_generation: AppServerConnectionGeneration,
}

impl AppServerEvent {
    pub(crate) fn for_session(
        workspace_id: impl Into<String>,
        message: Value,
        workspace_session_generation: WorkspaceSessionGeneration,
        app_server_connection_generation: AppServerConnectionGeneration,
    ) -> Self {
        Self {
            workspace_id: workspace_id.into(),
            message,
            daemon_process_generation: None,
            remote_transport_generation: None,
            workspace_session_generation,
            app_server_connection_generation,
        }
    }

    pub(crate) fn for_remote_delivery(
        mut self,
        daemon_process_generation: DaemonProcessGeneration,
        remote_transport_generation: RemoteTransportGeneration,
    ) -> Self {
        self.daemon_process_generation = Some(daemon_process_generation);
        self.remote_transport_generation = Some(remote_transport_generation);
        self
    }
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct TerminalOutput {
    #[serde(rename = "workspaceId")]
    pub(crate) workspace_id: String,
    #[serde(rename = "terminalId")]
    pub(crate) terminal_id: String,
    pub(crate) data: String,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct TerminalExit {
    #[serde(rename = "workspaceId")]
    pub(crate) workspace_id: String,
    #[serde(rename = "terminalId")]
    pub(crate) terminal_id: String,
}

pub(crate) trait EventSink: Clone + Send + Sync + 'static {
    fn emit_app_server_event(&self, event: AppServerEvent);
    fn emit_terminal_output(&self, event: TerminalOutput);
    fn emit_terminal_exit(&self, event: TerminalExit);
}

#[cfg(test)]
mod generation_tagged_event_contract_tests {
    use super::AppServerEvent;
    use crate::shared::codex_core::thread_lifecycle_observation::AppServerConnectionGeneration;
    use crate::shared::codex_core::writer_admission_observation::WorkspaceSessionGeneration;
    use crate::shared::remote_host_identity::DaemonProcessGeneration;
    use crate::shared::remote_request_provenance::RemoteTransportGeneration;
    use serde_json::json;
    use std::fs;

    fn session_generation(value: &str) -> WorkspaceSessionGeneration {
        WorkspaceSessionGeneration::new(value).expect("workspace generation")
    }

    fn connection_generation(value: &str) -> AppServerConnectionGeneration {
        AppServerConnectionGeneration::new(value).expect("connection generation")
    }

    #[test]
    fn app_server_event_contains_workspace_session_generation() {
        let event = AppServerEvent::for_session(
            "workspace-a",
            json!({ "method": "thread/updated" }),
            session_generation("workspace-generation-a"),
            connection_generation("connection-generation-a"),
        );
        let value = serde_json::to_value(event).expect("serialize event");

        assert_eq!(
            value["workspaceSessionGeneration"],
            "workspace-generation-a"
        );
    }

    #[test]
    fn app_server_event_contains_app_server_connection_generation() {
        let event = AppServerEvent::for_session(
            "workspace-a",
            json!({ "method": "thread/updated" }),
            session_generation("workspace-generation-a"),
            connection_generation("connection-generation-a"),
        );
        let value = serde_json::to_value(event).expect("serialize event");

        assert_eq!(
            value["appServerConnectionGeneration"],
            "connection-generation-a"
        );
    }

    #[test]
    fn remote_event_contains_daemon_process_generation() {
        let event = AppServerEvent::for_session(
            "workspace-a",
            json!({ "method": "thread/updated" }),
            session_generation("workspace-generation-a"),
            connection_generation("connection-generation-a"),
        )
        .for_remote_delivery(
            DaemonProcessGeneration::new("daemon-generation-a").expect("daemon generation"),
            RemoteTransportGeneration::new("transport-generation-a").expect("transport generation"),
        );
        let value = serde_json::to_value(event).expect("serialize event");

        assert_eq!(value["daemonProcessGeneration"], "daemon-generation-a");
        assert_eq!(value["remoteTransportGeneration"], "transport-generation-a");
    }

    #[test]
    fn local_app_event_does_not_invent_remote_transport_generation() {
        let event = AppServerEvent::for_session(
            "workspace-a",
            json!({ "method": "thread/updated" }),
            session_generation("workspace-generation-a"),
            connection_generation("connection-generation-a"),
        );
        let value = serde_json::to_value(event).expect("serialize event");

        assert!(value["daemonProcessGeneration"].is_null());
        assert!(value["remoteTransportGeneration"].is_null());
    }

    #[test]
    fn event_envelope_contains_no_remote_client_identity() {
        let value = serde_json::to_value(AppServerEvent::for_session(
            "workspace-a",
            json!({ "method": "thread/updated" }),
            session_generation("workspace-generation-a"),
            connection_generation("connection-generation-a"),
        ))
        .expect("serialize event");
        let text = serde_json::to_string(&value).expect("serialize text");

        assert!(!text.contains("remoteClientIdentity"));
    }

    #[test]
    fn event_envelope_contains_no_owner_or_lease() {
        let value = serde_json::to_value(AppServerEvent::for_session(
            "workspace-a",
            json!({ "method": "thread/updated" }),
            session_generation("workspace-generation-a"),
            connection_generation("connection-generation-a"),
        ))
        .expect("serialize event");
        let text = serde_json::to_string(&value).expect("serialize text");

        for forbidden in ["owner", "lease", "free", "available", "released"] {
            assert!(!text.to_ascii_lowercase().contains(forbidden));
        }
    }

    #[test]
    fn generation_tagged_event_fixtures_freeze_contract() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../docs/fixtures/generation-tagged-events");
        let required = [
            "current-local-app-event.json",
            "current-remote-event.json",
            "stale-transport-event.json",
            "stale-workspace-session-event.json",
            "stale-app-server-event.json",
            "stale-daemon-event.json",
            "missing-generation-event.json",
            "event-before-hydration.json",
            "event-on-current-coverage.json",
            "same-payload-different-generation.json",
            "multi-client-same-shared-event.json",
        ];

        for name in required {
            let text = fs::read_to_string(root.join(name))
                .unwrap_or_else(|error| panic!("read fixture {name}: {error}"));
            let _: serde_json::Value = serde_json::from_str(&text)
                .unwrap_or_else(|error| panic!("parse fixture {name}: {error}"));
            let lower = text.to_ascii_lowercase();
            for forbidden in [
                "remoteclientidentity",
                "eventgeneration",
                "projectiongeneration",
                "recoverygeneration",
                "clientgeneration",
                "writerowner",
                "subscriptionowner",
                "leaseid",
                "forcetakeover",
            ] {
                assert!(!lower.contains(forbidden), "{name} contains {forbidden}");
            }
        }
    }
}
