// REC-2 / D3 (PRD-023 WP-4): the stateful broker case-queue hook.
//
// Fetches `GET /api/broker/inbox` (WS-12), subscribes to the multiplexed graph
// socket for `broker:new_case` / `broker:case_decided` (P0 `broker_events.rs`),
// and decides a case through the WS-9 operator route
// `POST /api/broker/cases/{id}/decide`. The open-case count it exposes drives
// the ambient ACSP indicator.

import { useCallback, useEffect, useRef, useState } from 'react';
import { unifiedApiClient } from '../../../services/api/UnifiedApiClient';
import { webSocketService } from '../../../store/websocketStore';
import { createLogger } from '../../../utils/loggerConfig';
import {
  applyBrokerEvent,
  openCaseIds,
  parseBrokerEvent,
  toCaseView,
  type CaseView,
  type InboxCase,
} from './brokerCaseQueue';

const logger = createLogger('brokerCaseQueue');

export interface BrokerCaseQueue {
  cases: CaseView[];
  openCount: number;
  loading: boolean;
  refresh: () => Promise<void>;
  decide: (caseId: string, outcome: string, reasoning?: string) => Promise<boolean>;
}

export function useBrokerCaseQueue(): BrokerCaseQueue {
  const [cases, setCases] = useState<CaseView[]>([]);
  const [openIds, setOpenIds] = useState<ReadonlySet<string>>(new Set());
  const [loading, setLoading] = useState(true);
  const openIdsRef = useRef<ReadonlySet<string>>(openIds);
  openIdsRef.current = openIds;

  const refresh = useCallback(async () => {
    try {
      const resp = await unifiedApiClient.getData<{ cases?: InboxCase[] }>('/broker/inbox');
      const inbox = (resp?.cases ?? []) as InboxCase[];
      setCases(inbox.map(toCaseView));
      setOpenIds(openCaseIds(inbox));
    } catch (error) {
      // Fail-soft: an unauthenticated/unavailable inbox leaves an empty queue
      // rather than crashing the control centre.
      logger.debug('broker inbox fetch skipped:', error);
    } finally {
      setLoading(false);
    }
  }, []);

  // Initial load.
  useEffect(() => {
    refresh();
  }, [refresh]);

  // Live broker events over the multiplexed graph socket.
  useEffect(() => {
    const unsubscribe = webSocketService.onMessage((message: unknown) => {
      const event = parseBrokerEvent(message);
      if (!event) return;
      // Ambient count reacts instantly…
      setOpenIds((prev) => applyBrokerEvent(prev, event));
      // …and the rendered list refreshes to carry the full case metadata.
      refresh();
    });
    return unsubscribe;
  }, [refresh]);

  const decide = useCallback(
    async (caseId: string, outcome: string, reasoning?: string): Promise<boolean> => {
      try {
        const apiResponse = await unifiedApiClient.post(`/broker/cases/${encodeURIComponent(caseId)}/decide`, {
          outcome,
          reasoning,
        });
        const ok = !!apiResponse.data?.success;
        if (ok) {
          // Optimistically close; the broker:case_decided event will confirm.
          setOpenIds((prev) => {
            const next = new Set(prev);
            next.delete(caseId);
            return next;
          });
          refresh();
        }
        return ok;
      } catch (error) {
        logger.error('decide failed:', error);
        return false;
      }
    },
    [refresh],
  );

  return { cases, openCount: openIds.size, loading, refresh, decide };
}
