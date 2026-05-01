import { useEffect, useState, type FormEvent } from 'react';
import {
  agentExchangeListRemoteUsers,
  missionList,
  simulationCreateScenarioRun,
  simulationExportTemplateBundle,
  simulationExportTemplateBundleAuditLog,
  simulationGetComparisonMatrix,
  simulationGetOverview,
  simulationImportTemplateBundle,
  simulationListExternalSaasRuns,
  simulationListHighFidelitySandboxRuns,
  simulationListHandoffPolicyTemplates,
  simulationListTemplateBundleAuditLog,
  simulationListLocalSandboxRuns,
  simulationPreflightTemplateBundleImport,
  simulationRunExternalSaas,
  simulationRunHighFidelitySandbox,
  simulationRunLocalSandbox,
  simulationListScenarioRuns,
  simulationListScoringFormulaTemplates,
  simulationSaveHandoffPolicyTemplate,
  simulationSaveScoringFormulaTemplate,
  type AgentExchangeRemoteUser,
  type Mission,
  type ScenarioOptionCard,
  type ScenarioRun,
  type SimulationComparisonMatrix,
  type ScenarioVariable,
  type SimulationHandoffPolicyTemplate,
  type SimulationOverview,
  type SimulationScoringFormulaTemplate,
  type SimulationExportTemplateBundleAuditLogResponse,
  type SimulationExternalSaasRun,
  type SimulationExternalSaasRunHistoryItem,
  type SimulationHighFidelitySandboxRun,
  type SimulationHighFidelitySandboxRunHistoryItem,
  type SimulationLocalSandboxRun,
  type SimulationSandboxAgentRequest,
  type SimulationTemplateBundleAuditEntry,
  type SimulationTemplateBundlePreflightResponse,
} from '../lib/tauri';
import './SimulationPage.css';

const handoffTargetOptions = [
  {
    value: 'council_and_execution',
    label: 'Council + Execution',
    description: 'Create both a Scenario Reviewer Council step and an Execution review step.',
  },
  {
    value: 'council_only',
    label: 'Council only',
    description: 'Create only the Scenario Reviewer Council step after saving.',
  },
  {
    value: 'execution_only',
    label: 'Execution only',
    description: 'Create only the Execution review step after saving.',
  },
  {
    value: 'timeline_only',
    label: 'Timeline only',
    description: 'Record the simulation run and timeline event without review steps.',
  },
];

const executionRiskOptions = ['low', 'medium', 'high'];

interface ScenarioScoringFormula {
  baseScore: number;
  impactMultiplier: number;
  uncertaintyPenalty: number;
}

interface LocalSandboxReplayPayload {
  run_id: string;
  engine: string;
  agents: SimulationLocalSandboxRun['agents'];
  turns: SimulationLocalSandboxRun['turns'];
  option_scores: SimulationLocalSandboxRun['option_scores'];
  recommendation: SimulationLocalSandboxRun['recommendation'];
  audit_event_id: SimulationLocalSandboxRun['audit_event_id'];
}

interface LocalSandboxReplayActionState {
  tone: 'success' | 'error';
  message: string;
}

interface SimulationEvidenceActionState {
  tone: 'success' | 'error';
  message: string;
}

interface SimulationCapabilityEvidenceBundle {
  schema_version: string;
  exported_at: string;
  mission_id: string;
  target_remote_user_id: string | null;
  target_remote_user_profile: {
    user_id: string;
    display_name: string;
    default_agent_id: string;
    transport_label: string | null;
    route_hint: string | null;
    status: AgentExchangeRemoteUser['status'];
    created_at: string;
    updated_at: string;
  } | null;
  counts: {
    external_saas_runs: number;
    high_fidelity_sandbox_runs: number;
  };
  boundary_notes: string[];
  external_saas_runs: SimulationExternalSaasRunHistoryItem[];
  high_fidelity_sandbox_runs: SimulationHighFidelitySandboxRunHistoryItem[];
}

const defaultScoringFormula: ScenarioScoringFormula = {
  baseScore: 56,
  impactMultiplier: 18,
  uncertaintyPenalty: 14,
};

const SIMULATION_CAPABILITY_EVIDENCE_FILENAME = 'simulation-capability-evidence.json';
const AGENT_EXCHANGE_REMOTE_USER_LIMIT = 50;
const SIMULATION_CAPABILITY_EVIDENCE_BOUNDARY_NOTES = [
  'Dry-run previews may not call the network and should be treated as local capability previews only.',
  'Non-dry-run http_json requests require backend confirmation before any network call is attempted.',
  'High-fidelity sandbox output comes from a deterministic local world model, not an external high-fidelity simulator.',
  'target_remote_user_id is future remote user routing metadata only, not proof of remote delivery.',
];

function formatRemoteUserOptionLabel(remoteUser: AgentExchangeRemoteUser) {
  const displayName = remoteUser.display_name.trim();
  const defaultAgentId = remoteUser.default_agent_id.trim();
  if (displayName && defaultAgentId) {
    return `${displayName} (${remoteUser.user_id}) · ${defaultAgentId}`;
  }
  if (displayName) {
    return `${displayName} (${remoteUser.user_id})`;
  }
  return remoteUser.user_id;
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

function parseLocalSandboxAgents(value: string): SimulationSandboxAgentRequest[] {
  return value
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const parts = line.split('|').map((part) => part.trim());
      return {
        role: parts[0] || 'strategy',
        stance: parts[1] || 'balanced',
        name: parts[2] || null,
      };
    });
}

function extractErrorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

