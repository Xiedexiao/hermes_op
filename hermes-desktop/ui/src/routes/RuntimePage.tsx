import { useEffect, useState } from 'react';
import { useAppStore } from '../store/appStore';
import { useRuntimeStore } from '../store/runtimeStore';
import {
  agentExchangeListRemoteUsers,
  nativeCuaApplyModelOutput,
  nativeCuaExecuteAction,
  nativeCuaExportAuditEvents,
  nativeCuaExportTrajectory,
  nativeCuaInvokeModel,
  nativeCuaListAuditEvents,
  nativeCuaListHistory,
  nativeCuaObserve,
  nativeCuaPlanTask,
  nativeCuaPrepareModelTurn,
  nativeCuaPreviewModelRoute,
  nativeCuaProbe,
  nativeCuaRunStep,
  nativeCuaStartSession,
  settingsGet,
  settingsSave,
  runtimeAdapterExecuteDesktopAction,
  runtimeAdapterExecuteSkillTool,
  runtimeAdapterExportAuditEvents,
  runtimeAdapterListAuditEvents,
  runtimeAdapterProbeDesktopExecutor,
  runtimeAdapterRunGuiAutomation,
  runtimeAdapterSummarizeTrajectoryJsonl,
  runtimeGetStatus,
  runtimeStartEngine,
  runtimeStopEngine,
  runtimeRestartEngine,
  teamSyncCheckAccess,
  teamSyncExportAudit,
  teamSyncExportBundle,
  teamSyncGetState,
  teamSyncImportBundle,
  teamSyncRunFolderSync,
  teamSyncUpsertMember,
  trajectoryListLocalRlTrainingJobs,
  trajectoryRunLocalRlTraining,
  turixCuaExportAuditEvents,
  turixCuaListAuditEvents,
  turixCuaProbe,
  turixCuaRun,
  type NativeCuaActionType,
  type NativeCuaAuditEvent,
  type NativeCuaAutoModelSettings,
  type NativeCuaModelProfileSettings,
  type NativeCuaAuditExportFormat,
  type NativeCuaAuditExportResponse,
  type NativeCuaApplyModelOutputResponse,
  type NativeCuaExecuteActionResponse,
  type NativeCuaInvokeModelResponse,
  type NativeCuaModelRoutePreview,
  type NativeCuaModelRole,
  type NativeCuaModelTurnResponse,
  type NativeCuaObserveResponse,
  type NativeCuaPlanResponse,
  type NativeCuaProbe,
  type NativeCuaRunStepResponse,
  type NativeCuaSessionResponse,
  type NativeCuaStepRecord,
  type NativeCuaTrajectoryExportResponse,
  type AgentExchangeRemoteUser,
  type TeamAuditEvent,
  type TeamSyncAuditExportFormat,
  type TeamSyncBundle,
  type TeamSyncExportAuditResponse,
  type RuntimeAdapterAuditEvent,
  type RuntimeAdapterAuditExportResponse,
  type RuntimeSettings,
  type DesktopActionResponse,
  type DesktopExecutorProbe,
  type GuiAutomationResponse,
  type SkillToolResponse,
  type TeamRole,
  type TeamSyncAccessDecision,
  type TeamSyncState,
  type TrajectoryRlTrainingResult,
  type TrajectorySummaryResponse,
  type TurixCuaAuditEvent,
  type TurixCuaAuditExportFormat,
  type TurixCuaAuditExportResponse,
  type TurixCuaProbe,
  type TurixCuaRunResponse,
} from '../lib/tauri';
import './RuntimePage.css';

const SAMPLE_TRAJECTORY_JSONL = [
  JSON.stringify({ kind: 'run', source: 'local', reward_hint: 1 }),
  JSON.stringify({ kind: 'execution_step', source: 'desktop' }),
  JSON.stringify({ kind: 'run_event', source: 'skill', reward_hint: true }),
  '{invalid-json',
].join('\n');

const NON_DRY_RUN_CONFIRM_PHRASE = 'RUN DESKTOP ACTION';
const NON_DRY_RUN_NATIVE_CUA_CONFIRM_PHRASE = 'RUN NATIVE CUA ACTION';
const NON_DRY_RUN_NATIVE_CUA_MODEL_CONFIRM_PHRASE = 'INVOKE NATIVE CUA MODEL';
const AGENT_EXCHANGE_REMOTE_USER_LIMIT = 50;
const LOCAL_RL_TRAINING_JOB_LIMIT = 8;
const LOCAL_RL_ARTIFACT_EXPORT_BOUNDARY_NOTE =
  'This is a local tabular baseline training artifact. target_remote_user_id is future remote user routing metadata only; it is not remote RLHF infrastructure and does not prove remote delivery.';
const RUNTIME_ADAPTER_AUDIT_EXPORT_BOUNDARY_NOTE =
  'This runtime adapter audit handoff envelope is local-only. target_remote_user_id and target_remote_user_profile are future remote user routing metadata only; they do not prove remote delivery, remote account activity, or remote GUI automation execution.';
const RUNTIME_ADAPTER_AUDIT_HANDOFF_FILENAME = 'runtime-adapter-audit-handoff.json';
const NATIVE_CUA_AUDIT_EXPORT_FILENAME = 'native-cua-audit-export.json';
const NATIVE_CUA_AUDIT_EXPORT_JSONL_FILENAME = 'native-cua-audit-export.jsonl';
const TURIX_CUA_AUDIT_EXPORT_FILENAME = 'turix-cua-audit-export.json';
const TURIX_CUA_AUDIT_EXPORT_JSONL_FILENAME = 'turix-cua-audit-export.jsonl';
const SAMPLE_TURIX_TASK = 'Inspect the local TuriX runtime bridge, report readiness hints, and outline what a real CUA run would need.';
const SAMPLE_NATIVE_CUA_TASK = 'Inspect the current desktop, confirm whether Hermes native CUA can observe safely, and outline the next guarded action without claiming OSWorld or SOTA capability.';
const SAMPLE_NATIVE_CUA_SKILLS_JSON = JSON.stringify([
  { name: 'browser', description: 'Browser/page inspection and web workflow guidance' },
  { name: 'office', description: 'Office document editing, spreadsheet, and presentation workflow guidance' },
], null, 2);
const SAMPLE_NATIVE_CUA_STEP_ACTIONS_JSON = JSON.stringify([
  { wait: { text: 'Planner has prepared the next goal; provide concrete actor actions or keep dry-run wait.' } },
], null, 2);
const SAMPLE_NATIVE_CUA_ACTOR_OUTPUT_JSON = JSON.stringify({
  action: [
    { wait: { text: 'Prepared by model output seam; replace this with real VLM actor actions.' } },
  ],
}, null, 2);

const NATIVE_CUA_ACTION_OPTIONS: Array<{
  value: NativeCuaActionType;
  label: string;
  hint: string;
}> = [
  { value: 'wait', label: 'wait', hint: 'Pause the loop without an OS command.' },
  { value: 'click', label: 'click', hint: 'Use x/y to target a point.' },
  { value: 'double_click', label: 'double_click', hint: 'Use x/y for a guarded double-click attempt.' },
  { value: 'right_click', label: 'right_click', hint: 'Use x/y for a guarded secondary click.' },
  { value: 'type_text', label: 'type_text', hint: 'Use text to enter a string.' },
  { value: 'press_key', label: 'press_key', hint: 'Use key for a single key press.' },
  { value: 'hotkey', label: 'hotkey', hint: 'Use key plus modifiers for a shortcut.' },
  { value: 'launch_app', label: 'launch_app', hint: 'Use app to request an application launch.' },
  { value: 'move_pointer', label: 'move_pointer', hint: 'Use x/y to move the pointer only.' },
  { value: 'drag_pointer', label: 'drag_pointer', hint: 'Use x/y and dx/dy for a drag vector.' },
  { value: 'scroll', label: 'scroll', hint: 'Use dx/dy to request a scroll delta.' },
  { value: 'run_apple_script', label: 'run_apple_script', hint: 'Use text as macOS AppleScript; live execution still requires confirmation.' },
  { value: 'done', label: 'done', hint: 'Mark a native CUA loop step as complete.' },
];

type NativeCuaModelProviderPreset = {
  value: string;
  label: string;
  defaultModel: string;
  defaultBaseUrl: string;
  note: string;
};

const NATIVE_CUA_MODEL_PROVIDER_PRESETS: NativeCuaModelProviderPreset[] = [
  {
    value: 'openai',
    label: 'OpenAI-compatible',
    defaultModel: 'gpt-4o',
    defaultBaseUrl: 'https://api.openai.com/v1',
    note: 'Use OPENAI_API_KEY by default, or set api_key_ref to an env var name.',
  },
  {
    value: 'openrouter',
    label: 'OpenRouter',
    defaultModel: 'openai/gpt-4o-mini',
    defaultBaseUrl: 'https://openrouter.ai/api/v1',
    note: 'Model names usually include a provider prefix; OPENROUTER_API_KEY is the fallback env var.',
  },
  {
    value: 'deepseek',
    label: 'DeepSeek',
    defaultModel: 'deepseek-chat',
    defaultBaseUrl: 'https://api.deepseek.com/v1',
    note: 'Uses DEEPSEEK_API_KEY by default unless api_key_ref points to another env var.',
  },
  {
    value: 'anthropic',
    label: 'Anthropic',
    defaultModel: 'claude-sonnet-4',
    defaultBaseUrl: 'https://api.anthropic.com',
    note: 'Uses ANTHROPIC_API_KEY by default; the backend sends Anthropic-native message payloads.',
  },
  {
    value: 'ollama',
    label: 'Ollama local',
    defaultModel: 'qwen2.5-coder',
    defaultBaseUrl: 'http://localhost:11434',
    note: 'Local runtime; no API key is required by the Native CUA backend.',
  },
];

function findNativeCuaModelProviderPreset(provider: string) {
  return NATIVE_CUA_MODEL_PROVIDER_PRESETS.find((preset) => preset.value === provider.trim().toLowerCase());
}

function maskRuntimeSecretRef(value?: string | null) {
  const trimmed = value?.trim() ?? '';
  if (!trimmed) {
    return '未设置';
  }
  if (trimmed.length <= 8) {
    return '••••';
  }
  return `${trimmed.slice(0, 4)}••••${trimmed.slice(-4)}`;
}

type NativeCuaAutoModelTier = 'easy' | 'standard' | 'hard';

type NativeCuaAutoModelProfileForm = {
  provider: string;
  model: string;
  base_url: string;
  api_key_ref: string;
};

type NativeCuaAutoModelForm = Record<NativeCuaAutoModelTier, NativeCuaAutoModelProfileForm>;

const NATIVE_CUA_AUTO_MODEL_TIERS: Array<{
  value: NativeCuaAutoModelTier;
  label: string;
  hint: string;
}> = [
  { value: 'easy', label: 'Easy / Fast', hint: 'Direct clicks, opening apps, simple typing, short one-screen tasks.' },
  { value: 'standard', label: 'Standard / Balanced', hint: 'Normal multi-step desktop work with moderate planning.' },
  { value: 'hard', label: 'Hard / Deep', hint: 'Long, cross-app, analysis-heavy, research/debug/integration tasks.' },
];

function defaultAutoModelForProvider(provider: string, tier: NativeCuaAutoModelTier) {
  switch (provider) {
    case 'anthropic':
      return tier === 'hard' ? 'claude-opus-4' : 'claude-sonnet-4';
    case 'deepseek':
      return tier === 'hard' ? 'deepseek-reasoner' : 'deepseek-chat';
    case 'ollama':
      return tier === 'easy' ? 'llama3.1' : 'qwen2.5-coder';
    case 'openrouter':
      if (tier === 'easy') return 'openai/gpt-4o-mini';
      if (tier === 'hard') return 'anthropic/claude-opus-4';
      return 'anthropic/claude-sonnet-4';
    default:
      if (tier === 'easy') return 'gpt-4o-mini';
      if (tier === 'hard') return 'gpt-4.1';
      return 'gpt-4o';
  }
}

function createDefaultAutoModelProfile(provider: string, tier: NativeCuaAutoModelTier): NativeCuaAutoModelProfileForm {
  const preset = findNativeCuaModelProviderPreset(provider) ?? findNativeCuaModelProviderPreset('openai');
  const resolvedProvider = preset?.value ?? 'openai';
  return {
    provider: resolvedProvider,
    model: defaultAutoModelForProvider(resolvedProvider, tier),
    base_url: preset?.defaultBaseUrl ?? 'https://api.openai.com/v1',
    api_key_ref: '',
  };
}

function createDefaultAutoModelForm(provider = 'openai'): NativeCuaAutoModelForm {
  return {
    easy: createDefaultAutoModelProfile(provider, 'easy'),
    standard: createDefaultAutoModelProfile(provider, 'standard'),
    hard: createDefaultAutoModelProfile(provider, 'hard'),
  };
}

function nativeCuaAutoSettingsToForm(
  settings: RuntimeSettings | null | undefined,
): NativeCuaAutoModelForm {
  const provider = settings?.provider?.trim().toLowerCase() || 'openai';
  const defaults = createDefaultAutoModelForm(provider);
  const autoModels = settings?.native_cua_auto_models;
  const mergeProfile = (
    tier: NativeCuaAutoModelTier,
    profile?: NativeCuaModelProfileSettings | null,
  ): NativeCuaAutoModelProfileForm => ({
    provider: profile?.provider?.trim() || defaults[tier].provider,
    model: profile?.model?.trim() || defaults[tier].model,
    base_url: profile?.base_url?.trim() || defaults[tier].base_url,
    api_key_ref: profile?.api_key_ref?.trim() || '',
  });

  return {
    easy: mergeProfile('easy', autoModels?.easy),
    standard: mergeProfile('standard', autoModels?.standard),
    hard: mergeProfile('hard', autoModels?.hard),
  };
}

function nativeCuaAutoModelFormToSettings(
  form: NativeCuaAutoModelForm,
): NativeCuaAutoModelSettings {
  const normalizeProfile = (profile: NativeCuaAutoModelProfileForm): NativeCuaModelProfileSettings => ({
    provider: profile.provider.trim() || null,
    model: profile.model.trim() || null,
    base_url: profile.base_url.trim() || null,
    api_key_ref: profile.api_key_ref.trim() || null,
  });

  return {
    easy: normalizeProfile(form.easy),
    standard: normalizeProfile(form.standard),
    hard: normalizeProfile(form.hard),
  };
}

type ClipboardCopyState = {
  status: 'success' | 'error';
  message: string;
};

type NativeCuaActionFormState = {
  actionType: NativeCuaActionType;
  text: string;
  key: string;
  modifiers: string;
  app: string;
  x: string;
  y: string;
  dx: string;
  dy: string;
  dryRun: boolean;
  confirmationPhrase: string;
};

const INITIAL_NATIVE_CUA_ACTION_FORM: NativeCuaActionFormState = {
  actionType: 'click',
  text: '',
  key: '',
  modifiers: '',
  app: '',
  x: '',
  y: '',
  dx: '',
  dy: '',
  dryRun: true,
  confirmationPhrase: '',
};

function getErrorMessage(err: unknown) {
  return err instanceof Error ? err.message : String(err);
}

function formatAgentExchangeRemoteUserOption(remoteUser: AgentExchangeRemoteUser) {
  const displayName = remoteUser.display_name.trim();
  const defaultAgentId = remoteUser.default_agent_id.trim();
  if (displayName && defaultAgentId) {
    return `${displayName} (${remoteUser.user_id}) · ${defaultAgentId}`;
  }
  if (displayName) {
    return `${displayName} (${remoteUser.user_id})`;
  }
  if (defaultAgentId) {
    return `${remoteUser.user_id} · ${defaultAgentId}`;
  }
  return remoteUser.user_id;
}

function formatUnknownValue(value: unknown) {
  if (typeof value === 'string') {
    return value;
  }

  const serialized = JSON.stringify(value, null, 2);
  return serialized ?? String(value);
}

function filterTurixAuditEvents(events: TurixCuaAuditEvent[], query: string) {
  const normalized = query.trim().toLowerCase();
  if (!normalized) {
    return events;
  }

  return events.filter((event) => [
    event.action,
    event.status,
    event.summary,
    event.launcher,
    event.repo_path,
    event.resume_agent_id ?? '',
    event.command.join(' '),
  ].some((value) => value.toLowerCase().includes(normalized)));
}

function filterNativeCuaAuditEvents(events: NativeCuaAuditEvent[], query: string) {
  const normalized = query.trim().toLowerCase();
  if (!normalized) {
    return events;
  }

  return events.filter((event) => [
    event.id,
    event.event_type,
    event.status,
    event.summary,
    event.session_id ?? '',
  ].some((value) => value.toLowerCase().includes(normalized)));
}

function parseOptionalNumber(value: string) {
  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }

  const parsed = Number(trimmed);
  return Number.isFinite(parsed) ? parsed : null;
}

function parseModifiersInput(value: string) {
  const modifiers = value
    .split(',')
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0);

  return modifiers.length > 0 ? modifiers : null;
}

function parseJsonArrayInput(value: string, label: string): unknown[] {
  const trimmed = value.trim();
  if (!trimmed) {
    return [];
  }

  const parsed = JSON.parse(trimmed) as unknown;
  if (!Array.isArray(parsed)) {
    throw new Error(`${label} must be a JSON array.`);
  }
  return parsed;
}

function prependTrajectoryTrainingJob(
  jobs: TrajectoryRlTrainingResult[],
  nextJob: TrajectoryRlTrainingResult,
) {
  return [nextJob, ...jobs.filter((job) => job.job_id !== nextJob.job_id)].slice(0, LOCAL_RL_TRAINING_JOB_LIMIT);
}

function downloadJsonFile(filename: string, content: string) {
  const blob = new Blob([content], { type: 'application/json' });
  const objectUrl = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = objectUrl;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(objectUrl);
}

function nativeCuaAuditExportFilename(format: string | undefined) {
  return format === 'jsonl' ? NATIVE_CUA_AUDIT_EXPORT_JSONL_FILENAME : NATIVE_CUA_AUDIT_EXPORT_FILENAME;
}

function turixAuditExportFilename(format: string | undefined) {
  return format === 'jsonl' ? TURIX_CUA_AUDIT_EXPORT_JSONL_FILENAME : TURIX_CUA_AUDIT_EXPORT_FILENAME;
}

function getLocalRlArtifactFilename(jobId: string) {
  return `${jobId}-local-rl-artifact.json`;
}

function parseLocalRlArtifactJson(artifactJson: string): unknown {
  try {
    return JSON.parse(artifactJson) as unknown;
  } catch {
    return artifactJson;
  }
}

function buildTargetRemoteUserProfileSnapshot(
  targetRemoteUserId: string,
  remoteUser: AgentExchangeRemoteUser | null,
) {
  const normalizedTargetRemoteUserId = targetRemoteUserId.trim();
  if (!normalizedTargetRemoteUserId || remoteUser?.user_id !== normalizedTargetRemoteUserId) {
    return null;
  }

  return {
    user_id: remoteUser.user_id,
    display_name: remoteUser.display_name,
    default_agent_id: remoteUser.default_agent_id,
    transport_label: remoteUser.transport_label ?? null,
    route_hint: remoteUser.route_hint ?? null,
    status: remoteUser.status,
    created_at: remoteUser.created_at,
    updated_at: remoteUser.updated_at,
  };
}

