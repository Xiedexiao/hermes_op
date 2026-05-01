import { type FormEvent, useEffect, useMemo, useState } from 'react';
import {
  agentExchangeListRemoteUsers,
  parityToolsetList,
  skillEvolutionCandidateCreate,
  skillEvolutionCandidateGenerate,
  skillEvolutionCandidateList,
  sessionListRecent,
  skillsExecuteRuntime,
  skillEvolutionCandidateSetStatus,
  skillsInvoke,
  skillsInvokeIntoSession,
  skillsList,
  skillsListSessionInvocations,
  skillsMarketplaceInstall,
  skillsMarketplaceListInstallHistory,
  skillsMarketplaceList,
  skillsSetEnabled,
  type AgentExchangeRemoteUser,
  type ParityToolset,
  type SkillEvolutionAction,
  type SkillEvolutionCandidate,
  type SkillEvolutionConfidence,
  type SkillEvolutionSourceRef,
  type SkillEvolutionStatus,
  type Session,
  type SessionMessage,
  type SkillInvocationPayload,
  type SkillListItem,
  type SkillMarketplaceCatalog,
  type SkillMarketplaceInstallHistoryItem,
  type SkillRuntimeExecutionResult,
} from '../lib/tauri';
import './SkillsPage.css';

interface EvolutionCandidateForm {
  action: SkillEvolutionAction;
  targetSkillName: string;
  confidence: SkillEvolutionConfidence;
  evidenceSummary: string;
  recommendedChange: string;
  sourceRefsText: string;
  validationNotes: string;
}

interface MarketplaceHistoryFilters {
  marketplaceId: string;
  skillName: string;
  limit: number;
}

const DEFAULT_MARKETPLACE_HISTORY_LIMIT = 6;
const MIN_MARKETPLACE_HISTORY_LIMIT = 1;
const MAX_MARKETPLACE_HISTORY_LIMIT = 50;
const AGENT_EXCHANGE_REMOTE_USER_LIMIT = 50;

const initialEvolutionForm: EvolutionCandidateForm = {
  action: 'refine',
  targetSkillName: '',
  confidence: 'medium',
  evidenceSummary: '',
  recommendedChange: '',
  sourceRefsText: '',
  validationNotes: '',
};

function getSlashHint(skill: SkillListItem) {
  if (skill.enabled) {
    return '已启用。可以渲染 SKILL.md payload、保存为 session context，并通过 Runtime Adapter 的 printf/echo allowlist 做本地验证。';
  }

  return '已禁用。页面会保留发现结果，但不会渲染 payload 或触发 runtime validation。';
}

function matchesQuery(value: string | null | undefined, query: string) {
  return value?.toLowerCase().includes(query) ?? false;
}

function toolsetMatchesQuery(toolset: ParityToolset, query: string) {
  if (!query) {
    return true;
  }


  return (
    matchesQuery(toolset.name, query) ||
    matchesQuery(toolset.description, query) ||
    matchesQuery(toolset.source, query) ||
    toolset.tools.some(
      (tool) =>
        matchesQuery(tool.name, query) ||
        matchesQuery(tool.description, query) ||
        matchesQuery(tool.availability, query),
    )
  );
}

function candidateMatchesQuery(candidate: SkillEvolutionCandidate, query: string) {
  if (!query) {
    return true;
  }

  return (
    matchesQuery(candidate.target_skill_name, query) ||
    matchesQuery(candidate.action, query) ||
    matchesQuery(candidate.status, query) ||
    matchesQuery(candidate.evidence_summary, query) ||
    matchesQuery(candidate.recommended_change, query) ||
    matchesQuery(candidate.confidence, query) ||
    candidate.source_refs.some(
      (sourceRef) =>
        matchesQuery(sourceRef.kind, query) ||
        matchesQuery(sourceRef.id, query) ||
        matchesQuery(sourceRef.title, query),
    )
  );
}

function parseSourceRefs(value: string): SkillEvolutionSourceRef[] {
  return value
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const [kind, id, ...titleParts] = line.includes(':')
        ? line.split(':')
        : ['note', line];
      return {
        kind: kind.trim() || 'note',
        id: id?.trim() || line,
        title: titleParts.join(':').trim() || null,
      };
    });
}

function actionLabel(action: SkillEvolutionAction) {
  switch (action) {
    case 'refine':
      return 'Refine skill';
    case 'create':
      return 'Create skill';
    case 'skip':
      return 'Skip / observe';
  }
}

function statusLabel(status: SkillEvolutionStatus) {
  switch (status) {
    case 'pending':
      return 'pending review';
    case 'accepted':
      return 'accepted';
    case 'rejected':
      return 'rejected';
  }
}

function formatCandidateTimestamp(value: string) {
  return new Intl.DateTimeFormat('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(value));
}

function formatMarketplaceInstallTimestamp(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(date);
}

function formatRemoteUserOptionLabel(remoteUser: AgentExchangeRemoteUser) {
  return `${remoteUser.display_name} (${remoteUser.user_id})`;
}

function isRuntimeSourceRefKind(kind: string) {
  return ['run', 'execution_step', 'run_event'].includes(kind);
}

function getSourceRefKindMeta(kind: string) {
  switch (kind) {
    case 'session':
      return { label: 'Session', tone: 'manual' as const };
    case 'run':
      return { label: 'Run', tone: 'runtime' as const };
    case 'execution_step':
      return { label: 'Step', tone: 'runtime' as const };
    case 'run_event':
      return { label: 'Event', tone: 'runtime' as const };
    case 'note':
      return { label: 'Note', tone: 'manual' as const };
    default:
      return { label: kind.split('_').join(' '), tone: 'neutral' as const };
  }
}

function getCandidateOrigin(candidate: SkillEvolutionCandidate) {
  const autoGenerated =
    candidate.validation_notes?.toLowerCase().includes('auto-generated') ||
    candidate.source_refs.some((sourceRef) => isRuntimeSourceRefKind(sourceRef.kind));

  if (autoGenerated && candidate.action === 'refine' && candidate.target_skill_name) {
    return {
      key: 'auto-attributed' as const,
      label: 'Auto-attributed',
      description: 'Runtime failure signals were mapped onto an existing skill refinement.',
    };
  }

  if (autoGenerated) {
    return {
      key: 'auto-generated' as const,
      label: 'Auto-generated',
      description: 'Candidate came from runtime failures, risky steps, or failure events.',
    };
  }

  return {
    key: 'manual' as const,
    label: 'Manual',
    description: 'Candidate was recorded directly by a reviewer with explicit evidence.',
  };
}

function actionDescription(action: SkillEvolutionAction) {
  switch (action) {
    case 'refine':
      return 'Tighten guidance for an existing skill.';
    case 'create':
      return 'Propose a new skill from repeated gaps or failures.';
    case 'skip':
      return 'Record evidence without changing the skill library yet.';
  }
}

function formatInvocationPayloadJson(payload: SkillInvocationPayload) {
  return JSON.stringify(payload, null, 2);
}

function formatInvocationPayloadMarkdown(payload: SkillInvocationPayload) {
  return [
    '# Skill Invocation Payload',
    '',
    `- Name: ${payload.display_name || payload.name}`,
    `- Command: ${payload.command}`,
    `- Source: ${payload.source}`,
    `- Path: ${payload.path}`,
    payload.instruction ? `- Instruction: ${payload.instruction}` : null,
    '',
    '## Rendered Prompt',
    '```text',
    payload.rendered_prompt,
    '```',
  ]
    .filter(Boolean)
    .join('\n');
}

function hasReplayContent(message: SessionMessage | null | undefined) {
  return Boolean(message?.content.trim());
}

function formatRuntimeExecutionResult(result: SkillRuntimeExecutionResult) {
  return JSON.stringify(result, null, 2);
}

function normalizeMarketplaceHistoryLimit(value: string) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) {
    return DEFAULT_MARKETPLACE_HISTORY_LIMIT;
  }

  return Math.min(
    MAX_MARKETPLACE_HISTORY_LIMIT,
    Math.max(MIN_MARKETPLACE_HISTORY_LIMIT, parsed),
  );
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

