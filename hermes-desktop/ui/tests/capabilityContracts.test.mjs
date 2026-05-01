import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const testDir = dirname(fileURLToPath(import.meta.url));
const uiRoot = resolve(testDir, '..');
const repoRoot = resolve(uiRoot, '..');
const tauriSource = readFileSync(resolve(uiRoot, 'src/lib/tauri.ts'), 'utf8');
const mainSource = readFileSync(resolve(repoRoot, 'src/main.rs'), 'utf8');
const appSource = readFileSync(resolve(uiRoot, 'src/app/App.tsx'), 'utf8');
const sidebarSource = readFileSync(resolve(uiRoot, 'src/components/SidebarNav.tsx'), 'utf8');
const contextPanelSource = readFileSync(resolve(uiRoot, 'src/components/ContextPanel.tsx'), 'utf8');
const agentExchangePageSource = readFileSync(
  resolve(uiRoot, 'src/routes/AgentExchangePage.tsx'),
  'utf8',
);
const runtimePageSource = readFileSync(resolve(uiRoot, 'src/routes/RuntimePage.tsx'), 'utf8');
const simulationPageSource = readFileSync(resolve(uiRoot, 'src/routes/SimulationPage.tsx'), 'utf8');
const skillsPageSource = readFileSync(resolve(uiRoot, 'src/routes/SkillsPage.tsx'), 'utf8');
const packageJson = JSON.parse(readFileSync(resolve(uiRoot, 'package.json'), 'utf8'));

const capabilityCommands = [
  ['agentExchangeGetState', 'agent_exchange_get_state'],
  ['agentExchangeListMessages', 'agent_exchange_list_messages'],
  ['agentExchangeDraftOutbound', 'agent_exchange_draft_outbound'],
  ['agentExchangeIngestInbound', 'agent_exchange_ingest_inbound'],
  ['agentExchangeListRemoteUsers', 'agent_exchange_list_remote_users'],
  ['agentExchangeUpsertRemoteUser', 'agent_exchange_upsert_remote_user'],
  ['agentExchangeDeleteRemoteUser', 'agent_exchange_delete_remote_user'],
  ['agentExchangeExportBundle', 'agent_exchange_export_bundle'],
  ['agentExchangeImportBundle', 'agent_exchange_import_bundle'],
  ['agentExchangeUpdateMessageStatus', 'agent_exchange_update_message_status'],
  ['agentExchangeDeleteMessage', 'agent_exchange_delete_message'],
  ['agentExchangeRunFolderSync', 'agent_exchange_run_folder_sync'],
  ['runtimeAdapterRunGuiAutomation', 'runtime_adapter_run_gui_automation'],
  ['skillsMarketplaceList', 'skills_marketplace_list'],
  ['skillsMarketplaceInstall', 'skills_marketplace_install'],
  ['skillsMarketplaceListInstallHistory', 'skills_marketplace_list_install_history'],
  ['simulationRunExternalSaas', 'simulation_run_external_saas'],
  ['simulationRunHighFidelitySandbox', 'simulation_run_high_fidelity_sandbox'],
  ['simulationListExternalSaasRuns', 'simulation_list_external_saas_runs'],
  ['simulationListHighFidelitySandboxRuns', 'simulation_list_high_fidelity_sandbox_runs'],
  ['trajectoryListLocalRlTrainingJobs', 'trajectory_list_local_rl_training_jobs'],
  ['trajectoryRunLocalRlTraining', 'trajectory_run_local_rl_training'],
];

test('package test script runs all node test files', () => {
  assert.equal(packageJson.scripts?.test, 'node --test tests/*.test.mjs');
});

test('new capability wrappers invoke registered Tauri command names', () => {
  for (const [functionName, commandName] of capabilityCommands) {
    assert.match(
      tauriSource,
      new RegExp(`export\\s+async\\s+function\\s+${functionName}\\b`),
      `${functionName} wrapper should be exported`,
    );
    assert.match(
      tauriSource,
      new RegExp(`invoke(?:Optional)?<[^>]+>\\("${commandName}"`),
      `${functionName} should invoke ${commandName}`,
    );
    assert.match(
      mainSource,
      new RegExp(`hermes_desktop::${commandName}\\b`),
      `${commandName} should be registered in Tauri invoke_handler`,
    );
  }
});

