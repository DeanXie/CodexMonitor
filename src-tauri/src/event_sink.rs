#[cfg(desktop)]
use tauri::Manager;
use tauri::{AppHandle, Emitter};

use crate::backend::events::{AppServerEvent, EventSink, TerminalExit, TerminalOutput};

#[derive(Clone)]
pub(crate) struct TauriEventSink {
    app: AppHandle,
}

impl TauriEventSink {
    pub(crate) fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl EventSink for TauriEventSink {
    fn emit_app_server_event(&self, event: AppServerEvent) {
        let state = self.app.state::<crate::state::AppState>();
        let generations = state.projection_freshness.generations_for_session(
            event.workspace_session_generation.clone(),
            event.app_server_connection_generation.clone(),
        );
        let _ = state.projection_freshness.record_event_if_current(
            crate::shared::projection_freshness::ProjectionFreshnessKey::thread_catalog(
                &event.workspace_id,
            ),
            generations,
            chrono::Utc::now().timestamp_millis(),
        );
        #[cfg(desktop)]
        {
            let _ = state.global_rollout_runtime.ingest_app_server_event(
                &event.workspace_id,
                &event.message,
                chrono::Utc::now().timestamp_millis(),
            );
        }
        let _ = self.app.emit("app-server-event", event);
    }

    fn emit_terminal_output(&self, event: TerminalOutput) {
        let _ = self.app.emit("terminal-output", event);
    }

    fn emit_terminal_exit(&self, event: TerminalExit) {
        let _ = self.app.emit("terminal-exit", event);
    }
}
