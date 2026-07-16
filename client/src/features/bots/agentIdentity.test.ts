import { describe, it, expect } from 'vitest';
import { agentTrustKey, isDidKeyed, shortDid } from './agentIdentity';

const DID = 'did:nostr:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1234';

describe('agent-node identity keying (COM-14 / WP-1)', () => {
  it('keys by did:nostr when the server carried one', () => {
    expect(agentTrustKey({ id: 'task-7', did_nostr: DID })).toBe(DID);
    expect(isDidKeyed({ id: 'task-7', did_nostr: DID })).toBe(true);
  });

  it('falls back to task_id when no DID is present', () => {
    expect(agentTrustKey({ id: 'task-7' })).toBe('task-7');
    expect(isDidKeyed({ id: 'task-7' })).toBe(false);
    // An empty string is not a DID — still the fallback.
    expect(agentTrustKey({ id: 'task-7', did_nostr: '' })).toBe('task-7');
    expect(isDidKeyed({ id: 'task-7', did_nostr: '' })).toBe(false);
  });

  it('renders a legible short DID nameplate', () => {
    expect(shortDid(DID)).toBe('nostr:aaaaaa…1234');
    // Tolerates a bare hex (no prefix).
    expect(shortDid('aaaaaa00000000000000bbbb')).toBe('nostr:aaaaaa…bbbb');
    // Short inputs are returned whole.
    expect(shortDid('did:nostr:abcd')).toBe('nostr:abcd');
  });
});