function buildMarketplaceHistoryAuditJson(
  records: SkillMarketplaceInstallHistoryItem[],
  filters: MarketplaceHistoryFilters,
  targetRemoteUserId: string,
  targetRemoteUserProfile: AgentExchangeRemoteUser | null,
) {
  const normalizedTargetRemoteUserId = targetRemoteUserId.trim() || null;
  return JSON.stringify(
    {
      schema_version: 1,
      exported_at: new Date().toISOString(),
      target_remote_user_id: normalizedTargetRemoteUserId,
      target_remote_user_profile: buildTargetRemoteUserProfileSnapshot(
        targetRemoteUserId,
        targetRemoteUserProfile,
      ),
      filters: {
        limit: filters.limit,
        marketplace_id: filters.marketplaceId || null,
        skill_name: filters.skillName || null,
      },
      boundary_note:
        'This audit records local Hermes skills-directory writes only. target_remote_user_id is future remote user routing metadata only and does not imply remote marketplace account activity, remote marketplace state changes, or remote delivery.',
      record_count: records.length,
      records,
    },
    null,
    2,
  );
}

export function SkillsPage() {
  const [skills, setSkills] = useState<SkillListItem[]>([]);
  const [toolsets, setToolsets] = useState<ParityToolset[]>([]);
  const [evolutionCandidates, setEvolutionCandidates] = useState<SkillEvolutionCandidate[]>([]);
  const [query, setQuery] = useState('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [savingName, setSavingName] = useState<string | null>(null);
  const [invokingName, setInvokingName] = useState<string | null>(null);
  const [executingRuntimeName, setExecutingRuntimeName] = useState<string | null>(null);
  const [skillInstructionByName, setSkillInstructionByName] = useState<Record<string, string>>({});
  const [invocationResult, setInvocationResult] = useState<SkillInvocationPayload | null>(null);
  const [runtimeExecutionResult, setRuntimeExecutionResult] =
    useState<SkillRuntimeExecutionResult | null>(null);
  const [invocationError, setInvocationError] = useState<string | null>(null);
  const [recentSessions, setRecentSessions] = useState<Session[]>([]);
  const [selectedInvocationSessionId, setSelectedInvocationSessionId] = useState('');
  const [sessionInvocationMessages, setSessionInvocationMessages] = useState<SessionMessage[]>([]);
  const [sessionInvocationLoading, setSessionInvocationLoading] = useState(false);
  const [reviewingCandidateId, setReviewingCandidateId] = useState<string | null>(null);
  const [creatingCandidate, setCreatingCandidate] = useState(false);
  const [generatingCandidates, setGeneratingCandidates] = useState(false);
  const [evolutionForm, setEvolutionForm] = useState<EvolutionCandidateForm>(
    initialEvolutionForm,
  );
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [marketplaceManifestUrl, setMarketplaceManifestUrl] = useState('');
  const [marketplaceCatalog, setMarketplaceCatalog] = useState<SkillMarketplaceCatalog | null>(null);
  const [marketplaceLoading, setMarketplaceLoading] = useState(false);
  const [marketplaceInstallingName, setMarketplaceInstallingName] = useState<string | null>(null);
  const [marketplaceError, setMarketplaceError] = useState<string | null>(null);
  const [marketplaceInstallHistory, setMarketplaceInstallHistory] = useState<
    SkillMarketplaceInstallHistoryItem[]
  >([]);
  const [marketplaceHistoryLoading, setMarketplaceHistoryLoading] = useState(false);
  const [marketplaceHistoryError, setMarketplaceHistoryError] = useState<string | null>(null);
  const [marketplaceHistoryMarketplaceId, setMarketplaceHistoryMarketplaceId] = useState('');
  const [marketplaceHistorySkillName, setMarketplaceHistorySkillName] = useState('');
  const [marketplaceHistoryTargetRemoteUserId, setMarketplaceHistoryTargetRemoteUserId] =
    useState('');
  const [marketplaceRemoteUsers, setMarketplaceRemoteUsers] = useState<AgentExchangeRemoteUser[]>(
    [],
  );
  const [marketplaceRemoteUsersLoading, setMarketplaceRemoteUsersLoading] = useState(false);
  const [marketplaceRemoteUsersError, setMarketplaceRemoteUsersError] = useState<string | null>(
    null,
  );
  const [marketplaceHistoryLimit, setMarketplaceHistoryLimit] = useState(
    String(DEFAULT_MARKETPLACE_HISTORY_LIMIT),
  );
  const [marketplaceHistoryAppliedFilters, setMarketplaceHistoryAppliedFilters] =
    useState<MarketplaceHistoryFilters>({
      marketplaceId: '',
      skillName: '',
      limit: DEFAULT_MARKETPLACE_HISTORY_LIMIT,
    });

  useEffect(() => {
    void loadSkills();
  }, []);

  useEffect(() => {
    void loadMarketplaceRemoteUsers();
  }, []);

  useEffect(() => {
    void loadMarketplaceInstallHistory();
  }, []);

  useEffect(() => {
    if (!selectedInvocationSessionId) {
      setSessionInvocationMessages([]);
      return;
    }

    void loadSessionSkillInvocations(selectedInvocationSessionId);
  }, [selectedInvocationSessionId]);

  const normalizedQuery = query.trim().toLowerCase();
  const filteredSkills = useMemo(
    () =>
      skills.filter((skill) => {
        if (!normalizedQuery) {
          return true;
        }

        return (
          matchesQuery(skill.name, normalizedQuery) ||
          matchesQuery(skill.display_name, normalizedQuery) ||
          matchesQuery(skill.source, normalizedQuery) ||
          matchesQuery(skill.path, normalizedQuery)
        );
      }),
    [normalizedQuery, skills],
  );
  const filteredToolsets = useMemo(
    () => toolsets.filter((toolset) => toolsetMatchesQuery(toolset, normalizedQuery)),
    [normalizedQuery, toolsets],
  );
  const filteredEvolutionCandidates = useMemo(
    () =>
      evolutionCandidates.filter((candidate) =>
        candidateMatchesQuery(candidate, normalizedQuery),
      ),
    [evolutionCandidates, normalizedQuery],
  );
  const enabledCount = skills.filter((skill) => skill.enabled).length;
  const disabledCount = skills.length - enabledCount;
  const pendingEvolutionCount = evolutionCandidates.filter(
    (candidate) => candidate.status === 'pending',
  ).length;
  const acceptedEvolutionCount = evolutionCandidates.filter(
    (candidate) => candidate.status === 'accepted',
  ).length;
  const enabledToolsetCount = toolsets.filter((toolset) => toolset.enabled).length;
  const visibleToolCount = toolsets.reduce(
    (total, toolset) => total + toolset.tools.filter((tool) => tool.visible).length,
    0,
  );
  const enabledToolCount = toolsets.reduce(
    (total, toolset) => total + toolset.tools.filter((tool) => tool.enabled).length,
    0,
  );
  const selectedMarketplaceRemoteUser = useMemo(
    () =>
      marketplaceRemoteUsers.find(
        (remoteUser) => remoteUser.user_id === marketplaceHistoryTargetRemoteUserId.trim(),
      ) ?? null,
    [marketplaceHistoryTargetRemoteUserId, marketplaceRemoteUsers],
  );

  async function loadSkills() {
    setLoading(true);
    setError(null);
    try {
      const [skillData, toolsetData, candidateData, sessionData] = await Promise.all([
        skillsList(),
        parityToolsetList(),
        skillEvolutionCandidateList({ limit: 50 }),
        sessionListRecent(20),
      ]);
      setSkills(skillData);
      setToolsets(toolsetData);
      setEvolutionCandidates(candidateData);
      setRecentSessions(sessionData);
      setSelectedInvocationSessionId((current) =>
        current && sessionData.some((session) => session.id === current)
          ? current
          : sessionData[0]?.id ?? '',
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }

  async function loadSessionSkillInvocations(sessionId = selectedInvocationSessionId) {
    const trimmedSessionId = sessionId.trim();
    if (!trimmedSessionId) {
      setSessionInvocationMessages([]);
      return;
    }

    setSessionInvocationLoading(true);
    try {
      const messages = await skillsListSessionInvocations({
        session_id: trimmedSessionId,
        limit: 8,
      });
      setSessionInvocationMessages(messages);
    } catch (err) {
      setInvocationError(err instanceof Error ? err.message : String(err));
    } finally {
      setSessionInvocationLoading(false);
    }
  }

  async function handleToggleSkill(skill: SkillListItem) {
    setSavingName(skill.name);
    setStatusMessage(null);
    setInvocationError(null);
    setError(null);
    try {
      await skillsSetEnabled({ name: skill.name, enabled: !skill.enabled });
      setSkills((current) =>
        current.map((item) =>
          item.name === skill.name ? { ...item, enabled: !skill.enabled } : item,
        ),
      );
      setStatusMessage(`${skill.display_name || skill.name} 已${skill.enabled ? '禁用' : '启用'}。`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSavingName(null);
    }
  }

  async function handleInvokeSkill(skill: SkillListItem) {
    setInvokingName(skill.name);
    setStatusMessage(null);
    setInvocationError(null);
    setError(null);
    try {
      const instruction = skillInstructionByName[skill.name]?.trim() || null;
      const payload = await skillsInvoke({ name: skill.name, instruction });
      setInvocationResult(payload);
      setRuntimeExecutionResult(null);
      setStatusMessage(
        `${skill.display_name || skill.name} 已渲染为本地 invocation payload，可用于复核或移交；这不代表内容已送入真实 model/tool runtime。`,
      );
    } catch (err) {
      setInvocationError(err instanceof Error ? err.message : String(err));
    } finally {
      setInvokingName(null);
    }
  }

  async function handleInvokeSkillIntoSession(skill: SkillListItem) {
    if (!selectedInvocationSessionId) {
      setInvocationError('Select a recent session before saving the skill payload.');
      return;
    }
    setInvokingName(`session:${skill.name}`);
    setStatusMessage(null);
    setInvocationError(null);
    setError(null);
    try {
      const instruction = skillInstructionByName[skill.name]?.trim() || null;
      const saved = await skillsInvokeIntoSession({
        name: skill.name,
        instruction,
        session_id: selectedInvocationSessionId,
      });
      setInvocationResult(saved.invocation);
      setRuntimeExecutionResult(null);
      await loadSessionSkillInvocations(saved.session_id);
      setStatusMessage(
        `${skill.display_name || skill.name} 已把本地 invocation payload 的 prompt 内容保存到 session ${saved.session_id}，用于复核/移交，不代表已送入真实 model/tool runtime。`,
      );
    } catch (err) {
      setInvocationError(err instanceof Error ? err.message : String(err));
    } finally {
      setInvokingName(null);
    }
  }

  async function handleExecuteSkillRuntime(skill: SkillListItem, dryRun: boolean) {
    setExecutingRuntimeName(`${dryRun ? 'dry' : 'run'}:${skill.name}`);
    setStatusMessage(null);
    setInvocationError(null);
    setError(null);
    try {
      const instruction = skillInstructionByName[skill.name]?.trim() || null;
      const result = await skillsExecuteRuntime({
        name: skill.name,
        instruction,
        session_id: selectedInvocationSessionId || null,
        save_to_session: Boolean(selectedInvocationSessionId),
        dry_run: dryRun,
        tool_command: 'printf',
        timeout_ms: 1000,
      });
      setInvocationResult(result.invocation);
      setRuntimeExecutionResult(result);
      if (result.session_message) {
        await loadSessionSkillInvocations(result.session_message.session_id);
      }
      setStatusMessage(
        dryRun
          ? `${skill.display_name || skill.name} 已生成安全 runtime execution package；未执行本地工具。`
          : `${skill.display_name || skill.name} 已通过 Runtime Adapter 执行 allowlisted printf 验证。`,
      );
    } catch (err) {
      setInvocationError(err instanceof Error ? err.message : String(err));
    } finally {
      setExecutingRuntimeName(null);
    }
  }

  async function handleCreateEvolutionCandidate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setCreatingCandidate(true);
    setStatusMessage(null);
    setError(null);
    try {
      const created = await skillEvolutionCandidateCreate({
        action: evolutionForm.action,
        target_skill_name:
          evolutionForm.action === 'refine'
            ? evolutionForm.targetSkillName
            : evolutionForm.targetSkillName || null,
        confidence: evolutionForm.confidence,
        evidence_summary: evolutionForm.evidenceSummary,
        recommended_change: evolutionForm.recommendedChange,
        source_refs: parseSourceRefs(evolutionForm.sourceRefsText),
        validation_notes: evolutionForm.validationNotes || null,
      });
      setEvolutionCandidates((current) => [created, ...current]);
      setEvolutionForm(initialEvolutionForm);
      setStatusMessage('已记录新的 skill evolution candidate，等待验证后再进入技能库。');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setCreatingCandidate(false);
    }
  }

  async function handleSetCandidateStatus(
    candidate: SkillEvolutionCandidate,
    status: Extract<SkillEvolutionStatus, 'accepted' | 'rejected'>,
  ) {
    setReviewingCandidateId(candidate.id);
    setStatusMessage(null);
    setError(null);
    try {
      const updated = await skillEvolutionCandidateSetStatus({
        id: candidate.id,
        status,
        validation_notes:
          status === 'accepted'
            ? 'Reviewer accepted this candidate for subsequent skill/history work.'
            : 'Reviewer rejected this candidate; keep as evidence but do not apply.',
      });
      setEvolutionCandidates((current) =>
        current.map((item) => (item.id === updated.id ? updated : item)),
      );
      setStatusMessage(
        `${candidate.target_skill_name || actionLabel(candidate.action)} 已标记为 ${
          status === 'accepted' ? 'accepted' : 'rejected'
        }。`,
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setReviewingCandidateId(null);
    }
  }

  async function handleGenerateEvolutionCandidates() {
    setGeneratingCandidates(true);
    setStatusMessage(null);
    setError(null);
    try {
      const generated = await skillEvolutionCandidateGenerate({ limit: 12 });
      if (generated.length === 0) {
        setStatusMessage('未发现新的失败/高风险信号，当前没有新增候选。');
      } else {
        setEvolutionCandidates((current) => {
          const seen = new Set(current.map((candidate) => candidate.id));
          return [...generated.filter((candidate) => !seen.has(candidate.id)), ...current];
        });
        setStatusMessage(`已从运行轨迹自动生成 ${generated.length} 个候选改进。`);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setGeneratingCandidates(false);
    }
  }

  const hasQuery = query.trim().length > 0;
  const latestSessionInvocationMessage = sessionInvocationMessages[0] ?? null;

  async function copyTextToClipboard(
    text: string,
    successMessage: string,
    manualLabel: string,
    fallbackMessage?: string,
  ) {
    setStatusMessage(null);
    setInvocationError(null);
    setError(null);

    if (!text.trim()) {
      setInvocationError(`${manualLabel} 当前没有可复制内容。`);
      return;
    }

    if (!navigator.clipboard?.writeText) {
      setInvocationError(
        fallbackMessage ??
          `${manualLabel} 无法写入剪贴板，请从页面手动复制。这些内容仅用于本地 invocation payload 复核/移交，尚未送入真实 model/tool runtime。`,
      );
      return;
    }

    try {
      await navigator.clipboard.writeText(text);
      setStatusMessage(successMessage);
    } catch {
      setInvocationError(
        fallbackMessage ??
          `${manualLabel} 无法写入剪贴板，请从页面手动复制。这些内容仅用于本地 invocation payload 复核/移交，尚未送入真实 model/tool runtime。`,
      );
    }
  }

  function downloadTextFile(
    filename: string,
    content: string,
    mimeType: string,
    successMessage?: string,
  ) {
    setStatusMessage(null);
    setInvocationError(null);
    setError(null);

    if (!content.trim()) {
      setInvocationError(`${filename} 当前没有可导出内容。`);
      return;
    }

    const blob = new Blob([content], { type: mimeType });
    const objectUrl = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = objectUrl;
    link.download = filename;
    document.body.append(link);
    link.click();
    link.remove();
    URL.revokeObjectURL(objectUrl);
    setStatusMessage(
      successMessage ??
        `${filename} 已导出，用于本地 invocation payload 复核/移交；这不代表内容已送入真实 model/tool runtime。`,
    );
  }

  async function handleCopyInvocationPayload(
    payload: SkillInvocationPayload,
    format: 'json' | 'markdown',
  ) {
    const content =
      format === 'json'
        ? formatInvocationPayloadJson(payload)
        : formatInvocationPayloadMarkdown(payload);
    const label = format === 'json' ? 'payload JSON' : 'payload Markdown';
    await copyTextToClipboard(
      content,
      `${payload.display_name || payload.name} 的 ${label} 已复制。这些内容仅用于本地 invocation payload 复核/移交，尚未送入真实 model/tool runtime。`,
      label,
    );
  }

  function handleDownloadInvocationPayload(
    payload: SkillInvocationPayload,
    format: 'json' | 'markdown',
  ) {
    const baseName = `${payload.name}-invocation-payload`;
    if (format === 'json') {
      downloadTextFile(
        `${baseName}.json`,
        formatInvocationPayloadJson(payload),
        'application/json;charset=utf-8',
      );
      return;
    }

    downloadTextFile(
      `${baseName}.md`,
      formatInvocationPayloadMarkdown(payload),
      'text/markdown;charset=utf-8',
    );
  }

  async function handleCopySessionInvocationContent(
    message: SessionMessage,
    label: 'latest' | 'selected',
  ) {
    const sourceLabel = label === 'latest' ? '最新 session skill payload' : 'session skill payload';
    await copyTextToClipboard(
      message.content,
      `${sourceLabel} 已复制。当前 session replay 只保存本地 payload 的 prompt 内容，用于复核/移交，不代表已送入真实 model/tool runtime。`,
      sourceLabel,
    );
  }


  async function handleLoadMarketplace() {
    const manifestUrl = marketplaceManifestUrl.trim();
    if (!manifestUrl) {
      setMarketplaceError('Manifest URL or local path is required.');
      return;
    }

    setMarketplaceLoading(true);
    setMarketplaceError(null);
    try {
      const catalog = await skillsMarketplaceList({ manifest_url: manifestUrl, limit: 50 });
      setMarketplaceCatalog(catalog);
      setStatusMessage(`Loaded ${catalog.skills.length} marketplace skill(s) from ${catalog.marketplace_id}.`);
    } catch (err) {
      setMarketplaceCatalog(null);
      setMarketplaceError(err instanceof Error ? err.message : String(err));
    } finally {
      setMarketplaceLoading(false);
    }
  }

  async function loadMarketplaceRemoteUsers() {
    setMarketplaceRemoteUsersLoading(true);
    setMarketplaceRemoteUsersError(null);
    try {
      const remoteUsers = await agentExchangeListRemoteUsers({
        status: 'active',
        limit: AGENT_EXCHANGE_REMOTE_USER_LIMIT,
      });
      setMarketplaceRemoteUsers(remoteUsers);
    } catch (err) {
      setMarketplaceRemoteUsersError(
        `Local Agent Exchange remote user load failed: ${
          err instanceof Error ? err.message : String(err)
        }`,
      );
    } finally {
      setMarketplaceRemoteUsersLoading(false);
    }
  }

  async function loadMarketplaceInstallHistory() {
    setMarketplaceHistoryLoading(true);
    setMarketplaceHistoryError(null);
    try {
      const normalizedFilters: MarketplaceHistoryFilters = {
        marketplaceId: marketplaceHistoryMarketplaceId.trim(),
        skillName: marketplaceHistorySkillName.trim(),
        limit: normalizeMarketplaceHistoryLimit(marketplaceHistoryLimit),
      };
      const history = await skillsMarketplaceListInstallHistory({
        limit: normalizedFilters.limit,
        marketplace_id: normalizedFilters.marketplaceId || undefined,
        skill_name: normalizedFilters.skillName || undefined,
        target_remote_user_id: marketplaceHistoryTargetRemoteUserId.trim() || undefined,
      });
      setMarketplaceHistoryLimit(String(normalizedFilters.limit));
      setMarketplaceHistoryAppliedFilters(normalizedFilters);
      setMarketplaceInstallHistory(history);
    } catch (err) {
      setMarketplaceHistoryError(err instanceof Error ? err.message : String(err));
    } finally {
      setMarketplaceHistoryLoading(false);
    }
  }

  function getMarketplaceHistoryAuditJson() {
    setMarketplaceHistoryError(null);

    if (marketplaceInstallHistory.length === 0) {
      setMarketplaceHistoryError(
        'No local marketplace install history is currently loaded for audit export. Refresh the panel or broaden the filters first.',
      );
      return null;
    }

    return buildMarketplaceHistoryAuditJson(
      marketplaceInstallHistory,
      marketplaceHistoryAppliedFilters,
      marketplaceHistoryTargetRemoteUserId,
      selectedMarketplaceRemoteUser,
    );
  }

  async function handleCopyMarketplaceHistoryAudit() {
    const auditJson = getMarketplaceHistoryAuditJson();
    if (!auditJson) {
      return;
    }

    await copyTextToClipboard(
      auditJson,
      'Local marketplace install history audit JSON copied. It records local Hermes skills-directory writes only; target_remote_user_id is future remote user routing metadata only and does not imply remote marketplace account activity.',
      'marketplace install history audit JSON',
      'marketplace install history audit JSON could not be copied automatically. Copy it manually from the page; target_remote_user_id is future remote user routing metadata only and does not imply remote marketplace account activity.',
    );
  }

  function handleDownloadMarketplaceHistoryAudit() {
    const auditJson = getMarketplaceHistoryAuditJson();
    if (!auditJson) {
      return;
    }

    downloadTextFile(
      'marketplace-install-history-audit.json',
      auditJson,
      'application/json;charset=utf-8',
      'marketplace-install-history-audit.json exported. target_remote_user_id is future remote user routing metadata only; this local audit does not imply remote marketplace account activity.',
    );
  }

  async function handleInstallMarketplaceSkill(skillName: string) {
    const manifestUrl = marketplaceManifestUrl.trim();
    if (!manifestUrl) {
      setMarketplaceError('Manifest URL or local path is required.');
      return;
    }

    setMarketplaceInstallingName(skillName);
    setMarketplaceError(null);
    try {
      const installed = await skillsMarketplaceInstall({
        manifest_url: manifestUrl,
        name: skillName,
        force: true,
        target_remote_user_id: marketplaceHistoryTargetRemoteUserId.trim() || null,
      });
      setStatusMessage(
        installed.target_remote_user_id
          ? `Installed ${installed.installed_skill.display_name} from ${installed.marketplace_id} with future remote user metadata ${installed.target_remote_user_id}.`
          : `Installed ${installed.installed_skill.display_name} from ${installed.marketplace_id}.`,
      );
      await loadSkills();
      await loadMarketplaceInstallHistory();
    } catch (err) {
      setMarketplaceError(err instanceof Error ? err.message : String(err));
    } finally {
      setMarketplaceInstallingName(null);
    }
  }
  return (
    <div className="skills-page">
      <div className="skills-header">
        <div>
          <h2>Skills</h2>
          <p>
            展示 runtime 发现到的 skills 与 Hermes parity toolsets 元数据。当前页支持启用控制，
            并可把本地 SKILL.md 渲染成 invocation payload、保存为 session context，或经 Runtime Adapter 执行受限 printf/echo 本地验证。
          </p>
        </div>
        <div className="skills-header-controls">
          <input
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search skills, toolsets, tools"
            aria-label="Search skills and toolsets"
          />
          <button
            className="skills-refresh"
            type="button"
            onClick={() => {
              void loadSkills();
              void loadMarketplaceInstallHistory();
            }}
          >
            刷新
          </button>
        </div>
      </div>

      {error ? <div className="skills-error">{error}</div> : null}
      {invocationError ? <div className="skills-error">{invocationError}</div> : null}
      {statusMessage ? <div className="skills-status-message">{statusMessage}</div> : null}

      <div className="skills-callout-grid">
        <article className="skills-callout">
          <span className="skills-callout-label">Discovery</span>
          <strong>{skills.length} discovered skills</strong>
          <p>来自 hermes / codex / agents / config 技能目录的本地发现结果。</p>
        </article>
        <article className="skills-callout">
          <span className="skills-callout-label">Toolsets</span>
          <strong>{toolsets.length} parity toolsets</strong>
          <p>工具集、工具可见性、启用状态与 availability 元数据现在作为一等配置面展示。</p>
        </article>
        <article className="skills-callout">
          <span className="skills-callout-label">Evolution</span>
          <strong>{pendingEvolutionCount} pending candidates</strong>
          <p>SkillClaw 风格的候选改进先进入本地 inbox，验证后再进入技能库。</p>
        </article>
      </div>

      <div className="skills-summary">
        <article className="skills-summary-card">
          <span className="skills-summary-value">{enabledCount}</span>
          <span className="skills-summary-label">enabled skills</span>
        </article>
        <article className="skills-summary-card">
          <span className="skills-summary-value">{disabledCount}</span>
          <span className="skills-summary-label">disabled skills</span>
        </article>
        <article className="skills-summary-card">
          <span className="skills-summary-value">{enabledToolsetCount}</span>
          <span className="skills-summary-label">enabled toolsets</span>
        </article>
        <article className="skills-summary-card">
          <span className="skills-summary-value">{enabledToolCount}</span>
          <span className="skills-summary-label">enabled tools</span>
        </article>
        <article className="skills-summary-card">
          <span className="skills-summary-value">{pendingEvolutionCount}</span>
          <span className="skills-summary-label">pending evolution</span>
        </article>
        <article className="skills-summary-card">
          <span className="skills-summary-value">{acceptedEvolutionCount}</span>
          <span className="skills-summary-label">accepted evolution</span>
        </article>
      </div>

      <section className="skills-marketplace-section">
        <div className="skills-section-heading">
          <div>
            <h3>Remote Skill Marketplace</h3>
            <p>
              从 file/http/https manifest 读取远端技能目录；安装时只写入本地 Hermes skills 目录，未提供外部账号或付费 SaaS 凭证时不会伪造远端结果。
            </p>
          </div>
          <span>{marketplaceCatalog ? `${marketplaceCatalog.skills.length} skills` : 'manifest required'}</span>
        </div>
        <div className="skills-marketplace-panel">
          <label>
            Manifest URL or local path
            <input
              value={marketplaceManifestUrl}
              onChange={(event) => setMarketplaceManifestUrl(event.target.value)}
              placeholder="file:///path/to/marketplace.json 或 /path/to/marketplace.json"
            />
          </label>
          <button
            className="skills-refresh"
            type="button"
            onClick={() => void handleLoadMarketplace()}
            disabled={marketplaceLoading}
          >
            {marketplaceLoading ? 'Loading catalog...' : 'Load marketplace'}
          </button>
        </div>
        {marketplaceError ? <div className="skills-error">{marketplaceError}</div> : null}
        <div className="skills-marketplace-panel">
          <div className="skills-section-heading">
            <div>
              <h4>Marketplace install history</h4>
              <p>
                最近的 marketplace 安装记录只反映本地 Hermes skills 目录写入结果，可按 marketplace、skill/name 与 limit 过滤，并导出当前已加载记录的本地审计 JSON。
              </p>
            </div>
            <span>
              {marketplaceHistoryLoading
                ? 'refreshing...'
                : `${marketplaceInstallHistory.length} loaded`}
            </span>
          </div>
          <div className="skills-marketplace-history-controls">
            <label>
              Marketplace ID
              <input
                value={marketplaceHistoryMarketplaceId}
                onChange={(event) => setMarketplaceHistoryMarketplaceId(event.target.value)}
                placeholder="Optional: marketplace id"
              />
            </label>
            <label>
              Skill / installed name
              <input
                value={marketplaceHistorySkillName}
                onChange={(event) => setMarketplaceHistorySkillName(event.target.value)}
                placeholder="Optional: skill or installed name"
              />
            </label>
            <label>
              Target remote user id
              <input
                value={marketplaceHistoryTargetRemoteUserId}
                onChange={(event) => setMarketplaceHistoryTargetRemoteUserId(event.target.value)}
                placeholder="Optional future remote user"
              />
            </label>
            <label>
              Fill from local remote user
              <select
                value={selectedMarketplaceRemoteUser?.user_id ?? ''}
                onChange={(event) => setMarketplaceHistoryTargetRemoteUserId(event.target.value)}
                disabled={marketplaceRemoteUsersLoading || marketplaceRemoteUsers.length === 0}
              >
                <option value="">
                  {marketplaceRemoteUsersLoading
                    ? 'Loading local Agent Exchange users...'
                    : 'Choose local Agent Exchange user'}
                </option>
                {marketplaceRemoteUsers.map((remoteUser) => (
                  <option key={remoteUser.user_id} value={remoteUser.user_id}>
                    {formatRemoteUserOptionLabel(remoteUser)}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Limit
              <input
                type="number"
                min={MIN_MARKETPLACE_HISTORY_LIMIT}
                max={MAX_MARKETPLACE_HISTORY_LIMIT}
                value={marketplaceHistoryLimit}
                onChange={(event) => setMarketplaceHistoryLimit(event.target.value)}
                placeholder={String(DEFAULT_MARKETPLACE_HISTORY_LIMIT)}
              />
            </label>
            <div className="skills-marketplace-history-actions">
              <button
                className="skills-refresh"
                type="button"
                onClick={() => void loadMarketplaceInstallHistory()}
                disabled={marketplaceHistoryLoading}
              >
                {marketplaceHistoryLoading ? 'Refreshing...' : 'Apply filters'}
              </button>
              <button
                className="skills-toggle"
                type="button"
                onClick={() => void handleCopyMarketplaceHistoryAudit()}
                disabled={marketplaceHistoryLoading}
              >
                Copy audit JSON
              </button>
              <button
                className="skills-toggle"
                type="button"
                onClick={handleDownloadMarketplaceHistoryAudit}
                disabled={marketplaceHistoryLoading}
              >
                Download audit JSON
              </button>
            </div>
          </div>
          <div className="skills-card-note skills-marketplace-history-note">
            <strong>Local audit export</strong>
            <p>
              Exports the currently loaded records as JSON. This audit records local Hermes
              skills-directory writes only. New installs can also persist target_remote_user_id
              on the local install-history row; the field remains future remote user routing
              metadata only and does not imply remote marketplace account activity or remote
              marketplace state changes.
            </p>
            <p>
              {marketplaceRemoteUsersError ??
                (marketplaceRemoteUsers.length > 0
                  ? `${marketplaceRemoteUsers.length} active local Agent Exchange future remote user(s) are available; selection only fills local routing metadata.`
                  : 'No active local Agent Exchange future remote users found yet; manual target_remote_user_id entry still works.')}
            </p>
          </div>
          {marketplaceHistoryError ? (
            <div className="skills-error">{marketplaceHistoryError}</div>
          ) : null}
          {marketplaceInstallHistory.length ? (
            <div className="skills-marketplace-grid">
              {marketplaceInstallHistory.map((entry) => (
                <article className="skills-marketplace-card" key={entry.id}>
                  <div>
                    <strong>{entry.display_name || entry.installed_skill_name || entry.skill_name}</strong>
                    <p>
                      {entry.installed_skill_name === entry.skill_name
                        ? entry.skill_name
                        : `${entry.skill_name} -> ${entry.installed_skill_name}`}
                    </p>
                  </div>
                  <div className="skills-marketplace-meta">
                    <span>{formatMarketplaceInstallTimestamp(entry.installed_at)}</span>
                    <span>{entry.marketplace_id}</span>
                    {entry.target_remote_user_id ? <span>target {entry.target_remote_user_id}</span> : null}
                    <span>{entry.source_url ? 'source linked' : 'manifest only'}</span>
                  </div>
                  <code>{entry.source_url ?? entry.manifest_url}</code>
                </article>
              ))}
            </div>
          ) : (
            <p>
              {marketplaceHistoryLoading
                ? 'Loading local marketplace install history...'
                : 'No local marketplace installs match the current history filters yet.'}
            </p>
          )}
        </div>
        {marketplaceCatalog ? (
          <div className="skills-marketplace-grid">
            {marketplaceCatalog.skills.map((entry) => (
              <article className="skills-marketplace-card" key={entry.name}>
                <div>
                  <strong>{entry.title || entry.name}</strong>
                  <p>{entry.description || 'No description supplied by manifest.'}</p>
                </div>
                <div className="skills-marketplace-meta">
                  <span>{entry.name}</span>
                  {entry.tags.map((tag) => (
                    <span key={tag}>{tag}</span>
                  ))}
                </div>
                {entry.source_url ? <code>{entry.source_url}</code> : <code>inline content</code>}
                <button
                  className="skills-toggle"
                  type="button"
                  onClick={() => void handleInstallMarketplaceSkill(entry.name)}
                  disabled={marketplaceInstallingName === entry.name}
                >
                  {marketplaceInstallingName === entry.name ? 'Installing...' : 'Install / update'}
                </button>
              </article>
            ))}
          </div>
        ) : null}
      </section>

      <section className="skills-session-context-section">
        <div className="skills-section-heading">
          <div>
            <h3>Session Skill Context</h3>
              <p>
                查看已保存到当前 session 的本地 skill invocation payload replay；这里保存的是 prompt
                内容，用于复核/移交；runtime validation 会单独走 Runtime Adapter allowlist。
            </p>
          </div>
          <div className="skills-inline-actions">
            <button
              className="skills-toggle"
              type="button"
              onClick={() =>
                latestSessionInvocationMessage
                  ? void handleCopySessionInvocationContent(latestSessionInvocationMessage, 'latest')
                  : undefined
              }
              disabled={!hasReplayContent(latestSessionInvocationMessage)}
            >
              Copy latest payload
            </button>
            <button
              className="skills-refresh"
              type="button"
              onClick={() => void loadSessionSkillInvocations()}
              disabled={!selectedInvocationSessionId || sessionInvocationLoading}
            >
              {sessionInvocationLoading ? '加载中...' : 'Refresh context'}
            </button>
          </div>
        </div>
        <div className="skills-session-context-controls">
          <label htmlFor="skill-session-context-select">
            Session
            <select
              id="skill-session-context-select"
              value={selectedInvocationSessionId}
              onChange={(event) => setSelectedInvocationSessionId(event.target.value)}
              disabled={recentSessions.length === 0}
            >
              {recentSessions.length === 0 ? <option value="">No recent sessions</option> : null}
              {recentSessions.map((session) => (
                <option key={session.id} value={session.id}>
                  {session.title} · {session.source}
                </option>
              ))}
            </select>
          </label>
        </div>
        {sessionInvocationMessages.length === 0 ? (
          <div className="skills-session-context-empty">No saved skill invocation context for this session yet.</div>
        ) : (
          <div className="skills-session-context-list">
            {sessionInvocationMessages.map((message) => (
              <article className="skills-session-context-card" key={message.id}>
                <div className="skills-invocation-result-header">
                  <strong>{message.role} · {message.source}</strong>
                  <div className="skills-inline-actions">
                    <span>{message.created_at}</span>
                    <button
                      className="skills-toggle"
                      type="button"
                      onClick={() => void handleCopySessionInvocationContent(message, 'selected')}
                      disabled={!hasReplayContent(message)}
                    >
                      {hasReplayContent(message) ? 'Copy payload' : 'No payload content'}
                    </button>
                  </div>
                </div>
                <p className="skills-invocation-note">
                  Saved replay content only. Runtime validation is recorded separately through the
                  Runtime Adapter allowlisted skill-tool path.
                </p>
                <pre>{message.content}</pre>
              </article>
            ))}
          </div>
        )}
      </section>

      <section className="skills-evolution-section">
        <div className="skills-section-heading">
          <div>
            <h3>Evolution Inbox</h3>
            <p>
              将会话轨迹、工具错误和人工观察先沉淀成候选改进；首版只做记录和评审，不自动改写
              SKILL.md。
            </p>
          </div>
          <span>
            {evolutionCandidates.length} candidates · {pendingEvolutionCount} pending
          </span>
        </div>

        <div className="skills-evolution-layout">
          <form className="skills-evolution-form" onSubmit={handleCreateEvolutionCandidate}>
            <div
              className={`skills-evolution-form-intro skills-evolution-form-intro-${evolutionForm.action}`}
            >
              <div className="skills-evolution-badges">
                <span
                  className={`skills-source skills-evolution-action-badge skills-evolution-action-${evolutionForm.action}`}
                >
                  {actionLabel(evolutionForm.action)}
                </span>
                <span className="skills-source skills-evolution-origin skills-evolution-origin-manual">
                  Manual entry
                </span>
              </div>
              <p>{actionDescription(evolutionForm.action)}</p>
              <span>
                {evolutionForm.action === 'refine'
                  ? 'Choose the target skill and describe the exact behavior change to keep.'
                  : evolutionForm.action === 'create'
                    ? 'Capture the gap first. New skills should read like reusable recovery or preflight patterns.'
                    : 'Use skip when you want to preserve evidence but avoid proposing a change yet.'}
              </span>
            </div>
            <div className="skills-form-row">
              <label>
                Action
                <select
                  value={evolutionForm.action}
                  onChange={(event) =>
                    setEvolutionForm((current) => ({
                      ...current,
                      action: event.target.value as SkillEvolutionAction,
                    }))
                  }
                >
                  <option value="refine">Refine existing skill</option>
                  <option value="create">Create new skill</option>
                  <option value="skip">Skip / observe</option>
                </select>
              </label>
              <label>
                Confidence
                <select
                  value={evolutionForm.confidence}
                  onChange={(event) =>
                    setEvolutionForm((current) => ({
                      ...current,
                      confidence: event.target.value as SkillEvolutionConfidence,
                    }))
                  }
                >
                  <option value="low">Low</option>
                  <option value="medium">Medium</option>
                  <option value="high">High</option>
                </select>
              </label>
            </div>
            <label>
              Target skill
              <input
                list="skill-evolution-targets"
                value={evolutionForm.targetSkillName}
                required={evolutionForm.action === 'refine'}
                onChange={(event) =>
                  setEvolutionForm((current) => ({
                    ...current,
                    targetSkillName: event.target.value,
                  }))
                }
                placeholder="frontend-design"
              />
            </label>
            <datalist id="skill-evolution-targets">
              {skills.map((skill) => (
                <option key={skill.name} value={skill.name}>
                  {skill.display_name}
                </option>
              ))}
            </datalist>
            <label>
              Evidence summary
              <textarea
                value={evolutionForm.evidenceSummary}
                required
                onChange={(event) =>
                  setEvolutionForm((current) => ({
                    ...current,
                    evidenceSummary: event.target.value,
                  }))
                }
                placeholder="Repeated failures, useful tool sequence, or validation evidence."
              />
            </label>
            <label>
              Recommended change
              <textarea
                value={evolutionForm.recommendedChange}
                required
                onChange={(event) =>
                  setEvolutionForm((current) => ({
                    ...current,
                    recommendedChange: event.target.value,
                  }))
                }
                placeholder="The exact skill refinement, new skill idea, or reason to skip."
              />
            </label>
            <label>
              Source refs
              <textarea
                value={evolutionForm.sourceRefsText}
                onChange={(event) =>
                  setEvolutionForm((current) => ({
                    ...current,
                    sourceRefsText: event.target.value,
                  }))
                }
                placeholder="session:abc123:Slack analysis&#10;run:def456:CLI retry"
              />
              <span className="skills-evolution-field-hint">
                Use `session`, `note`, `run`, `execution_step`, or `run_event` to keep provenance
                readable in the inbox.
              </span>
            </label>
            <label>
              Validation notes
              <input
                value={evolutionForm.validationNotes}
                onChange={(event) =>
                  setEvolutionForm((current) => ({
                    ...current,
                    validationNotes: event.target.value,
                  }))
                }
                placeholder="Optional reviewer or validation context"
              />
            </label>
            <div className="skills-evolution-actions">
              <button
                className="skills-toggle"
                type="button"
                disabled={generatingCandidates}
                onClick={() => void handleGenerateEvolutionCandidates()}
              >
                {generatingCandidates ? 'Scanning...' : 'Generate from runtime'}
              </button>
              <button className="skills-refresh" type="submit" disabled={creatingCandidate}>
                {creatingCandidate ? 'Recording...' : 'Record candidate'}
              </button>
            </div>
          </form>

          <div className="skills-evolution-list">
            {!loading && filteredEvolutionCandidates.length === 0 ? (
              <div className="skills-empty">暂无 skill evolution candidates。</div>
            ) : null}
            {filteredEvolutionCandidates.map((candidate) => {
              const origin = getCandidateOrigin(candidate);
              const candidateTitle =
                candidate.target_skill_name ||
                (candidate.action === 'create'
                  ? 'New skill candidate'
                  : actionLabel(candidate.action));

              return (
                <article
                  className={`skills-evolution-card skills-evolution-card-${candidate.action} skills-evolution-card-${origin.key}`}
                  key={candidate.id}
                >
                  <div className="skills-evolution-topline">
                    <div className="skills-evolution-badges">
                      <span className={`skills-source skills-evolution-origin skills-evolution-origin-${origin.key}`}>
                        {origin.label}
                      </span>
                      <span
                        className={`skills-source skills-evolution-action-badge skills-evolution-action-${candidate.action}`}
                      >
                        {actionLabel(candidate.action)}
                      </span>
                      <span className={`skills-source skills-evolution-status-${candidate.status}`}>
                        {statusLabel(candidate.status)}
                      </span>
                    </div>
                    <span className="skills-evolution-timestamp">
                      updated {formatCandidateTimestamp(candidate.updated_at)}
                    </span>
                  </div>

                  <div className="skills-card-header skills-evolution-header">
                    <div>
                      <h3>{candidateTitle}</h3>
                      <p>{actionDescription(candidate.action)}</p>
                    </div>
                    <div className="skills-evolution-confidence">
                      <strong>{candidate.confidence}</strong>
                      <span>confidence</span>
                    </div>
                  </div>

                  <div className="skills-evolution-summary-grid">
                    <div className="skills-evolution-summary-card">
                      <strong>Origin</strong>
                      <p>{origin.description}</p>
                    </div>
                    <div className="skills-evolution-summary-card">
                      <strong>Scope</strong>
                      <p>
                        {candidate.action === 'refine' && candidate.target_skill_name
                          ? `Targets ${candidate.target_skill_name} for an incremental update.`
                          : candidate.action === 'create'
                            ? 'Proposes a new reusable skill, checklist, or troubleshooting guide.'
                            : 'Keeps the signal visible without committing to a library change.'}
                      </p>
                    </div>
                  </div>

                  <div className="skills-evolution-copy">
                    <strong>Evidence</strong>
                    <p>{candidate.evidence_summary}</p>
                  </div>
                  <div className="skills-evolution-copy">
                    <strong>Recommended change</strong>
                    <p>{candidate.recommended_change}</p>
                  </div>
                  {candidate.source_refs.length > 0 ? (
                    <div className="skills-evolution-refs">
                      {candidate.source_refs.map((sourceRef) => {
                        const sourceMeta = getSourceRefKindMeta(sourceRef.kind);

                        return (
                          <span
                            className={`skills-evolution-ref skills-evolution-ref-${sourceMeta.tone}`}
                            key={`${sourceRef.kind}:${sourceRef.id}`}
                          >
                            <strong>{sourceMeta.label}</strong>
                            <span>{sourceRef.id}</span>
                            {sourceRef.title ? <em>{sourceRef.title}</em> : null}
                          </span>
                        );
                      })}
                    </div>
                  ) : null}
                  {candidate.validation_notes ? (
                    <div className="skills-card-note">{candidate.validation_notes}</div>
                  ) : null}
                  {candidate.status === 'pending' ? (
                    <div className="skills-toggle-row">
                      <span className="skills-status">requires validation before deployment</span>
                      <div className="skills-evolution-actions">
                        <button
                          className="skills-toggle skills-toggle-enabled"
                          type="button"
                          disabled={reviewingCandidateId === candidate.id}
                          onClick={() => void handleSetCandidateStatus(candidate, 'accepted')}
                        >
                          Accept
                        </button>
                        <button
                          className="skills-toggle"
                          type="button"
                          disabled={reviewingCandidateId === candidate.id}
                          onClick={() => void handleSetCandidateStatus(candidate, 'rejected')}
                        >
                          Reject
                        </button>
                      </div>
                    </div>
                  ) : null}
                </article>
              );
            })}
          </div>
        </div>
      </section>

      {loading ? (
        <div className="skills-empty">加载 skills 和 toolsets...</div>
      ) : null}

      {!loading &&
      hasQuery &&
      filteredSkills.length === 0 &&
      filteredToolsets.length === 0 &&
      filteredEvolutionCandidates.length === 0 ? (
        <div className="skills-empty">没有匹配的 skill、toolset 或 tool。</div>
      ) : null}

      {!loading && toolsets.length > 0 ? (
        <section className="skills-toolsets-section">
          <div className="skills-section-heading">
            <div>
              <h3>Toolsets</h3>
              <p>
                当前是结构化 toolset metadata，可用于后续 runtime 决定哪些工具可见、启用或受限。
              </p>
            </div>
            <span>
              visible tools {visibleToolCount} / enabled tools {enabledToolCount}
            </span>
          </div>
          <div className="skills-toolset-grid">
            {filteredToolsets.map((toolset) => (
              <article className="skills-toolset-card" key={toolset.id}>
                <div className="skills-card-header">
                  <div>
                    <h3>{toolset.name}</h3>
                    <p>
                      {toolset.source} · {toolset.enabled ? 'enabled' : 'disabled'}
                    </p>
                  </div>
                  <span
                    className={`skills-source ${
                      toolset.enabled ? 'skills-source-enabled' : 'skills-source-disabled'
                    }`}
                  >
                    {toolset.tools.length} tools
                  </span>
                </div>
                {toolset.description ? (
                  <div className="skills-card-note">{toolset.description}</div>
                ) : null}
                <div className="skills-tool-list">
                  {toolset.tools.map((tool) => (
                    <div className="skills-tool-row" key={tool.name}>
                      <div>
                        <strong>{tool.name}</strong>
                        <span>{tool.description}</span>
                      </div>
                      <div className="skills-tool-badges">
                        <span>{tool.availability}</span>
                        <span>{tool.visible ? 'visible' : 'hidden'}</span>
                        <span>{tool.enabled ? 'enabled' : 'disabled'}</span>
                      </div>
                    </div>
                  ))}
                </div>
              </article>
            ))}
          </div>
        </section>
      ) : null}

      {!loading && skills.length === 0 ? (
        <div className="skills-empty">
          <strong>没有发现本地 skills。</strong>
          <p>
            这通常意味着 runtime 还没有在 hermes / codex / agents / config 技能目录中发现
            可索引的 SKILL.md。
          </p>
        </div>
      ) : null}

      {!loading && filteredSkills.length > 0 ? (
        <div className="skills-grid">
          {filteredSkills.map((skill) => (
            <article className="skills-card" key={skill.name}>
              <div className="skills-card-header">
                <div>
                  <h3>{skill.display_name || skill.name}</h3>
                  <p>{skill.name}</p>
                </div>
                <span
                  className={`skills-source ${
                    skill.enabled ? 'skills-source-enabled' : 'skills-source-disabled'
                  }`}
                >
                  {skill.enabled ? 'enabled' : 'disabled'}
                </span>
              </div>

              <dl className="skills-metadata">
                <div>
                  <dt>source</dt>
                  <dd>{skill.source}</dd>
                </div>
                <div>
                  <dt>path</dt>
                  <dd className="skills-path">{skill.path}</dd>
                </div>
              </dl>

              <div className="skills-card-note">{getSlashHint(skill)}</div>

              <div className="skills-invoke-panel">
                <label htmlFor={`skill-instruction-${skill.name}`}>
                  Invocation instruction
                  <textarea
                    id={`skill-instruction-${skill.name}`}
                    value={skillInstructionByName[skill.name] ?? ''}
                    onChange={(event) =>
                      setSkillInstructionByName((current) => ({
                        ...current,
                        [skill.name]: event.target.value,
                      }))
                    }
                    placeholder="可选：补充这次调用的用户目标，例如 Draft a launch plan"
                    disabled={!skill.enabled}
                  />
                </label>
                <div className="skills-invoke-session-row">
                  <label htmlFor={`skill-session-${skill.name}`}>
                    Save to session
                    <select
                      id={`skill-session-${skill.name}`}
                      value={selectedInvocationSessionId}
                      onChange={(event) => setSelectedInvocationSessionId(event.target.value)}
                      disabled={!skill.enabled || recentSessions.length === 0}
                    >
                      {recentSessions.length === 0 ? <option value="">No recent sessions</option> : null}
                      {recentSessions.map((session) => (
                        <option key={session.id} value={session.id}>
                          {session.title} · {session.source}
                        </option>
                      ))}
                    </select>
                  </label>
                </div>
                <div className="skills-invoke-actions">
                  <span>
                    {skill.enabled
                      ? 'Ready for local payload rendering and safe runtime validation'
                      : 'Enable before invoking'}
                  </span>
                  <button
                    className="skills-toggle skills-toggle-enabled"
                    type="button"
                    onClick={() => void handleInvokeSkill(skill)}
                    disabled={!skill.enabled || invokingName === skill.name}
                  >
                    {invokingName === skill.name ? 'Rendering...' : 'Render payload'}
                  </button>
                  <button
                    className="skills-toggle"
                    type="button"
                    onClick={() => void handleInvokeSkillIntoSession(skill)}
                    disabled={!skill.enabled || !selectedInvocationSessionId || invokingName === `session:${skill.name}`}
                  >
                    {invokingName === `session:${skill.name}` ? 'Saving...' : 'Save to session'}
                  </button>
                  <button
                    className="skills-toggle"
                    type="button"
                    onClick={() => void handleExecuteSkillRuntime(skill, true)}
                    disabled={!skill.enabled || executingRuntimeName === `dry:${skill.name}`}
                  >
                    {executingRuntimeName === `dry:${skill.name}` ? 'Packaging...' : 'Dry-run package'}
                  </button>
                  <button
                    className="skills-toggle skills-toggle-enabled"
                    type="button"
                    onClick={() => void handleExecuteSkillRuntime(skill, false)}
                    disabled={!skill.enabled || executingRuntimeName === `run:${skill.name}`}
                  >
                    {executingRuntimeName === `run:${skill.name}` ? 'Executing...' : 'Run printf validation'}
                  </button>
                </div>
                {runtimeExecutionResult?.invocation.name === skill.name ? (
                  <div className="skills-runtime-result">
                    <div className="skills-invocation-result-header">
                      <strong>
                        {runtimeExecutionResult.executed ? 'Runtime executed' : 'Runtime package'}
                      </strong>
                      <span>
                        {runtimeExecutionResult.execution_package.command} · timeout{' '}
                        {runtimeExecutionResult.execution_package.timeout_ms} ms
                      </span>
                    </div>
                    <p className="skills-invocation-note">
                      {runtimeExecutionResult.summary}. This path only uses the local Runtime Adapter
                      allowlisted printf/echo validation command; it does not call a paid model provider.
                    </p>
                    {runtimeExecutionResult.runtime_result ? (
                      <dl className="skills-runtime-stats">
                        <div>
                          <dt>exit</dt>
                          <dd>{runtimeExecutionResult.runtime_result.exit_code}</dd>
                        </div>
                        <div>
                          <dt>duration</dt>
                          <dd>{runtimeExecutionResult.runtime_result.duration_ms} ms</dd>
                        </div>
                        <div>
                          <dt>timed out</dt>
                          <dd>{runtimeExecutionResult.runtime_result.timed_out ? 'yes' : 'no'}</dd>
                        </div>
                      </dl>
                    ) : null}
                    <details className="skills-invocation-details" open>
                      <summary>Execution package / result JSON</summary>
                      <pre>{formatRuntimeExecutionResult(runtimeExecutionResult)}</pre>
                    </details>
                  </div>
                ) : null}
                {invocationResult?.name === skill.name ? (
                  <div className="skills-invocation-result">
                    <div className="skills-invocation-result-header">
                      <strong>{invocationResult.command}</strong>
                      <div className="skills-inline-actions">
                        <span>local review/handoff payload</span>
                        <button
                          className="skills-toggle"
                          type="button"
                          onClick={() => void handleCopyInvocationPayload(invocationResult, 'json')}
                        >
                          Copy JSON
                        </button>
                        <button
                          className="skills-toggle"
                          type="button"
                          onClick={() => handleDownloadInvocationPayload(invocationResult, 'json')}
                        >
                          Download JSON
                        </button>
                        <button
                          className="skills-toggle"
                          type="button"
                          onClick={() => void handleCopyInvocationPayload(invocationResult, 'markdown')}
                        >
                          Copy Markdown
                        </button>
                        <button
                          className="skills-toggle"
                          type="button"
                          onClick={() =>
                            handleDownloadInvocationPayload(invocationResult, 'markdown')
                          }
                        >
                          Download Markdown
                        </button>
                      </div>
                    </div>
                    <p className="skills-invocation-note">
                      For local invocation payload review/handoff only. This preview has not been sent
                      to a real model or tool runtime.
                    </p>
                    <pre>{formatInvocationPayloadJson(invocationResult)}</pre>
                    <details className="skills-invocation-details">
                      <summary>Rendered prompt preview</summary>
                      <pre>{invocationResult.rendered_prompt}</pre>
                    </details>
                  </div>
                ) : null}
              </div>

              <div className="skills-toggle-row">
                <span
                  className={`skills-status ${
                    skill.enabled ? 'skills-status-enabled' : 'skills-status-disabled'
                  }`}
                >
                  {skill.enabled ? 'runtime candidate enabled' : 'runtime candidate disabled'}
                </span>
                <button
                  className={`skills-toggle ${skill.enabled ? 'skills-toggle-enabled' : ''}`}
                  type="button"
                  onClick={() => void handleToggleSkill(skill)}
                  disabled={savingName === skill.name}
                >
                  {savingName === skill.name
                    ? '保存中...'
                    : skill.enabled
                      ? 'Disable'
                      : 'Enable'}
                </button>
              </div>
            </article>
          ))}
        </div>
      ) : null}
    </div>
  );
}