export function SimulationPage() {
  const [overview, setOverview] = useState<SimulationOverview | null>(null);
  const [missions, setMissions] = useState<Mission[]>([]);
  const [scenarioRuns, setScenarioRuns] = useState<ScenarioRun[]>([]);
  const [comparisonMatrix, setComparisonMatrix] = useState<SimulationComparisonMatrix | null>(null);
  const [handoffPolicyTemplates, setHandoffPolicyTemplates] = useState<SimulationHandoffPolicyTemplate[]>([]);
  const [scoringFormulaTemplates, setScoringFormulaTemplates] = useState<SimulationScoringFormulaTemplate[]>([]);
  const [selectedMissionId, setSelectedMissionId] = useState('');
  const [baseline, setBaseline] = useState('');
  const [optionsText, setOptionsText] = useState('');
  const [variables, setVariables] = useState<ScenarioVariable[]>(() => [createScenarioVariable()]);
  const [comparisonSummary, setComparisonSummary] = useState('');
  const [recommendation, setRecommendation] = useState('');
  const [selectedOptionId, setSelectedOptionId] = useState('');
  const [handoffTarget, setHandoffTarget] = useState('council_and_execution');
  const [executionRiskLevel, setExecutionRiskLevel] = useState('medium');
  const [selectedHandoffTemplateId, setSelectedHandoffTemplateId] = useState('council-and-execution');
  const [handoffTemplateName, setHandoffTemplateName] = useState('');
  const [handoffTemplateStatus, setHandoffTemplateStatus] = useState<string | null>(null);
  const [selectedFormulaTemplateId, setSelectedFormulaTemplateId] = useState('balanced');
  const [formulaTemplateName, setFormulaTemplateName] = useState('');
  const [formulaTemplateStatus, setFormulaTemplateStatus] = useState<string | null>(null);
  const [templateBundleText, setTemplateBundleText] = useState('');
  const [templateBundleStatus, setTemplateBundleStatus] = useState<string | null>(null);
  const [templateBundleAuditLog, setTemplateBundleAuditLog] = useState<SimulationTemplateBundleAuditEntry[]>([]);
  const [templateBundleAuditExport, setTemplateBundleAuditExport] = useState<SimulationExportTemplateBundleAuditLogResponse | null>(null);
  const [templateBundlePreflight, setTemplateBundlePreflight] = useState<SimulationTemplateBundlePreflightResponse | null>(null);
  const [localSandboxAgentsText, setLocalSandboxAgentsText] = useState('strategy|optimistic|Strategy Agent\nrisk|skeptical|Risk Agent\nops|pragmatic|Ops Agent');
  const [localSandboxRounds, setLocalSandboxRounds] = useState(3);
  const [localSandboxResult, setLocalSandboxResult] = useState<SimulationLocalSandboxRun | null>(null);
  const [externalSaasProvider, setExternalSaasProvider] = useState('local_echo');
  const [externalSaasEndpoint, setExternalSaasEndpoint] = useState('https://example.invalid/simulate');
  const [externalSaasInputJson, setExternalSaasInputJson] = useState('{\n  "scenario": "local simulation adapter",\n  "reward_hint": 1\n}');
  const [externalSaasConfirmPhrase, setExternalSaasConfirmPhrase] = useState('');
  const [externalSaasResult, setExternalSaasResult] = useState<SimulationExternalSaasRun | null>(null);
  const [highFidelityResult, setHighFidelityResult] = useState<SimulationHighFidelitySandboxRun | null>(null);
  const [localSandboxHistory, setLocalSandboxHistory] = useState<SimulationLocalSandboxRun[]>([]);
  const [externalSaasHistory, setExternalSaasHistory] = useState<SimulationExternalSaasRunHistoryItem[]>([]);
  const [highFidelityHistory, setHighFidelityHistory] = useState<SimulationHighFidelitySandboxRunHistoryItem[]>([]);
  const [localSandboxHistoryError, setLocalSandboxHistoryError] = useState<string | null>(null);
  const [externalSaasHistoryError, setExternalSaasHistoryError] = useState<string | null>(null);
  const [highFidelityHistoryError, setHighFidelityHistoryError] = useState<string | null>(null);
  const [localSandboxStatus, setLocalSandboxStatus] = useState<string | null>(null);
  const [localSandboxLoading, setLocalSandboxLoading] = useState(false);
  const [selectedLocalSandboxRunId, setSelectedLocalSandboxRunId] = useState<string | null>(null);
  const [localSandboxReplayActionState, setLocalSandboxReplayActionState] =
    useState<LocalSandboxReplayActionState | null>(null);
  const [simulationEvidenceJson, setSimulationEvidenceJson] = useState('');
  const [simulationEvidenceTargetRemoteUserId, setSimulationEvidenceTargetRemoteUserId] =
    useState('');
  const [agentExchangeRemoteUsers, setAgentExchangeRemoteUsers] = useState<AgentExchangeRemoteUser[]>([]);
  const [agentExchangeRemoteUsersLoading, setAgentExchangeRemoteUsersLoading] = useState(false);
  const [agentExchangeRemoteUsersError, setAgentExchangeRemoteUsersError] = useState<string | null>(null);
  const [simulationEvidenceStatus, setSimulationEvidenceStatus] =
    useState<SimulationEvidenceActionState | null>(null);
  const [scoringFormula, setScoringFormula] = useState<ScenarioScoringFormula>(defaultScoringFormula);
  const [loading, setLoading] = useState(true);
  const [scenarioRunsLoading, setScenarioRunsLoading] = useState(false);
  const [comparisonLoading, setComparisonLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [scenarioRunsError, setScenarioRunsError] = useState<string | null>(null);
  const [comparisonError, setComparisonError] = useState<string | null>(null);
  const [formError, setFormError] = useState<string | null>(null);
  const [comparisonReloadKey, setComparisonReloadKey] = useState(0);

  const selectedMission =
    missions.find((mission) => mission.id === selectedMissionId) ?? missions[0] ?? null;
  const parsedOptions = parseOptions(optionsText);
  const activeVariables = sanitizeVariables(variables);
  const previewCards = buildOptionCards(parsedOptions, activeVariables, scoringFormula);
  const recommendedCard = getSelectedCard(previewCards, selectedOptionId);
  const comparisonDraft = buildComparisonSummary(previewCards, recommendedCard?.id ?? null);
  const recommendationDraft = buildRecommendationReason(previewCards, recommendedCard?.id ?? null);
  const pathEvolutionSteps = [...(comparisonMatrix?.path_evolution ?? [])].sort((left, right) =>
    left.created_at.localeCompare(right.created_at),
  );
  const latestPathStep = pathEvolutionSteps[pathEvolutionSteps.length - 1] ?? null;
  const selectedLocalSandboxRun =
    localSandboxHistory.find((run) => run.run_id === selectedLocalSandboxRunId) ??
    (localSandboxResult?.run_id === selectedLocalSandboxRunId ? localSandboxResult : null) ??
    localSandboxHistory[0] ??
    localSandboxResult;
  const selectedLocalSandboxReplayPayload = selectedLocalSandboxRun
    ? buildLocalSandboxReplayPayload(selectedLocalSandboxRun)
    : null;
  const selectedLocalSandboxReplayJson = selectedLocalSandboxReplayPayload
    ? JSON.stringify(selectedLocalSandboxReplayPayload, null, 2)
    : '';
  const selectedSimulationRemoteUser =
    agentExchangeRemoteUsers.find(
      (remoteUser) => remoteUser.user_id === simulationEvidenceTargetRemoteUserId.trim(),
    ) ?? null;

  useEffect(() => {
    let cancelled = false;

    async function loadPage() {
      setLoading(true);
      setError(null);
      try {
        const [overviewData, missionData, handoffTemplates, formulaTemplates, auditLog] = await Promise.all([
          simulationGetOverview(),
          missionList({ limit: 50 }),
          simulationListHandoffPolicyTemplates(),
          simulationListScoringFormulaTemplates(),
          simulationListTemplateBundleAuditLog(),
        ]);

        if (cancelled) {
          return;
        }

        const defaultFormulaTemplate =
          formulaTemplates.find((template) => template.id === 'balanced') ?? formulaTemplates[0];

        setOverview(overviewData);
        setMissions(missionData);
        setHandoffPolicyTemplates(handoffTemplates);
        setScoringFormulaTemplates(formulaTemplates);
        setTemplateBundleAuditLog(auditLog);
        if (defaultFormulaTemplate) {
          setSelectedFormulaTemplateId(defaultFormulaTemplate.id);
          setScoringFormula(formulaFromTemplate(defaultFormulaTemplate));
        }
        setSelectedMissionId((current) =>
          current && missionData.some((mission) => mission.id === current)
            ? current
            : missionData[0]?.id ?? '',
        );
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err));
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    void loadPage();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    void loadAgentExchangeRemoteUsers();
  }, []);

  useEffect(() => {
    if (!selectedMissionId) {
      setScenarioRuns([]);
      setScenarioRunsError(null);
      return;
    }

    let cancelled = false;

    async function loadScenarioRuns() {
      setScenarioRunsLoading(true);
      setScenarioRunsError(null);
      try {
        const items = await simulationListScenarioRuns(selectedMissionId);
        if (!cancelled) {
          setScenarioRuns(items);
        }
      } catch (err) {
        if (!cancelled) {
          setScenarioRunsError(err instanceof Error ? err.message : String(err));
        }
      } finally {
        if (!cancelled) {
          setScenarioRunsLoading(false);
        }
      }
    }

    void loadScenarioRuns();
    return () => {
      cancelled = true;
    };
  }, [selectedMissionId]);

  useEffect(() => {
    if (!selectedMissionId) {
      setComparisonMatrix(null);
      setComparisonLoading(false);
      setComparisonError(null);
      return;
    }

    let cancelled = false;

    async function loadComparisonMatrix() {
      setComparisonLoading(true);
      setComparisonError(null);
      try {
        const matrix = await simulationGetComparisonMatrix({ mission_id: selectedMissionId });
        if (!cancelled) {
          setComparisonMatrix(matrix);
        }
      } catch (err) {
        if (!cancelled) {
          setComparisonMatrix(null);
          setComparisonError(err instanceof Error ? err.message : String(err));
        }
      } finally {
        if (!cancelled) {
          setComparisonLoading(false);
        }
      }
    }

    void loadComparisonMatrix();
    return () => {
      cancelled = true;
    };
  }, [comparisonReloadKey, selectedMissionId]);

  useEffect(() => {
    if (previewCards.length === 0) {
      if (selectedOptionId) {
        setSelectedOptionId('');
      }
      return;
    }

    if (!previewCards.some((card) => card.id === selectedOptionId)) {
      setSelectedOptionId(getTopOptionId(previewCards));
    }
  }, [optionsText, selectedOptionId, variables]);

  function handleAddVariable() {
    setVariables((current) => [...current, createScenarioVariable()]);
  }

  function handleUpdateVariable(
    variableId: string,
    field: keyof ScenarioVariable,
    value: string | number,
  ) {
    setVariables((current) =>
      current.map((variable) =>
        variable.id === variableId ? { ...variable, [field]: value } : variable,
      ),
    );
  }

  function handleRemoveVariable(variableId: string) {
    setVariables((current) => {
      const next = current.filter((variable) => variable.id !== variableId);
      return next.length > 0 ? next : [createScenarioVariable()];
    });
  }

  function handleApplyHandoffTemplate(templateId: string) {
    setSelectedHandoffTemplateId(templateId);
    const template = handoffPolicyTemplates.find((item) => item.id === templateId);
    if (!template) {
      return;
    }
    setHandoffTarget(template.handoff_target);
    setExecutionRiskLevel(template.execution_risk_level);
    setHandoffTemplateName(template.name);
    setHandoffTemplateStatus(`Applied ${template.name}.`);
  }

  useEffect(() => {
    if (!selectedMissionId) {
      setLocalSandboxHistory([]);
      setLocalSandboxHistoryError(null);
      return;
    }

    let cancelled = false;
    async function loadLocalSandboxHistory() {
      setLocalSandboxHistoryError(null);
      try {
        const history = await simulationListLocalSandboxRuns({
          mission_id: selectedMissionId,
          limit: 5,
        });
        if (!cancelled) {
          setLocalSandboxHistory(history);
        }
      } catch (err) {
        if (!cancelled) {
          setLocalSandboxHistory([]);
          setLocalSandboxHistoryError(err instanceof Error ? err.message : String(err));
        }
      }
    }

    void loadLocalSandboxHistory();
    return () => {
      cancelled = true;
    };
  }, [selectedMissionId, comparisonReloadKey]);

  useEffect(() => {
    if (!selectedMissionId) {
      setExternalSaasHistory([]);
      setHighFidelityHistory([]);
      setExternalSaasHistoryError(null);
      setHighFidelityHistoryError(null);
      return;
    }

    let cancelled = false;

    async function loadRecentSimulationExtensionRuns() {
      setExternalSaasHistoryError(null);
      setHighFidelityHistoryError(null);

      const [externalResult, highFidelityHistoryResult] = await Promise.allSettled([
        simulationListExternalSaasRuns({
          mission_id: selectedMissionId,
          limit: 5,
          target_remote_user_id: simulationEvidenceTargetRemoteUserId.trim() || null,
        }),
        simulationListHighFidelitySandboxRuns({
          mission_id: selectedMissionId,
          limit: 5,
          target_remote_user_id: simulationEvidenceTargetRemoteUserId.trim() || null,
        }),
      ]);

      if (cancelled) {
        return;
      }

      if (externalResult.status === 'fulfilled') {
        setExternalSaasHistory(externalResult.value);
      } else {
        setExternalSaasHistory([]);
        setExternalSaasHistoryError(extractErrorMessage(externalResult.reason));
      }

      if (highFidelityHistoryResult.status === 'fulfilled') {
        setHighFidelityHistory(highFidelityHistoryResult.value);
      } else {
        setHighFidelityHistory([]);
        setHighFidelityHistoryError(extractErrorMessage(highFidelityHistoryResult.reason));
      }
    }

    void loadRecentSimulationExtensionRuns();
    return () => {
      cancelled = true;
    };
  }, [selectedMissionId, comparisonReloadKey]);

  useEffect(() => {
    if (localSandboxHistory.length === 0) {
      setSelectedLocalSandboxRunId(localSandboxResult?.run_id ?? null);
      return;
    }

    setSelectedLocalSandboxRunId((current) => {
      if (current && localSandboxHistory.some((run) => run.run_id === current)) {
        return current;
      }
      if (localSandboxResult && localSandboxHistory.some((run) => run.run_id === localSandboxResult.run_id)) {
        return localSandboxResult.run_id;
      }
      return localSandboxHistory[0]?.run_id ?? null;
    });
  }, [localSandboxHistory, localSandboxResult]);

  useEffect(() => {
    setLocalSandboxReplayActionState(null);
  }, [selectedLocalSandboxRunId]);

  useEffect(() => {
    setSimulationEvidenceJson('');
    setSimulationEvidenceStatus(null);
  }, [selectedMissionId, simulationEvidenceTargetRemoteUserId]);

  async function loadAgentExchangeRemoteUsers() {
    setAgentExchangeRemoteUsersLoading(true);
    setAgentExchangeRemoteUsersError(null);
    try {
      const remoteUsers = await agentExchangeListRemoteUsers({
        status: 'active',
        limit: AGENT_EXCHANGE_REMOTE_USER_LIMIT,
      });
      setAgentExchangeRemoteUsers(remoteUsers);
    } catch (err) {
      setAgentExchangeRemoteUsersError(
        `Local Agent Exchange remote user load failed: ${extractErrorMessage(err)}`,
      );
    } finally {
      setAgentExchangeRemoteUsersLoading(false);
    }
  }

  async function handleSaveHandoffTemplate() {
    const name = handoffTemplateName.trim();
    if (!name) {
      setHandoffTemplateStatus('Name the template before saving it.');
      return;
    }

    try {
      const saved = await simulationSaveHandoffPolicyTemplate({
        id: selectedHandoffTemplateId || null,
        name,
        handoff_target: handoffTarget,
        execution_risk_level: executionRiskLevel,
        description: describeHandoffOutcome(handoffTarget, executionRiskLevel),
      });
      const templates = await simulationListHandoffPolicyTemplates();
      setHandoffPolicyTemplates(templates);
      setSelectedHandoffTemplateId(saved.id);
      setHandoffTemplateName(saved.name);
      setHandoffTemplateStatus(`Saved ${saved.name}.`);
    } catch (err) {
      setHandoffTemplateStatus(err instanceof Error ? err.message : String(err));
    }
  }

  function handleApplyFormulaTemplate(templateId: string) {
    setSelectedFormulaTemplateId(templateId);
    const template = scoringFormulaTemplates.find((item) => item.id === templateId);
    if (!template) {
      return;
    }

    setScoringFormula(formulaFromTemplate(template));
    setFormulaTemplateName(template.name);
    setFormulaTemplateStatus(`Applied ${template.name}.`);
  }

  function handleResetFormula() {
    const defaultTemplate =
      scoringFormulaTemplates.find((template) => template.id === 'balanced') ?? scoringFormulaTemplates[0];

    if (defaultTemplate) {
      setSelectedFormulaTemplateId(defaultTemplate.id);
      setScoringFormula(formulaFromTemplate(defaultTemplate));
    } else {
      setSelectedFormulaTemplateId('balanced');
      setScoringFormula(defaultScoringFormula);
    }

    setFormulaTemplateName('');
    setFormulaTemplateStatus(null);
  }

  async function handleSaveFormulaTemplate() {
    const name = formulaTemplateName.trim();
    if (!name) {
      setFormulaTemplateStatus('Name the formula template before saving it.');
      return;
    }

    const selectedTemplate = scoringFormulaTemplates.find(
      (template) => template.id === selectedFormulaTemplateId,
    );
    const templateId =
      selectedTemplate?.name.trim().toLowerCase() === name.toLowerCase() ? selectedTemplate.id : null;

    try {
      const saved = await simulationSaveScoringFormulaTemplate({
        id: templateId,
        name,
        base_score: scoringFormula.baseScore,
        impact_multiplier: scoringFormula.impactMultiplier,
        uncertainty_penalty: scoringFormula.uncertaintyPenalty,
        description: describeFormulaTemplate(scoringFormula),
      });
      const templates = await simulationListScoringFormulaTemplates();
      setScoringFormulaTemplates(templates);
      setSelectedFormulaTemplateId(saved.id);
      setFormulaTemplateName(saved.name);
      setScoringFormula(formulaFromTemplate(saved));
      setFormulaTemplateStatus(`Saved ${saved.name}.`);
    } catch (err) {
      setFormulaTemplateStatus(err instanceof Error ? err.message : String(err));
    }
  }

  async function handleExportTemplateBundle() {
    try {
      const bundle = await simulationExportTemplateBundle();
      setTemplateBundleText(JSON.stringify(bundle, null, 2));
      setTemplateBundleStatus(
        `Exported ${bundle.handoff_policy_templates.length} policy templates and ${bundle.scoring_formula_templates.length} formula templates.`,
      );
      setTemplateBundleAuditLog(await simulationListTemplateBundleAuditLog());
    } catch (err) {
      setTemplateBundleStatus(err instanceof Error ? err.message : String(err));
    }
  }

  async function handleRunLocalSandbox() {
    if (!selectedMissionId) {
      setLocalSandboxStatus('Select a Mission before running the local sandbox.');
      return;
    }
    if (!baseline.trim()) {
      setLocalSandboxStatus('Add a baseline before running the local sandbox.');
      return;
    }
    if (parsedOptions.length === 0) {
      setLocalSandboxStatus('Add at least one option before running the local sandbox.');
      return;
    }

    setLocalSandboxLoading(true);
    setLocalSandboxStatus(null);
    try {
      const result = await simulationRunLocalSandbox({
        mission_id: selectedMissionId,
        baseline,
        options: parsedOptions,
        agents: parseLocalSandboxAgents(localSandboxAgentsText),
        rounds: localSandboxRounds,
      });
      setLocalSandboxResult(result);
      setSelectedLocalSandboxRunId(result.run_id);
      setComparisonReloadKey((value) => value + 1);
      setScenarioRuns(await simulationListScenarioRuns(selectedMissionId));
      setLocalSandboxHistory(await simulationListLocalSandboxRuns({ mission_id: selectedMissionId, limit: 5 }));
      setLocalSandboxStatus(
        `Local sandbox completed with ${result.turns.length} turns; recommendation: ${result.recommendation.option}.`,
      );
    } catch (err) {
      setLocalSandboxStatus(err instanceof Error ? err.message : String(err));
    } finally {
      setLocalSandboxLoading(false);
    }
  }


  async function handleRunExternalSaas(dryRun: boolean) {
    if (!selectedMissionId) {
      setLocalSandboxStatus('Select a Mission before running an external SaaS simulation adapter.');
      return;
    }

    setLocalSandboxLoading(true);
    setLocalSandboxStatus(null);
    try {
      const result = await simulationRunExternalSaas({
        mission_id: selectedMissionId,
        provider: externalSaasProvider,
        endpoint_url: externalSaasProvider === 'http_json' ? externalSaasEndpoint : null,
        input_json: externalSaasInputJson,
        dry_run: dryRun,
        confirmation_phrase: dryRun ? null : externalSaasConfirmPhrase,
        target_remote_user_id: simulationEvidenceTargetRemoteUserId.trim() || null,
      });
      setExternalSaasResult(result);
      try {
        const recentRuns = await simulationListExternalSaasRuns({
          mission_id: selectedMissionId,
          limit: 5,
          target_remote_user_id: simulationEvidenceTargetRemoteUserId.trim() || null,
        });
        setExternalSaasHistory(recentRuns);
        setExternalSaasHistoryError(null);
      } catch (historyErr) {
        setExternalSaasHistory([]);
        setExternalSaasHistoryError(
          `Latest run completed, but history refresh failed: ${extractErrorMessage(historyErr)}`,
        );
      }
      setComparisonReloadKey((value) => value + 1);
      setLocalSandboxStatus(result.summary);
    } catch (err) {
      setLocalSandboxStatus(extractErrorMessage(err));
    } finally {
      setLocalSandboxLoading(false);
    }
  }

  async function handleRunHighFidelitySandbox() {
    if (!selectedMissionId) {
      setLocalSandboxStatus('Select a Mission before running the high-fidelity sandbox.');
      return;
    }
    if (!baseline.trim()) {
      setLocalSandboxStatus('Add a baseline before running the high-fidelity sandbox.');
      return;
    }
    if (parsedOptions.length === 0) {
      setLocalSandboxStatus('Add at least one option before running the high-fidelity sandbox.');
      return;
    }

    setLocalSandboxLoading(true);
    setLocalSandboxStatus(null);
    try {
      const result = await simulationRunHighFidelitySandbox({
        mission_id: selectedMissionId,
        baseline,
        options: parsedOptions,
        agents: parseLocalSandboxAgents(localSandboxAgentsText),
        rounds: localSandboxRounds,
        variables: activeVariables,
        target_remote_user_id: simulationEvidenceTargetRemoteUserId.trim() || null,
      });
      setHighFidelityResult(result);
      try {
        const recentRuns = await simulationListHighFidelitySandboxRuns({
          mission_id: selectedMissionId,
          limit: 5,
          target_remote_user_id: simulationEvidenceTargetRemoteUserId.trim() || null,
        });
        setHighFidelityHistory(recentRuns);
        setHighFidelityHistoryError(null);
      } catch (historyErr) {
        setHighFidelityHistory([]);
        setHighFidelityHistoryError(
          `Latest run completed, but history refresh failed: ${extractErrorMessage(historyErr)}`,
        );
      }
      setComparisonReloadKey((value) => value + 1);
      setLocalSandboxStatus(result.summary);
    } catch (err) {
      setLocalSandboxStatus(extractErrorMessage(err));
    } finally {
      setLocalSandboxLoading(false);
    }
  }

  async function handleExportTemplateBundleAuditLog() {
    try {
      const exported = await simulationExportTemplateBundleAuditLog({ limit: 50 });
      setTemplateBundleAuditExport(exported);
      setTemplateBundleStatus(
        `Exported ${exported.exported_count}/${exported.total} local audit events as read-only JSON.`,
      );
    } catch (err) {
      setTemplateBundleStatus(err instanceof Error ? err.message : String(err));
    }
  }

  async function handlePreflightTemplateBundle() {
    const bundleJson = templateBundleText.trim();
    if (!bundleJson) {
      setTemplateBundleStatus('Paste a template bundle JSON before running preflight.');
      return;
    }

    try {
      const preflight = await simulationPreflightTemplateBundleImport({ bundle_json: bundleJson });
      setTemplateBundlePreflight(preflight);
      setTemplateBundleStatus(
        `Preflight: ${preflight.total_count} templates, ${preflight.conflicts.length} updates/conflicts.`,
      );
    } catch (err) {
      setTemplateBundleStatus(err instanceof Error ? err.message : String(err));
    }
  }

  async function handleImportTemplateBundle() {
    const bundleJson = templateBundleText.trim();
    if (!bundleJson) {
      setTemplateBundleStatus('Paste a template bundle JSON before importing.');
      return;
    }

    try {
      const imported = await simulationImportTemplateBundle({ bundle_json: bundleJson });
      setHandoffPolicyTemplates(imported.handoff_policy_templates);
      setScoringFormulaTemplates(imported.scoring_formula_templates);
      setTemplateBundleStatus(
        `Imported ${imported.imported_handoff_policy_templates} policy templates and ${imported.imported_scoring_formula_templates} formula templates.`,
      );
      setTemplateBundlePreflight(null);
      setTemplateBundleAuditLog(await simulationListTemplateBundleAuditLog());
    } catch (err) {
      setTemplateBundleStatus(err instanceof Error ? err.message : String(err));
    }
  }

  async function handleSaveScenarioRun(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (!selectedMissionId) {
      setFormError('请选择一个 Mission。');
      return;
    }

    if (!baseline.trim()) {
      setFormError('请填写 baseline。');
      return;
    }

    if (parsedOptions.length === 0) {
      setFormError('请至少提供一个 option。');
      return;
    }

    const nextVariables = sanitizeVariables(variables);
    const nextOptionCards = buildOptionCards(parsedOptions, nextVariables, scoringFormula);
    const nextSelectedOptionId =
      nextOptionCards.find((card) => card.id === selectedOptionId)?.id ?? getTopOptionId(nextOptionCards);
    const nextSelectedOptionLabel =
      nextOptionCards.find((card) => card.id === nextSelectedOptionId)?.label ?? null;

    setSaving(true);
    setFormError(null);

    try {
      await simulationCreateScenarioRun({
        mission_id: selectedMissionId,
        baseline: baseline.trim(),
        options: parsedOptions,
        variables: nextVariables,
        option_cards: nextOptionCards,
        recommendation: nextSelectedOptionLabel,
        recommendation_reason:
          recommendation.trim() || buildRecommendationReason(nextOptionCards, nextSelectedOptionId),
        comparison_summary:
          comparisonSummary.trim() || buildComparisonSummary(nextOptionCards, nextSelectedOptionId),
        selected_option_id: nextSelectedOptionId,
        handoff_target: handoffTarget,
        execution_risk_level: executionRiskLevel,
      });

      const items = await simulationListScenarioRuns(selectedMissionId);
      setScenarioRuns(items);
      setComparisonReloadKey((current) => current + 1);
      setBaseline('');
      setOptionsText('');
      setVariables([createScenarioVariable()]);
      setComparisonSummary('');
      setRecommendation('');
      setSelectedOptionId('');
      setHandoffTarget('council_and_execution');
      setExecutionRiskLevel('medium');
      setSelectedHandoffTemplateId('council-and-execution');
      setHandoffTemplateName('');
      setHandoffTemplateStatus(null);
      setSelectedFormulaTemplateId('balanced');
      setFormulaTemplateName('');
      setFormulaTemplateStatus(null);
      setScoringFormula(defaultScoringFormula);
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  }

  async function handleCopyLocalSandboxReplayJson() {
    if (!selectedLocalSandboxReplayJson) {
      setLocalSandboxReplayActionState({
        tone: 'error',
        message: 'No replay JSON is available yet. Run or select a local sandbox replay first.',
      });
      return;
    }

    if (!navigator.clipboard?.writeText) {
      setLocalSandboxReplayActionState({
        tone: 'error',
        message:
          'Clipboard is unavailable in this environment. Open the replay JSON preview below and copy it manually from the <pre> block.',
      });
      return;
    }

    try {
      await navigator.clipboard.writeText(selectedLocalSandboxReplayJson);
      setLocalSandboxReplayActionState({
        tone: 'success',
        message: `Copied replay JSON for ${selectedLocalSandboxRun?.run_id ?? 'the selected run'} to the clipboard.`,
      });
    } catch (err) {
      setLocalSandboxReplayActionState({
        tone: 'error',
        message:
          err instanceof Error
            ? `Copy failed: ${err.message}. Open the replay JSON preview below and copy it manually from the <pre> block.`
            : 'Copy failed. Open the replay JSON preview below and copy it manually from the <pre> block.',
      });
    }
  }

  function handleExportLocalSandboxReplayJson() {
    if (!selectedLocalSandboxReplayJson || !selectedLocalSandboxRun) {
      setLocalSandboxReplayActionState({
        tone: 'error',
        message: 'No replay JSON is available yet. Run or select a local sandbox replay first.',
      });
      return;
    }

    try {
      const fileName = `${selectedLocalSandboxRun.run_id}-replay.json`;
      const blob = new Blob([selectedLocalSandboxReplayJson], { type: 'application/json' });
      const objectUrl = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.href = objectUrl;
      link.download = fileName;
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
      URL.revokeObjectURL(objectUrl);
      setLocalSandboxReplayActionState({
        tone: 'success',
        message: `Exported replay JSON for ${selectedLocalSandboxRun.run_id} to ${fileName}.`,
      });
    } catch (err) {
      setLocalSandboxReplayActionState({
        tone: 'error',
        message:
          err instanceof Error
            ? `Export failed: ${err.message}.`
            : 'Export failed for the selected replay JSON.',
      });
    }
  }

  function handleBuildSimulationCapabilityEvidence() {
    if (!selectedMissionId) {
      setSimulationEvidenceStatus({
        tone: 'error',
        message: 'Select a mission before building local simulation capability evidence JSON.',
      });
      return;
    }

    const bundle = buildSimulationCapabilityEvidenceBundle(
      selectedMissionId,
      simulationEvidenceTargetRemoteUserId,
      selectedSimulationRemoteUser,
      externalSaasHistory,
      highFidelityHistory,
    );
    setSimulationEvidenceJson(JSON.stringify(bundle, null, 2));
    setSimulationEvidenceStatus({
      tone: 'success',
      message:
        `Built evidence JSON for ${selectedMissionId} with ` +
        `${bundle.counts.external_saas_runs} external SaaS run(s) and ` +
        `${bundle.counts.high_fidelity_sandbox_runs} high-fidelity sandbox run(s).`,
    });
  }

  function handleDownloadSimulationCapabilityEvidence() {
    if (!simulationEvidenceJson) {
      setSimulationEvidenceStatus({
        tone: 'error',
        message: 'Build evidence JSON first so the local capability bundle can be downloaded.',
      });
      return;
    }

    try {
      downloadJsonFile(SIMULATION_CAPABILITY_EVIDENCE_FILENAME, simulationEvidenceJson);
      setSimulationEvidenceStatus({
        tone: 'success',
        message: `Downloaded ${SIMULATION_CAPABILITY_EVIDENCE_FILENAME}.`,
      });
    } catch (err) {
      setSimulationEvidenceStatus({
        tone: 'error',
        message:
          err instanceof Error
            ? `Evidence export failed: ${err.message}.`
            : 'Evidence export failed for the local simulation capability bundle.',
      });
    }
  }

  return (
    <div className="simulation-page">
      <section className="simulation-card simulation-card--hero">
        <div className="simulation-hero-copy">
          <span className="simulation-eyebrow">Simulation Sandbox</span>
          <h2>Mission Scenario Lab</h2>
          <p>
            Inject variables, compare options side by side, and save an explicit recommendation
            reason before a Mission leaves the sandbox.
          </p>
        </div>
        <div className="simulation-hero-strip">
          <div className="simulation-hero-pill">
            <strong>{activeVariables.length}</strong>
            <span>live variables</span>
          </div>
          <div className="simulation-hero-pill">
            <strong>{previewCards.length}</strong>
            <span>options scored</span>
          </div>
          <div className="simulation-hero-pill">
            <strong>{recommendedCard?.time_horizon ?? 'pending'}</strong>
            <span>selected horizon</span>
          </div>
        </div>
      </section>

      {error ? <div className="simulation-empty">{error}</div> : null}
      {loading ? <div className="simulation-empty">加载中...</div> : null}

      {!loading && !error ? (
        <>
          <div className="simulation-layout simulation-layout--editor">
            <section className="simulation-card simulation-card--sandbox">
              <div className="simulation-card-title-row">
                <div>
                  <h3>Scenario Runner</h3>
                  <p>
                    {selectedMission
                      ? `Working against "${selectedMission.title}" with a sandbox-first comparison flow.`
                      : 'Create or select a Mission to start building scenario runs.'}
                  </p>
                </div>
                <div className="simulation-score-badge">
                  {recommendedCard ? `${recommendedCard.score}/100` : 'Awaiting options'}
                </div>
              </div>

              {missions.length === 0 ? (
                <div className="simulation-empty">
                  当前没有可用 Mission，先在 Missions 页面创建一个任务。
                </div>
              ) : (
                <form className="simulation-form" onSubmit={handleSaveScenarioRun}>
                  <label className="simulation-field">
                    <span>Mission</span>
                    <select
                      value={selectedMissionId}
                      onChange={(event) => setSelectedMissionId(event.target.value)}
                    >
                      {missions.map((mission) => (
                        <option key={mission.id} value={mission.id}>
                          {mission.title}
                        </option>
                      ))}
                    </select>
                  </label>

                  <label className="simulation-field">
                    <span>Baseline</span>
                    <textarea
                      rows={4}
                      value={baseline}
                      onChange={(event) => setBaseline(event.target.value)}
                      placeholder="Describe the current state, constraints, and what must remain true."
                    />
                  </label>

                  <label className="simulation-field">
                    <span>Options</span>
                    <textarea
                      rows={5}
                      value={optionsText}
                      onChange={(event) => setOptionsText(event.target.value)}
                      placeholder={'One option per line\nIncrease partner budget by 10%\nDelay launch by 2 weeks\nRun a limited pilot first'}
                    />
                    <small>Each line becomes a scored comparison card.</small>
                  </label>

                  <section className="simulation-editor-panel">
                    <div className="simulation-section-heading">
                      <div>
                        <h4>Variable Injection Lab</h4>
                        <p>Model the deltas you want to push into the sandbox before scoring options.</p>
                      </div>
                      <button
                        className="simulation-secondary-button"
                        type="button"
                        onClick={handleAddVariable}
                      >
                        Add variable
                      </button>
                    </div>

                    <div className="simulation-variable-list">
                      {variables.map((variable, index) => (
                        <article className="simulation-variable-card" key={variable.id}>
                          <div className="simulation-variable-card-header">
                            <strong>Variable {index + 1}</strong>
                            <button
                              className="simulation-text-button"
                              type="button"
                              onClick={() => handleRemoveVariable(variable.id)}
                            >
                              Remove
                            </button>
                          </div>
                          <div className="simulation-variable-grid">
                            <label className="simulation-field">
                              <span>Label</span>
                              <input
                                type="text"
                                value={variable.label}
                                onChange={(event) =>
                                  handleUpdateVariable(variable.id, 'label', event.target.value)
                                }
                                placeholder="Budget, timeline, staffing..."
                              />
                            </label>
                            <label className="simulation-field">
                              <span>Current value</span>
                              <input
                                type="text"
                                value={variable.current_value}
                                onChange={(event) =>
                                  handleUpdateVariable(
                                    variable.id,
                                    'current_value',
                                    event.target.value,
                                  )
                                }
                                placeholder="Current state"
                              />
                            </label>
                            <label className="simulation-field">
                              <span>Proposed value</span>
                              <input
                                type="text"
                                value={variable.proposed_value}
                                onChange={(event) =>
                                  handleUpdateVariable(
                                    variable.id,
                                    'proposed_value',
                                    event.target.value,
                                  )
                                }
                                placeholder="Target sandbox value"
                              />
                            </label>
                            <label className="simulation-field simulation-slider-field">
                              <span>Impact · {formatWeightLabel(variable.impact_weight)}</span>
                              <input
                                type="range"
                                min="0"
                                max="100"
                                step="1"
                                value={variable.impact_weight}
                                onChange={(event) =>
                                  handleUpdateVariable(
                                    variable.id,
                                    'impact_weight',
                                    Number(event.target.value),
                                  )
                                }
                              />
                              <small>{Math.round(variable.impact_weight)} / 100 influence on option scoring</small>
                            </label>
                            <label className="simulation-field simulation-slider-field">
                              <span>Uncertainty · {formatWeightLabel(variable.uncertainty_weight)}</span>
                              <input
                                type="range"
                                min="0"
                                max="100"
                                step="1"
                                value={variable.uncertainty_weight}
                                onChange={(event) =>
                                  handleUpdateVariable(
                                    variable.id,
                                    'uncertainty_weight',
                                    Number(event.target.value),
                                  )
                                }
                              />
                              <small>{Math.round(variable.uncertainty_weight)} / 100 penalty against brittle paths</small>
                            </label>
                          </div>
                        </article>
                      ))}
                    </div>
                  </section>

                  <section className="simulation-editor-panel simulation-editor-panel--formula">
                    <div className="simulation-section-heading">
                      <div>
                        <h4>Scoring Formula</h4>
                        <p>Tune the local scoring template before option cards are generated and persisted.</p>
                      </div>
                      <button
                        className="simulation-secondary-button"
                        type="button"
                        onClick={handleResetFormula}
                      >
                        Reset formula
                      </button>
                    </div>
                    <div className="simulation-policy-template-row simulation-formula-template-row">
                      <label className="simulation-field">
                        <span>Formula template</span>
                        <select
                          value={selectedFormulaTemplateId}
                          onChange={(event) => handleApplyFormulaTemplate(event.target.value)}
                        >
                          {scoringFormulaTemplates.map((template) => (
                            <option key={template.id} value={template.id}>
                              {template.name}
                            </option>
                          ))}
                        </select>
                        <small>
                          {scoringFormulaTemplates.find((template) => template.id === selectedFormulaTemplateId)
                            ?.description ?? 'Choose a saved scoring formula.'}
                        </small>
                      </label>
                      <label className="simulation-field">
                        <span>Save current formula as</span>
                        <input
                          type="text"
                          value={formulaTemplateName}
                          onChange={(event) => setFormulaTemplateName(event.target.value)}
                          placeholder="My scoring formula"
                        />
                        <button className="simulation-secondary-button" type="button" onClick={handleSaveFormulaTemplate}>
                          Save formula
                        </button>
                        {formulaTemplateStatus ? <small>{formulaTemplateStatus}</small> : null}
                      </label>
                    </div>
                    <div className="simulation-formula-grid">
                      <label className="simulation-field simulation-slider-field">
                        <span>Base score · {scoringFormula.baseScore}</span>
                        <input
                          type="range"
                          min="20"
                          max="80"
                          step="1"
                          value={scoringFormula.baseScore}
                          onChange={(event) =>
                            setScoringFormula((current) => ({
                              ...current,
                              baseScore: Number(event.target.value),
                            }))
                          }
                        />
                        <small>Starting point before strategy, evidence, risk, and variable adjustments.</small>
                      </label>
                      <label className="simulation-field simulation-slider-field">
                        <span>Impact multiplier · {scoringFormula.impactMultiplier}</span>
                        <input
                          type="range"
                          min="0"
                          max="30"
                          step="1"
                          value={scoringFormula.impactMultiplier}
                          onChange={(event) =>
                            setScoringFormula((current) => ({
                              ...current,
                              impactMultiplier: Number(event.target.value),
                            }))
                          }
                        />
                        <small>How strongly high-impact variables lift assertive options.</small>
                      </label>
                      <label className="simulation-field simulation-slider-field">
                        <span>Uncertainty penalty · {scoringFormula.uncertaintyPenalty}</span>
                        <input
                          type="range"
                          min="0"
                          max="30"
                          step="1"
                          value={scoringFormula.uncertaintyPenalty}
                          onChange={(event) =>
                            setScoringFormula((current) => ({
                              ...current,
                              uncertaintyPenalty: Number(event.target.value),
                            }))
                          }
                        />
                        <small>How heavily high uncertainty suppresses confidence and score.</small>
                      </label>
                    </div>
                  </section>

                  <section className="simulation-editor-panel">
                    <div className="simulation-section-heading">
                      <div>
                        <h4>Option Arena</h4>
                        <p>Score each path against the injected variables and nominate the winner.</p>
                      </div>
                    </div>

                    {previewCards.length === 0 ? (
                      <div className="simulation-empty simulation-empty--subtle">
                        Add at least one option to generate comparison cards.
                      </div>
                    ) : (
                      <div className="simulation-option-grid">
                        {previewCards.map((option) => (
                          <OptionComparisonCard
                            key={option.id}
                            option={option}
                            selected={option.id === (recommendedCard?.id ?? '')}
                            selectionName="selected-scenario-option"
                            onSelect={() => setSelectedOptionId(option.id)}
                          />
                        ))}
                      </div>
                    )}
                  </section>

                  <div className="simulation-notes-grid">
                    <label className="simulation-field">
                      <span>Comparison summary</span>
                      <textarea
                        rows={4}
                        value={comparisonSummary}
                        onChange={(event) => setComparisonSummary(event.target.value)}
                        placeholder="Summarize how the winner compares with the main alternatives."
                      />
                      <small>{comparisonSummary.trim() ? 'Using custom summary.' : comparisonDraft}</small>
                    </label>

                    <label className="simulation-field">
                      <span>Recommendation reason</span>
                      <textarea
                        rows={4}
                        value={recommendation}
                        onChange={(event) => setRecommendation(event.target.value)}
                        placeholder="State why the selected option is recommended."
                      />
                      <small>
                        {recommendation.trim() ? 'Using custom recommendation.' : recommendationDraft}
                      </small>
                    </label>
                  </div>

                  <section className="simulation-editor-panel simulation-editor-panel--share">
                    <div className="simulation-section-heading">
                      <div>
                        <h4>Template Sharing Bundle</h4>
                        <p>Export or import local handoff policy and scoring formula templates as portable JSON.</p>
                      </div>
                      <button className="simulation-secondary-button" type="button" onClick={handleExportTemplateBundle}>
                        Export bundle
                      </button>
                      <button className="simulation-secondary-button" type="button" onClick={handleExportTemplateBundleAuditLog}>
                        Export audit JSON
                      </button>
                    </div>
                    <label className="simulation-field">
                      <span>Template bundle JSON</span>
                      <textarea
                        rows={5}
                        value={templateBundleText}
                        onChange={(event) => setTemplateBundleText(event.target.value)}
                        placeholder="Paste a simulation template bundle JSON here to import shared policy/formula templates."
                      />
                      <small>
                        This is a manual team-sharing bridge; remote sync and RBAC still require a team service.
                      </small>
                    </label>
                    <div className="simulation-template-bundle-actions">
                      <button className="simulation-secondary-button" type="button" onClick={handlePreflightTemplateBundle}>
                        Preflight import
                      </button>
                      <button className="simulation-secondary-button" type="button" onClick={handleImportTemplateBundle}>
                        Import bundle
                      </button>
                      {templateBundleStatus ? <small>{templateBundleStatus}</small> : null}
                    </div>
                    {templateBundlePreflight ? (
                      <div className="simulation-template-bundle-preflight">
                        <strong>Import preflight</strong>
                        <span>Total templates: {templateBundlePreflight.total_count}</span>
                        <span>
                          Policies: +{templateBundlePreflight.handoff_policy_templates.create_count} / update {templateBundlePreflight.handoff_policy_templates.update_count} / unchanged {templateBundlePreflight.handoff_policy_templates.unchanged_count}
                        </span>
                        <span>
                          Formulas: +{templateBundlePreflight.scoring_formula_templates.create_count} / update {templateBundlePreflight.scoring_formula_templates.update_count} / unchanged {templateBundlePreflight.scoring_formula_templates.unchanged_count}
                        </span>
                        {templateBundlePreflight.conflicts.length > 0 ? (
                          <div className="simulation-template-bundle-conflicts">
                            {templateBundlePreflight.conflicts.map((conflict) => (
                              <span key={`${conflict.template_type}:${conflict.id}`}>
                                {conflict.template_type} {conflict.id}: {conflict.existing_name} → {conflict.incoming_name}
                              </span>
                            ))}
                          </div>
                        ) : null}
                      </div>
                    ) : null}
                    {templateBundleAuditExport ? (
                      <div className="simulation-template-bundle-export">
                        <strong>Audit export preview</strong>
                        <span>{templateBundleAuditExport.exported_count}/{templateBundleAuditExport.total} events included · read-only local export</span>
                        <pre>{JSON.stringify(templateBundleAuditExport, null, 2)}</pre>
                      </div>
                    ) : null}
                    <div className="simulation-template-bundle-audit">
                      <strong>Local bundle audit</strong>
                      {templateBundleAuditLog.length === 0 ? <span>No local bundle events yet.</span> : null}
                      {templateBundleAuditLog.slice(0, 5).map((entry) => (
                        <div className="simulation-template-bundle-audit-row" key={entry.id}>
                          <span>{entry.action}</span>
                          <p>
                            {entry.handoff_policy_template_count} policies · {entry.scoring_formula_template_count} formulas · {entry.actor} · {entry.occurred_at}
                          </p>
                        </div>
                      ))}
                    </div>
                  </section>

                  <section className="simulation-editor-panel simulation-editor-panel--sandbox">
                    <div className="simulation-section-heading">
                      <div>
                        <h4>Local Multi-Agent Sandbox</h4>
                        <p>Run a deterministic built-in sandbox across options, agents, and rounds. It persists a completed simulation run and audit event locally.</p>
                      </div>
                    </div>
                    <div className="simulation-provider-status-grid">
                      <article className="simulation-provider-status simulation-provider-status--active">
                        <span>Provider status</span>
                        <strong>Local provider enabled</strong>
                        <p>
                          This UI is connected to the deterministic built-in provider and can run,
                          persist, and replay local sandbox sessions immediately.
                        </p>
                      </article>
                      <article className="simulation-provider-status simulation-provider-status--inactive">
                        <span>Provider status</span>
                        <strong>External provider adapter available with explicit safeguards</strong>
                        <p>
                          local_echo runs fully offline; http_json can call a real endpoint only when the
                          backend confirmation phrase is supplied.
                        </p>
                      </article>
                    </div>
                    <div className="simulation-policy-template-row">
                      <label className="simulation-field">
                        <span>Agents</span>
                        <textarea
                          rows={4}
                          value={localSandboxAgentsText}
                          onChange={(event) => setLocalSandboxAgentsText(event.target.value)}
                          placeholder="role|stance|name, one agent per line"
                        />
                        <small>Format: role|stance|name. Empty input uses built-in strategy/risk/ops agents.</small>
                      </label>
                      <label className="simulation-field">
                        <span>Rounds</span>
                        <input
                          type="number"
                          min={1}
                          max={12}
                          value={localSandboxRounds}
                          onChange={(event) => setLocalSandboxRounds(Number(event.target.value) || 1)}
                        />
                        <button
                          className="simulation-secondary-button"
                          type="button"
                          onClick={handleRunLocalSandbox}
                          disabled={localSandboxLoading}
                        >
                          {localSandboxLoading ? 'Running sandbox...' : 'Run local sandbox'}
                        </button>
                      </label>
                    </div>
                    {localSandboxStatus ? (
                      <div
                        className={`simulation-local-sandbox-feedback simulation-local-sandbox-feedback--${getLocalSandboxStatusTone(localSandboxStatus)}`}
                      >
                        {localSandboxStatus}
                      </div>
                    ) : null}
                    {localSandboxResult ? (
                      <div className="simulation-local-sandbox-result">
                        <strong>{localSandboxResult.engine}</strong>
                        <span>{localSandboxResult.rounds} rounds · {localSandboxResult.agents.length} agents · {localSandboxResult.turns.length} turns</span>
                        <p>{localSandboxResult.recommendation.rationale}</p>
                        <div className="simulation-local-sandbox-scores">
                          {localSandboxResult.option_scores.map((score) => (
                            <span key={score.option}>
                              {score.option}: {score.average_score.toFixed(1)} / 100
                            </span>
                          ))}
                        </div>
                      </div>
                    ) : null}
                    <div className="simulation-extension-grid">
                      <section className="simulation-extension-card">
                        <div className="simulation-section-heading">
                          <div>
                            <h4>External SaaS Simulation Adapter</h4>
                            <p>Provider adapter supports local_echo and confirmed http_json calls; dry-run previews never call external SaaS.</p>
                          </div>
                        </div>
                        <div className="simulation-field-grid">
                          <label className="simulation-field">
                            <span>Provider</span>
                            <select value={externalSaasProvider} onChange={(event) => setExternalSaasProvider(event.target.value)}>
                              <option value="local_echo">local_echo</option>
                              <option value="http_json">http_json</option>
                            </select>
                          </label>
                          <label className="simulation-field">
                            <span>Endpoint URL</span>
                            <input value={externalSaasEndpoint} onChange={(event) => setExternalSaasEndpoint(event.target.value)} disabled={externalSaasProvider !== 'http_json'} />
                            <small>Only used by http_json non-dry-run; backend requires phrase RUN EXTERNAL SAAS SIMULATION.</small>
                          </label>
                        </div>
                        <label className="simulation-field">
                          <span>Input JSON</span>
                          <textarea rows={5} value={externalSaasInputJson} onChange={(event) => setExternalSaasInputJson(event.target.value)} />
                        </label>
                        <label className="simulation-field">
                          <span>Confirmation phrase</span>
                          <input value={externalSaasConfirmPhrase} onChange={(event) => setExternalSaasConfirmPhrase(event.target.value)} placeholder="RUN EXTERNAL SAAS SIMULATION" />
                        </label>
                        <div className="simulation-inline-actions">
                          <button className="simulation-secondary-button" type="button" onClick={() => void handleRunExternalSaas(true)} disabled={localSandboxLoading}>Dry-run provider</button>
                          <button className="simulation-secondary-button" type="button" onClick={() => void handleRunExternalSaas(false)} disabled={localSandboxLoading}>Run provider</button>
                        </div>
                        {externalSaasResult ? (
                          <pre className="simulation-json-preview">{JSON.stringify(externalSaasResult, null, 2)}</pre>
                        ) : null}
                        <div className="simulation-local-sandbox-history">
                          <strong>Recent external SaaS runs</strong>
                          {!selectedMissionId ? (
                            <div className="simulation-empty simulation-empty--subtle">
                              Select a mission to load recent external SaaS runs.
                            </div>
                          ) : externalSaasHistoryError ? (
                            <div className="simulation-inline-error">
                              Unable to load recent external SaaS runs: {externalSaasHistoryError}
                            </div>
                          ) : externalSaasHistory.length === 0 ? (
                            <div className="simulation-empty simulation-empty--subtle">
                              No external SaaS runs recorded for this mission yet.
                            </div>
                          ) : (
                            <div className="simulation-local-sandbox-history-list">
                              {externalSaasHistory.map((run) => (
                                <div className="simulation-local-sandbox-history-row" key={run.run_id}>
                                  <div className="simulation-local-sandbox-history-row-main">
                                    <span>{run.provider}</span>
                                    <small>
                                      {formatOptionalTimestamp(run.created_at, run.run_id)} ·{' '}
                                      {getExternalSaasRunStatus(run)} ·{' '}
                                      {run.network_invocation ? 'network' : 'offline'}
                                      {run.target_remote_user_id ? ` · target ${run.target_remote_user_id}` : ''}
                                    </small>
                                    <small>{run.summary}</small>
                                  </div>
                                  <strong>{formatExternalSaasRunOutcome(run)}</strong>
                                </div>
                              ))}
                            </div>
                          )}
                        </div>
                      </section>

                      <section className="simulation-extension-card">
                        <div className="simulation-section-heading">
                          <div>
                            <h4>High-Fidelity Local Sandbox</h4>
                            <p>Builds a deterministic world model with entities, variables, event graph, timeline, and option metric heatmap.</p>
                          </div>
                        </div>
                        <button className="simulation-secondary-button" type="button" onClick={() => void handleRunHighFidelitySandbox()} disabled={localSandboxLoading}>
                          Run high-fidelity sandbox
                        </button>
                        {highFidelityResult ? (
                          <div className="simulation-local-sandbox-result">
                            <strong>{highFidelityResult.engine}</strong>
                            <span>{highFidelityResult.world.entities.length} entities · {highFidelityResult.world.timeline.length} timeline events · {highFidelityResult.world.event_graph.edges.length} graph edges</span>
                            <p>{highFidelityResult.summary}</p>
                            <pre className="simulation-json-preview">{JSON.stringify(highFidelityResult.world, null, 2)}</pre>
                          </div>
                        ) : null}
                        <div className="simulation-local-sandbox-history">
                          <strong>Recent high-fidelity sandbox runs</strong>
                          {!selectedMissionId ? (
                            <div className="simulation-empty simulation-empty--subtle">
                              Select a mission to load recent high-fidelity sandbox runs.
                            </div>
                          ) : highFidelityHistoryError ? (
                            <div className="simulation-inline-error">
                              Unable to load recent high-fidelity sandbox runs: {highFidelityHistoryError}
                            </div>
                          ) : highFidelityHistory.length === 0 ? (
                            <div className="simulation-empty simulation-empty--subtle">
                              No high-fidelity sandbox runs recorded for this mission yet.
                            </div>
                          ) : (
                            <div className="simulation-local-sandbox-history-list">
                              {highFidelityHistory.map((run) => (
                                <div className="simulation-local-sandbox-history-row" key={run.run_id}>
                                  <div className="simulation-local-sandbox-history-row-main">
                                    <span>{run.base_run.recommendation.option}</span>
                                    <small>
                                      {formatOptionalTimestamp(run.created_at, run.run_id)} ·{' '}
                                      {getHighFidelitySandboxRunStatus(run)} ·{' '}
                                      {run.world.timeline.length} events
                                      {run.target_remote_user_id ? ` · target ${run.target_remote_user_id}` : ''}
                                    </small>
                                    <small>{run.summary}</small>
                                  </div>
                                  <strong>{run.base_run.recommendation.average_score.toFixed(1)}</strong>
                                </div>
                              ))}
                            </div>
                          )}
                        </div>
                      </section>

                      <section className="simulation-extension-card simulation-extension-card--wide">
                        <div className="simulation-section-heading">
                          <div>
                            <h4>Simulation Capability Evidence</h4>
                            <p>
                              Build a local evidence bundle from the currently loaded external SaaS
                              and high-fidelity histories for this mission.
                            </p>
                          </div>
                        </div>
                        <div className="simulation-evidence-summary">
                          <span>
                            Mission <code>{selectedMissionId || 'none selected'}</code>
                          </span>
                          <span>{externalSaasHistory.length} external SaaS run(s) loaded</span>
                          <span>{highFidelityHistory.length} high-fidelity sandbox run(s) loaded</span>
                        </div>
                        <label className="simulation-field">
                          <span>Target remote user id</span>
                          <input
                            value={simulationEvidenceTargetRemoteUserId}
                            onChange={(event) => setSimulationEvidenceTargetRemoteUserId(event.target.value)}
                            placeholder="Optional future remote user"
                          />
                          <small>
                            Routing metadata for a future handoff only; this does not prove remote delivery.
                          </small>
                        </label>
                        <label className="simulation-field">
                          <span>Fill from local Agent Exchange future remote user</span>
                          <select
                            value={selectedSimulationRemoteUser?.user_id ?? ''}
                            onChange={(event) => setSimulationEvidenceTargetRemoteUserId(event.target.value)}
                            disabled={agentExchangeRemoteUsersLoading || agentExchangeRemoteUsers.length === 0}
                          >
                            <option value="">
                              {agentExchangeRemoteUsersLoading
                                ? 'Loading active local Agent Exchange users...'
                                : 'Choose active local Agent Exchange user'}
                            </option>
                            {agentExchangeRemoteUsers.map((remoteUser) => (
                              <option key={remoteUser.user_id} value={remoteUser.user_id}>
                                {formatRemoteUserOptionLabel(remoteUser)}
                              </option>
                            ))}
                          </select>
                          <small>
                            {agentExchangeRemoteUsersError ??
                              `${agentExchangeRemoteUsers.length} active local Agent Exchange future remote user(s) available; selection only fills local routing metadata and does not imply remote delivery.`}
                          </small>
                        </label>
                        <div className="simulation-inline-actions">
                          <button
                            className="simulation-secondary-button"
                            type="button"
                            onClick={handleBuildSimulationCapabilityEvidence}
                          >
                            {simulationEvidenceJson ? 'Refresh evidence JSON' : 'Build evidence JSON'}
                          </button>
                          <button
                            className="simulation-secondary-button"
                            type="button"
                            onClick={handleDownloadSimulationCapabilityEvidence}
                          >
                            Download evidence JSON
                          </button>
                        </div>
                        <div className="simulation-evidence-notes">
                          <strong>Boundary notes</strong>
                          <ul>
                            {SIMULATION_CAPABILITY_EVIDENCE_BOUNDARY_NOTES.map((note) => (
                              <li key={note}>{note}</li>
                            ))}
                          </ul>
                        </div>
                        {simulationEvidenceStatus ? (
                          <div
                            className={`simulation-local-sandbox-feedback simulation-local-sandbox-feedback--${simulationEvidenceStatus.tone}`}
                          >
                            {simulationEvidenceStatus.message}
                          </div>
                        ) : null}
                        {simulationEvidenceJson ? (
                          <div className="simulation-local-sandbox-result">
                            <strong>{SIMULATION_CAPABILITY_EVIDENCE_FILENAME}</strong>
                            <span>Preview of the current local capability evidence bundle.</span>
                            <pre className="simulation-json-preview">{simulationEvidenceJson}</pre>
                          </div>
                        ) : (
                          <div className="simulation-empty simulation-empty--subtle">
                            Build evidence JSON to preview and export the current local capability bundle.
                          </div>
                        )}
                      </section>
                    </div>

                    <div className="simulation-local-sandbox-history">
                      <strong>Recent local sandbox runs</strong>
                      {!selectedMissionId ? (
                        <div className="simulation-empty simulation-empty--subtle">
                          Select a mission to load local sandbox history and replay details.
                        </div>
                      ) : localSandboxHistoryError ? (
                        <div className="simulation-inline-error">
                          Unable to load recent local sandbox history: {localSandboxHistoryError}
                        </div>
                      ) : localSandboxHistory.length === 0 ? (
                        <div className="simulation-empty simulation-empty--subtle">
                          No completed local sandbox runs yet. Use the built-in provider above to create the
                          first replayable run for this mission.
                        </div>
                      ) : (
                        <div className="simulation-local-sandbox-history-list">
                          {localSandboxHistory.map((run) => {
                            const selected = run.run_id === selectedLocalSandboxRun?.run_id;
                            return (
                              <button
                                className={`simulation-local-sandbox-history-row${selected ? ' simulation-local-sandbox-history-row--selected' : ''}`}
                                key={run.run_id}
                                type="button"
                                onClick={() => setSelectedLocalSandboxRunId(run.run_id)}
                              >
                                <div className="simulation-local-sandbox-history-row-main">
                                  <span>{run.recommendation.option}</span>
                                  <small>
                                    {run.rounds} rounds · {run.agents.length} agents · {run.turns.length} turns
                                  </small>
                                  <small>{run.run_id}</small>
                                </div>
                                <strong>{run.recommendation.average_score.toFixed(1)}</strong>
                              </button>
                            );
                          })}
                        </div>
                      )}
                    </div>
                    <div className="simulation-local-sandbox-replay">
                      <div className="simulation-section-heading">
                        <div>
                          <h4>Replay details</h4>
                          <p>
                            Review the selected run across agents, rounds, turn-by-turn scoring, and the
                            final recommendation.
                          </p>
                        </div>
                      </div>
                      {!selectedMissionId ? (
                        <div className="simulation-empty simulation-empty--subtle">
                          Choose a mission before opening replay details.
                        </div>
                      ) : localSandboxHistoryError && !selectedLocalSandboxRun ? (
                        <div className="simulation-inline-error">
                          History load failed, so replay details need a successful local sandbox history response.
                        </div>
                      ) : !selectedLocalSandboxRun ? (
                        <div className="simulation-empty simulation-empty--subtle">
                          Run the local sandbox once to unlock per-run playback, score history, and
                          recommendation rationale.
                        </div>
                      ) : (
                        <>
                          <div className="simulation-local-sandbox-replay-actions">
                            <button
                              className="simulation-secondary-button"
                              type="button"
                              onClick={handleCopyLocalSandboxReplayJson}
                            >
                              Copy JSON
                            </button>
                            <button
                              className="simulation-secondary-button"
                              type="button"
                              onClick={handleExportLocalSandboxReplayJson}
                            >
                              Export JSON
                            </button>
                          </div>
                          {localSandboxReplayActionState ? (
                            <div
                              className={`simulation-local-sandbox-feedback simulation-local-sandbox-feedback--${localSandboxReplayActionState.tone}`}
                            >
                              {localSandboxReplayActionState.message}
                            </div>
                          ) : null}
                          <div className="simulation-local-sandbox-replay-header">
                            <div>
                              <strong>{selectedLocalSandboxRun.recommendation.option}</strong>
                              <p>{selectedLocalSandboxRun.recommendation.rationale}</p>
                            </div>
                            <span>{selectedLocalSandboxRun.run_id}</span>
                          </div>
                          <div className="simulation-local-sandbox-replay-metrics">
                            <div className="simulation-local-sandbox-replay-metric">
                              <span>Engine</span>
                              <strong>{selectedLocalSandboxRun.engine}</strong>
                            </div>
                            <div className="simulation-local-sandbox-replay-metric">
                              <span>Recommendation</span>
                              <strong>{selectedLocalSandboxRun.recommendation.average_score.toFixed(1)} avg</strong>
                            </div>
                            <div className="simulation-local-sandbox-replay-metric">
                              <span>Rounds</span>
                              <strong>{selectedLocalSandboxRun.rounds}</strong>
                            </div>
                            <div className="simulation-local-sandbox-replay-metric">
                              <span>Total turns</span>
                              <strong>{selectedLocalSandboxRun.turns.length}</strong>
                            </div>
                          </div>
                          <div className="simulation-local-sandbox-replay-agents">
                            <span>Agents</span>
                            <div className="simulation-local-sandbox-agent-list">
                              {selectedLocalSandboxRun.agents.map((agent) => (
                                <div className="simulation-local-sandbox-agent-chip" key={`${agent.role}:${agent.name}`}>
                                  <strong>{agent.name}</strong>
                                  <small>
                                    {agent.role} · {agent.stance}
                                  </small>
                                </div>
                              ))}
                            </div>
                          </div>
                          <div className="simulation-local-sandbox-replay-scores">
                            <span>Scoreboard</span>
                            <div className="simulation-local-sandbox-scores">
                              {selectedLocalSandboxRun.option_scores.map((score) => (
                                <span key={score.option}>
                                  {score.option}: {score.average_score.toFixed(1)} avg · {score.total_score.toFixed(1)} total · {score.turn_count} turns
                                </span>
                              ))}
                            </div>
                          </div>
                          <details className="simulation-local-sandbox-json-preview">
                            <summary>Show replay JSON preview</summary>
                            <p>
                              The preview contains the exact replay JSON used by Copy JSON and Export
                              JSON. If clipboard access is unavailable, copy from the{' '}
                              <code>&lt;pre&gt;</code> block below.
                            </p>
                            <pre>{selectedLocalSandboxReplayJson}</pre>
                          </details>
                          <div className="simulation-local-sandbox-turn-list">
                            {selectedLocalSandboxRun.turns.map((turn, index) => (
                              <article
                                className="simulation-local-sandbox-turn-card"
                                key={`${selectedLocalSandboxRun.run_id}:${index}:${turn.agent_name}:${turn.option}`}
                              >
                                <div className="simulation-local-sandbox-turn-header">
                                  <div>
                                    <strong>Turn {index + 1}</strong>
                                    <small>
                                      Round {turn.round} · {turn.option}
                                    </small>
                                  </div>
                                  <span>{turn.score.toFixed(1)}</span>
                                </div>
                                <div className="simulation-local-sandbox-turn-agent">
                                  <span>{turn.agent_name}</span>
                                  <small>
                                    {turn.agent_role} · {turn.agent_stance}
                                  </small>
                                </div>
                                <p>{turn.rationale}</p>
                              </article>
                            ))}
                          </div>
                        </>
                      )}
                    </div>
                  </section>

                  <section className="simulation-editor-panel simulation-editor-panel--handoff">
                    <div className="simulation-section-heading">
                      <div>
                        <h4>Governance Handoff Policy</h4>
                        <p>Choose how this scenario should enter Council and Execution review after saving.</p>
                      </div>
                    </div>
                    <div className="simulation-policy-template-row">
                      <label className="simulation-field">
                        <span>Policy template</span>
                        <select
                          value={selectedHandoffTemplateId}
                          onChange={(event) => handleApplyHandoffTemplate(event.target.value)}
                        >
                          {handoffPolicyTemplates.map((template) => (
                            <option key={template.id} value={template.id}>
                              {template.name}
                            </option>
                          ))}
                        </select>
                        <small>
                          {handoffPolicyTemplates.find((template) => template.id === selectedHandoffTemplateId)
                            ?.description ?? 'Choose a saved handoff policy.'}
                        </small>
                      </label>
                      <label className="simulation-field">
                        <span>Save current policy as</span>
                        <input
                          type="text"
                          value={handoffTemplateName}
                          onChange={(event) => setHandoffTemplateName(event.target.value)}
                          placeholder="My approval policy"
                        />
                        <button className="simulation-secondary-button" type="button" onClick={handleSaveHandoffTemplate}>
                          Save template
                        </button>
                        {handoffTemplateStatus ? <small>{handoffTemplateStatus}</small> : null}
                      </label>
                    </div>
                    <div className="simulation-policy-grid">
                      <label className="simulation-field">
                        <span>Handoff route</span>
                        <select
                          value={handoffTarget}
                          onChange={(event) => setHandoffTarget(event.target.value)}
                        >
                          {handoffTargetOptions.map((option) => (
                            <option key={option.value} value={option.value}>
                              {option.label}
                            </option>
                          ))}
                        </select>
                        <small>{describeHandoffTarget(handoffTarget)}</small>
                      </label>
                      <label className="simulation-field">
                        <span>Execution review risk</span>
                        <select
                          value={executionRiskLevel}
                          onChange={(event) => setExecutionRiskLevel(event.target.value)}
                          disabled={handoffTarget === 'council_only' || handoffTarget === 'timeline_only'}
                        >
                          {executionRiskOptions.map((option) => (
                            <option key={option} value={option}>
                              {option}
                            </option>
                          ))}
                        </select>
                        <small>
                          High risk creates an awaiting-approval Execution step; low/medium create pending review.
                        </small>
                      </label>
                    </div>
                  </section>

                  <div className="simulation-recommendation-card">
                    <span>Recommendation explanation</span>
                    <strong>{recommendedCard?.label ?? 'Select an option to generate a recommendation.'}</strong>
                    <p>{recommendation.trim() || recommendationDraft || 'The explanation appears once options are scored.'}</p>
                  </div>

                  <div className="simulation-handoff-callout">
                    <strong>Automatic handoff</strong>
                    <span>
                      Saving creates a completed Simulation run, a timeline event, a Scenario Reviewer
                      {describeHandoffOutcome(handoffTarget, executionRiskLevel)}
                    </span>
                  </div>

                  <div className="simulation-sandbox-strip">
                    <div className="simulation-sandbox-stat">
                      <span>High uncertainty</span>
                      <strong>{activeVariables.filter((variable) => variable.uncertainty === 'high').length}</strong>
                    </div>
                    <div className="simulation-sandbox-stat">
                      <span>Selected option</span>
                      <strong>{recommendedCard?.label ?? 'None yet'}</strong>
                    </div>
                    <div className="simulation-sandbox-stat">
                      <span>Score edge</span>
                      <strong>{getScoreEdge(previewCards, recommendedCard?.id ?? null)}</strong>
                    </div>
                  </div>

                  {formError ? <div className="simulation-inline-error">{formError}</div> : null}

                  <div className="simulation-actions">
                    <button type="submit" disabled={saving}>
                      {saving ? 'Saving...' : 'Save / Run Scenario'}
                    </button>
                  </div>
                </form>
              )}
            </section>

            <section className="simulation-card simulation-card--history">
              <div className="simulation-card-title-row">
                <div>
                  <h3>Recent Scenarios</h3>
                  <p>Saved runs keep the sandbox inputs, comparison summary, and final recommendation.</p>
                </div>
              </div>

              {scenarioRunsError ? <div className="simulation-empty">{scenarioRunsError}</div> : null}
              {scenarioRunsLoading ? <div className="simulation-empty">加载 scenario runs...</div> : null}
              {!scenarioRunsLoading && !scenarioRunsError && scenarioRuns.length === 0 ? (
                <div className="simulation-empty">这个 Mission 还没有保存过 scenario runs。</div>
              ) : null}

              {!scenarioRunsLoading && scenarioRuns.length > 0 ? (
                <div className="simulation-run-list">
                  {scenarioRuns.map((run) => {
                    const runVariables = sanitizeVariables(run.variables ?? []);
                    const runOptionCards =
                      run.option_cards && run.option_cards.length > 0
                        ? run.option_cards
                        : buildOptionCards(run.options, runVariables, defaultScoringFormula);
                    const runSelectedOption =
                      run.selected_option_id ?? getTopOptionId(runOptionCards) ?? null;
                    const runComparisonSummary =
                      run.comparison_summary ?? buildComparisonSummary(runOptionCards, runSelectedOption);
                    const runRecommendation =
                      run.recommendation_reason ?? buildRecommendationReason(runOptionCards, runSelectedOption);
                    const runHandoffTarget = run.handoff_target ?? 'council_and_execution';
                    const runExecutionRisk = run.execution_risk_level ?? 'medium';

                    return (
                      <article className="simulation-run-item" key={run.id}>
                        <div className="simulation-run-title-row">
                          <strong>{run.mission_title}</strong>
                          <span>{formatTimestamp(run.created_at)}</span>
                        </div>
                        <div className="simulation-run-summary">{run.baseline}</div>

                        {runVariables.length > 0 ? (
                          <div className="simulation-variable-chip-row">
                            {runVariables.map((variable) => (
                              <div className="simulation-variable-chip" key={variable.id}>
                                <strong>{variable.label || 'Unnamed'}</strong>
                                <span>{formatVariableShift(variable)}</span>
                                <small>
                                  {formatWeightLabel(variable.impact_weight)} impact / {formatWeightLabel(variable.uncertainty_weight)} uncertainty
                                </small>
                              </div>
                            ))}
                          </div>
                        ) : null}

                        <div className="simulation-run-insight-grid">
                          <div className="simulation-run-insight">
                            <span>Comparison summary</span>
                            <p>{runComparisonSummary || 'No comparison summary recorded.'}</p>
                          </div>
                          <div className="simulation-run-insight">
                            <span>Recommendation reason</span>
                            <p>{runRecommendation || 'No recommendation reason recorded.'}</p>
                          </div>
                          <div className="simulation-run-insight simulation-run-insight--handoff">
                            <span>Governance handoff</span>
                            <p>{describeHandoffOutcome(runHandoffTarget, runExecutionRisk)}</p>
                          </div>
                        </div>

                        <div className="simulation-option-grid simulation-option-grid--history">
                          {runOptionCards.map((option) => (
                            <OptionComparisonCard
                              key={option.id}
                              option={option}
                              selected={option.id === runSelectedOption}
                            />
                          ))}
                        </div>
                      </article>
                    );
                  })}
                </div>
              ) : null}
            </section>
          </div>

          <section className="simulation-card simulation-card--comparison">
            <div className="simulation-card-title-row">
              <div>
                <h3>Mission Comparison Matrix</h3>
                <p>
                  Roll saved scenario runs into a mission-level pattern readout. These comparisons
                  are directional strategy simulations, not factual outcomes.
                </p>
              </div>
              <div className="simulation-score-badge">
                {comparisonMatrix ? `${comparisonMatrix.scenario_count} runs` : 'Awaiting runs'}
              </div>
            </div>

            {comparisonError ? <div className="simulation-empty">{comparisonError}</div> : null}
            {comparisonLoading ? <div className="simulation-empty">加载 comparison matrix...</div> : null}

            {!comparisonLoading && !comparisonError && comparisonMatrix ? (
              <>
                <div className="simulation-comparison-summary">
                  <div className="simulation-comparison-summary-copy">
                    <span className="simulation-comparison-kicker">
                      {comparisonMatrix.mission_title || selectedMission?.title || 'Mission summary'}
                    </span>
                    <p>
                      {comparisonMatrix.summary ||
                        'Comparison summary is available once the backend aggregates mission runs.'}
                    </p>
                  </div>

                  <div className="simulation-comparison-metrics">
                    <div className="simulation-comparison-metric">
                      <span>Scenario runs</span>
                      <strong>{comparisonMatrix.scenario_count}</strong>
                    </div>
                    <div className="simulation-comparison-metric">
                      <span>Option patterns</span>
                      <strong>{comparisonMatrix.option_patterns.length}</strong>
                    </div>
                    <div className="simulation-comparison-metric">
                      <span>Variable axes</span>
                      <strong>{comparisonMatrix.variable_axes.length}</strong>
                    </div>
                    <div className="simulation-comparison-metric">
                      <span>Latest path</span>
                      <strong>{latestPathStep?.selected_option_label ?? 'Pending'}</strong>
                    </div>
                  </div>
                </div>

                <div className="simulation-comparison-grid">
                  <section className="simulation-comparison-panel">
                    <div className="simulation-section-heading">
                      <div>
                        <h4>Option Pattern Matrix</h4>
                        <p>See which options recur, get selected, and sustain stronger scores.</p>
                      </div>
                    </div>

                    {comparisonMatrix.option_patterns.length === 0 ? (
                      <div className="simulation-empty simulation-empty--subtle">
                        No option patterns have been aggregated for this mission yet.
                      </div>
                    ) : (
                      <div className="simulation-pattern-table-wrap">
                        <table className="simulation-pattern-table">
                          <thead>
                            <tr>
                              <th>Option</th>
                              <th>Appears</th>
                              <th>Selected</th>
                              <th>Selection rate</th>
                              <th>Avg score</th>
                              <th>Latest horizon</th>
                            </tr>
                          </thead>
                          <tbody>
                            {comparisonMatrix.option_patterns.map((pattern) => {
                              const selectionRate =
                                pattern.appearance_count > 0
                                  ? (pattern.selected_count / pattern.appearance_count) * 100
                                  : 0;

                              return (
                                <tr key={pattern.label}>
                                  <td>{pattern.label}</td>
                                  <td>{pattern.appearance_count}</td>
                                  <td>{pattern.selected_count}</td>
                                  <td>{formatPercent(selectionRate)}</td>
                                  <td>{formatScoreValue(pattern.average_score)}</td>
                                  <td>{pattern.latest_time_horizon || 'n/a'}</td>
                                </tr>
                              );
                            })}
                          </tbody>
                        </table>
                      </div>
                    )}
                  </section>

                  <section className="simulation-comparison-panel">
                    <div className="simulation-section-heading">
                      <div>
                        <h4>Variable Axes</h4>
                        <p>Track how the same decision space is being stressed across saved runs.</p>
                      </div>
                    </div>

                    {comparisonMatrix.variable_axes.length === 0 ? (
                      <div className="simulation-empty simulation-empty--subtle">
                        No variable axes have been recorded for this mission yet.
                      </div>
                    ) : (
                      <div className="simulation-axis-grid">
                        {comparisonMatrix.variable_axes.map((axis) => (
                          <article className="simulation-axis-card" key={axis.label}>
                            <div className="simulation-axis-card-header">
                              <strong>{axis.label}</strong>
                              <span>{axis.values.length} states</span>
                            </div>
                            <div className="simulation-axis-detail">
                              <span>Values</span>
                              <p>{formatList(axis.values, 'No values recorded yet.')}</p>
                            </div>
                            <div className="simulation-axis-detail">
                              <span>Impacts</span>
                              <p>{formatList(axis.impacts, 'No impact labels recorded yet.')}</p>
                            </div>
                            <div className="simulation-axis-detail">
                              <span>Uncertainties</span>
                              <p>
                                {formatList(
                                  axis.uncertainties,
                                  'No uncertainty labels recorded yet.',
                                )}
                              </p>
                            </div>
                          </article>
                        ))}
                      </div>
                    )}
                  </section>
                </div>

                <section className="simulation-comparison-panel simulation-comparison-panel--timeline">
                  <div className="simulation-section-heading">
                    <div>
                      <h4>Path Evolution</h4>
                      <p>
                        Follow how the selected path changes over time as new scenario runs are
                        saved.
                      </p>
                    </div>
                  </div>

                  {pathEvolutionSteps.length === 0 ? (
                    <div className="simulation-empty simulation-empty--subtle">
                      No path evolution steps are available for this mission yet.
                    </div>
                  ) : (
                    <div className="simulation-timeline">
                      {pathEvolutionSteps.map((step) => (
                        <article className="simulation-timeline-item" key={step.scenario_run_id}>
                          <div className="simulation-timeline-rail">
                            <span className="simulation-timeline-dot" />
                          </div>

                          <div className="simulation-timeline-content">
                            <div className="simulation-timeline-header">
                              <div>
                                <strong>{step.selected_option_label}</strong>
                                <div className="simulation-timeline-meta">
                                  <span>{formatTimestamp(step.created_at)}</span>
                                  <span>Scenario {step.scenario_run_id.slice(0, 8)}</span>
                                </div>
                              </div>
                              <div className="simulation-timeline-score">
                                {formatScoreValue(step.score)}/100
                              </div>
                            </div>

                            <p className="simulation-timeline-narrative">
                              {step.narrative || 'No narrative recorded for this path step.'}
                            </p>

                            {step.variable_changes.length > 0 ? (
                              <div className="simulation-timeline-change-row">
                                {step.variable_changes.map((change) => (
                                  <span className="simulation-timeline-change" key={change}>
                                    {change}
                                  </span>
                                ))}
                              </div>
                            ) : null}
                          </div>
                        </article>
                      ))}
                    </div>
                  )}
                </section>
              </>
            ) : null}

            {!comparisonLoading && !comparisonError && !comparisonMatrix ? (
              <div className="simulation-empty simulation-empty--subtle">
                {scenarioRuns.length === 0
                  ? 'Save scenario runs to unlock the mission-level comparison matrix.'
                  : 'Mission comparison data is not available from the current backend yet.'}
              </div>
            ) : null}
          </section>

          <div className="simulation-summary-grid">
            <MetricCard label="Total missions" value={overview?.summary.total_missions ?? 0} />
            <MetricCard label="Active missions" value={overview?.summary.active_missions ?? 0} />
            <MetricCard
              label="Simulating missions"
              value={overview?.summary.simulating_missions ?? 0}
            />
            <MetricCard label="Total runs" value={overview?.summary.total_runs ?? 0} />
            <MetricCard label="Simulation runs" value={overview?.summary.simulation_runs ?? 0} />
            <MetricCard
              label="Missions with runs"
              value={overview?.summary.missions_with_runs ?? 0}
            />
          </div>

          <div className="simulation-layout">
            <section className="simulation-card">
              <h3>Run Type Mix</h3>
              <div className="simulation-count-list">
                {overview?.counts_by_type.map((item) => (
                  <div className="simulation-count-row" key={item.key}>
                    <span>{item.key}</span>
                    <strong>{item.count}</strong>
                  </div>
                ))}
              </div>
            </section>

            <section className="simulation-card">
              <h3>Status Mix</h3>
              <div className="simulation-count-list">
                {overview?.counts_by_status.map((item) => (
                  <div className="simulation-count-row" key={item.key}>
                    <span>{item.key}</span>
                    <strong>{item.count}</strong>
                  </div>
                ))}
              </div>
            </section>
          </div>

          <section className="simulation-card">
            <h3>Recent Mission Runs</h3>
            {overview && overview.recent_runs.length === 0 ? (
              <div className="simulation-empty">当前还没有可展示的 recent simulation-oriented runs。</div>
            ) : (
              <div className="simulation-run-list">
                {overview?.recent_runs.map((run) => (
                  <article className="simulation-run-item" key={run.run_id}>
                    <div className="simulation-run-title-row">
                      <strong>{run.mission_title}</strong>
                      <span>{run.run_type}</span>
                    </div>
                    <div className="simulation-run-meta">
                      {run.mission_status} · {run.mission_priority} · {run.run_status}
                    </div>
                    <div className="simulation-run-meta">activity: {formatTimestamp(run.activity_at)}</div>
                    <div className="simulation-run-summary">
                      {run.summary ?? run.error_message ?? 'No summary available.'}
                    </div>
                  </article>
                ))}
              </div>
            )}
          </section>
        </>
      ) : null}
    </div>
  );
}

function OptionComparisonCard({
  option,
  selected,
  selectionName,
  onSelect,
}: {
  option: ScenarioOptionCard;
  selected: boolean;
  selectionName?: string;
  onSelect?: () => void;
}) {
  return (
    <article
      className={`simulation-option-card${selected ? ' simulation-option-card--selected' : ''}${onSelect ? ' simulation-option-card--interactive' : ''}`}
    >
      <div className="simulation-option-card-header">
        <div>
          <strong>{option.label}</strong>
          <div className="simulation-option-card-meta">
            <span>{option.confidence} confidence</span>
            <span>{option.time_horizon}</span>
          </div>
        </div>
        <div className="simulation-option-score-ring">
          <span>{option.score}</span>
          <small>score</small>
        </div>
      </div>

      {onSelect ? (
        <label className="simulation-option-selector">
          <input
            checked={selected}
            name={selectionName}
            type="radio"
            onChange={onSelect}
          />
          <span>{selected ? 'Recommended in sandbox' : 'Set as recommended option'}</span>
        </label>
      ) : selected ? (
        <div className="simulation-option-selector simulation-option-selector--static">
          <span>Saved as selected option</span>
        </div>
      ) : null}

      <div className="simulation-option-section">
        <span>Projected outcomes</span>
        <ul>
          {(option.projected_outcomes.length > 0
            ? option.projected_outcomes
            : ['No projected outcomes recorded.']
          ).map((item) => (
            <li key={item}>{item}</li>
          ))}
        </ul>
      </div>

      <div className="simulation-option-detail-grid">
        <div className="simulation-option-detail">
          <span>Assumptions</span>
          <p>
            {option.assumptions[0] ?? 'No assumptions recorded.'}
          </p>
        </div>
        <div className="simulation-option-detail">
          <span>Best outcome</span>
          <p>{option.expected_benefits[0] ?? 'No explicit upside recorded.'}</p>
        </div>
        <div className="simulation-option-detail">
          <span>Main risk</span>
          <p>{option.risks[0] ?? 'No explicit risk recorded.'}</p>
        </div>
      </div>
    </article>
  );
}

function MetricCard({ label, value }: { label: string; value: number }) {
  return (
    <div className="simulation-metric-card">
      <span className="simulation-metric-value">{value}</span>
      <span className="simulation-metric-label">{label}</span>
    </div>
  );
}

function parseOptions(value: string): string[] {
  return value
    .split('\n')
    .map((option) => option.trim())
    .filter(Boolean);
}

function createScenarioVariable(): ScenarioVariable {
  return {
    id: `scenario-variable-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    label: '',
    current_value: '',
    proposed_value: '',
    impact: 'medium',
    uncertainty: 'medium',
    impact_weight: 55,
    uncertainty_weight: 55,
  };
}

function formulaFromTemplate(template: SimulationScoringFormulaTemplate): ScenarioScoringFormula {
  return {
    baseScore: normalizeFormulaValue(template.base_score, 20, 80, defaultScoringFormula.baseScore),
    impactMultiplier: normalizeFormulaValue(
      template.impact_multiplier,
      0,
      30,
      defaultScoringFormula.impactMultiplier,
    ),
    uncertaintyPenalty: normalizeFormulaValue(
      template.uncertainty_penalty,
      0,
      30,
      defaultScoringFormula.uncertaintyPenalty,
    ),
  };
}

function normalizeFormulaValue(value: number, min: number, max: number, fallback: number): number {
  const finiteValue = Number.isFinite(value) ? value : fallback;
  return Math.round(clamp(finiteValue, min, max));
}

function describeFormulaTemplate(formula: ScenarioScoringFormula): string {
  return `Base ${formula.baseScore}, impact +${formula.impactMultiplier}, uncertainty -${formula.uncertaintyPenalty}.`;
}

function sanitizeVariables(variables: ScenarioVariable[]): ScenarioVariable[] {
  return variables
    .map((variable) => ({
      ...variable,
      label: variable.label.trim(),
      current_value: variable.current_value.trim(),
      proposed_value: variable.proposed_value.trim(),
      impact_weight: normalizeWeight(variable.impact_weight, variable.impact),
      uncertainty_weight: normalizeWeight(variable.uncertainty_weight, variable.uncertainty),
      impact: formatWeightLabel(normalizeWeight(variable.impact_weight, variable.impact)),
      uncertainty: formatWeightLabel(normalizeWeight(variable.uncertainty_weight, variable.uncertainty)),
    }))
    .filter(
      (variable) => variable.label || variable.current_value || variable.proposed_value,
    );
}

function buildOptionCards(
  options: string[],
  variables: ScenarioVariable[],
  formula: ScenarioScoringFormula,
): ScenarioOptionCard[] {
  const cleanVariables = sanitizeVariables(variables);

  return options.map((label, index) => {
    const strategy = inferOptionStrategy(label);
    const focusedVariables = rotateVariables(cleanVariables, index).slice(0, 2);
    const impactScore = averageScore(
      focusedVariables.map((variable) => getImpactScore(variable.impact_weight, formula.impactMultiplier)),
    );
    const uncertaintyPenalty = averageScore(
      focusedVariables.map((variable) =>
        getUncertaintyPenalty(variable.uncertainty_weight, formula.uncertaintyPenalty),
      ),
    );
    const score = clamp(
      Math.round(formula.baseScore + strategy.scoreBonus + impactScore - uncertaintyPenalty),
      18,
      96,
    );
    const confidence =
      uncertaintyPenalty <= 4 ? 'high' : uncertaintyPenalty <= 9 ? 'medium' : 'low';

    const projectedOutcomes =
      focusedVariables.length > 0
        ? focusedVariables.map(
            (variable) =>
              `${variable.label || 'Signal'} moves ${formatVariableShift(variable)} with ${formatWeightLabel(variable.impact_weight)} impact / ${formatWeightLabel(variable.uncertainty_weight)} uncertainty.`,
          )
        : [`${label} creates a dedicated sandbox branch for controlled evaluation.`];

    const assumptions =
      focusedVariables.length > 0
        ? focusedVariables.map(
            (variable) =>
              `${variable.label || 'Signal'} can be steered toward ${variable.proposed_value || 'a new target'}.`,
          )
        : ['A contained simulation run is enough to compare this path before rollout.'];

    const expectedBenefits = [
      strategy.benefit,
      focusedVariables[0]
        ? `${focusedVariables[0].label || 'Primary signal'} becomes easier to validate before execution.`
        : 'Produces a clearer decision record for the Mission owner.',
    ];

    const risks = [
      focusedVariables.find((variable) => variable.uncertainty_weight >= 67)
        ? `${focusedVariables.find((variable) => variable.uncertainty_weight >= 67)?.label || 'A key signal'} still carries high uncertainty.`
        : strategy.risk,
      strategy.secondaryRisk,
    ];

    return {
      id: `option-${index + 1}`,
      label,
      assumptions: uniqueItems(assumptions).slice(0, 2),
      expected_benefits: uniqueItems(expectedBenefits).slice(0, 2),
      risks: uniqueItems(risks).slice(0, 2),
      projected_outcomes: uniqueItems(projectedOutcomes).slice(0, 3),
      score,
      time_horizon: strategy.timeHorizon,
      confidence,
    };
  });
}

function getSelectedCard(
  options: ScenarioOptionCard[],
  selectedOptionId: string,
): ScenarioOptionCard | null {
  if (options.length === 0) {
    return null;
  }

  return options.find((option) => option.id === selectedOptionId) ?? options[0] ?? null;
}

function getTopOptionId(options: ScenarioOptionCard[]): string {
  return [...options].sort((left, right) => right.score - left.score)[0]?.id ?? '';
}

function getScoreEdge(options: ScenarioOptionCard[], selectedOptionId: string | null): string {
  if (options.length < 2 || !selectedOptionId) {
    return 'n/a';
  }

  const selected = options.find((option) => option.id === selectedOptionId);
  const challenger = [...options]
    .filter((option) => option.id !== selectedOptionId)
    .sort((left, right) => right.score - left.score)[0];

  if (!selected || !challenger) {
    return 'n/a';
  }

  return `${selected.score - challenger.score} pts`;
}

function buildComparisonSummary(
  options: ScenarioOptionCard[],
  selectedOptionId: string | null,
): string {
  if (options.length === 0) {
    return '';
  }

  const selected = getSelectedCard(options, selectedOptionId ?? '');
  const challenger = [...options]
    .filter((option) => option.id !== selected?.id)
    .sort((left, right) => right.score - left.score)[0];

  if (!selected) {
    return 'No recommended option selected yet.';
  }

  if (!challenger) {
    return `${selected.label} is the only scenario option and currently defines the sandbox baseline.`;
  }

  return `${selected.label} leads ${challenger.label} by ${selected.score - challenger.score} points because it pairs a ${selected.time_horizon} horizon with ${selected.confidence} confidence and a clearer outcome path.`;
}

function buildRecommendationReason(
  options: ScenarioOptionCard[],
  selectedOptionId: string | null,
): string {
  const selected = getSelectedCard(options, selectedOptionId ?? '');

  if (!selected) {
    return '';
  }

  const strongestOutcome =
    selected.projected_outcomes[0] ?? selected.expected_benefits[0] ?? 'it creates the clearest next move';

  return `Recommend ${selected.label} because it scores ${selected.score}/100, fits a ${selected.time_horizon} decision window, and shows the strongest immediate outcome: ${strongestOutcome}`;
}

function getLocalSandboxStatusTone(status: string): 'info' | 'success' | 'error' {
  if (status.startsWith('Local sandbox completed')) {
    return 'success';
  }
  if (status.startsWith('Select a Mission') || status.startsWith('Add ')) {
    return 'info';
  }
  return 'error';
}

function buildLocalSandboxReplayPayload(
  run: SimulationLocalSandboxRun,
): LocalSandboxReplayPayload {
  return {
    run_id: run.run_id,
    engine: run.engine,
    agents: run.agents,
    turns: run.turns,
    option_scores: run.option_scores,
    recommendation: run.recommendation,
    audit_event_id: run.audit_event_id ?? null,
  };
}

function buildSimulationCapabilityEvidenceBundle(
  missionId: string,
  targetRemoteUserId: string,
  targetRemoteUserProfile: AgentExchangeRemoteUser | null,
  externalSaasRuns: SimulationExternalSaasRunHistoryItem[],
  highFidelityRuns: SimulationHighFidelitySandboxRunHistoryItem[],
): SimulationCapabilityEvidenceBundle {
  return {
    schema_version: '1.0.0',
    exported_at: new Date().toISOString(),
    mission_id: missionId,
    target_remote_user_id: targetRemoteUserId.trim() || null,
    target_remote_user_profile: buildTargetRemoteUserProfileSnapshot(
      targetRemoteUserId,
      targetRemoteUserProfile,
    ),
    counts: {
      external_saas_runs: externalSaasRuns.length,
      high_fidelity_sandbox_runs: highFidelityRuns.length,
    },
    boundary_notes: [...SIMULATION_CAPABILITY_EVIDENCE_BOUNDARY_NOTES],
    external_saas_runs: externalSaasRuns,
    high_fidelity_sandbox_runs: highFidelityRuns,
  };
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

function formatTimestamp(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat('zh-CN', {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(date);
}

function formatOptionalTimestamp(value: string | null | undefined, fallback: string): string {
  return value ? formatTimestamp(value) : fallback.slice(0, 8);
}

function getExternalSaasRunStatus(run: SimulationExternalSaasRunHistoryItem): string {
  if (run.status) {
    return run.status;
  }
  if (run.dry_run) {
    return 'previewed';
  }
  return run.executed ? 'completed' : 'planned';
}

function formatExternalSaasRunOutcome(run: SimulationExternalSaasRunHistoryItem): string {
  if (typeof run.response_status === 'number') {
    return String(run.response_status);
  }
  return run.dry_run ? 'dry-run' : 'local';
}

function getHighFidelitySandboxRunStatus(run: SimulationHighFidelitySandboxRunHistoryItem): string {
  return run.status ?? 'completed';
}

function formatVariableShift(variable: ScenarioVariable): string {
  const from = variable.current_value || 'current';
  const to = variable.proposed_value || 'proposed';
  return `${from} -> ${to}`;
}

function formatScoreValue(value: number): string {
  const rounded = Math.round(value * 10) / 10;
  return Number.isInteger(rounded) ? String(rounded) : rounded.toFixed(1);
}

function formatPercent(value: number): string {
  return `${Math.round(value)}%`;
}

function formatList(values: string[], emptyLabel: string): string {
  const items = uniqueItems(values);
  return items.length > 0 ? items.join(' • ') : emptyLabel;
}

function averageScore(values: number[]): number {
  if (values.length === 0) {
    return 0;
  }

  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

function rotateVariables(variables: ScenarioVariable[], offset: number): ScenarioVariable[] {
  if (variables.length === 0) {
    return [];
  }

  const start = offset % variables.length;
  return [...variables.slice(start), ...variables.slice(0, start)];
}

function getImpactScore(value: number, multiplier: number): number {
  return (clamp(value, 0, 100) / 100) * multiplier;
}

function getUncertaintyPenalty(value: number, multiplier: number): number {
  return (clamp(value, 0, 100) / 100) * multiplier;
}

function normalizeWeight(value: number | undefined, fallbackLevel: string): number {
  if (typeof value === 'number' && Number.isFinite(value) && value > 0) {
    return clamp(value, 0, 100);
  }

  switch (fallbackLevel) {
    case 'high':
      return 85;
    case 'low':
      return 25;
    default:
      return 55;
  }
}

function describeHandoffTarget(value: string): string {
  return handoffTargetOptions.find((option) => option.value === value)?.description ?? handoffTargetOptions[0].description;
}

function describeHandoffOutcome(target: string, riskLevel: string): string {
  switch (target) {
    case 'council_only':
      return 'Saving creates a completed Simulation run, timeline event, and Scenario Reviewer Council step.';
    case 'execution_only':
      return `Saving creates a completed Simulation run, timeline event, and ${riskLevel} Execution review step.`;
    case 'timeline_only':
      return 'Saving creates only the completed Simulation run and timeline event; no review steps are generated.';
    default:
      return `Saving creates a completed Simulation run, timeline event, Scenario Reviewer Council step, and ${riskLevel} Execution review step.`;
  }
}

function formatWeightLabel(value: number): string {
  if (value >= 67) {
    return 'high';
  }
  if (value <= 33) {
    return 'low';
  }
  return 'medium';
}

function inferOptionStrategy(label: string) {
  const lower = label.toLowerCase();

  if (/(pilot|test|trial|sandbox|simulate|explore)/.test(lower)) {
    return {
      scoreBonus: 10,
      timeHorizon: '2-6 weeks',
      benefit: 'Improves learning velocity before a full commitment.',
      risk: 'Insights are only useful if the follow-through path stays funded.',
      secondaryRisk: 'A pilot can slip into permanent limbo without clear exit criteria.',
    };
  }

  if (/(increase|accelerate|expand|launch|hire|push)/.test(lower)) {
    return {
      scoreBonus: 6,
      timeHorizon: '30-60 days',
      benefit: 'Creates the fastest visible movement on the primary objective.',
      risk: 'Execution pressure rises quickly if support signals do not keep up.',
      secondaryRisk: 'Aggressive moves can surface downstream bottlenecks earlier.',
    };
  }

  if (/(delay|reduce|defer|pause|cut|stabil|protect)/.test(lower)) {
    return {
      scoreBonus: 2,
      timeHorizon: '1-2 quarters',
      benefit: 'Protects downside risk while preserving room to reassess.',
      risk: 'Can dampen momentum and stakeholder confidence.',
      secondaryRisk: 'Deferred action may compound later costs.',
    };
  }

  if (/(partner|bundle|shared|cross|joint)/.test(lower)) {
    return {
      scoreBonus: 5,
      timeHorizon: '6-10 weeks',
      benefit: 'Shares load across teams or partners instead of concentrating risk.',
      risk: 'Delivery confidence depends on external alignment.',
      secondaryRisk: 'Shared ownership can blur accountability if the plan is not crisp.',
    };
  }

  return {
    scoreBonus: 4,
    timeHorizon: '6-8 weeks',
    benefit: 'Balances speed, reversibility, and signal quality.',
    risk: 'Results depend on consistent operator follow-through.',
    secondaryRisk: 'This path may look safe while still hiding execution drift.',
  };
}

function uniqueItems(values: string[]): string[] {
  return [...new Set(values.filter(Boolean))];
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
