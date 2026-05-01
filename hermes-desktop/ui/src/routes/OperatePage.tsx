import { useEffect, useMemo, useState } from 'react';
import {
  executionAddStepNote,
  executionApproveStep,
  executionCompleteStep,
  executionConfirmSkipStep,
  executionListByMission,
  executionListDesktopHandoffQueue,
  executionMarkDesktopHandoffReviewed,
  executionPauseStep,
  executionPrepareDesktopHandoff,
  executionRerunStep,
  executionResumeStep,
  executionRetryStep,
  executionStartStep,
  type ExecutionDesktopHandoff,
  type ExecutionDesktopHandoffQueueItem,
  type ExecutionStep,
  playbookGet,
  type MissionPlaybook,
} from '../lib/tauri';
import { useMissionStore } from '../store/missionStore';
import './OperatePage.css';

const recoverySectionCopy = {
  failed: {
    heading: 'Failed',
    empty: '暂无失败步骤。',
    hint: '失败步骤需要人工判断是直接重试，还是先补充上下文后再恢复。',
    badge: '需要重试',
    primaryAction: '重试步骤',
    secondaryAction: '查看失败上下文',
    actionHint: '可以直接重试；查看失败上下文会把该步骤切到右侧检查区。',
    summaryFallback: '暂无失败摘要，请先查看运行上下文。',
  },
  paused: {
    heading: 'Paused',
    empty: '暂无暂停步骤。',
    hint: '暂停步骤通常在等待人工确认、环境恢复或时机满足后继续执行。',
    badge: '等待恢复',
    primaryAction: '恢复执行',
    secondaryAction: '确认继续条件',
    actionHint: '恢复执行会把步骤重新送回 running；也可以先切到右侧确认条件。',
    summaryFallback: '暂无暂停原因说明，请结合上游步骤确认恢复条件。',
  },
  skipped: {
    heading: 'Skipped',
    empty: '暂无跳过步骤。',
    hint: '被跳过的步骤需要决定是维持跳过，还是在条件具备后补跑。',
    badge: '待确认策略',
    primaryAction: '补跑此步骤',
    secondaryAction: '确认保持跳过',
    actionHint: '补跑会把步骤送回 pending；确认保持跳过会保留当前状态并刷新摘要。',
    summaryFallback: '暂无跳过说明，请人工确认是否需要回补。',
  },
} as const;

type RecoverySectionTone = keyof typeof recoverySectionCopy;

type OperateActionKind = 'approve' | 'start' | 'pause' | 'complete';

type OperateActionPresentation = {
  label: string;
  detail: string;
  disabled: boolean;
  busy: boolean;
};

type DesktopHandoffPackage = {
  source: 'prepared' | 'derived';
  step_id: string;
  run_id: string;
  mission_id: string;
  title: string;
  risk: string;
  status: string;
  handoff_state: 'needs_handoff' | 'prepared' | 'reviewed';
  input_payload: unknown | null;
  input_payload_raw: string | null;
  checklist: string[];
  handoff_prompt: string;
  review_note_guidance: string;
  reason: string;
  manual_operator_handoff: true;
  gui_automation_executed: false;
};

