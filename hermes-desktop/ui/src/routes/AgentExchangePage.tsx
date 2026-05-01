import { useEffect, useMemo, useState, type FormEvent } from 'react';
import {
  agentExchangeDeleteMessage,
  agentExchangeDeleteRemoteUser,
  agentExchangeDraftOutbound,
  agentExchangeExportBundle,
  agentExchangeGetState,
  agentExchangeImportBundle,
  agentExchangeIngestInbound,
  agentExchangeListMessages,
  agentExchangeListRemoteUsers,
  agentExchangeRunFolderSync,
  agentExchangeUpdateMessageStatus,
  agentExchangeUpsertRemoteUser,
  type AgentExchangeBundle,
  type AgentExchangeDirection,
  type AgentExchangeMessage,
  type AgentExchangeMessageStatus,
  type AgentExchangeRemoteUser,
  type AgentExchangeRemoteUserStatus,
  type AgentExchangeRunFolderSyncResponse,
  type AgentExchangeState,
} from '../lib/tauri';
import './AgentExchangePage.css';

const defaultLocalAgentId = 'hermes-operator';
const defaultRemoteAgentId = 'peer-agent';
const defaultLimit = 50;
const agentExchangeBundleFilename = 'agent-exchange-bundle.json';

type DirectionFilter = AgentExchangeDirection | '';
type StatusFilter = AgentExchangeMessageStatus | '';
type RemoteUserStatusFilter = AgentExchangeRemoteUserStatus | '';
type BundlePreview = {
  exportedAt: string | null;
  messageCount: number;
  remoteUserCount: number;
  schemaVersion: number | null;
};

function parseOptionalJson(value: string): unknown | null {
  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }
  return JSON.parse(trimmed) as unknown;
}

function messageTitle(message: AgentExchangeMessage) {
  return message.subject?.trim() || `${message.direction} message`;
}

function statusTone(status: AgentExchangeMessageStatus) {
  if (status === 'received') {
    return 'agent-exchange-pill agent-exchange-pill-green';
  }
  if (status === 'draft') {
    return 'agent-exchange-pill agent-exchange-pill-blue';
  }
  if (status === 'sent') {
    return 'agent-exchange-pill agent-exchange-pill-indigo';
  }
  return 'agent-exchange-pill';
}

function canMarkSent(message: AgentExchangeMessage) {
  return message.direction === 'outbound' && message.status === 'draft';
}

function canArchive(message: AgentExchangeMessage) {
  return message.status !== 'archived';
}

function canRestore(message: AgentExchangeMessage) {
  return message.status === 'archived';
}

function restoreStatusForMessage(message: AgentExchangeMessage): AgentExchangeMessageStatus {
  return message.direction === 'outbound' ? 'draft' : 'received';
}

function parseBundlePreview(value: string): BundlePreview | null {
  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }

  try {
    const parsed = JSON.parse(trimmed) as Partial<AgentExchangeBundle>;
    const messageCount = Array.isArray(parsed.messages) ? parsed.messages.length : null;
    const remoteUserCount = Array.isArray(parsed.remote_users) ? parsed.remote_users.length : null;
    if (messageCount === null && remoteUserCount === null) {
      return null;
    }
    return {
      exportedAt: typeof parsed.exported_at === 'string' ? parsed.exported_at : null,
      messageCount: messageCount ?? 0,
      remoteUserCount: remoteUserCount ?? 0,
      schemaVersion: typeof parsed.schema_version === 'number' ? parsed.schema_version : null,
    };
  } catch {
    return null;
  }
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

