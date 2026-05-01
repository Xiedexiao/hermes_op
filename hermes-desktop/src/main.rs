#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

//! Hermes Desktop - Tauri 主入口
//!
//! 这是 Tauri 应用的入口点

use hermes_desktop::backend::{
    ControlApiConfig, ControlApiServerHandle, Database, ParityCronRuntimeService,
    ParityMcpRuntimeManager, create_app_state, maybe_run_engine_daemon_from_args,
};
use std::time::Duration;
use tauri::Manager;

fn main() {
    if maybe_run_engine_daemon_from_args(std::env::args_os())
        .expect("Failed to enter engine daemon mode")
    {
        return;
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        // 初始化应用状态
        .setup(|app| {
            // 创建数据库
            let app_state = create_app_state().expect("Failed to create app state");
            let db_path = {
                let state = app_state.read();
                state.db_path.clone()
            };
            let db = Database::new(&db_path).expect("Failed to create database");
            let control_api = ControlApiServerHandle::start(
                db.clone(),
                app_state.clone(),
                ControlApiConfig::default(),
            )
            .expect("Failed to start control API server");

            // 管理状态
            app.manage(app_state);
            app.manage(db.clone());
            app.manage(ParityMcpRuntimeManager::default());
            app.manage(control_api);

            tauri::async_runtime::spawn(async move {
                let runtime = ParityCronRuntimeService::new(db);
                loop {
                    if let Err(err) = runtime.poll_once() {
                        tracing::error!("Parity cron runtime poll failed: {}", err);
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            });

            tracing::info!("Hermes Operator initialized");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 兼容命令
            hermes_desktop::check_environment,
            hermes_desktop::get_hermes_status,
            hermes_desktop::install_hermes,
            hermes_desktop::uninstall_hermes,
            hermes_desktop::upgrade_hermes,
            hermes_desktop::start_hermes,
            hermes_desktop::stop_hermes,
            hermes_desktop::restart_hermes,
            hermes_desktop::load_config,
            hermes_desktop::save_config,
            // 新命令
            hermes_desktop::agent_exchange_get_state,
            hermes_desktop::agent_exchange_list_remote_users,
            hermes_desktop::agent_exchange_upsert_remote_user,
            hermes_desktop::agent_exchange_delete_remote_user,
            hermes_desktop::agent_exchange_list_messages,
            hermes_desktop::agent_exchange_draft_outbound,
            hermes_desktop::agent_exchange_ingest_inbound,
            hermes_desktop::agent_exchange_export_bundle,
            hermes_desktop::agent_exchange_import_bundle,
            hermes_desktop::agent_exchange_update_message_status,
            hermes_desktop::agent_exchange_delete_message,
            hermes_desktop::agent_exchange_run_folder_sync,
            hermes_desktop::app_get_bootstrap,
            hermes_desktop::app_get_workspace_diagnostics,
            hermes_desktop::council_step_list,
            hermes_desktop::council_step_create,
            hermes_desktop::execution_list_by_mission,
            hermes_desktop::execution_list_desktop_handoff_queue,
            hermes_desktop::execution_mark_desktop_handoff_reviewed,
            hermes_desktop::execution_add_step_note,
            hermes_desktop::execution_prepare_desktop_handoff,
            hermes_desktop::execution_approve_step,
            hermes_desktop::execution_start_step,
            hermes_desktop::execution_pause_step,
            hermes_desktop::execution_complete_step,
            hermes_desktop::execution_retry_step,
            hermes_desktop::execution_resume_step,
            hermes_desktop::execution_rerun_step,
            hermes_desktop::execution_confirm_skip_step,
            hermes_desktop::execution_run_cli_step,
            hermes_desktop::gateway_ingest_message,
            hermes_desktop::gateway_list_recent_conversations,
            hermes_desktop::gateway_list_recent_messages,
            hermes_desktop::knowledge_fetch_url_preview,
            hermes_desktop::knowledge_import,
            hermes_desktop::knowledge_import_folder,
            hermes_desktop::knowledge_list,
            hermes_desktop::knowledge_source_list,
            hermes_desktop::memory_record_list,
            hermes_desktop::memory_record_create,
            hermes_desktop::memory_record_search,
            hermes_desktop::mission_list,
            hermes_desktop::mission_create,
            hermes_desktop::mission_update,
            hermes_desktop::mission_set_pinned,
            hermes_desktop::mission_set_status,
            hermes_desktop::mission_generate_plan,
            hermes_desktop::mission_get,
            hermes_desktop::notifications_list,
            hermes_desktop::parity_get_catalog,
            hermes_desktop::parity_get_runtime_readiness,
            hermes_desktop::parity_save_provider_selection,
            hermes_desktop::parity_toolset_list,
            hermes_desktop::parity_toolset_save,
            hermes_desktop::parity_cron_list,
            hermes_desktop::parity_cron_create,
            hermes_desktop::parity_cron_set_enabled,
            hermes_desktop::parity_cron_run_now,
            hermes_desktop::parity_cron_runtime_status,
            hermes_desktop::parity_cron_runtime_tick,
            hermes_desktop::parity_mcp_list,
            hermes_desktop::parity_mcp_probe,
            hermes_desktop::parity_mcp_upsert,
            hermes_desktop::parity_mcp_runtime_list_status,
            hermes_desktop::parity_mcp_runtime_start,
            hermes_desktop::parity_mcp_runtime_stop,
            hermes_desktop::parity_mcp_runtime_reload,
            hermes_desktop::parity_quick_command_list,
            hermes_desktop::parity_quick_command_save,
            hermes_desktop::session_list_recent,
            hermes_desktop::session_get,
            hermes_desktop::session_get_active,
            hermes_desktop::session_get_latest,
            hermes_desktop::session_activate,
            hermes_desktop::session_clear_active,
            hermes_desktop::session_continue_latest,
            hermes_desktop::session_message_list,
            hermes_desktop::session_message_create,
            hermes_desktop::session_search,
            hermes_desktop::session_replay_snapshot,
            hermes_desktop::session_resume_by_title,
            hermes_desktop::session_rename,
            hermes_desktop::global_search,
            hermes_desktop::settings_get,
            hermes_desktop::settings_save,
            hermes_desktop::skill_evolution_candidate_list,
            hermes_desktop::skill_evolution_candidate_create,
            hermes_desktop::skill_evolution_candidate_generate,
            hermes_desktop::skill_evolution_candidate_set_status,
            hermes_desktop::skills_list,
            hermes_desktop::skills_search,
            hermes_desktop::skills_view,
            hermes_desktop::skills_install,
            hermes_desktop::skills_marketplace_list,
            hermes_desktop::skills_marketplace_install,
            hermes_desktop::skills_marketplace_list_install_history,
            hermes_desktop::skills_set_enabled,
            hermes_desktop::skills_execute_runtime,
            hermes_desktop::skills_invoke,
            hermes_desktop::skills_invoke_into_session,
            hermes_desktop::skills_list_session_invocations,
            hermes_desktop::team_sync_get_state,
            hermes_desktop::team_sync_upsert_member,
            hermes_desktop::team_sync_check_access,
            hermes_desktop::team_sync_export_bundle,
            hermes_desktop::team_sync_export_audit,
            hermes_desktop::team_sync_import_bundle,
            hermes_desktop::team_sync_run_folder_sync,
            hermes_desktop::terminal_backend_list_profiles,
            hermes_desktop::terminal_backend_save_profile,
            hermes_desktop::terminal_backend_list_status,
            hermes_desktop::terminal_backend_test_profile,
            hermes_desktop::playbook_get,
            hermes_desktop::simulation_get_overview,
            hermes_desktop::simulation_create_scenario_run,
            hermes_desktop::simulation_list_scenario_runs,
            hermes_desktop::simulation_compare_scenarios,
            hermes_desktop::simulation_get_comparison_matrix,
            hermes_desktop::simulation_run_local_sandbox,
            hermes_desktop::simulation_list_local_sandbox_runs,
            hermes_desktop::simulation_run_external_saas,
            hermes_desktop::simulation_list_external_saas_runs,
            hermes_desktop::simulation_run_high_fidelity_sandbox,
            hermes_desktop::simulation_list_high_fidelity_sandbox_runs,
            hermes_desktop::simulation_list_handoff_policy_templates,
            hermes_desktop::simulation_save_handoff_policy_template,
            hermes_desktop::simulation_list_scoring_formula_templates,
            hermes_desktop::simulation_save_scoring_formula_template,
            hermes_desktop::simulation_export_template_bundle,
            hermes_desktop::simulation_export_template_bundle_audit_log,
            hermes_desktop::simulation_import_template_bundle,
            hermes_desktop::simulation_list_template_bundle_audit_log,
            hermes_desktop::simulation_preflight_template_bundle_import,
            hermes_desktop::run_event_list,
            hermes_desktop::trajectory_export_dataset,
            hermes_desktop::trajectory_list_local_rl_training_jobs,
            hermes_desktop::trajectory_run_local_rl_training,
            hermes_desktop::runtime_get_status,
            hermes_desktop::runtime_start_engine,
            hermes_desktop::runtime_stop_engine,
            hermes_desktop::runtime_restart_engine,
            hermes_desktop::runtime_adapter_execute_skill_tool,
            hermes_desktop::runtime_adapter_probe_desktop_executor,
            hermes_desktop::runtime_adapter_execute_desktop_action,
            hermes_desktop::runtime_adapter_run_gui_automation,
            hermes_desktop::runtime_adapter_summarize_trajectory_jsonl,
            hermes_desktop::runtime_adapter_list_audit_events,
            hermes_desktop::runtime_adapter_export_audit_events,
            hermes_desktop::native_cua_probe,
            hermes_desktop::native_cua_start_session,
            hermes_desktop::native_cua_preview_model_route,
            hermes_desktop::native_cua_observe,
            hermes_desktop::native_cua_execute_action,
            hermes_desktop::native_cua_list_audit_events,
            hermes_desktop::native_cua_export_audit_events,
            hermes_desktop::native_cua_plan_task,
            hermes_desktop::native_cua_run_step,
            hermes_desktop::native_cua_list_history,
            hermes_desktop::native_cua_record_info,
            hermes_desktop::native_cua_export_trajectory,
            hermes_desktop::native_cua_prepare_model_turn,
            hermes_desktop::native_cua_apply_model_output,
            hermes_desktop::native_cua_invoke_model,
            hermes_desktop::turix_cua_probe,
            hermes_desktop::turix_cua_plan_command,
            hermes_desktop::turix_cua_run,
            hermes_desktop::turix_cua_list_audit_events,
            hermes_desktop::turix_cua_export_audit_events,
            hermes_desktop::voice_update_settings,
            hermes_desktop::voice_list_providers,
            hermes_desktop::voice_status,
            hermes_desktop::voice_set_enabled,
            hermes_desktop::voice_transcribe,
            hermes_desktop::voice_transcribe_stub,
            hermes_desktop::voice_speak,
            hermes_desktop::voice_speak_stub,
            hermes_desktop::voice_list_history,
            hermes_desktop::voice_process_speak_queue,
        ])
        .run(tauri::generate_context!())
        .expect("启动 Hermes Operator 失败");
}
