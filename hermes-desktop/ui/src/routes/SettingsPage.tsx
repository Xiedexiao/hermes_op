import { useEffect, useMemo, useState } from 'react';
import { useAppStore } from '../store/appStore';
import { useRuntimeStore } from '../store/runtimeStore';
import {
  appGetWorkspaceDiagnostics,
  parityCronCreate,
  parityCronList,
  parityCronRunNow,
  parityCronRuntimeStatus,
  parityCronRuntimeTick,
  parityCronSetEnabled,
  parityGetCatalog,
  parityGetRuntimeReadiness,
  parityMcpList,
  parityMcpProbe,
  parityMcpRuntimeListStatus,
  parityMcpRuntimeReload,
  parityMcpRuntimeStart,
  parityMcpRuntimeStop,
  parityMcpUpsert,
  parityQuickCommandList,
  parityQuickCommandSave,
  paritySaveProviderSelection,
  settingsGet,
  settingsSave,
  terminalBackendListProfiles,
  terminalBackendListStatus,
  terminalBackendSaveProfile,
  terminalBackendTestProfile,
  type AppSettings,
  type ParityCatalog,
  type ParityCronRuntimeStatus,
  type ParityCronJob,
  type ParityMcpProbeResult,
  type ParityMcpServer,
  type ParityMcpServerRuntimeStatus,
  type ParityQuickCommand,
  type ParityRuntimeReadiness,
  type RuntimeSettings,
  type TerminalBackendProfile,
  type TerminalBackendStatus,
  type WorkspaceDiagnosticsPayload,
} from '../lib/tauri';
import './SettingsPage.css';

type ProviderPreset = {
  label: string;
  defaultModel: string;
  defaultBaseUrl: string | null;
  note: string;
};

const PROVIDER_PRESETS: Record<string, ProviderPreset> = {
  openai: {
    label: 'OpenAI',
    defaultModel: 'gpt-4o',
    defaultBaseUrl: 'https://api.openai.com/v1',
    note: '默认适合云端 API 直连，通常只需要 model 与 API key ref。',
  },
  anthropic: {
    label: 'Anthropic',
    defaultModel: 'claude-3-7-sonnet-latest',
    defaultBaseUrl: 'https://api.anthropic.com',
    note: '适合 Claude 模型；base URL 为空时可沿用 runtime 默认解析。',
  },
  deepseek: {
    label: 'DeepSeek',
    defaultModel: 'deepseek-chat',
    defaultBaseUrl: 'https://api.deepseek.com',
    note: '常见于高性价比对话与推理模型接入。',
  },
  ollama: {
    label: 'Ollama',
    defaultModel: 'llama3.1',
    defaultBaseUrl: 'http://localhost:11434',
    note: '本地模型优先，通常需要明确 base URL 与 engine profile。',
  },
  openrouter: {
    label: 'OpenRouter',
    defaultModel: 'openai/gpt-4o-mini',
    defaultBaseUrl: 'https://openrouter.ai/api/v1',
    note: '适合统一聚合多 provider，model 名通常带 provider 前缀。',
  },
};

type NormalizedAppSettings = {
  theme_mode: string;
  language: string;
  launch_at_login: boolean;
  default_workspace_path: string;
  log_level: string;
  require_approval_for_risk: string;
};

type NormalizedRuntimeSettings = {
  provider: string;
  model: string;
  base_url: string;
  api_key_ref: string;
  engine_profile: string;
  agent_engine_enabled: boolean;
  busy_input_mode: string;
  native_cua_auto_models: RuntimeSettings['native_cua_auto_models'];
};

type CronFormState = {
  name: string;
  schedule: string;
  prompt: string;
  deliver_to: string;
  enabled: boolean;
};

type McpFormState = {
  id: string;
  name: string;
  transport: string;
  endpoint: string;
  enabled: boolean;
  tool_filter_mode: string;
  allowed_tools_text: string;
  blocked_tools_text: string;
  resources_enabled: boolean;
  prompts_enabled: boolean;
};

type QuickCommandFormState = {
  id: string;
  name: string;
  command: string;
  description: string;
  enabled: boolean;
};

type TerminalBackendFormState = {
  id: string;
  kind: string;
  display_name: string;
  enabled: boolean;
  config_text: string;
};

const emptyCronForm: CronFormState = {
  name: '',
  schedule: '0 9 * * *',
  prompt: '',
  deliver_to: '',
  enabled: true,
};

const emptyMcpForm: McpFormState = {
  id: '',
  name: '',
  transport: 'stdio',
  endpoint: '',
  enabled: true,
  tool_filter_mode: 'allow_all',
  allowed_tools_text: '',
  blocked_tools_text: '',
  resources_enabled: true,
  prompts_enabled: true,
};

const emptyQuickCommandForm: QuickCommandFormState = {
  id: '',
  name: '',
  command: '',
  description: '',
  enabled: true,
};

const emptyTerminalBackendForm: TerminalBackendFormState = {
  id: '',
  kind: 'local',
  display_name: '',
  enabled: true,
  config_text: '{}',
};

function normalizeAppSettings(settings: AppSettings | null | undefined): NormalizedAppSettings {
  return {
    theme_mode: settings?.theme_mode ?? 'system',
    language: settings?.language ?? 'zh-CN',
    launch_at_login: settings?.launch_at_login ?? false,
    default_workspace_path: settings?.default_workspace_path ?? '',
    log_level: settings?.log_level ?? 'info',
    require_approval_for_risk: settings?.require_approval_for_risk ?? 'high',
  };
}

function normalizeRuntimeSettings(
  settings: RuntimeSettings | null | undefined,
): NormalizedRuntimeSettings {
  return {
    provider: settings?.provider ?? 'openai',
    model: settings?.model ?? '',
    base_url: settings?.base_url ?? '',
    api_key_ref: settings?.api_key_ref ?? '',
    engine_profile: settings?.engine_profile ?? 'default',
    agent_engine_enabled: settings?.agent_engine_enabled ?? true,
    busy_input_mode: settings?.busy_input_mode ?? 'interrupt',
    native_cua_auto_models: settings?.native_cua_auto_models ?? null,
  };
}

function maskValue(value: string) {
  if (!value.trim()) {
    return '未配置';
  }

  if (value.length <= 6) {
    return '已配置';
  }

  return `${value.slice(0, 3)}••••${value.slice(-2)}`;
}