function buildLocalRlArtifactExportJson(
  job: TrajectoryRlTrainingResult,
  targetRemoteUserId: string,
  targetRemoteUserProfile: AgentExchangeRemoteUser | null,
) {
  const artifactJson = job.artifact_json?.trim() ?? '';
  return JSON.stringify(
    {
      schema_version: 1,
      exported_at: new Date().toISOString(),
      target_remote_user_id: targetRemoteUserId.trim() || null,
      target_remote_user_profile: buildTargetRemoteUserProfileSnapshot(
        targetRemoteUserId,
        targetRemoteUserProfile,
      ),
      boundary_note: LOCAL_RL_ARTIFACT_EXPORT_BOUNDARY_NOTE,
      job_id: job.job_id,
      artifact: parseLocalRlArtifactJson(artifactJson),
    },
    null,
    2,
  );
}

function buildRuntimeAdapterAuditHandoffExportJson(
  exportResult: RuntimeAdapterAuditExportResponse,
  targetRemoteUserId: string,
  targetRemoteUserProfile: AgentExchangeRemoteUser | null,
) {
  return JSON.stringify(
    {
      schema_version: 1,
      enveloped_at: new Date().toISOString(),
      source_format: exportResult.format,
      target_remote_user_id: targetRemoteUserId.trim() || null,
      target_remote_user_profile: buildTargetRemoteUserProfileSnapshot(
        targetRemoteUserId,
        targetRemoteUserProfile,
      ),
      boundary_note: RUNTIME_ADAPTER_AUDIT_EXPORT_BOUNDARY_NOTE,
      total: exportResult.total,
      exported_count: exportResult.exported_count,
      events: exportResult.events,
      raw_payload: exportResult.payload,
    },
    null,
    2,
  );
}

function getDesktopActionSample(platform?: string) {
  if (platform === 'macos') {
    return {
      executor: 'osascript',
      args: ['-e', 'tell application "System Events" to get name of first process'],
      label: 'Allowlisted probe example: read first visible process name through System Events',
    };
  }

  if (platform === 'windows') {
    return {
      executor: 'powershell',
      args: ['-Command', 'Get-Process | Select-Object -First 1'],
      label: 'Allowlisted probe example: read first process entry through PowerShell',
    };
  }

  return {
    executor: 'xdotool',
    args: ['getwindowfocus'],
    label: 'Allowlisted probe example: read focused window id through xdotool',
  };
}

