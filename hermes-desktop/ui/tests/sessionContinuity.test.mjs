import test from 'node:test';
import assert from 'node:assert/strict';
import {
  buildSessionContinuityCards,
  buildTranscriptReplaySummary,
  truncateTranscriptPreview,
} from '../src/routes/sessionContinuity.ts';

function createSession(id, title) {
  return {
    id,
    source: 'desktop',
    title,
    model_name: 'gpt-test',
    parent_session_id: null,
    started_at: '2026-04-26T08:00:00.000Z',
    updated_at: '2026-04-26T08:10:00.000Z',
    ended_at: null,
  };
}

function createActiveSelection(session, reason = 'manual_resume') {
  return {
    session,
    reason,
    activated_at: '2026-04-26T08:11:00.000Z',
  };
}

function createMessage(id, role, createdAt, content) {
  return {
    id,
    session_id: 'session-1',
    role,
    content,
    source: 'local',
    created_at: createdAt,
  };
}

test('buildSessionContinuityCards deduplicates active latest and selected into one card', () => {
  const session = createSession('session-1', 'Same Session');
  const cards = buildSessionContinuityCards({
    activeSession: createActiveSelection(session, 'continue_latest'),
    latestSession: session,
    selectedSession: session,
  });

  assert.equal(cards.length, 1);
  assert.deepEqual(cards[0]?.roles, ['selected', 'active', 'latest']);
  assert.equal(cards[0]?.activeReason, 'continue_latest');
});

test('buildSessionContinuityCards keeps distinct sessions ordered by continuity priority', () => {
  const selected = createSession('selected', 'Selected');
  const active = createSession('active', 'Active');
  const latest = createSession('latest', 'Latest');
  const cards = buildSessionContinuityCards({
    activeSession: createActiveSelection(active),
    latestSession: latest,
    selectedSession: selected,
  });

  assert.deepEqual(
    cards.map((card) => card.session.id),
    ['selected', 'active', 'latest'],
  );
});

test('buildTranscriptReplaySummary sorts messages chronologically and returns the latest tail', () => {
  const summary = buildTranscriptReplaySummary([
    createMessage('3', 'assistant', '2026-04-26T08:03:00.000Z', 'third'),
    createMessage('1', 'user', '2026-04-26T08:01:00.000Z', 'first'),
    createMessage('2', 'note', '2026-04-26T08:02:00.000Z', 'second'),
    createMessage('4', 'assistant', '2026-04-26T08:04:00.000Z', 'fourth'),
  ]);

  assert.equal(summary.count, 4);
  assert.equal(summary.latestMessage?.id, '4');
  assert.equal(summary.latestPreview, 'fourth');
  assert.deepEqual(
    summary.latestMessages.map((message) => message.id),
    ['2', '3', '4'],
  );
});

test('truncateTranscriptPreview normalizes whitespace and truncates long content', () => {
  assert.equal(truncateTranscriptPreview('  alpha   beta  '), 'alpha beta');
  assert.equal(
    truncateTranscriptPreview('a'.repeat(145), 12),
    'aaaaaaaaaaa…',
  );
});