function RecoveryStepSection({
  tone,
  items,
  actionLoading,
  onInspect,
  onPrimaryAction,
  onSecondaryAction,
}: {
  tone: RecoverySectionTone;
  items: ExecutionStep[];
  actionLoading: string | null;
  onInspect: (step: ExecutionStep) => void;
  onPrimaryAction: (step: ExecutionStep) => void;
  onSecondaryAction: (step: ExecutionStep) => void;
}) {
  const copy = recoverySectionCopy[tone];

  return (
    <section className={`operate-card operate-card-${tone}`}>
      <div className="operate-section-header">
        <div>
          <h3>{copy.heading}</h3>
          <p className="operate-section-hint">{copy.hint}</p>
        </div>
        <span className={`operate-status-pill operate-status-pill-${tone}`}>{copy.badge}</span>
      </div>
      {items.length === 0 ? (
        <div className="operate-target-label">{copy.empty}</div>
      ) : (
        <div className="operate-list">
          {items.map((item) => (
            <div className={`operate-list-item operate-list-item-${tone}`} key={item.id}>
              {(() => {
                const primaryKey =
                  tone === 'failed'
                    ? `retry:${item.id}`
                    : tone === 'paused'
                      ? `resume:${item.id}`
                      : `rerun:${item.id}`;
                const secondaryKey = tone === 'skipped' ? `confirm-skip:${item.id}` : null;
                const primaryBusy = actionLoading === primaryKey;
                const secondaryBusy = secondaryKey !== null && actionLoading === secondaryKey;

                return (
                  <>
              <div className="operate-list-title-row">
                <div className="operate-list-title">{item.title}</div>
                <span className={`operate-inline-status operate-inline-status-${tone}`}>{item.status}</span>
              </div>
              <div className="operate-list-meta">
                <span>{item.mode}</span>
                <span>{item.risk_level}</span>
                <span>{item.status}</span>
              </div>
              <div className="operate-completed-summary">
                {item.output_summary ?? copy.summaryFallback}
              </div>
              <div className="operate-status-actions">
                <button
                  type="button"
                  className={`secondary operate-recovery-button operate-recovery-button-${tone}${primaryBusy ? ' is-loading' : ''}`}
                  onClick={() => onPrimaryAction(item)}
                  disabled={actionLoading !== null}
                >
                  {copy.primaryAction}
                </button>
                <button
                  type="button"
                  className={`secondary operate-recovery-button${secondaryBusy ? ' is-loading' : ''}`}
                  onClick={() => onSecondaryAction(item)}
                  disabled={actionLoading !== null}
                >
                  {copy.secondaryAction}
                </button>
              </div>
              <div className="operate-status-note">
                {tone === 'skipped'
                  ? copy.actionHint
                  : '查看失败/暂停上下文会把该步骤切到右侧检查器。'}
              </div>
              <button
                type="button"
                className="operate-inline-link"
                onClick={() => onInspect(item)}
                disabled={actionLoading !== null}
              >
                在右侧检查此步骤
              </button>
                  </>
                );
              })()}
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

function getActionPresentation(
  action: OperateActionKind,
  activeStep: ExecutionStep | null,
  actionLoading: string | null,
): OperateActionPresentation {
  const busy = actionLoading === action;
  const anotherActionBusy = actionLoading !== null && actionLoading !== action;

  if (!activeStep) {
    const idleCopy = {
      approve: {
        label: '审批通过',
        detail: '当前没有可审批步骤',
      },
      start: {
        label: '开始执行',
        detail: '当前没有可启动步骤',
      },
      pause: {
        label: '暂停执行',
        detail: '当前没有可暂停步骤',
      },
      complete: {
        label: '标记完成',
        detail: '当前没有可完成步骤',
      },
    } satisfies Record<OperateActionKind, Pick<OperateActionPresentation, 'label' | 'detail'>>;

    return {
      ...idleCopy[action],
      disabled: true,
      busy: false,
    };
  }

  if (busy) {
    const busyCopy = {
      approve: {
        label: '审批提交中...',
        detail: '正在提交审批并刷新步骤列表',
      },
      start: {
        label: '启动中...',
        detail: '正在触发执行并同步最新状态',
      },
      pause: {
        label: '暂停中...',
        detail: '正在写入暂停状态并刷新步骤列表',
      },
      complete: {
        label: '完成提交中...',
        detail: '正在写入完成结果并刷新状态',
      },
    } satisfies Record<OperateActionKind, Pick<OperateActionPresentation, 'label' | 'detail'>>;

    return {
      ...busyCopy[action],
      disabled: true,
      busy: true,
    };
  }

  if (anotherActionBusy) {
    const waitingCopy = {
      approve: '另一个动作处理中，请稍候',
      start: '另一个动作处理中，请稍候',
      pause: '另一个动作处理中，请稍候',
      complete: '另一个动作处理中，请稍候',
    } satisfies Record<OperateActionKind, string>;

    return {
      label:
        action === 'approve'
          ? '审批通过'
          : action === 'start'
            ? '开始执行'
            : action === 'pause'
              ? '暂停执行'
              : '标记完成',
      detail: waitingCopy[action],
      disabled: true,
      busy: false,
    };
  }

  switch (action) {
    case 'approve':
      if (activeStep.status !== 'awaiting_approval') {
        const detail =
          activeStep.status === 'running'
            ? '运行中步骤无需再次审批'
            : activeStep.status === 'pending'
              ? '步骤尚未进入待审批状态'
              : '当前状态不可执行审批';
        return {
          label: '审批通过',
          detail,
          disabled: true,
          busy: false,
        };
      }
      return {
        label: '审批通过',
        detail: '确认后将允许该步骤继续推进',
        disabled: false,
        busy: false,
      };
    case 'start':
      if (activeStep.status === 'completed') {
        return {
          label: '开始执行',
          detail: '当前步骤已经完成',
          disabled: true,
          busy: false,
        };
      }
      return {
        label: activeStep.status === 'running' ? '重新发送开始' : '开始执行',
        detail:
          activeStep.status === 'running'
            ? '步骤显示为运行中，可再次触发开始'
            : '立即触发当前步骤进入执行',
        disabled: false,
        busy: false,
      };
    case 'complete':
      if (activeStep.status === 'completed') {
        return {
          label: '标记完成',
          detail: '当前步骤已经完成',
          disabled: true,
          busy: false,
        };
      }
      return {
        label: '标记完成',
        detail:
          activeStep.status === 'running'
            ? '确认输出无误后写入完成结果'
            : '直接将当前步骤记为已完成',
        disabled: false,
        busy: false,
      };
    case 'pause':
      if (activeStep.status !== 'running') {
        return {
          label: '暂停执行',
          detail: activeStep.status === 'paused' ? '当前步骤已经暂停' : '只有运行中步骤可以暂停',
          disabled: true,
          busy: false,
        };
      }
      return {
        label: '暂停执行',
        detail: '把当前运行中的步骤暂存到恢复队列',
        disabled: false,
        busy: false,
      };
  }
}

export function OperatePage() {
  const missions = useMissionStore((state) => state.missions);
  const selectedMissionId = useMissionStore((state) => state.selectedMissionId);
  const [steps, setSteps] = useState<ExecutionStep[]>([]);
  const [loading, setLoading] = useState(false);
  const [actionLoading, setActionLoading] = useState<string | null>(null);
  const [actionMessage, setActionMessage] = useState<string | null>(null);
  const [inspectedStepId, setInspectedStepId] = useState<string | null>(null);
  const [stepNoteDraft, setStepNoteDraft] = useState('');
  const [desktopHandoffReviewNote, setDesktopHandoffReviewNote] = useState('');
  const [desktopHandoffsByStepId, setDesktopHandoffsByStepId] = useState<
    Record<string, ExecutionDesktopHandoff>
  >({});
  const [desktopHandoffQueue, setDesktopHandoffQueue] = useState<ExecutionDesktopHandoffQueueItem[]>([]);
  const [playbook, setPlaybook] = useState<MissionPlaybook | null>(null);
  const selectedMission = useMemo(
    () => missions.find((mission) => mission.id === selectedMissionId) ?? null,
    [missions, selectedMissionId],
  );
  const currentSteps = useMemo(
    () => steps.filter((step) => step.status === 'running' || step.status === 'awaiting_approval'),
    [steps],
  );
  const queuedSteps = useMemo(
    () => steps.filter((step) => step.status === 'pending'),
    [steps],
  );
  const completedSteps = useMemo(
    () => steps.filter((step) => step.status === 'completed'),
    [steps],
  );
  const failedSteps = useMemo(
    () => steps.filter((step) => step.status === 'failed'),
    [steps],
  );
  const pausedSteps = useMemo(
    () => steps.filter((step) => String(step.status) === 'paused'),
    [steps],
  );
  const skippedSteps = useMemo(
    () => steps.filter((step) => step.status === 'skipped'),
    [steps],
  );
  const activeStep = useMemo(
    () => currentSteps[0] ?? queuedSteps[0] ?? null,
    [currentSteps, queuedSteps],
  );
  const inspectedStep = useMemo(
    () => steps.find((step) => step.id === inspectedStepId) ?? activeStep,
    [activeStep, inspectedStepId, steps],
  );
  const approveAction = useMemo(
    () => getActionPresentation('approve', activeStep, actionLoading),
    [activeStep, actionLoading],
  );
  const startAction = useMemo(
    () => getActionPresentation('start', activeStep, actionLoading),
    [activeStep, actionLoading],
  );
  const pauseAction = useMemo(
    () => getActionPresentation('pause', activeStep, actionLoading),
    [activeStep, actionLoading],
  );
  const completeAction = useMemo(
    () => getActionPresentation('complete', activeStep, actionLoading),
    [activeStep, actionLoading],
  );
  const actionStatus = useMemo(() => {
    if (actionLoading === 'approve') {
      return {
        tone: 'loading',
        title: '正在提交审批通过',
        text: '按钮暂时锁定，等待审批结果写入并刷新当前步骤列表。',
      };
    }
    if (actionLoading === 'start') {
      return {
        tone: 'loading',
        title: '正在启动当前步骤',
        text: '界面会在刷新后展示最新状态，避免重复点击。',
      };
    }
    if (actionLoading === 'pause') {
      return {
        tone: 'loading',
        title: '正在暂停当前步骤',
        text: '步骤会转入恢复队列，方便稍后继续执行。',
      };
    }
    if (actionLoading === 'complete') {
      return {
        tone: 'loading',
        title: '正在标记步骤完成',
        text: '完成摘要写入期间，其他动作会暂时禁用。',
      };
    }
    if (!activeStep) {
      return {
        tone: 'idle',
        title: '暂无可操作步骤',
        text: '当前没有 running、awaiting_approval 或 pending 的步骤，因此操作按钮保持禁用。',
      };
    }
    if (activeStep.status === 'awaiting_approval') {
      return {
        tone: 'waiting',
        title: '当前步骤等待人工审批',
        text: '优先使用“审批通过”，审批完成后再继续执行或直接完成。',
      };
    }
    if (activeStep.status === 'running') {
      return {
        tone: 'active',
        title: '当前步骤执行中',
        text: '可以继续观察执行情况，也可以先暂停，再稍后恢复。',
      };
    }
    return {
      tone: 'idle',
      title: '当前步骤处于排队状态',
      text: '可以手动开始执行，按钮文案会明确展示下一步动作。',
    };
  }, [activeStep, actionLoading]);

  useEffect(() => {
    if (!selectedMissionId) {
      setSteps([]);
      setDesktopHandoffsByStepId({});
      setDesktopHandoffQueue([]);
      return;
    }
    const missionId = selectedMissionId;
    setDesktopHandoffsByStepId({});

    let cancelled = false;
    async function loadSteps() {
      setLoading(true);
      try {
        const [items, desktopQueue] = await Promise.all([
          executionListByMission(missionId),
          executionListDesktopHandoffQueue({ mission_id: missionId }),
        ]);
        const playbookData = await playbookGet(missionId).catch(() => null);
        if (!cancelled) {
          setSteps(items);
          setDesktopHandoffQueue(desktopQueue);
          setPlaybook(playbookData);
          setActionMessage(null);
          setInspectedStepId((current) =>
            current && items.some((item) => item.id === current) ? current : null,
          );
        }
      } catch {
        if (!cancelled) {
          setSteps([]);
          setPlaybook(null);
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    void loadSteps();
    return () => {
      cancelled = true;
    };
  }, [selectedMissionId]);

  const recentEvidence = useMemo(() => {
    if (playbook?.evidence_cards?.length) {
      return playbook.evidence_cards.map((item) => ({
        title: item.title,
        text: item.summary,
      }));
    }
    return [
      {
        title: 'Execution Queue',
        text:
          queuedSteps.length > 0
            ? `${queuedSteps.length} 个步骤仍在排队，优先补齐开始/审批链路。`
            : '当前没有排队中的步骤。',
      },
      {
        title: 'Completed Output',
        text:
          completedSteps[0]?.output_summary ??
          '最近尚未沉淀完成摘要，建议先在 Missions 中补齐 playbook 证据。',
      },
    ];
  }, [completedSteps, playbook, queuedSteps.length]);
  const latestStepNote = useMemo(
    () => extractLatestUserNote(inspectedStep?.input_payload),
    [inspectedStep?.input_payload],
  );
  const inspectedDesktopQueueItem = useMemo(
    () => desktopHandoffQueue.find((item) => item.step.id === inspectedStep?.id) ?? null,
    [desktopHandoffQueue, inspectedStep?.id],
  );
  const preparedDesktopHandoff = useMemo(
    () => (inspectedStep ? desktopHandoffsByStepId[inspectedStep.id] ?? null : null),
    [desktopHandoffsByStepId, inspectedStep],
  );
  const desktopHandoffPackage = useMemo(
    () =>
      inspectedStep?.mode === 'desktop'
        ? buildDesktopHandoffPackage(inspectedStep, inspectedDesktopQueueItem, preparedDesktopHandoff)
        : null,
    [inspectedDesktopQueueItem, inspectedStep, preparedDesktopHandoff],
  );
  const desktopHandoffMarkdown = useMemo(
    () => (desktopHandoffPackage ? buildDesktopHandoffMarkdown(desktopHandoffPackage) : ''),
    [desktopHandoffPackage],
  );
  const noteActionLoading = inspectedStep ? actionLoading === `note:${inspectedStep.id}` : false;
  const desktopHandoffLoading = inspectedStep ? actionLoading === `desktop-handoff:${inspectedStep.id}` : false;
  const desktopHandoffReviewLoading = inspectedStep
    ? actionLoading === `desktop-review:${inspectedStep.id}`
    : false;

  const currentTargetLabel = useMemo(() => {
    if (inspectedStep) {
      return `${inspectedStep.mode.toUpperCase()} / 当前执行适配器`;
    }
    if (activeStep) {
      return `${activeStep.mode.toUpperCase()} / 当前 mission 通道`;
    }
    return 'Mission-scoped operator workflow';
  }, [activeStep, inspectedStep]);

  async function refreshSteps() {
    if (!selectedMissionId) return;
    const [items, desktopQueue] = await Promise.all([
      executionListByMission(selectedMissionId),
      executionListDesktopHandoffQueue({ mission_id: selectedMissionId }),
    ]);
    setSteps(items);
    setDesktopHandoffQueue(desktopQueue);
    setInspectedStepId((current) =>
      current && items.some((item) => item.id === current) ? current : null,
    );
  }

  async function handleApprove() {
    if (!activeStep) return;
    setActionLoading('approve');
    try {
      await executionApproveStep(activeStep.id);
      await refreshSteps();
      setActionMessage('审批已通过，步骤已进入运行状态。');
    } finally {
      setActionLoading(null);
    }
  }

  async function handleStart() {
    if (!activeStep) return;
    setActionLoading('start');
    try {
      await executionStartStep(activeStep.id);
      await refreshSteps();
      setActionMessage('步骤已开始执行。');
    } finally {
      setActionLoading(null);
    }
  }

  async function handlePause() {
    if (!activeStep) return;
    setActionLoading('pause');
    try {
      await executionPauseStep(activeStep.id);
      await refreshSteps();
      setActionMessage('步骤已暂停，并移入恢复队列。');
    } finally {
      setActionLoading(null);
    }
  }

  async function handleComplete() {
    if (!activeStep) return;
    setActionLoading('complete');
    try {
      await executionCompleteStep(activeStep.id, 'UI 手动完成');
      await refreshSteps();
      setActionMessage('步骤已标记为完成。');
    } finally {
      setActionLoading(null);
    }
  }

  async function handleRecoveryAction(step: ExecutionStep, action: 'retry' | 'resume' | 'rerun' | 'confirm-skip') {
    const actionKey =
      action === 'retry'
        ? `retry:${step.id}`
        : action === 'resume'
          ? `resume:${step.id}`
          : action === 'rerun'
            ? `rerun:${step.id}`
            : `confirm-skip:${step.id}`;

    setActionLoading(actionKey);
    try {
      if (action === 'retry') {
        await executionRetryStep(step.id);
        setActionMessage(`已将“${step.title}”重新放回待执行队列。`);
      } else if (action === 'resume') {
        await executionResumeStep(step.id);
        setActionMessage(`已恢复“${step.title}”，步骤重新进入运行状态。`);
      } else if (action === 'rerun') {
        await executionRerunStep(step.id);
        setActionMessage(`已将“${step.title}”补跑，步骤重新进入待执行队列。`);
      } else {
        await executionConfirmSkipStep(step.id);
        setActionMessage(`已确认“${step.title}”继续保持跳过。`);
      }
      await refreshSteps();
      setInspectedStepId(step.id);
    } finally {
      setActionLoading(null);
    }
  }

  async function handlePrepareDesktopHandoff() {
    if (!inspectedStep) return;
    setActionLoading(`desktop-handoff:${inspectedStep.id}`);
    try {
      const handoff = await executionPrepareDesktopHandoff({ id: inspectedStep.id });
      setDesktopHandoffsByStepId((current) => ({
        ...current,
        [handoff.step_id]: handoff,
      }));
      await refreshSteps();
      setActionMessage('已生成 manual/operator desktop handoff；当前未执行任何 GUI 自动化。');
    } finally {
      setActionLoading(null);
    }
  }

  async function handleMarkDesktopHandoffReviewed() {
    if (!inspectedStep) return;
    setActionLoading(`desktop-review:${inspectedStep.id}`);
    try {
      await executionMarkDesktopHandoffReviewed({
        run_id: inspectedStep.run_id,
        step_id: inspectedStep.id,
        review_note: desktopHandoffReviewNote.trim() || null,
      });
      setDesktopHandoffReviewNote('');
      await refreshSteps();
      setInspectedStepId(inspectedStep.id);
      setActionMessage('已记录 desktop handoff reviewed 事件；仍未执行任何 GUI 自动化。');
    } finally {
      setActionLoading(null);
    }
  }

  async function handleAddStepNote() {
    if (!inspectedStep) return;
    const note = stepNoteDraft.trim();
    if (!note) {
      setActionMessage('请先填写要写入执行上下文的批注。');
      return;
    }

    setActionLoading(`note:${inspectedStep.id}`);
    try {
      const updated = await executionAddStepNote({
        id: inspectedStep.id,
        note,
        pause_before_continue: true,
      });
      setStepNoteDraft('');
      setActionMessage(
        updated.status === 'paused'
          ? `已写入批注并暂停“${updated.title}”，等待人工复核后再继续。`
          : `已写入批注到“${updated.title}”的执行上下文。`,
      );
      await refreshSteps();
      setInspectedStepId(updated.id);
    } finally {
      setActionLoading(null);
    }
  }

  function handleInspectStep(step: ExecutionStep) {
    setInspectedStepId(step.id);
    setStepNoteDraft('');
    setDesktopHandoffReviewNote('');
    setActionMessage(`已切换到“${step.title}”的步骤详情。`);
  }

  async function handleCopyDesktopHandoffPrompt() {
    if (!desktopHandoffPackage) return;

    if (!navigator.clipboard?.writeText) {
      setActionMessage('系统剪贴板不可用，请从下方 <pre> 区域手动复制 handoff prompt；此处仅提供 manual/operator handoff，不会触发 GUI 自动化。');
      return;
    }

    try {
      await navigator.clipboard.writeText(desktopHandoffPackage.handoff_prompt);
      setActionMessage('已复制 manual/operator handoff prompt；此操作不会触发 GUI 自动化。');
    } catch {
      setActionMessage('无法写入系统剪贴板，请从下方 <pre> 区域手动复制 handoff prompt；此处不会执行 GUI 自动化。');
    }
  }

  function handleExportDesktopHandoff(format: 'json' | 'md') {
    if (!desktopHandoffPackage) return;
    const slug = toFileSlug(desktopHandoffPackage.title);
    const filename = `${slug || 'desktop-handoff'}-${desktopHandoffPackage.step_id}.${format === 'json' ? 'json' : 'md'}`;
    const content =
      format === 'json'
        ? `${JSON.stringify(desktopHandoffPackage, null, 2)}\n`
        : desktopHandoffMarkdown;

    downloadTextFile(
      filename,
      content,
      format === 'json' ? 'application/json;charset=utf-8' : 'text/markdown;charset=utf-8',
    );
    setActionMessage(
      format === 'json'
        ? '已导出 manual/operator handoff JSON；导出内容不包含任何 GUI 自动化执行。'
        : '已导出 manual/operator handoff Markdown；导出内容不包含任何 GUI 自动化执行。',
    );
  }

  if (!selectedMission) {
    return (
      <div className="operate-page">
        <h2>Operate</h2>
        <div className="operate-card">
          <h3>Current Target App</h3>
          <div className="operate-target">
            <span className="operate-target-label">当前状态</span>
            <span className="operate-target-value">暂无选中的 Mission</span>
            <span className="operate-target-label">
              先到 Missions 页面创建或选中一个任务，再进入 Operate。
            </span>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="operate-page">
      <h2>Operate</h2>
      {actionMessage ? <div className="operate-page-message">{actionMessage}</div> : null}
      <div className="operate-grid">
        <section className="operate-card">
          <h3>Current Target App</h3>
          <div className="operate-target">
            <span className="operate-target-label">当前目标应用</span>
            <span className="operate-target-value">{currentTargetLabel}</span>
            <span className="operate-target-label">当前 Mission</span>
            <span className="operate-target-value">{selectedMission.title}</span>
            <span className="operate-target-label">当前目标</span>
            <span className="operate-target-value">{selectedMission.goal}</span>
          </div>
          <div className="operate-actions">
            <button
              type="button"
              onClick={handleStart}
              disabled={startAction.disabled}
              aria-busy={startAction.busy}
              className={`operate-action-button${startAction.busy ? ' is-loading' : ''}${startAction.disabled ? ' is-disabled' : ''}`}
            >
              <span className="operate-action-button-label">{startAction.label}</span>
              <span className="operate-action-button-detail">
                {startAction.busy ? <span className="operate-action-button-spinner" aria-hidden="true" /> : null}
                {startAction.detail}
              </span>
            </button>
            <button
              type="button"
              onClick={handlePause}
              disabled={pauseAction.disabled}
              aria-busy={pauseAction.busy}
              className={`secondary operate-action-button${pauseAction.busy ? ' is-loading' : ''}${pauseAction.disabled ? ' is-disabled' : ''}`}
            >
              <span className="operate-action-button-label">{pauseAction.label}</span>
              <span className="operate-action-button-detail">
                {pauseAction.busy ? <span className="operate-action-button-spinner" aria-hidden="true" /> : null}
                {pauseAction.detail}
              </span>
            </button>
          </div>
        </section>

        <section className="operate-card">
          <h3>Current</h3>
          {loading ? <div className="operate-target-label">正在加载执行步骤...</div> : null}
          {!loading && currentSteps.length === 0 ? (
            <div className="operate-target-label">当前没有运行中或待审批步骤。</div>
          ) : null}
          <div className="operate-list">
            {currentSteps.map((item) => (
              <div className="operate-list-item" key={item.id}>
                <div className="operate-list-title">{item.title}</div>
                <div className="operate-list-meta">
                  <span>{item.mode}</span>
                  <span>{item.risk_level}</span>
                  <span>{item.status}</span>
                </div>
              </div>
            ))}
          </div>
        </section>

        <section className="operate-card operate-card-desktop-handoff">
          <div className="operate-section-header">
            <div>
              <h3>Desktop Handoff Queue</h3>
              <p className="operate-section-hint">Desktop steps are prepared for manual/runtime handoff only; no GUI automation runs here.</p>
            </div>
            <span className="operate-status-pill operate-status-pill-skipped">{desktopHandoffQueue.length} desktop</span>
          </div>
          {desktopHandoffQueue.length === 0 ? (
            <div className="operate-target-label">暂无 desktop handoff 步骤。</div>
          ) : (
            <div className="operate-list">
              {desktopHandoffQueue.map((item) => (
                <div className="operate-list-item" key={item.step.id}>
                  <div className="operate-list-title-row">
                    <div className="operate-list-title">{item.step.title}</div>
                    <span className={`operate-inline-status ${item.handoff_reviewed ? 'operate-inline-status-completed' : item.handoff_prepared ? 'operate-inline-status-paused' : 'operate-inline-status-skipped'}`}>
                      {item.handoff_reviewed ? 'reviewed' : item.handoff_prepared ? 'prepared' : 'needs handoff'}
                    </span>
                  </div>
                  <div className="operate-list-meta">
                    <span>{item.step.risk_level}</span>
                    <span>{item.step.status}</span>
                    <span>{item.latest_prepared_at ?? 'not prepared'}</span>
                    <span>{item.latest_reviewed_at ?? 'not reviewed'}</span>
                  </div>
                  <button
                    type="button"
                    className="operate-inline-link"
                    onClick={() => handleInspectStep(item.step)}
                    disabled={actionLoading !== null}
                  >
                    Inspect desktop step
                  </button>
                </div>
              ))}
            </div>
          )}
        </section>

        <section className="operate-card">
          <h3>Queued</h3>
          {!loading && queuedSteps.length === 0 ? (
            <div className="operate-target-label">暂无排队步骤。</div>
          ) : null}
          <div className="operate-list">
            {queuedSteps.map((item) => (
              <div className="operate-list-item" key={item.id}>
                <div className="operate-list-title">{item.title}</div>
                <div className="operate-list-meta">
                  <span>{item.mode}</span>
                  <span>{item.risk_level}</span>
                  <span>{item.status}</span>
                </div>
              </div>
            ))}
          </div>
        </section>

        <section className="operate-card">
          <h3>Step Inspector</h3>
          {inspectedStep ? (
            <div className="operate-list-item">
              <div className="operate-list-title">当前步骤：{inspectedStep.title}</div>
              <div className="operate-list-meta">
                <span>mode: {inspectedStep.mode}</span>
                <span>risk: {inspectedStep.risk_level}</span>
                <span>status: {inspectedStep.status}</span>
              </div>
              <div className="operate-detail-fields">
                <div className="operate-detail-field">
                  <span className="operate-detail-label">input</span>
                  <span className="operate-detail-value">{inspectedStep.input_payload ?? '-'}</span>
                </div>
                <div className="operate-detail-field">
                  <span className="operate-detail-label">summary</span>
                  <span className="operate-detail-value">{inspectedStep.output_summary ?? '-'}</span>
                </div>
              </div>
              {inspectedStep.mode === 'desktop' ? (
                <div className="operate-desktop-handoff-panel">
                  <div>
                    <strong>Desktop manual/operator handoff</strong>
                    <p>Generate or export an auditable handoff package for a desktop runtime or a human operator. This page does not execute GUI automation.</p>
                  </div>
                  <button
                    type="button"
                    className={`secondary operate-action-button${desktopHandoffLoading ? ' is-loading' : ''}`}
                    onClick={() => void handlePrepareDesktopHandoff()}
                    disabled={actionLoading !== null}
                    aria-busy={desktopHandoffLoading}
                  >
                    <span className="operate-action-button-label">Prepare desktop handoff</span>
                    <span className="operate-action-button-detail">
                      {desktopHandoffLoading ? <span className="operate-action-button-spinner" aria-hidden="true" /> : null}
                      Render a manual/operator prompt and checklist without automating the desktop.
                    </span>
                  </button>
                  {desktopHandoffPackage ? (
                    <div className="operate-desktop-handoff-result">
                      <div className="operate-desktop-handoff-summary">
                        <span className="operate-detail-label">handoff package source</span>
                        <span className="operate-detail-value">
                          {desktopHandoffPackage.source === 'prepared'
                            ? '最近一次 prepare 返回值'
                            : '根据当前 step/input payload 本地构造'}
                        </span>
                      </div>
                      <span>{desktopHandoffPackage.reason}</span>
                      <div className="operate-detail-fields operate-desktop-handoff-meta">
                        <div className="operate-detail-field">
                          <span className="operate-detail-label">step / run / mission</span>
                          <span className="operate-detail-value">
                            {desktopHandoffPackage.step_id} / {desktopHandoffPackage.run_id} / {desktopHandoffPackage.mission_id}
                          </span>
                        </div>
                        <div className="operate-detail-field">
                          <span className="operate-detail-label">risk / status / handoff</span>
                          <span className="operate-detail-value">
                            {desktopHandoffPackage.risk} / {desktopHandoffPackage.status} / {desktopHandoffPackage.handoff_state}
                          </span>
                        </div>
                        <div className="operate-detail-field">
                          <span className="operate-detail-label">review note guidance</span>
                          <span className="operate-detail-value">{desktopHandoffPackage.review_note_guidance}</span>
                        </div>
                      </div>
                      <ul>
                        {desktopHandoffPackage.checklist.map((item) => (
                          <li key={item}>{item}</li>
                        ))}
                      </ul>
                      <div className="operate-desktop-handoff-actions">
                        <button
                          type="button"
                          className="secondary operate-action-button"
                          onClick={() => void handleCopyDesktopHandoffPrompt()}
                          disabled={actionLoading !== null}
                        >
                          <span className="operate-action-button-label">Copy handoff prompt</span>
                          <span className="operate-action-button-detail">
                            Copy the manual/operator prompt only; no GUI automation runs here.
                          </span>
                        </button>
                        <button
                          type="button"
                          className="secondary operate-action-button"
                          onClick={() => handleExportDesktopHandoff('json')}
                          disabled={actionLoading !== null}
                        >
                          <span className="operate-action-button-label">Export handoff JSON</span>
                          <span className="operate-action-button-detail">
                            Download a local blob with step metadata, checklist, payload, and review guidance.
                          </span>
                        </button>
                        <button
                          type="button"
                          className="secondary operate-action-button"
                          onClick={() => handleExportDesktopHandoff('md')}
                          disabled={actionLoading !== null}
                        >
                          <span className="operate-action-button-label">Export handoff Markdown</span>
                          <span className="operate-action-button-detail">
                            Download a manual/operator handoff brief without triggering desktop actions.
                          </span>
                        </button>
                      </div>
                      <p className="operate-desktop-handoff-copy-hint">
                        If clipboard access is unavailable, copy the manual/operator prompt directly from the <code>&lt;pre&gt;</code> block below.
                      </p>
                      <pre>{desktopHandoffPackage.handoff_prompt}</pre>
                    </div>
                  ) : null}
                  <div className="operate-desktop-review-panel">
                    <div className="operate-detail-field">
                      <span className="operate-detail-label">handoff state</span>
                      <span className="operate-detail-value">
                        {inspectedDesktopQueueItem?.handoff_reviewed
                          ? `reviewed · ${inspectedDesktopQueueItem.latest_reviewed_at ?? 'timestamp pending'}`
                          : inspectedDesktopQueueItem?.handoff_prepared
                            ? 'prepared, waiting for review'
                            : 'prepare handoff before review'}
                      </span>
                    </div>
                    <label className="operate-note-field">
                      <span>Review note</span>
                      <textarea
                        rows={2}
                        value={desktopHandoffReviewNote}
                        onChange={(event) => setDesktopHandoffReviewNote(event.target.value)}
                        placeholder="记录人工核对结果，例如窗口、输入、风险确认；不会触发 GUI 自动化"
                      />
                    </label>
                    <button
                      type="button"
                      className={`secondary operate-action-button${desktopHandoffReviewLoading ? ' is-loading' : ''}`}
                      onClick={() => void handleMarkDesktopHandoffReviewed()}
                      disabled={!inspectedDesktopQueueItem?.handoff_prepared || actionLoading !== null}
                      aria-busy={desktopHandoffReviewLoading}
                    >
                      <span className="operate-action-button-label">Mark handoff reviewed</span>
                      <span className="operate-action-button-detail">
                        {desktopHandoffReviewLoading ? <span className="operate-action-button-spinner" aria-hidden="true" /> : null}
                        Record an auditable review event for this exact desktop step.
                      </span>
                    </button>
                  </div>
                </div>
              ) : null}
              <div className="operate-note-panel">
                <div className="operate-detail-field">
                  <span className="operate-detail-label">latest user note</span>
                  <span className="operate-detail-value">{latestStepNote ?? '-'}</span>
                </div>
                <label className="operate-note-field">
                  <span>用户批注</span>
                  <textarea
                    rows={3}
                    value={stepNoteDraft}
                    onChange={(event) => setStepNoteDraft(event.target.value)}
                    placeholder="写入约束、复核意见或继续执行前必须考虑的信息"
                  />
                </label>
                <button
                  type="button"
                  className={`secondary operate-action-button${noteActionLoading ? ' is-loading' : ''}`}
                  onClick={() => void handleAddStepNote()}
                  disabled={!stepNoteDraft.trim() || actionLoading !== null}
                  aria-busy={noteActionLoading}
                >
                  <span className="operate-action-button-label">写入批注</span>
                  <span className="operate-action-button-detail">
                    {noteActionLoading ? <span className="operate-action-button-spinner" aria-hidden="true" /> : null}
                    运行中步骤会暂停，其他步骤会把批注注入执行上下文。
                  </span>
                </button>
              </div>
              <div className="operate-actions">
                <button
                  type="button"
                  onClick={handleApprove}
                  disabled={approveAction.disabled}
                  aria-busy={approveAction.busy}
                  className={`operate-action-button${approveAction.busy ? ' is-loading' : ''}${approveAction.disabled ? ' is-disabled' : ''}`}
                >
                  <span className="operate-action-button-label">{approveAction.label}</span>
                  <span className="operate-action-button-detail">
                    {approveAction.busy ? <span className="operate-action-button-spinner" aria-hidden="true" /> : null}
                    {approveAction.detail}
                  </span>
                </button>
                <button
                  type="button"
                  onClick={handleStart}
                  disabled={startAction.disabled}
                  aria-busy={startAction.busy}
                  className={`secondary operate-action-button${startAction.busy ? ' is-loading' : ''}${startAction.disabled ? ' is-disabled' : ''}`}
                >
                  <span className="operate-action-button-label">{startAction.label}</span>
                  <span className="operate-action-button-detail">
                    {startAction.busy ? <span className="operate-action-button-spinner" aria-hidden="true" /> : null}
                    {startAction.detail}
                  </span>
                </button>
                <button
                  type="button"
                  onClick={handleComplete}
                  disabled={completeAction.disabled}
                  aria-busy={completeAction.busy}
                  className={`secondary operate-action-button${completeAction.busy ? ' is-loading' : ''}${completeAction.disabled ? ' is-disabled' : ''}`}
                >
                  <span className="operate-action-button-label">{completeAction.label}</span>
                  <span className="operate-action-button-detail">
                    {completeAction.busy ? <span className="operate-action-button-spinner" aria-hidden="true" /> : null}
                    {completeAction.detail}
                  </span>
                </button>
              </div>
              <div className={`operate-action-feedback operate-action-feedback-${actionStatus.tone}`} aria-live="polite">
                <div className="operate-action-feedback-title">{actionStatus.title}</div>
                <div className="operate-action-feedback-text">{actionStatus.text}</div>
              </div>
            </div>
          ) : (
            <div className="operate-target-label">暂无可检视的执行步骤。</div>
          )}
        </section>

        <section className="operate-card">
          <h3>Completed</h3>
          {completedSteps.length === 0 ? (
            <div className="operate-target-label">暂无已完成步骤。</div>
          ) : (
            <div className="operate-list">
              {completedSteps.map((item) => (
                <div className="operate-list-item" key={item.id}>
                  <div className="operate-list-title">{item.title}</div>
                  <div className="operate-list-meta">
                    <span>{item.mode}</span>
                    <span>{item.risk_level}</span>
                    <span>{item.status}</span>
                  </div>
                  <div className="operate-completed-summary">{item.output_summary ?? '暂无 summary'}</div>
                </div>
              ))}
            </div>
          )}
        </section>

        <RecoveryStepSection
          tone="failed"
          items={failedSteps}
          actionLoading={actionLoading}
          onInspect={handleInspectStep}
          onPrimaryAction={(step) => void handleRecoveryAction(step, 'retry')}
          onSecondaryAction={handleInspectStep}
        />

        <RecoveryStepSection
          tone="paused"
          items={pausedSteps}
          actionLoading={actionLoading}
          onInspect={handleInspectStep}
          onPrimaryAction={(step) => void handleRecoveryAction(step, 'resume')}
          onSecondaryAction={handleInspectStep}
        />

        <RecoveryStepSection
          tone="skipped"
          items={skippedSteps}
          actionLoading={actionLoading}
          onInspect={handleInspectStep}
          onPrimaryAction={(step) => void handleRecoveryAction(step, 'rerun')}
          onSecondaryAction={(step) => void handleRecoveryAction(step, 'confirm-skip')}
        />

        <section className="operate-card">
          <h3>Recent Run Evidence</h3>
          <div className="operate-evidence">
            {recentEvidence.map((item) => (
              <div className="operate-evidence-item" key={item.title}>
                <div className="operate-evidence-title">{item.title}</div>
                <div className="operate-evidence-text">{item.text}</div>
              </div>
            ))}
          </div>
        </section>
      </div>
    </div>
  );
}

function extractLatestUserNote(inputPayload?: string | null): string | null {
  const parsed = parseInputPayload(inputPayload);
  if (!isRecord(parsed)) {
    return null;
  }

  if (typeof parsed.latest_user_note === 'string' && parsed.latest_user_note.trim()) {
    return parsed.latest_user_note;
  }
  if (Array.isArray(parsed.user_notes)) {
    const latest = parsed.user_notes[parsed.user_notes.length - 1] as { note?: unknown } | undefined;
    return typeof latest?.note === 'string' && latest.note.trim() ? latest.note : null;
  }

  return null;
}

function buildDesktopHandoffPackage(
  step: ExecutionStep,
  queueItem: ExecutionDesktopHandoffQueueItem | null,
  preparedHandoff: ExecutionDesktopHandoff | null,
): DesktopHandoffPackage {
  const parsedInputPayload = parseInputPayload(step.input_payload);
  const promptFromPayload =
    findFirstString(parsedInputPayload, [
      'handoff_prompt',
      'handoffPrompt',
      'desktop_handoff_prompt',
      'desktopPrompt',
      'operator_prompt',
      'operatorPrompt',
      'prompt',
    ]) ?? null;
  const checklistFromPayload =
    findFirstStringArray(parsedInputPayload, [
      'checklist',
      'handoff_checklist',
      'handoffChecklist',
      'desktop_checklist',
      'desktopChecklist',
      'operator_checklist',
      'operatorChecklist',
    ]) ?? [];
  const latestUserNote = extractLatestUserNote(step.input_payload);
  const fallbackChecklist = checklistFromPayload.length > 0 ? checklistFromPayload : buildFallbackChecklist(step);
  const reviewNoteGuidance =
    findFirstString(parsedInputPayload, [
      'review_note_guidance',
      'reviewNoteGuidance',
      'review_guidance',
      'reviewGuidance',
      'operator_review_note_guidance',
    ]) ??
    '记录人工/operator 实际检查到的窗口、输入、风险确认和任何偏差；该 review note 仅用于审计，不会触发 GUI 自动化。';

  return {
    source: preparedHandoff ? 'prepared' : 'derived',
    step_id: step.id,
    run_id: step.run_id,
    mission_id: step.mission_id,
    title: preparedHandoff?.title ?? step.title,
    risk: preparedHandoff?.risk_level ?? step.risk_level,
    status: preparedHandoff?.status ?? step.status,
    handoff_state: queueItem?.handoff_reviewed
      ? 'reviewed'
      : queueItem?.handoff_prepared || preparedHandoff
        ? 'prepared'
        : 'needs_handoff',
    input_payload: preparedHandoff?.input_payload ?? parsedInputPayload ?? step.input_payload ?? null,
    input_payload_raw: step.input_payload ?? null,
    checklist: preparedHandoff?.checklist?.length ? preparedHandoff.checklist : fallbackChecklist,
    handoff_prompt:
      preparedHandoff?.handoff_prompt ??
      promptFromPayload ??
      buildFallbackPrompt(step, parsedInputPayload, latestUserNote, fallbackChecklist, reviewNoteGuidance),
    review_note_guidance: reviewNoteGuidance,
    reason:
      preparedHandoff?.reason ??
      '本地构造的 manual/operator handoff package，用于补齐 copy/export 闭环；不会触发 GUI 自动化。',
    manual_operator_handoff: true,
    gui_automation_executed: false,
  };
}

function buildFallbackChecklist(step: ExecutionStep): string[] {
  return [
    `确认将由人工 operator 或外部 desktop runtime 执行“${step.title}”，当前页面不会执行 GUI 自动化。`,
    '在操作前核对输入 payload、最近用户批注和目标窗口状态。',
    '执行完成后，把实际观察、风险确认和任何偏差记录到 review note。',
  ];
}

function buildFallbackPrompt(
  step: ExecutionStep,
  parsedInputPayload: unknown,
  latestUserNote: string | null,
  checklist: string[],
  reviewNoteGuidance: string,
): string {
  const payloadBlock = formatPayloadForDisplay(parsedInputPayload ?? step.input_payload ?? null);
  return [
    `Manual/operator desktop handoff for "${step.title}".`,
    'This Operate page does not execute GUI automation. A human operator or a separate desktop runtime must perform the GUI work manually.',
    `Mission ID: ${step.mission_id}`,
    `Run ID: ${step.run_id}`,
    `Step ID: ${step.id}`,
    `Risk: ${step.risk_level}`,
    `Status: ${step.status}`,
    latestUserNote ? `Latest user note: ${latestUserNote}` : null,
    '',
    'Checklist:',
    ...checklist.map((item, index) => `${index + 1}. ${item}`),
    '',
    'Input payload:',
    payloadBlock,
    '',
    `Review note guidance: ${reviewNoteGuidance}`,
  ]
    .filter((line): line is string => line !== null)
    .join('\n');
}

function buildDesktopHandoffMarkdown(handoffPackage: DesktopHandoffPackage): string {
  return [
    `# Desktop Manual/Operator Handoff`,
    '',
    '> This package is for manual/operator handoff only. It does not trigger GUI automation.',
    '',
    `- Step ID: ${handoffPackage.step_id}`,
    `- Run ID: ${handoffPackage.run_id}`,
    `- Mission ID: ${handoffPackage.mission_id}`,
    `- Title: ${handoffPackage.title}`,
    `- Risk: ${handoffPackage.risk}`,
    `- Status: ${handoffPackage.status}`,
    `- Handoff State: ${handoffPackage.handoff_state}`,
    `- Source: ${handoffPackage.source}`,
    '',
    '## Reason',
    '',
    handoffPackage.reason,
    '',
    '## Checklist',
    '',
    ...handoffPackage.checklist.map((item) => `- ${item}`),
    '',
    '## Review Note Guidance',
    '',
    handoffPackage.review_note_guidance,
    '',
    '## Handoff Prompt',
    '',
    '```text',
    handoffPackage.handoff_prompt,
    '```',
    '',
    '## Input Payload',
    '',
    '```json',
    formatPayloadForDisplay(handoffPackage.input_payload),
    '```',
    '',
  ].join('\n');
}

function downloadTextFile(filename: string, content: string, mimeType: string) {
  const blob = new Blob([content], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}

function parseInputPayload(inputPayload?: string | null): unknown | null {
  if (!inputPayload) {
    return null;
  }

  try {
    return JSON.parse(inputPayload) as unknown;
  } catch {
    return null;
  }
}

function findFirstString(value: unknown, keys: string[]): string | null {
  const candidate = findFirstValue(value, keys);
  return typeof candidate === 'string' && candidate.trim() ? candidate : null;
}

function findFirstStringArray(value: unknown, keys: string[]): string[] | null {
  const candidate = findFirstValue(value, keys);
  if (!Array.isArray(candidate)) {
    return null;
  }

  const items = candidate.filter((item): item is string => typeof item === 'string' && item.trim().length > 0);
  return items.length > 0 ? items : null;
}

function findFirstValue(value: unknown, keys: string[]): unknown {
  if (Array.isArray(value)) {
    for (const item of value) {
      const nested = findFirstValue(item, keys);
      if (nested !== undefined) {
        return nested;
      }
    }
    return undefined;
  }

  if (!isRecord(value)) {
    return undefined;
  }

  for (const key of keys) {
    const direct = value[key];
    if (direct !== undefined) {
      return direct;
    }
  }

  for (const nestedValue of Object.values(value)) {
    const nested = findFirstValue(nestedValue, keys);
    if (nested !== undefined) {
      return nested;
    }
  }

  return undefined;
}

function formatPayloadForDisplay(payload: unknown): string {
  if (payload == null) {
    return 'null';
  }

  if (typeof payload === 'string') {
    return payload;
  }

  try {
    return JSON.stringify(payload, null, 2);
  } catch {
    return String(payload);
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function toFileSlug(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
}
