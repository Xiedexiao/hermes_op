import { useEffect, useMemo, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { sessionListRecent, type Session } from '../lib/tauri';
import { useAppStore } from '../store/appStore';
import { useMissionStore } from '../store/missionStore';
import { useRuntimeStore } from '../store/runtimeStore';
import './ContextPanel.css';

const routeCopy: Record<string, { title: string; detail: string }> = {
  '/home': {
    title: 'Workspace Overview',
    detail: '快速切回当前最重要的 Mission、最新 Session 和待处理执行项。',
  },
  '/missions': {
    title: 'Mission Context',
    detail: '当前选中的 Mission 会在这里保持可见，避免跨页丢上下文。',
  },
  '/operate': {
    title: 'Execution Context',
    detail: 'Operate 的恢复流、待审批数和最近 Session 会在这里持续提供侧边参考。',
  },
  '/notifications': {
    title: 'Notification Context',
    detail: '把待审批、失败和完成提醒集中查看，再回跳到对应 Mission / Operate / Simulation 工作面。',
  },
  '/knowledge': {
    title: 'Knowledge Context',
    detail: '这里会保持当前 Mission 和最新 Session 可见，方便把知识条目回挂到正在进行的工作。',
  },
  '/simulation': {
    title: 'Simulation Context',
    detail: '推演页面会和当前 Mission、活跃执行态一起显示，方便从运行记录回看方案路径。',
  },
  '/skills': {
    title: 'Skills Context',
    detail: '技能管理页会和当前 runtime model、最新 Session 一起展示，方便判断哪个 skill 应该被调用。',
  },
  '/agent-exchange': {
    title: 'Agent Exchange Context',
    detail: '本地 mailbox 用 JSON bundle 预留未来远端用户及其 agent 的交接能力；当前只做可审计手动传递，不声称远端实时投递。',
  },
  '/voice': {
    title: 'Voice Context',
    detail: '查看本地 transcript、speak queue 与当前 runtime，一边管理 voice workflow 一边保持 mission 上下文。',
  },
  '/sessions': {
    title: 'Session Context',
    detail: '最近 Session 和当前选中 Mission 会一起呈现，方便恢复跨线程工作。',
  },
  '/runtime': {
    title: 'Runtime Context',
    detail: '引擎状态与待审批执行项一起展示，方便判断是否需要介入。',
  },
  '/settings': {
    title: 'Settings Context',
    detail: '修改运行时配置前，可以先确认当前 Mission 和 Session 的工作指向。',
  },
};

export function ContextPanel() {
  const location = useLocation();
  const navigate = useNavigate();
  const missions = useMissionStore((state) => state.missions);
  const selectedMissionId = useMissionStore((state) => state.selectedMissionId);
  const selectMission = useMissionStore((state) => state.selectMission);
  const { activeMissionCount, pendingApprovalCount, engine, appRuntime } = useRuntimeStore();
  const activeSession = useAppStore((state) => state.activeSession);
  const [recentSessions, setRecentSessions] = useState<Session[]>([]);
  const [sessionError, setSessionError] = useState<string | null>(null);

  const selectedMission = useMemo(
    () => missions.find((mission) => mission.id === selectedMissionId) ?? missions[0] ?? null,
    [missions, selectedMissionId],
  );
  const latestSession = recentSessions[0] ?? null;
  const sessionCard = activeSession?.session ?? latestSession;
  const copy = routeCopy[location.pathname] ?? {
    title: 'Context',
    detail: '当前工作区的上下文摘要会显示在这里。',
  };

  useEffect(() => {
    let cancelled = false;

    async function loadRecentSessions() {
      try {
        const data = await sessionListRecent(6);
        if (!cancelled) {
          setRecentSessions(data);
          setSessionError(null);
        }
      } catch (err) {
        if (!cancelled) {
          setSessionError(err instanceof Error ? err.message : String(err));
        }
      }
    }

    void loadRecentSessions();
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <aside className="context-panel">
      <div className="context-panel-card">
        <div className="context-panel-eyebrow">Right Panel</div>
        <h3>{copy.title}</h3>
        <p>{copy.detail}</p>
      </div>

      <div className="context-panel-card">
        <div className="context-panel-eyebrow">Runtime Snapshot</div>
        <div className="context-panel-stats">
          <div className="context-panel-stat">
            <span className="context-panel-stat-value">{activeMissionCount}</span>
            <span className="context-panel-stat-label">Active missions</span>
          </div>
          <div className="context-panel-stat">
            <span className="context-panel-stat-value">{pendingApprovalCount}</span>
            <span className="context-panel-stat-label">Pending approvals</span>
          </div>
        </div>
        <div className="context-panel-list">
          <div className="context-panel-row">
            <span>Engine</span>
            <strong>{engine.running ? 'Running' : 'Stopped'}</strong>
          </div>
          <div className="context-panel-row">
            <span>Runtime</span>
            <strong>{appRuntime.running ? 'Online' : 'Offline'}</strong>
          </div>
          {engine.profile ? (
            <div className="context-panel-row">
              <span>Profile</span>
              <strong>{engine.profile}</strong>
            </div>
          ) : null}
        </div>
      </div>

      <div className="context-panel-card">
        <div className="context-panel-eyebrow">Active Mission</div>
        {selectedMission ? (
          <button
            type="button"
            className="context-panel-linkcard"
            onClick={() => {
              selectMission(selectedMission.id);
              navigate('/missions');
            }}
          >
            <span className="context-panel-linkcard-title">{selectedMission.title}</span>
            <span className="context-panel-linkcard-text">{selectedMission.goal}</span>
            <span className="context-panel-linkcard-meta">
              {selectedMission.status} · {selectedMission.priority}
            </span>
          </button>
        ) : (
          <p className="context-panel-empty">暂无选中的 Mission。</p>
        )}
      </div>

      <div className="context-panel-card">
        <div className="context-panel-eyebrow">
          {activeSession ? 'Active Session' : 'Latest Session'}
        </div>
        {sessionError ? <p className="context-panel-empty">{sessionError}</p> : null}
        {!sessionError && sessionCard ? (
          <button
            type="button"
            className="context-panel-linkcard"
            onClick={() => navigate('/sessions')}
          >
            <span className="context-panel-linkcard-title">{sessionCard.title}</span>
            <span className="context-panel-linkcard-text">
              {sessionCard.source} · {sessionCard.model_name ?? '-'}
            </span>
            <span className="context-panel-linkcard-meta">
              {activeSession ? `${activeSession.reason} · ` : ''}
              {sessionCard.updated_at}
            </span>
          </button>
        ) : null}
        {!sessionError && !sessionCard ? (
          <p className="context-panel-empty">暂无 recent session。</p>
        ) : null}
      </div>
    </aside>
  );
}
