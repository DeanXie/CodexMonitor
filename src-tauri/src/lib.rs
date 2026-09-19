#[cfg(desktop)]
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;
#[cfg(desktop)]
use tauri::RunEvent;
#[cfg(target_os = "macos")]
use tauri::WindowEvent;

mod backend;
mod bootstrap;
mod codex;
mod daemon_binary;
mod dictation;
mod event_sink;
mod files;
mod git;
mod git_utils;
#[cfg(desktop)]
mod global_sources;
mod local_usage;
#[cfg(desktop)]
mod menu;
#[cfg(not(desktop))]
#[path = "menu_mobile.rs"]
mod menu;
mod notifications;
mod prompts;
mod remote_backend;
mod rules;
mod settings;
mod shared;
mod state;
mod storage;
mod tailscale;
#[cfg(desktop)]
mod terminal;
#[cfg(not(desktop))]
#[path = "terminal_mobile.rs"]
mod terminal;
mod tray;
mod types;
mod utils;
mod window;
mod workspaces;

#[cfg(desktop)]
static EXIT_CLEANUP_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

#[cfg(desktop)]
fn keep_daemon_running_after_close(app_handle: &tauri::AppHandle) -> bool {
    let Some(state) = app_handle.try_state::<state::AppState>() else {
        return false;
    };
    tauri::async_runtime::block_on(async {
        state
            .app_settings
            .lock()
            .await
            .keep_daemon_running_after_app_close
    })
}

#[cfg(desktop)]
async fn stop_managed_daemons_for_exit(app_handle: tauri::AppHandle) {
    if let Some(state) = app_handle.try_state::<state::AppState>() {
        global_sources::shutdown(&app_handle).await;
        let _ = tailscale::tailscale_daemon_stop(state).await;
    }
}

#[tauri::command]
fn is_mobile_runtime() -> bool {
    cfg!(any(target_os = "ios", target_os = "android"))
}

#[cfg(desktop)]
fn is_bootstrap_invoke(command: &str) -> bool {
    matches!(
        command,
        "get_bootstrap_status" | "activate_fresh_profile" | "recover_profile_activation"
    )
}

