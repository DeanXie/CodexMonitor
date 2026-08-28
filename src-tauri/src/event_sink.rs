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
        #[cfg(desktop)]
        {
            let state = self.app.state::<crate::state::AppState>();
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
