import { describe, it, expect } from 'vitest';
import {
  encodePositionPayload,
  decodePositionUpdates,
  encodeAgentStatePayload,
  decodeAgentState,
  encodeAgentActionPayload,
  decodeAgentAction,
  decodeAgentActions,
} from '../agentMessages';
import { AGENT_POSITION_SIZE_V2, AGENT_STATE_SIZE_V2, AGENT_ACTION_HEADER_SIZE } from '../frameTypes';
import type { AgentPositionUpdate, AgentStateData, AgentActionEvent } from '../frameTypes';

// ---- Position round-trip ----

const samplePosition: AgentPositionUpdate = {
  agentId: 42,
  position: { x: 1.5, y: -2.25, z: 0.0 },
  timestamp: 123456,
  flags: 0b00000011,
};

describe('position encode/decode round-trip', () => {
  it('single update', () => {
    const buf = new ArrayBuffer(AGENT_POSITION_SIZE_V2);
    encodePositionPayload([samplePosition], buf);
    const [decoded] = decodePositionUpdates(buf);
    expect(decoded.agentId).toBe(samplePosition.agentId);
    expect(decoded.position.x).toBeCloseTo(samplePosition.position.x, 4);
    expect(decoded.position.y).toBeCloseTo(samplePosition.position.y, 4);
    expect(decoded.position.z).toBeCloseTo(samplePosition.position.z, 4);
    expect(decoded.timestamp).toBe(samplePosition.timestamp);
    expect(decoded.flags).toBe(samplePosition.flags);
  });

  it('multiple updates', () => {
    const updates: AgentPositionUpdate[] = Array.from({ length: 5 }, (_, i) => ({
      agentId: i + 1,
      position: { x: i * 0.5, y: i * 1.0, z: i * -0.5 },
      timestamp: 1000 + i,
      flags: i,
    }));
    const buf = new ArrayBuffer(5 * AGENT_POSITION_SIZE_V2);
    encodePositionPayload(updates, buf);
    const decoded = decodePositionUpdates(buf);
    expect(decoded.length).toBe(5);
    updates.forEach((u, i) => {
      expect(decoded[i].agentId).toBe(u.agentId);
      expect(decoded[i].position.x).toBeCloseTo(u.position.x, 4);
    });
  });

  it('returns empty array for empty payload', () => {
    expect(decodePositionUpdates(new ArrayBuffer(0))).toEqual([]);
  });

  it('returns empty array for undersized payload', () => {
    expect(decodePositionUpdates(new ArrayBuffer(4))).toEqual([]);
  });
});

// ---- AgentState round-trip ----

const sampleState: AgentStateData = {
  agentId: 7,
  position: { x: 10, y: 20, z: 30 },
  velocity: { x: 0.1, y: -0.2, z: 0.3 },
  health: 0.95,
  cpuUsage: 0.4,
  memoryUsage: 0.6,
  workload: 0.75,
  tokens: 999,
  flags: 0xff,
};

describe('agentState encode/decode round-trip', () => {
  it('single agent state', () => {
    const buf = encodeAgentStatePayload([sampleState]);
    expect(buf.byteLength).toBe(AGENT_STATE_SIZE_V2);
    const [decoded] = decodeAgentState(buf);
    expect(decoded.agentId).toBe(sampleState.agentId);
    expect(decoded.position.x).toBeCloseTo(sampleState.position.x, 4);
    expect(decoded.velocity.y).toBeCloseTo(sampleState.velocity.y, 4);
    expect(decoded.health).toBeCloseTo(sampleState.health, 4);
    expect(decoded.tokens).toBe(sampleState.tokens);
    expect(decoded.flags).toBe(sampleState.flags);
  });

  it('multiple agents', () => {
    const states: AgentStateData[] = [sampleState, { ...sampleState, agentId: 8, tokens: 0 }];
    const buf = encodeAgentStatePayload(states);
    expect(buf.byteLength).toBe(2 * AGENT_STATE_SIZE_V2);
    const decoded = decodeAgentState(buf);
    expect(decoded.length).toBe(2);
    expect(decoded[1].agentId).toBe(8);
    expect(decoded[1].tokens).toBe(0);
  });

  it('returns empty for empty payload', () => {
    expect(decodeAgentState(new ArrayBuffer(0))).toEqual([]);
  });
});