#[cfg(desktop)]
fn business_invoke_allowed(command: &str, runtime_ready: bool) -> bool {
    runtime_ready || is_bootstrap_invoke(command)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    {
        // Avoid WebKit compositing issues on NVIDIA Linux setups (GBM buffer errors).
        if std::env::var_os("__NV_PRIME_RENDER_OFFLOAD").is_none() {
            std::env::set_var("__NV_PRIME_RENDER_OFFLOAD", "1");
        }
        let is_wayland = std::env::var("XDG_SESSION_TYPE")
            .map(|session| session.eq_ignore_ascii_case("wayland"))
            .unwrap_or(false)
            || std::env::var_os("WAYLAND_DISPLAY").is_some();
        let has_nvidia = std::path::Path::new("/proc/driver/nvidia/version").exists();
        if is_wayland && has_nvidia && std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none()
        {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
        let is_x11 = !is_wayland && std::env::var_os("DISPLAY").is_some();
        // Work around sporadic blank WebKitGTK renders on X11 by disabling compositing mode.
        // Keep Wayland untouched because this can interfere with input behavior on some setups.
        if is_x11 && std::env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_none() {
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
    }

    #[cfg(desktop)]
    let builder = tauri::Builder::default()
        .manage(menu::MenuItemRegistry::<tauri::Wry>::default())
        .manage(tray::TrayState::default())
        .on_menu_event(menu::handle_menu_event)
        .enable_macos_default_menu(false)
        .menu(menu::build_menu);

    #[cfg(not(desktop))]
    let builder = tauri::Builder::default();

    let builder = builder
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            #[cfg(target_os = "macos")]
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            #[cfg(desktop)]
            let normal_load_allowed = {
                let target_root = app.path().app_data_dir().map_err(|error| {
                    format!("failed to resolve activated app data root: {error}")
                })?;
                let bootstrap_state = bootstrap::BootstrapState::inspect(target_root.clone());
                let bootstrap_status = bootstrap_state.initial_status();
                let mut candidate_state = None;
                if bootstrap_status
                    .as_ref()
                    .map(|status| {
                        status.inspection.disposition
                            == shared::startup_activation::StartupDisposition::RuntimeValidationRequired
                    })
                    .unwrap_or(false)
                {
                    if let Err(error) = bootstrap_state.begin_runtime_validation() {
                        let _ = bootstrap_state.fail_runtime_validation(&error);
                    } else {
                        match state::AppState::load_for_runtime_validation(target_root) {
                            Ok((state, metadata)) => {
                                match bootstrap_state
                                    .complete_runtime_validation(&metadata.transaction_id)
                                {
                                    Ok(()) => candidate_state = Some(state),
                                    Err(_) => {}
                                }
                            }
                            Err(error) => {
                                let _ = bootstrap_state.fail_runtime_validation(&error);
                            }
                        }
                    }
                }
                let normal_load_allowed = bootstrap_state
                    .business_access_allowed()
                    .unwrap_or(false);
                app.manage(bootstrap_state);
                if let Some(state) = candidate_state {
                    app.manage(state);
                    #[cfg(desktop)]
                    global_sources::start(&app.handle())?;
                }
                normal_load_allowed
            };

            #[cfg(not(desktop))]
            let normal_load_allowed = {
                app.manage(state::AppState::load_mobile(&app.handle())?);
                true
            };
            #[cfg(target_os = "macos")]
            {
                let tray_state = app.state::<tray::TrayState>();
                tray::initialize(&app.handle(), tray_state.inner())?;
            }
            #[cfg(target_os = "windows")]
            {
                if let Some(main_window) = app.get_webview_window("main") {
                    let _ = main_window.set_decorations(false);
                    // Keep menu accelerators wired while suppressing a visible native menu bar.
                    let _ = main_window.hide_menu();
                }
            }
            #[cfg(desktop)]
            {
                if normal_load_allowed {
                    let app_handle = app.handle().clone();
                    tauri::async_runtime::spawn(async move {
                        let Some(state) = app_handle.try_state::<state::AppState>() else {
                            return;
                        };
                        let settings = state.app_settings.lock().await.clone();
                        if matches!(
                            settings.remote_backend_provider,
                            crate::types::RemoteBackendProvider::Tcp
                        ) {
                            if matches!(settings.backend_mode, crate::types::BackendMode::Remote) {
                                // Remote mode: ensure daemon is up and version-current.
                                let state = app_handle.state::<state::AppState>();
                                let _ = tailscale::tailscale_daemon_start(state).await;
                            } else {
                                // Local mode: only enforce version if daemon is already running.
                                let state = app_handle.state::<state::AppState>();
                                if let Ok(status) = tailscale::tailscale_daemon_status(state).await
                                {
                                    if matches!(status.state, crate::types::TcpDaemonState::Running)
                                    {
                                        let state = app_handle.state::<state::AppState>();
                                        let _ = tailscale::tailscale_daemon_start(state).await;
                                    }
                                }
                            }
                        }
                    });
                }
            }
            #[cfg(target_os = "ios")]
            {
                if let Some(main_webview) = app.get_webview_window("main") {
                    let _ = window::configure_ios_webview_edge_to_edge(&main_webview);
                }
            }
            Ok(())
        });

    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_window_state::Builder::default().build());

    let app = builder
        .plugin(tauri_plugin_liquid_glass::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(|invoke: tauri::ipc::Invoke<tauri::Wry>| {
            #[cfg(desktop)]
            {
                let command = invoke.message.command();
                let ready = invoke
                    .message
                    .webview_ref()
                    .try_state::<bootstrap::BootstrapState>()
                    .and_then(|state| state.business_access_allowed().ok())
                    .unwrap_or(false);
                if !business_invoke_allowed(command, ready) {
                    invoke.resolver.reject(
                        "normal business IPC is unavailable until runtime validation succeeds",
                    );
                    return true;
                }
            }
            let handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = tauri::generate_handler![
                bootstrap::get_bootstrap_status,
                bootstrap::activate_fresh_profile,
                bootstrap::recover_profile_activation,
                settings::get_app_settings,
                settings::update_app_settings,
                settings::get_codex_config_path,
                remote_backend::get_remote_host_availability,
                files::file_read,
                files::file_write,
                files::read_image_as_data_url,
                files::write_text_file,
                codex::get_config_model,
                menu::menu_set_accelerators,
                tray::set_tray_recent_threads,
                tray::set_tray_session_usage,
                codex::codex_doctor,
                codex::codex_update,
                workspaces::list_workspaces,
                workspaces::is_workspace_path_dir,
                workspaces::add_workspace,
                workspaces::add_workspace_from_git_url,
                workspaces::add_clone,
                workspaces::add_worktree,
                workspaces::worktree_setup_status,
                workspaces::worktree_setup_mark_ran,
                workspaces::remove_workspace,
                workspaces::remove_worktree,
                workspaces::rename_worktree,
                workspaces::rename_worktree_upstream,
                workspaces::apply_worktree_changes,
                workspaces::update_workspace_settings,
                workspaces::set_workspace_runtime_codex_args,
                codex::start_thread,
                codex::get_creation_context,
                codex::get_creation_intent_status,
                codex::get_first_turn_intent_status,
                codex::send_user_message,
                codex::turn_steer,
                codex::turn_interrupt,
                codex::start_review,
                codex::respond_to_server_request,
                codex::remember_approval_rule,
                codex::generate_commit_message,
                codex::generate_run_metadata,
                codex::generate_agent_description,
                codex::resume_thread,
                codex::get_writer_admission_observation,
                codex::get_authoritative_observation_snapshot,
                codex::get_projection_freshness,
                codex::read_thread,
                codex::thread_live_subscribe,
                codex::thread_live_unsubscribe,
                codex::thread_upstream_unsubscribe,
                codex::fork_thread,
                codex::list_threads,
                codex::list_mcp_server_status,
                codex::archive_thread,
                codex::delete_thread,
                codex::compact_thread,
                codex::set_thread_name,
                codex::collaboration_mode_list,
                workspaces::connect_workspace,
                git::get_git_status,
                git::init_git_repo,
                git::create_github_repo,
                git::list_git_roots,
                git::get_git_diffs,
                git::get_git_log,
                git::get_git_commit_diff,
                git::get_git_remote,
                git::stage_git_file,
                git::stage_git_all,
                git::unstage_git_file,
                git::revert_git_file,
                git::revert_git_all,
                git::commit_git,
                git::push_git,
                git::pull_git,
                git::fetch_git,
                git::sync_git,
                git::get_github_issues,
                git::get_github_pull_requests,
                git::get_github_pull_request_diff,
                git::get_github_pull_request_comments,
                git::checkout_github_pull_request,
                workspaces::list_workspace_files,
                workspaces::read_workspace_file,
                workspaces::open_workspace_in,
                workspaces::get_open_app_icon,
                git::list_git_branches,
                git::checkout_git_branch,
                git::create_git_branch,
                codex::model_list,
                codex::experimental_feature_list,
                codex::set_codex_feature_flag,
                codex::get_agents_settings,
                codex::set_agents_core_settings,
                codex::create_agent,
                codex::update_agent,
                codex::delete_agent,
                codex::read_agent_config_toml,
                codex::write_agent_config_toml,
                codex::account_rate_limits,
                codex::account_read,
                codex::codex_login,
                codex::codex_login_cancel,
                codex::skills_list,
                codex::apps_list,
                prompts::prompts_list,
                prompts::prompts_create,
                prompts::prompts_update,
                prompts::prompts_delete,
                prompts::prompts_move,
                prompts::prompts_workspace_dir,
                prompts::prompts_global_dir,
                terminal::terminal_open,
                terminal::terminal_write,
                terminal::terminal_resize,
                terminal::terminal_close,
                dictation::dictation_model_status,
                dictation::dictation_download_model,
                dictation::dictation_cancel_download,
                dictation::dictation_remove_model,
                dictation::dictation_start,
                dictation::dictation_request_permission,
                dictation::dictation_stop,
                dictation::dictation_cancel,
                local_usage::local_usage_snapshot,
                #[cfg(desktop)]
                global_sources::snapshot::global_source_snapshot,
                notifications::is_macos_debug_build,
                notifications::app_build_type,
                notifications::send_notification_fallback,
                tailscale::tailscale_status,
                tailscale::tailscale_daemon_command_preview,
                tailscale::tailscale_daemon_start,
                tailscale::tailscale_daemon_stop,
                tailscale::tailscale_daemon_status,
                is_mobile_runtime
            ];
            handler(invoke)
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application");

    app.run(|app_handle, event| {
        #[cfg(desktop)]
        if let RunEvent::ExitRequested { api, .. } = event {
            if !EXIT_CLEANUP_IN_PROGRESS.load(Ordering::SeqCst)
                && !keep_daemon_running_after_close(app_handle)
            {
                api.prevent_exit();
                EXIT_CLEANUP_IN_PROGRESS.store(true, Ordering::SeqCst);
                let app_handle = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    stop_managed_daemons_for_exit(app_handle.clone()).await;
                    app_handle.exit(0);
                });
            }
            return;
        }

        #[cfg(target_os = "macos")]
        if let RunEvent::Reopen { .. } = event {
            if let Some(window) = app_handle.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    });
}

#[cfg(all(test, desktop))]
mod startup_gate_tests {
    use super::business_invoke_allowed;

    #[test]
    fn validating_process_only_allows_bootstrap_commands() {
        assert!(business_invoke_allowed("get_bootstrap_status", false));
        assert!(business_invoke_allowed("activate_fresh_profile", false));
        assert!(business_invoke_allowed("recover_profile_activation", false));
        assert!(!business_invoke_allowed("list_workspaces", false));
        assert!(!business_invoke_allowed("resume_thread", false));
        assert!(!business_invoke_allowed("file_write", false));
    }

    #[test]
    fn ready_process_allows_registered_business_commands() {
        assert!(business_invoke_allowed("list_workspaces", true));
        assert!(business_invoke_allowed("resume_thread", true));
    }
}
