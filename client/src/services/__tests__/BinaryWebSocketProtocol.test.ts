/**
 * BinaryWebSocketProtocol Tests
 *
 * Tests V2 protocol (u32 IDs) - V1 backward compatibility removed.
 * See: src/utils/binary_protocol.rs for Rust implementation.
 */

import { describe, it, expect, beforeEach } from 'vitest';
import {
  BinaryWebSocketProtocol,
  MESSAGE_HEADER_SIZE,
  AGENT_POSITION_SIZE_V2,
  AGENT_STATE_SIZE_V2,
  AGENT_ACTION_HEADER_SIZE,
  AgentActionType,
  AGENT_ACTION_COLORS,
  type AgentPositionUpdate,
  type AgentStateData,
  type AgentActionEvent,
} from '../BinaryWebSocketProtocol';

/**
 * Helper to create a decode payload (raw node records, no version prefix).
 *
 * Post-ADR-090 the protocol version travels in the 6-byte message header, not in
 * the payload — production strips the header via extractPayload() before calling
 * decode*(). The decoders therefore receive a header-less payload whose bytes are
 * the raw node records, so dataOffset is 0.
 */
function createPayload(dataSize: number): { buffer: ArrayBuffer; dataView: DataView; dataOffset: number } {
  const buffer = new ArrayBuffer(dataSize);
  const view = new DataView(buffer);
  return { buffer, dataView: view, dataOffset: 0 };
}

