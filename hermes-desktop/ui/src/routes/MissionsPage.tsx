import { useEffect, useMemo, useState } from 'react';
import {
  councilStepCreate,
  councilStepList,
  memoryRecordCreate,
  memoryRecordList,
  missionCreate,
  missionGeneratePlan,
  missionGet,
  missionList,
  missionSetPinned,
  missionSetStatus,
  missionUpdate,
  playbookGet,
  runEventList,
  trajectoryExportDataset,
  type CouncilStepItem,
  type MemoryRecordItem,
  type Mission,
  type MissionDetail,
  type MissionPlaybook,
  type MissionPriority,
  type MissionStatus,
  type RunEventItem,
  type TrajectoryDatasetExport,
} from '../lib/tauri';
import { useMissionStore } from '../store/missionStore';
import './MissionsPage.css';

const editableStatuses: MissionStatus[] = [
  'draft',
  'researching',
  'simulating',
  'planning',
  'awaiting_approval',
  'executing',
  'paused',
  'completed',
  'failed',
  'archived',
];

const initialForm = {
  title: '',
  goal: '',
  constraints: '',
  successCriteria: '',
  priority: 'medium' as MissionPriority,
};

const allStatusFilter = 'all';
const trajectoryPreviewLineLimit = 5;
type TrajectoryCopyState = {
  status: 'idle' | 'success' | 'error';
  message: string | null;
};
const trajectoryCopyIdleState = {
  status: 'idle' as const,
  message: null as string | null,
};

