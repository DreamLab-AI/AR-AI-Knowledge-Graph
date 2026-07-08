// COM-15 / D6 / M5 (PRD-023 WP-5): pure helpers for binding push-to-talk to the
// selected agent.
//
// These are renderer-free and DOM-free so the selection→did resolution and the
// governed-dispatch predicate can be unit-tested without React or a live socket.
// The stateful consumer that wires them into the live voice path is
// `usePushToTalkAgentBinding`.

import type { PTTState } from '../../services/PushToTalkService';

/** Canonical `did:nostr:<64-hex>` per ADR-125 I1 (BIP-340 x-only hex). */
const DID_NOSTR_RE = /^did:nostr:[0-9a-f]{64}$/;

/** True iff `s` is a canonical `did:nostr` — the only shape that may bind. */
export const isCanonicalDid = (s: string | null | undefined): s is string =>
  typeof s === 'string' && DID_NOSTR_RE.test(s);

/**
 * The `visionclaw:node-selected` event detail shape (subset). The selected
 * node's `did:nostr` may arrive two ways: an agent node is *keyed* by its DID
 * (COM-14 `agentTrustKey`), so `nodeId` may itself be the DID; otherwise the DID
 * rides the node metadata.
 */
export interface NodeSelectedDetail {
  nodeId?: string;
  did_nostr?: string;
  metadata?: Record<string, unknown> | null;
}

/**
 * Resolve the selected agent's `did:nostr` from a node-selected event detail, or
 * `null` when the selection is not a DID-keyed agent node. A non-agent node (its
 * `nodeId` is a plain graph id and its metadata carries no DID) resolves to
 * `null`, so PTT unbinds — a spoken command then never targets a non-agent.
 */
export const resolveSelectedAgentDid = (
  detail: NodeSelectedDetail | null | undefined,
): string | null => {
  if (!detail) return null;

  // 1) Agent nodes are keyed by their DID; the selected id may be the DID.
  if (isCanonicalDid(detail.nodeId)) return detail.nodeId;

  // 2) Else the DID may ride the node metadata (snake_case wire, or camelCase).
  const meta = detail.metadata ?? {};
  const metaDid =
    detail.did_nostr ??
    (typeof meta.did_nostr === 'string' ? meta.did_nostr : undefined) ??
    (typeof (meta as { didNostr?: unknown }).didNostr === 'string'
      ? ((meta as { didNostr?: string }).didNostr as string)
      : undefined);

  return isCanonicalDid(metaDid) ? metaDid : null;
};

/**
 * Whether a final transcript should be dispatched down the GOVERNED voice path
 * (signed 31402 → `/v1/voice-intent`) rather than the settings assistant: only
 * when PTT is actively commanding AND bound to a canonical agent DID.
 */
export const shouldDispatchGoverned = (
  state: PTTState,
  did: string | null,
): boolean => state === 'commanding' && isCanonicalDid(did);
