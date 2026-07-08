// REC-2 / D3 (PRD-023 WP-4): pure logic for the control-centre broker case
// queue and the ambient ACSP indicator.
//
// Renderer-free and DOM-free so the wire parsing (broker:new_case /
// broker:case_decided) and the open-case bookkeeping are unit-testable without a
// live socket. The stateful consumer that fetches `/api/broker/inbox`, wires the
// socket and calls the decide route is `useBrokerCaseQueue`.

/** A broker case as the control centre renders it. */
export interface CaseView {
  id: string;
  title: string;
  category: string;
  status: 'pending' | 'claimed' | 'decided';
  targetPath?: string;
  content?: string;
  proposedBy?: string;
}

/** The two broker events P0 (`services/broker_events.rs`) publishes. */
export type BrokerCaseEvent =
  | { kind: 'new_case'; caseId: string; title: string; category: string }
  | { kind: 'case_decided'; caseId: string; action: string; decisionId?: string };

interface RawWsMessage {
  type?: string;
  channel?: string;
  payload?: Record<string, unknown>;
}

function payStr(p: Record<string, unknown> | undefined, key: string): string | undefined {
  const v = p?.[key];
  return typeof v === 'string' && v.length > 0 ? v : undefined;
}

/**
 * Parse an inbound WS text message into a broker case event, or `null` when the
 * message is anything else (the same socket multiplexes every frame, so the
 * queue must ignore non-broker traffic).
 */
export function parseBrokerEvent(message: unknown): BrokerCaseEvent | null {
  if (!message || typeof message !== 'object') return null;
  const m = message as RawWsMessage;
  const caseId = payStr(m.payload, 'caseId');
  if (!caseId) return null;

  if (m.type === 'broker:new_case') {
    return {
      kind: 'new_case',
      caseId,
      title: payStr(m.payload, 'title') ?? caseId,
      category: payStr(m.payload, 'category') ?? 'knowledge_enrichment',
    };
  }
  if (m.type === 'broker:case_decided') {
    return {
      kind: 'case_decided',
      caseId,
      action: payStr(m.payload, 'action') ?? 'decided',
      decisionId: payStr(m.payload, 'decisionId'),
    };
  }
  return null;
}

/** The subset of an `/api/broker/inbox` case object the queue consumes. */
export interface InboxCase {
  id: string;
  category?: string;
  status?: string;
  metadata?: Record<string, unknown> | null;
}

function normaliseStatus(s: string | undefined): CaseView['status'] {
  if (s === 'claimed') return 'claimed';
  if (s === 'decided') return 'decided';
  return 'pending';
}

/** Project an inbox case into the render shape. */
export function toCaseView(c: InboxCase): CaseView {
  const meta = c.metadata ?? {};
  const str = (k: string) => {
    const v = (meta as Record<string, unknown>)[k];
    return typeof v === 'string' && v.length > 0 ? v : undefined;
  };
  const targetPath = str('target_path');
  return {
    id: c.id,
    title: targetPath ?? c.id,
    category: c.category ?? 'knowledge_enrichment',
    status: normaliseStatus(c.status),
    targetPath,
    content: str('content'),
    proposedBy: str('proposed_by') ?? str('agent_did'),
  };
}

/**
 * The open (not-yet-decided) case ids from an inbox snapshot — the count the
 * ambient ACSP indicator shows.
 */
export function openCaseIds(cases: InboxCase[]): Set<string> {
  const open = new Set<string>();
  for (const c of cases) {
    if (normaliseStatus(c.status) !== 'decided') open.add(c.id);
  }
  return open;
}

/**
 * Fold a broker event into the open-case set: a new case opens, a decided case
 * closes. Returns a NEW set (never mutates the input) so React state updates
 * stay referentially honest.
 */
export function applyBrokerEvent(openIds: ReadonlySet<string>, event: BrokerCaseEvent): Set<string> {
  const next = new Set(openIds);
  if (event.kind === 'new_case') next.add(event.caseId);
  else if (event.kind === 'case_decided') next.delete(event.caseId);
  return next;
}
