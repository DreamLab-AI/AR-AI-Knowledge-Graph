// REC-2 / D3 (PRD-023 WP-4): the control-centre broker case queue + the ambient
// ACSP indicator.
//
// An always-mounted glass pill shows the open-case count (the ambient ACSP
// indicator, driven by the broker:new_case / broker:case_decided WS events);
// clicking it expands the pending-judgment queue, each case decidable through
// the WS-9 operator decide route. A decided case round-trips
// `broker:new_case → broker:case_decided`, which fires `CANARY-VC-REC2-CASE`
// server-side.

import React, { useState } from 'react';
import { Scale } from 'lucide-react';
import { GlassPanel } from '../primitives/GlassPanel';
import { useBrokerCaseQueue } from './useBrokerCaseQueue';

export const AcspCaseQueue: React.FC = () => {
  const { cases, openCount, loading, decide } = useBrokerCaseQueue();
  const [expanded, setExpanded] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);

  const pending = cases.filter((c) => c.status !== 'decided');

  const onDecide = async (caseId: string, outcome: string) => {
    setBusyId(caseId);
    try {
      await decide(caseId, outcome, `operator ${outcome} via control centre`);
    } finally {
      setBusyId(null);
    }
  };

  return (
    <div className="fixed bottom-20 left-4 z-40 flex flex-col items-start gap-2" style={{ pointerEvents: 'auto' }}>
      {expanded && (
        <GlassPanel
          elevation="overlay"
          data-testid="acsp-case-queue"
          role="region"
          aria-label="Broker case queue"
          className="w-80 max-h-[60vh] overflow-y-auto p-3 text-foreground"
        >
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm font-semibold">Governance Queue</span>
            <button
              type="button"
              aria-label="Close case queue"
              onClick={() => setExpanded(false)}
              className="text-xs text-muted-foreground hover:text-foreground"
            >
              ✕
            </button>
          </div>

          {loading ? (
            <p className="text-xs text-muted-foreground">Loading cases…</p>
          ) : pending.length === 0 ? (
            <p className="text-xs text-muted-foreground">No pending judgments.</p>
          ) : (
            <ul className="space-y-2">
              {pending.map((c) => (
                <li key={c.id} className="rounded bg-white/5 p-2" data-testid="acsp-case">
                  <div className="text-[11px] font-medium truncate" title={c.title}>
                    {c.title}
                  </div>
                  <div className="text-[10px] text-muted-foreground mb-1">
                    {c.category} · {c.status}
                    {c.proposedBy ? ` · ${c.proposedBy.slice(0, 18)}` : ''}
                  </div>
                  {c.content && (
                    <div className="text-[10px] text-muted-foreground line-clamp-2 mb-2">{c.content}</div>
                  )}
                  <div className="flex gap-2">
                    <button
                      type="button"
                      disabled={busyId === c.id}
                      onClick={() => onDecide(c.id, 'approve')}
                      className="flex-1 text-[10px] px-2 py-1 rounded bg-emerald-500/15 text-emerald-400 hover:bg-emerald-500/25 disabled:opacity-50"
                    >
                      Approve
                    </button>
                    <button
                      type="button"
                      disabled={busyId === c.id}
                      onClick={() => onDecide(c.id, 'reject')}
                      className="flex-1 text-[10px] px-2 py-1 rounded bg-red-500/15 text-red-400 hover:bg-red-500/25 disabled:opacity-50"
                    >
                      Reject
                    </button>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </GlassPanel>
      )}

      <button
        type="button"
        data-testid="acsp-indicator"
        aria-label={`Governance queue — ${openCount} open case${openCount === 1 ? '' : 's'}`}
        aria-expanded={expanded}
        onClick={() => setExpanded((v) => !v)}
        className="cc-glass flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs text-foreground hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        <Scale size={13} aria-hidden="true" />
        ACSP
        <span
          className="inline-flex items-center justify-center min-w-[18px] h-[18px] px-1 rounded-full text-[10px]"
          style={{
            background: openCount > 0 ? 'rgba(245,158,11,0.2)' : 'rgba(107,114,128,0.2)',
            color: openCount > 0 ? '#f59e0b' : '#9ca3af',
          }}
        >
          {openCount}
        </span>
      </button>
    </div>
  );
};

AcspCaseQueue.displayName = 'AcspCaseQueue';

export default AcspCaseQueue;
