import type { ActiveSessionSelection, Session, SessionMessage } from '../lib/tauri.ts';

export type SessionContinuityRole = 'selected' | 'active' | 'latest';

export interface SessionContinuityCard {
  session: Session;
  roles: SessionContinuityRole[];
  activeReason: string | null;
}

export interface TranscriptReplaySummary {
  count: number;
  latestMessage: SessionMessage | null;
  latestPreview: string | null;
  latestMessages: SessionMessage[];
}

function sortMessagesAscending(messages: SessionMessage[]) {
  return [...messages].sort(
    (left, right) =>
      new Date(left.created_at).getTime() - new Date(right.created_at).getTime(),
  );
}

function addCardRole(
  cards: Map<string, SessionContinuityCard>,
  role: SessionContinuityRole,
  session: Session | null,
  activeReason: string | null = null,
) {
  if (!session) {
    return;
  }

  const existing = cards.get(session.id);
  if (existing) {
    if (!existing.roles.includes(role)) {
      existing.roles.push(role);
    }
    if (role === 'active' && activeReason) {
      existing.activeReason = activeReason;
    }
    return;
  }

  cards.set(session.id, {
    session,
    roles: [role],
    activeReason: role === 'active' ? activeReason : null,
  });
}

export function buildSessionContinuityCards(input: {
  activeSession: ActiveSessionSelection | null;
  latestSession: Session | null;
  selectedSession: Session | null;
}): SessionContinuityCard[] {
  const cards = new Map<string, SessionContinuityCard>();

  addCardRole(cards, 'selected', input.selectedSession);
  addCardRole(cards, 'active', input.activeSession?.session ?? null, input.activeSession?.reason ?? null);
  addCardRole(cards, 'latest', input.latestSession);

  return Array.from(cards.values()).sort((left, right) => {
    const roleWeight = (roles: SessionContinuityRole[]) =>
      (roles.includes('selected') ? 100 : 0) +
      (roles.includes('active') ? 10 : 0) +
      (roles.includes('latest') ? 1 : 0);

    return roleWeight(right.roles) - roleWeight(left.roles);
  });
}

export function buildTranscriptReplaySummary(
  messages: SessionMessage[],
  previewLimit = 3,
): TranscriptReplaySummary {
  const ordered = sortMessagesAscending(messages);
  const latestMessage = ordered[ordered.length - 1] ?? null;

  return {
    count: ordered.length,
    latestMessage,
    latestPreview: latestMessage ? truncateTranscriptPreview(latestMessage.content) : null,
    latestMessages: ordered.slice(Math.max(ordered.length - previewLimit, 0)),
  };
}

export function truncateTranscriptPreview(content: string, maxLength = 140) {
  const compact = content.replace(/\s+/g, ' ').trim();
  if (compact.length <= maxLength) {
    return compact;
  }

  return `${compact.slice(0, Math.max(maxLength - 1, 1)).trimEnd()}…`;
}
