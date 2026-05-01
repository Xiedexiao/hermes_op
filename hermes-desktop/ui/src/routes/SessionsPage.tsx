import { useEffect, useMemo, useRef, useState, type FormEvent } from 'react';
import {
  sessionActivate,
  sessionClearActive,
  sessionContinueLatest,
  sessionGet,
  sessionGetActive,
  sessionGetLatest,
  sessionListRecent,
  sessionMessageCreate,
  sessionMessageList,
  sessionRename,
  sessionResumeByTitle,
  sessionSearch,
  type SessionMessage,
  type Session,
} from '../lib/tauri';
import {
  buildSessionContinuityCards,
  buildTranscriptReplaySummary,
  truncateTranscriptPreview,
} from './sessionContinuity';
import { useAppStore } from '../store/appStore';
import './SessionsPage.css';

function formatDateTime(value?: string | null) {
  if (!value) {
    return '-';
  }

  return new Intl.DateTimeFormat('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(value));
}

const transcriptComposerRoles = ['note', 'user', 'assistant'] as const;
const transcriptFilterRoles = ['all', 'note', 'user', 'assistant', 'system', 'tool'] as const;
const continuityRoleLabels = {
  selected: 'Open now',
  active: 'Active handoff',
  latest: 'Latest persisted',
} as const;

type ContinuityPreviewState = {
  status: 'loading' | 'ready' | 'error';
  messages: SessionMessage[];
  error: string | null;
};

const messageRoleMeta: Record<
  SessionMessage['role'],
  { label: string; description: string; tone: 'note' | 'user' | 'assistant' | 'system' | 'tool' }
> = {
  assistant: {
    label: 'Assistant',
    description: 'Agent replies and generated output.',
    tone: 'assistant',
  },
  note: {
    label: 'Note',
    description: 'Operator notes or manual context.',
    tone: 'note',
  },
  system: {
    label: 'System',
    description: 'System generated lifecycle events.',
    tone: 'system',
  },
  tool: {
    label: 'Tool',
    description: 'Tooling or automation side effects.',
    tone: 'tool',
  },
  user: {
    label: 'User',
    description: 'Human-authored prompts and instructions.',
    tone: 'user',
  },
};

function getMessageSourceTone(source: string) {
  if (source === 'local') {
    return 'local';
  }
  if (source === 'remote') {
    return 'remote';
  }
  return 'neutral';
}

export function SessionsPage() {
  const activeSession = useAppStore((state) => state.activeSession);
  const setActiveSession = useAppStore((state) => state.setActiveSession);
  const [sessions, setSessions] = useState<Session[]>([]);
  const [searchResults, setSearchResults] = useState<Session[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [selectedSession, setSelectedSession] = useState<Session | null>(null);
  const [messageHistory, setMessageHistory] = useState<SessionMessage[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [loading, setLoading] = useState(true);
  const [searchLoading, setSearchLoading] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);
  const [titleDraft, setTitleDraft] = useState('');
  const [renameSaving, setRenameSaving] = useState(false);
  const [renameError, setRenameError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [recentRefreshing, setRecentRefreshing] = useState(false);
  const [continueLatestLoading, setContinueLatestLoading] = useState(false);
  const [resumeByTitleLoading, setResumeByTitleLoading] = useState(false);
  const [detailRefreshTick, setDetailRefreshTick] = useState(0);
  const [focusTitleAfterLoad, setFocusTitleAfterLoad] = useState(false);
  const [actionMessage, setActionMessage] = useState<string | null>(null);
  const [renameSuccessMessage, setRenameSuccessMessage] = useState<string | null>(null);
  const [messageDraft, setMessageDraft] = useState('');
  const [messageRole, setMessageRole] = useState<'note' | 'user' | 'assistant'>('note');
  const [messageSaving, setMessageSaving] = useState(false);
  const [messageFilterRole, setMessageFilterRole] =
    useState<(typeof transcriptFilterRoles)[number]>('all');
  const [messageFilterQuery, setMessageFilterQuery] = useState('');
  const [continuityPreviewById, setContinuityPreviewById] = useState<
    Record<string, ContinuityPreviewState>
  >({});
  const titleInputRef = useRef<HTMLInputElement | null>(null);
  const mountedRef = useRef(true);

  const latestSession = sessions[0] ?? null;
  const activeSessionId = activeSession?.session.id ?? null;
  const trimmedSearchQuery = searchQuery.trim();
  const trimmedTitleDraft = titleDraft.trim();
  const trimmedMessageFilterQuery = messageFilterQuery.trim();
  const hasRenameChanges =
    selectedSession !== null &&
    trimmedTitleDraft.length > 0 &&
    trimmedTitleDraft !== selectedSession.title;

  const filteredSessions = useMemo(
    () => (trimmedSearchQuery ? searchResults : sessions),
    [searchResults, sessions, trimmedSearchQuery],
  );
  const transcriptMessages = useMemo(
    () =>
      [...messageHistory].sort(
        (left, right) =>
          new Date(left.created_at).getTime() - new Date(right.created_at).getTime(),
      ),
    [messageHistory],
  );
  const transcriptStats = useMemo(() => {
    const uniqueRoles = new Set(transcriptMessages.map((item) => item.role));
    const uniqueSources = new Set(transcriptMessages.map((item) => item.source));
    const latestMessage = transcriptMessages[transcriptMessages.length - 1] ?? null;

    return {
      count: transcriptMessages.length,
      uniqueRoles: uniqueRoles.size,
      uniqueSources: uniqueSources.size,
      latestMessage,
    };
  }, [transcriptMessages]);
  const messageDraftLength = messageDraft.trim().length;
  const composerPlaceholder =
    messageRole === 'assistant'
      ? 'Summarize the latest outcome, answer, or handoff response'
      : messageRole === 'user'
        ? 'Add the operator prompt or recovery instruction that belongs in this transcript'
        : 'Capture a note, annotation, or manual breadcrumb for this session';

  const selectedSessionPreview =
    filteredSessions.find((session) => session.id === selectedId) ??
    sessions.find((session) => session.id === selectedId) ??
    latestSession;
  const continuitySelectedSession =
    selectedSession?.id === selectedId ? selectedSession : selectedSessionPreview ?? selectedSession;
  const continuityCards = useMemo(
    () =>
      buildSessionContinuityCards({
        activeSession,
        latestSession,
        selectedSession: continuitySelectedSession ?? null,
      }),
    [activeSession, continuitySelectedSession, latestSession],
  );
  const selectedReplaySummary = useMemo(
    () => buildTranscriptReplaySummary(transcriptMessages),
    [transcriptMessages],
  );
  const continuityCardStates = useMemo(
    () =>
      continuityCards.map((card) => {
        const previewState =
          card.session.id === selectedSession?.id
            ? {
                status: 'ready' as const,
                messages: transcriptMessages,
                error: null,
              }
            : continuityPreviewById[card.session.id] ?? {
                status: 'loading' as const,
                messages: [],
                error: null,
              };

        return {
          ...card,
          previewState,
          replaySummary:
            card.session.id === selectedSession?.id
              ? selectedReplaySummary
              : buildTranscriptReplaySummary(previewState.messages),
        };
      }),
    [continuityPreviewById, continuityCards, selectedReplaySummary, selectedSession?.id, transcriptMessages],
  );

  useEffect(() => {
    return () => {
      mountedRef.current = false;
    };
  }, []);

  async function loadRecentSessions(options?: { preserveSelection?: boolean }) {
    const preserveSelection = options?.preserveSelection ?? false;
    setError(null);
    try {
      const data = await sessionListRecent(20);
      if (!mountedRef.current) {
        return;
      }

      setSessions(data);
      setSelectedId((current) => {
        if (preserveSelection && current && data.some((item) => item.id === current)) {
          return current;
        }
        return data[0]?.id ?? null;
      });
    } catch (err) {
      if (!mountedRef.current) {
        return;
      }
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  function upsertSession(nextSession: Session) {
    setSessions((current) => {
      const remaining = current.filter((item) => item.id !== nextSession.id);
      return [nextSession, ...remaining];
    });
  }

  useEffect(() => {
    let cancelled = false;

    async function loadSessions() {
      setLoading(true);
      try {
        await Promise.all([loadRecentSessions(), loadActiveSelection()]);
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

    void loadSessions();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    async function loadSearchResults() {
      if (!trimmedSearchQuery) {
        setSearchResults([]);
        setSearchLoading(false);
        return;
      }

      setSearchLoading(true);
      try {
        const items = await sessionSearch(trimmedSearchQuery, 20);
        if (!cancelled) {
          setSearchResults(items);
          setSelectedId((current) => current ?? items[0]?.id ?? null);
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err));
        }
      } finally {
        if (!cancelled) {
          setSearchLoading(false);
        }
      }
    }

    void loadSearchResults();
    return () => {
      cancelled = true;
    };
  }, [trimmedSearchQuery]);

  useEffect(() => {
    if (!selectedId) {
      setSelectedSession(null);
      setMessageHistory([]);
      return;
    }

    const sessionId = selectedId;
    let cancelled = false;

    async function loadSessionDetail() {
      setDetailLoading(true);
      setError(null);
      try {
        const [data, history] = await Promise.all([
          sessionGet(sessionId),
          sessionMessageList({
            session_id: sessionId,
            limit: 50,
            role: messageFilterRole === 'all' ? null : messageFilterRole,
            query: trimmedMessageFilterQuery || null,
          }),
        ]);
        if (!cancelled) {
          if (!data) {
            setSelectedSession(null);
            setMessageHistory([]);
            setActionMessage('所选 session 已不可用，请从 recent list 中重新选择。');
            return;
          }
          setSelectedSession(data);
          setMessageHistory(history);
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

    void loadSessionDetail();
    return () => {
      cancelled = true;
    };
  }, [detailRefreshTick, messageFilterRole, selectedId, trimmedMessageFilterQuery]);

  useEffect(() => {
    setTitleDraft(selectedSession?.title ?? '');
    setRenameError(null);
    setRenameSuccessMessage(null);
  }, [selectedSession?.id, selectedSession?.title]);

  useEffect(() => {
    if (!selectedSession) {
      return;
    }

    setContinuityPreviewById((current) => ({
      ...current,
      [selectedSession.id]: {
        status: 'ready',
        messages: transcriptMessages,
        error: null,
      },
    }));
  }, [selectedSession, transcriptMessages]);

  useEffect(() => {
    const previewIds = Array.from(
      new Set(
        continuityCards
          .map((card) => card.session.id)
          .filter((sessionId) => sessionId !== selectedSession?.id),
      ),
    ).filter((sessionId) => {
      const currentPreview = continuityPreviewById[sessionId];
      return !currentPreview || currentPreview.status === 'error';
    });

    if (previewIds.length === 0) {
      return;
    }

    let cancelled = false;
    setContinuityPreviewById((current) => {
      const next = { ...current };
      for (const sessionId of previewIds) {
        if (!next[sessionId] || next[sessionId]?.status === 'error') {
          next[sessionId] = {
            status: 'loading',
            messages: [],
            error: null,
          };
        }
      }
      return next;
    });

    void Promise.all(
      previewIds.map(async (sessionId) => {
        try {
          const messages = await sessionMessageList({
            session_id: sessionId,
            limit: 3,
          });
          return {
            sessionId,
            nextState: {
              status: 'ready' as const,
              messages,
              error: null,
            },
          };
        } catch (err) {
          return {
            sessionId,
            nextState: {
              status: 'error' as const,
              messages: [],
              error: err instanceof Error ? err.message : String(err),
            },
          };
        }
      }),
    ).then((results) => {
      if (cancelled || !mountedRef.current) {
        return;
      }

      setContinuityPreviewById((current) => {
        const next = { ...current };
        for (const result of results) {
          next[result.sessionId] = result.nextState;
        }
        return next;
      });
    });

    return () => {
      cancelled = true;
    };
  }, [continuityCards, continuityPreviewById, selectedSession?.id]);

  useEffect(() => {
    if (!selectedSession || !focusTitleAfterLoad || selectedSession.id !== selectedId) {
      return;
    }

    titleInputRef.current?.focus();
    setFocusTitleAfterLoad(false);
  }, [focusTitleAfterLoad, selectedId, selectedSession]);

  async function handleRefreshRecent() {
    setRecentRefreshing(true);
    setActionMessage(null);
    try {
      await Promise.all([loadRecentSessions({ preserveSelection: true }), loadActiveSelection()]);
      setActionMessage('Recent sessions 已刷新。搜索结果和 latest 卡片已同步。');
    } finally {
      if (mountedRef.current) {
        setRecentRefreshing(false);
      }
    }
  }

  async function loadActiveSelection() {
    const active = await sessionGetActive();
    if (mountedRef.current) {
      setActiveSession(active);
    }
  }

  function handleSelectLatest() {
    if (!latestSession) {
      return;
    }

    setSelectedId(latestSession.id);
    setActionMessage('已选中 latest session。右侧详情会显示它的标题、ID、source 与 model。');
  }

  function handleOpenSessionTranscript(
    session: Session,
    sourceLabel: string,
    options?: { focusTitle?: boolean },
  ) {
    setSelectedId(session.id);
    setFocusTitleAfterLoad(options?.focusTitle ?? false);
    setDetailRefreshTick((current) => current + 1);
    setActionMessage(`已打开 ${sourceLabel} transcript：${session.title}`);
  }

  function handleOpenLatestDetail() {
    if (!latestSession) {
      return;
    }

    handleOpenSessionTranscript(latestSession, 'latest session', { focusTitle: true });
  }

  function handleOpenActiveDetail() {
    if (!activeSession) {
      return;
    }

    handleOpenSessionTranscript(activeSession.session, 'active handoff');
  }

  async function handleContinueLatest() {
    setContinueLatestLoading(true);
    setError(null);
    setActionMessage(null);
    try {
      const latest = await sessionContinueLatest();
      if (!mountedRef.current) {
        return;
      }

      if (!latest) {
        setError('没有可继续的 session。');
        return;
      }

      upsertSession(latest);
      const active = await sessionGetActive();
      setActiveSession(active);
      setSelectedId(latest.id);
      setFocusTitleAfterLoad(false);
      setDetailRefreshTick((current) => current + 1);
      setActionMessage(
        '已执行 continue latest，并更新当前 active session handoff。',
      );
    } catch (err) {
      if (mountedRef.current) {
        setError(err instanceof Error ? err.message : String(err));
      }
    } finally {
      if (mountedRef.current) {
        setContinueLatestLoading(false);
      }
    }
  }

  async function handleResumeByTitle() {
    const query = trimmedSearchQuery;
    if (!query) {
      setActionMessage('先输入 title、session id 或关键词，再执行标题恢复。');
      return;
    }

    setResumeByTitleLoading(true);
    setError(null);
    setActionMessage(null);
    try {
      const remoteMatch = await sessionResumeByTitle(query);
      const matched =
        remoteMatch ?? (await sessionSearch(query, 5))[0] ?? null;

      if (!matched) {
        setActionMessage('没有找到匹配标题。当前已经搜索持久化 session 数据库。');
        return;
      }

      upsertSession(matched);
      const active = await sessionGetActive();
      setActiveSession(active);
      setSelectedId(matched.id);
      setDetailRefreshTick((current) => current + 1);
      setActionMessage(
        remoteMatch
          ? `已通过后端标题恢复命中 session：${matched.title}`
          : `已通过持久化 session search 命中最接近的结果：${matched.title}`,
      );
    } catch (err) {
      if (mountedRef.current) {
        setError(err instanceof Error ? err.message : String(err));
      }
    } finally {
      if (mountedRef.current) {
        setResumeByTitleLoading(false);
      }
    }
  }

  async function handleLoadAbsoluteLatest() {
    setContinueLatestLoading(true);
    setError(null);
    setActionMessage(null);
    try {
      const latest = await sessionGetLatest();
      if (!latest) {
        setActionMessage('后端返回没有 latest session。');
        return;
      }

      upsertSession(latest);
      setSelectedId(latest.id);
      setDetailRefreshTick((current) => current + 1);
      setActionMessage('已直接读取 latest session 元数据。');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setContinueLatestLoading(false);
    }
  }

  async function handleRenameSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedSession || renameSaving) {
      return;
    }

    const nextTitle = titleDraft.trim();
    if (!nextTitle) {
      setRenameError('标题不能为空。');
      return;
    }

    if (nextTitle === selectedSession.title) {
      setRenameError(null);
      return;
    }

    setRenameSaving(true);
    setRenameError(null);
    setRenameSuccessMessage(null);
    try {
      const updated = await sessionRename({ id: selectedSession.id, title: nextTitle });
      setSelectedSession((current) => (current?.id === updated.id ? updated : current));
      setSessions((current) => current.map((item) => (item.id === updated.id ? updated : item)));
      setTitleDraft(updated.title);
      setRenameSuccessMessage('标题已保存。');
      setActionMessage(`已更新 session title：${updated.title}`);
    } catch (err) {
      setRenameError(err instanceof Error ? err.message : String(err));
    } finally {
      setRenameSaving(false);
    }
  }

  async function handleCreateMessage(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedSession || messageSaving || messageDraft.trim().length === 0) {
      return;
    }
    setMessageSaving(true);
    setError(null);
    try {
      const created = await sessionMessageCreate({
        session_id: selectedSession.id,
        role: messageRole,
        content: messageDraft.trim(),
        source: 'local',
      });
      setMessageHistory((current) => [created, ...current]);
      setMessageDraft('');
      setActionMessage(`已追加 session message：${created.role}`);
      setDetailRefreshTick((current) => current + 1);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setMessageSaving(false);
    }
  }

  async function handleSetActiveSession() {
    if (!selectedSession) {
      return;
    }
    const active = await sessionActivate({
      id: selectedSession.id,
      reason: 'manual_resume',
    });
    setActiveSession(active);
    setActionMessage(`已将 ${selectedSession.title} 设为当前 active session handoff。`);
  }

  async function handleClearActiveSession() {
    await sessionClearActive();
    setActiveSession(null);
    setActionMessage('已清除当前 active session handoff。');
  }

  return (
    <div className="sessions-page">
      <div className="sessions-header">
        <div>
          <h2>Sessions</h2>
          <p className="sessions-header-copy">
            当前页覆盖 latest / continue / resume / title，并持久化当前 active session handoff，
            让 Home、Context Panel 和 Sessions 使用同一份恢复上下文。
          </p>
        </div>
        <div className="sessions-summary-row">
          <div className="sessions-stat-card">
            <span className="sessions-stat-value">{sessions.length}</span>
            <span className="sessions-stat-label">Recent loaded</span>
          </div>
          <div className="sessions-stat-card">
            <span className="sessions-stat-value">{filteredSessions.length}</span>
            <span className="sessions-stat-label">Search matched</span>
          </div>
          <div className="sessions-stat-card">
            <span className="sessions-stat-value">{latestSession ? 'yes' : 'no'}</span>
            <span className="sessions-stat-label">Latest available</span>
          </div>
        </div>
      </div>

      {error ? <div className="sessions-error">{error}</div> : null}
      {actionMessage ? <div className="sessions-inline-note">{actionMessage}</div> : null}

      <div className="sessions-grid">
        <section className="sessions-card">
          <div className="sessions-card-header">
            <div>
              <h3>Recent Sessions</h3>
              <p className="sessions-card-subtitle">
                {latestSession
                  ? `latest: ${latestSession.title} · updated ${formatDateTime(latestSession.updated_at)}`
                  : '还没有可用的 recent session。'}
              </p>
            </div>
            <div className="sessions-actions" aria-label="Recent session quick actions">
              <button
                type="button"
                className="sessions-action-button"
                onClick={() => void handleRefreshRecent()}
                disabled={loading || recentRefreshing}
              >
                {recentRefreshing ? '刷新中...' : '刷新列表'}
              </button>
              <button
                type="button"
                className="sessions-action-button"
                onClick={handleSelectLatest}
                disabled={loading || !latestSession || selectedId === latestSession.id}
              >
                仅选中 latest
              </button>
              <button
                type="button"
                className="sessions-action-button"
                onClick={handleOpenActiveDetail}
                disabled={loading || !activeSession || selectedId === activeSessionId}
              >
                打开 active
              </button>
              <button
                type="button"
                className="sessions-action-button"
                onClick={() => void handleLoadAbsoluteLatest()}
                disabled={loading || !latestSession || continueLatestLoading}
              >
                读取 latest
              </button>
            </div>
          </div>

          <div className="sessions-search-panel">
            <input
              type="search"
              value={searchQuery}
              onChange={(event) => setSearchQuery(event.target.value)}
              placeholder="Search persisted sessions by title / id / source / model"
              aria-label="Search persisted sessions"
            />
            <button
              type="button"
              className="sessions-action-button sessions-action-button-primary"
              onClick={() => void handleResumeByTitle()}
              disabled={resumeByTitleLoading || trimmedSearchQuery.length === 0}
            >
              {resumeByTitleLoading ? '恢复中...' : '按标题恢复'}
            </button>
          </div>
          <p className="sessions-card-subtitle">
            当前搜索会查询持久化 session 数据库，不再只过滤内存中的 recent list。标题恢复也会优先命中数据库。
          </p>
          {activeSession ? (
            <p className="sessions-card-subtitle">
              active handoff: {activeSession.session.title} · {activeSession.reason}
            </p>
          ) : null}

          <div className="sessions-continue-latest">
            <div className="sessions-continue-latest-copy">
              <div className="sessions-continue-latest-label">Continue latest</div>
              <div className="sessions-continue-latest-title">
                {latestSession ? latestSession.title : '直接读取并继续最近一个 session'}
              </div>
              <p className="sessions-card-subtitle">
                用于对应 `/continue` 的行为面。当前页会定位 latest session、更新列表并同步 active handoff。
              </p>
            </div>
            <div className="sessions-continue-actions">
              <button
                type="button"
                className="sessions-action-button"
                onClick={handleOpenLatestDetail}
                disabled={loading || !latestSession}
              >
                打开详情
              </button>
              <button
                type="button"
                className="sessions-action-button sessions-action-button-primary sessions-continue-latest-button"
                onClick={() => void handleContinueLatest()}
                disabled={loading || continueLatestLoading}
              >
                {continueLatestLoading ? '载入中...' : '继续 latest'}
              </button>
            </div>
          </div>

          {loading ? (
            <div className="sessions-placeholder">
              <p>加载中...</p>
            </div>
          ) : null}

          {!loading && searchLoading ? (
            <div className="sessions-placeholder">
              <p>搜索持久化 sessions...</p>
            </div>
          ) : null}

          {!loading && sessions.length === 0 ? (
            <div className="sessions-placeholder">
              <p>还没有 session。</p>
            </div>
          ) : null}

          {!loading && !searchLoading && sessions.length > 0 && filteredSessions.length === 0 ? (
            <div className="sessions-placeholder">
              <p>没有匹配当前搜索条件的持久化 session。</p>
            </div>
          ) : null}

          {!loading && !searchLoading && filteredSessions.length > 0 ? (
            <div className="session-list">
              {filteredSessions.map((item) => (
                <button
                  key={item.id}
                  type="button"
                  className={`session-item ${selectedId === item.id ? 'session-item-active' : ''}`}
                  onClick={() => setSelectedId(item.id)}
                >
                  <div className="session-item-header">
                    <div className="session-item-title">{item.title}</div>
                    <div className="session-item-badges">
                      {item.id === activeSessionId ? <span className="session-item-badge">Active</span> : null}
                      {item.id === latestSession?.id ? <span className="session-item-badge">Latest</span> : null}
                    </div>
                  </div>
                  <div className="session-item-meta">
                    id={item.id} · source={item.source}
                  </div>
                  <div className="session-item-meta">
                    model={item.model_name ?? '-'} · updated={formatDateTime(item.updated_at)}
                  </div>
                  <div className="session-item-meta">
                    {item.id === activeSessionId
                      ? `handoff=${activeSession?.reason ?? 'active'}`
                      : item.parent_session_id
                        ? `parent=${item.parent_session_id}`
                        : 'standalone session'}
                  </div>
                </button>
              ))}
            </div>
          ) : null}
        </section>

        <section className="sessions-card">
          <div className="sessions-card-header">
            <div>
              <h3>Session Detail</h3>
              <p className="sessions-card-subtitle">
                {selectedSessionPreview
                  ? `当前聚焦: ${selectedSessionPreview.title}`
                  : '选择一个 session 查看详情。'}
              </p>
            </div>
            <div className="sessions-detail-hints">
              <span className="sessions-pill">/continue</span>
              <span className="sessions-pill">/resume &lt;id&gt;</span>
              <span className="sessions-pill">/title ...</span>
            </div>
          </div>

          {detailLoading ? (
            <div className="sessions-placeholder">
              <p>加载详情中...</p>
            </div>
          ) : null}

          {!detailLoading && !selectedSession ? (
            <div className="sessions-placeholder">
              <p>选择一个 session 查看详情。</p>
            </div>
          ) : null}

          {!detailLoading && selectedSession ? (
            <div className="sessions-detail-stack">
              <div className="session-meta-grid">
                <article className="session-meta-card">
                  <span className="session-meta-label">Session ID</span>
                  <strong>{selectedSession.id}</strong>
                </article>
                <article className="session-meta-card">
                  <span className="session-meta-label">Source / Model</span>
                  <strong>
                    {selectedSession.source} / {selectedSession.model_name ?? '-'}
                  </strong>
                </article>
                <article className="session-meta-card">
                  <span className="session-meta-label">Timeline</span>
                  <strong>{formatDateTime(selectedSession.updated_at)}</strong>
                </article>
              </div>

              <div className="detail-block">
                <div className="detail-block-header">
                  <div>
                    <h4>Continuation Surface</h4>
                    <p className="detail-block-copy">
                      对齐当前打开的 transcript、active handoff 和 latest persisted session，避免恢复时丢失线程连续性。
                    </p>
                  </div>
                  <span className="sessions-pill sessions-pill-muted">
                    tracking {continuityCardStates.length} continuity {continuityCardStates.length === 1 ? 'thread' : 'threads'}
                  </span>
                </div>

                <div className="session-continuity-grid">
                  {continuityCardStates.map((card) => {
                    const isSelectedCard = card.roles.includes('selected');
                    const isLatestCard = card.roles.includes('latest');
                    const isActiveCard = card.roles.includes('active');
                    const previewMessages = card.replaySummary.latestMessages;

                    return (
                      <article
                        key={card.session.id}
                        className={`session-continuity-card ${isSelectedCard ? 'session-continuity-card-selected' : ''}`}
                      >
                        <div className="session-continuity-header">
                          <div>
                            <span className="session-meta-label">Continuity card</span>
                            <h5>{card.session.title}</h5>
                          </div>
                          <div className="session-continuity-badges">
                            {card.roles.map((role) => (
                              <span
                                key={`${card.session.id}-${role}`}
                                className={`session-continuity-badge session-continuity-badge-${role}`}
                              >
                                {continuityRoleLabels[role]}
                              </span>
                            ))}
                          </div>
                        </div>

                        <div className="session-continuity-meta">
                          id={card.session.id} · source={card.session.source} · model={card.session.model_name ?? '-'}
                        </div>
                        <div className="session-continuity-meta">
                          updated={formatDateTime(card.session.updated_at)}
                          {card.activeReason ? ` · reason=${card.activeReason}` : ''}
                        </div>

                        {card.previewState.status === 'loading' ? (
                          <div className="session-continuity-placeholder">正在读取 transcript tail...</div>
                        ) : null}

                        {card.previewState.status === 'error' ? (
                          <div className="session-continuity-error">
                            {card.previewState.error ?? '读取 transcript tail 失败。'}
                          </div>
                        ) : null}

                        {card.previewState.status === 'ready' && previewMessages.length === 0 ? (
                          <div className="session-continuity-placeholder">暂无可回放的 transcript entry。</div>
                        ) : null}

                        {card.previewState.status === 'ready' && previewMessages.length > 0 ? (
                          <div className="session-replay-list">
                            {previewMessages.map((message) => {
                              const roleMeta = messageRoleMeta[message.role];
                              return (
                                <div key={message.id} className="session-replay-item">
                                  <div className="session-replay-item-header">
                                    <span
                                      className={`transcript-role-badge transcript-role-badge-${roleMeta.tone}`}
                                    >
                                      {roleMeta.label}
                                    </span>
                                    <span className="session-replay-item-time">
                                      {formatDateTime(message.created_at)}
                                    </span>
                                  </div>
                                  <p className="session-replay-item-copy">
                                    {truncateTranscriptPreview(message.content, 180)}
                                  </p>
                                </div>
                              );
                            })}
                          </div>
                        ) : null}

                        <div className="session-continuity-footer">
                          <span className="session-continuity-tail-meta">
                            {card.previewState.status === 'ready'
                              ? `${card.replaySummary.count} entries loaded`
                              : 'tail preview pending'}
                          </span>
                          <div className="sessions-actions">
                            {!isSelectedCard ? (
                              <button
                                type="button"
                                className="sessions-action-button"
                                onClick={() =>
                                  handleOpenSessionTranscript(
                                    card.session,
                                    isActiveCard
                                      ? 'active handoff'
                                      : isLatestCard
                                        ? 'latest session'
                                        : 'continuity session',
                                  )
                                }
                              >
                                打开 transcript
                              </button>
                            ) : (
                              <span className="sessions-pill sessions-pill-muted">当前查看中</span>
                            )}
                            {isLatestCard && !isActiveCard ? (
                              <button
                                type="button"
                                className="sessions-action-button sessions-action-button-primary"
                                onClick={() => void handleContinueLatest()}
                                disabled={continueLatestLoading}
                              >
                                {continueLatestLoading ? '载入中...' : '继续这个 latest'}
                              </button>
                            ) : null}
                          </div>
                        </div>
                      </article>
                    );
                  })}
                </div>
              </div>

              <div className="session-item">
                <form className="session-title-form" onSubmit={handleRenameSubmit}>
                  <label className="session-title-label" htmlFor="session-title-input">
                    Session title
                  </label>
                  <div className="session-title-caption">
                    对应 Hermes 的 `/title` 行为面。当前实现会更新 recent list 与 detail 标题，不改动其他会话内容。
                  </div>
                  <div className="session-title-row">
                    <input
                      id="session-title-input"
                      className="session-title-input"
                      ref={titleInputRef}
                      value={titleDraft}
                      onChange={(event) => {
                        setTitleDraft(event.target.value);
                        setRenameError(null);
                        setRenameSuccessMessage(null);
                      }}
                      disabled={renameSaving}
                      aria-label="Session title"
                    />
                    <button
                      type="submit"
                      className="session-title-save"
                      disabled={renameSaving || trimmedTitleDraft.length === 0 || !hasRenameChanges}
                    >
                      {renameSaving ? '保存中...' : '保存标题'}
                    </button>
                  </div>
                  {renameError ? <div className="sessions-inline-error">{renameError}</div> : null}
                  {!renameError ? (
                    <div className={`session-title-status ${renameSuccessMessage ? 'session-title-status-success' : ''}`}>
                      {renameSaving
                        ? '正在保存标题...'
                        : renameSuccessMessage ??
                          (trimmedTitleDraft.length === 0
                            ? '标题不能为空。'
                            : hasRenameChanges
                              ? `将提交为: /title ${trimmedTitleDraft}`
                              : '标题未修改。')}
                    </div>
                  ) : null}
                </form>

                <div className="sessions-actions">
                  <button
                    type="button"
                    className="sessions-action-button sessions-action-button-primary"
                    onClick={() => void handleSetActiveSession()}
                    disabled={activeSession?.session.id === selectedSession.id}
                  >
                    {activeSession?.session.id === selectedSession.id
                      ? '当前 active handoff'
                      : '设为当前 handoff'}
                  </button>
                  <button
                    type="button"
                    className="sessions-action-button"
                    onClick={() => void handleClearActiveSession()}
                    disabled={!activeSession}
                  >
                    清除 handoff
                  </button>
                </div>

                <div className="session-detail-list">
                  <div className="session-detail-row">
                    <span>source</span>
                    <strong>{selectedSession.source}</strong>
                  </div>
                  <div className="session-detail-row">
                    <span>model</span>
                    <strong>{selectedSession.model_name ?? '-'}</strong>
                  </div>
                  <div className="session-detail-row">
                    <span>parent</span>
                    <strong>{selectedSession.parent_session_id ?? '-'}</strong>
                  </div>
                  <div className="session-detail-row">
                    <span>started</span>
                    <strong>{formatDateTime(selectedSession.started_at)}</strong>
                  </div>
                  <div className="session-detail-row">
                    <span>updated</span>
                    <strong>{formatDateTime(selectedSession.updated_at)}</strong>
                  </div>
                  <div className="session-detail-row">
                    <span>ended</span>
                    <strong>{formatDateTime(selectedSession.ended_at)}</strong>
                  </div>
                </div>

                <div className="detail-block">
                  <div className="detail-block-header">
                    <div>
                      <h4>Message Transcript</h4>
                      <p className="detail-block-copy">
                        当前 history surface 以时间顺序呈现，方便快速扫读角色、来源和最后一次交互。
                      </p>
                    </div>
                    <span className="sessions-pill sessions-pill-muted">
                      showing {transcriptStats.count} entries
                      {messageFilterRole !== 'all' || trimmedMessageFilterQuery
                        ? ' · filtered'
                        : ''}
                    </span>
                  </div>

                  <div className="transcript-stats-grid">
                    <article className="transcript-stat-card">
                      <span className="transcript-stat-label">Entries loaded</span>
                      <strong className="transcript-stat-value">{transcriptStats.count}</strong>
                    </article>
                    <article className="transcript-stat-card">
                      <span className="transcript-stat-label">Roles present</span>
                      <strong className="transcript-stat-value">{transcriptStats.uniqueRoles}</strong>
                    </article>
                    <article className="transcript-stat-card">
                      <span className="transcript-stat-label">Sources present</span>
                      <strong className="transcript-stat-value">{transcriptStats.uniqueSources}</strong>
                    </article>
                  </div>

                  <div className="transcript-filter-row">
                    <label className="transcript-filter-field">
                      <span>Role filter</span>
                      <select
                        value={messageFilterRole}
                        onChange={(event) =>
                          setMessageFilterRole(
                            event.target.value as (typeof transcriptFilterRoles)[number],
                          )
                        }
                      >
                        {transcriptFilterRoles.map((role) => (
                          <option key={role} value={role}>
                            {role}
                          </option>
                        ))}
                      </select>
                    </label>
                    <label className="transcript-filter-field transcript-filter-field-wide">
                      <span>Search transcript</span>
                      <input
                        type="search"
                        value={messageFilterQuery}
                        onChange={(event) => setMessageFilterQuery(event.target.value)}
                        placeholder="Filter message content"
                      />
                    </label>
                  </div>

                  <form className="session-composer" onSubmit={handleCreateMessage}>
                    <div className="session-composer-header">
                      <div>
                        <h5>Append to transcript</h5>
                        <p className="detail-block-copy">
                          New entries are written with `source=local` and appear in this session history immediately after save.
                        </p>
                      </div>
                      <span className="sessions-pill sessions-pill-muted">source: local</span>
                    </div>

                    <div className="session-composer-role-group" aria-label="Message role">
                      {transcriptComposerRoles.map((role) => {
                        const roleMeta = messageRoleMeta[role];
                        const isActive = messageRole === role;

                        return (
                          <button
                            key={role}
                            type="button"
                            className={`session-role-option ${isActive ? 'session-role-option-active' : ''}`}
                            onClick={() => setMessageRole(role)}
                            aria-pressed={isActive}
                          >
                            <span className="session-role-option-label">{roleMeta.label}</span>
                            <span className="session-role-option-copy">{roleMeta.description}</span>
                          </button>
                        );
                      })}
                    </div>

                    <label className="session-title-label" htmlFor="session-message-draft">
                      Transcript content
                    </label>
                    <textarea
                      id="session-message-draft"
                      className="session-composer-textarea"
                      value={messageDraft}
                      onChange={(event) => setMessageDraft(event.target.value)}
                      placeholder={composerPlaceholder}
                    />

                    <div className="session-composer-footer">
                      <div className="session-composer-hint">
                        {messageRoleMeta[messageRole].label} entries are styled distinctly in the transcript for faster review.
                      </div>
                      <div className="session-composer-actions">
                        <span className="session-composer-count">{messageDraftLength} chars</span>
                        <button type="submit" disabled={messageSaving || messageDraftLength === 0}>
                          {messageSaving ? '保存中...' : '追加到 Transcript'}
                        </button>
                      </div>
                    </div>
                  </form>

                  {transcriptMessages.length === 0 ? (
                    <div className="sessions-placeholder transcript-placeholder">
                      <p>暂无 message history。</p>
                    </div>
                  ) : null}

                  {transcriptMessages.length > 0 ? (
                    <div className="transcript-list" aria-label="Session transcript">
                      {transcriptMessages.map((item, index) => {
                        const roleMeta = messageRoleMeta[item.role];
                        const sourceTone = getMessageSourceTone(item.source);
                        const isLatestEntry = index === transcriptMessages.length - 1;

                        return (
                          <article
                            className={`transcript-entry transcript-entry-${roleMeta.tone}`}
                            key={item.id}
                          >
                            <div className="transcript-entry-rail">
                              <span className="transcript-entry-index">#{index + 1}</span>
                              <span className="transcript-entry-time">
                                {formatDateTime(item.created_at)}
                              </span>
                            </div>
                            <div className="transcript-entry-card">
                              <div className="transcript-entry-header">
                                <div className="transcript-entry-badges">
                                  <span
                                    className={`transcript-role-badge transcript-role-badge-${roleMeta.tone}`}
                                  >
                                    {roleMeta.label}
                                  </span>
                                  <span
                                    className={`transcript-source-badge transcript-source-badge-${sourceTone}`}
                                  >
                                    {item.source}
                                  </span>
                                  {isLatestEntry ? (
                                    <span className="transcript-latest-badge">Latest</span>
                                  ) : null}
                                </div>
                                <span className="transcript-entry-meta">
                                  {item.id.slice(0, 8)}
                                  {transcriptStats.latestMessage?.id === item.id
                                    ? ` · ${formatDateTime(transcriptStats.latestMessage.created_at)}`
                                    : null}
                                </span>
                              </div>
                              <p className="transcript-entry-copy">{item.content}</p>
                            </div>
                          </article>
                        );
                      })}
                    </div>
                  ) : null}
                </div>
              </div>
            </div>
          ) : null}
        </section>
      </div>
    </div>
  );
}
