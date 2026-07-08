import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { PushToTalkService } from './PushToTalkService';

const DID = 'did:nostr:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1234';
const DID_B = 'did:nostr:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb5678';

/**
 * COM-15 / D6 / M5: the selection-scoped PTT state. PTT is no longer globally
 * scoped — it carries the selected agent's did:nostr, and the server-notify
 * callback carries that target on every PTT edge.
 */
describe('PushToTalkService selected-agent binding (COM-15 / D6)', () => {
  let ptt: PushToTalkService;

  beforeEach(() => {
    ptt = PushToTalkService.getInstance();
    ptt.setSelectedAgentDid(null); // reset the singleton binding
  });

  afterEach(() => {
    ptt.deactivate();
    ptt.setSelectedAgentDid(null);
  });

  it('binds a canonical did:nostr and reports it', () => {
    ptt.setSelectedAgentDid(DID);
    expect(ptt.getSelectedAgentDid()).toBe(DID);
    expect(ptt.isBoundToAgent()).toBe(true);
  });

  it('refuses a non-did target (verify before trust)', () => {
    ptt.setSelectedAgentDid('researcher-7');
    expect(ptt.getSelectedAgentDid()).toBeNull();
    expect(ptt.isBoundToAgent()).toBe(false);
  });

  it('clears the binding on deselect (null)', () => {
    ptt.setSelectedAgentDid(DID);
    ptt.setSelectedAgentDid(null);
    expect(ptt.getSelectedAgentDid()).toBeNull();
    expect(ptt.isBoundToAgent()).toBe(false);
  });

  it('carries the bound did:nostr on the server-notify at PTT-start', () => {
    const notifications: Array<{ active: boolean; did: string | null }> = [];
    ptt.onServerNotify((active, did) => notifications.push({ active, did }));

    ptt.setSelectedAgentDid(DID);
    ptt.activate('operator');

    // Simulate holding the PTT key (push mode): keydown → commanding.
    document.dispatchEvent(new KeyboardEvent('keydown', { key: ' ', bubbles: true }));

    const start = notifications.find((n) => n.active);
    expect(start, 'a PTT-start notification fired').toBeTruthy();
    expect(start?.did).toBe(DID);
  });

  it('re-notifies with the new target when the selection changes mid-command', () => {
    const notifications: Array<{ active: boolean; did: string | null }> = [];
    ptt.onServerNotify((active, did) => notifications.push({ active, did }));

    ptt.setSelectedAgentDid(DID);
    ptt.activate('operator');
    document.dispatchEvent(new KeyboardEvent('keydown', { key: ' ', bubbles: true }));

    // Re-select a different agent while commanding → the server is re-notified
    // with the new bound DID, so a mid-utterance change re-targets.
    ptt.setSelectedAgentDid(DID_B);

    const last = notifications[notifications.length - 1];
    expect(last.active).toBe(true);
    expect(last.did).toBe(DID_B);
  });
});
