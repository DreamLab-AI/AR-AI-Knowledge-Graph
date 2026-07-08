// Agent-node identity helpers (COM-14 / WP-1).
//
// The DID (`did:nostr:<hex>`), minted by agentbox at spawn and carried by the
// server through `/api/bots/agents`, is the trust key for an agent node. The
// `task_id` (`id`) remains only as the fallback key until a DID arrives (DDD
// invariant 1: no surface keys an agent by task_id alone). These are pure
// functions, kept out of the R3F component so they can be unit-tested without a
// renderer.

/** The identity-bearing subset of an agent node. */
export interface AgentIdentityFields {
  id: string;
  /** Wire key is snake_case `did_nostr`, matching the Rust `Agent` serialisation. */
  did_nostr?: string;
}

/** True iff this node carries a non-empty DID rather than only a task_id. */
export const isDidKeyed = (agent: AgentIdentityFields): boolean =>
  typeof agent.did_nostr === 'string' && agent.did_nostr.length > 0;

/**
 * The identity key for an agent node: its `did:nostr` when the server has
 * carried a non-empty one, else the `task_id` (`id`) fallback. The DID is the
 * trust key; `task_id` is the fallback, never the trust key on its own. An empty
 * DID is treated as absent (the server emits a validated DID or omits the field).
 */
export const agentTrustKey = (agent: AgentIdentityFields): string =>
  isDidKeyed(agent) ? (agent.did_nostr as string) : agent.id;

/** Legible nameplate form of a did:nostr: `nostr:<first6>…<last4>`. */
export const shortDid = (did: string): string => {
  const hex = did.startsWith('did:nostr:') ? did.slice(10) : did;
  return hex.length > 12 ? `nostr:${hex.slice(0, 6)}…${hex.slice(-4)}` : `nostr:${hex}`;
};