export function RuntimePage() {
  const {
    engine,
    appRuntime,
    foreground,
    setEngineStatus,
    setAppRuntimeStatus,
    setForegroundStatus,
    loading,
    setLoading,
    actionInProgress,
    setActionInProgress,
  } = useRuntimeStore();
  const runtimeSettings = useAppStore((state) => state.runtimeSettings);
  const setRuntimeSettings = useAppStore((state) => state.setRuntimeSettings);

  const [error, setError] = useState<string | null>(null);
  const [adapterStatus, setAdapterStatus] = useState<string | null>(null);
  const [teamStatus, setTeamStatus] = useState<string | null>(null);
  const [desktopProbe, setDesktopProbe] = useState<DesktopExecutorProbe | null>(null);
  const [skillToolResult, setSkillToolResult] = useState<SkillToolResponse | null>(null);
  const [desktopActionResult, setDesktopActionResult] = useState<DesktopActionResponse | null>(null);
  const [guiAutomationJson, setGuiAutomationJson] = useState('');
  const [guiAutomationStopOnError, setGuiAutomationStopOnError] = useState(true);
  const [guiAutomationTargetRemoteUserId, setGuiAutomationTargetRemoteUserId] = useState('');
  const [agentExchangeRemoteUsers, setAgentExchangeRemoteUsers] = useState<AgentExchangeRemoteUser[]>([]);
  const [agentExchangeRemoteUsersLoading, setAgentExchangeRemoteUsersLoading] = useState(false);
  const [agentExchangeRemoteUsersError, setAgentExchangeRemoteUsersError] = useState<string | null>(null);
  const selectedGuiAutomationRemoteUser =
    agentExchangeRemoteUsers.find(
      (remoteUser) => remoteUser.user_id === guiAutomationTargetRemoteUserId.trim(),
    ) ?? null;
  const [guiAutomationResult, setGuiAutomationResult] = useState<GuiAutomationResponse | null>(null);
  const [trajectorySummary, setTrajectorySummary] = useState<TrajectorySummaryResponse | null>(null);
  const [trajectoryTrainingResult, setTrajectoryTrainingResult] =
    useState<TrajectoryRlTrainingResult | null>(null);
  const [recentTrajectoryTrainingJobs, setRecentTrajectoryTrainingJobs] = useState<TrajectoryRlTrainingResult[]>([]);
  const [localRlArtifactTargetRemoteUserId, setLocalRlArtifactTargetRemoteUserId] = useState('');
  const selectedLocalRlArtifactRemoteUser =
    agentExchangeRemoteUsers.find(
      (remoteUser) => remoteUser.user_id === localRlArtifactTargetRemoteUserId.trim(),
    ) ?? null;
  const [trajectoryJsonl, setTrajectoryJsonl] = useState('');
  const [runtimeAuditEvents, setRuntimeAuditEvents] = useState<RuntimeAdapterAuditEvent[]>([]);
  const [runtimeAuditExport, setRuntimeAuditExport] = useState<RuntimeAdapterAuditExportResponse | null>(null);
  const [runtimeAuditHandoffPayload, setRuntimeAuditHandoffPayload] = useState('');
  const [runtimeAuditCopyState, setRuntimeAuditCopyState] = useState<ClipboardCopyState | null>(null);
  const [localRlArtifactActionState, setLocalRlArtifactActionState] =
    useState<(ClipboardCopyState & { jobId: string }) | null>(null);
  const [runtimeAuditKindFilter, setRuntimeAuditKindFilter] = useState('');
  const [runtimeAuditStatusFilter, setRuntimeAuditStatusFilter] = useState('');
  const [runtimeAuditExportFormat, setRuntimeAuditExportFormat] = useState<'json' | 'jsonl'>('json');
  const [nativeCuaStatus, setNativeCuaStatus] = useState<string | null>(null);
  const [nativeCuaProbeResult, setNativeCuaProbeResult] = useState<NativeCuaProbe | null>(null);
  const [nativeCuaTask, setNativeCuaTask] = useState(SAMPLE_NATIVE_CUA_TASK);
  const [nativeCuaSessionId, setNativeCuaSessionId] = useState('');
  const [nativeCuaSessionResult, setNativeCuaSessionResult] = useState<NativeCuaSessionResponse | null>(null);
  const [nativeCuaSessionModelMode, setNativeCuaSessionModelMode] = useState<'auto' | 'custom'>('auto');
  const [nativeCuaModelRoutePreview, setNativeCuaModelRoutePreview] = useState<NativeCuaModelRoutePreview | null>(null);
  const [nativeCuaObserveDryRun, setNativeCuaObserveDryRun] = useState(true);
  const [nativeCuaObserveCaptureScreenshot, setNativeCuaObserveCaptureScreenshot] = useState(true);
  const [nativeCuaObserveResult, setNativeCuaObserveResult] = useState<NativeCuaObserveResponse | null>(null);
  const [nativeCuaActionForm, setNativeCuaActionForm] = useState<NativeCuaActionFormState>(INITIAL_NATIVE_CUA_ACTION_FORM);
  const [nativeCuaActionResult, setNativeCuaActionResult] = useState<NativeCuaExecuteActionResponse | null>(null);
  const [nativeCuaSkillCatalogJson, setNativeCuaSkillCatalogJson] = useState(SAMPLE_NATIVE_CUA_SKILLS_JSON);
  const [nativeCuaStepActionsJson, setNativeCuaStepActionsJson] = useState(SAMPLE_NATIVE_CUA_STEP_ACTIONS_JSON);
  const [nativeCuaStepDryRun, setNativeCuaStepDryRun] = useState(true);
  const [nativeCuaStepCaptureScreenshot, setNativeCuaStepCaptureScreenshot] = useState(false);
  const [nativeCuaPlanResult, setNativeCuaPlanResult] = useState<NativeCuaPlanResponse | null>(null);
  const [nativeCuaRunStepResult, setNativeCuaRunStepResult] = useState<NativeCuaRunStepResponse | null>(null);
  const [nativeCuaHistory, setNativeCuaHistory] = useState<NativeCuaStepRecord[]>([]);
  const [nativeCuaTrajectoryExport, setNativeCuaTrajectoryExport] = useState<NativeCuaTrajectoryExportResponse | null>(null);
  const [nativeCuaModelRole, setNativeCuaModelRole] = useState<NativeCuaModelRole>('actor');
  const [nativeCuaModelProvider, setNativeCuaModelProvider] = useState('openai');
  const [nativeCuaModelName, setNativeCuaModelName] = useState('gpt-4o');
  const [nativeCuaModelBaseUrl, setNativeCuaModelBaseUrl] = useState('https://api.openai.com/v1');
  const [nativeCuaModelApiKeyRef, setNativeCuaModelApiKeyRef] = useState('');
  const [nativeCuaAutoModelForm, setNativeCuaAutoModelForm] = useState<NativeCuaAutoModelForm>(() => createDefaultAutoModelForm('openai'));
  const [nativeCuaModelConfigSaving, setNativeCuaModelConfigSaving] = useState(false);
  const [nativeCuaInvokeDryRun, setNativeCuaInvokeDryRun] = useState(true);
  const [nativeCuaInvokeApplyOutput, setNativeCuaInvokeApplyOutput] = useState(true);
  const [nativeCuaModelConfirmationPhrase, setNativeCuaModelConfirmationPhrase] = useState('');
  const [nativeCuaModelExtraContext, setNativeCuaModelExtraContext] = useState('Prefer dry-run-safe actions and return strict JSON only.');
  const [nativeCuaModelOutputJson, setNativeCuaModelOutputJson] = useState(SAMPLE_NATIVE_CUA_ACTOR_OUTPUT_JSON);
  const [nativeCuaModelTurnResult, setNativeCuaModelTurnResult] = useState<NativeCuaModelTurnResponse | null>(null);
  const [nativeCuaInvokeModelResult, setNativeCuaInvokeModelResult] = useState<NativeCuaInvokeModelResponse | null>(null);
  const [nativeCuaApplyModelOutputResult, setNativeCuaApplyModelOutputResult] = useState<NativeCuaApplyModelOutputResponse | null>(null);
  const [nativeCuaAuditEvents, setNativeCuaAuditEvents] = useState<NativeCuaAuditEvent[]>([]);
  const [nativeCuaAuditExport, setNativeCuaAuditExport] = useState<NativeCuaAuditExportResponse | null>(null);
  const [nativeCuaAuditCopyState, setNativeCuaAuditCopyState] = useState<ClipboardCopyState | null>(null);
  const [nativeCuaAuditEventTypeFilter, setNativeCuaAuditEventTypeFilter] = useState('');
  const [nativeCuaAuditStatusFilter, setNativeCuaAuditStatusFilter] = useState('');
  const [nativeCuaAuditQuery, setNativeCuaAuditQuery] = useState('');
  const [nativeCuaAuditExportFormat, setNativeCuaAuditExportFormat] = useState<NativeCuaAuditExportFormat>('json');
  const [turixStatus, setTurixStatus] = useState<string | null>(null);
  const [turixProbeResult, setTurixProbeResult] = useState<TurixCuaProbe | null>(null);
  const [turixTask, setTurixTask] = useState(SAMPLE_TURIX_TASK);
  const [turixResumeAgentId, setTurixResumeAgentId] = useState('');
  const [turixDryRun, setTurixDryRun] = useState(true);
  const [turixRunResult, setTurixRunResult] = useState<TurixCuaRunResponse | null>(null);
  const [turixAuditEvents, setTurixAuditEvents] = useState<TurixCuaAuditEvent[]>([]);
  const [turixAuditExport, setTurixAuditExport] = useState<TurixCuaAuditExportResponse | null>(null);
  const [turixAuditCopyState, setTurixAuditCopyState] = useState<ClipboardCopyState | null>(null);
  const [turixAuditKindFilter, setTurixAuditKindFilter] = useState('');
  const [turixAuditStatusFilter, setTurixAuditStatusFilter] = useState('');
  const [turixAuditQuery, setTurixAuditQuery] = useState('');
  const [turixAuditExportFormat, setTurixAuditExportFormat] = useState<TurixCuaAuditExportFormat>('json');
  const [desktopActionConfirmChecked, setDesktopActionConfirmChecked] = useState(false);
  const [desktopActionConfirmPhrase, setDesktopActionConfirmPhrase] = useState('');
  const [teamState, setTeamState] = useState<TeamSyncState | null>(null);
  const [exportedTeamBundle, setExportedTeamBundle] = useState<TeamSyncBundle | null>(null);
  const [teamActorId, setTeamActorId] = useState('local-owner');
  const [teamMemberId, setTeamMemberId] = useState('teammate');
  const [teamMemberRole, setTeamMemberRole] = useState<TeamRole>('editor');
  const [teamAccessResource, setTeamAccessResource] = useState('bundle');
  const [teamAccessAction, setTeamAccessAction] = useState('export');
  const [teamAccessDecision, setTeamAccessDecision] = useState<TeamSyncAccessDecision | null>(null);
  const [teamBundleJson, setTeamBundleJson] = useState('');
  const [teamFolderSyncPath, setTeamFolderSyncPath] = useState('');
  const [teamAuditSource, setTeamAuditSource] = useState<'state' | 'bundle'>('state');
  const [teamAuditActorFilter, setTeamAuditActorFilter] = useState('');
  const [teamAuditActionFilter, setTeamAuditActionFilter] = useState('');
  const [teamAuditSearch, setTeamAuditSearch] = useState('');
  const [teamAuditExportFormat, setTeamAuditExportFormat] = useState<TeamSyncAuditExportFormat>('json');
  const [teamAuditBackendExport, setTeamAuditBackendExport] = useState<TeamSyncExportAuditResponse | null>(null);
  const [teamAuditCopyState, setTeamAuditCopyState] = useState<ClipboardCopyState | null>(null);

  useEffect(() => {
    loadStatus();
    void loadAgentExchangeRemoteUsers();
  }, []);

  const loadStatus = async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await runtimeGetStatus();
      setEngineStatus({
        ...data.engine,
        last_error: null,
      });
      setAppRuntimeStatus(data.appRuntime);
      setForegroundStatus(data.foreground);
      await loadRuntimeIntegrations();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const loadRuntimeIntegrations = async () => {
    const [
      probeResult,
      teamStateResult,
      runtimeAuditResult,
      nativeCuaProbeSettled,
      nativeCuaAuditSettled,
      nativeCuaHistorySettled,
      settingsSettled,
      turixProbeSettled,
      turixAuditSettled,
      localRlTrainingJobsSettled,
    ] = await Promise.allSettled([
      runtimeAdapterProbeDesktopExecutor(),
      teamSyncGetState(),
      runtimeAdapterListAuditEvents({ limit: 25 }),
      nativeCuaProbe(),
      nativeCuaListAuditEvents({ limit: 25 }),
      nativeCuaListHistory({ limit: 10 }),
      settingsGet(),
      turixCuaProbe(),
      turixCuaListAuditEvents({ limit: 25 }),
      trajectoryListLocalRlTrainingJobs({ limit: LOCAL_RL_TRAINING_JOB_LIMIT }),
    ]);

    if (probeResult.status === 'fulfilled') {
      setDesktopProbe(probeResult.value);
    } else {
      setAdapterStatus(`Desktop probe load failed: ${getErrorMessage(probeResult.reason)}`);
    }

    if (teamStateResult.status === 'fulfilled') {
      setTeamState(teamStateResult.value);
    } else {
      setTeamStatus(`Team governance load failed: ${getErrorMessage(teamStateResult.reason)}`);
    }

    if (runtimeAuditResult.status === 'fulfilled') {
      setRuntimeAuditEvents(runtimeAuditResult.value);
    } else {
      setAdapterStatus(`Runtime adapter audit load failed: ${getErrorMessage(runtimeAuditResult.reason)}`);
    }

    if (nativeCuaProbeSettled.status === 'fulfilled') {
      setNativeCuaProbeResult(nativeCuaProbeSettled.value);
    } else {
      setNativeCuaStatus(`Hermes native CUA probe unavailable: ${getErrorMessage(nativeCuaProbeSettled.reason)}`);
    }

    if (nativeCuaAuditSettled.status === 'fulfilled') {
      setNativeCuaAuditEvents(nativeCuaAuditSettled.value);
    } else {
      setNativeCuaStatus(`Hermes native CUA audit load failed: ${getErrorMessage(nativeCuaAuditSettled.reason)}`);
    }

    if (nativeCuaHistorySettled.status === 'fulfilled') {
      setNativeCuaHistory(nativeCuaHistorySettled.value);
    } else {
      setNativeCuaStatus(`Hermes native CUA history load failed: ${getErrorMessage(nativeCuaHistorySettled.reason)}`);
    }

    if (settingsSettled.status === 'fulfilled') {
      setRuntimeSettings(settingsSettled.value.runtime);
      applyNativeCuaModelSettingsToForm(settingsSettled.value.runtime);
    } else {
      setNativeCuaStatus(`Hermes native CUA model settings load failed: ${getErrorMessage(settingsSettled.reason)}`);
    }

    if (turixProbeSettled.status === 'fulfilled') {
      setTurixProbeResult(turixProbeSettled.value);
    } else {
      setTurixStatus(`TuriX bridge probe unavailable: ${getErrorMessage(turixProbeSettled.reason)}`);
    }

    if (turixAuditSettled.status === 'fulfilled') {
      setTurixAuditEvents(turixAuditSettled.value);
    } else {
      setTurixStatus(`TuriX bridge audit load failed: ${getErrorMessage(turixAuditSettled.reason)}`);
    }

    if (localRlTrainingJobsSettled.status === 'fulfilled') {
      setRecentTrajectoryTrainingJobs(localRlTrainingJobsSettled.value);
    } else {
      setAdapterStatus(`Local RL training history load failed: ${getErrorMessage(localRlTrainingJobsSettled.reason)}`);
    }
  };

  const loadAgentExchangeRemoteUsers = async () => {
    setAgentExchangeRemoteUsersLoading(true);
    setAgentExchangeRemoteUsersError(null);
    try {
      setAgentExchangeRemoteUsers(await agentExchangeListRemoteUsers({
        status: 'active',
        limit: AGENT_EXCHANGE_REMOTE_USER_LIMIT,
      }));
    } catch (err) {
      setAgentExchangeRemoteUsersError(`Local Agent Exchange remote user load failed: ${getErrorMessage(err)}`);
    } finally {
      setAgentExchangeRemoteUsersLoading(false);
    }
  };

  const describeAgentExchangeRemoteUsers = () => {
    if (agentExchangeRemoteUsersError) {
      return agentExchangeRemoteUsersError;
    }
    if (agentExchangeRemoteUsersLoading) {
      return 'Loading active local Agent Exchange future remote users.';
    }
    if (agentExchangeRemoteUsers.length === 0) {
      return 'No active local Agent Exchange future remote users found yet. Manual target_remote_user_id entry still works.';
    }
    return `${agentExchangeRemoteUsers.length} active local Agent Exchange future remote user(s) available. This only fills local routing metadata and does not imply remote delivery or remote RLHF.`;
  };

  function applyNativeCuaModelSettingsToForm(settings: RuntimeSettings) {
    const provider = settings.provider?.trim().toLowerCase() || 'openai';
    const preset = findNativeCuaModelProviderPreset(provider) ?? findNativeCuaModelProviderPreset('openai');
    setNativeCuaModelProvider(provider);
    setNativeCuaModelName(settings.model?.trim() || preset?.defaultModel || 'gpt-4o');
    setNativeCuaModelBaseUrl(settings.base_url?.trim() || preset?.defaultBaseUrl || 'https://api.openai.com/v1');
    setNativeCuaModelApiKeyRef(settings.api_key_ref?.trim() || '');
    setNativeCuaAutoModelForm(nativeCuaAutoSettingsToForm(settings));
  }

  function handleNativeCuaModelProviderChange(provider: string) {
    const normalizedProvider = provider.trim().toLowerCase();
    const previousPreset = findNativeCuaModelProviderPreset(nativeCuaModelProvider);
    const nextPreset = findNativeCuaModelProviderPreset(normalizedProvider);
    setNativeCuaModelRoutePreview(null);
    setNativeCuaModelProvider(normalizedProvider);
    setNativeCuaModelName((current) => {
      const trimmed = current.trim();
      if (trimmed && trimmed !== previousPreset?.defaultModel) {
        return current;
      }
      return nextPreset?.defaultModel ?? current;
    });
    setNativeCuaModelBaseUrl((current) => {
      const trimmed = current.trim();
      if (trimmed && trimmed !== previousPreset?.defaultBaseUrl) {
        return current;
      }
      return nextPreset?.defaultBaseUrl ?? current;
    });
  }

  function updateNativeCuaAutoModelTier(
    tier: NativeCuaAutoModelTier,
    patch: Partial<NativeCuaAutoModelProfileForm>,
  ) {
    setNativeCuaAutoModelForm((current) => ({
      ...current,
      [tier]: {
        ...current[tier],
        ...patch,
      },
    }));
  }

  function handleNativeCuaAutoModelProviderChange(tier: NativeCuaAutoModelTier, provider: string) {
    const preset = findNativeCuaModelProviderPreset(provider);
    updateNativeCuaAutoModelTier(tier, {
      provider,
      model: defaultAutoModelForProvider(provider, tier),
      base_url: preset?.defaultBaseUrl ?? nativeCuaAutoModelForm[tier].base_url,
    });
  }

  async function handleLoadNativeCuaModelSettings() {
    setNativeCuaStatus(null);
    setNativeCuaModelConfigSaving(true);
    try {
      const settings = await settingsGet();
      setRuntimeSettings(settings.runtime);
      applyNativeCuaModelSettingsToForm(settings.runtime);
      setNativeCuaStatus('Loaded saved desktop runtime model settings into the Native CUA form.');
    } catch (err) {
      setNativeCuaStatus(`Hermes native CUA model settings load failed: ${getErrorMessage(err)}`);
    } finally {
      setNativeCuaModelConfigSaving(false);
    }
  }

  async function handleSaveNativeCuaModelSettings() {
    setNativeCuaStatus(null);
    const provider = nativeCuaModelProvider.trim().toLowerCase();
    const preset = findNativeCuaModelProviderPreset(provider);
    if (!provider || !preset) {
      setNativeCuaStatus('Choose a supported provider before saving desktop model settings.');
      return;
    }
    const model = nativeCuaModelName.trim() || preset.defaultModel;
    if (!model) {
      setNativeCuaStatus('Provide a model name before saving desktop model settings.');
      return;
    }

    setNativeCuaModelConfigSaving(true);
    try {
      const settings = await settingsGet();
      const nextRuntime: RuntimeSettings = {
        ...settings.runtime,
        provider,
        model,
        base_url: nativeCuaModelBaseUrl.trim() || preset.defaultBaseUrl,
        api_key_ref: nativeCuaModelApiKeyRef.trim(),
        native_cua_auto_models: nativeCuaAutoModelFormToSettings(nativeCuaAutoModelForm),
        engine_profile: settings.runtime.engine_profile?.trim() || 'default',
        agent_engine_enabled: settings.runtime.agent_engine_enabled ?? true,
        busy_input_mode: settings.runtime.busy_input_mode?.trim() || 'interrupt',
      };
      await settingsSave({ runtime: nextRuntime });
      setRuntimeSettings(nextRuntime);
      applyNativeCuaModelSettingsToForm(nextRuntime);
      setNativeCuaStatus(`Saved ${provider} / ${model} as the desktop Native CUA model default.`);
    } catch (err) {
      setNativeCuaStatus(`Hermes native CUA model settings save failed: ${getErrorMessage(err)}`);
    } finally {
      setNativeCuaModelConfigSaving(false);
    }
  }

  const refreshRuntimeAuditEvents = async () => {
    try {
      const events = await runtimeAdapterListAuditEvents({
        limit: 25,
        kind: runtimeAuditKindFilter || null,
        status: runtimeAuditStatusFilter || null,
        target_remote_user_id: guiAutomationTargetRemoteUserId.trim() || null,
      });
      setRuntimeAuditEvents(events);
    } catch (err) {
      setAdapterStatus(`Runtime adapter audit refresh failed: ${getErrorMessage(err)}`);
    }
  };

  const refreshLocalRlTrainingJobs = async () => {
    try {
      const jobs = await trajectoryListLocalRlTrainingJobs({
        limit: LOCAL_RL_TRAINING_JOB_LIMIT,
        target_remote_user_id: localRlArtifactTargetRemoteUserId.trim() || null,
      });
      setRecentTrajectoryTrainingJobs(jobs);
    } catch (err) {
      setAdapterStatus((current) => current
        ? `${current} Recent local RL jobs refresh failed: ${getErrorMessage(err)}`
        : `Recent local RL jobs refresh failed: ${getErrorMessage(err)}`);
    }
  };

  const handleCopyLocalRlArtifact = async (job: TrajectoryRlTrainingResult) => {
    const artifactJson = job.artifact_json?.trim() ?? '';
    if (!artifactJson) {
      setLocalRlArtifactActionState({
        jobId: job.job_id,
        status: 'error',
        message: 'No local RL artifact JSON is available for this job yet.',
      });
      return;
    }

    if (!navigator.clipboard?.writeText) {
      setLocalRlArtifactActionState({
        jobId: job.job_id,
        status: 'error',
        message:
          'Clipboard is unavailable in this environment. This local tabular baseline training artifact is still shown below in the job history, but it cannot be copied automatically.',
      });
      return;
    }

    try {
      const exportJson = buildLocalRlArtifactExportJson(
        job,
        localRlArtifactTargetRemoteUserId,
        selectedLocalRlArtifactRemoteUser,
      );
      await navigator.clipboard.writeText(exportJson);
      setLocalRlArtifactActionState({
        jobId: job.job_id,
        status: 'success',
        message:
          `Copied local tabular baseline training artifact JSON for ${job.job_id} to the clipboard with future remote user routing metadata.`,
      });
    } catch (err) {
      setLocalRlArtifactActionState({
        jobId: job.job_id,
        status: 'error',
        message:
          err instanceof Error
            ? `Copy failed: ${err.message}. This local tabular baseline training artifact is not remote RLHF infrastructure; use Download artifact JSON if clipboard access stays unavailable.`
            : 'Copy failed. This local tabular baseline training artifact is not remote RLHF infrastructure; use Download artifact JSON if clipboard access stays unavailable.',
      });
    }
  };

  const handleDownloadLocalRlArtifact = (job: TrajectoryRlTrainingResult) => {
    const artifactJson = job.artifact_json?.trim() ?? '';
    if (!artifactJson) {
      setLocalRlArtifactActionState({
        jobId: job.job_id,
        status: 'error',
        message: 'No local RL artifact JSON is available for this job yet.',
      });
      return;
    }

    try {
      const filename = getLocalRlArtifactFilename(job.job_id);
      const exportJson = buildLocalRlArtifactExportJson(
        job,
        localRlArtifactTargetRemoteUserId,
        selectedLocalRlArtifactRemoteUser,
      );
      downloadJsonFile(filename, exportJson);
      setLocalRlArtifactActionState({
        jobId: job.job_id,
        status: 'success',
        message:
          `Downloaded ${filename} with future remote user routing metadata for this local tabular baseline training artifact.`,
      });
    } catch (err) {
      setLocalRlArtifactActionState({
        jobId: job.job_id,
        status: 'error',
        message:
          err instanceof Error
            ? `Artifact download failed: ${err.message}.`
            : 'Artifact download failed for this local RL job.',
      });
    }
  };

  const refreshNativeCuaAuditEvents = async () => {
    try {
      const resolvedSessionId = nativeCuaSessionId.trim() || nativeCuaSessionResult?.session_id || null;
      const events = await nativeCuaListAuditEvents({
        limit: 25,
        session_id: resolvedSessionId,
        event_type: nativeCuaAuditEventTypeFilter || null,
        status: nativeCuaAuditStatusFilter || null,
      });
      setNativeCuaAuditEvents(filterNativeCuaAuditEvents(events, nativeCuaAuditQuery));
    } catch (err) {
      setNativeCuaStatus(`Hermes native CUA audit refresh failed: ${getErrorMessage(err)}`);
    }
  };

  const handleProbeNativeCua = async () => {
    setNativeCuaStatus(null);
    try {
      const result = await nativeCuaProbe();
      setNativeCuaProbeResult(result);
      setNativeCuaStatus(
        result.available
          ? result.readiness || 'Hermes native CUA probe completed. Review readiness, notes, and warnings below.'
          : result.readiness || 'Hermes native CUA probe completed, but the native safety surface is not ready yet.',
      );
    } catch (err) {
      setNativeCuaStatus(`Hermes native CUA probe failed: ${getErrorMessage(err)}`);
    }
  };

  const handlePreviewNativeCuaModelRoute = async () => {
    setNativeCuaStatus(null);

    const trimmedTask = nativeCuaTask.trim();
    if (!trimmedTask) {
      setNativeCuaModelRoutePreview(null);
      setNativeCuaStatus('Provide a task before previewing the Hermes native CUA model route.');
      return;
    }

    try {
      const useCustomModel = nativeCuaSessionModelMode === 'custom';
      const preview = await nativeCuaPreviewModelRoute({
        task: trimmedTask,
        model_mode: nativeCuaSessionModelMode,
        provider: useCustomModel ? nativeCuaModelProvider.trim() || null : null,
        model: useCustomModel ? nativeCuaModelName.trim() || null : null,
        base_url: useCustomModel ? nativeCuaModelBaseUrl.trim() || null : null,
        api_key_ref: useCustomModel ? nativeCuaModelApiKeyRef.trim() || null : null,
      });
      setNativeCuaModelRoutePreview(preview);
      setNativeCuaStatus(preview.summary);
    } catch (err) {
      setNativeCuaStatus(`Hermes native CUA model route preview failed: ${getErrorMessage(err)}`);
    }
  };

  const handleStartNativeCuaSession = async () => {
    setNativeCuaStatus(null);

    const trimmedTask = nativeCuaTask.trim();
    const trimmedSessionId = nativeCuaSessionId.trim();
    if (!trimmedTask) {
      setNativeCuaSessionResult(null);
      setNativeCuaStatus('Provide a task before starting or resuming a Hermes native CUA session.');
      return;
    }

    try {
      const useCustomModel = nativeCuaSessionModelMode === 'custom';
      const result = await nativeCuaStartSession({
        task: trimmedTask,
        session_id: trimmedSessionId || null,
        model_mode: nativeCuaSessionModelMode,
        provider: useCustomModel ? nativeCuaModelProvider.trim() || null : null,
        model: useCustomModel ? nativeCuaModelName.trim() || null : null,
        base_url: useCustomModel ? nativeCuaModelBaseUrl.trim() || null : null,
        api_key_ref: useCustomModel ? nativeCuaModelApiKeyRef.trim() || null : null,
      });
      setNativeCuaSessionResult(result);
      setNativeCuaSessionId(result.session_id);
      setNativeCuaModelRoutePreview({
        model_mode: result.model_mode || nativeCuaSessionModelMode,
        provider: result.provider ?? null,
        model: result.model ?? null,
        base_url: result.base_url ?? null,
        api_key_ref: result.api_key_ref ?? null,
        model_difficulty: result.model_difficulty ?? null,
        model_selection_reason: result.model_selection_reason ?? null,
        summary: result.model_selection_reason || result.summary || 'Native CUA model route persisted on session start.',
      });
      if (result.model_mode === 'custom') {
        setNativeCuaSessionModelMode('custom');
        setNativeCuaModelProvider(result.provider || nativeCuaModelProvider);
        setNativeCuaModelName(result.model || nativeCuaModelName);
        setNativeCuaModelBaseUrl(result.base_url || nativeCuaModelBaseUrl);
        setNativeCuaModelApiKeyRef(result.api_key_ref || nativeCuaModelApiKeyRef);
      }
      setNativeCuaStatus(
        result.summary
          || (result.resumed
            ? `Resumed Hermes native CUA session ${result.session_id}.`
            : `Started Hermes native CUA session ${result.session_id}.`),
      );
    } catch (err) {
      setNativeCuaStatus(`Hermes native CUA session start failed: ${getErrorMessage(err)}`);
    } finally {
      await refreshNativeCuaAuditEvents();
    }
  };

  const handleObserveNativeCua = async () => {
    setNativeCuaStatus(null);
    const resolvedSessionId = nativeCuaSessionId.trim() || nativeCuaSessionResult?.session_id || null;

    if (!resolvedSessionId) {
      setNativeCuaObserveResult(null);
      setNativeCuaStatus('Start or resume a Hermes native CUA session before observing.');
      return;
    }

    try {
      const result = await nativeCuaObserve({
        session_id: resolvedSessionId,
        dry_run: nativeCuaObserveDryRun,
        capture_screenshot: nativeCuaObserveCaptureScreenshot,
      });
      setNativeCuaObserveResult(result);
      setNativeCuaSessionId(result.session_id);
      setNativeCuaStatus(
        result.summary
          || `${result.dry_run ? 'Dry-run' : 'Live'} observe completed for Hermes native CUA session ${result.session_id}.`,
      );
    } catch (err) {
      setNativeCuaStatus(`Hermes native CUA observe failed: ${getErrorMessage(err)}`);
    } finally {
      await refreshNativeCuaAuditEvents();
    }
  };

  const handleExecuteNativeCuaAction = async () => {
    setNativeCuaStatus(null);
    const resolvedSessionId = nativeCuaSessionId.trim() || nativeCuaSessionResult?.session_id || null;

    if (!resolvedSessionId) {
      setNativeCuaActionResult(null);
      setNativeCuaStatus('Start or resume a Hermes native CUA session before executing an action.');
      return;
    }

    if (
      !nativeCuaActionForm.dryRun
      && nativeCuaActionForm.confirmationPhrase.trim() !== NON_DRY_RUN_NATIVE_CUA_CONFIRM_PHRASE
    ) {
      setNativeCuaActionResult(null);
      setNativeCuaStatus(`Type ${NON_DRY_RUN_NATIVE_CUA_CONFIRM_PHRASE} exactly before any non-dry-run native CUA action.`);
      return;
    }

    try {
      const result = await nativeCuaExecuteAction({
        session_id: resolvedSessionId,
        action_type: nativeCuaActionForm.actionType,
        text: nativeCuaActionForm.text.trim() || null,
        key: nativeCuaActionForm.key.trim() || null,
        modifiers: parseModifiersInput(nativeCuaActionForm.modifiers),
        app: nativeCuaActionForm.app.trim() || null,
        x: parseOptionalNumber(nativeCuaActionForm.x),
        y: parseOptionalNumber(nativeCuaActionForm.y),
        dx: parseOptionalNumber(nativeCuaActionForm.dx),
        dy: parseOptionalNumber(nativeCuaActionForm.dy),
        dry_run: nativeCuaActionForm.dryRun,
        confirmation_phrase: nativeCuaActionForm.confirmationPhrase.trim() || null,
      });
      setNativeCuaActionResult(result);
      setNativeCuaSessionId(result.session_id);
      setNativeCuaStatus(
        result.audit_message
          || result.summary
          || `${result.dry_run ? 'Dry-run' : 'Live'} action ${result.action_type} completed for Hermes native CUA session ${result.session_id}.`,
      );
      if (!nativeCuaActionForm.dryRun) {
        setNativeCuaActionForm((current) => ({
          ...current,
          confirmationPhrase: '',
        }));
      }
    } catch (err) {
      setNativeCuaStatus(`Hermes native CUA action failed: ${getErrorMessage(err)}`);
    } finally {
      await refreshNativeCuaAuditEvents();
    }
  };

  const refreshNativeCuaHistory = async () => {
    try {
      const resolvedSessionId = nativeCuaSessionId.trim() || nativeCuaSessionResult?.session_id || null;
      const history = await nativeCuaListHistory({
        session_id: resolvedSessionId,
        limit: 10,
      });
      setNativeCuaHistory(history);
    } catch (err) {
      setNativeCuaStatus(`Hermes native CUA history refresh failed: ${getErrorMessage(err)}`);
    }
  };

  const handlePlanNativeCuaTask = async () => {
    setNativeCuaStatus(null);
    const resolvedSessionId = nativeCuaSessionId.trim() || nativeCuaSessionResult?.session_id || null;
    if (!resolvedSessionId) {
      setNativeCuaStatus('Start or resume a Hermes native CUA session before planning a loop.');
      return;
    }

    try {
      const skillCatalog = parseJsonArrayInput(nativeCuaSkillCatalogJson, 'Skill catalog') as Array<{ name: string; description: string }>;
      const result = await nativeCuaPlanTask({
        session_id: resolvedSessionId,
        task: nativeCuaTask.trim() || null,
        skill_catalog: skillCatalog,
        max_steps: 8,
      });
      setNativeCuaPlanResult(result);
      setNativeCuaStatus(result.summary || `Planned Hermes native CUA loop for ${result.session_id}.`);
    } catch (err) {
      setNativeCuaStatus(`Hermes native CUA planning failed: ${getErrorMessage(err)}`);
    } finally {
      await refreshNativeCuaAuditEvents();
      await refreshNativeCuaHistory();
    }
  };

  const handleRunNativeCuaStep = async () => {
    setNativeCuaStatus(null);
    const resolvedSessionId = nativeCuaSessionId.trim() || nativeCuaSessionResult?.session_id || null;
    if (!resolvedSessionId) {
      setNativeCuaStatus('Start or resume a Hermes native CUA session before running a loop step.');
      return;
    }
    if (!nativeCuaStepDryRun && nativeCuaActionForm.confirmationPhrase.trim() !== NON_DRY_RUN_NATIVE_CUA_CONFIRM_PHRASE) {
      setNativeCuaStatus(`Type ${NON_DRY_RUN_NATIVE_CUA_CONFIRM_PHRASE} exactly before any non-dry-run native CUA loop step.`);
      return;
    }

    try {
      const actions = parseJsonArrayInput(nativeCuaStepActionsJson, 'TuriX-compatible step actions');
      const result = await nativeCuaRunStep({
        session_id: resolvedSessionId,
        dry_run: nativeCuaStepDryRun,
        capture_screenshot: nativeCuaStepCaptureScreenshot,
        actions: actions.length > 0 ? actions : null,
        max_actions: 8,
        confirmation_phrase: nativeCuaActionForm.confirmationPhrase.trim() || null,
      });
      setNativeCuaRunStepResult(result);
      setNativeCuaStatus(result.summary || `Ran Hermes native CUA step ${result.step.step_index}.`);
      if (!nativeCuaStepDryRun) {
        setNativeCuaActionForm((current) => ({ ...current, confirmationPhrase: '' }));
      }
    } catch (err) {
      setNativeCuaStatus(`Hermes native CUA loop step failed: ${getErrorMessage(err)}`);
    } finally {
      await refreshNativeCuaAuditEvents();
      await refreshNativeCuaHistory();
    }
  };

  const handleExportNativeCuaTrajectory = async () => {
    setNativeCuaStatus(null);
    const resolvedSessionId = nativeCuaSessionId.trim() || nativeCuaSessionResult?.session_id || null;
    try {
      const result = await nativeCuaExportTrajectory({
        session_id: resolvedSessionId,
        format: nativeCuaAuditExportFormat,
        include_audit: true,
      });
      setNativeCuaTrajectoryExport(result);
      setNativeCuaStatus(`Exported ${result.exported_count} Hermes native CUA trajectory lines as ${result.format}.`);
    } catch (err) {
      setNativeCuaStatus(`Hermes native CUA trajectory export failed: ${getErrorMessage(err)}`);
    }
  };

  const handlePrepareNativeCuaModelTurn = async () => {
    setNativeCuaStatus(null);
    const resolvedSessionId = nativeCuaSessionId.trim() || nativeCuaSessionResult?.session_id || null;
    if (!resolvedSessionId) {
      setNativeCuaStatus('Start or resume a Hermes native CUA session before preparing a model turn.');
      return;
    }

    try {
      const result = await nativeCuaPrepareModelTurn({
        session_id: resolvedSessionId,
        role: nativeCuaModelRole,
        include_screenshot_data_url: false,
        max_history: 6,
        extra_context: nativeCuaModelExtraContext.trim() || null,
      });
      setNativeCuaModelTurnResult(result);
      setNativeCuaStatus(result.summary || `Prepared ${result.role} model turn.`);
    } catch (err) {
      setNativeCuaStatus(`Hermes native CUA model turn preparation failed: ${getErrorMessage(err)}`);
    } finally {
      await refreshNativeCuaAuditEvents();
    }
  };

  const handleInvokeNativeCuaModel = async () => {
    setNativeCuaStatus(null);
    const resolvedSessionId = nativeCuaSessionId.trim() || nativeCuaSessionResult?.session_id || null;
    if (!resolvedSessionId) {
      setNativeCuaStatus('Start or resume a Hermes native CUA session before invoking a model.');
      return;
    }
    if (!nativeCuaInvokeDryRun && nativeCuaModelConfirmationPhrase.trim() !== NON_DRY_RUN_NATIVE_CUA_MODEL_CONFIRM_PHRASE) {
      setNativeCuaStatus(`Type ${NON_DRY_RUN_NATIVE_CUA_MODEL_CONFIRM_PHRASE} exactly before invoking a non-dry-run native CUA model request.`);
      return;
    }
    if (
      !nativeCuaInvokeDryRun
      && nativeCuaInvokeApplyOutput
      && nativeCuaActionForm.confirmationPhrase.trim() !== NON_DRY_RUN_NATIVE_CUA_CONFIRM_PHRASE
    ) {
      setNativeCuaStatus(`Type ${NON_DRY_RUN_NATIVE_CUA_CONFIRM_PHRASE} exactly before invoking non-dry-run model output with Apply output enabled.`);
      return;
    }

    try {
      const result = await nativeCuaInvokeModel({
        session_id: resolvedSessionId,
        role: nativeCuaModelRole,
        provider: nativeCuaModelProvider.trim() || null,
        model: nativeCuaModelName.trim() || null,
        base_url: nativeCuaModelBaseUrl.trim() || null,
        api_key_ref: nativeCuaModelApiKeyRef.trim() || null,
        dry_run: nativeCuaInvokeDryRun,
        apply_output: nativeCuaInvokeApplyOutput,
        capture_screenshot: nativeCuaStepCaptureScreenshot,
        extra_context: nativeCuaModelExtraContext.trim() || null,
        model_confirmation_phrase: nativeCuaModelConfirmationPhrase.trim() || null,
        action_confirmation_phrase: nativeCuaActionForm.confirmationPhrase.trim() || null,
      });
      setNativeCuaInvokeModelResult(result);
      setNativeCuaModelTurnResult(result.prompt_turn);
      if (result.parsed_output !== undefined) {
        setNativeCuaModelOutputJson(formatUnknownValue(result.parsed_output));
      }
      setNativeCuaApplyModelOutputResult(result.apply_result ?? null);
      if (result.apply_result?.step_result) {
        setNativeCuaRunStepResult(result.apply_result.step_result);
      }
      setNativeCuaStatus(result.summary || `Invoked ${result.role} model runtime seam.`);
      if (!nativeCuaInvokeDryRun) {
        setNativeCuaModelConfirmationPhrase('');
      }
    } catch (err) {
      setNativeCuaStatus(`Hermes native CUA model invoke failed: ${getErrorMessage(err)}`);
    } finally {
      await refreshNativeCuaAuditEvents();
      await refreshNativeCuaHistory();
    }
  };

  const handleApplyNativeCuaModelOutput = async () => {
    setNativeCuaStatus(null);
    const resolvedSessionId = nativeCuaSessionId.trim() || nativeCuaSessionResult?.session_id || null;
    if (!resolvedSessionId) {
      setNativeCuaStatus('Start or resume a Hermes native CUA session before applying model output.');
      return;
    }
    if (!nativeCuaStepDryRun && nativeCuaActionForm.confirmationPhrase.trim() !== NON_DRY_RUN_NATIVE_CUA_CONFIRM_PHRASE) {
      setNativeCuaStatus(`Type ${NON_DRY_RUN_NATIVE_CUA_CONFIRM_PHRASE} exactly before applying non-dry-run actor output.`);
      return;
    }

    try {
      const output = JSON.parse(nativeCuaModelOutputJson) as unknown;
      const result = await nativeCuaApplyModelOutput({
        session_id: resolvedSessionId,
        role: nativeCuaModelRole,
        output,
        dry_run: nativeCuaStepDryRun,
        capture_screenshot: nativeCuaStepCaptureScreenshot,
        confirmation_phrase: nativeCuaActionForm.confirmationPhrase.trim() || null,
      });
      setNativeCuaApplyModelOutputResult(result);
      if (result.step_result) {
        setNativeCuaRunStepResult(result.step_result);
      }
      setNativeCuaStatus(result.summary || `Applied ${result.role} model output.`);
    } catch (err) {
      setNativeCuaStatus(`Hermes native CUA model output apply failed: ${getErrorMessage(err)}`);
    } finally {
      await refreshNativeCuaAuditEvents();
      await refreshNativeCuaHistory();
    }
  };

  const handleExportNativeCuaAudit = async () => {
    setNativeCuaStatus(null);
    setNativeCuaAuditCopyState(null);
    try {
      const resolvedSessionId = nativeCuaSessionId.trim() || nativeCuaSessionResult?.session_id || null;
      const result = await nativeCuaExportAuditEvents({
        limit: 25,
        session_id: resolvedSessionId,
        event_type: nativeCuaAuditEventTypeFilter || null,
        status: nativeCuaAuditStatusFilter || null,
        format: nativeCuaAuditExportFormat,
      });
      setNativeCuaAuditExport(result);
      setNativeCuaAuditEvents(filterNativeCuaAuditEvents(result.events, nativeCuaAuditQuery));
      setNativeCuaStatus(`Exported ${result.exported_count} / ${result.total} Hermes native CUA audit events as ${result.format}.`);
    } catch (err) {
      setNativeCuaStatus(`Hermes native CUA audit export failed: ${getErrorMessage(err)}`);
    }
  };

  const handleCopyNativeCuaAuditPayload = async () => {
    if (!nativeCuaAuditExport?.payload) {
      setNativeCuaAuditCopyState({
        status: 'error',
        message: '先运行 Export audit，生成 payload 后才能复制。',
      });
      return;
    }

    if (!navigator.clipboard?.writeText) {
      setNativeCuaAuditCopyState({
        status: 'error',
        message: '当前环境不支持剪贴板写入。请直接选择下方 Hermes native CUA audit payload 手动复制。',
      });
      return;
    }

    try {
      await navigator.clipboard.writeText(nativeCuaAuditExport.payload);
      setNativeCuaAuditCopyState({
        status: 'success',
        message: `已复制 Hermes native CUA ${nativeCuaAuditExport.format ?? nativeCuaAuditExportFormat} payload 到剪贴板。`,
      });
    } catch (err) {
      setNativeCuaAuditCopyState({
        status: 'error',
        message:
          err instanceof Error
            ? `复制失败：${err.message}。请直接选择下方 Hermes native CUA audit payload 手动复制。`
            : '复制失败。请直接选择下方 Hermes native CUA audit payload 手动复制。',
      });
    }
  };

  const handleDownloadNativeCuaAuditPayload = () => {
    if (!nativeCuaAuditExport?.payload) {
      setNativeCuaAuditCopyState({
        status: 'error',
        message: '先运行 Export audit，生成 payload 后才能下载。',
      });
      return;
    }

    try {
      const filename = nativeCuaAuditExportFilename(nativeCuaAuditExport.format ?? nativeCuaAuditExportFormat);
      downloadJsonFile(filename, nativeCuaAuditExport.payload);
      setNativeCuaAuditCopyState({
        status: 'success',
        message: `已下载 ${filename}。该文件只是本地 Hermes native CUA 审计记录，不代表 remote sync 或 benchmark validation。`,
      });
    } catch (err) {
      setNativeCuaAuditCopyState({
        status: 'error',
        message:
          err instanceof Error
            ? `下载失败：${err.message}。请直接选择下方 Hermes native CUA audit payload 手动保存。`
            : '下载失败。请直接选择下方 Hermes native CUA audit payload 手动保存。',
      });
    }
  };

  const refreshTurixAuditEvents = async () => {
    try {
      const events = await turixCuaListAuditEvents({
        limit: 25,
        action: turixAuditKindFilter || null,
        status: turixAuditStatusFilter || null,
      });
      setTurixAuditEvents(filterTurixAuditEvents(events, turixAuditQuery));
    } catch (err) {
      setTurixStatus(`TuriX bridge audit refresh failed: ${getErrorMessage(err)}`);
    }
  };

  const handleProbeTurixBridge = async () => {
    setTurixStatus(null);
    try {
      const result = await turixCuaProbe();
      setTurixProbeResult(result);
      setTurixStatus(
        result.status
          || (!result.repo_exists
            ? 'TuriX bridge probe completed, but the local bridge is not ready yet.'
            : 'TuriX bridge probe completed. Review the repo/config/script/python/permissions hints below.'),
      );
    } catch (err) {
      setTurixStatus(`TuriX bridge probe failed: ${getErrorMessage(err)}`);
    }
  };

  const handleRunTurixBridge = async () => {
    setTurixStatus(null);

    const trimmedTask = turixTask.trim();
    const trimmedResumeAgentId = turixResumeAgentId.trim();
    if (!trimmedTask && !trimmedResumeAgentId) {
      setTurixRunResult(null);
      setTurixStatus('Provide a task or resume agent id before calling the local TuriX CUA bridge.');
      return;
    }

    try {
      const result = await turixCuaRun({
        task: trimmedTask || null,
        resume_agent_id: trimmedResumeAgentId || null,
        dry_run: turixDryRun,
      });
      setTurixRunResult(result);
      setTurixStatus(
        result.audit_message
          || 'TuriX bridge run request completed. Review the backend response below.',
      );
    } catch (err) {
      setTurixStatus(`TuriX bridge run failed: ${getErrorMessage(err)}`);
    } finally {
      await refreshTurixAuditEvents();
    }
  };

  const handleExportTurixAudit = async () => {
    setTurixStatus(null);
    setTurixAuditCopyState(null);
    try {
      const result = await turixCuaExportAuditEvents({
        limit: 25,
        action: turixAuditKindFilter || null,
        status: turixAuditStatusFilter || null,
        format: turixAuditExportFormat,
      });
      setTurixAuditExport(result);
      setTurixAuditEvents(filterTurixAuditEvents(result.events, turixAuditQuery));
      setTurixStatus(
        `Exported ${result.exported_count} / ${result.total} TuriX audit events as ${result.format}.`,
      );
    } catch (err) {
      setTurixStatus(`TuriX bridge audit export failed: ${getErrorMessage(err)}`);
    }
  };

  const handleCopyTurixAuditPayload = async () => {
    if (!turixAuditExport?.payload) {
      setTurixAuditCopyState({
        status: 'error',
        message: '先运行 Export audit，生成 payload 后才能复制。',
      });
      return;
    }

    if (!navigator.clipboard?.writeText) {
      setTurixAuditCopyState({
        status: 'error',
        message: '当前环境不支持剪贴板写入。请直接选择下方 TuriX audit payload 手动复制。',
      });
      return;
    }

    try {
      await navigator.clipboard.writeText(turixAuditExport.payload);
      setTurixAuditCopyState({
        status: 'success',
        message: `已复制 TuriX bridge ${turixAuditExport.format ?? turixAuditExportFormat} payload 到剪贴板。`,
      });
    } catch (err) {
      setTurixAuditCopyState({
        status: 'error',
        message:
          err instanceof Error
            ? `复制失败：${err.message}。请直接选择下方 TuriX audit payload 手动复制。`
            : '复制失败。请直接选择下方 TuriX audit payload 手动复制。',
      });
    }
  };

  const handleDownloadTurixAuditPayload = () => {
    if (!turixAuditExport?.payload) {
      setTurixAuditCopyState({
        status: 'error',
        message: '先运行 Export audit，生成 payload 后才能下载。',
      });
      return;
    }

    try {
      const filename = turixAuditExportFilename(turixAuditExport.format ?? turixAuditExportFormat);
      downloadJsonFile(filename, turixAuditExport.payload);
      setTurixAuditCopyState({
        status: 'success',
        message: `已下载 ${filename}。该文件只是本地 TuriX bridge 审计记录，不代表 remote sync、Hermes 原生 OSWorld 能力或 benchmark validation。`,
      });
    } catch (err) {
      setTurixAuditCopyState({
        status: 'error',
        message:
          err instanceof Error
            ? `下载失败：${err.message}。请直接选择下方 TuriX audit payload 手动保存。`
            : '下载失败。请直接选择下方 TuriX audit payload 手动保存。',
      });
    }
  };

  const refreshTeamState = async () => {
    const state = await teamSyncGetState();
    setTeamState(state);
  };

  const handleRunSkillToolSample = async () => {
    setAdapterStatus(null);
    try {
      const result = await runtimeAdapterExecuteSkillTool({
        command: 'echo',
        args: ['Hermes runtime adapter is executing locally'],
      });
      setSkillToolResult(result);
      setAdapterStatus(result.audit_message);
    } catch (err) {
      setAdapterStatus(err instanceof Error ? err.message : String(err));
    } finally {
      await refreshRuntimeAuditEvents();
    }
  };

  const handleDesktopAction = async (dryRun: boolean) => {
    setAdapterStatus(null);
    try {
      const sample = getDesktopActionSample(desktopProbe?.platform);
      const result = await runtimeAdapterExecuteDesktopAction({
        executor: sample.executor,
        args: sample.args,
        dry_run: dryRun,
        confirmation_phrase: dryRun ? null : desktopActionConfirmPhrase.trim(),
      });
      setDesktopActionResult(result);
      setAdapterStatus(
        dryRun
          ? `${result.audit_message} Dry-run preview planned the command without touching the desktop session.`
          : `${result.audit_message} Non-dry-run execution still used the backend allowlisted executor path and may still be rejected by backend, session, or platform safety checks.`,
      );
      if (!dryRun) {
        setDesktopActionConfirmChecked(false);
        setDesktopActionConfirmPhrase('');
      }
    } catch (err) {
      setAdapterStatus(err instanceof Error ? err.message : String(err));
    } finally {
      await refreshRuntimeAuditEvents();
    }
  };

  const loadGuiAutomationSample = () => {
    const sample = getDesktopActionSample(desktopProbe?.platform);
    setGuiAutomationJson(JSON.stringify({
      steps: [
        {
          label: 'observe-active-window',
          executor: sample.executor,
          args: sample.args,
        },
      ],
      dry_run: true,
      stop_on_error: true,
    }, null, 2));
  };

  const handleRunGuiAutomation = async (dryRun: boolean) => {
    setAdapterStatus(null);
    const raw = guiAutomationJson.trim();
    if (!raw) {
      setAdapterStatus('Load or paste a GUI automation macro JSON first.');
      return;
    }

    try {
      const parsed = JSON.parse(raw) as { steps?: unknown } | unknown[];
      const request = Array.isArray(parsed)
        ? { steps: parsed }
        : parsed;
      const result = await runtimeAdapterRunGuiAutomation({
        ...(request as { steps: [] }),
        dry_run: dryRun,
        confirmation_phrase: dryRun ? null : desktopActionConfirmPhrase.trim(),
        stop_on_error: guiAutomationStopOnError,
        target_remote_user_id: guiAutomationTargetRemoteUserId.trim() || null,
      });
      setGuiAutomationResult(result);
      setAdapterStatus(result.audit_message);
      if (!dryRun) {
        setDesktopActionConfirmChecked(false);
        setDesktopActionConfirmPhrase('');
      }
    } catch (err) {
      setAdapterStatus(err instanceof Error ? err.message : String(err));
    } finally {
      await refreshRuntimeAuditEvents();
    }
  };

  const handleSummarizeTrajectory = async () => {
    setAdapterStatus(null);
    const jsonl = trajectoryJsonl.trim();
    if (!jsonl) {
      setAdapterStatus('Provide JSONL rows first, or load the sample JSONL to review parser behavior only.');
      setTrajectorySummary(null);
      return;
    }
    try {
      const summary = await runtimeAdapterSummarizeTrajectoryJsonl({ jsonl });
      setTrajectorySummary(summary);
      setAdapterStatus(`Trajectory summary counted ${summary.line_count} valid rows and ${summary.invalid_line_count} invalid rows.`);
    } catch (err) {
      setAdapterStatus(err instanceof Error ? err.message : String(err));
    } finally {
      await refreshRuntimeAuditEvents();
    }
  };

  const handleRunLocalRlTraining = async () => {
    setAdapterStatus(null);
    const jsonl = trajectoryJsonl.trim();
    if (!jsonl) {
      setAdapterStatus('Provide trajectory JSONL first, or load the sample JSONL to train a local tabular baseline.');
      return;
    }

    try {
      const result = await trajectoryRunLocalRlTraining({
        jsonl,
        epochs: 5,
        alpha: 0.25,
        gamma: 0.9,
        job_name: 'desktop-runtime-local-rl',
        target_remote_user_id: localRlArtifactTargetRemoteUserId.trim() || null,
      });
      setTrajectoryTrainingResult(result);
      setRecentTrajectoryTrainingJobs((current) => prependTrajectoryTrainingJob(current, result));
      setAdapterStatus(result.summary);
      await refreshLocalRlTrainingJobs();
    } catch (err) {
      setAdapterStatus(err instanceof Error ? err.message : String(err));
    }
  };

  const handleExportRuntimeAudit = async () => {
    setAdapterStatus(null);
    setRuntimeAuditCopyState(null);
    setRuntimeAuditHandoffPayload('');
    try {
      const result = await runtimeAdapterExportAuditEvents({
        limit: 25,
        kind: runtimeAuditKindFilter || null,
        status: runtimeAuditStatusFilter || null,
        format: runtimeAuditExportFormat,
        target_remote_user_id: guiAutomationTargetRemoteUserId.trim() || null,
      });
      setRuntimeAuditExport(result);
      setRuntimeAuditEvents(result.events);
      setRuntimeAuditHandoffPayload(buildRuntimeAdapterAuditHandoffExportJson(
        result,
        guiAutomationTargetRemoteUserId,
        selectedGuiAutomationRemoteUser,
      ));
      setAdapterStatus(`Exported ${result.exported_count} / ${result.total} runtime adapter audit events as a local handoff envelope with source format ${result.format}.`);
    } catch (err) {
      setAdapterStatus(err instanceof Error ? err.message : String(err));
    }
  };

  const handleCopyRuntimeAuditPayload = async () => {
    const payload = runtimeAuditHandoffPayload || runtimeAuditExport?.payload;
    if (!payload) {
      setRuntimeAuditCopyState({
        status: 'error',
        message: '先运行 Export filtered audit，生成 payload 后才能复制。',
      });
      return;
    }

    if (!navigator.clipboard?.writeText) {
      setRuntimeAuditCopyState({
        status: 'error',
        message: '当前环境不支持剪贴板写入。请直接选择下方 runtime adapter audit payload 手动复制。',
      });
      return;
    }

    try {
      await navigator.clipboard.writeText(payload);
      setRuntimeAuditCopyState({
        status: 'success',
        message: `已复制 runtime adapter audit handoff envelope 到剪贴板。`,
      });
    } catch (err) {
      setRuntimeAuditCopyState({
        status: 'error',
        message:
          err instanceof Error
            ? `复制失败：${err.message}。请直接选择下方 runtime adapter audit payload 手动复制。`
            : '复制失败。请直接选择下方 runtime adapter audit payload 手动复制。',
      });
    }
  };

  const handleDownloadRuntimeAuditHandoff = () => {
    if (!runtimeAuditHandoffPayload) {
      setRuntimeAuditCopyState({
        status: 'error',
        message: 'Run Export filtered audit first so the local runtime adapter audit handoff envelope can be downloaded.',
      });
      return;
    }

    try {
      downloadJsonFile(RUNTIME_ADAPTER_AUDIT_HANDOFF_FILENAME, runtimeAuditHandoffPayload);
      setRuntimeAuditCopyState({
        status: 'success',
        message: `${RUNTIME_ADAPTER_AUDIT_HANDOFF_FILENAME} downloaded. The envelope remains local-only future remote user routing metadata, not remote delivery proof.`,
      });
    } catch (err) {
      setRuntimeAuditCopyState({
        status: 'error',
        message: err instanceof Error
          ? `Runtime adapter audit handoff download failed: ${err.message}.`
          : 'Runtime adapter audit handoff download failed.',
      });
    }
  };

  const handleBootstrapOwner = async () => {
    setTeamStatus(null);
    try {
      await teamSyncUpsertMember({
        actor_member_id: teamActorId,
        member_id: teamActorId,
        role: 'owner',
      });
      await refreshTeamState();
      setTeamStatus(`Bootstrapped or refreshed local owner ${teamActorId}.`);
    } catch (err) {
      setTeamStatus(err instanceof Error ? err.message : String(err));
    }
  };

  const handleUpsertTeamMember = async () => {
    setTeamStatus(null);
    try {
      await teamSyncUpsertMember({
        actor_member_id: teamActorId,
        member_id: teamMemberId,
        role: teamMemberRole,
      });
      await refreshTeamState();
      setTeamStatus(`Upserted ${teamMemberId} as ${teamMemberRole}.`);
    } catch (err) {
      setTeamStatus(err instanceof Error ? err.message : String(err));
    }
  };

  const handleCheckTeamAccess = async () => {
    setTeamStatus(null);
    try {
      const decision = await teamSyncCheckAccess({
        actor_member_id: teamActorId,
        resource: teamAccessResource,
        action: teamAccessAction,
      });
      setTeamAccessDecision(decision);
      setTeamStatus(decision.reason);
    } catch (err) {
      setTeamStatus(err instanceof Error ? err.message : String(err));
    }
  };

  const handleExportTeamBundle = async () => {
    setTeamStatus(null);
    try {
      const bundle = await teamSyncExportBundle({ actor_member_id: teamActorId });
      setExportedTeamBundle(bundle);
      setTeamAuditSource('bundle');
      setTeamBundleJson(JSON.stringify(bundle, null, 2));
      setTeamStatus(`Exported local team bundle with ${bundle.members.length} members and ${bundle.audit_events.length} audit events.`);
    } catch (err) {
      setTeamStatus(err instanceof Error ? err.message : String(err));
    }
  };

  const handleExportTeamAudit = async () => {
    setTeamStatus(null);
    setTeamAuditCopyState(null);
    try {
      const result = await teamSyncExportAudit({
        actor_member_id: teamActorId,
        actor: teamAuditActorFilter || null,
        action: teamAuditActionFilter || null,
        limit: 50,
        format: teamAuditExportFormat,
      });
      setTeamAuditBackendExport(result);
      await refreshTeamState();
      setTeamStatus(`Exported ${result.exported_count} / ${result.total} local team audit events as ${teamAuditExportFormat}.`);
    } catch (err) {
      setTeamStatus(err instanceof Error ? err.message : String(err));
    }
  };

  const handleCopyTeamAuditPayload = async () => {
    if (!teamAuditBackendExport?.payload) {
      setTeamAuditCopyState({
        status: 'error',
        message: '先运行 Export audit via RBAC，生成 backend payload 后才能复制。',
      });
      return;
    }

    if (!navigator.clipboard?.writeText) {
      setTeamAuditCopyState({
        status: 'error',
        message: '当前环境不支持剪贴板写入。请直接选择下方 team audit backend payload 手动复制。',
      });
      return;
    }

    try {
      await navigator.clipboard.writeText(teamAuditBackendExport.payload);
      setTeamAuditCopyState({
        status: 'success',
        message: `已复制 team audit backend ${teamAuditExportFormat} payload 到剪贴板。`,
      });
    } catch (err) {
      setTeamAuditCopyState({
        status: 'error',
        message:
          err instanceof Error
            ? `复制失败：${err.message}。请直接选择下方 team audit backend payload 手动复制。`
            : '复制失败。请直接选择下方 team audit backend payload 手动复制。',
      });
    }
  };

  const handleRunTeamFolderSync = async () => {
    setTeamStatus(null);
    try {
      const result = await teamSyncRunFolderSync({
        actor_member_id: teamActorId,
        file_path: teamFolderSyncPath.trim() || null,
      });
      setTeamState(result.state);
      if (result.bundle) {
        setExportedTeamBundle(result.bundle);
        setTeamAuditSource('bundle');
        setTeamBundleJson(JSON.stringify(result.bundle, null, 2));
      }
      setTeamStatus(
        teamFolderSyncPath.trim()
          ? `Synced local team bundle through ${teamFolderSyncPath.trim()}.`
          : 'Loaded local team governance state without a folder path.',
      );
    } catch (err) {
      setTeamStatus(err instanceof Error ? err.message : String(err));
    }
  };

  const handleImportTeamBundle = async () => {
    setTeamStatus(null);
    try {
      const bundle = JSON.parse(teamBundleJson);
      const state = await teamSyncImportBundle({
        actor_member_id: teamActorId,
        bundle,
      });
      setExportedTeamBundle(bundle as TeamSyncBundle);
      setTeamState(state);
      setTeamStatus(`Imported local team bundle with ${state.members.length} members.`);
    } catch (err) {
      setTeamStatus(err instanceof Error ? err.message : String(err));
    }
  };

  const handleStart = async () => {
    setActionInProgress('start');
    setError(null);
    try {
      const data = await runtimeStartEngine();
      setEngineStatus({
        ...data.engine,
        last_error: null,
      });
      setAppRuntimeStatus(data.appRuntime);
      setForegroundStatus(data.foreground);
      await loadRuntimeIntegrations();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setActionInProgress(null);
    }
  };

  const handleStop = async () => {
    setActionInProgress('stop');
    setError(null);
    try {
      const data = await runtimeStopEngine();
      setEngineStatus({
        ...data.engine,
        last_error: null,
      });
      setAppRuntimeStatus(data.appRuntime);
      setForegroundStatus(data.foreground);
      await loadRuntimeIntegrations();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setActionInProgress(null);
    }
  };

  const handleRestart = async () => {
    setActionInProgress('restart');
    setError(null);
    try {
      const data = await runtimeRestartEngine();
      setEngineStatus({
        ...data.engine,
        last_error: null,
      });
      setAppRuntimeStatus(data.appRuntime);
      setForegroundStatus(data.foreground);
      await loadRuntimeIntegrations();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setActionInProgress(null);
    }
  };

  if (loading) {
    return (
      <div className="runtime-page">
        <div className="loading">加载中...</div>
      </div>
    );
  }

  const desktopActionSample = getDesktopActionSample(desktopProbe?.platform);
  const desktopActionAvailable = desktopProbe
    ? desktopProbe.tool_availability[desktopActionSample.executor] !== false
    : false;
  const desktopActionPhraseMatches = desktopActionConfirmPhrase.trim() === NON_DRY_RUN_CONFIRM_PHRASE;
  const desktopActionGuardReasons = [
    !desktopActionAvailable
      ? `Executor ${desktopActionSample.executor} is not ready for this backend/session/platform probe.`
      : null,
    !desktopActionConfirmChecked
      ? 'Check the confirmation box to acknowledge that non-dry-run attempts a real desktop command.'
      : null,
    !desktopActionPhraseMatches
      ? `Type ${NON_DRY_RUN_CONFIRM_PHRASE} exactly to unlock the non-dry-run action.`
      : null,
  ].filter((reason): reason is string => Boolean(reason));
  const nonDryRunDesktopActionDisabled = desktopActionGuardReasons.length > 0;
  const selectedAuditEvents: TeamAuditEvent[] = teamAuditSource === 'bundle'
    ? exportedTeamBundle?.audit_events ?? []
    : teamState?.audit_events ?? [];
  const normalizedAuditSearch = teamAuditSearch.trim().toLowerCase();
  const filteredAuditEvents = selectedAuditEvents.filter((event) => {
    if (teamAuditActorFilter && event.actor_member_id !== teamAuditActorFilter) {
      return false;
    }
    if (teamAuditActionFilter && event.action !== teamAuditActionFilter) {
      return false;
    }
    if (!normalizedAuditSearch) {
      return true;
    }
    return [
      event.id,
      event.actor_member_id,
      event.subject_member_id ?? '',
      event.action,
      event.detail,
      event.at,
    ].some((value) => value.toLowerCase().includes(normalizedAuditSearch));
  });
  const availableAuditActors = Array.from(new Set(selectedAuditEvents.map((event) => event.actor_member_id))).sort();
  const availableAuditActions = Array.from(new Set(selectedAuditEvents.map((event) => event.action))).sort();
  const auditPreviewJson = JSON.stringify(
    {
      source: teamAuditSource === 'bundle' ? 'exported-bundle' : 'live-state',
      total_events: selectedAuditEvents.length,
      filtered_events: filteredAuditEvents.length,
      audit_events: filteredAuditEvents,
    },
    null,
    2,
  );
  const teamAuditBackendPreview = teamAuditBackendExport?.payload ?? auditPreviewJson;
  const trajectoryLineCount = trajectoryJsonl
    ? trajectoryJsonl.split('\n').filter((line) => line.trim().length > 0).length
    : 0;
  const runtimeAuditPreview = runtimeAuditExport
    ? runtimeAuditHandoffPayload || runtimeAuditExport.payload
    : JSON.stringify(runtimeAuditEvents, null, 2);
  const nativeCuaResolvedSessionId = nativeCuaSessionResult?.session_id || nativeCuaSessionId.trim() || '-';
  const nativeCuaSessionReady = nativeCuaResolvedSessionId !== '-';
  const nativeCuaActionPhraseMatches =
    nativeCuaActionForm.confirmationPhrase.trim() === NON_DRY_RUN_NATIVE_CUA_CONFIRM_PHRASE;
  const nativeCuaNonDryRunDisabled = !nativeCuaActionForm.dryRun && !nativeCuaActionPhraseMatches;
  const nativeCuaActionHint = NATIVE_CUA_ACTION_OPTIONS.find((option) => option.value === nativeCuaActionForm.actionType)?.hint
    ?? 'Provide the fields that match this native CUA action.';
  const nativeCuaSelectedModelPreset = findNativeCuaModelProviderPreset(nativeCuaModelProvider)
    ?? findNativeCuaModelProviderPreset('openai');
  const nativeCuaSavedRuntimeProvider = runtimeSettings?.provider?.trim() || 'openai';
  const nativeCuaSavedRuntimePreset = findNativeCuaModelProviderPreset(nativeCuaSavedRuntimeProvider)
    ?? findNativeCuaModelProviderPreset('openai');
  const nativeCuaSavedRuntimeModel = runtimeSettings?.model?.trim()
    || nativeCuaSavedRuntimePreset?.defaultModel
    || 'gpt-4o';
  const nativeCuaSavedRuntimeBaseUrl = runtimeSettings?.base_url?.trim()
    || nativeCuaSavedRuntimePreset?.defaultBaseUrl
    || 'backend default';
  const nativeCuaRouteSummaryProvider = nativeCuaModelRoutePreview?.provider || nativeCuaSavedRuntimeProvider;
  const nativeCuaRouteSummaryModel = nativeCuaModelRoutePreview?.model || nativeCuaSavedRuntimeModel;
  const nativeCuaSessionModelSummary = nativeCuaSessionModelMode === 'auto'
    ? nativeCuaModelRoutePreview?.model_mode === 'auto'
      ? `Auto · ${nativeCuaModelRoutePreview.model_difficulty || 'standard'} · ${nativeCuaRouteSummaryProvider} / ${nativeCuaRouteSummaryModel}`
      : `Auto · ${nativeCuaSavedRuntimeProvider} / ${nativeCuaSavedRuntimeModel}`
    : `Custom · ${nativeCuaModelProvider || 'provider'} / ${nativeCuaModelName || 'model'}`;
  const nativeCuaProbePreview = nativeCuaProbeResult
    ? formatUnknownValue(nativeCuaProbeResult)
    : '(run Probe readiness to load Hermes native CUA readiness)';
  const nativeCuaSessionPreview = nativeCuaSessionResult
    ? formatUnknownValue(nativeCuaSessionResult)
    : '(no Hermes native CUA session response yet)';
  const nativeCuaModelRoutePreviewText = nativeCuaModelRoutePreview
    ? formatUnknownValue(nativeCuaModelRoutePreview)
    : '(preview the route to see the exact model before starting a task)';
  const nativeCuaObservePreview = nativeCuaObserveResult
    ? formatUnknownValue(nativeCuaObserveResult)
    : '(no Hermes native CUA observe response yet)';
  const nativeCuaActionPreview = nativeCuaActionResult
    ? formatUnknownValue(nativeCuaActionResult)
    : '(no Hermes native CUA action response yet)';
  const nativeCuaPlanPreview = nativeCuaPlanResult
    ? formatUnknownValue(nativeCuaPlanResult)
    : '(no Hermes native CUA loop plan yet)';
  const nativeCuaRunStepPreview = nativeCuaRunStepResult
    ? formatUnknownValue(nativeCuaRunStepResult)
    : '(no Hermes native CUA loop step response yet)';
  const nativeCuaHistoryPreview = formatUnknownValue(nativeCuaHistory);
  const nativeCuaTrajectoryPreview = nativeCuaTrajectoryExport?.payload ?? '(no Hermes native CUA trajectory export yet)';
  const nativeCuaModelTurnPreview = nativeCuaModelTurnResult
    ? formatUnknownValue(nativeCuaModelTurnResult)
    : '(no Hermes native CUA model turn prepared yet)';
  const nativeCuaModelConfirmationMatches =
    nativeCuaModelConfirmationPhrase.trim() === NON_DRY_RUN_NATIVE_CUA_MODEL_CONFIRM_PHRASE;
  const nativeCuaInvokeNeedsActionConfirmation =
    !nativeCuaInvokeDryRun && nativeCuaInvokeApplyOutput;
  const nativeCuaInvokeActionConfirmationMatches =
    nativeCuaActionForm.confirmationPhrase.trim() === NON_DRY_RUN_NATIVE_CUA_CONFIRM_PHRASE;
  const nativeCuaInvokeNonDryRunDisabled =
    !nativeCuaInvokeDryRun
    && (
      !nativeCuaModelConfirmationMatches
      || (nativeCuaInvokeNeedsActionConfirmation && !nativeCuaInvokeActionConfirmationMatches)
    );
  const nativeCuaInvokeModelPreview = nativeCuaInvokeModelResult
    ? formatUnknownValue(nativeCuaInvokeModelResult)
    : '(no Hermes native CUA model invocation response yet)';
  const nativeCuaApplyModelOutputPreview = nativeCuaApplyModelOutputResult
    ? formatUnknownValue(nativeCuaApplyModelOutputResult)
    : '(no Hermes native CUA model output applied yet)';
  const nativeCuaAuditPreview = nativeCuaAuditExport?.payload
    ?? formatUnknownValue(nativeCuaAuditEvents);
  const nativeCuaLatestAuditTimestamp = nativeCuaAuditEvents[0]?.occurred_at ?? '-';
  const nativeCuaProbeDetails = [
    {
      label: 'Readiness',
      value: nativeCuaProbeResult?.readiness ?? 'No readiness probe reported yet.',
    },
    {
      label: 'Available',
      value: nativeCuaProbeResult ? (nativeCuaProbeResult.available ? 'yes' : 'no') : 'unknown',
    },
    {
      label: 'Safety mode',
      value: nativeCuaProbeResult?.safety_mode ?? 'No safety mode reported yet.',
    },
    {
      label: 'Active session',
      value: nativeCuaProbeResult?.active_session_id ?? nativeCuaResolvedSessionId,
    },
    {
      label: 'Capabilities',
      value: nativeCuaProbeResult?.capabilities.join(', ') || 'No capabilities reported yet.',
    },
    {
      label: 'Warnings',
      value: nativeCuaProbeResult?.warnings.join(', ') || 'No warnings reported yet.',
    },
  ];
  const turixRunDisabled = !turixTask.trim() && !turixResumeAgentId.trim();
  const turixProbeDetails = [
    {
      label: 'Repo',
      value: turixProbeResult
        ? `${turixProbeResult.repo_exists ? 'found' : 'missing'} · ${turixProbeResult.repo_path}`
        : 'No repo probe reported yet.',
    },
    {
      label: 'Config',
      value: turixProbeResult
        ? `${turixProbeResult.config_exists ? 'found' : 'missing'} · ${turixProbeResult.config_path}`
        : 'No config probe reported yet.',
    },
    {
      label: 'Script',
      value: turixProbeResult
        ? `${turixProbeResult.run_script_ready ? 'ready' : 'not ready'} · ${turixProbeResult.run_script_path}`
        : 'No script probe reported yet.',
    },
    {
      label: 'Python entry',
      value: turixProbeResult
        ? `${turixProbeResult.python_entry_exists ? 'found' : 'missing'} · ${turixProbeResult.python_entry_path}`
        : 'No Python entry probe reported yet.',
    },
    {
      label: 'Permissions hint',
      value: turixProbeResult?.permission_hints.join(', ')
        ?? 'No permissions hint reported yet.',
    },
  ];
  const turixProbePreview = turixProbeResult
    ? formatUnknownValue(turixProbeResult)
    : '(run Probe bridge to load local hints)';
  const turixRunPreview = turixRunResult
    ? formatUnknownValue(turixRunResult)
    : '(no TuriX bridge run response yet)';
  const turixAuditPreview = turixAuditExport?.payload
    ?? formatUnknownValue(turixAuditEvents);
  const turixLatestAuditEvent = turixAuditEvents[0];
  const turixLatestAuditTimestamp = turixLatestAuditEvent?.occurred_at ?? '-';
  const turixResumePreview = turixLatestAuditEvent?.resume_agent_id || turixResumeAgentId.trim() || '-';
  const turixRunDryRun = turixRunResult?.dry_run ?? turixDryRun;

  return (
    <div className="runtime-page">
      <h2>运行时管理</h2>

      {error && (
        <div className="error-message">
          <p>{error}</p>
        </div>
      )}

      <div className="runtime-section">
        <h3>Agent Engine</h3>
        <div className="runtime-card">
          <div className="status-row">
            <span className="label">状态:</span>
            <span className={`status-badge ${engine.running ? 'running' : 'stopped'}`}>
              {engine.running ? '运行中' : '已停止'}
            </span>
          </div>
          {engine.profile && (
            <div className="status-row">
              <span className="label">Profile:</span>
              <span>{engine.profile}</span>
            </div>
          )}
          {engine.pid && (
            <div className="status-row">
              <span className="label">PID:</span>
              <span>{engine.pid}</span>
            </div>
          )}
          {engine.started_at ? (
            <div className="status-row">
              <span className="label">Started:</span>
              <span>{engine.started_at}</span>
            </div>
          ) : null}
          {engine.last_heartbeat_at ? (
            <div className="status-row">
              <span className="label">Heartbeat:</span>
              <span>{engine.last_heartbeat_at}</span>
            </div>
          ) : null}
          <div className="status-row">
            <span className="label">Queued bg:</span>
            <span>{engine.queued_background_runs ?? 0}</span>
          </div>
          <div className="status-row">
            <span className="label">Approvals:</span>
            <span>{engine.awaiting_approval_steps ?? 0}</span>
          </div>
          {engine.last_error && (
            <div className="error-detail">
              <span className="label">错误:</span>
              <span>{engine.last_error}</span>
            </div>
          )}

          <div className="action-buttons">
            {!engine.running ? (
              <button
                className="btn primary"
                onClick={handleStart}
                disabled={actionInProgress !== null}
              >
                {actionInProgress === 'start' ? '启动中...' : '启动'}
              </button>
            ) : (
              <>
                <button
                  className="btn danger"
                  onClick={handleStop}
                  disabled={actionInProgress !== null}
                >
                  {actionInProgress === 'stop' ? '停止中...' : '停止'}
                </button>
                <button
                  className="btn"
                  onClick={handleRestart}
                  disabled={actionInProgress !== null}
                >
                  {actionInProgress === 'restart' ? '重启中...' : '重启'}
                </button>
              </>
            )}
          </div>
        </div>
      </div>

      <div className="runtime-section">
        <h3>Application Runtime</h3>
        <div className="runtime-card">
          <div className="status-row">
            <span className="label">组件状态:</span>
            <span>{appRuntime.installed ? '已就绪' : '未就绪'}</span>
          </div>
          <div className="status-row">
            <span className="label">运行状态:</span>
            <span className={`status-badge ${appRuntime.running ? 'running' : 'stopped'}`}>
              {appRuntime.running ? '运行中' : '已停止'}
            </span>
          </div>
          {appRuntime.version && (
            <div className="status-row">
              <span className="label">版本:</span>
              <span>{appRuntime.version}</span>
            </div>
          )}
        </div>
      </div>

      <div className="runtime-section">
        <h3>Local Runtime Adapters</h3>
        <div className="runtime-card runtime-integration-card">
          <p className="runtime-muted">Restricted local adapters execute allowlisted skill-tool commands, probe desktop executor readiness, and summarize trajectory JSONL without training.</p>
          {adapterStatus ? <div className="runtime-status-note">{adapterStatus}</div> : null}
          <div className="runtime-integration-grid">
            <button className="btn" type="button" onClick={handleRunSkillToolSample}>Run allowlisted echo</button>
            <button className="btn" type="button" onClick={() => void handleDesktopAction(true)}>Dry-run desktop action</button>
            <button
              className="btn"
              type="button"
              onClick={() => void handleDesktopAction(false)}
              disabled={nonDryRunDesktopActionDisabled}
            >
              Run desktop action
            </button>
            <button className="btn" type="button" onClick={handleSummarizeTrajectory}>Summarize trajectory</button>
            <button className="btn" type="button" onClick={() => void handleRunLocalRlTraining()}>Train local RL baseline</button>
            <button className="btn" type="button" onClick={() => setTrajectoryJsonl(SAMPLE_TRAJECTORY_JSONL)}>Load parser sample JSONL</button>
            <button className="btn" type="button" onClick={() => void loadRuntimeIntegrations()}>Refresh probes</button>
          </div>
          <div className="runtime-status-note runtime-status-note-warn">
            Dry-run preview audits the allowlisted desktop command plan. Non-dry-run still only calls the existing backend allowlisted executor path and may still be rejected by backend, session, or platform safety checks.
          </div>
          <div className="runtime-danger-panel">
            <div className="runtime-danger-panel__header">
              <strong>Non-dry-run confirmation gate</strong>
              <span>Required before any real desktop action attempt</span>
            </div>
            <label className="runtime-checkbox-row">
              <input
                type="checkbox"
                checked={desktopActionConfirmChecked}
                onChange={(event) => setDesktopActionConfirmChecked(event.target.checked)}
              />
              <span>I understand non-dry-run still only targets an allowlisted executor and may be denied by backend, session, or platform checks.</span>
            </label>
            <label className="runtime-text-field">
              Confirmation phrase
              <input
                value={desktopActionConfirmPhrase}
                onChange={(event) => setDesktopActionConfirmPhrase(event.target.value)}
                placeholder={NON_DRY_RUN_CONFIRM_PHRASE}
                spellCheck={false}
              />
            </label>
            <div className={`runtime-status-note ${nonDryRunDesktopActionDisabled ? 'runtime-status-note-warn' : 'runtime-status-note-ok'}`}>
              {nonDryRunDesktopActionDisabled
                ? desktopActionGuardReasons.join(' ')
                : 'Confirmation gate satisfied. Non-dry-run is enabled, but the backend/session/platform may still reject the request.'}
            </div>
          </div>
          <div className="runtime-inline-metadata">
            <span>Desktop allowlist example: {desktopActionSample.label}</span>
            <span>Executor ready: {desktopActionAvailable ? 'yes' : 'no'}</span>
            <span>JSONL rows: {trajectoryLineCount}</span>
          </div>
          {desktopProbe ? (
            <div className="runtime-adapter-output">
              <strong>Desktop probe</strong>
              <span>{desktopProbe.platform} · graphical: {desktopProbe.has_graphical_session ? 'yes' : 'no'}</span>
              <pre>{JSON.stringify(desktopProbe.tool_availability, null, 2)}</pre>
            </div>
          ) : null}
          {skillToolResult ? (
            <div className="runtime-adapter-output">
              <strong>Skill tool result</strong>
              <span>exit {skillToolResult.exit_code} · {skillToolResult.duration_ms} ms</span>
              <pre>{skillToolResult.stdout || skillToolResult.stderr || '(no output)'}</pre>
            </div>
          ) : null}
          {desktopActionResult ? (
            <div className="runtime-adapter-output">
              <strong>Desktop action</strong>
              <span>{desktopActionResult.executed ? 'executed' : 'dry-run'} · exit {desktopActionResult.exit_code ?? '-'} · {desktopActionResult.duration_ms} ms · timeout: {desktopActionResult.timed_out ? 'yes' : 'no'}</span>
              <pre>{JSON.stringify({
                planned_command: desktopActionResult.planned_command,
                stdout: desktopActionResult.stdout,
                stderr: desktopActionResult.stderr,
                audit_message: desktopActionResult.audit_message,
              }, null, 2)}</pre>
            </div>
          ) : null}
          <label className="runtime-textarea-field">
            GUI automation macro JSON
            <textarea
              rows={7}
              value={guiAutomationJson}
              onChange={(event) => setGuiAutomationJson(event.target.value)}
              placeholder='{"steps":[{"label":"observe","executor":"xdotool","args":["getactivewindow"]}]}'
            />
          </label>
          <label className="runtime-checkbox-row">
            <input
              type="checkbox"
              checked={guiAutomationStopOnError}
              onChange={(event) => setGuiAutomationStopOnError(event.target.checked)}
            />
            <span>Stop macro on first error</span>
          </label>
          <label className="runtime-text-field">
            <span>GUI target remote user id</span>
            <input
              value={guiAutomationTargetRemoteUserId}
              onChange={(event) => setGuiAutomationTargetRemoteUserId(event.target.value)}
              placeholder="Optional future remote user"
            />
            <small>Future remote user routing metadata for local GUI macro audit only.</small>
          </label>
          <div className="runtime-form-grid">
            <label>
              Fill from local Agent Exchange future remote user
              <select
                value={agentExchangeRemoteUsers.some((remoteUser) => remoteUser.user_id === guiAutomationTargetRemoteUserId)
                  ? guiAutomationTargetRemoteUserId
                  : ''}
                onChange={(event) => setGuiAutomationTargetRemoteUserId(event.target.value)}
                disabled={agentExchangeRemoteUsersLoading || agentExchangeRemoteUsers.length === 0}
              >
                <option value="">
                  {agentExchangeRemoteUsersLoading
                    ? 'Loading active local Agent Exchange users...'
                    : 'Choose active local Agent Exchange user'}
                </option>
                {agentExchangeRemoteUsers.map((remoteUser) => (
                  <option key={remoteUser.user_id} value={remoteUser.user_id}>
                    {formatAgentExchangeRemoteUserOption(remoteUser)}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <p className="runtime-muted">{describeAgentExchangeRemoteUsers()}</p>
          <div className="runtime-integration-grid">
            <button className="btn" type="button" onClick={loadGuiAutomationSample}>Load GUI macro sample</button>
            <button className="btn" type="button" onClick={() => void loadAgentExchangeRemoteUsers()}>
              Refresh local Agent Exchange users
            </button>
            <button className="btn" type="button" onClick={() => void handleRunGuiAutomation(true)}>Dry-run GUI macro</button>
            <button
              className="btn"
              type="button"
              onClick={() => void handleRunGuiAutomation(false)}
              disabled={nonDryRunDesktopActionDisabled}
            >
              Run GUI macro
            </button>
          </div>
          {guiAutomationResult ? (
            <div className="runtime-adapter-output">
              <strong>GUI automation macro</strong>
              <span>{guiAutomationResult.dry_run ? 'dry-run' : 'executed'} · {guiAutomationResult.completed_count}/{guiAutomationResult.step_count} steps</span>
              <pre>{JSON.stringify(guiAutomationResult, null, 2)}</pre>
            </div>
          ) : null}
          <label className="runtime-textarea-field">
            Trajectory JSONL
            <textarea
              rows={7}
              value={trajectoryJsonl}
              onChange={(event) => setTrajectoryJsonl(event.target.value)}
              placeholder='Paste JSONL rows here, one event per line. Example: {"kind":"run","source":"local"}'
            />
          </label>
          {trajectorySummary ? (
            <div className="runtime-adapter-output">
              <strong>Trajectory summary</strong>
              <span>{trajectorySummary.line_count} valid rows · {trajectorySummary.invalid_line_count} invalid · {trajectorySummary.reward_hint_count} reward hints</span>
              <pre>{JSON.stringify({ kinds: trajectorySummary.kind_counts, sources: trajectorySummary.source_counts }, null, 2)}</pre>
            </div>
          ) : null}
          {trajectoryTrainingResult ? (
            <div className="runtime-adapter-output">
              <strong>Local RL training artifact</strong>
              <span>{trajectoryTrainingResult.valid_transition_count} transition(s) · {trajectoryTrainingResult.episode_count} episode(s) · avg reward {trajectoryTrainingResult.average_reward}</span>
              <pre>{trajectoryTrainingResult.artifact_json}</pre>
            </div>
          ) : null}
          <div className="runtime-adapter-output">
            <strong>Recent local RL jobs</strong>
            <span>
              {recentTrajectoryTrainingJobs.length > 0
                ? `${recentTrajectoryTrainingJobs.length} persisted job(s) from local settings history`
                : 'No persisted local RL training jobs yet. Run the local baseline once to seed history.'}
            </span>
            <span>
              Each export is a local tabular baseline training artifact, not remote RLHF infrastructure.
              Target remote user id is optional future remote user routing metadata only.
            </span>
            <label className="runtime-text-field">
              Target remote user id
              <input
                value={localRlArtifactTargetRemoteUserId}
                onChange={(event) => setLocalRlArtifactTargetRemoteUserId(event.target.value)}
                placeholder="Optional future remote user"
              />
            </label>
            <div className="runtime-form-grid">
              <label>
                Fill local RL target from Agent Exchange
                <select
                  value={agentExchangeRemoteUsers.some((remoteUser) => remoteUser.user_id === localRlArtifactTargetRemoteUserId)
                    ? localRlArtifactTargetRemoteUserId
                    : ''}
                  onChange={(event) => setLocalRlArtifactTargetRemoteUserId(event.target.value)}
                  disabled={agentExchangeRemoteUsersLoading || agentExchangeRemoteUsers.length === 0}
                >
                  <option value="">
                    {agentExchangeRemoteUsersLoading
                      ? 'Loading active local Agent Exchange users...'
                      : 'Choose active local Agent Exchange user'}
                  </option>
                  {agentExchangeRemoteUsers.map((remoteUser) => (
                    <option key={remoteUser.user_id} value={remoteUser.user_id}>
                      {formatAgentExchangeRemoteUserOption(remoteUser)}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            <div className="action-buttons runtime-wrap-buttons">
              <button className="btn" type="button" onClick={() => void refreshLocalRlTrainingJobs()}>
                Refresh local RL jobs for target
              </button>
            </div>
            {recentTrajectoryTrainingJobs.length > 0 ? (
              <div className="runtime-probe-grid">
                {recentTrajectoryTrainingJobs.map((job) => (
                  <div key={job.job_id} className="runtime-probe-item">
                    <strong>
                      {job.job_name?.trim() || 'unnamed-local-rl-job'}
                      {trajectoryTrainingResult?.job_id === job.job_id ? ' · latest run' : ''}
                    </strong>
                    <span>{job.trained_at}</span>
                    <span><code>{job.job_id}</code></span>
                    <span>
                      {job.valid_transition_count} tx · {job.episode_count} ep · reward {job.average_reward} · epochs {job.epochs}
                    </span>
                    <span>
                      policy {job.policy.length} · invalid {job.invalid_line_count} / {job.input_line_count}
                    </span>
                    <div className="runtime-wrap-buttons">
                      <button className="btn" type="button" onClick={() => void handleCopyLocalRlArtifact(job)}>
                        Copy artifact JSON
                      </button>
                      <button className="btn" type="button" onClick={() => handleDownloadLocalRlArtifact(job)}>
                        Download artifact JSON
                      </button>
                    </div>
                    {localRlArtifactActionState?.jobId === job.job_id ? (
                      <div
                        className={`runtime-status-note ${
                          localRlArtifactActionState.status === 'success'
                            ? 'runtime-status-note-ok'
                            : 'runtime-status-note-warn'
                        }`}
                      >
                        {localRlArtifactActionState.message}
                      </div>
                    ) : null}
                  </div>
                ))}
              </div>
            ) : null}
          </div>
          <div className="runtime-audit-panel">
            <div className="runtime-audit-header">
              <strong>Runtime adapter audit log</strong>
              <span>{runtimeAuditEvents.length} persisted events shown from local SQLite settings</span>
            </div>
            <div className="runtime-form-grid">
              <label>
                Kind filter
                <select value={runtimeAuditKindFilter} onChange={(event) => setRuntimeAuditKindFilter(event.target.value)}>
                  <option value="">all kinds</option>
                  <option value="skill_tool">skill_tool</option>
                  <option value="desktop_action">desktop_action</option>
                  <option value="gui_automation">gui_automation</option>
                  <option value="trajectory_summary">trajectory_summary</option>
                </select>
              </label>
              <label>
                Status filter
                <select value={runtimeAuditStatusFilter} onChange={(event) => setRuntimeAuditStatusFilter(event.target.value)}>
                  <option value="">all statuses</option>
                  <option value="succeeded">succeeded</option>
                  <option value="failed">failed</option>
                  <option value="rejected">rejected</option>
                  <option value="timed_out">timed_out</option>
                </select>
              </label>
              <label>
                Export format
                <select value={runtimeAuditExportFormat} onChange={(event) => setRuntimeAuditExportFormat(event.target.value as 'json' | 'jsonl')}>
                  <option value="json">json</option>
                  <option value="jsonl">jsonl</option>
                </select>
              </label>
            </div>
            <div className="action-buttons runtime-wrap-buttons">
              <button className="btn" type="button" onClick={() => void refreshRuntimeAuditEvents()}>Refresh audit log</button>
              <button className="btn" type="button" onClick={handleExportRuntimeAudit}>Export filtered audit</button>
              <button className="btn" type="button" onClick={() => void handleCopyRuntimeAuditPayload()} disabled={!runtimeAuditExport?.payload}>
                Copy payload
              </button>
              <button className="btn" type="button" onClick={handleDownloadRuntimeAuditHandoff} disabled={!runtimeAuditHandoffPayload}>
                Download handoff JSON
              </button>
            </div>
            {runtimeAuditCopyState ? (
              <div className={`runtime-status-note ${runtimeAuditCopyState.status === 'success' ? 'runtime-status-note-ok' : 'runtime-status-note-warn'}`}>
                {runtimeAuditCopyState.message}
              </div>
            ) : null}
            <div className="runtime-inline-metadata">
              <span>Audit is local-only and records adapter calls without relaxing allowlists.</span>
              <span>Exports are previews for copy/review; no remote sync is implied.</span>
            </div>
            <div className="runtime-adapter-output">
              <strong>{runtimeAuditExport ? `Last ${runtimeAuditExport.format} export` : 'Recent audit events'}</strong>
              <pre>{runtimeAuditPreview || '(no runtime adapter audit events yet)'}</pre>
            </div>
          </div>
        </div>
      </div>

      <div className="runtime-section">
        <h3>Hermes Native CUA Console</h3>
        <div className="runtime-card runtime-integration-card">
          <div className="runtime-native-banner">
            <strong>Hermes native safety surface</strong>
            <span>
              This is Hermes native guarded execution and audit UI. It is not evidence of OSWorld results, SOTA
              ranking, or benchmark-grade desktop-agent performance.
            </span>
          </div>
          <p className="runtime-muted">
            Probe native readiness first, then start or resume a session, observe safely, execute a guarded action,
            and review or export local audit records.
          </p>
          {nativeCuaStatus ? <div className="runtime-status-note">{nativeCuaStatus}</div> : null}
          <div className="runtime-inline-metadata">
            <span>Readiness: {nativeCuaProbeResult?.readiness ?? 'unknown'}</span>
            <span>Session: {nativeCuaResolvedSessionId}</span>
            <span>Latest audit event: {nativeCuaLatestAuditTimestamp}</span>
          </div>
          <div className="runtime-probe-grid runtime-probe-grid--wide">
            {nativeCuaProbeDetails.map((item) => (
              <div key={item.label} className="runtime-probe-item">
                <strong>{item.label}</strong>
                <span>{item.value}</span>
              </div>
            ))}
          </div>
          <div className="action-buttons runtime-wrap-buttons">
            <button className="btn" type="button" onClick={() => void handleProbeNativeCua()}>
              Probe readiness
            </button>
            <button className="btn" type="button" onClick={() => void loadRuntimeIntegrations()}>
              Refresh native data
            </button>
          </div>
          <div className="runtime-adapter-output">
            <strong>Readiness payload</strong>
            <pre>{nativeCuaProbePreview}</pre>
          </div>
          <label className="runtime-textarea-field">
            Session task
            <textarea
              rows={5}
              value={nativeCuaTask}
              onChange={(event) => {
                setNativeCuaTask(event.target.value);
                setNativeCuaModelRoutePreview(null);
              }}
              placeholder="Describe the desktop task Hermes native CUA should reason about before any guarded action."
            />
          </label>
          <div className="runtime-form-grid">
            <label>
              Resume session id
              <input
                value={nativeCuaSessionId}
                onChange={(event) => setNativeCuaSessionId(event.target.value)}
                placeholder="Optional existing session id"
                spellCheck={false}
              />
            </label>
            <label>
              Task model mode
              <select
                value={nativeCuaSessionModelMode}
                onChange={(event) => {
                  setNativeCuaSessionModelMode(event.target.value as 'auto' | 'custom');
                  setNativeCuaModelRoutePreview(null);
                }}
              >
                <option value="auto">Auto · use saved desktop default</option>
                <option value="custom">Custom · use model fields below</option>
              </select>
            </label>
          </div>
          <div className={`runtime-status-note ${nativeCuaSessionModelMode === 'auto' ? 'runtime-status-note-ok' : 'runtime-status-note-warn'}`}>
            Start-task model: <strong>{nativeCuaSessionModelSummary}</strong>. Preview resolves the exact Auto tier/provider/model
            from saved desktop settings before creating a session; save router edits first to include them. Custom stores the current
            provider/model/base_url/api_key_ref on this Native CUA session and takes precedence during invoke.
          </div>
          <div className="action-buttons runtime-wrap-buttons">
            <button className="btn" type="button" onClick={() => void handlePreviewNativeCuaModelRoute()}>
              Preview model route
            </button>
            <button className="btn primary" type="button" onClick={() => void handleStartNativeCuaSession()}>
              {nativeCuaSessionId.trim() ? 'Resume session' : 'Start session'}
            </button>
            <button className="btn" type="button" onClick={() => setNativeCuaTask(SAMPLE_NATIVE_CUA_TASK)}>
              Load boundary sample task
            </button>
          </div>
          <div className="runtime-adapter-output">
            <strong>Model route preview</strong>
            <pre>{nativeCuaModelRoutePreviewText}</pre>
          </div>
          <div className="runtime-adapter-output">
            <strong>Session response</strong>
            <pre>{nativeCuaSessionPreview}</pre>
          </div>
          <div className="runtime-audit-panel">
            <div className="runtime-audit-header">
              <strong>Observe session</strong>
              <span>Dry-run observe can capture a screenshot request without implying benchmark-grade capability.</span>
            </div>
            <div className="runtime-form-grid">
              <label className="runtime-checkbox-card">
                Observe dry-run
                <span>Keep enabled while validating how the backend describes native observation.</span>
                <input
                  type="checkbox"
                  checked={nativeCuaObserveDryRun}
                  onChange={(event) => setNativeCuaObserveDryRun(event.target.checked)}
                />
              </label>
              <label className="runtime-checkbox-card">
                Capture screenshot
                <span>Requests a screenshot as part of observe when the backend allows it.</span>
                <input
                  type="checkbox"
                  checked={nativeCuaObserveCaptureScreenshot}
                  onChange={(event) => setNativeCuaObserveCaptureScreenshot(event.target.checked)}
                />
              </label>
            </div>
            <div className="action-buttons runtime-wrap-buttons">
              <button className="btn" type="button" onClick={() => void handleObserveNativeCua()} disabled={!nativeCuaSessionReady}>
                {nativeCuaObserveDryRun ? 'Observe dry-run' : 'Observe session'}
              </button>
            </div>
            <div className="runtime-adapter-output">
              <strong>Observe response</strong>
              <pre>{nativeCuaObservePreview}</pre>
            </div>
          </div>
          <div className="runtime-audit-panel">
            <div className="runtime-audit-header">
              <strong>Execute action</strong>
              <span>{nativeCuaActionHint}</span>
            </div>
            <div className="runtime-form-grid">
              <label>
                Action type
                <select
                  value={nativeCuaActionForm.actionType}
                  onChange={(event) => setNativeCuaActionForm((current) => ({
                    ...current,
                    actionType: event.target.value as NativeCuaActionType,
                  }))}
                >
                  {NATIVE_CUA_ACTION_OPTIONS.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Text
                <input
                  value={nativeCuaActionForm.text}
                  onChange={(event) => setNativeCuaActionForm((current) => ({ ...current, text: event.target.value }))}
                  placeholder="Optional typed text"
                />
              </label>
              <label>
                Key
                <input
                  value={nativeCuaActionForm.key}
                  onChange={(event) => setNativeCuaActionForm((current) => ({ ...current, key: event.target.value }))}
                  placeholder="Enter, Escape, Tab"
                  spellCheck={false}
                />
              </label>
              <label>
                Modifiers
                <input
                  value={nativeCuaActionForm.modifiers}
                  onChange={(event) => setNativeCuaActionForm((current) => ({ ...current, modifiers: event.target.value }))}
                  placeholder="cmd,shift"
                  spellCheck={false}
                />
              </label>
              <label>
                App
                <input
                  value={nativeCuaActionForm.app}
                  onChange={(event) => setNativeCuaActionForm((current) => ({ ...current, app: event.target.value }))}
                  placeholder="Optional application name"
                />
              </label>
              <label>
                X
                <input
                  value={nativeCuaActionForm.x}
                  onChange={(event) => setNativeCuaActionForm((current) => ({ ...current, x: event.target.value }))}
                  placeholder="120"
                  inputMode="numeric"
                />
              </label>
              <label>
                Y
                <input
                  value={nativeCuaActionForm.y}
                  onChange={(event) => setNativeCuaActionForm((current) => ({ ...current, y: event.target.value }))}
                  placeholder="360"
                  inputMode="numeric"
                />
              </label>
              <label>
                dX
                <input
                  value={nativeCuaActionForm.dx}
                  onChange={(event) => setNativeCuaActionForm((current) => ({ ...current, dx: event.target.value }))}
                  placeholder="0"
                  inputMode="numeric"
                />
              </label>
              <label>
                dY
                <input
                  value={nativeCuaActionForm.dy}
                  onChange={(event) => setNativeCuaActionForm((current) => ({ ...current, dy: event.target.value }))}
                  placeholder="-480"
                  inputMode="numeric"
                />
              </label>
              <label className="runtime-checkbox-card">
                Dry-run
                <span>Keep enabled unless you explicitly intend a real native action attempt.</span>
                <input
                  type="checkbox"
                  checked={nativeCuaActionForm.dryRun}
                  onChange={(event) => setNativeCuaActionForm((current) => ({
                    ...current,
                    dryRun: event.target.checked,
                  }))}
                />
              </label>
            </div>
            <div className="runtime-status-note runtime-status-note-warn">
              Non-dry-run requires the exact phrase <code>{NON_DRY_RUN_NATIVE_CUA_CONFIRM_PHRASE}</code>. This is a
              Hermes native safety gate, not an OSWorld or SOTA capability claim.
            </div>
            <label className="runtime-text-field">
              Confirmation phrase
              <input
                value={nativeCuaActionForm.confirmationPhrase}
                onChange={(event) => setNativeCuaActionForm((current) => ({
                  ...current,
                  confirmationPhrase: event.target.value,
                }))}
                placeholder={NON_DRY_RUN_NATIVE_CUA_CONFIRM_PHRASE}
                spellCheck={false}
              />
            </label>
            <div className={`runtime-status-note ${nativeCuaNonDryRunDisabled ? 'runtime-status-note-warn' : 'runtime-status-note-ok'}`}>
              {nativeCuaActionForm.dryRun
                ? 'Dry-run is enabled. The backend should only simulate the Hermes native CUA action contract.'
                : nativeCuaNonDryRunDisabled
                  ? `Type ${NON_DRY_RUN_NATIVE_CUA_CONFIRM_PHRASE} exactly to unlock non-dry-run native CUA execution.`
                  : 'Confirmation gate satisfied. The backend may still reject the non-dry-run request based on platform or policy safety checks.'}
            </div>
            <div className="action-buttons runtime-wrap-buttons">
              <button
                className="btn"
                type="button"
                onClick={() => void handleExecuteNativeCuaAction()}
                disabled={!nativeCuaSessionReady || nativeCuaNonDryRunDisabled}
              >
                {nativeCuaActionForm.dryRun ? 'Execute dry-run action' : 'Execute native action'}
              </button>
              <button
                className="btn"
                type="button"
                onClick={() => setNativeCuaActionForm(INITIAL_NATIVE_CUA_ACTION_FORM)}
              >
                Reset action form
              </button>
            </div>
            <div className="runtime-adapter-output">
              <strong>Action response</strong>
              <pre>{nativeCuaActionPreview}</pre>
            </div>
          </div>
          <div className="runtime-audit-panel">
            <div className="runtime-audit-header">
              <strong>Full native CUA loop</strong>
              <span>{nativeCuaHistory.length} recent step records · TuriX-compatible action JSON</span>
            </div>
            <div className="runtime-native-banner">
              <strong>Brain / Actor / Planner / Memory substrate</strong>
              <span>
                This loop plans the task, accepts TuriX-style actor actions, executes through the guarded native action
                surface, records memory files, keeps step history, and exports trajectory JSONL. It remains local and
                confirmation-gated; it is not an OSWorld/SOTA claim.
              </span>
            </div>
            <div className="runtime-form-grid">
              <label className="runtime-checkbox-card">
                Step dry-run
                <span>Keep enabled to translate and audit actor actions without OS execution.</span>
                <input
                  type="checkbox"
                  checked={nativeCuaStepDryRun}
                  onChange={(event) => setNativeCuaStepDryRun(event.target.checked)}
                />
              </label>
              <label className="runtime-checkbox-card">
                Step screenshot observe
                <span>Request a screenshot during the loop observe phase; dry-run preview plans it.</span>
                <input
                  type="checkbox"
                  checked={nativeCuaStepCaptureScreenshot}
                  onChange={(event) => setNativeCuaStepCaptureScreenshot(event.target.checked)}
                />
              </label>
              <label>
                Trajectory format
                <select
                  value={nativeCuaAuditExportFormat}
                  onChange={(event) => setNativeCuaAuditExportFormat(event.target.value as NativeCuaAuditExportFormat)}
                >
                  <option value="json">json</option>
                  <option value="jsonl">jsonl</option>
                </select>
              </label>
            </div>
            <label className="runtime-textarea-field">
              Skill catalog JSON
              <textarea
                rows={5}
                value={nativeCuaSkillCatalogJson}
                onChange={(event) => setNativeCuaSkillCatalogJson(event.target.value)}
                spellCheck={false}
              />
            </label>
            <label className="runtime-textarea-field">
              TuriX-compatible step actions JSON
              <textarea
                rows={8}
                value={nativeCuaStepActionsJson}
                onChange={(event) => setNativeCuaStepActionsJson(event.target.value)}
                spellCheck={false}
              />
            </label>
            <div className="action-buttons runtime-wrap-buttons">
              <button className="btn" type="button" onClick={() => void handlePlanNativeCuaTask()} disabled={!nativeCuaSessionReady}>
                Plan loop
              </button>
              <button
                className="btn primary"
                type="button"
                onClick={() => void handleRunNativeCuaStep()}
                disabled={!nativeCuaSessionReady || (!nativeCuaStepDryRun && nativeCuaNonDryRunDisabled)}
              >
                Run loop step
              </button>
              <button className="btn" type="button" onClick={() => void refreshNativeCuaHistory()} disabled={!nativeCuaSessionReady}>
                Refresh history
              </button>
              <button className="btn" type="button" onClick={() => void handleExportNativeCuaTrajectory()} disabled={!nativeCuaSessionReady}>
                Export trajectory
              </button>
            </div>
            <div className="runtime-status-note runtime-status-note-warn">
              Non-dry-run loop steps use the same confirmation phrase field as single actions: <code>{NON_DRY_RUN_NATIVE_CUA_CONFIRM_PHRASE}</code>.
            </div>
            <div className="runtime-adapter-output">
              <strong>Planner response</strong>
              <pre>{nativeCuaPlanPreview}</pre>
            </div>
            <div className="runtime-adapter-output">
              <strong>Step response</strong>
              <pre>{nativeCuaRunStepPreview}</pre>
            </div>
            <div className="runtime-adapter-output">
              <strong>Recent step history</strong>
              <pre>{nativeCuaHistoryPreview}</pre>
            </div>
            <div className="runtime-adapter-output">
              <strong>Trajectory export</strong>
              <pre>{nativeCuaTrajectoryPreview}</pre>
            </div>
          </div>
          <div className="runtime-audit-panel">
            <div className="runtime-audit-header">
              <strong>Model runtime seam</strong>
              <span>Prepare Brain/Actor/Planner/Memory prompts and route guarded model JSON output</span>
            </div>
            <div className="runtime-native-banner">
              <strong>Real VLM adapter boundary</strong>
              <span>
                Use Prepare model turn to create the exact prompt/messages/schema for an external or later in-process model runtime.
                Invoke model calls the backend runtime seam directly and can optionally feed parsed output into the same guarded apply path.
                The manual JSON textarea remains available for paste/edit/replay.
              </span>
            </div>
            <div className="runtime-status-note runtime-status-note-ok">
              Saved desktop model default: <strong>{nativeCuaSavedRuntimeProvider} / {nativeCuaSavedRuntimeModel}</strong> ·
              base_url: <code>{nativeCuaSavedRuntimeBaseUrl}</code> ·
              api_key_ref: <code>{maskRuntimeSecretRef(runtimeSettings?.api_key_ref)}</code>.
              Native CUA invoke uses these defaults whenever the form fields are empty or freshly loaded.
            </div>
            <div className="runtime-audit-panel">
              <div className="runtime-audit-header">
                <strong>Auto model router</strong>
                <span>Configure which model Auto mode should use for easy, standard, and hard desktop tasks.</span>
              </div>
              {NATIVE_CUA_AUTO_MODEL_TIERS.map((tier) => (
                <div className="runtime-audit-panel" key={tier.value}>
                  <div className="runtime-audit-header">
                    <strong>{tier.label}</strong>
                    <span>{tier.hint}</span>
                  </div>
                  <div className="runtime-form-grid">
                    <label>
                      Provider
                      <select
                        value={nativeCuaAutoModelForm[tier.value].provider}
                        onChange={(event) => handleNativeCuaAutoModelProviderChange(tier.value, event.target.value)}
                      >
                        {NATIVE_CUA_MODEL_PROVIDER_PRESETS.map((preset) => (
                          <option key={preset.value} value={preset.value}>
                            {preset.label}
                          </option>
                        ))}
                      </select>
                    </label>
                    <label>
                      Model
                      <input
                        value={nativeCuaAutoModelForm[tier.value].model}
                        onChange={(event) => updateNativeCuaAutoModelTier(tier.value, { model: event.target.value })}
                        spellCheck={false}
                      />
                    </label>
                    <label>
                      Base URL
                      <input
                        value={nativeCuaAutoModelForm[tier.value].base_url}
                        onChange={(event) => updateNativeCuaAutoModelTier(tier.value, { base_url: event.target.value })}
                        spellCheck={false}
                      />
                    </label>
                    <label>
                      API key ref
                      <input
                        value={nativeCuaAutoModelForm[tier.value].api_key_ref}
                        onChange={(event) => updateNativeCuaAutoModelTier(tier.value, { api_key_ref: event.target.value })}
                        placeholder="optional env var override"
                        spellCheck={false}
                      />
                    </label>
                  </div>
                </div>
              ))}
            </div>
            <div className="action-buttons runtime-wrap-buttons">
              <button
                className="btn"
                type="button"
                onClick={() => void handleLoadNativeCuaModelSettings()}
                disabled={nativeCuaModelConfigSaving}
              >
                Load saved model config
              </button>
              <button
                className="btn primary"
                type="button"
                onClick={() => void handleSaveNativeCuaModelSettings()}
                disabled={nativeCuaModelConfigSaving}
              >
                {nativeCuaModelConfigSaving ? 'Saving model config...' : 'Save as desktop default'}
              </button>
            </div>
            <div className="runtime-form-grid">
              <label>
                Model role
                <select
                  value={nativeCuaModelRole}
                  onChange={(event) => setNativeCuaModelRole(event.target.value as NativeCuaModelRole)}
                >
                  <option value="actor">actor</option>
                  <option value="brain">brain</option>
                  <option value="planner">planner</option>
                  <option value="memory">memory</option>
                </select>
              </label>
              <label>
                Provider
                <select
                  value={nativeCuaModelProvider}
                  onChange={(event) => handleNativeCuaModelProviderChange(event.target.value)}
                >
                  {NATIVE_CUA_MODEL_PROVIDER_PRESETS.map((preset) => (
                    <option key={preset.value} value={preset.value}>
                      {preset.label}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Model
                <input
                  value={nativeCuaModelName}
                  onChange={(event) => {
                    setNativeCuaModelName(event.target.value);
                    setNativeCuaModelRoutePreview(null);
                  }}
                  placeholder={nativeCuaSelectedModelPreset?.defaultModel ?? 'gpt-4o'}
                  spellCheck={false}
                />
              </label>
              <label>
                Base URL
                <input
                  value={nativeCuaModelBaseUrl}
                  onChange={(event) => {
                    setNativeCuaModelBaseUrl(event.target.value);
                    setNativeCuaModelRoutePreview(null);
                  }}
                  placeholder={nativeCuaSelectedModelPreset?.defaultBaseUrl ?? 'https://api.example.com/v1'}
                  spellCheck={false}
                />
              </label>
              <label>
                API key ref
                <input
                  value={nativeCuaModelApiKeyRef}
                  onChange={(event) => {
                    setNativeCuaModelApiKeyRef(event.target.value);
                    setNativeCuaModelRoutePreview(null);
                  }}
                  placeholder="OPENAI_API_KEY / OPENROUTER_API_KEY"
                  spellCheck={false}
                />
              </label>
              <label className="runtime-checkbox-card">
                Invoke dry-run
                <span>Default on. Turn off only when you intend a real external model request.</span>
                <input
                  type="checkbox"
                  checked={nativeCuaInvokeDryRun}
                  onChange={(event) => setNativeCuaInvokeDryRun(event.target.checked)}
                />
              </label>
              <label className="runtime-checkbox-card">
                Apply output
                <span>Default on. Parsed output is routed through the guarded apply seam; live actions still require their own gate.</span>
                <input
                  type="checkbox"
                  checked={nativeCuaInvokeApplyOutput}
                  onChange={(event) => setNativeCuaInvokeApplyOutput(event.target.checked)}
                />
              </label>
              <label className="runtime-checkbox-card">
                Apply dry-run
                <span>Actor output uses the same dry-run gate as loop steps.</span>
                <input
                  type="checkbox"
                  checked={nativeCuaStepDryRun}
                  onChange={(event) => setNativeCuaStepDryRun(event.target.checked)}
                />
              </label>
              <label>
                Model confirmation phrase
                <input
                  value={nativeCuaModelConfirmationPhrase}
                  onChange={(event) => setNativeCuaModelConfirmationPhrase(event.target.value)}
                  placeholder={NON_DRY_RUN_NATIVE_CUA_MODEL_CONFIRM_PHRASE}
                  spellCheck={false}
                />
              </label>
            </div>
            <label className="runtime-textarea-field">
              Extra model context
              <textarea
                rows={3}
                value={nativeCuaModelExtraContext}
                onChange={(event) => setNativeCuaModelExtraContext(event.target.value)}
              />
            </label>
            <div className="runtime-status-note">
              Provider hint: {nativeCuaSelectedModelPreset?.note ?? 'Supported providers are OpenAI-compatible, Anthropic, DeepSeek, OpenRouter, and Ollama.'}
              {' '}The API key ref is an environment variable name, not a raw secret value; provider-specific env vars remain the fallback.
            </div>
            <div className="runtime-status-note runtime-status-note-warn">
              Non-dry-run can call an external or paid model endpoint. This seam is not OSWorld proof, benchmark evidence,
              or SOTA validation. Use <code>{NON_DRY_RUN_NATIVE_CUA_MODEL_CONFIRM_PHRASE}</code> exactly for live model
              invocation. If live Apply output stays enabled, the existing action phrase <code>{NON_DRY_RUN_NATIVE_CUA_CONFIRM_PHRASE}</code>
              must also be set in the Execute action controls.
            </div>
            <label className="runtime-textarea-field">
              Model output JSON (manual paste/edit path)
              <textarea
                rows={8}
                value={nativeCuaModelOutputJson}
                onChange={(event) => setNativeCuaModelOutputJson(event.target.value)}
                spellCheck={false}
              />
            </label>
            <div className="action-buttons runtime-wrap-buttons">
              <button className="btn" type="button" onClick={() => void handlePrepareNativeCuaModelTurn()} disabled={!nativeCuaSessionReady}>
                Prepare model turn
              </button>
              <button
                className="btn"
                type="button"
                onClick={() => void handleInvokeNativeCuaModel()}
                disabled={!nativeCuaSessionReady || nativeCuaInvokeNonDryRunDisabled}
              >
                Invoke model
              </button>
              <button
                className="btn primary"
                type="button"
                onClick={() => void handleApplyNativeCuaModelOutput()}
                disabled={!nativeCuaSessionReady || (!nativeCuaStepDryRun && nativeCuaNonDryRunDisabled)}
              >
                Apply model output
              </button>
            </div>
            <div className="runtime-adapter-output">
              <strong>Prepared model turn</strong>
              <pre>{nativeCuaModelTurnPreview}</pre>
            </div>
            <div className="runtime-adapter-output">
              <strong>Invoke response preview</strong>
              <pre>{nativeCuaInvokeModelPreview}</pre>
            </div>
            <div className="runtime-adapter-output">
              <strong>Applied model output</strong>
              <pre>{nativeCuaApplyModelOutputPreview}</pre>
            </div>
          </div>
          <div className="runtime-audit-panel">
            <div className="runtime-audit-header">
              <strong>Hermes native CUA audit</strong>
              <span>{nativeCuaAuditEvents.length} events shown from the native safety surface</span>
            </div>
            <div className="runtime-form-grid">
              <label>
                Session filter
                <input
                  value={nativeCuaSessionId}
                  onChange={(event) => setNativeCuaSessionId(event.target.value)}
                  placeholder="Optional session id"
                  spellCheck={false}
                />
              </label>
              <label>
                Event type filter
                <input
                  value={nativeCuaAuditEventTypeFilter}
                  onChange={(event) => setNativeCuaAuditEventTypeFilter(event.target.value)}
                  placeholder="observe, execute_action"
                  spellCheck={false}
                />
              </label>
              <label>
                Status filter
                <input
                  value={nativeCuaAuditStatusFilter}
                  onChange={(event) => setNativeCuaAuditStatusFilter(event.target.value)}
                  placeholder="succeeded, failed, rejected"
                  spellCheck={false}
                />
              </label>
              <label>
                Search query
                <input
                  value={nativeCuaAuditQuery}
                  onChange={(event) => setNativeCuaAuditQuery(event.target.value)}
                  placeholder="Match event type, session, summary"
                />
              </label>
              <label>
                Export format
                <select
                  value={nativeCuaAuditExportFormat}
                  onChange={(event) => setNativeCuaAuditExportFormat(event.target.value as NativeCuaAuditExportFormat)}
                >
                  <option value="json">json</option>
                  <option value="jsonl">jsonl</option>
                </select>
              </label>
            </div>
            <div className="action-buttons runtime-wrap-buttons">
              <button className="btn" type="button" onClick={() => void refreshNativeCuaAuditEvents()}>
                Refresh audit
              </button>
              <button className="btn" type="button" onClick={() => void handleExportNativeCuaAudit()}>
                Export audit
              </button>
              <button className="btn" type="button" onClick={() => void handleCopyNativeCuaAuditPayload()} disabled={!nativeCuaAuditExport?.payload}>
                Copy payload
              </button>
              <button className="btn" type="button" onClick={handleDownloadNativeCuaAuditPayload} disabled={!nativeCuaAuditExport?.payload}>
                Download Native CUA audit payload
              </button>
            </div>
            {nativeCuaAuditCopyState ? (
              <div className={`runtime-status-note ${nativeCuaAuditCopyState.status === 'success' ? 'runtime-status-note-ok' : 'runtime-status-note-warn'}`}>
                {nativeCuaAuditCopyState.message}
              </div>
            ) : null}
            <div className="runtime-inline-metadata">
              <span>Audit list/export stay local to Hermes native execution history.</span>
              <span>Copy/export is for review only and does not imply remote sync or benchmark validation.</span>
            </div>
            <div className="runtime-adapter-output">
              <strong>{nativeCuaAuditExport ? 'Last audit export payload' : 'Recent audit events'}</strong>
              <pre>{nativeCuaAuditPreview || '(no Hermes native CUA audit events yet)'}</pre>
            </div>
          </div>
        </div>
      </div>

      <div className="runtime-section">
        <h3>TuriX CUA Bridge Console</h3>
        <div className="runtime-card runtime-integration-card">
          <div className="runtime-bridge-banner">
            <strong>Local bridge only</strong>
            <span>
              This panel targets a local TuriX runtime bridge. It does not mean Hermes natively recreates OSWorld
              or any SOTA desktop-agent runtime.
            </span>
          </div>
          <p className="runtime-muted">
            Use probe to inspect repo/config/script/python/permissions hints, then issue a local run request or export
            bridge audit events for review.
          </p>
          {turixStatus ? <div className="runtime-status-note">{turixStatus}</div> : null}
          <div className="runtime-inline-metadata">
            <span>Bridge readiness: {turixProbeResult?.status ?? 'unknown'}</span>
            <span>Dry-run default: {turixDryRun ? 'enabled' : 'disabled'}</span>
            <span>Latest audit event: {turixLatestAuditTimestamp}</span>
          </div>
          <div className="runtime-probe-grid">
            {turixProbeDetails.map((item) => (
              <div key={item.label} className="runtime-probe-item">
                <strong>{item.label}</strong>
                <span>{item.value}</span>
              </div>
            ))}
          </div>
          <div className="action-buttons runtime-wrap-buttons">
            <button className="btn" type="button" onClick={() => void handleProbeTurixBridge()}>
              Probe bridge
            </button>
            <button className="btn" type="button" onClick={() => void loadRuntimeIntegrations()}>
              Refresh bridge data
            </button>
          </div>
          <div className="runtime-adapter-output">
            <strong>Bridge probe payload</strong>
            <pre>{turixProbePreview}</pre>
          </div>
          <label className="runtime-textarea-field">
            CUA task
            <textarea
              rows={5}
              value={turixTask}
              onChange={(event) => setTurixTask(event.target.value)}
              placeholder="Describe the local desktop or workflow task for the external TuriX bridge; keep dry-run enabled until local readiness is proven."
            />
          </label>
          <div className="runtime-form-grid">
            <label>
              Resume agent id
              <input
                value={turixResumeAgentId}
                onChange={(event) => setTurixResumeAgentId(event.target.value)}
                placeholder="Optional resume agent id"
                spellCheck={false}
              />
            </label>
            <label className="runtime-checkbox-card">
              Dry-run
              <span>Prefer enabled while validating a local bridge request contract.</span>
              <input
                type="checkbox"
                checked={turixDryRun}
                onChange={(event) => setTurixDryRun(event.target.checked)}
              />
            </label>
          </div>
          <div className="runtime-status-note runtime-status-note-warn">
            Dry-run should be the default while command names and payloads are still aligning. Non-dry-run behavior is
            backend-defined and may still be rejected by local policy or permissions checks.
          </div>
          <div className="action-buttons runtime-wrap-buttons">
            <button className="btn primary" type="button" onClick={() => void handleRunTurixBridge()} disabled={turixRunDisabled}>
              {turixDryRun ? 'Run dry-run request' : 'Run bridge request'}
            </button>
            <button className="btn" type="button" onClick={() => setTurixTask(SAMPLE_TURIX_TASK)}>
              Load boundary sample task
            </button>
          </div>
          <div className="runtime-adapter-output">
            <strong>Run response</strong>
            <span>
              resume_agent_id: {turixResumePreview} · launcher: {turixRunResult?.launcher ?? '-'} · dry_run:{' '}
              {turixRunDryRun ? 'yes' : 'no'}
            </span>
            <pre>{turixRunPreview}</pre>
          </div>
          <div className="runtime-audit-panel">
            <div className="runtime-audit-header">
              <strong>TuriX bridge audit</strong>
              <span>{turixAuditEvents.length} events shown from the local bridge audit surface</span>
            </div>
            <div className="runtime-form-grid">
              <label>
                Action filter
                <input
                  value={turixAuditKindFilter}
                  onChange={(event) => setTurixAuditKindFilter(event.target.value)}
                  placeholder="start"
                  spellCheck={false}
                />
              </label>
              <label>
                Status filter
                <input
                  value={turixAuditStatusFilter}
                  onChange={(event) => setTurixAuditStatusFilter(event.target.value)}
                  placeholder="succeeded, failed, rejected"
                  spellCheck={false}
                />
              </label>
              <label>
                Search query
                <input
                  value={turixAuditQuery}
                  onChange={(event) => setTurixAuditQuery(event.target.value)}
                  placeholder="Match task, status, summary"
                />
              </label>
              <label>
                Export format
                <select
                  value={turixAuditExportFormat}
                  onChange={(event) => setTurixAuditExportFormat(event.target.value as TurixCuaAuditExportFormat)}
                >
                  <option value="json">json</option>
                  <option value="jsonl">jsonl</option>
                </select>
              </label>
            </div>
            <div className="action-buttons runtime-wrap-buttons">
              <button className="btn" type="button" onClick={() => void refreshTurixAuditEvents()}>
                Refresh audit
              </button>
              <button className="btn" type="button" onClick={() => void handleExportTurixAudit()}>
                Export audit
              </button>
              <button className="btn" type="button" onClick={() => void handleCopyTurixAuditPayload()} disabled={!turixAuditExport?.payload}>
                Copy payload
              </button>
              <button className="btn" type="button" onClick={handleDownloadTurixAuditPayload} disabled={!turixAuditExport?.payload}>
                Download TuriX audit payload
              </button>
            </div>
            {turixAuditCopyState ? (
              <div className={`runtime-status-note ${turixAuditCopyState.status === 'success' ? 'runtime-status-note-ok' : 'runtime-status-note-warn'}`}>
                {turixAuditCopyState.message}
              </div>
            ) : null}
            <div className="runtime-inline-metadata">
              <span>Audit list/export are local review surfaces; no remote sync or benchmark claim is implied.</span>
              <span>Prefer explicit backend field alignment before wiring deeper automation.</span>
            </div>
            <div className="runtime-adapter-output">
              <strong>{turixAuditExport ? 'Last audit export payload' : 'Recent audit events'}</strong>
              <pre>{turixAuditPreview || '(no TuriX bridge audit events yet)'}</pre>
            </div>
          </div>
        </div>
      </div>

      <div className="runtime-section">
        <h3>Local Team Governance</h3>
        <div className="runtime-card runtime-integration-card">
          <p className="runtime-muted">Local RBAC, audit events, and JSON bundle sync run from the desktop database. This is a real local service, not cloud RBAC.</p>
          {teamStatus ? <div className="runtime-status-note">{teamStatus}</div> : null}
          <div className="runtime-form-grid">
            <label>
              Actor member
              <input value={teamActorId} onChange={(event) => setTeamActorId(event.target.value)} />
            </label>
            <label>
              Member
              <input value={teamMemberId} onChange={(event) => setTeamMemberId(event.target.value)} />
            </label>
            <label>
              Role
              <select value={teamMemberRole} onChange={(event) => setTeamMemberRole(event.target.value as TeamRole)}>
                <option value="owner">owner</option>
                <option value="admin">admin</option>
                <option value="editor">editor</option>
                <option value="viewer">viewer</option>
              </select>
            </label>
            <label>
              Resource
              <input value={teamAccessResource} onChange={(event) => setTeamAccessResource(event.target.value)} />
            </label>
            <label>
              Action
              <input value={teamAccessAction} onChange={(event) => setTeamAccessAction(event.target.value)} />
            </label>
            <label>
              Sync file path
              <input
                value={teamFolderSyncPath}
                onChange={(event) => setTeamFolderSyncPath(event.target.value)}
                placeholder="/tmp/hermes-team-bundle.json"
              />
            </label>
          </div>
          <div className="action-buttons runtime-wrap-buttons">
            <button className="btn primary" type="button" onClick={handleBootstrapOwner}>Bootstrap owner</button>
            <button className="btn" type="button" onClick={handleUpsertTeamMember}>Upsert member</button>
            <button className="btn" type="button" onClick={handleCheckTeamAccess}>Check access</button>
            <button className="btn" type="button" onClick={handleExportTeamBundle}>Export bundle</button>
            <button className="btn" type="button" onClick={handleImportTeamBundle} disabled={!teamBundleJson.trim()}>Import bundle</button>
            <button className="btn" type="button" onClick={handleRunTeamFolderSync}>Run file sync</button>
          </div>
          {teamAccessDecision ? (
            <div className={`runtime-status-note ${teamAccessDecision.allowed ? 'runtime-status-note-ok' : 'runtime-status-note-warn'}`}>
              {teamAccessDecision.allowed ? 'Allowed' : 'Denied'} · {teamAccessDecision.reason}
            </div>
          ) : null}
          {teamState ? (
            <div className="runtime-adapter-output">
              <strong>{teamState.members.length} members · {teamState.audit_events.length} audit events</strong>
              <pre>{JSON.stringify(teamState.members, null, 2)}</pre>
            </div>
          ) : null}
          <div className="runtime-audit-panel">
            <div className="runtime-audit-header">
              <strong>Audit event browser</strong>
              <span>
                {filteredAuditEvents.length} / {selectedAuditEvents.length} events
                {teamAuditSource === 'bundle' ? ' from exported bundle preview' : ' from live state'}
              </span>
            </div>
            <div className="runtime-form-grid">
              <label>
                Audit source
                <select value={teamAuditSource} onChange={(event) => setTeamAuditSource(event.target.value as 'state' | 'bundle')}>
                  <option value="state">live state</option>
                  <option value="bundle" disabled={!exportedTeamBundle}>exported bundle</option>
                </select>
              </label>
              <label>
                Actor filter
                <select value={teamAuditActorFilter} onChange={(event) => setTeamAuditActorFilter(event.target.value)}>
                  <option value="">all actors</option>
                  {availableAuditActors.map((actorId) => (
                    <option key={actorId} value={actorId}>{actorId}</option>
                  ))}
                </select>
              </label>
              <label>
                Action filter
                <select value={teamAuditActionFilter} onChange={(event) => setTeamAuditActionFilter(event.target.value)}>
                  <option value="">all actions</option>
                  {availableAuditActions.map((action) => (
                    <option key={action} value={action}>{action}</option>
                  ))}
                </select>
              </label>
              <label>
                Search
                <input
                  value={teamAuditSearch}
                  onChange={(event) => setTeamAuditSearch(event.target.value)}
                  placeholder="Match id, actor, action, detail, timestamp"
                />
              </label>
              <label>
                Backend export format
                <select value={teamAuditExportFormat} onChange={(event) => setTeamAuditExportFormat(event.target.value as TeamSyncAuditExportFormat)}>
                  <option value="json">json</option>
                  <option value="jsonl">jsonl</option>
                </select>
              </label>
            </div>
            <div className="action-buttons runtime-wrap-buttons">
              <button className="btn" type="button" onClick={handleExportTeamAudit}>Export audit via RBAC</button>
              <button className="btn" type="button" onClick={() => void handleCopyTeamAuditPayload()} disabled={!teamAuditBackendExport?.payload}>
                Copy payload
              </button>
            </div>
            {teamAuditCopyState ? (
              <div className={`runtime-status-note ${teamAuditCopyState.status === 'success' ? 'runtime-status-note-ok' : 'runtime-status-note-warn'}`}>
                {teamAuditCopyState.message}
              </div>
            ) : null}
            <div className="runtime-inline-metadata">
              <span>Use Export bundle to capture a shareable bundle snapshot.</span>
              <span>Backend audit export checks local RBAC and records an export audit event.</span>
              <span>The preview below is local-only and does not bypass backend policy.</span>
            </div>
            <div className="runtime-adapter-output">
              <strong>{teamAuditBackendExport ? 'Backend audit export payload' : 'Filtered export preview'}</strong>
              <span>Derived from {teamAuditBackendExport ? 'teamSyncExportAudit' : teamAuditSource === 'bundle' ? 'teamSyncExportBundle' : 'teamSyncGetState'} data.</span>
              <pre>{teamAuditBackendPreview}</pre>
            </div>
          </div>
          <label className="runtime-textarea-field">
            Team bundle JSON
            <textarea rows={6} value={teamBundleJson} onChange={(event) => setTeamBundleJson(event.target.value)} />
          </label>
        </div>
      </div>

      <div className="runtime-section">
        <h3>Foreground Snapshot</h3>
        <div className="runtime-card">
          <div className="status-row">
            <span className="label">活跃状态:</span>
            <span className={`status-badge ${foreground.active ? 'running' : 'stopped'}`}>
              {foreground.active ? '活跃' : '空闲'}
            </span>
          </div>
          <div className="status-row">
            <span className="label">Foreground state:</span>
            <span>{foreground.state}</span>
          </div>
          <div className="status-row">
            <span className="label">Session ID:</span>
            <span>{foreground.session_id ?? '-'}</span>
          </div>
          <div className="status-row">
            <span className="label">Run ID:</span>
            <span>{foreground.run_id ?? '-'}</span>
          </div>
          <div className="status-row">
            <span className="label">Cancel state:</span>
            <span>{foreground.cancel_state ?? '-'}</span>
          </div>
          <div className="status-row">
            <span className="label">Pending count:</span>
            <span>{foreground.pending_count}</span>
          </div>
          <div className="status-row">
            <span className="label">Interrupt count:</span>
            <span>{foreground.interrupt_count}</span>
          </div>
          <div className="status-row">
            <span className="label">Updated at:</span>
            <span>{foreground.updated_at || '-'}</span>
          </div>
        </div>
      </div>
    </div>
  );
}
