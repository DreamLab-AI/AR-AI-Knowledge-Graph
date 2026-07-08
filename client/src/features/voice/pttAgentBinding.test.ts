import { describe, it, expect } from 'vitest';
import {
  isCanonicalDid,
  resolveSelectedAgentDid,
  shouldDispatchGoverned,
} from './pttAgentBinding';

const DID = 'did:nostr:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1234';

describe('PTT agent binding resolution (COM-15 / D6)', () => {
  it('accepts only a canonical did:nostr', () => {
    expect(isCanonicalDid(DID)).toBe(true);
    expect(isCanonicalDid('researcher-7')).toBe(false);
    expect(isCanonicalDid('did:nostr:xyz')).toBe(false);
    expect(isCanonicalDid('')).toBe(false);
    expect(isCanonicalDid(null)).toBe(false);
    expect(isCanonicalDid(undefined)).toBe(false);
  });

  it('resolves the DID when an agent node is keyed by it (COM-14 agentTrustKey)', () => {
    expect(resolveSelectedAgentDid({ nodeId: DID })).toBe(DID);
  });

  it('resolves the DID from node metadata (snake_case or camelCase)', () => {
    expect(resolveSelectedAgentDid({ nodeId: 'node-42', metadata: { did_nostr: DID } })).toBe(DID);
    expect(resolveSelectedAgentDid({ nodeId: 'node-42', metadata: { didNostr: DID } })).toBe(DID);
    expect(resolveSelectedAgentDid({ nodeId: 'node-42', did_nostr: DID })).toBe(DID);
  });

  it('returns null for a non-agent node so PTT unbinds (never targets a non-agent)', () => {
    expect(resolveSelectedAgentDid({ nodeId: 'concept-101', metadata: {} })).toBeNull();
    expect(resolveSelectedAgentDid({ nodeId: 'concept-101' })).toBeNull();
    expect(resolveSelectedAgentDid(null)).toBeNull();
    expect(resolveSelectedAgentDid(undefined)).toBeNull();
    // A malformed DID in metadata does not bind.
    expect(resolveSelectedAgentDid({ metadata: { did_nostr: 'did:nostr:short' } })).toBeNull();
  });

  it('dispatches governed only when commanding AND bound', () => {
    expect(shouldDispatchGoverned('commanding', DID)).toBe(true);
    expect(shouldDispatchGoverned('commanding', null)).toBe(false);
    expect(shouldDispatchGoverned('chatting', DID)).toBe(false);
    expect(shouldDispatchGoverned('idle', DID)).toBe(false);
  });
});
