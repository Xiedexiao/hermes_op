//! Hermes Desktop - 库入口
//!
//! 导出所有模块和命令

pub mod backend;
pub mod commands;

// 重新导出兼容命令
pub use commands::compat::{
    check_environment, get_hermes_status, install_hermes, load_config, restart_hermes, save_config,
    start_hermes, stop_hermes, uninstall_hermes, upgrade_hermes,
};

// 重新导出新命令
pub use commands::agent_exchange::{
    agent_exchange_delete_message, agent_exchange_delete_remote_user,
    agent_exchange_draft_outbound, agent_exchange_export_bundle, agent_exchange_get_state,
    agent_exchange_import_bundle, agent_exchange_ingest_inbound, agent_exchange_list_messages,
    agent_exchange_list_remote_users, agent_exchange_run_folder_sync,
    agent_exchange_update_message_status, agent_exchange_upsert_remote_user,
};
pub use commands::app::{app_get_bootstrap, app_get_workspace_diagnostics};
pub use commands::council::{council_step_create, council_step_list};
pub use commands::execution::{
    execution_add_step_note, execution_approve_step, execution_complete_step,
    execution_confirm_skip_step, execution_list_by_mission, execution_list_desktop_handoff_queue,
    execution_mark_desktop_handoff_reviewed, execution_pause_step,
    execution_prepare_desktop_handoff, execution_rerun_step, execution_resume_step,
    execution_retry_step, execution_run_cli_step, execution_start_step,
};
pub use commands::gateway::{
    gateway_ingest_message, gateway_list_recent_conversations, gateway_list_recent_messages,
};
pub use commands::knowledge::{
    knowledge_fetch_url_preview, knowledge_import, knowledge_import_folder, knowledge_list,
    knowledge_source_list,
};
pub use commands::memory::{memory_record_create, memory_record_list, memory_record_search};
pub use commands::mission::{
    mission_create, mission_generate_plan, mission_get, mission_list, mission_set_pinned,
    mission_set_status, mission_update,
};
pub use commands::native_cua::{
    native_cua_apply_model_output, native_cua_execute_action, native_cua_export_audit_events,
    native_cua_export_trajectory, native_cua_invoke_model, native_cua_list_audit_events,
    native_cua_list_history, native_cua_observe, native_cua_plan_task,
    native_cua_prepare_model_turn, native_cua_preview_model_route, native_cua_probe,
    native_cua_record_info, native_cua_run_step, native_cua_start_session,
};
pub use commands::notifications::notifications_list;
pub use commands::parity::{
    parity_cron_create, parity_cron_list, parity_cron_run_now, parity_cron_runtime_status,
    parity_cron_runtime_tick, parity_cron_set_enabled, parity_get_catalog,
    parity_get_runtime_readiness, parity_mcp_list, parity_mcp_probe,
    parity_mcp_runtime_list_status, parity_mcp_runtime_reload, parity_mcp_runtime_start,
    parity_mcp_runtime_stop, parity_mcp_upsert, parity_quick_command_list,
    parity_quick_command_save, parity_save_provider_selection, parity_toolset_list,
    parity_toolset_save,
};
pub use commands::playbook::playbook_get;
pub use commands::runtime::{
    runtime_get_status, runtime_restart_engine, runtime_start_engine, runtime_stop_engine,
};
pub use commands::runtime_adapters::{
    runtime_adapter_execute_desktop_action, runtime_adapter_execute_skill_tool,
    runtime_adapter_export_audit_events, runtime_adapter_list_audit_events,
    runtime_adapter_probe_desktop_executor, runtime_adapter_run_gui_automation,
    runtime_adapter_summarize_trajectory_jsonl,
};
pub use commands::search::global_search;
pub use commands::sessions::{
    session_activate, session_clear_active, session_continue_latest, session_get,
    session_get_active, session_get_latest, session_list_recent, session_message_create,
    session_message_list, session_rename, session_replay_snapshot, session_resume_by_title,
    session_search,
};
pub use commands::settings::{settings_get, settings_save};
pub use commands::simulation::{
    simulation_compare_scenarios, simulation_create_scenario_run,
    simulation_export_template_bundle, simulation_export_template_bundle_audit_log,
    simulation_get_comparison_matrix, simulation_get_overview, simulation_import_template_bundle,
    simulation_list_external_saas_runs, simulation_list_handoff_policy_templates,
    simulation_list_high_fidelity_sandbox_runs, simulation_list_local_sandbox_runs,
    simulation_list_scenario_runs, simulation_list_scoring_formula_templates,
    simulation_list_template_bundle_audit_log, simulation_preflight_template_bundle_import,
    simulation_run_external_saas, simulation_run_high_fidelity_sandbox,
    simulation_run_local_sandbox, simulation_save_handoff_policy_template,
    simulation_save_scoring_formula_template,
};
pub use commands::skill_evolution::{
    skill_evolution_candidate_create, skill_evolution_candidate_generate,
    skill_evolution_candidate_list, skill_evolution_candidate_set_status,
};
pub use commands::skills::{
    skills_execute_runtime, skills_install, skills_invoke, skills_invoke_into_session, skills_list,
    skills_list_session_invocations, skills_marketplace_install, skills_marketplace_list,
    skills_marketplace_list_install_history, skills_search, skills_set_enabled, skills_view,
};
pub use commands::team_sync::{
    team_sync_check_access, team_sync_export_audit, team_sync_export_bundle, team_sync_get_state,
    team_sync_import_bundle, team_sync_run_folder_sync, team_sync_upsert_member,
};
pub use commands::terminal_backends::{
    terminal_backend_list_profiles, terminal_backend_list_status, terminal_backend_save_profile,
    terminal_backend_test_profile,
};
pub use commands::timeline::run_event_list;
pub use commands::trajectory::{
    trajectory_export_dataset, trajectory_list_local_rl_training_jobs,
    trajectory_run_local_rl_training,
};
pub use commands::turix_cua::{
    turix_cua_export_audit_events, turix_cua_list_audit_events, turix_cua_plan_command,
    turix_cua_probe, turix_cua_run,
};
pub use commands::voice::{
    voice_list_history, voice_list_providers, voice_process_speak_queue, voice_set_enabled,
    voice_speak, voice_speak_stub, voice_status, voice_transcribe, voice_transcribe_stub,
    voice_update_settings,
};