function formatSavedTime(value: string | null) {
  if (!value) {
    return '尚未保存本次更改';
  }

  return new Intl.DateTimeFormat('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(value));
}

function formatDateTime(value: string | null | undefined) {
  if (!value) {
    return 'never';
  }

  return new Intl.DateTimeFormat('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(value));
}

function splitList(value: string) {
  return value
    .split(/[\n,]/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function isAppSettingsEqual(left: AppSettings, right: AppSettings) {
  const normalizedLeft = normalizeAppSettings(left);
  const normalizedRight = normalizeAppSettings(right);
  return JSON.stringify(normalizedLeft) === JSON.stringify(normalizedRight);
}

function isRuntimeSettingsEqual(left: RuntimeSettings, right: RuntimeSettings) {
  const normalizedLeft = normalizeRuntimeSettings(left);
  const normalizedRight = normalizeRuntimeSettings(right);
  return JSON.stringify(normalizedLeft) === JSON.stringify(normalizedRight);
}

export function SettingsPage() {
  const { setAppSettings, setRuntimeSettings, runtimeSettings } = useAppStore();
  const { engine, appRuntime } = useRuntimeStore();
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastSavedAt, setLastSavedAt] = useState<string | null>(null);
  const [saveNoticeVisible, setSaveNoticeVisible] = useState(false);
  const [activeAppSettings, setActiveAppSettings] = useState<AppSettings>({});
  const [activeRuntimeSettings, setActiveRuntimeSettings] = useState<RuntimeSettings>({});
  const [formAppSettings, setFormAppSettings] = useState<AppSettings>({});
  const [formRuntimeSettings, setFormRuntimeSettings] = useState<RuntimeSettings>({});
  const [parityCatalog, setParityCatalog] = useState<ParityCatalog | null>(null);
  const [runtimeReadiness, setRuntimeReadiness] = useState<ParityRuntimeReadiness | null>(null);
  const [cronJobs, setCronJobs] = useState<ParityCronJob[]>([]);
  const [mcpServers, setMcpServers] = useState<ParityMcpServer[]>([]);
  const [quickCommands, setQuickCommands] = useState<ParityQuickCommand[]>([]);
  const [cronRuntimeStatus, setCronRuntimeStatus] = useState<ParityCronRuntimeStatus | null>(null);
  const [mcpRuntimeStatuses, setMcpRuntimeStatuses] = useState<ParityMcpServerRuntimeStatus[]>([]);
  const [mcpProbeById, setMcpProbeById] = useState<Record<string, ParityMcpProbeResult>>({});
  const [terminalProfiles, setTerminalProfiles] = useState<TerminalBackendProfile[]>([]);
  const [terminalStatuses, setTerminalStatuses] = useState<TerminalBackendStatus[]>([]);
  const [parityLoading, setParityLoading] = useState(false);
  const [parityAction, setParityAction] = useState<string | null>(null);
  const [parityError, setParityError] = useState<string | null>(null);
  const [parityNotice, setParityNotice] = useState<string | null>(null);
  const [terminalLoading, setTerminalLoading] = useState(false);
  const [terminalAction, setTerminalAction] = useState<string | null>(null);
  const [terminalError, setTerminalError] = useState<string | null>(null);
  const [terminalNotice, setTerminalNotice] = useState<string | null>(null);
  const [cronForm, setCronForm] = useState<CronFormState>(emptyCronForm);
  const [mcpForm, setMcpForm] = useState<McpFormState>(emptyMcpForm);
  const [quickCommandForm, setQuickCommandForm] =
    useState<QuickCommandFormState>(emptyQuickCommandForm);
  const [terminalBackendForm, setTerminalBackendForm] =
    useState<TerminalBackendFormState>(emptyTerminalBackendForm);
  const [workspaceDiagnostics, setWorkspaceDiagnostics] =
    useState<WorkspaceDiagnosticsPayload | null>(null);

  useEffect(() => {
    void loadSettings();
    void loadParitySurfaces();
    void loadTerminalBackends();
  }, []);

  useEffect(() => {
    if (!saveNoticeVisible) {
      return;
    }

    const timer = window.setTimeout(() => setSaveNoticeVisible(false), 3200);
    return () => window.clearTimeout(timer);
  }, [saveNoticeVisible]);

  async function loadSettings() {
    setLoading(true);
    setError(null);
    try {
      const [data, diagnostics] = await Promise.all([
        settingsGet(),
        appGetWorkspaceDiagnostics(),
      ]);
      const normalizedRuntime = normalizeRuntimeSettings(data.runtime);
      setActiveAppSettings(data.app);
      setActiveRuntimeSettings(normalizedRuntime);
      setFormAppSettings(data.app);
      setFormRuntimeSettings(normalizedRuntime);
      setAppSettings(data.app);
      setRuntimeSettings(normalizedRuntime);
      setWorkspaceDiagnostics(diagnostics);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }

  async function loadParitySurfaces() {
    setParityLoading(true);
    setParityError(null);
    try {
      const [catalog, readiness, jobs, servers, commands, cronRuntime, mcpStatuses] = await Promise.all([
        parityGetCatalog(),
        parityGetRuntimeReadiness(),
        parityCronList(),
        parityMcpList(),
        parityQuickCommandList(),
        parityCronRuntimeStatus(),
        parityMcpRuntimeListStatus(),
      ]);
      setParityCatalog(catalog);
      setRuntimeReadiness(readiness);
      setCronJobs(jobs);
      setMcpServers(servers);
      setQuickCommands(commands);
      setCronRuntimeStatus(cronRuntime);
      setMcpRuntimeStatuses(mcpStatuses);
      setMcpForm((current) => ({
        ...current,
        tool_filter_mode: current.tool_filter_mode || catalog.mcp_filter_modes[0] || 'allow_all',
      }));
    } catch (err) {
      setParityError(err instanceof Error ? err.message : String(err));
    } finally {
      setParityLoading(false);
    }
  }

  async function loadTerminalBackends() {
    setTerminalLoading(true);
    setTerminalError(null);
    try {
      const [profiles, statuses] = await Promise.all([
        terminalBackendListProfiles(),
        terminalBackendListStatus(),
      ]);
      setTerminalProfiles(profiles);
      setTerminalStatuses(statuses);
    } catch (err) {
      setTerminalError(err instanceof Error ? err.message : String(err));
    } finally {
      setTerminalLoading(false);
    }
  }

  const normalizedActiveRuntime = useMemo(
    () => normalizeRuntimeSettings(runtimeSettings ?? activeRuntimeSettings),
    [activeRuntimeSettings, runtimeSettings],
  );
  const normalizedDraftRuntime = useMemo(
    () => normalizeRuntimeSettings(formRuntimeSettings),
    [formRuntimeSettings],
  );
  const selectedProviderPreset =
    PROVIDER_PRESETS[normalizedDraftRuntime.provider] ?? PROVIDER_PRESETS.openai;
  const catalogProviderCount = parityCatalog?.providers.length ?? 0;
  const catalogModelCount =
    parityCatalog?.providers.reduce((total, provider) => total + provider.models.length, 0) ?? 0;
  const mcpFilterModes = parityCatalog?.mcp_filter_modes.length
    ? parityCatalog.mcp_filter_modes
    : ['allow_all', 'allowlist', 'blocklist'];
  const mcpRuntimeById = useMemo(
    () => new Map(mcpRuntimeStatuses.map((status) => [status.id, status])),
    [mcpRuntimeStatuses],
  );

  const appDirty = useMemo(
    () => !isAppSettingsEqual(formAppSettings, activeAppSettings),
    [activeAppSettings, formAppSettings],
  );
  const runtimeDirty = useMemo(
    () => !isRuntimeSettingsEqual(formRuntimeSettings, activeRuntimeSettings),
    [activeRuntimeSettings, formRuntimeSettings],
  );
  const hasDirtyChanges = appDirty || runtimeDirty;

  const runtimeChangedFields = useMemo(() => {
    const changed: string[] = [];

    if (normalizedActiveRuntime.provider !== normalizedDraftRuntime.provider) {
      changed.push('provider');
    }
    if (normalizedActiveRuntime.model !== normalizedDraftRuntime.model) {
      changed.push('model');
    }
    if (normalizedActiveRuntime.base_url !== normalizedDraftRuntime.base_url) {
      changed.push('base_url');
    }
    if (normalizedActiveRuntime.api_key_ref !== normalizedDraftRuntime.api_key_ref) {
      changed.push('api_key_ref');
    }
    if (normalizedActiveRuntime.engine_profile !== normalizedDraftRuntime.engine_profile) {
      changed.push('engine_profile');
    }
    if (
      normalizedActiveRuntime.agent_engine_enabled !==
      normalizedDraftRuntime.agent_engine_enabled
    ) {
      changed.push('agent_engine_enabled');
    }
    if (normalizedActiveRuntime.busy_input_mode !== normalizedDraftRuntime.busy_input_mode) {
      changed.push('busy_input_mode');
    }

    return changed;
  }, [normalizedActiveRuntime, normalizedDraftRuntime]);

  const saveStateLabel = saving
    ? '保存中...'
    : error
      ? '保存失败'
      : hasDirtyChanges
        ? '有未保存更改'
        : lastSavedAt
          ? `已保存 ${formatSavedTime(lastSavedAt)}`
          : '已与当前设置同步';
  const saveStateTone = saving
    ? 'loading'
    : error
      ? 'error'
      : hasDirtyChanges
        ? 'dirty'
        : 'saved';

  async function handleSave() {
    setSaving(true);
    setError(null);
    try {
      await settingsSave({
        app: formAppSettings,
        runtime: normalizedDraftRuntime,
      });
      if (normalizedDraftRuntime.provider.trim() && normalizedDraftRuntime.model.trim()) {
        const selection = await paritySaveProviderSelection({
          provider: normalizedDraftRuntime.provider,
          model: normalizedDraftRuntime.model,
          base_url: normalizedDraftRuntime.base_url || null,
        });
        setParityCatalog((current) =>
          current
            ? {
                ...current,
                active_provider: selection.provider,
                active_model: selection.model,
              }
            : current,
        );
      }
      setActiveAppSettings(formAppSettings);
      setActiveRuntimeSettings(normalizedDraftRuntime);
      setAppSettings(formAppSettings);
      setRuntimeSettings(normalizedDraftRuntime);
      setLastSavedAt(new Date().toISOString());
      setSaveNoticeVisible(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  }

  async function handleCreateCronJob(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setParityAction('cron-create');
    setParityError(null);
    setParityNotice(null);
    try {
      const created = await parityCronCreate({
        name: cronForm.name,
        schedule: cronForm.schedule,
        prompt: cronForm.prompt,
        deliver_to: cronForm.deliver_to.trim() || null,
        enabled: cronForm.enabled,
      });
      setCronJobs((current) => [created, ...current]);
      setCronForm(emptyCronForm);
      setParityNotice('Cron metadata saved. Scheduler execution is still owned by the runtime layer.');
    } catch (err) {
      setParityError(err instanceof Error ? err.message : String(err));
    } finally {
      setParityAction(null);
    }
  }

  async function handleSetCronEnabled(job: ParityCronJob, enabled: boolean) {
    setParityAction(`cron-enabled:${job.id}`);
    setParityError(null);
    setParityNotice(null);
    try {
      const updated = await parityCronSetEnabled(job.id, enabled);
      setCronJobs((current) => current.map((item) => (item.id === updated.id ? updated : item)));
      setParityNotice(enabled ? 'Cron job resumed in metadata.' : 'Cron job paused in metadata.');
    } catch (err) {
      setParityError(err instanceof Error ? err.message : String(err));
    } finally {
      setParityAction(null);
    }
  }

  async function handleRunCronNow(job: ParityCronJob) {
    setParityAction(`cron-run:${job.id}`);
    setParityError(null);
    setParityNotice(null);
    try {
      const updated = await parityCronRunNow(job.id);
      setCronJobs((current) => current.map((item) => (item.id === updated.id ? updated : item)));
      const runtime = await parityCronRuntimeStatus();
      setCronRuntimeStatus(runtime);
      setParityNotice('Run-now request dispatched through the local cron runtime.');
    } catch (err) {
      setParityError(err instanceof Error ? err.message : String(err));
    } finally {
      setParityAction(null);
    }
  }

  async function handleCronRuntimeTick() {
    setParityAction('cron-tick');
    setParityError(null);
    setParityNotice(null);
    try {
      const result = await parityCronRuntimeTick();
      setCronRuntimeStatus(await parityCronRuntimeStatus());
      setCronJobs(await parityCronList());
      setParityNotice(
        `Cron runtime checked ${result.checked_jobs} jobs and dispatched ${result.dispatched_jobs}.`,
      );
    } catch (err) {
      setParityError(err instanceof Error ? err.message : String(err));
    } finally {
      setParityAction(null);
    }
  }

  async function handleUpsertMcpServer(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setParityAction('mcp-upsert');
    setParityError(null);
    setParityNotice(null);
    try {
      const saved = await parityMcpUpsert({
        id: mcpForm.id.trim() || null,
        name: mcpForm.name,
        transport: mcpForm.transport,
        endpoint: mcpForm.endpoint,
        enabled: mcpForm.enabled,
        tool_filter_mode: mcpForm.tool_filter_mode,
        allowed_tools: splitList(mcpForm.allowed_tools_text),
        blocked_tools: splitList(mcpForm.blocked_tools_text),
        resources_enabled: mcpForm.resources_enabled,
        prompts_enabled: mcpForm.prompts_enabled,
      });
      setMcpServers((current) => {
        const exists = current.some((item) => item.id === saved.id);
        return exists
          ? current.map((item) => (item.id === saved.id ? saved : item))
          : [saved, ...current];
      });
      setMcpForm(emptyMcpForm);
      setParityNotice('MCP server metadata saved. Process startup/reload remains a runtime responsibility.');
    } catch (err) {
      setParityError(err instanceof Error ? err.message : String(err));
    } finally {
      setParityAction(null);
    }
  }

  async function handleSaveQuickCommand(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setParityAction('quick-command-save');
    setParityError(null);
    setParityNotice(null);
    try {
      const saved = await parityQuickCommandSave({
        id: quickCommandForm.id.trim() || null,
        name: quickCommandForm.name,
        command: quickCommandForm.command,
        description: quickCommandForm.description.trim() || null,
        enabled: quickCommandForm.enabled,
      });
      setQuickCommands((current) => {
        const exists = current.some((item) => item.id === saved.id);
        return exists
          ? current.map((item) => (item.id === saved.id ? saved : item))
          : [saved, ...current];
      });
      setQuickCommandForm(emptyQuickCommandForm);
      setParityNotice('Quick command saved for slash-command parity metadata.');
    } catch (err) {
      setParityError(err instanceof Error ? err.message : String(err));
    } finally {
      setParityAction(null);
    }
  }

  async function handleMcpRuntimeAction(
    serverId: string,
    action: 'start' | 'reload' | 'stop',
  ) {
    setParityAction(`mcp-${action}:${serverId}`);
    setParityError(null);
    setParityNotice(null);
    try {
      const updated =
        action === 'start'
          ? await parityMcpRuntimeStart(serverId)
          : action === 'reload'
            ? await parityMcpRuntimeReload(serverId)
            : await parityMcpRuntimeStop(serverId);

      setMcpRuntimeStatuses((current) => {
        const exists = current.some((item) => item.id === updated.id);
        return exists
          ? current.map((item) => (item.id === updated.id ? updated : item))
          : [updated, ...current];
      });
      setParityNotice(
        action === 'start'
          ? 'MCP server started.'
          : action === 'reload'
            ? 'MCP server reloaded.'
            : 'MCP server stopped.',
      );
    } catch (err) {
      setParityError(err instanceof Error ? err.message : String(err));
    } finally {
      setParityAction(null);
    }
  }

  async function handleProbeMcpServer(serverId: string) {
    setParityAction(`mcp-probe:${serverId}`);
    setParityError(null);
    try {
      const result = await parityMcpProbe(serverId);
      setMcpProbeById((current) => ({ ...current, [serverId]: result }));
      setParityNotice(`${result.name}: ${result.message}`);
    } catch (err) {
      setParityError(err instanceof Error ? err.message : String(err));
    } finally {
      setParityAction(null);
    }
  }

  async function handleSaveTerminalBackend(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setTerminalAction('save');
    setTerminalError(null);
    setTerminalNotice(null);
    try {
      const config = JSON.parse(terminalBackendForm.config_text) as Record<string, unknown>;
      const saved = await terminalBackendSaveProfile({
        id: terminalBackendForm.id.trim() || null,
        kind: terminalBackendForm.kind,
        display_name: terminalBackendForm.display_name,
        enabled: terminalBackendForm.enabled,
        config,
      });
      const [profiles, statuses] = await Promise.all([
        terminalBackendListProfiles(),
        terminalBackendListStatus(),
      ]);
      setTerminalProfiles(profiles);
      setTerminalStatuses(statuses);
      setTerminalBackendForm({
        id: saved.id,
        kind: saved.kind,
        display_name: saved.display_name,
        enabled: saved.enabled,
        config_text: JSON.stringify(saved.config, null, 2),
      });
      setTerminalNotice('Terminal backend profile saved.');
    } catch (err) {
      setTerminalError(err instanceof Error ? err.message : String(err));
    } finally {
      setTerminalAction(null);
    }
  }

  async function handleTestTerminalBackend(id: string) {
    setTerminalAction(`test:${id}`);
    setTerminalError(null);
    setTerminalNotice(null);
    try {
      const result = await terminalBackendTestProfile(id);
      setTerminalStatuses(await terminalBackendListStatus());
      setTerminalNotice(`${result.kind} test ${result.status}: ${result.message}`);
    } catch (err) {
      setTerminalError(err instanceof Error ? err.message : String(err));
    } finally {
      setTerminalAction(null);
    }
  }

  function handleEditTerminalBackend(profile: TerminalBackendProfile) {
    setTerminalBackendForm({
      id: profile.id,
      kind: profile.kind,
      display_name: profile.display_name,
      enabled: profile.enabled,
      config_text: JSON.stringify(profile.config, null, 2),
    });
    setTerminalNotice(`Loaded ${profile.display_name} into the editor.`);
  }

  function handleProviderChange(provider: string) {
    const preset = PROVIDER_PRESETS[provider] ?? PROVIDER_PRESETS.openai;
    setFormRuntimeSettings((current) => ({
      ...current,
      provider,
      model: current.model?.trim() ? current.model : preset.defaultModel,
      base_url: current.base_url?.trim()
        ? current.base_url
        : preset.defaultBaseUrl ?? '',
      engine_profile: current.engine_profile?.trim() ? current.engine_profile : 'default',
    }));
  }

  if (loading) {
    return (
      <div className="settings-page">
        <div className="loading">加载中...</div>
      </div>
    );
  }

  return (
    <div className="settings-page">
      <div className="settings-header">
        <div>
          <h2>Settings</h2>
          <p className="settings-header-copy">
            当前页只接入已有设置读写契约。Provider / Model 的切换会立即反映到 UI store，
            真实连接校验仍由 runtime 负责。
          </p>
        </div>
        <div className="settings-header-actions">
          <div className={`settings-save-status settings-save-status-${saveStateTone}`}>
            {saveStateLabel}
          </div>
          <button className="btn" type="button" onClick={() => void loadSettings()} disabled={saving}>
            重新加载
          </button>
          <button
            className="btn primary"
            type="button"
            onClick={() => void handleSave()}
            disabled={saving || !hasDirtyChanges}
          >
            {saving ? '保存中...' : '保存设置'}
          </button>
        </div>
      </div>

      {error ? <div className="error-message">{error}</div> : null}
      {saveNoticeVisible ? (
        <div className="success-message">
          设置已保存。当前 active provider / model 已同步到页面状态。
        </div>
      ) : null}

      <div className="settings-overview">
        <article className="settings-overview-card">
          <span className="settings-overview-label">Active Runtime</span>
          <strong>
            {PROVIDER_PRESETS[normalizedActiveRuntime.provider]?.label ?? normalizedActiveRuntime.provider}
            {' / '}
            {normalizedActiveRuntime.model || '未设置 model'}
          </strong>
          <p>
            base_url: {normalizedActiveRuntime.base_url || '默认'}
            <br />
            api_key_ref: {maskValue(normalizedActiveRuntime.api_key_ref)}
            <br />
            engine_profile: {normalizedActiveRuntime.engine_profile}
            <br />
            busy_input_mode: {normalizedActiveRuntime.busy_input_mode}
          </p>
        </article>
        <article className="settings-overview-card">
          <span className="settings-overview-label">Draft Delta</span>
          <strong>{runtimeChangedFields.length > 0 ? `${runtimeChangedFields.length} 项变更` : '无 runtime 差异'}</strong>
          <p>
            {runtimeChangedFields.length > 0
              ? runtimeChangedFields.join(', ')
              : 'Draft 已与 active runtime 设置保持一致。'}
          </p>
        </article>
        <article className="settings-overview-card">
          <span className="settings-overview-label">Engine Surface</span>
          <strong>
            {engine.running ? 'Agent Engine 运行中' : 'Agent Engine 未运行'}
          </strong>
          <p>
            runtime: {appRuntime.running ? 'running' : appRuntime.installed ? 'installed' : 'unavailable'}
            <br />
            profile: {engine.profile ?? normalizedActiveRuntime.engine_profile}
            <br />
            switch: {normalizedDraftRuntime.agent_engine_enabled ? 'enabled' : 'disabled'}
            <br />
            busy input: {normalizedDraftRuntime.busy_input_mode}
          </p>
        </article>
      </div>

      <div className="settings-sections">
        <section className="settings-section">
          <h3>App</h3>

          <div className="form-group">
            <label>主题</label>
            <select
              value={formAppSettings.theme_mode || 'system'}
              onChange={(event) =>
                setFormAppSettings({ ...formAppSettings, theme_mode: event.target.value })
              }
            >
              <option value="system">跟随系统</option>
              <option value="light">浅色</option>
              <option value="dark">深色</option>
            </select>
          </div>

          <div className="form-group">
            <label>语言</label>
            <select
              value={formAppSettings.language || 'zh-CN'}
              onChange={(event) =>
                setFormAppSettings({ ...formAppSettings, language: event.target.value })
              }
            >
              <option value="zh-CN">简体中文</option>
              <option value="en-US">English</option>
            </select>
          </div>

          <div className="form-group checkbox">
            <label>
              <input
                type="checkbox"
                checked={formAppSettings.launch_at_login || false}
                onChange={(event) =>
                  setFormAppSettings({
                    ...formAppSettings,
                    launch_at_login: event.target.checked,
                  })
                }
              />
              开机启动
            </label>
          </div>

          <div className="form-group">
            <label>默认工作目录</label>
            <input
              type="text"
              value={formAppSettings.default_workspace_path || ''}
              onChange={(event) =>
                setFormAppSettings({
                  ...formAppSettings,
                  default_workspace_path: event.target.value,
                })
              }
              placeholder="/path/to/workspace"
            />
          </div>

          <div className="form-group">
            <label>日志级别</label>
            <select
              value={formAppSettings.log_level || 'info'}
              onChange={(event) =>
                setFormAppSettings({ ...formAppSettings, log_level: event.target.value })
              }
            >
              <option value="debug">Debug</option>
              <option value="info">Info</option>
              <option value="warn">Warning</option>
              <option value="error">Error</option>
            </select>
          </div>

          <div className="form-group">
            <label>高风险操作审批</label>
            <select
              value={formAppSettings.require_approval_for_risk || 'high'}
              onChange={(event) =>
                setFormAppSettings({
                  ...formAppSettings,
                  require_approval_for_risk: event.target.value,
                })
              }
            >
              <option value="high">仅高风险</option>
              <option value="medium">中风险及以上</option>
              <option value="never">不需要审批</option>
            </select>
          </div>
        </section>

        <section className="settings-section">
          <h3>Runtime</h3>

          <div className="settings-provider-panel">
            <div>
              <span className="settings-overview-label">Active provider</span>
              <strong>
                {PROVIDER_PRESETS[normalizedActiveRuntime.provider]?.label ?? normalizedActiveRuntime.provider}
                {' / '}
                {normalizedActiveRuntime.model || '未设置 model'}
              </strong>
              <p>{selectedProviderPreset.note}</p>
            </div>
            <div className="settings-provider-badges">
              <span className="settings-pill">base_url: {normalizedActiveRuntime.base_url || '默认'}</span>
              <span className="settings-pill">
                api_key_ref: {maskValue(normalizedActiveRuntime.api_key_ref)}
              </span>
              <span className="settings-pill">profile: {normalizedActiveRuntime.engine_profile}</span>
              <span className="settings-pill">busy input: {normalizedActiveRuntime.busy_input_mode}</span>
            </div>
          </div>

          <div className="form-group">
            <label>Provider</label>
            <select
              value={normalizedDraftRuntime.provider}
              onChange={(event) => handleProviderChange(event.target.value)}
            >
              {Object.entries(PROVIDER_PRESETS).map(([value, preset]) => (
                <option key={value} value={value}>
                  {preset.label}
                </option>
              ))}
            </select>
            <div className="field-hint">
              推荐 model: {selectedProviderPreset.defaultModel}
              {' · '}
              推荐 base_url: {selectedProviderPreset.defaultBaseUrl ?? '使用默认'}
            </div>
          </div>

          <div className="form-group">
            <label>Model</label>
            <input
              type="text"
              value={formRuntimeSettings.model || ''}
              onChange={(event) =>
                setFormRuntimeSettings({ ...formRuntimeSettings, model: event.target.value })
              }
              placeholder={selectedProviderPreset.defaultModel}
            />
            <div className="field-hint">
              active: {normalizedActiveRuntime.model || '未设置'}
            </div>
          </div>

          <div className="form-group">
            <label>Base URL</label>
            <input
              type="text"
              value={formRuntimeSettings.base_url || ''}
              onChange={(event) =>
                setFormRuntimeSettings({ ...formRuntimeSettings, base_url: event.target.value })
              }
              placeholder={selectedProviderPreset.defaultBaseUrl ?? '留空表示使用默认 endpoint'}
            />
            <div className="field-hint">
              active: {normalizedActiveRuntime.base_url || '默认 endpoint'}
            </div>
          </div>

          <div className="form-group">
            <label>API Key Ref</label>
            <input
              type="password"
              value={formRuntimeSettings.api_key_ref || ''}
              onChange={(event) =>
                setFormRuntimeSettings({
                  ...formRuntimeSettings,
                  api_key_ref: event.target.value,
                })
              }
              placeholder="OPENAI_API_KEY 或内部 secret ref"
            />
            <div className="field-hint">
              active: {maskValue(normalizedActiveRuntime.api_key_ref)}
            </div>
          </div>

          <div className="form-group">
            <label>Engine Profile</label>
            <input
              type="text"
              value={formRuntimeSettings.engine_profile || ''}
              onChange={(event) =>
                setFormRuntimeSettings({
                  ...formRuntimeSettings,
                  engine_profile: event.target.value,
                })
              }
              placeholder="default"
            />
            <div className="field-hint">
              active: {normalizedActiveRuntime.engine_profile}
            </div>
          </div>

          <div className="form-group checkbox">
            <label>
              <input
                type="checkbox"
                checked={formRuntimeSettings.agent_engine_enabled ?? true}
                onChange={(event) =>
                  setFormRuntimeSettings({
                    ...formRuntimeSettings,
                    agent_engine_enabled: event.target.checked,
                  })
                }
              />
              启用 Agent Engine
            </label>
            <div className="field-hint">
              保存的是开关偏好；真实启停状态仍取决于 runtime 当前运行态。
            </div>
          </div>

          <div className="form-group">
            <label>Busy Input Mode</label>
            <select
              value={formRuntimeSettings.busy_input_mode || 'interrupt'}
              onChange={(event) =>
                setFormRuntimeSettings({
                  ...formRuntimeSettings,
                  busy_input_mode: event.target.value,
                })
              }
            >
              <option value="interrupt">interrupt</option>
              <option value="queue">queue</option>
            </select>
            <div className="field-hint">
              active: {normalizedActiveRuntime.busy_input_mode}
              {' · '}
              CLI 忙碌时回车行为，`interrupt` 会打断当前前台轮次，`queue` 会排队等待。
            </div>
          </div>
        </section>

        <section className="settings-section settings-parity-section">
          <div className="settings-section-heading-row">
            <div>
              <h3>Hermes Parity Surfaces</h3>
              <p>
                这里保存 Hermes 对标所需的 provider catalog、cron、MCP 与 quick command
                控制面元数据，并提供 cron run-now/tick、MCP start/reload/stop 与 quick command 保存入口。
              </p>
            </div>
            <button
              className="btn"
              type="button"
              onClick={() => void loadParitySurfaces()}
              disabled={parityLoading || parityAction !== null}
            >
              {parityLoading ? '刷新中...' : '刷新 parity'}
            </button>
          </div>

          {parityError ? <div className="error-message">{parityError}</div> : null}
          {parityNotice ? <div className="success-message">{parityNotice}</div> : null}

          <div className="settings-parity-grid">
            <article className="settings-parity-card settings-parity-card-wide">
              <div className="settings-parity-card-header">
                <div>
                  <span className="settings-overview-label">Provider Catalog</span>
                  <strong>
                    {catalogProviderCount} providers / {catalogModelCount} models
                  </strong>
                  <p>
                    active: {parityCatalog?.active_provider ?? normalizedActiveRuntime.provider} /{' '}
                    {parityCatalog?.active_model ?? normalizedActiveRuntime.model}
                  </p>
                  {runtimeReadiness ? (
                    <>
                      <p>
                        readiness: {runtimeReadiness.status} · {runtimeReadiness.message}
                      </p>
                      <p>
                        auth: {runtimeReadiness.auth.label}
                        {runtimeReadiness.auth.env_var ? ` · ${runtimeReadiness.auth.env_var}` : ''}
                        {' · '}
                        {runtimeReadiness.can_authenticate ? 'usable' : 'missing'}
                      </p>
                      <p>
                        sources:{' '}
                        {runtimeReadiness.sources
                          .map((source) =>
                            `${source.label}${source.env_var ? ` (${source.env_var})` : ''}: ${
                              source.available ? 'ready' : 'missing'
                            }`,
                          )
                          .join(' · ')}
                      </p>
                    </>
                  ) : null}
                </div>
              </div>
              <div className="settings-provider-catalog-list">
                {(parityCatalog?.providers ?? []).map((provider) => (
                  <div className="settings-provider-catalog-item" key={provider.id}>
                    <div>
                      <strong>{provider.display_name}</strong>
                      <span>
                        {provider.id}
                        {provider.supports_custom_endpoint ? ' · custom endpoint' : ''}
                      </span>
                    </div>
                    <div className="settings-provider-models">
                      {provider.models.map((model) => (
                        <span className="settings-pill" key={model.id}>
                          {model.display_name}
                          {model.recommended ? ' · recommended' : ''}
                        </span>
                      ))}
                    </div>
                  </div>
                ))}
                {!parityCatalog && !parityLoading ? (
                  <div className="settings-parity-empty">Provider catalog 尚未加载。</div>
                ) : null}
              </div>
            </article>

            <article className="settings-parity-card">
              <div className="settings-parity-card-header">
                <div>
                  <span className="settings-overview-label">Cron Scheduling</span>
                  <strong>{cronJobs.length} jobs</strong>
                  <p>保存 cron 表达式、prompt、deliver_to 和 run-now 请求元数据。</p>
                </div>
                <div className="settings-parity-runtime-chip">
                  <strong>{cronRuntimeStatus?.status ?? 'unknown'}</strong>
                  <span>
                    heartbeat {formatDateTime(cronRuntimeStatus?.last_heartbeat_at)}
                    {' · '}
                    last dispatch {cronRuntimeStatus?.last_dispatch_count ?? 0}
                  </span>
                </div>
              </div>
              <form className="settings-parity-form" onSubmit={handleCreateCronJob}>
                <div className="form-group">
                  <label>Cron name</label>
                  <input
                    type="text"
                    value={cronForm.name}
                    onChange={(event) => setCronForm({ ...cronForm, name: event.target.value })}
                    placeholder="Daily planning sweep"
                    required
                  />
                </div>
                <div className="settings-form-row">
                  <div className="form-group">
                    <label>Schedule</label>
                    <input
                      type="text"
                      value={cronForm.schedule}
                      onChange={(event) =>
                        setCronForm({ ...cronForm, schedule: event.target.value })
                      }
                      placeholder="0 9 * * *"
                      required
                    />
                  </div>
                  <div className="form-group">
                    <label>Deliver to</label>
                    <input
                      type="text"
                      value={cronForm.deliver_to}
                      onChange={(event) =>
                        setCronForm({ ...cronForm, deliver_to: event.target.value })
                      }
                      placeholder="desktop / slack / email"
                    />
                  </div>
                </div>
                <div className="form-group">
                  <label>Prompt</label>
                  <textarea
                    value={cronForm.prompt}
                    onChange={(event) => setCronForm({ ...cronForm, prompt: event.target.value })}
                    placeholder="Summarize active missions and flag blocked work."
                    required
                  />
                </div>
                <div className="form-group checkbox">
                  <label>
                    <input
                      type="checkbox"
                      checked={cronForm.enabled}
                      onChange={(event) =>
                        setCronForm({ ...cronForm, enabled: event.target.checked })
                      }
                    />
                    enabled
                  </label>
                </div>
                <button className="btn primary" type="submit" disabled={parityAction === 'cron-create'}>
                  {parityAction === 'cron-create' ? '保存中...' : '创建 cron metadata'}
                </button>
                <button
                  className="btn"
                  type="button"
                  onClick={() => void handleCronRuntimeTick()}
                  disabled={parityAction === 'cron-tick'}
                >
                  {parityAction === 'cron-tick' ? '轮询中...' : '手动 tick runtime'}
                </button>
              </form>
              <div className="settings-parity-list">
                {cronJobs.map((job) => (
                  <div className="settings-parity-list-item" key={job.id}>
                    <div>
                      <strong>{job.name}</strong>
                      <span>
                        {job.schedule} · {job.status} · runs {job.run_count}
                      </span>
                      <span>last run request: {formatDateTime(job.last_run_requested_at)}</span>
                    </div>
                    <div className="settings-parity-actions">
                      <button
                        className="btn"
                        type="button"
                        onClick={() => void handleRunCronNow(job)}
                        disabled={parityAction === `cron-run:${job.id}`}
                      >
                        Run now
                      </button>
                      <button
                        className="btn"
                        type="button"
                        onClick={() => void handleSetCronEnabled(job, !job.enabled)}
                        disabled={parityAction === `cron-enabled:${job.id}`}
                      >
                        {job.enabled ? 'Pause' : 'Resume'}
                      </button>
                    </div>
                  </div>
                ))}
                {cronJobs.length === 0 ? (
                  <div className="settings-parity-empty">还没有 cron metadata。</div>
                ) : null}
              </div>
            </article>

            <article className="settings-parity-card">
              <div className="settings-parity-card-header">
                <div>
                  <span className="settings-overview-label">MCP Servers</span>
                  <strong>{mcpServers.length} servers</strong>
                  <p>保存 MCP endpoint、工具过滤与 resources/prompts 开关，并可控制 stdio server 启停。</p>
                </div>
              </div>
              <form className="settings-parity-form" onSubmit={handleUpsertMcpServer}>
                <input
                  type="hidden"
                  value={mcpForm.id}
                  onChange={(event) => setMcpForm({ ...mcpForm, id: event.target.value })}
                />
                <div className="settings-form-row">
                  <div className="form-group">
                    <label>Name</label>
                    <input
                      type="text"
                      value={mcpForm.name}
                      onChange={(event) => setMcpForm({ ...mcpForm, name: event.target.value })}
                      placeholder="filesystem"
                      required
                    />
                  </div>
                  <div className="form-group">
                    <label>Transport</label>
                    <select
                      value={mcpForm.transport}
                      onChange={(event) =>
                        setMcpForm({ ...mcpForm, transport: event.target.value })
                      }
                    >
                      <option value="stdio">stdio</option>
                      <option value="http">http</option>
                      <option value="sse">sse</option>
                    </select>
                  </div>
                </div>
                <div className="form-group">
                  <label>Endpoint / command</label>
                  <input
                    type="text"
                    value={mcpForm.endpoint}
                    onChange={(event) => setMcpForm({ ...mcpForm, endpoint: event.target.value })}
                    placeholder="npx @modelcontextprotocol/server-filesystem"
                    required
                  />
                </div>
                <div className="settings-form-row">
                  <div className="form-group">
                    <label>Tool filter</label>
                    <select
                      value={mcpForm.tool_filter_mode}
                      onChange={(event) =>
                        setMcpForm({ ...mcpForm, tool_filter_mode: event.target.value })
                      }
                    >
                      {mcpFilterModes.map((mode) => (
                        <option key={mode} value={mode}>
                          {mode}
                        </option>
                      ))}
                    </select>
                  </div>
                  <div className="form-group checkbox">
                    <label>
                      <input
                        type="checkbox"
                        checked={mcpForm.enabled}
                        onChange={(event) =>
                          setMcpForm({ ...mcpForm, enabled: event.target.checked })
                        }
                      />
                      enabled
                    </label>
                  </div>
                </div>
                <div className="settings-form-row">
                  <div className="form-group">
                    <label>Allowed tools</label>
                    <textarea
                      value={mcpForm.allowed_tools_text}
                      onChange={(event) =>
                        setMcpForm({ ...mcpForm, allowed_tools_text: event.target.value })
                      }
                      placeholder="tool_one, tool_two"
                    />
                  </div>
                  <div className="form-group">
                    <label>Blocked tools</label>
                    <textarea
                      value={mcpForm.blocked_tools_text}
                      onChange={(event) =>
                        setMcpForm({ ...mcpForm, blocked_tools_text: event.target.value })
                      }
                      placeholder="dangerous_tool"
                    />
                  </div>
                </div>
                <div className="settings-checkbox-row">
                  <label>
                    <input
                      type="checkbox"
                      checked={mcpForm.resources_enabled}
                      onChange={(event) =>
                        setMcpForm({ ...mcpForm, resources_enabled: event.target.checked })
                      }
                    />
                    resources
                  </label>
                  <label>
                    <input
                      type="checkbox"
                      checked={mcpForm.prompts_enabled}
                      onChange={(event) =>
                        setMcpForm({ ...mcpForm, prompts_enabled: event.target.checked })
                      }
                    />
                    prompts
                  </label>
                </div>
                <button className="btn primary" type="submit" disabled={parityAction === 'mcp-upsert'}>
                  {parityAction === 'mcp-upsert' ? '保存中...' : '保存 MCP metadata'}
                </button>
              </form>
              <div className="settings-parity-list">
                {mcpServers.map((server) => (
                  <div className="settings-parity-list-item" key={server.id}>
                    <div>
                      <strong>{server.name}</strong>
                      <span>
                        {server.transport} · {server.enabled ? 'enabled' : 'disabled'} ·{' '}
                        {server.tool_filter_mode}
                      </span>
                      <span>{server.endpoint}</span>
                      <span>
                        runtime:{' '}
                        {mcpRuntimeById.get(server.id)?.runtime_status ?? 'unknown'}
                        {' · '}
                        mode {mcpRuntimeById.get(server.id)?.management_mode ?? '-'}
                        {' · '}
                        pid {mcpRuntimeById.get(server.id)?.pid ?? '-'}
                      </span>
                      {mcpProbeById[server.id] ? (
                        <>
                          <span>
                            probe: {mcpProbeById[server.id].status} · {mcpProbeById[server.id].message}
                          </span>
                          <span>
                            handshake: {mcpProbeById[server.id].handshake_status} ·{' '}
                            {mcpProbeById[server.id].handshake_reason}
                          </span>
                          <span>
                            inventory: allow {mcpProbeById[server.id].allowed_tool_count} · block{' '}
                            {mcpProbeById[server.id].blocked_tool_count} · resources{' '}
                            {mcpProbeById[server.id].resources_enabled ? 'on' : 'off'} · prompts{' '}
                            {mcpProbeById[server.id].prompts_enabled ? 'on' : 'off'}
                          </span>
                          {mcpProbeById[server.id].parsed_command ? (
                            <span>
                              command: {mcpProbeById[server.id].parsed_command}
                              {mcpProbeById[server.id].parsed_args?.length
                                ? ` ${mcpProbeById[server.id].parsed_args?.join(' ')}`
                                : ''}
                            </span>
                          ) : null}
                          {mcpProbeById[server.id].endpoint_detail ? (
                            <span>detail: {mcpProbeById[server.id].endpoint_detail}</span>
                          ) : null}
                        </>
                      ) : null}
                    </div>
                    <div className="settings-parity-actions">
                      <button
                        className="btn"
                        type="button"
                        onClick={() =>
                          setMcpForm({
                            id: server.id,
                            name: server.name,
                            transport: server.transport,
                            endpoint: server.endpoint,
                            enabled: server.enabled,
                            tool_filter_mode: server.tool_filter_mode,
                            allowed_tools_text: server.allowed_tools.join('\n'),
                            blocked_tools_text: server.blocked_tools.join('\n'),
                            resources_enabled: server.resources_enabled,
                            prompts_enabled: server.prompts_enabled,
                          })
                        }
                      >
                        Edit
                      </button>
                      <button
                        className="btn"
                        type="button"
                        onClick={() => void handleProbeMcpServer(server.id)}
                        disabled={parityAction === `mcp-probe:${server.id}`}
                      >
                        Probe
                      </button>
                      <button
                        className="btn"
                        type="button"
                        onClick={() => void handleMcpRuntimeAction(server.id, 'start')}
                        disabled={parityAction === `mcp-start:${server.id}`}
                      >
                        Start
                      </button>
                      <button
                        className="btn"
                        type="button"
                        onClick={() => void handleMcpRuntimeAction(server.id, 'reload')}
                        disabled={parityAction === `mcp-reload:${server.id}`}
                      >
                        Reload
                      </button>
                      <button
                        className="btn"
                        type="button"
                        onClick={() => void handleMcpRuntimeAction(server.id, 'stop')}
                        disabled={parityAction === `mcp-stop:${server.id}`}
                      >
                        Stop
                      </button>
                    </div>
                  </div>
                ))}
                {mcpServers.length === 0 ? (
                  <div className="settings-parity-empty">还没有 MCP server metadata。</div>
                ) : null}
              </div>
            </article>

            <article className="settings-parity-card">
              <div className="settings-parity-card-header">
                <div>
                  <span className="settings-overview-label">Quick Commands</span>
                  <strong>{quickCommands.length} commands</strong>
                  <p>保存自定义 slash/quick command 的名称、描述和启用状态。</p>
                </div>
              </div>
              <form className="settings-parity-form" onSubmit={handleSaveQuickCommand}>
                <input
                  type="hidden"
                  value={quickCommandForm.id}
                  onChange={(event) =>
                    setQuickCommandForm({ ...quickCommandForm, id: event.target.value })
                  }
                />
                <div className="form-group">
                  <label>Name</label>
                  <input
                    type="text"
                    value={quickCommandForm.name}
                    onChange={(event) =>
                      setQuickCommandForm({ ...quickCommandForm, name: event.target.value })
                    }
                    placeholder="/brief"
                    required
                  />
                </div>
                <div className="form-group">
                  <label>Command text</label>
                  <textarea
                    value={quickCommandForm.command}
                    onChange={(event) =>
                      setQuickCommandForm({ ...quickCommandForm, command: event.target.value })
                    }
                    placeholder="Summarize active missions and next actions."
                    required
                  />
                </div>
                <div className="form-group">
                  <label>Description</label>
                  <input
                    type="text"
                    value={quickCommandForm.description}
                    onChange={(event) =>
                      setQuickCommandForm({
                        ...quickCommandForm,
                        description: event.target.value,
                      })
                    }
                    placeholder="Daily mission brief"
                  />
                </div>
                <div className="form-group checkbox">
                  <label>
                    <input
                      type="checkbox"
                      checked={quickCommandForm.enabled}
                      onChange={(event) =>
                        setQuickCommandForm({
                          ...quickCommandForm,
                          enabled: event.target.checked,
                        })
                      }
                    />
                    enabled
                  </label>
                </div>
                <button
                  className="btn primary"
                  type="submit"
                  disabled={parityAction === 'quick-command-save'}
                >
                  {parityAction === 'quick-command-save' ? '保存中...' : '保存 quick command'}
                </button>
              </form>
              <div className="settings-parity-list">
                {quickCommands.map((command) => (
                  <div className="settings-parity-list-item" key={command.id}>
                    <div>
                      <strong>{command.name}</strong>
                      <span>{command.description ?? 'No description'}</span>
                      <span>{command.enabled ? 'enabled' : 'disabled'}</span>
                    </div>
                    <button
                      className="btn"
                      type="button"
                      onClick={() =>
                        setQuickCommandForm({
                          id: command.id,
                          name: command.name,
                          command: command.command,
                          description: command.description ?? '',
                          enabled: command.enabled,
                        })
                      }
                    >
                      Edit
                    </button>
                  </div>
                ))}
                {quickCommands.length === 0 ? (
                  <div className="settings-parity-empty">还没有 quick command metadata。</div>
                ) : null}
              </div>
            </article>
          </div>
        </section>

        <section className="settings-section settings-parity-section">
          <div className="settings-section-heading-row">
            <div>
              <h3>Terminal Backends</h3>
              <p>
                管理 local、docker、ssh、modal、daytona、singularity backend profiles，并提供本地 availability / config test。
              </p>
            </div>
            <button
              className="btn"
              type="button"
              onClick={() => void loadTerminalBackends()}
              disabled={terminalLoading || terminalAction !== null}
            >
              {terminalLoading ? '刷新中...' : '刷新 backends'}
            </button>
          </div>

          {terminalError ? <div className="error-message">{terminalError}</div> : null}
          {terminalNotice ? <div className="success-message">{terminalNotice}</div> : null}

          <div className="settings-parity-grid">
            <article className="settings-parity-card">
              <div className="settings-parity-card-header">
                <div>
                  <span className="settings-overview-label">Profiles</span>
                  <strong>{terminalProfiles.length} backend profiles</strong>
                  <p>默认 profile 会自动种子化；云 backend 当前只做 configured/unavailable 运行面。</p>
                </div>
              </div>
              <form className="settings-parity-form" onSubmit={handleSaveTerminalBackend}>
                <div className="settings-form-row">
                  <div className="form-group">
                    <label>Profile ID</label>
                    <input
                      type="text"
                      value={terminalBackendForm.id}
                      onChange={(event) =>
                        setTerminalBackendForm({ ...terminalBackendForm, id: event.target.value })
                      }
                      placeholder="ssh-staging"
                    />
                  </div>
                  <div className="form-group">
                    <label>Kind</label>
                    <select
                      value={terminalBackendForm.kind}
                      onChange={(event) =>
                        setTerminalBackendForm({ ...terminalBackendForm, kind: event.target.value })
                      }
                    >
                      {['local', 'docker', 'ssh', 'modal', 'daytona', 'singularity'].map((kind) => (
                        <option key={kind} value={kind}>
                          {kind}
                        </option>
                      ))}
                    </select>
                  </div>
                </div>
                <div className="form-group">
                  <label>Display name</label>
                  <input
                    type="text"
                    value={terminalBackendForm.display_name}
                    onChange={(event) =>
                      setTerminalBackendForm({
                        ...terminalBackendForm,
                        display_name: event.target.value,
                      })
                    }
                    placeholder="SSH Staging"
                    required
                  />
                </div>
                <div className="form-group checkbox">
                  <label>
                    <input
                      type="checkbox"
                      checked={terminalBackendForm.enabled}
                      onChange={(event) =>
                        setTerminalBackendForm({
                          ...terminalBackendForm,
                          enabled: event.target.checked,
                        })
                      }
                    />
                    enabled
                  </label>
                </div>
                <div className="form-group">
                  <label>Config JSON</label>
                  <textarea
                    className="settings-terminal-config"
                    value={terminalBackendForm.config_text}
                    onChange={(event) =>
                      setTerminalBackendForm({
                        ...terminalBackendForm,
                        config_text: event.target.value,
                      })
                    }
                    placeholder={'{\n  "host": "staging.example.test",\n  "user": "agent"\n}'}
                  />
                </div>
                <button className="btn primary" type="submit" disabled={terminalAction === 'save'}>
                  {terminalAction === 'save' ? '保存中...' : '保存 backend profile'}
                </button>
              </form>
            </article>

            <article className="settings-parity-card settings-parity-card-wide">
              <div className="settings-parity-card-header">
                <div>
                  <span className="settings-overview-label">Backend Status</span>
                  <strong>{terminalStatuses.length} statuses</strong>
                  <p>Local/docker/ssh 会根据命令可用性或配置状态返回真实 availability。</p>
                </div>
              </div>
              <div className="settings-parity-list">
                {terminalProfiles.map((profile) => {
                  const status = terminalStatuses.find((item) => item.id === profile.id);
                  return (
                    <div className="settings-parity-list-item" key={profile.id}>
                      <div>
                        <strong>{profile.display_name}</strong>
                        <span>
                          {profile.kind} · {status?.availability ?? 'unknown'} ·{' '}
                          {status?.configured ? 'configured' : 'not configured'}
                        </span>
                        <span>{status?.message ?? 'No runtime status yet.'}</span>
                      </div>
                      <div className="settings-parity-actions">
                        <button
                          className="btn"
                          type="button"
                          onClick={() => handleEditTerminalBackend(profile)}
                        >
                          Edit
                        </button>
                        <button
                          className="btn"
                          type="button"
                          onClick={() => void handleTestTerminalBackend(profile.id)}
                          disabled={terminalAction === `test:${profile.id}`}
                        >
                          {terminalAction === `test:${profile.id}` ? '测试中...' : 'Test'}
                        </button>
                      </div>
                    </div>
                  );
                })}
                {terminalProfiles.length === 0 ? (
                  <div className="settings-parity-empty">还没有 terminal backend profiles。</div>
                ) : null}
              </div>
            </article>
          </div>
        </section>

        <section className="settings-section settings-placeholder-section">
          <h3>Workspace / Diagnostics</h3>
          <div className="settings-placeholder-grid">
            <article className="settings-placeholder-card">
              <strong>Workspace</strong>
              {workspaceDiagnostics ? (
                <>
                  <p>
                    workspace={workspaceDiagnostics.paths.default_workspace_path ?? '未配置'} ·
                    control API={workspaceDiagnostics.paths.control_api_url}
                  </p>
                  <p>
                    config={workspaceDiagnostics.paths.config_dir}
                    <br />
                    data={workspaceDiagnostics.paths.data_dir}
                    <br />
                    db={workspaceDiagnostics.paths.db_path}
                  </p>
                  <p>
                    engine heartbeat {formatDateTime(workspaceDiagnostics.status.engine_last_heartbeat_at)}
                    <br />
                    foreground {formatDateTime(workspaceDiagnostics.status.foreground_updated_at)}
                  </p>
                </>
              ) : (
                <p>正在读取 workspace 状态...</p>
              )}
            </article>
            <article className="settings-placeholder-card">
              <strong>Diagnostics</strong>
              {workspaceDiagnostics ? (
                <>
                  <p>
                    missions={workspaceDiagnostics.counts.missions} · sessions={workspaceDiagnostics.counts.sessions} ·
                    knowledge={workspaceDiagnostics.counts.knowledge_sources}
                  </p>
                  <p>
                    memory={workspaceDiagnostics.counts.memory_records} · run events={workspaceDiagnostics.counts.run_events} ·
                    cron jobs={workspaceDiagnostics.counts.cron_jobs}
                  </p>
                  <p>
                    queued background={workspaceDiagnostics.status.engine_queued_background_runs} ·
                    awaiting approval={workspaceDiagnostics.status.engine_awaiting_approval_steps}
                  </p>
                  <p>
                    logs:
                    {workspaceDiagnostics.recent_logs.length > 0
                      ? ` ${workspaceDiagnostics.recent_logs
                          .map((item) => `${item.name} (${item.size_bytes}B)`)
                          .join(', ')}`
                      : ' 暂无日志文件'}
                  </p>
                </>
              ) : (
                <p>正在收集诊断信息...</p>
              )}
            </article>
          </div>
        </section>
      </div>
    </div>
  );
}
