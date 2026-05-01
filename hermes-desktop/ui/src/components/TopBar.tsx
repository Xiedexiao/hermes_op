import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  globalSearch,
  notificationsList,
  sessionGetActive,
  sessionListRecent,
  type GlobalSearchResult,
  type NotificationItem,
  type Session,
} from '../lib/tauri';
import { useAppStore } from '../store/appStore';
import { useMissionStore } from '../store/missionStore';
import { useRuntimeStore } from '../store/runtimeStore';
import { StatusBadge } from './StatusBadge';
import './TopBar.css';

export function TopBar() {
  const navigate = useNavigate();
  const missions = useMissionStore((state) => state.missions);
  const selectMission = useMissionStore((state) => state.selectMission);
  const { engine, appRuntime, pendingApprovalCount, loading } = useRuntimeStore();
  const runtimeSettings = useAppStore((state) => state.runtimeSettings);
  const activeSession = useAppStore((state) => state.activeSession);
  const setActiveSession = useAppStore((state) => state.setActiveSession);
  const [query, setQuery] = useState('');
  const [recentSessions, setRecentSessions] = useState<Session[]>([]);
  const [searchResults, setSearchResults] = useState<GlobalSearchResult[]>([]);
  const [notifications, setNotifications] = useState<NotificationItem[]>([]);
  const [searchLoading, setSearchLoading] = useState(false);
  const [notificationsLoading, setNotificationsLoading] = useState(false);
  const [showSearch, setShowSearch] = useState(false);
  const [showNotifications, setShowNotifications] = useState(false);

  useEffect(() => {
    let cancelled = false;

    async function loadSessions() {
      const [activeResult, recentResult] = await Promise.allSettled([
        sessionGetActive(),
        sessionListRecent(10),
      ]);

      if (cancelled) {
        return;
      }

      if (activeResult.status === 'fulfilled') {
        setActiveSession(activeResult.value);
      } else {
        setActiveSession(null);
      }

      if (recentResult.status === 'fulfilled') {
        setRecentSessions(recentResult.value);
      } else {
        setRecentSessions([]);
      }
    }

    void loadSessions();
    return () => {
      cancelled = true;
    };
  }, [setActiveSession]);

  useEffect(() => {
    let cancelled = false;

    async function loadNotifications() {
      setNotificationsLoading(true);
      try {
        const data = await notificationsList();
        if (!cancelled) {
          setNotifications(data);
        }
      } catch {
        if (!cancelled) {
          setNotifications([]);
        }
      } finally {
        if (!cancelled) {
          setNotificationsLoading(false);
        }
      }
    }

    void loadNotifications();
    return () => {
      cancelled = true;
    };
  }, [pendingApprovalCount]);

  useEffect(() => {
    let cancelled = false;

    async function loadSearch() {
      const trimmed = query.trim();
      if (!trimmed) {
        setSearchResults([]);
        setSearchLoading(false);
        return;
      }

      setSearchLoading(true);
      try {
        const data = await globalSearch({ query: trimmed });
        if (!cancelled) {
          setSearchResults(data);
        }
      } catch {
        if (!cancelled) {
          setSearchResults([]);
        }
      } finally {
        if (!cancelled) {
          setSearchLoading(false);
        }
      }
    }

    void loadSearch();
    return () => {
      cancelled = true;
    };
  }, [query]);

  const engineBadgeStatus = engine.last_error
    ? 'error'
    : loading
      ? 'loading'
      : engine.running
        ? 'running'
        : 'stopped';
  const runtimeBadgeStatus = loading
    ? 'loading'
    : appRuntime.running
      ? 'running'
      : appRuntime.installed
        ? 'stopped'
        : 'error';
  const searchPlaceholder = useMemo(() => {
    if (missions.length === 0 && recentSessions.length === 0) {
      return 'Search across missions, sessions, knowledge, and skills';
    }
    return 'Search missions, sessions, knowledge, and skills';
  }, [missions.length, recentSessions.length]);
  const activeSessionLabel = activeSession?.session.title ?? 'No active handoff';
  const activeSessionMeta = activeSession
    ? `${activeSession.reason} · ${activeSession.session.source} · ${activeSession.session.model_name ?? 'model unknown'}`
    : 'Open Sessions to pick or inspect the current handoff';

  return (
    <header className="top-bar">
      <div className="top-bar-title">
        <h1>Hermes Operator</h1>
        <span className="top-bar-subtitle">
          {runtimeSettings?.provider ?? 'openai'} / {runtimeSettings?.model ?? 'gpt-4o'}
        </span>
      </div>
      <button
        type="button"
        className={`top-bar-session-chip${activeSession ? ' top-bar-session-chip-active' : ''}`}
        onClick={() => navigate('/sessions')}
        aria-label={
          activeSession
            ? `Open Sessions for active handoff ${activeSession.session.title}`
            : 'Open Sessions'
        }
      >
        <span className="top-bar-session-label">
          {activeSession ? 'Active handoff' : 'Sessions'}
        </span>
        <span className="top-bar-session-title">{activeSessionLabel}</span>
        <span className="top-bar-session-detail">{activeSessionMeta}</span>
      </button>
      <div className="top-bar-search">
        <input
          type="search"
          value={query}
          onFocus={() => setShowSearch(true)}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={searchPlaceholder}
          aria-label="Global search"
        />
        {showSearch && query.trim() ? (
          <div className="top-bar-popover">
            {searchLoading ? (
              <div className="top-bar-empty">Searching...</div>
            ) : searchResults.length > 0 ? (
              searchResults.map((item) => (
                <button
                  key={`${item.kind}:${item.id}`}
                  type="button"
                  className="top-bar-result"
                  onClick={() => {
                    if (item.kind === 'mission') {
                      selectMission(item.id);
                    }
                    navigate(item.route);
                    if (item.kind === 'session' && item.route !== '/sessions') {
                      navigate('/sessions');
                    }
                    setShowSearch(false);
                    setQuery('');
                  }}
                >
                  <span className="top-bar-result-title">{item.title}</span>
                  <span className="top-bar-result-detail">
                    {item.kind} · {item.detail}
                  </span>
                </button>
              ))
            ) : (
              <div className="top-bar-empty">没有匹配的 Mission 或 Session。</div>
            )}
          </div>
        ) : null}
      </div>
      <div className="top-bar-right">
        <StatusBadge status={engineBadgeStatus} label="Engine" />
        <StatusBadge status={runtimeBadgeStatus} label="Runtime" />
        <button
          type="button"
          className="top-bar-notification"
          onClick={() => setShowNotifications((current) => !current)}
        >
          <span className="top-bar-notification-count">{notifications.length}</span>
          Notifications
        </button>
        {showNotifications ? (
          <div className="top-bar-popover top-bar-notification-popover">
            {notificationsLoading ? (
              <div className="top-bar-empty">Loading notifications...</div>
            ) : notifications.length > 0 ? (
              <>
                {notifications.map((item) => (
                  <button
                    key={item.id}
                    type="button"
                    className="top-bar-result"
                    onClick={() => {
                      if (item.mission_id) {
                        selectMission(item.mission_id);
                      }
                      navigate(item.route);
                      setShowNotifications(false);
                    }}
                  >
                    <span className="top-bar-result-title">{item.title}</span>
                    <span className="top-bar-result-detail">
                      {item.kind} · {item.message}
                    </span>
                  </button>
                ))}
                <button
                  type="button"
                  className="top-bar-result top-bar-result-secondary"
                  onClick={() => {
                    navigate('/notifications');
                    setShowNotifications(false);
                  }}
                >
                  <span className="top-bar-result-title">Open notification center</span>
                  <span className="top-bar-result-detail">查看全部通知与跳转入口</span>
                </button>
              </>
            ) : (
              <div className="top-bar-empty">当前没有通知。</div>
            )}
          </div>
        ) : null}
      </div>
    </header>
  );
}
