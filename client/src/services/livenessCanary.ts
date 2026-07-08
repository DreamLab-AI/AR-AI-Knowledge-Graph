// Client-side liveness-canary observer (RES-a harness, ADR-130 Decision 3).
//
// A thin fire-and-forget wrapper over `POST /api/canary/observe/{id}` for
// client-observed live traffic (e.g. the D8 swarm dashboard mounting with live
// poll data). Fail-open: a harness or network error is swallowed so a canary
// fire never disrupts the UI. The harness records the fire from THIS observed
// event — it is not a synthetic probe (DDD invariant 5).

import { unifiedApiClient } from './api/UnifiedApiClient';
import { createLogger } from '../utils/loggerConfig';

const logger = createLogger('livenessCanary');

/** Fire a registered canary from observed live traffic. Never throws. */
export async function observeCanary(canaryId: string, evidence: string): Promise<void> {
  try {
    await unifiedApiClient.post(`/canary/observe/${encodeURIComponent(canaryId)}`, { evidence });
    logger.debug(`observed canary ${canaryId}`);
  } catch (error) {
    // Fail-open: an unregistered canary (404) or a down harness must not break
    // the surface that observed the traffic.
    logger.debug(`canary observe skipped (${canaryId}):`, error);
  }
}