test('new UI surface does not export legacy voice stub wrappers', () => {
  assert.doesNotMatch(tauriSource, /export\s+interface\s+VoiceTranscribeStubRequest\b/);
  assert.doesNotMatch(tauriSource, /export\s+interface\s+VoiceSpeakStubRequest\b/);
  assert.doesNotMatch(tauriSource, /export\s+async\s+function\s+voiceTranscribeStub\b/);
  assert.doesNotMatch(tauriSource, /export\s+async\s+function\s+voiceSpeakStub\b/);
});

test('agent exchange folder sync wrapper targets the agreed backend command name', () => {
  assert.match(
    tauriSource,
    /export\s+async\s+function\s+agentExchangeRunFolderSync\b/,
    'agentExchangeRunFolderSync wrapper should be exported',
  );
  assert.match(
    tauriSource,
    /invoke(?:Optional)?<[^>]+>\("agent_exchange_run_folder_sync"/,
    'agentExchangeRunFolderSync should invoke agent_exchange_run_folder_sync',
  );
  assert.match(
    mainSource,
    /hermes_desktop::agent_exchange_run_folder_sync\b/,
    'agent_exchange_run_folder_sync should be registered in Tauri invoke_handler',
  );
});

test('trajectory local RL training jobs wrapper targets the agreed backend command name', () => {
  assert.match(
    tauriSource,
    /export\s+async\s+function\s+trajectoryListLocalRlTrainingJobs\b/,
    'trajectoryListLocalRlTrainingJobs wrapper should be exported',
  );
  assert.match(
    tauriSource,
    /invoke(?:Optional)?<[^>]+>\("trajectory_list_local_rl_training_jobs"/,
    'trajectoryListLocalRlTrainingJobs should invoke trajectory_list_local_rl_training_jobs',
  );
  assert.match(
    mainSource,
    /hermes_desktop::trajectory_list_local_rl_training_jobs\b/,
    'trajectory_list_local_rl_training_jobs should be registered in Tauri invoke_handler',
  );
});

test('trajectory local RL training request and result types persist target remote user metadata', () => {
  assert.match(
    tauriSource,
    /export\s+interface\s+TrajectoryRlTrainingRequest\s*\{[\s\S]*target_remote_user_id\?: string \| null;/,
    'TrajectoryRlTrainingRequest should expose optional target_remote_user_id metadata',
  );
  assert.match(
    tauriSource,
    /export\s+interface\s+TrajectoryRlTrainingResult\s*\{[\s\S]*target_remote_user_id\?: string \| null;/,
    'TrajectoryRlTrainingResult should expose persisted optional target_remote_user_id metadata',
  );
  assert.match(
    tauriSource,
    /export\s+interface\s+TrajectoryRlTrainingJobListRequest\s*\{[^}]*target_remote_user_id\?: string \| null;/,
    'TrajectoryRlTrainingJobListRequest should expose optional target_remote_user_id history filter',
  );
});

test('agent exchange has an operable route for cross-agent bundle handoff', () => {
  assert.match(appSource, /import\s+\{\s*AgentExchangePage\s*\}\s+from\s+'..\/routes\/AgentExchangePage'/);
  assert.match(appSource, /path="agent-exchange"\s+element=\{<AgentExchangePage\s*\/>\}/);
  assert.match(sidebarSource, /\{\s*path:\s*'\/agent-exchange',\s*label:\s*'Agent Exchange'\s*\}/);
  assert.match(contextPanelSource, /'\/agent-exchange':\s*\{/);

  for (const functionName of [
    'agentExchangeGetState',
    'agentExchangeListMessages',
    'agentExchangeDraftOutbound',
    'agentExchangeIngestInbound',
    'agentExchangeListRemoteUsers',
    'agentExchangeUpsertRemoteUser',
    'agentExchangeDeleteRemoteUser',
    'agentExchangeExportBundle',
    'agentExchangeImportBundle',
    'agentExchangeUpdateMessageStatus',
    'agentExchangeDeleteMessage',
    'agentExchangeRunFolderSync',
  ]) {
    assert.match(
      agentExchangePageSource,
      new RegExp(`\\b${functionName}\\b`),
      `AgentExchangePage should use ${functionName}`,
    );
  }

  assert.match(agentExchangePageSource, /Local-only mailbox/);
  assert.match(agentExchangePageSource, /future remote users/i);
  assert.match(agentExchangePageSource, /Future Remote Users|future remote users/i);
  assert.match(agentExchangePageSource, /Save remote user|Draft outbound message/i);
  assert.match(agentExchangePageSource, /Use for outbound draft|Future remote user identity/i);
  assert.match(agentExchangePageSource, /Use for inbound ingest/i);
  assert.match(agentExchangePageSource, /remote_user_id|Remote user id|remote user directory|remote user filter/i);
  assert.match(agentExchangePageSource, /remote user identity/i);
  assert.match(agentExchangePageSource, /does not call a remote service/);
  assert.match(agentExchangePageSource, /Shared-file sync path/);
  assert.match(agentExchangePageSource, /Run file sync/);
  assert.match(agentExchangePageSource, /remote user profile\(s\)|remote user profiles/i);
  assert.match(agentExchangePageSource, /Bundle preview|Parsed bundle|parsed bundle/i);
  assert.match(
    agentExchangePageSource,
    /Download bundle JSON|agent-exchange-bundle\.json/,
    'AgentExchangePage should expose a download action for the local bundle handoff JSON',
  );
  assert.match(agentExchangePageSource, /Mark sent/);
  assert.match(agentExchangePageSource, /Archive/);
  assert.match(agentExchangePageSource, /Restore/);
  assert.match(agentExchangePageSource, /Delete/);
  assert.match(agentExchangePageSource, /imported/i);
  assert.match(agentExchangePageSource, /skipped/i);
  assert.match(agentExchangePageSource, /exported/i);
});

test('runtime page surfaces recent local RL training jobs via the shared capability wrapper', () => {
  assert.match(
    runtimePageSource,
    /\btrajectoryListLocalRlTrainingJobs\b/,
    'RuntimePage should reference trajectoryListLocalRlTrainingJobs',
  );
  assert.match(
    runtimePageSource,
    /Recent RL training jobs|Recent training jobs|RL training history|recent jobs/i,
    'RuntimePage should surface recent jobs/history copy for local RL training runs',
  );
  assert.match(
    runtimePageSource,
    /Copy artifact JSON|Download artifact JSON/,
    'RuntimePage should expose local RL artifact export controls',
  );
  assert.match(
    runtimePageSource,
    /Target remote user id|target_remote_user_id/,
    'RuntimePage should expose future remote user routing metadata for local RL artifact exports',
  );
  assert.match(
    runtimePageSource,
    /trajectoryRunLocalRlTraining\(\{[\s\S]*target_remote_user_id: localRlArtifactTargetRemoteUserId\.trim\(\) \|\| null,/,
    'RuntimePage should persist target_remote_user_id when starting local RL training jobs',
  );
  assert.match(
    runtimePageSource,
    /trajectoryListLocalRlTrainingJobs\(\{[^}]*target_remote_user_id: localRlArtifactTargetRemoteUserId\.trim\(\) \|\| null,/,
    'RuntimePage should filter local RL job history by target_remote_user_id when the target field is set',
  );
  assert.match(
    runtimePageSource,
    /schema_version|exported_at|boundary_note|job_id/,
    'RuntimePage should wrap local RL artifact exports in an evidence envelope',
  );
  assert.match(
    runtimePageSource,
    /local tabular baseline training artifact/i,
    'RuntimePage should clarify the export is local baseline training evidence, not remote RLHF infrastructure',
  );
});

test('skills page surfaces marketplace install history via the shared capability wrapper', () => {
  assert.match(
    skillsPageSource,
    /\bskillsMarketplaceListInstallHistory\b/,
    'SkillsPage should reference skillsMarketplaceListInstallHistory',
  );
  assert.match(
    skillsPageSource,
    /Marketplace install history/,
    'SkillsPage should surface marketplace install history copy',
  );
  assert.match(
    skillsPageSource,
    /Local audit export/i,
    'SkillsPage should expose local audit export copy for marketplace install history',
  );
  assert.match(
    skillsPageSource,
    /Copy audit JSON/i,
    'SkillsPage should expose a copy action for the current local audit JSON',
  );
  assert.match(
    skillsPageSource,
    /Download audit JSON/i,
    'SkillsPage should expose a download action for the current local audit JSON',
  );
  assert.match(
    skillsPageSource,
    /Target remote user id|target_remote_user_id/,
    'SkillsPage should expose optional future remote user routing metadata for audit exports',
  );
  assert.match(
    skillsPageSource,
    /future remote user routing metadata only|no remote marketplace account activity/i,
    'SkillsPage should explain that remote user metadata is local-only routing metadata',
  );
  assert.match(
    tauriSource,
    /export\s+interface\s+SkillMarketplaceInstallHistoryListRequest\s*\{[^}]*target_remote_user_id\?: string \| null;/,
    'SkillMarketplaceInstallHistoryListRequest should expose optional target_remote_user_id history filter',
  );
  assert.match(
    skillsPageSource,
    /skillsMarketplaceListInstallHistory\(\{[\s\S]*target_remote_user_id: marketplaceHistoryTargetRemoteUserId\.trim\(\) \|\| undefined,/,
    'SkillsPage should filter marketplace install history by target_remote_user_id when the target field is set',
  );
  assert.match(
    tauriSource,
    /export\s+interface\s+SkillMarketplaceInstallRequest\s*\{[\s\S]*target_remote_user_id\?: string \| null;/,
    'SkillMarketplaceInstallRequest should expose optional target_remote_user_id metadata',
  );
  assert.match(
    tauriSource,
    /export\s+interface\s+SkillMarketplaceInstallHistoryItem\s*\{[\s\S]*target_remote_user_id\?: string \| null;/,
    'SkillMarketplaceInstallHistoryItem should expose persisted target_remote_user_id metadata',
  );
  assert.match(
    skillsPageSource,
    /skillsMarketplaceInstall\(\{[\s\S]*target_remote_user_id: marketplaceHistoryTargetRemoteUserId\.trim\(\) \|\| null,/,
    'SkillsPage should persist target_remote_user_id when installing marketplace skills',
  );
});

test('capability pages fill target remote user metadata from the local Agent Exchange directory', () => {
  for (const [sourceName, source] of [
    ['SkillsPage', skillsPageSource],
    ['RuntimePage', runtimePageSource],
    ['SimulationPage', simulationPageSource],
  ]) {
    assert.match(
      source,
      /\bagentExchangeListRemoteUsers\b/,
      `${sourceName} should load local Agent Exchange future remote users`,
    );
    assert.match(
      source,
      /agentExchangeListRemoteUsers\(\{[\s\S]*status: 'active'[\s\S]*limit: AGENT_EXCHANGE_REMOTE_USER_LIMIT/s,
      `${sourceName} should request active local Agent Exchange future remote users with a bounded limit`,
    );
    assert.match(
      source,
      /Fill from local (?:Agent Exchange )?(?:future )?remote user/i,
      `${sourceName} should expose a visible local remote user fill control`,
    );
    assert.match(
      source,
      /local Agent Exchange future remote user\(s\)[\s\S]*local routing metadata/i,
      `${sourceName} should explain that the selection only fills local routing metadata`,
    );
  }
});

test('local capability exports include selected future remote user profile snapshots', () => {
  assert.match(
    skillsPageSource,
    /target_remote_user_profile/,
    'SkillsPage marketplace audit export should include selected future remote user profile snapshot',
  );
  assert.match(
    skillsPageSource,
    /buildMarketplaceHistoryAuditJson\([\s\S]*selectedMarketplaceRemoteUser/s,
    'SkillsPage should pass the selected future remote user into marketplace audit export generation',
  );
  assert.match(
    runtimePageSource,
    /target_remote_user_profile/,
    'RuntimePage local RL artifact export should include selected future remote user profile snapshot',
  );
  assert.match(
    runtimePageSource,
    /buildLocalRlArtifactExportJson\(\s*job,\s*localRlArtifactTargetRemoteUserId,\s*selectedLocalRlArtifactRemoteUser/s,
    'RuntimePage should pass the selected future remote user into local RL artifact export generation',
  );
  assert.match(
    runtimePageSource,
    /buildRuntimeAdapterAuditHandoffExportJson\(\s*result,\s*guiAutomationTargetRemoteUserId,\s*selectedGuiAutomationRemoteUser/s,
    'RuntimePage should pass the selected GUI future remote user into runtime adapter audit handoff export generation',
  );
  assert.match(
    simulationPageSource,
    /target_remote_user_profile/,
    'SimulationPage capability evidence export should include selected future remote user profile snapshot',
  );
  assert.match(
    simulationPageSource,
    /buildSimulationCapabilityEvidenceBundle\([\s\S]*selectedSimulationRemoteUser/s,
    'SimulationPage should pass the selected future remote user into simulation capability evidence generation',
  );
});

test('runtime page exposes GUI macro stop-on-error as an explicit UI contract', () => {
  assert.match(
    runtimePageSource,
    /\bstop_on_error\b/,
    'RuntimePage should reference stop_on_error in the GUI automation request path',
  );
  assert.match(
    runtimePageSource,
    /Stop macro on first error/,
    'RuntimePage should expose visible stop-on-error copy for GUI automation',
  );
  assert.match(
    tauriSource,
    /export\s+interface\s+GuiAutomationRequest\s*\{[\s\S]*target_remote_user_id\?: string \| null;/,
    'GuiAutomationRequest should expose optional target_remote_user_id metadata',
  );
  assert.match(
    tauriSource,
    /export\s+interface\s+RuntimeAdapterAuditEvent\s*\{[\s\S]*target_remote_user_id\?: string \| null;/,
    'RuntimeAdapterAuditEvent should expose persisted GUI target_remote_user_id metadata',
  );
  assert.match(
    tauriSource,
    /export\s+interface\s+RuntimeAdapterAuditListRequest\s*\{[^}]*target_remote_user_id\?: string \| null;/,
    'RuntimeAdapterAuditListRequest should expose optional target_remote_user_id history filter',
  );
  assert.match(
    tauriSource,
    /export\s+interface\s+RuntimeAdapterAuditExportRequest\s*\{[^}]*target_remote_user_id\?: string \| null;/,
    'RuntimeAdapterAuditExportRequest should expose optional target_remote_user_id export filter',
  );
  assert.match(
    runtimePageSource,
    /GUI target remote user id|guiAutomationTargetRemoteUserId/,
    'RuntimePage should expose future remote user metadata for GUI macro audits',
  );
  assert.match(
    runtimePageSource,
    /runtimeAdapterRunGuiAutomation\(\{[\s\S]*target_remote_user_id: guiAutomationTargetRemoteUserId\.trim\(\) \|\| null,/,
    'RuntimePage should send target_remote_user_id into GUI automation requests',
  );
  assert.match(
    runtimePageSource,
    /runtimeAdapterListAuditEvents\(\{[\s\S]*target_remote_user_id: guiAutomationTargetRemoteUserId\.trim\(\) \|\| null,/,
    'RuntimePage should filter runtime adapter audit list by target_remote_user_id when the target field is set',
  );
  assert.match(
    runtimePageSource,
    /runtimeAdapterExportAuditEvents\(\{[\s\S]*target_remote_user_id: guiAutomationTargetRemoteUserId\.trim\(\) \|\| null,/,
    'RuntimePage should filter runtime adapter audit export by target_remote_user_id when the target field is set',
  );
  assert.match(
    runtimePageSource,
    /Download handoff JSON|Download audit JSON/,
    'RuntimePage should expose a download action for the local runtime adapter audit handoff envelope',
  );
  assert.match(
    runtimePageSource,
    /NATIVE_CUA_AUDIT_EXPORT_FILENAME|native-cua-audit-export\.json|Download Native CUA audit payload/,
    'RuntimePage should expose a download action for the local Hermes native CUA audit export payload',
  );
  assert.match(
    runtimePageSource,
    /TURIX_CUA_AUDIT_EXPORT_FILENAME|turix-cua-audit-export\.json|Download TuriX audit payload/,
    'RuntimePage should expose a download action for the local TuriX bridge audit export payload',
  );
  assert.match(
    runtimePageSource,
    /<option value="gui_automation">gui_automation<\/option>/,
    'RuntimePage runtime adapter audit filter should expose gui_automation audit events',
  );
});

test('simulation page surfaces external SaaS and high-fidelity sandbox run history via shared wrappers', () => {
  for (const functionName of [
    'simulationListExternalSaasRuns',
    'simulationListHighFidelitySandboxRuns',
  ]) {
    assert.match(
      simulationPageSource,
      new RegExp(`\\b${functionName}\\b`),
      `SimulationPage should reference ${functionName}`,
    );
  }

  assert.match(
    simulationPageSource,
    /Recent external SaaS runs/,
    'SimulationPage should surface external SaaS run history copy',
  );
  assert.match(
    simulationPageSource,
    /Recent high-fidelity sandbox runs/,
    'SimulationPage should surface high-fidelity sandbox run history copy',
  );
  assert.match(
    simulationPageSource,
    /simulation-capability-evidence\.json/,
    'SimulationPage should expose a named simulation capability evidence export artifact',
  );
  assert.match(
    simulationPageSource,
    /Build evidence JSON|Refresh evidence JSON|Download evidence JSON/,
    'SimulationPage should expose evidence build and download controls',
  );
  assert.match(
    simulationPageSource,
    /Target remote user id|target_remote_user_id/,
    'SimulationPage should expose optional target remote user metadata for evidence export',
  );
  assert.match(
    tauriSource,
    /export\s+interface\s+SimulationCapabilityRunListRequest\s*\{[^}]*target_remote_user_id\?: string \| null;/,
    'SimulationCapabilityRunListRequest should expose optional target_remote_user_id history filter',
  );
  assert.match(
    simulationPageSource,
    /simulationListExternalSaasRuns\(\{[\s\S]*target_remote_user_id: simulationEvidenceTargetRemoteUserId\.trim\(\) \|\| null,/,
    'SimulationPage should filter external SaaS history by target_remote_user_id when the target field is set',
  );
  assert.match(
    simulationPageSource,
    /simulationListHighFidelitySandboxRuns\(\{[\s\S]*target_remote_user_id: simulationEvidenceTargetRemoteUserId\.trim\(\) \|\| null,/,
    'SimulationPage should filter high-fidelity sandbox history by target_remote_user_id when the target field is set',
  );
  assert.match(
    tauriSource,
    /export\s+interface\s+SimulationRunExternalSaasRequest\s*\{[\s\S]*target_remote_user_id\?: string \| null;/,
    'SimulationRunExternalSaasRequest should expose optional target_remote_user_id metadata',
  );
  assert.match(
    tauriSource,
    /export\s+interface\s+SimulationExternalSaasRun\s*\{[\s\S]*target_remote_user_id\?: string \| null;/,
    'SimulationExternalSaasRun should expose persisted target_remote_user_id metadata',
  );
  assert.match(
    tauriSource,
    /export\s+interface\s+SimulationRunHighFidelitySandboxRequest\s*\{[\s\S]*target_remote_user_id\?: string \| null;/,
    'SimulationRunHighFidelitySandboxRequest should expose optional target_remote_user_id metadata',
  );
  assert.match(
    tauriSource,
    /export\s+interface\s+SimulationHighFidelitySandboxRun\s*\{[\s\S]*target_remote_user_id\?: string \| null;/,
    'SimulationHighFidelitySandboxRun should expose persisted target_remote_user_id metadata',
  );
  assert.match(
    simulationPageSource,
    /simulationRunExternalSaas\(\{[\s\S]*target_remote_user_id: simulationEvidenceTargetRemoteUserId\.trim\(\) \|\| null,/,
    'SimulationPage should persist target_remote_user_id on external SaaS runs',
  );
  assert.match(
    simulationPageSource,
    /simulationRunHighFidelitySandbox\(\{[\s\S]*target_remote_user_id: simulationEvidenceTargetRemoteUserId\.trim\(\) \|\| null,/,
    'SimulationPage should persist target_remote_user_id on high-fidelity sandbox runs',
  );
  assert.match(
    simulationPageSource,
    /future handoff|remote delivery|routing metadata/i,
    'SimulationPage should explain the boundary between remote routing metadata and remote delivery proof',
  );
});
