import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAppStore } from '../store/appStore';
import { useMissionStore } from '../store/missionStore';
import { useRuntimeStore } from '../store/runtimeStore';
import { appGetBootstrap, sessionGetLatest } from '../lib/tauri';
import './HomePage.css';

export function HomePage() {
  const navigate = useNavigate();
  const { setAppSettings, setRuntimeSettings, setInitialized, setError, setActiveSession } =
    useAppStore();
  const [sessionSummary, setSessionSummary] = useState({
    recent_session_count: 0,
    has_recent_session: false,
  });
  const [sessionShortcutLoading, setSessionShortcutLoading] = useState(false);
  const missions = useMissionStore((state) => state.missions);
  const selectMission = useMissionStore((state) => state.selectMission);
  const upsertMission = useMissionStore((state) => state.upsertMission);
  const {
    setEngineStatus,
    setAppRuntimeStatus,
    setForegroundStatus,
    setActiveMissionCount,
    setPendingApprovalCount,
    loading,
    setLoading,
  } = useRuntimeStore();

  useEffect(() => {
    loadBootstrap();
  }, [upsertMission]);

  const loadBootstrap = async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await appGetBootstrap();
      setAppSettings(data.app_settings);
      setRuntimeSettings(data.runtime_settings);
      setEngineStatus(data.engine_status);
      setAppRuntimeStatus(data.app_runtime_status);
      setForegroundStatus(data.foreground_snapshot);
      setActiveMissionCount(data.summary.active_mission_count);
      setPendingApprovalCount(data.summary.pending_approval_count);
      setActiveSession(data.active_session ?? null);
      if (data.active_mission) {
        upsertMission(data.active_mission);
      }
      setSessionSummary({
        recent_session_count: data.summary.recent_session_count,
        has_recent_session: data.summary.has_recent_session,
      });
      setInitialized(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const engine = useRuntimeStore((state) => state.engine);
  const appRuntime = useRuntimeStore((state) => state.appRuntime);
  const activeMissionCount = useRuntimeStore((state) => state.activeMissionCount);
  const pendingApprovalCount = useRuntimeStore((state) => state.pendingApprovalCount);
  const error = useAppStore((state) => state.error);
  const activeSession = useAppStore((state) => state.activeSession);
  const latestMission = missions[0] ?? null;

  async function handleOpenLatestSession() {
    setSessionShortcutLoading(true);
    try {
      if (activeSession) {
        navigate('/sessions');
        return;
      }
      const latest = await sessionGetLatest();
      if (latest) {
        navigate('/sessions');
      }
    } finally {
      setSessionShortcutLoading(false);
    }
  }

  if (loading) {
    return (
      <div className="home-page">
        <div className="loading">加载中...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="home-page">
        <div className="error-card">
          <h3>加载失败</h3>
          <p>{error}</p>
          <button onClick={loadBootstrap}>重试</button>
        </div>
      </div>
    );
  }

  return (
    <div className="home-page">
      <h2>概览</h2>

      <div className="cards-grid">
        <div className="card">
          <h3>Agent Engine</h3>
          <div className="status-indicator">
            <span className={`dot ${engine.running ? 'running' : 'stopped'}`} />
            <span>{engine.running ? '运行中' : '已停止'}</span>
          </div>
          {engine.profile && <p>Profile: {engine.profile}</p>}
          {engine.pid && <p>PID: {engine.pid}</p>}
        </div>

        <div className="card">
          <h3>Application Runtime</h3>
          <div className="status-indicator">
            <span className={`dot ${appRuntime.running ? 'running' : 'stopped'}`} />
            <span>{appRuntime.running ? '运行中' : '已停止'}</span>
          </div>
          {appRuntime.installed && <p>运行组件已就绪</p>}
          {appRuntime.version && <p>版本: {appRuntime.version}</p>}
        </div>

        <div className="card">
          <h3>任务统计</h3>
          <div className="stat">
            <span className="stat-value">{activeMissionCount}</span>
            <span className="stat-label">活跃任务</span>
          </div>
          <div className="stat">
            <span className="stat-value">{pendingApprovalCount}</span>
            <span className="stat-label">待审批</span>
          </div>
        </div>

        <div className="card">
          <h3>Session 摘要</h3>
          <div className="stat">
            <span className="stat-value">{sessionSummary.recent_session_count}</span>
            <span className="stat-label">Recent sessions</span>
          </div>
          {activeSession ? (
            <p>
              当前 handoff: {activeSession.session.title} · {activeSession.reason}
            </p>
          ) : null}
          <div className="home-session-status">
            <span
              className={`dot ${(activeSession ?? sessionSummary.has_recent_session) ? 'running' : 'stopped'}`}
            />
            <span>
              {activeSession
                ? '已设置 active session handoff'
                : sessionSummary.has_recent_session
                  ? '已检测到最新 session'
                  : '暂无 recent session'}
            </span>
          </div>
          <button
            type="button"
            className="home-session-button"
            onClick={() => void handleOpenLatestSession()}
            disabled={(!activeSession && !sessionSummary.has_recent_session) || sessionShortcutLoading}
          >
            {sessionShortcutLoading
              ? '打开中...'
              : activeSession
                ? '打开 Active Session'
                : '打开最新 Session'}
          </button>
        </div>
      </div>

      <div className="quick-actions">
        <h3>快速操作</h3>
        <div className="action-buttons">
          <button className="action-btn primary" onClick={() => navigate('/missions')}>
            新建 Mission
          </button>
          <button className="action-btn" onClick={() => navigate('/operate')}>
            打开 Operate
          </button>
          <button className="action-btn" onClick={() => navigate('/settings')}>
            打开设置
          </button>
        </div>
      </div>

      <div className="latest-mission-card">
        <h3>{latestMission ? '恢复未完成 Mission' : '最近 Mission'}</h3>
        {latestMission ? (
          <button
            type="button"
            className="latest-mission-button"
            onClick={() => {
              selectMission(latestMission.id);
              navigate('/missions');
            }}
          >
            <span className="latest-mission-title">{latestMission.title}</span>
            <span className="latest-mission-goal">{latestMission.goal}</span>
            <span className="latest-mission-meta">
              {latestMission.status} · {latestMission.priority} · last active {latestMission.last_activity_at}
            </span>
          </button>
        ) : (
          <p className="latest-mission-empty">还没有可查看的 Mission。</p>
        )}
      </div>
    </div>
  );
}