// ---- AgentAction round-trip ----

const sampleAction: AgentActionEvent = {
  sourceAgentId: 1,
  targetNodeId: 255,
  actionType: 3,
  timestamp: 9999,
  durationMs: 500,
};

describe('agentAction encode/decode round-trip', () => {
  it('action without payload', () => {
    const buf = encodeAgentActionPayload(sampleAction);
    expect(buf.byteLength).toBe(AGENT_ACTION_HEADER_SIZE);
    const decoded = decodeAgentAction(buf);
    expect(decoded).not.toBeNull();
    expect(decoded!.sourceAgentId).toBe(sampleAction.sourceAgentId);
    expect(decoded!.targetNodeId).toBe(sampleAction.targetNodeId);
    expect(decoded!.actionType).toBe(sampleAction.actionType);
    expect(decoded!.timestamp).toBe(sampleAction.timestamp);
    expect(decoded!.durationMs).toBe(sampleAction.durationMs);
    expect(decoded!.payload).toBeUndefined();
  });

  it('action with extra payload bytes', () => {
    const extra = new Uint8Array([0xde, 0xad, 0xbe, 0xef]);
    const action: AgentActionEvent = { ...sampleAction, payload: extra };
    const buf = encodeAgentActionPayload(action);
    expect(buf.byteLength).toBe(AGENT_ACTION_HEADER_SIZE + 4);
    const decoded = decodeAgentAction(buf);
    expect(decoded!.payload).toEqual(extra);
  });

  it('returns null for undersized payload', () => {
    expect(decodeAgentAction(new ArrayBuffer(AGENT_ACTION_HEADER_SIZE - 1))).toBeNull();
  });
});


// ---- Shipping 0x23 wire frame (bare type tag + batch) ----
//
// Mirrors the server's encode_agent_actions (src/utils/binary_protocol.rs:1233):
// `[0x23][count:u16 le][(len:u16 le)(event 15B+payload)]…` — no 6-byte V4
// header. The store's tagged-frame branch decodes with data.slice(1); this
// round-trip pins that contract so the gate/offset can never silently drift
// again (beams went dark when a 6-byte strip misaligned the batch count).

function buildServerBatchFrame(actions: AgentActionEvent[]): ArrayBuffer {
  const encoded = actions.map(a => new Uint8Array(encodeAgentActionPayload(a)));
  const total = 3 + encoded.reduce((n, e) => n + 2 + e.byteLength, 0);
  const out = new Uint8Array(total);
  const view = new DataView(out.buffer);
  out[0] = 0x23;
  view.setUint16(1, actions.length, true);
  let offset = 3;
  for (const e of encoded) {
    view.setUint16(offset, e.byteLength, true);
    offset += 2;
    out.set(e, offset);
    offset += e.byteLength;
  }
  return out.buffer;
}

describe('shipping 0x23 batch frame decodes after a 1-byte tag strip', () => {
  it('round-trips a coalesced batch of three actions', () => {
    const actions: AgentActionEvent[] = [0, 1, 2].map(i => ({
      ...sampleAction,
      sourceAgentId: 10000 + i,
      targetNodeId: 400 + i,
    }));
    const frame = buildServerBatchFrame(actions);
    const decoded = decodeAgentActions(frame.slice(1));
    expect(decoded).toHaveLength(3);
    decoded.forEach((d, i) => {
      expect(d.sourceAgentId).toBe(10000 + i);
      expect(d.targetNodeId).toBe(400 + i);
      expect(d.durationMs).toBe(sampleAction.durationMs);
    });
  });

  it('a 6-byte strip misaligns the batch (the regression this guards)', () => {
    const frame = buildServerBatchFrame([sampleAction]);
    expect(decodeAgentActions(frame.slice(6))).not.toHaveLength(1);
  });
});