describe('BinaryWebSocketProtocol - V2 Protocol', () => {
  let protocol: BinaryWebSocketProtocol;

  beforeEach(() => {
    protocol = BinaryWebSocketProtocol.getInstance();
  });

  describe('Position Updates (u32 IDs)', () => {
    it('should encode V2 format with large IDs', () => {
      protocol.setUserInteracting(true);

      const updates: AgentPositionUpdate[] = [
        {
          agentId: 20000,
          position: { x: 1.0, y: 2.0, z: 3.0 },
          timestamp: Date.now(),
          flags: 0,
        },
      ];

      const encoded = protocol.encodePositionUpdates(updates);

      expect(encoded).not.toBeNull();
      // Header + position data
      expect(encoded!.byteLength).toBe(MESSAGE_HEADER_SIZE + AGENT_POSITION_SIZE_V2);
    });

    it('should decode V2 format with large IDs', () => {
      const { buffer, dataView, dataOffset } = createPayload(AGENT_POSITION_SIZE_V2);

      dataView.setUint32(dataOffset + 0, 50000, true);
      dataView.setFloat32(dataOffset + 4, 1.0, true);
      dataView.setFloat32(dataOffset + 8, 2.0, true);
      dataView.setFloat32(dataOffset + 12, 3.0, true);
      dataView.setUint32(dataOffset + 16, Date.now(), true);
      dataView.setUint8(dataOffset + 20, 0);

      const updates = protocol.decodePositionUpdates(buffer);

      expect(updates).toHaveLength(1);
      expect(updates[0].agentId).toBe(50000);
      expect(updates[0].position.x).toBe(1.0);
    });

    it('should handle very large node IDs', () => {
      const largeIds = [16384, 20000, 50000, 100000, 1000000];

      for (const nodeId of largeIds) {
        const { buffer, dataView, dataOffset } = createPayload(AGENT_POSITION_SIZE_V2);

        dataView.setUint32(dataOffset + 0, nodeId, true);
        dataView.setFloat32(dataOffset + 4, 1.0, true);
        dataView.setFloat32(dataOffset + 8, 2.0, true);
        dataView.setFloat32(dataOffset + 12, 3.0, true);
        dataView.setUint32(dataOffset + 16, Date.now(), true);
        dataView.setUint8(dataOffset + 20, 0);

        const updates = protocol.decodePositionUpdates(buffer);

        expect(updates).toHaveLength(1);
        expect(updates[0].agentId).toBe(nodeId);
      }
    });

    it('should decode multiple V2 updates correctly', () => {
      const nodeIds = [100, 20000, 50000];
      const { buffer, dataView, dataOffset } = createPayload(AGENT_POSITION_SIZE_V2 * nodeIds.length);

      nodeIds.forEach((nodeId, i) => {
        const offset = dataOffset + i * AGENT_POSITION_SIZE_V2;
        dataView.setUint32(offset, nodeId, true);
        dataView.setFloat32(offset + 4, i + 1.0, true);
        dataView.setFloat32(offset + 8, i + 2.0, true);
        dataView.setFloat32(offset + 12, i + 3.0, true);
        dataView.setUint32(offset + 16, Date.now(), true);
        dataView.setUint8(offset + 20, 0);
      });

      const updates = protocol.decodePositionUpdates(buffer);

      expect(updates).toHaveLength(3);
      expect(updates[0].agentId).toBe(100);
      expect(updates[1].agentId).toBe(20000);
      expect(updates[2].agentId).toBe(50000);
    });

    it('should support maximum 30-bit node ID', () => {
      const maxId = 0x3FFFFFFF;
      const { buffer, dataView, dataOffset } = createPayload(AGENT_POSITION_SIZE_V2);

      dataView.setUint32(dataOffset + 0, maxId, true);
      dataView.setFloat32(dataOffset + 4, 1.0, true);
      dataView.setFloat32(dataOffset + 8, 2.0, true);
      dataView.setFloat32(dataOffset + 12, 3.0, true);
      dataView.setUint32(dataOffset + 16, Date.now(), true);
      dataView.setUint8(dataOffset + 20, 0);

      const updates = protocol.decodePositionUpdates(buffer);

      expect(updates).toHaveLength(1);
      expect(updates[0].agentId).toBe(maxId);
    });
  });

  describe('Agent State Data', () => {
    it('should encode V2 agent state with large IDs', () => {
      const agents: AgentStateData[] = [
        {
          agentId: 50000,
          position: { x: 1.0, y: 2.0, z: 3.0 },
          velocity: { x: 0.1, y: 0.2, z: 0.3 },
          health: 100.0,
          cpuUsage: 50.0,
          memoryUsage: 60.0,
          workload: 70.0,
          tokens: 1000,
          flags: 0,
        },
      ];

      const encoded = protocol.encodeAgentState(agents);

      // Payload includes header + raw data
      expect(encoded.byteLength).toBe(MESSAGE_HEADER_SIZE + AGENT_STATE_SIZE_V2);
    });

    it('should decode V2 agent state with large IDs', () => {
      const { buffer, dataView, dataOffset } = createPayload(AGENT_STATE_SIZE_V2);

      dataView.setUint32(dataOffset + 0, 100000, true);
      dataView.setFloat32(dataOffset + 4, 1.0, true);
      dataView.setFloat32(dataOffset + 8, 2.0, true);
      dataView.setFloat32(dataOffset + 12, 3.0, true);
      dataView.setFloat32(dataOffset + 16, 0.1, true);
      dataView.setFloat32(dataOffset + 20, 0.2, true);
      dataView.setFloat32(dataOffset + 24, 0.3, true);
      dataView.setFloat32(dataOffset + 28, 100.0, true);
      dataView.setFloat32(dataOffset + 32, 50.0, true);
      dataView.setFloat32(dataOffset + 36, 60.0, true);
      dataView.setFloat32(dataOffset + 40, 70.0, true);
      dataView.setUint32(dataOffset + 44, 1000, true);
      dataView.setUint8(dataOffset + 48, 0);

      const agents = protocol.decodeAgentState(buffer);

      expect(agents).toHaveLength(1);
      expect(agents[0].agentId).toBe(100000);
      expect(agents[0].position.x).toBe(1.0);
      expect(agents[0].health).toBe(100.0);
    });
  });

  describe('No ID Collision Tests', () => {
    it('should have no collisions for different large IDs', () => {
      const nodeIds = [16384, 20000, 50000, 100000, 500000];
      const { buffer, dataView, dataOffset } = createPayload(AGENT_POSITION_SIZE_V2 * nodeIds.length);

      nodeIds.forEach((nodeId, i) => {
        const offset = dataOffset + i * AGENT_POSITION_SIZE_V2;
        dataView.setUint32(offset, nodeId, true);
        dataView.setFloat32(offset + 4, i + 1.0, true);
        dataView.setFloat32(offset + 8, i + 2.0, true);
        dataView.setFloat32(offset + 12, i + 3.0, true);
        dataView.setUint32(offset + 16, Date.now(), true);
        dataView.setUint8(offset + 20, 0);
      });

      const updates = protocol.decodePositionUpdates(buffer);

      expect(updates).toHaveLength(nodeIds.length);

      const decodedIds = updates.map(u => u.agentId);
      const uniqueIds = new Set(decodedIds);
      expect(uniqueIds.size).toBe(nodeIds.length);

      nodeIds.forEach((nodeId, i) => {
        expect(updates[i].agentId).toBe(nodeId);
      });
    });

    it('should distinguish IDs that would collide in u16 space', () => {
      const id1 = 100;
      const id2 = 16384 + 100; // Would truncate to same value in u16

      const { buffer, dataView, dataOffset } = createPayload(AGENT_POSITION_SIZE_V2 * 2);

      dataView.setUint32(dataOffset + 0, id1, true);
      dataView.setFloat32(dataOffset + 4, 1.0, true);
      dataView.setFloat32(dataOffset + 8, 2.0, true);
      dataView.setFloat32(dataOffset + 12, 3.0, true);
      dataView.setUint32(dataOffset + 16, Date.now(), true);
      dataView.setUint8(dataOffset + 20, 0);

      const offset2 = dataOffset + AGENT_POSITION_SIZE_V2;
      dataView.setUint32(offset2 + 0, id2, true);
      dataView.setFloat32(offset2 + 4, 4.0, true);
      dataView.setFloat32(offset2 + 8, 5.0, true);
      dataView.setFloat32(offset2 + 12, 6.0, true);
      dataView.setUint32(offset2 + 16, Date.now(), true);
      dataView.setUint8(offset2 + 20, 0);

      const updates = protocol.decodePositionUpdates(buffer);

      expect(updates).toHaveLength(2);
      expect(updates[0].agentId).toBe(id1);
      expect(updates[1].agentId).toBe(id2);
      expect(updates[0].agentId).not.toBe(updates[1].agentId);
    });
  });

  describe('Invalid Payload Handling', () => {
    it('should handle invalid payload sizes gracefully', () => {
      const payload = new ArrayBuffer(17);

      const updates = protocol.decodePositionUpdates(payload);

      expect(updates).toHaveLength(0);
    });

    it('should handle empty payload', () => {
      const payload = new ArrayBuffer(0);

      const updates = protocol.decodePositionUpdates(payload);

      expect(updates).toHaveLength(0);
    });

    it('should reject unsupported protocol versions', () => {
      const buffer = new ArrayBuffer(1 + AGENT_POSITION_SIZE_V2);
      const view = new DataView(buffer);
      view.setUint8(0, 1); // V1 - unsupported

      const updates = protocol.decodePositionUpdates(buffer);

      expect(updates).toHaveLength(0);
    });
  });

  describe('Performance and Bandwidth', () => {
    it('should calculate correct V2 bandwidth', () => {
      const agentCount = 100;
      const updateRateHz = 60;

      const bandwidth = protocol.calculateBandwidth(agentCount, updateRateHz);

      const expectedFullState = agentCount * 49 * updateRateHz + MESSAGE_HEADER_SIZE * updateRateHz;

      expect(bandwidth.fullState).toBe(expectedFullState);
    });
  });

  describe('Agent Action Events (0x23)', () => {
    it('should decode a single agent action event', () => {
      // Create payload: 15 bytes header
      const payload = new ArrayBuffer(AGENT_ACTION_HEADER_SIZE);
      const view = new DataView(payload);

      // sourceAgentId: 1001
      view.setUint32(0, 1001, true);
      // targetNodeId: 5000
      view.setUint32(4, 5000, true);
      // actionType: Query (0)
      view.setUint8(8, AgentActionType.Query);
      // timestamp: 1234567890
      view.setUint32(9, 1234567890, true);
      // durationMs: 500
      view.setUint16(13, 500, true);

      const event = protocol.decodeAgentAction(payload);

      expect(event).not.toBeNull();
      expect(event!.sourceAgentId).toBe(1001);
      expect(event!.targetNodeId).toBe(5000);
      expect(event!.actionType).toBe(AgentActionType.Query);
      expect(event!.timestamp).toBe(1234567890);
      expect(event!.durationMs).toBe(500);
    });

    it('should decode all action types correctly', () => {
      const actionTypes = [
        AgentActionType.Query,
        AgentActionType.Update,
        AgentActionType.Create,
        AgentActionType.Delete,
        AgentActionType.Link,
        AgentActionType.Transform,
      ];

      for (const actionType of actionTypes) {
        const payload = new ArrayBuffer(AGENT_ACTION_HEADER_SIZE);
        const view = new DataView(payload);

        view.setUint32(0, 100, true);
        view.setUint32(4, 200, true);
        view.setUint8(8, actionType);
        view.setUint32(9, Date.now(), true);
        view.setUint16(13, 300, true);

        const event = protocol.decodeAgentAction(payload);

        expect(event).not.toBeNull();
        expect(event!.actionType).toBe(actionType);
      }
    });

    it('should encode and decode agent action roundtrip', () => {
      const original: AgentActionEvent = {
        sourceAgentId: 42,
        targetNodeId: 9999,
        actionType: AgentActionType.Create,
        timestamp: Date.now(),
        durationMs: 750,
      };

      const encoded = protocol.encodeAgentAction(original);

      // Skip message header to get payload
      const payload = encoded.slice(MESSAGE_HEADER_SIZE);
      const decoded = protocol.decodeAgentAction(payload);

      expect(decoded).not.toBeNull();
      expect(decoded!.sourceAgentId).toBe(original.sourceAgentId);
      expect(decoded!.targetNodeId).toBe(original.targetNodeId);
      expect(decoded!.actionType).toBe(original.actionType);
      expect(decoded!.durationMs).toBe(original.durationMs);
    });

    it('should handle agent action with payload', () => {
      const extraData = new Uint8Array([0xDE, 0xAD, 0xBE, 0xEF]);
      const payload = new ArrayBuffer(AGENT_ACTION_HEADER_SIZE + extraData.length);
      const view = new DataView(payload);

      view.setUint32(0, 100, true);
      view.setUint32(4, 200, true);
      view.setUint8(8, AgentActionType.Update);
      view.setUint32(9, 0, true);
      view.setUint16(13, 500, true);

      // Add extra payload
      new Uint8Array(payload, AGENT_ACTION_HEADER_SIZE).set(extraData);

      const event = protocol.decodeAgentAction(payload);

      expect(event).not.toBeNull();
      expect(event!.payload).toBeDefined();
      expect(event!.payload!.length).toBe(4);
      expect(event!.payload![0]).toBe(0xDE);
    });

    it('should reject payload that is too small', () => {
      const tooSmall = new ArrayBuffer(10); // Less than 15 bytes

      const event = protocol.decodeAgentAction(tooSmall);

      expect(event).toBeNull();
    });

    it('should have correct color mappings for all action types', () => {
      expect(AGENT_ACTION_COLORS[AgentActionType.Query]).toBe('#3b82f6');     // Blue
      expect(AGENT_ACTION_COLORS[AgentActionType.Update]).toBe('#eab308');    // Yellow
      expect(AGENT_ACTION_COLORS[AgentActionType.Create]).toBe('#22c55e');    // Green
      expect(AGENT_ACTION_COLORS[AgentActionType.Delete]).toBe('#ef4444');    // Red
      expect(AGENT_ACTION_COLORS[AgentActionType.Link]).toBe('#a855f7');      // Purple
      expect(AGENT_ACTION_COLORS[AgentActionType.Transform]).toBe('#06b6d4'); // Cyan
    });

    it('should have correct header size constant', () => {
      // 4 (sourceAgentId) + 4 (targetNodeId) + 1 (actionType) + 4 (timestamp) + 2 (durationMs) = 15
      expect(AGENT_ACTION_HEADER_SIZE).toBe(15);
    });
  });
});