export function MissionsPage() {
  const missions = useMissionStore((state) => state.missions);
  const selectedId = useMissionStore((state) => state.selectedMissionId);
  const setMissions = useMissionStore((state) => state.setMissions);
  const prependMission = useMissionStore((state) => state.prependMission);
  const selectMission = useMissionStore((state) => state.selectMission);
  const [loading, setLoading] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [savingMission, setSavingMission] = useState(false);
  const [planGenerating, setPlanGenerating] = useState(false);
  const [missionActionId, setMissionActionId] = useState<string | null>(null);
  const [query, setQuery] = useState('');
  const [statusFilter, setStatusFilter] = useState<MissionStatus | typeof allStatusFilter>(
    allStatusFilter,
  );
  const [error, setError] = useState<string | null>(null);
  const [form, setForm] = useState(initialForm);
  const [editForm, setEditForm] = useState(initialForm);
  const [detail, setDetail] = useState<MissionDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [timeline, setTimeline] = useState<RunEventItem[]>([]);
  const [trajectoryExport, setTrajectoryExport] = useState<TrajectoryDatasetExport | null>(null);
  const [trajectoryExporting, setTrajectoryExporting] = useState(false);
  const [trajectoryCopyState, setTrajectoryCopyState] =
    useState<TrajectoryCopyState>(trajectoryCopyIdleState);
  const [playbook, setPlaybook] = useState<MissionPlaybook | null>(null);
  const [memoryRecords, setMemoryRecords] = useState<MemoryRecordItem[]>([]);
  const [councilSteps, setCouncilSteps] = useState<CouncilStepItem[]>([]);
  const [memoryForm, setMemoryForm] = useState({
    title: '',
    content: '',
    importance: 'medium',
  });
  const [memorySaving, setMemorySaving] = useState(false);
  const [councilForm, setCouncilForm] = useState({
    runId: '',
    role: 'critic',
    status: 'pending',
    inputSummary: '',
    outputSummary: '',
    reviewNote: '',
  });
  const [councilSaving, setCouncilSaving] = useState(false);

  useEffect(() => {
    void loadMissions();
  }, [query, statusFilter]);

  const selectedMission = useMemo(
    () => missions.find((mission) => mission.id === selectedId) ?? null,
    [missions, selectedId],
  );
  const trajectoryReview = useMemo(
    () => parseTrajectoryJsonl(trajectoryExport?.jsonl ?? ''),
    [trajectoryExport?.jsonl],
  );
  const trajectoryHasRows = trajectoryReview.totalLineCount > 0;
  const trajectoryValidRowCount =
    trajectoryReview.totalLineCount - trajectoryReview.invalidLineCount;

  useEffect(() => {
    if (!selectedMission) {
      setEditForm(initialForm);
      return;
    }

    setEditForm(missionToForm(selectedMission));
  }, [selectedMission]);

  useEffect(() => {
    if (!selectedId) {
      setDetail(null);
      setTimeline([]);
      setTrajectoryExport(null);
      setTrajectoryCopyState(trajectoryCopyIdleState);
      setPlaybook(null);
      setMemoryRecords([]);
      setCouncilSteps([]);
      return;
    }
    const missionId = selectedId;

    let cancelled = false;
    async function loadDetail() {
      setDetailLoading(true);
      try {
        const [detailData, timelineData, playbookData, memoryData, councilData] = await Promise.all([
          missionGet(missionId),
          runEventList(missionId),
          playbookGet(missionId),
          memoryRecordList({ scope: 'mission', scope_ref: missionId }),
          councilStepList(missionId),
        ]);
        if (!cancelled) {
          setDetail(detailData);
          setTimeline(timelineData);
          setPlaybook(playbookData);
          setMemoryRecords(memoryData);
          setCouncilSteps(councilData);
          setCouncilForm((current) => ({
            ...current,
            runId: detailData?.runs[0]?.id ?? '',
          }));
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err));
        }
      } finally {
        if (!cancelled) {
          setDetailLoading(false);
        }
      }
    }

    void loadDetail();
    return () => {
      cancelled = true;
    };
  }, [selectedId]);

  async function loadMissions() {
    setLoading(true);
    setError(null);
    try {
      const items = await missionList({
        query: query.trim() || undefined,
        status: statusFilter === allStatusFilter ? undefined : statusFilter,
        limit: 50,
      });
      setMissions(items);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }

  function mergeMission(updated: Mission) {
    setMissions(missions.map((mission) => (mission.id === updated.id ? updated : mission)));
    setDetail((current) =>
      current && current.mission.id === updated.id ? { ...current, mission: updated } : current,
    );
  }

  async function handleCreate(e: React.FormEvent) {
    e.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      const created = await missionCreate({
        title: form.title,
        goal: form.goal,
        constraints: splitLines(form.constraints),
        success_criteria: splitLines(form.successCriteria),
        priority: form.priority,
      });

      prependMission(created);
      setForm(initialForm);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSubmitting(false);
    }
  }

  async function handleUpdateMission(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedMission) {
      return;
    }

    setSavingMission(true);
    setError(null);
    try {
      const updated = await missionUpdate({
        id: selectedMission.id,
        title: editForm.title,
        goal: editForm.goal,
        constraints: splitLines(editForm.constraints),
        success_criteria: splitLines(editForm.successCriteria),
        priority: editForm.priority,
      });
      mergeMission(updated);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSavingMission(false);
    }
  }

  async function handleTogglePinned(mission: Mission) {
    setMissionActionId(mission.id);
    setError(null);
    try {
      const updated = await missionSetPinned({
        id: mission.id,
        pinned: !mission.pinned,
      });
      mergeMission(updated);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setMissionActionId(null);
    }
  }

  async function handleSetMissionStatus(mission: Mission, status: MissionStatus) {
    setMissionActionId(mission.id);
    setError(null);
    try {
      const updated = await missionSetStatus({
        id: mission.id,
        status,
      });
      mergeMission(updated);
      if (statusFilter !== allStatusFilter && statusFilter !== status) {
        setMissions(missions.filter((item) => item.id !== mission.id));
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setMissionActionId(null);
    }
  }

  async function handleGeneratePlan(mission: Mission) {
    setPlanGenerating(true);
    setError(null);
    try {
      await missionGeneratePlan(mission.id);
      const detailData = await missionGet(mission.id);
      if (detailData) {
        mergeMission(detailData.mission);
        setDetail(detailData);
      }
      await loadMissions();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setPlanGenerating(false);
    }
  }

  async function handleExportTrajectory() {
    if (!selectedId) {
      return;
    }
    setTrajectoryExporting(true);
    setError(null);
    try {
      const exported = await trajectoryExportDataset({
        mission_id: selectedId,
        include_session_messages: true,
      });
      setTrajectoryExport(exported);
      setTrajectoryCopyState(trajectoryCopyIdleState);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setTrajectoryExporting(false);
    }
  }

  async function handleCopyTrajectoryJsonl() {
    if (!trajectoryExport?.jsonl) {
      setTrajectoryCopyState({
        status: 'error',
        message: '当前没有可复制的 JSONL 内容。',
      });
      return;
    }

    if (!navigator.clipboard?.writeText) {
      setTrajectoryCopyState({
        status: 'error',
        message: '当前环境不支持剪贴板写入。请直接选择下方 JSONL 内容手动复制。',
      });
      return;
    }

    try {
      await navigator.clipboard.writeText(trajectoryExport.jsonl);
      setTrajectoryCopyState({
        status: 'success',
        message: `已复制 ${trajectoryReview.totalLineCount} 行 JSONL 到剪贴板，可用于本地 dataset review / replay preparation。`,
      });
    } catch (err) {
      setTrajectoryCopyState({
        status: 'error',
        message:
          err instanceof Error
            ? `复制失败：${err.message}。请直接选择下方 JSONL 内容手动复制。`
            : '复制失败。请直接选择下方 JSONL 内容手动复制。',
      });
    }
  }

  async function handleCreateMemoryRecord(event: React.FormEvent) {
    event.preventDefault();
    if (!selectedId) {
      return;
    }
    setMemorySaving(true);
    setError(null);
    try {
      const created = await memoryRecordCreate({
        scope: 'mission',
        scope_ref: selectedId,
        title: memoryForm.title,
        content: memoryForm.content,
        source_type: 'manual',
        importance: memoryForm.importance,
      });
      setMemoryRecords((current) => [created, ...current]);
      setMemoryForm({
        title: '',
        content: '',
        importance: 'medium',
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setMemorySaving(false);
    }
  }

  async function handleCreateCouncilStep(event: React.FormEvent) {
    event.preventDefault();
    if (!selectedId) {
      return;
    }
    setCouncilSaving(true);
    setError(null);
    try {
      const created = await councilStepCreate({
        mission_id: selectedId,
        run_id: councilForm.runId,
        role: councilForm.role,
        status: councilForm.status,
        input_summary: councilForm.inputSummary || null,
        output_summary: councilForm.outputSummary || null,
        review_note: councilForm.reviewNote || null,
      });
      setCouncilSteps((current) => [created, ...current]);
      setCouncilForm((current) => ({
        ...current,
        inputSummary: '',
        outputSummary: '',
        reviewNote: '',
      }));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setCouncilSaving(false);
    }
  }

  return (
    <div className="missions-page">
      <h2>Missions</h2>
      {error ? <div className="missions-error">{error}</div> : null}
      <div className="missions-layout">
        <section className="missions-card">
          <h3>新建 Mission</h3>
          <form className="missions-form" onSubmit={handleCreate}>
            <input
              value={form.title}
              onChange={(e) => setForm((prev) => ({ ...prev, title: e.target.value }))}
              placeholder="标题"
            />
            <textarea
              value={form.goal}
              onChange={(e) => setForm((prev) => ({ ...prev, goal: e.target.value }))}
              placeholder="目标"
            />
            <textarea
              value={form.constraints}
              onChange={(e) => setForm((prev) => ({ ...prev, constraints: e.target.value }))}
              placeholder="约束，一行一个"
            />
            <textarea
              value={form.successCriteria}
              onChange={(e) =>
                setForm((prev) => ({ ...prev, successCriteria: e.target.value }))
              }
              placeholder="成功标准，一行一个"
            />
            <select
              value={form.priority}
              onChange={(e) =>
                setForm((prev) => ({
                  ...prev,
                  priority: e.target.value as MissionPriority,
                }))
              }
            >
              <option value="low">Low</option>
              <option value="medium">Medium</option>
              <option value="high">High</option>
            </select>
            <button type="submit" disabled={submitting}>
              {submitting ? '创建中...' : '创建 Mission'}
            </button>
          </form>
        </section>

        <section className="missions-card">
          <div className="missions-list-header">
            <div>
              <h3>Mission 列表</h3>
              <p>支持搜索、状态筛选、Pin/Unpin 与归档。</p>
            </div>
            <button type="button" className="missions-refresh" onClick={() => void loadMissions()}>
              刷新
            </button>
          </div>
          <div className="missions-filters">
            <input
              type="search"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="搜索标题或目标"
            />
            <select
              value={statusFilter}
              onChange={(event) =>
                setStatusFilter(event.target.value as MissionStatus | typeof allStatusFilter)
              }
            >
              <option value={allStatusFilter}>All statuses</option>
              {editableStatuses.map((status) => (
                <option key={status} value={status}>
                  {status}
                </option>
              ))}
            </select>
          </div>
          {loading ? <p className="missions-empty">加载中...</p> : null}
          {!loading && missions.length === 0 ? (
            <p className="missions-empty">当前筛选条件下没有 Mission。</p>
          ) : null}
          {!loading && missions.length > 0 ? (
            <div className="missions-list">
              {missions.map((mission) => (
                <button
                  key={mission.id}
                  type="button"
                  className={`mission-item ${selectedId === mission.id ? 'active' : ''}`}
                  onClick={() => selectMission(mission.id)}
                >
                  <div className="mission-item-title">
                    {mission.pinned ? <span className="mission-pin">PIN</span> : null}
                    {mission.title}
                  </div>
                  <div className="mission-item-meta">
                    <span>{mission.status}</span>
                    <span>{mission.priority}</span>
                    <span>{mission.last_activity_at}</span>
                  </div>
                </button>
              ))}
            </div>
          ) : null}

          {selectedMission ? (
            <div className="mission-detail">
              <div className="mission-detail-title-row">
                <div>
                  <h3>{selectedMission.title}</h3>
                  <p>
                    {selectedMission.status} · {selectedMission.priority}
                  </p>
                </div>
                <div className="mission-actions">
                  <button
                    type="button"
                    onClick={() => void handleGeneratePlan(selectedMission)}
                    disabled={planGenerating || selectedMission.status === 'archived'}
                  >
                    {planGenerating ? 'Generating...' : 'Generate Plan'}
                  </button>
                  <button
                    type="button"
                    onClick={() => void handleTogglePinned(selectedMission)}
                    disabled={missionActionId === selectedMission.id}
                  >
                    {selectedMission.pinned ? 'Unpin' : 'Pin'}
                  </button>
                  <button
                    type="button"
                    className="mission-danger-action"
                    onClick={() => void handleSetMissionStatus(selectedMission, 'archived')}
                    disabled={
                      missionActionId === selectedMission.id ||
                      selectedMission.status === 'archived'
                    }
                  >
                    Archive
                  </button>
                </div>
              </div>

              <form className="missions-edit-form" onSubmit={handleUpdateMission}>
                <h4>编辑 Mission</h4>
                <input
                  value={editForm.title}
                  onChange={(e) => setEditForm((prev) => ({ ...prev, title: e.target.value }))}
                  placeholder="标题"
                />
                <textarea
                  value={editForm.goal}
                  onChange={(e) => setEditForm((prev) => ({ ...prev, goal: e.target.value }))}
                  placeholder="目标"
                />
                <textarea
                  value={editForm.constraints}
                  onChange={(e) =>
                    setEditForm((prev) => ({ ...prev, constraints: e.target.value }))
                  }
                  placeholder="约束，一行一个"
                />
                <textarea
                  value={editForm.successCriteria}
                  onChange={(e) =>
                    setEditForm((prev) => ({ ...prev, successCriteria: e.target.value }))
                  }
                  placeholder="成功标准，一行一个"
                />
                <div className="missions-edit-row">
                  <select
                    value={editForm.priority}
                    onChange={(e) =>
                      setEditForm((prev) => ({
                        ...prev,
                        priority: e.target.value as MissionPriority,
                      }))
                    }
                  >
                    <option value="low">Low</option>
                    <option value="medium">Medium</option>
                    <option value="high">High</option>
                  </select>
                  <select
                    value={selectedMission.status}
                    onChange={(event) =>
                      void handleSetMissionStatus(
                        selectedMission,
                        event.target.value as MissionStatus,
                      )
                    }
                    disabled={missionActionId === selectedMission.id}
                  >
                    {editableStatuses.map((status) => (
                      <option key={status} value={status}>
                        {status}
                      </option>
                    ))}
                  </select>
                </div>
                <button type="submit" disabled={savingMission}>
                  {savingMission ? '保存中...' : '保存 Mission'}
                </button>
              </form>

              <div className="detail-block">
                <h4>目标</h4>
                <p>{selectedMission.goal}</p>
              </div>
              <div className="detail-block">
                <h4>约束</h4>
                {selectedMission.constraints.length > 0 ? (
                  <ul>
                    {selectedMission.constraints.map((constraint) => (
                      <li key={constraint}>{constraint}</li>
                    ))}
                  </ul>
                ) : (
                  <p>暂无约束</p>
                )}
              </div>
              <div className="detail-block">
                <h4>成功标准</h4>
                {selectedMission.success_criteria.length > 0 ? (
                  <ul>
                    {selectedMission.success_criteria.map((criterion) => (
                      <li key={criterion}>{criterion}</li>
                    ))}
                  </ul>
                ) : (
                  <p>暂无成功标准</p>
                )}
              </div>
              <div className="detail-block">
                <h4>Context Items</h4>
                {detailLoading ? <p>加载中...</p> : null}
                {!detailLoading && detail && detail.context_items.length > 0 ? (
                  <ul>
                    {detail.context_items.map((item) => (
                      <li key={item.id}>{item.title}</li>
                    ))}
                  </ul>
                ) : null}
                {!detailLoading && (!detail || detail.context_items.length === 0) ? (
                  <p>暂无 context items</p>
                ) : null}
              </div>
              <div className="detail-summary-grid">
                <div className="detail-summary-card">
                  <span className="detail-summary-value">{detail?.runs.length ?? 0}</span>
                  <span className="detail-summary-label">Runs</span>
                </div>
                <div className="detail-summary-card">
                  <span className="detail-summary-value">{detail?.artifacts.length ?? 0}</span>
                  <span className="detail-summary-label">Artifacts</span>
                </div>
              </div>
              <div className="detail-block">
                <h4>Playbook / Growth</h4>
                {playbook ? <p>{playbook.summary}</p> : <p>正在生成 mission playbook...</p>}
                {playbook && playbook.suggestions.length > 0 ? (
                  <div className="detail-list">
                    {playbook.suggestions.map((item) => (
                      <div className="detail-list-item" key={item.id}>
                        <strong>{item.title}</strong>
                        <span>{item.priority}</span>
                        <span>{item.rationale}</span>
                        {item.actions.length > 0 ? (
                          <span>{item.actions.join(' · ')}</span>
                        ) : null}
                      </div>
                    ))}
                  </div>
                ) : null}
                {playbook && playbook.evidence_cards.length > 0 ? (
                  <div className="detail-list">
                    {playbook.evidence_cards.map((card) => (
                      <div className="detail-list-item" key={card.id}>
                        <strong>{card.title}</strong>
                        <span>{card.summary}</span>
                        {card.bullets.length > 0 ? <span>{card.bullets.join(' · ')}</span> : null}
                      </div>
                    ))}
                  </div>
                ) : null}
              </div>
              <div className="detail-block">
                <div className="detail-block-header">
                  <div>
                    <h4>Timeline</h4>
                    <p>
                      Export local run, step, event, and session-message evidence as JSONL for
                      local dataset review / replay preparation. This does not start RL training.
                    </p>
                  </div>
                  <button
                    className="detail-secondary-action"
                    type="button"
                    onClick={() => void handleExportTrajectory()}
                    disabled={!selectedId || trajectoryExporting}
                  >
                    {trajectoryExporting ? 'Exporting...' : 'Export trajectory JSONL'}
                  </button>
                </div>
                {detailLoading ? <p>加载中...</p> : null}
                {!detailLoading && timeline.length === 0 ? <p>暂无 timeline 事件</p> : null}
                {timeline.length > 0 ? (
                  <div className="detail-timeline">
                    {timeline.map((event) => (
                      <div className="detail-timeline-item" key={event.id}>
                        <div className="detail-timeline-title">{event.event_type}</div>
                        <div className="detail-timeline-text">{event.message}</div>
                        <div className="detail-timeline-meta">
                          run={event.run_id} · {event.created_at}
                        </div>
                      </div>
                    ))}
                  </div>
                ) : null}
                {trajectoryExport ? (
                  <div className="trajectory-export-panel">
                    <div className="trajectory-export-meta">
                      <div>
                        <strong>{trajectoryExport.item_count} JSONL items</strong>
                        <span>
                          schema v{trajectoryExport.schema_version} · {trajectoryExport.exported_at}
                        </span>
                      </div>
                      <button
                        type="button"
                        className="detail-secondary-action"
                        onClick={() => void handleCopyTrajectoryJsonl()}
                        disabled={!trajectoryExport.jsonl}
                      >
                        Copy JSONL
                      </button>
                    </div>
                    <div className="trajectory-export-note">
                      This panel is for local dataset review / replay preparation only. It helps
                      inspect exported evidence before reuse and does not perform RL training.
                    </div>
                    {trajectoryCopyState.message ? (
                      <div
                        className={`trajectory-export-status trajectory-export-status-${trajectoryCopyState.status}`}
                      >
                        {trajectoryCopyState.message}
                      </div>
                    ) : null}
                    {trajectoryHasRows ? (
                      <>
                        <div className="trajectory-export-summary-grid">
                          <div className="trajectory-export-summary-card">
                            <span className="trajectory-export-summary-value">
                              {trajectoryValidRowCount}
                            </span>
                            <span className="trajectory-export-summary-label">valid rows</span>
                          </div>
                          <div className="trajectory-export-summary-card">
                            <span className="trajectory-export-summary-value">
                              {trajectoryReview.rewardHintCount}
                            </span>
                            <span className="trajectory-export-summary-label">reward_hint rows</span>
                          </div>
                          <div className="trajectory-export-summary-card">
                            <span className="trajectory-export-summary-value">
                              {trajectoryReview.invalidLineCount}
                            </span>
                            <span className="trajectory-export-summary-label">invalid rows</span>
                          </div>
                          <div className="trajectory-export-summary-card">
                            <span className="trajectory-export-summary-value">
                              {trajectoryReview.totalLineCount}
                            </span>
                            <span className="trajectory-export-summary-label">non-empty rows</span>
                          </div>
                        </div>

                        <div className="trajectory-export-counts">
                          <div className="trajectory-export-count-group">
                            <h5>Kind counts</h5>
                            {Object.keys(trajectoryReview.kindCounts).length > 0 ? (
                              <div className="trajectory-export-chip-list">
                                {Object.entries(trajectoryReview.kindCounts).map(([kind, count]) => (
                                  <span className="trajectory-export-chip" key={kind}>
                                    {kind}: {count}
                                  </span>
                                ))}
                              </div>
                            ) : (
                              <p>没有可统计的 kind 字段。</p>
                            )}
                          </div>
                          <div className="trajectory-export-count-group">
                            <h5>Source counts</h5>
                            {Object.keys(trajectoryReview.sourceCounts).length > 0 ? (
                              <div className="trajectory-export-chip-list">
                                {Object.entries(trajectoryReview.sourceCounts).map(
                                  ([source, count]) => (
                                    <span className="trajectory-export-chip" key={source}>
                                      {source}: {count}
                                    </span>
                                  ),
                                )}
                              </div>
                            ) : (
                              <p>没有可统计的 source 字段。</p>
                            )}
                          </div>
                        </div>

                        <div className="trajectory-export-preview-section">
                          <div className="trajectory-export-preview-header">
                            <h5>Recent preview</h5>
                            <span>
                              最近 {trajectoryReview.recentRows.length} 行，按导出顺序倒序展示
                            </span>
                          </div>
                          <div className="trajectory-export-preview-list">
                            {trajectoryReview.recentRows.map((row) => (
                              <div className="trajectory-export-preview-item" key={row.key}>
                                <div className="trajectory-export-preview-meta">
                                  <strong>Line {row.lineNumber}</strong>
                                  <span>{row.kindLabel}</span>
                                  <span>{row.sourceLabel}</span>
                                  {row.hasRewardHint ? <span>reward_hint</span> : null}
                                  {row.isInvalid ? (
                                    <span className="trajectory-export-preview-invalid">
                                      invalid JSON
                                    </span>
                                  ) : null}
                                </div>
                                <pre>{row.raw}</pre>
                              </div>
                            ))}
                          </div>
                        </div>
                      </>
                    ) : (
                      <div className="trajectory-export-empty-state">
                        <strong>No exported trajectory rows yet.</strong>
                        <p>
                          当前 Mission 还没有可回放的本地 evidence rows。导出按钮仍可继续使用；
                          当 run / step / event / session message 出现后，这里会展示 review
                          summary 和 preview。
                        </p>
                      </div>
                    )}
                    <pre>{trajectoryExport.jsonl || 'No trajectory items for this Mission yet.'}</pre>
                  </div>
                ) : (
                  <div className="trajectory-export-panel trajectory-export-panel-empty">
                    <div className="trajectory-export-empty-state">
                      <strong>Export trajectory JSONL to start local review.</strong>
                      <p>
                        这里会在导出后展示 JSONL 的 kind/source 计数、reward_hint 行数、invalid
                        行数以及最近几行 preview，帮助你做本地 dataset review / replay
                        preparation，而不是 RL training。
                      </p>
                    </div>
                  </div>
                )}
              </div>
              <div className="detail-block">
                <h4>Council</h4>
                <form className="detail-inline-form" onSubmit={handleCreateCouncilStep}>
                  <select
                    value={councilForm.runId}
                    onChange={(e) =>
                      setCouncilForm((prev) => ({ ...prev, runId: e.target.value }))
                    }
                  >
                    <option value="" disabled>
                      Select run
                    </option>
                    {detail?.runs.map((run) => (
                      <option key={run.id} value={run.id}>
                        {run.id} · {run.type}
                      </option>
                    ))}
                  </select>
                  <select
                    value={councilForm.role}
                    onChange={(e) =>
                      setCouncilForm((prev) => ({ ...prev, role: e.target.value }))
                    }
                  >
                    <option value="scout">scout</option>
                    <option value="analyst">analyst</option>
                    <option value="critic">critic</option>
                    <option value="planner">planner</option>
                    <option value="executor">executor</option>
                    <option value="reviewer">reviewer</option>
                  </select>
                  <select
                    value={councilForm.status}
                    onChange={(e) =>
                      setCouncilForm((prev) => ({ ...prev, status: e.target.value }))
                    }
                  >
                    <option value="pending">pending</option>
                    <option value="running">running</option>
                    <option value="completed">completed</option>
                    <option value="rejected">rejected</option>
                    <option value="failed">failed</option>
                  </select>
                  <input
                    value={councilForm.inputSummary}
                    onChange={(e) =>
                      setCouncilForm((prev) => ({ ...prev, inputSummary: e.target.value }))
                    }
                    placeholder="Input summary"
                  />
                  <input
                    value={councilForm.outputSummary}
                    onChange={(e) =>
                      setCouncilForm((prev) => ({ ...prev, outputSummary: e.target.value }))
                    }
                    placeholder="Output summary"
                  />
                  <input
                    value={councilForm.reviewNote}
                    onChange={(e) =>
                      setCouncilForm((prev) => ({ ...prev, reviewNote: e.target.value }))
                    }
                    placeholder="Review note"
                  />
                  <button type="submit" disabled={councilSaving || !councilForm.runId}>
                    {councilSaving ? '保存中...' : '新增 Council Step'}
                  </button>
                </form>
                {councilSteps.length === 0 ? <p>暂无 council steps</p> : null}
                {councilSteps.length > 0 ? (
                  <div className="detail-list">
                    {councilSteps.map((step) => (
                      <div className="detail-list-item" key={step.id}>
                        <strong>{step.role}</strong>
                        <span>{step.status}</span>
                        <span>{step.review_note ?? step.output_summary ?? step.input_summary ?? '-'}</span>
                      </div>
                    ))}
                  </div>
                ) : null}
              </div>
              <div className="detail-block">
                <h4>Memory</h4>
                <form className="detail-inline-form" onSubmit={handleCreateMemoryRecord}>
                  <input
                    value={memoryForm.title}
                    onChange={(e) =>
                      setMemoryForm((prev) => ({ ...prev, title: e.target.value }))
                    }
                    placeholder="Memory title"
                  />
                  <textarea
                    value={memoryForm.content}
                    onChange={(e) =>
                      setMemoryForm((prev) => ({ ...prev, content: e.target.value }))
                    }
                    placeholder="Memory content"
                  />
                  <select
                    value={memoryForm.importance}
                    onChange={(e) =>
                      setMemoryForm((prev) => ({ ...prev, importance: e.target.value }))
                    }
                  >
                    <option value="low">low</option>
                    <option value="medium">medium</option>
                    <option value="high">high</option>
                  </select>
                  <button type="submit" disabled={memorySaving}>
                    {memorySaving ? '保存中...' : '新增 Memory'}
                  </button>
                </form>
                {memoryRecords.length === 0 ? <p>暂无 memory records</p> : null}
                {memoryRecords.length > 0 ? (
                  <div className="detail-list">
                    {memoryRecords.map((record) => (
                      <div className="detail-list-item" key={record.id}>
                        <strong>{record.title}</strong>
                        <span>{record.importance}</span>
                        <span>{record.content}</span>
                      </div>
                    ))}
                  </div>
                ) : null}
              </div>
            </div>
          ) : null}
        </section>
      </div>
    </div>
  );
}

function splitLines(value: string): string[] {
  return value
    .split('\n')
    .map((item) => item.trim())
    .filter(Boolean);
}

function missionToForm(mission: Mission) {
  return {
    title: mission.title,
    goal: mission.goal,
    constraints: mission.constraints.join('\n'),
    successCriteria: mission.success_criteria.join('\n'),
    priority: mission.priority,
  };
}

type TrajectoryJsonObject = Record<string, unknown>;

interface TrajectoryPreviewRow {
  key: string;
  lineNumber: number;
  raw: string;
  kindLabel: string;
  sourceLabel: string;
  hasRewardHint: boolean;
  isInvalid: boolean;
}

interface TrajectoryReviewSummary {
  totalLineCount: number;
  invalidLineCount: number;
  rewardHintCount: number;
  kindCounts: Record<string, number>;
  sourceCounts: Record<string, number>;
  recentRows: TrajectoryPreviewRow[];
}

function parseTrajectoryJsonl(jsonl: string): TrajectoryReviewSummary {
  const kindCounts: Record<string, number> = {};
  const sourceCounts: Record<string, number> = {};
  const previewRows: TrajectoryPreviewRow[] = [];
  let totalLineCount = 0;
  let invalidLineCount = 0;
  let rewardHintCount = 0;

  jsonl.split('\n').forEach((rawLine, index) => {
    const line = rawLine.trim();
    if (!line) {
      return;
    }

    totalLineCount += 1;
    const lineNumber = index + 1;
    const key = `${lineNumber}:${line.slice(0, 48)}`;

    try {
      const parsed = JSON.parse(line) as unknown;
      if (!isTrajectoryJsonObject(parsed)) {
        invalidLineCount += 1;
        previewRows.push({
          key,
          lineNumber,
          raw: rawLine,
          kindLabel: 'kind: invalid',
          sourceLabel: 'source: invalid',
          hasRewardHint: false,
          isInvalid: true,
        });
        return;
      }

      const kind = readTrajectoryString(parsed, [
        ['kind'],
      ]) ?? 'unknown';
      const source = readTrajectoryString(parsed, [
        ['source'],
        ['session', 'source'],
        ['payload', 'source'],
        ['metadata', 'source'],
        ['input_payload', 'source'],
      ]) ?? 'unknown';
      const hasRewardHint =
        hasTrajectoryValue(parsed, ['reward_hint']) ||
        hasTrajectoryValue(parsed, ['payload', 'reward_hint']) ||
        hasTrajectoryValue(parsed, ['metadata', 'reward_hint']) ||
        hasTrajectoryValue(parsed, ['input_payload', 'reward_hint']);

      kindCounts[kind] = (kindCounts[kind] ?? 0) + 1;
      sourceCounts[source] = (sourceCounts[source] ?? 0) + 1;
      if (hasRewardHint) {
        rewardHintCount += 1;
      }

      previewRows.push({
        key,
        lineNumber,
        raw: rawLine,
        kindLabel: `kind: ${kind}`,
        sourceLabel: `source: ${source}`,
        hasRewardHint,
        isInvalid: false,
      });
    } catch {
      invalidLineCount += 1;
      previewRows.push({
        key,
        lineNumber,
        raw: rawLine,
        kindLabel: 'kind: invalid',
        sourceLabel: 'source: invalid',
        hasRewardHint: false,
        isInvalid: true,
      });
    }
  });

  return {
    totalLineCount,
    invalidLineCount,
    rewardHintCount,
    kindCounts,
    sourceCounts,
    recentRows: previewRows.slice(-trajectoryPreviewLineLimit).reverse(),
  };
}

function isTrajectoryJsonObject(value: unknown): value is TrajectoryJsonObject {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

function readTrajectoryString(
  value: TrajectoryJsonObject,
  paths: string[][],
): string | null {
  for (const path of paths) {
    const candidate = readTrajectoryValue(value, path);
    if (typeof candidate === 'string' && candidate.trim()) {
      return candidate;
    }
  }

  return null;
}

function hasTrajectoryValue(value: TrajectoryJsonObject, path: string[]): boolean {
  const candidate = readTrajectoryValue(value, path);
  return candidate !== undefined && candidate !== null;
}

function readTrajectoryValue(
  value: TrajectoryJsonObject,
  path: string[],
): unknown {
  let current: unknown = value;
  for (const segment of path) {
    if (!isTrajectoryJsonObject(current) || !(segment in current)) {
      return undefined;
    }
    current = current[segment];
  }
  return current;
}