export function AgentExchangePage() {
  const [state, setState] = useState<AgentExchangeState | null>(null);
  const [messages, setMessages] = useState<AgentExchangeMessage[]>([]);
  const [remoteUsers, setRemoteUsers] = useState<AgentExchangeRemoteUser[]>([]);
  const [loading, setLoading] = useState(true);
  const [actionLoading, setActionLoading] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [directionFilter, setDirectionFilter] = useState<DirectionFilter>('');
  const [statusFilter, setStatusFilter] = useState<StatusFilter>('');
  const [threadFilter, setThreadFilter] = useState('');
  const [remoteFilter, setRemoteFilter] = useState('');
  const [remoteUserFilter, setRemoteUserFilter] = useState('');

  const [remoteUserQuery, setRemoteUserQuery] = useState('');
  const [remoteUserStatusFilter, setRemoteUserStatusFilter] = useState<RemoteUserStatusFilter>('');
  const [remoteUserId, setRemoteUserId] = useState('');
  const [remoteUserDisplayName, setRemoteUserDisplayName] = useState('');
  const [remoteUserDefaultAgentId, setRemoteUserDefaultAgentId] = useState('');
  const [remoteUserTransportLabel, setRemoteUserTransportLabel] = useState('');
  const [remoteUserRouteHint, setRemoteUserRouteHint] = useState('');
  const [remoteUserStatus, setRemoteUserStatus] = useState<AgentExchangeRemoteUserStatus>('active');

  const [outboundLocalAgentId, setOutboundLocalAgentId] = useState(defaultLocalAgentId);
  const [outboundRemoteAgentId, setOutboundRemoteAgentId] = useState(defaultRemoteAgentId);
  const [outboundRemoteUserId, setOutboundRemoteUserId] = useState('');
  const [outboundThreadId, setOutboundThreadId] = useState('');
  const [outboundSubject, setOutboundSubject] = useState('');
  const [outboundBody, setOutboundBody] = useState('');
  const [outboundPayload, setOutboundPayload] = useState('{\n  "kind": "handoff"\n}');

  const [inboundLocalAgentId, setInboundLocalAgentId] = useState(defaultLocalAgentId);
  const [inboundRemoteAgentId, setInboundRemoteAgentId] = useState(defaultRemoteAgentId);
  const [inboundRemoteUserId, setInboundRemoteUserId] = useState('');
  const [inboundThreadId, setInboundThreadId] = useState('');
  const [inboundSubject, setInboundSubject] = useState('');
  const [inboundBody, setInboundBody] = useState('');
  const [inboundSourceMessageId, setInboundSourceMessageId] = useState('');
  const [inboundPayload, setInboundPayload] = useState('');

  const [bundleText, setBundleText] = useState('');
  const [importLocalAgentId, setImportLocalAgentId] = useState(defaultLocalAgentId);
  const [importAsInbound, setImportAsInbound] = useState(true);
  const [folderSyncPath, setFolderSyncPath] = useState('');
  const [folderSyncResult, setFolderSyncResult] = useState<AgentExchangeRunFolderSyncResponse | null>(null);

  useEffect(() => {
    void loadMailbox();
  }, []);

  const outboundCount = useMemo(
    () => state?.messages.filter((message) => message.direction === 'outbound').length ?? 0,
    [state],
  );
  const inboundCount = useMemo(
    () => state?.messages.filter((message) => message.direction === 'inbound').length ?? 0,
    [state],
  );
  const latestMessage = messages[0] ?? state?.messages[0] ?? null;
  const bundlePreview = useMemo(() => parseBundlePreview(bundleText), [bundleText]);

  async function loadRemoteUserDirectory() {
    const directory = await agentExchangeListRemoteUsers({
      query: remoteUserQuery.trim() || null,
      status: remoteUserStatusFilter || null,
      limit: defaultLimit,
    });
    setRemoteUsers(directory);
    return directory;
  }

  async function loadMailbox() {
    setLoading(true);
    setError(null);
    try {
      const [mailboxState, filteredMessages, directory] = await Promise.all([
        agentExchangeGetState(),
        agentExchangeListMessages({
          direction: directionFilter || null,
          status: statusFilter || null,
          thread_id: threadFilter.trim() || null,
          remote_agent_id: remoteFilter.trim() || null,
          remote_user_id: remoteUserFilter.trim() || null,
          limit: defaultLimit,
        }),
        agentExchangeListRemoteUsers({
          query: remoteUserQuery.trim() || null,
          status: remoteUserStatusFilter || null,
          limit: defaultLimit,
        }),
      ]);
      setState(mailboxState);
      setMessages(filteredMessages);
      setRemoteUsers(directory);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }

  async function refreshFilteredMessages() {
    setError(null);
    try {
      setMessages(
        await agentExchangeListMessages({
          direction: directionFilter || null,
          status: statusFilter || null,
          thread_id: threadFilter.trim() || null,
          remote_agent_id: remoteFilter.trim() || null,
          remote_user_id: remoteUserFilter.trim() || null,
          limit: defaultLimit,
        }),
      );
      setState(await agentExchangeGetState());
      await loadRemoteUserDirectory();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  function clearRemoteUserForm() {
    setRemoteUserId('');
    setRemoteUserDisplayName('');
    setRemoteUserDefaultAgentId('');
    setRemoteUserTransportLabel('');
    setRemoteUserRouteHint('');
    setRemoteUserStatus('active');
  }

  function editRemoteUser(remoteUser: AgentExchangeRemoteUser) {
    setRemoteUserId(remoteUser.user_id);
    setRemoteUserDisplayName(remoteUser.display_name);
    setRemoteUserDefaultAgentId(remoteUser.default_agent_id);
    setRemoteUserTransportLabel(remoteUser.transport_label ?? '');
    setRemoteUserRouteHint(remoteUser.route_hint ?? '');
    setRemoteUserStatus(remoteUser.status);
  }

  async function handleSaveRemoteUser(event: FormEvent) {
    event.preventDefault();
    setActionLoading('remote-user-save');
    setError(null);
    setNotice(null);
    try {
      const remoteUser = await agentExchangeUpsertRemoteUser({
        user_id: remoteUserId,
        display_name: remoteUserDisplayName,
        default_agent_id: remoteUserDefaultAgentId,
        transport_label: remoteUserTransportLabel.trim() || null,
        route_hint: remoteUserRouteHint.trim() || null,
        status: remoteUserStatus,
      });
      editRemoteUser(remoteUser);
      setNotice(`Saved future remote user ${remoteUser.user_id} for local routing metadata.`);
      await refreshFilteredMessages();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setActionLoading(null);
    }
  }

  async function handleDeleteRemoteUser(remoteUser: AgentExchangeRemoteUser) {
    setActionLoading(`remote-user-delete:${remoteUser.user_id}`);
    setError(null);
    setNotice(null);
    try {
      const nextState = await agentExchangeDeleteRemoteUser({ user_id: remoteUser.user_id });
      setState(nextState);
      setNotice(`Deleted remote user profile ${remoteUser.user_id}. Existing messages remain intact.`);
      if (remoteUserId.trim() === remoteUser.user_id) {
        clearRemoteUserForm();
      }
      await refreshFilteredMessages();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setActionLoading(null);
    }
  }

  function useRemoteUserForOutbound(remoteUser: AgentExchangeRemoteUser) {
    setOutboundRemoteUserId(remoteUser.user_id);
    setOutboundRemoteAgentId(remoteUser.default_agent_id);
    setNotice(`Using ${remoteUser.user_id} as outbound remote user routing metadata.`);
  }

  function useRemoteUserForInbound(remoteUser: AgentExchangeRemoteUser) {
    setInboundRemoteUserId(remoteUser.user_id);
    setInboundRemoteAgentId(remoteUser.default_agent_id);
    setNotice(`Using ${remoteUser.user_id} as inbound remote user metadata for local ingest.`);
  }

  async function filterMailboxByRemoteUser(remoteUser: AgentExchangeRemoteUser) {
    setRemoteUserFilter(remoteUser.user_id);
    setError(null);
    try {
      const filtered = await agentExchangeListMessages({
        direction: directionFilter || null,
        status: statusFilter || null,
        thread_id: threadFilter.trim() || null,
        remote_agent_id: remoteFilter.trim() || null,
        remote_user_id: remoteUser.user_id,
        limit: defaultLimit,
      });
      setMessages(filtered);
      setState(await agentExchangeGetState());
      setNotice(`Mailbox filter set to remote user ${remoteUser.user_id}.`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function handleDraftOutbound(event: FormEvent) {
    event.preventDefault();
    setActionLoading('draft');
    setError(null);
    setNotice(null);
    try {
      const message = await agentExchangeDraftOutbound({
        local_agent_id: outboundLocalAgentId,
        remote_agent_id: outboundRemoteAgentId,
        remote_user_id: outboundRemoteUserId.trim() || null,
        thread_id: outboundThreadId.trim() || null,
        subject: outboundSubject.trim() || null,
        body: outboundBody,
        payload_json: parseOptionalJson(outboundPayload),
      });
      setOutboundThreadId(message.thread_id);
      setOutboundSubject('');
      setOutboundBody('');
      setNotice(`Drafted outbound message ${message.id}. Export a bundle to hand it to the peer agent.`);
      await refreshFilteredMessages();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setActionLoading(null);
    }
  }

  async function handleIngestInbound(event: FormEvent) {
    event.preventDefault();
    setActionLoading('ingest');
    setError(null);
    setNotice(null);
    try {
      const message = await agentExchangeIngestInbound({
        local_agent_id: inboundLocalAgentId,
        remote_agent_id: inboundRemoteAgentId,
        remote_user_id: inboundRemoteUserId.trim() || null,
        thread_id: inboundThreadId.trim() || null,
        subject: inboundSubject.trim() || null,
        body: inboundBody,
        payload_json: parseOptionalJson(inboundPayload),
        source_message_id: inboundSourceMessageId.trim() || null,
      });
      setInboundThreadId(message.thread_id);
      setInboundSubject('');
      setInboundBody('');
      setInboundSourceMessageId('');
      setNotice(`Ingested inbound message ${message.id}.`);
      await refreshFilteredMessages();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setActionLoading(null);
    }
  }

  async function handleExportBundle() {
    setActionLoading('export');
    setError(null);
    setNotice(null);
    try {
      const bundle = await agentExchangeExportBundle({
        direction: directionFilter || null,
        status: statusFilter || null,
        thread_id: threadFilter.trim() || null,
        remote_agent_id: remoteFilter.trim() || null,
        remote_user_id: remoteUserFilter.trim() || null,
        limit: defaultLimit,
      });
      setBundleText(JSON.stringify(bundle, null, 2));
      setNotice(
        `Exported ${bundle.messages.length} message(s) and ${bundle.remote_users.length} remote user profile(s) into a portable local JSON bundle for out-of-band handoff.`,
      );
      setState(await agentExchangeGetState());
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setActionLoading(null);
    }
  }

  async function handleImportBundle() {
    setActionLoading('import');
    setError(null);
    setNotice(null);
    setFolderSyncResult(null);
    try {
      const bundle = JSON.parse(bundleText) as AgentExchangeBundle;
      const result = await agentExchangeImportBundle({
        bundle,
        local_agent_id: importLocalAgentId.trim() || null,
        as_inbound: importAsInbound,
      });
      setState(result.state);
      setNotice(
        `Imported ${result.imported_count} message(s), skipped ${result.skipped_count} duplicate(s).`,
      );
      await refreshFilteredMessages();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setActionLoading(null);
    }
  }

  function handleDownloadBundle() {
    setError(null);
    setNotice(null);
    try {
      const parsed = JSON.parse(bundleText) as unknown;
      downloadJsonFile(agentExchangeBundleFilename, JSON.stringify(parsed, null, 2));
      setNotice(
        `Downloaded ${agentExchangeBundleFilename} as a local out-of-band handoff file. This is not proof of remote delivery.`,
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function handleRunFolderSync() {
    setActionLoading('folder-sync');
    setError(null);
    setNotice(null);
    try {
      const result = await agentExchangeRunFolderSync({
        path: folderSyncPath.trim(),
        local_agent_id: importLocalAgentId.trim() || null,
        as_inbound: importAsInbound,
      });
      setFolderSyncResult(result);
      setState(result.state);
      setNotice(
        `Synced ${result.path}: imported ${result.imported_count}, skipped ${result.skipped_count}, exported ${result.exported_count}.`,
      );
      await refreshFilteredMessages();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setActionLoading(null);
    }
  }

  async function handleUpdateMessageStatus(
    message: AgentExchangeMessage,
    status: AgentExchangeMessageStatus,
  ) {
    setActionLoading(`status:${message.id}:${status}`);
    setError(null);
    setNotice(null);
    try {
      const updated = await agentExchangeUpdateMessageStatus({
        message_id: message.id,
        status,
      });
      setNotice(`Updated message ${updated.id} to ${updated.status}.`);
      await refreshFilteredMessages();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setActionLoading(null);
    }
  }

  async function handleDeleteMessage(message: AgentExchangeMessage) {
    setActionLoading(`delete:${message.id}`);
    setError(null);
    setNotice(null);
    try {
      const nextState = await agentExchangeDeleteMessage({ message_id: message.id });
      setState(nextState);
      setNotice(`Deleted message ${message.id}.`);
      await refreshFilteredMessages();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setActionLoading(null);
    }
  }

  return (
    <div className="agent-exchange-page">
      <div className="agent-exchange-hero">
        <div>
          <p className="agent-exchange-eyebrow">Local-only mailbox</p>
          <h2>Agent Exchange</h2>
          <p>
            Reserve communication with future remote users and their agents through a local,
            auditable JSON bundle mailbox. This page records remote user identity for handoff
            readiness, but does not call a remote service or claim realtime delivery.
          </p>
        </div>
        <div className="agent-exchange-stats">
          <div>
            <span>{state?.messages.length ?? 0}</span>
            <small>messages</small>
          </div>
          <div>
            <span>{outboundCount}</span>
            <small>outbound</small>
          </div>
          <div>
            <span>{inboundCount}</span>
            <small>inbound</small>
          </div>
          <div>
            <span>{state?.remote_users.length ?? 0}</span>
            <small>remote users</small>
          </div>
        </div>
      </div>

      {error ? <div className="agent-exchange-banner agent-exchange-banner-error">{error}</div> : null}
      {notice ? <div className="agent-exchange-banner">{notice}</div> : null}

      <section className="agent-exchange-panel agent-exchange-panel-compact">
        <div className="agent-exchange-panel-header">
          <div>
            <h3>Mailbox Filters</h3>
            <p>Filter the local message list and reuse the same scope for bundle export.</p>
          </div>
          <button type="button" className="agent-exchange-button" onClick={() => void loadMailbox()}>
            Refresh
          </button>
        </div>
        <div className="agent-exchange-filter-grid">
          <label>
            Direction
            <select
              value={directionFilter}
              onChange={(event) => setDirectionFilter(event.target.value as DirectionFilter)}
            >
              <option value="">all</option>
              <option value="inbound">inbound</option>
              <option value="outbound">outbound</option>
            </select>
          </label>
          <label>
            Status
            <select
              value={statusFilter}
              onChange={(event) => setStatusFilter(event.target.value as StatusFilter)}
            >
              <option value="">all</option>
              <option value="draft">draft</option>
              <option value="sent">sent</option>
              <option value="received">received</option>
              <option value="archived">archived</option>
            </select>
          </label>
          <label>
            Thread id
            <input value={threadFilter} onChange={(event) => setThreadFilter(event.target.value)} />
          </label>
          <label>
            Remote agent id
            <input value={remoteFilter} onChange={(event) => setRemoteFilter(event.target.value)} />
          </label>
          <label>
            Remote user id
            <input value={remoteUserFilter} onChange={(event) => setRemoteUserFilter(event.target.value)} />
            <small>Exact remote_user_id scope; directory actions can fill this.</small>
          </label>
        </div>
        <div className="agent-exchange-actions">
          <button type="button" className="agent-exchange-button agent-exchange-button-primary" onClick={() => void refreshFilteredMessages()}>
            Apply filters
          </button>
          <button type="button" className="agent-exchange-button" onClick={handleExportBundle} disabled={actionLoading === 'export'}>
            {actionLoading === 'export' ? 'Exporting...' : 'Export scoped bundle'}
          </button>
        </div>
      </section>

      <section className="agent-exchange-panel agent-exchange-panel-compact">
        <div className="agent-exchange-panel-header">
          <div>
            <h3>Future Remote Users</h3>
            <p>
              Maintain a local directory for future remote user routing metadata. Profiles help fill
              outbound drafts and mailbox filters, but they do not deliver messages to remote accounts.
            </p>
          </div>
          <div className="agent-exchange-directory-controls">
            <input
              value={remoteUserQuery}
              onChange={(event) => setRemoteUserQuery(event.target.value)}
              placeholder="Search remote users"
            />
            <select
              value={remoteUserStatusFilter}
              onChange={(event) =>
                setRemoteUserStatusFilter(event.target.value as RemoteUserStatusFilter)
              }
            >
              <option value="">all statuses</option>
              <option value="active">active</option>
              <option value="paused">paused</option>
              <option value="blocked">blocked</option>
            </select>
            <button
              type="button"
              className="agent-exchange-button"
              onClick={() => void loadRemoteUserDirectory()}
            >
              Refresh directory
            </button>
          </div>
        </div>

        <form className="agent-exchange-remote-user-form" onSubmit={(event) => void handleSaveRemoteUser(event)}>
          <div className="agent-exchange-form-grid agent-exchange-form-grid-wide">
            <label>
              User id
              <input value={remoteUserId} onChange={(event) => setRemoteUserId(event.target.value)} required />
            </label>
            <label>
              Display name
              <input value={remoteUserDisplayName} onChange={(event) => setRemoteUserDisplayName(event.target.value)} required />
            </label>
            <label>
              Default agent id
              <input value={remoteUserDefaultAgentId} onChange={(event) => setRemoteUserDefaultAgentId(event.target.value)} required />
            </label>
            <label>
              Status
              <select
                value={remoteUserStatus}
                onChange={(event) => setRemoteUserStatus(event.target.value as AgentExchangeRemoteUserStatus)}
              >
                <option value="active">active</option>
                <option value="paused">paused</option>
                <option value="blocked">blocked</option>
              </select>
            </label>
            <label>
              Transport label
              <input value={remoteUserTransportLabel} onChange={(event) => setRemoteUserTransportLabel(event.target.value)} />
            </label>
            <label>
              Route hint
              <input value={remoteUserRouteHint} onChange={(event) => setRemoteUserRouteHint(event.target.value)} />
            </label>
          </div>
          <div className="agent-exchange-actions">
            <button
              type="submit"
              className="agent-exchange-button agent-exchange-button-primary"
              disabled={actionLoading === 'remote-user-save'}
            >
              {actionLoading === 'remote-user-save' ? 'Saving...' : 'Save remote user'}
            </button>
            <button type="button" className="agent-exchange-button" onClick={clearRemoteUserForm}>
              Clear form
            </button>
          </div>
        </form>

        <div className="agent-exchange-remote-user-list">
          {remoteUsers.length === 0 ? (
            <div className="agent-exchange-empty">
              No future remote users match the current directory filters.
            </div>
          ) : null}
          {remoteUsers.map((remoteUser) => (
            <article className="agent-exchange-remote-user" key={remoteUser.user_id}>
              <div>
                <strong>{remoteUser.display_name}</strong>
                <span><code>{remoteUser.user_id}</code> · agent {remoteUser.default_agent_id}</span>
                <small>
                  {remoteUser.status}
                  {remoteUser.transport_label ? ` · ${remoteUser.transport_label}` : ''}
                  {remoteUser.route_hint ? ` · ${remoteUser.route_hint}` : ''}
                </small>
              </div>
              <div className="agent-exchange-actions agent-exchange-remote-user-actions">
                <button type="button" className="agent-exchange-button" onClick={() => editRemoteUser(remoteUser)}>
                  Edit
                </button>
                <button type="button" className="agent-exchange-button" onClick={() => useRemoteUserForOutbound(remoteUser)}>
                  Use for outbound draft
                </button>
                <button type="button" className="agent-exchange-button" onClick={() => useRemoteUserForInbound(remoteUser)}>
                  Use for inbound ingest
                </button>
                <button type="button" className="agent-exchange-button" onClick={() => void filterMailboxByRemoteUser(remoteUser)}>
                  Filter mailbox
                </button>
                <button
                  type="button"
                  className="agent-exchange-button agent-exchange-button-danger"
                  onClick={() => void handleDeleteRemoteUser(remoteUser)}
                  disabled={actionLoading === `remote-user-delete:${remoteUser.user_id}`}
                >
                  {actionLoading === `remote-user-delete:${remoteUser.user_id}` ? 'Deleting...' : 'Delete'}
                </button>
              </div>
            </article>
          ))}
        </div>
      </section>

      <div className="agent-exchange-grid">
        <form className="agent-exchange-panel" onSubmit={(event) => void handleDraftOutbound(event)}>
          <div className="agent-exchange-panel-header">
            <div>
              <h3>Draft Outbound</h3>
              <p>Create a local draft, then export it as a bundle for another user-owned agent.</p>
            </div>
          </div>
          <div className="agent-exchange-form-grid">
            <label>
              Local agent id
              <input value={outboundLocalAgentId} onChange={(event) => setOutboundLocalAgentId(event.target.value)} required />
            </label>
            <label>
              Remote agent id
              <input value={outboundRemoteAgentId} onChange={(event) => setOutboundRemoteAgentId(event.target.value)} required />
            </label>
            <label>
              Remote user id
              <input value={outboundRemoteUserId} onChange={(event) => setOutboundRemoteUserId(event.target.value)} />
              <small>Future remote user identity; directory mapping can fill the default agent route.</small>
            </label>
            <label>
              Thread id
              <input value={outboundThreadId} onChange={(event) => setOutboundThreadId(event.target.value)} />
            </label>
          </div>
          <label className="agent-exchange-field">
            Subject
            <input value={outboundSubject} onChange={(event) => setOutboundSubject(event.target.value)} />
          </label>
          <label className="agent-exchange-field">
            Body
            <textarea rows={5} value={outboundBody} onChange={(event) => setOutboundBody(event.target.value)} required />
          </label>
          <label className="agent-exchange-field">
            Payload JSON
            <textarea rows={5} value={outboundPayload} onChange={(event) => setOutboundPayload(event.target.value)} />
          </label>
          <button type="submit" className="agent-exchange-button agent-exchange-button-primary" disabled={actionLoading === 'draft'}>
            {actionLoading === 'draft' ? 'Drafting...' : 'Draft outbound message'}
          </button>
        </form>

        <form className="agent-exchange-panel" onSubmit={(event) => void handleIngestInbound(event)}>
          <div className="agent-exchange-panel-header">
            <div>
              <h3>Ingest Inbound</h3>
              <p>
                Manually record a peer response into the local mailbox after an out-of-band
                handoff; Hermes only stores the local ingest record here.
              </p>
            </div>
          </div>
          <div className="agent-exchange-form-grid">
            <label>
              Local agent id
              <input value={inboundLocalAgentId} onChange={(event) => setInboundLocalAgentId(event.target.value)} required />
            </label>
            <label>
              Remote agent id
              <input value={inboundRemoteAgentId} onChange={(event) => setInboundRemoteAgentId(event.target.value)} required />
            </label>
            <label>
              Remote user id
              <input value={inboundRemoteUserId} onChange={(event) => setInboundRemoteUserId(event.target.value)} />
              <small>Future remote user identity captured from the peer handoff and local directory mapping.</small>
            </label>
            <label>
              Thread id
              <input value={inboundThreadId} onChange={(event) => setInboundThreadId(event.target.value)} />
            </label>
          </div>
          <label className="agent-exchange-field">
            Source message id
            <input value={inboundSourceMessageId} onChange={(event) => setInboundSourceMessageId(event.target.value)} />
          </label>
          <label className="agent-exchange-field">
            Subject
            <input value={inboundSubject} onChange={(event) => setInboundSubject(event.target.value)} />
          </label>
          <label className="agent-exchange-field">
            Body
            <textarea rows={5} value={inboundBody} onChange={(event) => setInboundBody(event.target.value)} required />
          </label>
          <label className="agent-exchange-field">
            Payload JSON
            <textarea rows={4} value={inboundPayload} onChange={(event) => setInboundPayload(event.target.value)} />
          </label>
          <button type="submit" className="agent-exchange-button agent-exchange-button-primary" disabled={actionLoading === 'ingest'}>
            {actionLoading === 'ingest' ? 'Ingesting...' : 'Ingest inbound message'}
          </button>
        </form>
      </div>

      <div className="agent-exchange-grid agent-exchange-grid-wide">
        <section className="agent-exchange-panel">
          <div className="agent-exchange-panel-header">
            <div>
              <h3>Bundle Transfer</h3>
              <p>
                Exported JSON can be handed to a future remote user or that user&apos;s agent
                through an approved out-of-band channel, or synced through a shared file path.
                Hermes prepares the local bundle only and does not perform live remote delivery.
              </p>
            </div>
          </div>
          <div className="agent-exchange-form-grid">
            <label>
              Import local agent id
              <input value={importLocalAgentId} onChange={(event) => setImportLocalAgentId(event.target.value)} />
            </label>
            <label className="agent-exchange-checkbox">
              <input type="checkbox" checked={importAsInbound} onChange={(event) => setImportAsInbound(event.target.checked)} />
              Import bundle messages as inbound
            </label>
          </div>
          <label className="agent-exchange-field">
            Shared-file sync path
            <input
              value={folderSyncPath}
              onChange={(event) => setFolderSyncPath(event.target.value)}
              placeholder="/path/to/agent-exchange"
            />
          </label>
          <label className="agent-exchange-field">
            Agent exchange bundle JSON
            <textarea rows={12} value={bundleText} onChange={(event) => setBundleText(event.target.value)} />
          </label>
          {bundlePreview ? (
            <div className="agent-exchange-bundle-preview">
              <strong>Parsed bundle preview</strong>
              <div className="agent-exchange-message-meta">
                <span>messages: {bundlePreview.messageCount}</span>
                <span>remote user profiles: {bundlePreview.remoteUserCount}</span>
                {bundlePreview.schemaVersion !== null ? (
                  <span>schema: v{bundlePreview.schemaVersion}</span>
                ) : null}
                {bundlePreview.exportedAt ? <span>exported: {bundlePreview.exportedAt}</span> : null}
              </div>
              <p>Preview is computed from local JSON only and does not imply remote delivery.</p>
            </div>
          ) : null}
          <div className="agent-exchange-actions">
            <button type="button" className="agent-exchange-button" onClick={handleExportBundle} disabled={actionLoading === 'export'}>
              Export scoped bundle
            </button>
            <button type="button" className="agent-exchange-button" onClick={handleDownloadBundle} disabled={!bundleText.trim()}>
              Download bundle JSON
            </button>
            <button type="button" className="agent-exchange-button agent-exchange-button-primary" onClick={handleImportBundle} disabled={actionLoading === 'import' || !bundleText.trim()}>
              {actionLoading === 'import' ? 'Importing...' : 'Import bundle'}
            </button>
            <button
              type="button"
              className="agent-exchange-button"
              onClick={handleRunFolderSync}
              disabled={actionLoading === 'folder-sync' || !folderSyncPath.trim()}
            >
              {actionLoading === 'folder-sync' ? 'Syncing...' : 'Run file sync'}
            </button>
          </div>
          {folderSyncResult ? (
            <div className="agent-exchange-message-meta">
              <span>path: {folderSyncResult.path}</span>
              <span>synced: {folderSyncResult.synced_at}</span>
              <span>imported: {folderSyncResult.imported_count}</span>
              <span>skipped: {folderSyncResult.skipped_count}</span>
              <span>exported: {folderSyncResult.exported_count}</span>
            </div>
          ) : null}
        </section>

        <section className="agent-exchange-panel">
          <div className="agent-exchange-panel-header">
            <div>
              <h3>Local Mailbox</h3>
              <p>
                {loading
                  ? 'Loading messages...'
                  : `${messages.length} filtered message(s). Latest update: ${latestMessage?.updated_at ?? '-'}`}
              </p>
            </div>
          </div>
          <div className="agent-exchange-message-list">
            {!loading && messages.length === 0 ? (
              <div className="agent-exchange-empty">No local messages match the current filters.</div>
            ) : null}
            {messages.map((message) => (
              <article className="agent-exchange-message" key={message.id}>
                <div className="agent-exchange-message-header">
                  <div>
                    <strong>{messageTitle(message)}</strong>
                    <span>{message.thread_id}</span>
                  </div>
                  <div className="agent-exchange-message-pills">
                    <span className="agent-exchange-pill">{message.direction}</span>
                    <span className={statusTone(message.status)}>{message.status}</span>
                  </div>
                </div>
                <p>{message.body}</p>
                <div className="agent-exchange-message-meta">
                  <span>local: {message.local_agent_id}</span>
                  <span>remote: {message.remote_agent_id}</span>
                  <span>user: {message.remote_user_id ?? '-'}</span>
                  <span>source: {message.source_message_id ?? '-'}</span>
                  <span>updated: {message.updated_at}</span>
                </div>
                <div className="agent-exchange-actions agent-exchange-message-actions">
                  {canMarkSent(message) ? (
                    <button
                      type="button"
                      className="agent-exchange-button"
                      onClick={() => void handleUpdateMessageStatus(message, 'sent')}
                      disabled={actionLoading === `status:${message.id}:sent`}
                    >
                      {actionLoading === `status:${message.id}:sent` ? 'Saving...' : 'Mark sent'}
                    </button>
                  ) : null}
                  {canArchive(message) ? (
                    <button
                      type="button"
                      className="agent-exchange-button"
                      onClick={() => void handleUpdateMessageStatus(message, 'archived')}
                      disabled={actionLoading === `status:${message.id}:archived`}
                    >
                      {actionLoading === `status:${message.id}:archived` ? 'Saving...' : 'Archive'}
                    </button>
                  ) : null}
                  {canRestore(message) ? (
                    <button
                      type="button"
                      className="agent-exchange-button"
                      onClick={() =>
                        void handleUpdateMessageStatus(message, restoreStatusForMessage(message))
                      }
                      disabled={
                        actionLoading
                        === `status:${message.id}:${restoreStatusForMessage(message)}`
                      }
                    >
                      {actionLoading
                        === `status:${message.id}:${restoreStatusForMessage(message)}`
                        ? 'Saving...'
                        : 'Restore'}
                    </button>
                  ) : null}
                  <button
                    type="button"
                    className="agent-exchange-button agent-exchange-button-danger"
                    onClick={() => void handleDeleteMessage(message)}
                    disabled={actionLoading === `delete:${message.id}`}
                  >
                    {actionLoading === `delete:${message.id}` ? 'Deleting...' : 'Delete'}
                  </button>
                </div>
                {message.payload_json ? (
                  <pre>{JSON.stringify(message.payload_json, null, 2)}</pre>
                ) : null}
              </article>
            ))}
          </div>
        </section>
      </div>
    </div>
  );
}
