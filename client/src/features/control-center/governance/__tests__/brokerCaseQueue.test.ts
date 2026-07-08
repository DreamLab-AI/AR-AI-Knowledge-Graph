// REC-2 / D3 (PRD-023 WP-4): broker event parsing + open-case bookkeeping.

import { describe, it, expect } from 'vitest';
import {
  applyBrokerEvent,
  openCaseIds,
  parseBrokerEvent,
  toCaseView,
  type InboxCase,
} from '../brokerCaseQueue';

describe('parseBrokerEvent', () => {
  it('parses a broker:new_case frame', () => {
    const ev = parseBrokerEvent({
      type: 'broker:new_case',
      channel: 'inbox',
      payload: { caseId: 'case-7', title: 'Elevate concept', category: 'knowledge_enrichment' },
    });
    expect(ev).toEqual({ kind: 'new_case', caseId: 'case-7', title: 'Elevate concept', category: 'knowledge_enrichment' });
  });

  it('parses a broker:case_decided frame', () => {
    const ev = parseBrokerEvent({
      type: 'broker:case_decided',
      channel: 'case:case-7',
      payload: { caseId: 'case-7', decisionId: 'dec-1', action: 'approve' },
    });
    expect(ev).toEqual({ kind: 'case_decided', caseId: 'case-7', action: 'approve', decisionId: 'dec-1' });
  });

  it('ignores non-broker multiplexed frames', () => {
    expect(parseBrokerEvent({ type: 'initialGraphLoad', nodes: [] })).toBeNull();
    expect(parseBrokerEvent({ type: 'broker:new_case', payload: {} })).toBeNull(); // no caseId
    expect(parseBrokerEvent(null)).toBeNull();
    expect(parseBrokerEvent('not-an-object')).toBeNull();
  });
});

describe('open-case bookkeeping', () => {
  const inbox: InboxCase[] = [
    { id: 'c1', status: 'pending' },
    { id: 'c2', status: 'claimed' },
    { id: 'c3', status: 'decided' },
  ];

  it('counts only not-yet-decided cases as open', () => {
    const open = openCaseIds(inbox);
    expect([...open].sort()).toEqual(['c1', 'c2']);
  });

  it('a new_case opens and a case_decided closes, without mutating the input set', () => {
    const start = openCaseIds(inbox); // {c1, c2}
    const afterNew = applyBrokerEvent(start, { kind: 'new_case', caseId: 'c9', title: 'x', category: 'y' });
    expect(afterNew.has('c9')).toBe(true);
    expect(start.has('c9')).toBe(false); // input untouched

    const afterDecided = applyBrokerEvent(afterNew, { kind: 'case_decided', caseId: 'c1', action: 'approve' });
    expect(afterDecided.has('c1')).toBe(false);
    expect(afterNew.has('c1')).toBe(true); // previous set untouched
  });
});

describe('toCaseView', () => {
  it('projects an inbox case, titling by target_path', () => {
    const view = toCaseView({
      id: 'case-7',
      category: 'knowledge_enrichment',
      status: 'pending',
      metadata: { target_path: 'pages/foo.md', content: 'body', proposed_by: 'did:nostr:aaaa' },
    });
    expect(view).toEqual({
      id: 'case-7',
      title: 'pages/foo.md',
      category: 'knowledge_enrichment',
      status: 'pending',
      targetPath: 'pages/foo.md',
      content: 'body',
      proposedBy: 'did:nostr:aaaa',
    });
  });

  it('falls back to the id as title and normalises unknown status to pending', () => {
    const view = toCaseView({ id: 'case-9', metadata: {} });
    expect(view.title).toBe('case-9');
    expect(view.status).toBe('pending');
  });
});
