import { createLogger } from '../utils/loggerConfig';

const logger = createLogger('binaryProtocol');


/**
 * Binary Protocol for WebSocket node data communication
 * Aligns with server-side src/utils/binary_protocol.rs
 *
 * Protocol Versions:
 * - V2: 36 bytes per node (basic pathfinding)
 * - V3: 52 bytes per node (adds cluster_id, anomaly_score, community_id, centrality)
 * - V4: Delta encoding (20 bytes per changed node, only changed nodes sent)
 *
 * V3 wire layout (ADR-031 D2): id@0, position@4 (12B), velocity@16 (12B),
 * sssp_distance@28 (f32), sssp_parent@32 (i32), cluster_id@36 (u32),
 * anomaly@40 (f32), community@44 (u32), centrality@48 (f32). Stride 52 B.
 */

export interface Vec3 {
  x: number;
  y: number;
  z: number;
}

export interface BinaryNodeData {
  nodeId: number;
  position: Vec3;
  velocity: Vec3;
  ssspDistance: number;
  ssspParent: number;
  // V3 analytics fields (optional for backwards compatibility)
  clusterId?: number;
  anomalyScore?: number;
  communityId?: number;
  /** PageRank centrality, normalised (ADR-031 D2, wire offset 48). */
  centrality?: number;
}

/**
 * Result of parsing binary node data, distinguishing full vs delta updates.
 * - Full updates: nodes contain absolute positions (replace targetPositions)
 * - Delta updates: nodes contain position/velocity DELTAS (add to targetPositions)
 */
export interface ParsedBinaryFrame {
  /** 'full' for V2/V3/V5 full state, 'delta' for V4 delta encoding */
  type: 'full' | 'delta';
  /** Parsed node data. For delta frames, position/velocity are DELTAS, not absolute. */
  nodes: BinaryNodeData[];
  /** V4 only: frame number within the delta cycle (0-59) */
  frameNumber?: number;
  /** V5 only: authoritative server broadcast sequence for backpressure ack correlation */
  broadcastSequence?: number;
}

// V4 delta encoding constants (must match server delta_encoding.rs)
const DELTA_SCALE_FACTOR = 100.0;
const DELTA_ITEM_SIZE = 20;
const DELTA_POSITION_CHANGED = 0x01;
const DELTA_VELOCITY_CHANGED = 0x02;

// Protocol version constants (must match server)
export const PROTOCOL_V2 = 2;
export const PROTOCOL_V3 = 3;
export const PROTOCOL_V4 = 4;
export const PROTOCOL_V5 = 5;

/** Last broadcast sequence received from server (V5 frames). Undefined until first V5 frame. */
export let lastBroadcastSequence: number | undefined;

/**
 * Opcodes that ride the same binary socket as position frames but belong to a
 * DIFFERENT codec with its own framing rules (ADR-2019 tag-byte registry):
 *
 * - `0x23` agent-action batch: `[0x23][u16 count]([u16 len][event])*`
 * - `0x43` avatar pose (XR presence sibling frame)
 * - `0x44` agent co-presence (ADR-2020)
 *
 * None of these is a position-frame version. The position decoder must decline
 * them rather than attempt size-based auto-detection, which would happily
 * reinterpret their payload as node records (ADR-2057 removed that fallback
 * entirely — an unrecognised version is now declined outright).
 */
export const SIBLING_OPCODES: ReadonlySet<number> = new Set([0x23, 0x43, 0x44]);

// V3 wire format: 52 bytes per node. ADR-2057 removed the 36-byte V2 stride and
// its size-autodetect heuristic: the server rejects V2 outright
// (`src/utils/binary_protocol.rs:565`), so nothing on the live wire is 36 bytes.
export const BINARY_NODE_SIZE_V3 = 52;
// Default to V3 (current server default)
export const BINARY_NODE_SIZE = BINARY_NODE_SIZE_V3;

// Field offsets (same for V2 and V3)
export const BINARY_NODE_ID_OFFSET = 0;
export const BINARY_POSITION_OFFSET = 4;
export const BINARY_VELOCITY_OFFSET = 16;
export const BINARY_SSSP_DISTANCE_OFFSET = 28;
export const BINARY_SSSP_PARENT_OFFSET = 32;
// V3 analytics offsets
export const BINARY_CLUSTER_ID_OFFSET = 36;
export const BINARY_ANOMALY_SCORE_OFFSET = 40;
export const BINARY_COMMUNITY_ID_OFFSET = 44;
// ADR-031 D2: PageRank centrality (f32) appended at the V3 tail.
export const BINARY_CENTRALITY_OFFSET = 48;

// Node type flag constants (Protocol V2/V3 - must match server)
export const AGENT_NODE_FLAG = 0x80000000;
export const KNOWLEDGE_NODE_FLAG = 0x40000000;
// NODE_ID_MASK: bits 0-25 only (excludes ALL flag bits 26-31: agent, knowledge, ontology type)
// Must match server's NODE_ID_MASK (0x03FFFFFF) to correctly strip ontology type flags
export const NODE_ID_MASK = 0x03FFFFFF;

// Ontology node type flags (bits 26-28)
export const ONTOLOGY_TYPE_MASK = 0x1C000000;
export const ONTOLOGY_CLASS_FLAG = 0x04000000;
export const ONTOLOGY_INDIVIDUAL_FLAG = 0x08000000;
export const ONTOLOGY_PROPERTY_FLAG = 0x10000000;

export enum NodeType {
  Knowledge = 'knowledge',
  Agent = 'agent',
  OntologyClass = 'ontology_class',
  OntologyIndividual = 'ontology_individual',
  OntologyProperty = 'ontology_property',
  Unknown = 'unknown'
}

export function getNodeType(nodeId: number): NodeType {
  if ((nodeId & AGENT_NODE_FLAG) !== 0) {
    return NodeType.Agent;
  } else if ((nodeId & KNOWLEDGE_NODE_FLAG) !== 0) {
    return NodeType.Knowledge;
  } else if ((nodeId & ONTOLOGY_TYPE_MASK) === ONTOLOGY_CLASS_FLAG) {
    return NodeType.OntologyClass;
  } else if ((nodeId & ONTOLOGY_TYPE_MASK) === ONTOLOGY_INDIVIDUAL_FLAG) {
    return NodeType.OntologyIndividual;
  } else if ((nodeId & ONTOLOGY_TYPE_MASK) === ONTOLOGY_PROPERTY_FLAG) {
    return NodeType.OntologyProperty;
  }
  return NodeType.Unknown;
}

export function getActualNodeId(nodeId: number): number {
  return nodeId & NODE_ID_MASK;
}

export function isAgentNode(nodeId: number): boolean {
  return (nodeId & AGENT_NODE_FLAG) !== 0;
}

export function isKnowledgeNode(nodeId: number): boolean {
  return (nodeId & KNOWLEDGE_NODE_FLAG) !== 0;
}

export function isOntologyNode(nodeId: number): boolean {
  return (nodeId & ONTOLOGY_TYPE_MASK) !== 0;
}

/**
 * Parse binary node data from server
 * Supports Protocol V2 (36 bytes) and V3 (52 bytes)
 */
export function parseBinaryNodeData(buffer: ArrayBuffer): BinaryNodeData[] {
  if (!buffer || buffer.byteLength === 0) {
    return [];
  }

  // Create a copy to avoid issues with detached buffers
  const safeBuffer = buffer.slice(0);
  const view = new DataView(safeBuffer);
  const nodes: BinaryNodeData[] = [];

  try {
    if (safeBuffer.byteLength < 1) {
      return [];
    }

    // Read protocol version byte
    const protocolVersion = view.getUint8(0);
    let offset = 1; // Skip version byte
    let nodeSize: number;
    let hasAnalytics: boolean;

    switch (protocolVersion) {
      case PROTOCOL_V2:
        // ADR-2057: the server rejects V2 outright
        // (`src/utils/binary_protocol.rs:565` — "V2 protocol no longer supported.
        // Please upgrade client to V3+."), so a V2 frame can only be a stale
        // sender or a mis-routed payload. Decline rather than decode 36-byte
        // records the live wire never carries.
        logger.warn(
          'Declining V2 position frame — the server no longer emits or accepts V2. ' +
          'Expected V3 (0x03) or the V5 envelope (0x05).'
        );
        return [];
      case PROTOCOL_V3:
        nodeSize = BINARY_NODE_SIZE_V3;
        hasAnalytics = true;
        break;
      case PROTOCOL_V4:
        // Delta encoding - decode via parseBinaryFrameData() instead
        // Return empty from this legacy function; callers should use parseBinaryFrameData()
        return parseDeltaNodes(safeBuffer);
      case PROTOCOL_V5:
        // V5 = V3 node data with 8-byte broadcast sequence prefix
        // Extract sequence, then parse remainder as V3
        return parseV5Nodes(safeBuffer);
      default:
        // ADR-2019 unknown-tag handling at the demultiplexer.
        //
        // A frame whose first byte is a KNOWN SIBLING OPCODE is not an unknown
        // position version — it is another codec's frame that reached this
        // decoder by mistake. Falling through to size auto-detection would
        // reinterpret its bytes as V2 node records whenever its length happened
        // to be a multiple of 36, fabricating nodes at arbitrary positions from
        // an agent-action or presence payload. Decline it instead.
        if (SIBLING_OPCODES.has(protocolVersion)) {
          logger.warn(
            `Declining frame for sibling opcode 0x${protocolVersion.toString(16)} — ` +
            `not a position frame; route it to its own decoder.`
          );
          return [];
        }
        // ADR-2057: fail closed. The previous behaviour re-read the buffer from
        // offset 0 as 36-byte V2 records whenever the length happened to divide
        // by 36 — fabricating nodes at arbitrary positions from a frame whose
        // version we do not recognise. The live wire is V3 and V5 only, so an
        // unknown version has no plausible auto-detection.
        logger.warn(
          `Declining frame with unknown protocol version ${protocolVersion} — ` +
          `expected V3 (0x03) or the V5 envelope (0x05).`
        );
        return [];
    }

    const dataLength = safeBuffer.byteLength - offset;

    // Validate data length. ADR-2057: the V2 size-swap heuristic is gone with V2
    // itself — every frame reaching here is V3 or a V5 body, both 52-byte stride.
    if (dataLength % nodeSize !== 0) {
      {
        logger.warn(
          `Binary data length (${dataLength} bytes) is not a multiple of node size (${nodeSize}). ` +
          `Expected ${Math.floor(dataLength / nodeSize)} complete nodes.`
        );
      }
    }

    const completeNodes = Math.floor(dataLength / nodeSize);

    if (completeNodes === 0) {
      return [];
    }

    // Parse each node
    for (let i = 0; i < completeNodes; i++) {
      const nodeOffset = offset + (i * nodeSize);

      if (nodeOffset + nodeSize > safeBuffer.byteLength) {
        break;
      }

      // Node ID (4 bytes) - includes type flags in high bits
      const nodeId = view.getUint32(nodeOffset + BINARY_NODE_ID_OFFSET, true);

      // Position (12 bytes)
      const position: Vec3 = {
        x: view.getFloat32(nodeOffset + BINARY_POSITION_OFFSET, true),
        y: view.getFloat32(nodeOffset + BINARY_POSITION_OFFSET + 4, true),
        z: view.getFloat32(nodeOffset + BINARY_POSITION_OFFSET + 8, true)
      };

      // Velocity (12 bytes)
      const velocity: Vec3 = {
        x: view.getFloat32(nodeOffset + BINARY_VELOCITY_OFFSET, true),
        y: view.getFloat32(nodeOffset + BINARY_VELOCITY_OFFSET + 4, true),
        z: view.getFloat32(nodeOffset + BINARY_VELOCITY_OFFSET + 8, true)
      };

      // SSSP data (8 bytes)
      const ssspDistance = view.getFloat32(nodeOffset + BINARY_SSSP_DISTANCE_OFFSET, true);
      const ssspParent = view.getInt32(nodeOffset + BINARY_SSSP_PARENT_OFFSET, true);

      // Validate position and velocity (reject NaN/Inf)
      const isValid =
        !isNaN(position.x) && isFinite(position.x) &&
        !isNaN(position.y) && isFinite(position.y) &&
        !isNaN(position.z) && isFinite(position.z) &&
        !isNaN(velocity.x) && isFinite(velocity.x) &&
        !isNaN(velocity.y) && isFinite(velocity.y) &&
        !isNaN(velocity.z) && isFinite(velocity.z);

      if (isValid) {
        const node: BinaryNodeData = {
          nodeId,
          position,
          velocity,
          ssspDistance,
          ssspParent
        };

        // Parse V3 analytics fields if present
        if (hasAnalytics) {
          node.clusterId = view.getUint32(nodeOffset + BINARY_CLUSTER_ID_OFFSET, true);
          node.anomalyScore = view.getFloat32(nodeOffset + BINARY_ANOMALY_SCORE_OFFSET, true);
          node.communityId = view.getUint32(nodeOffset + BINARY_COMMUNITY_ID_OFFSET, true);
          node.centrality = view.getFloat32(nodeOffset + BINARY_CENTRALITY_OFFSET, true);
        }

        nodes.push(node);
      } else {
        // Only log first few corrupted nodes to avoid spam
        if (i < 3) {
          logger.warn(
            `Skipping corrupted node at index ${i}: id=${nodeId}, ` +
            `pos=[${position.x}, ${position.y}, ${position.z}]`
          );
        }
      }
    }
  } catch (error) {
    logger.error('Error parsing binary data:', error);
  }

  return nodes;
}

/**
 * Parse V4 delta-encoded frame into BinaryNodeData entries.
 * The returned nodes contain DELTA values (not absolute positions).
 * Position and velocity fields represent the CHANGE from the previous frame.
 *
 * V4 wire format:
 *   [1 byte: version=4][1 byte: frame_number][2 bytes: num_changed (u16 LE)]
 *   For each changed node (20 bytes):
 *     [4 bytes: node_id (u32 LE)][1 byte: change_flags][3 bytes: padding]
 *     [2 bytes: dx (i16 LE)][2 bytes: dy][2 bytes: dz]
 *     [2 bytes: dvx (i16 LE)][2 bytes: dvy][2 bytes: dvz]
 */
function parseDeltaNodes(buffer: ArrayBuffer): BinaryNodeData[] {
  const view = new DataView(buffer);
  const nodes: BinaryNodeData[] = [];

  // Minimum size: 1 (version) + 1 (frame) + 2 (count) = 4 bytes
  if (buffer.byteLength < 4) {
    return [];
  }

  // Skip version byte (already validated as V4)
  const _frameNumber = view.getUint8(1);
  const numChanged = view.getUint16(2, true);

  const expectedSize = 4 + numChanged * DELTA_ITEM_SIZE;
  if (buffer.byteLength < expectedSize) {
    logger.warn(`V4 delta frame truncated: expected ${expectedSize} bytes, got ${buffer.byteLength}`);
    return [];
  }

  let offset = 4;
  for (let i = 0; i < numChanged; i++) {
    const nodeId = view.getUint32(offset, true);
    const changeFlags = view.getUint8(offset + 4);
    // offset + 5..7 = padding

    const dxScaled = view.getInt16(offset + 8, true);
    const dyScaled = view.getInt16(offset + 10, true);
    const dzScaled = view.getInt16(offset + 12, true);
    const dvxScaled = view.getInt16(offset + 14, true);
    const dvyScaled = view.getInt16(offset + 16, true);
    const dvzScaled = view.getInt16(offset + 18, true);

    const dx = (changeFlags & DELTA_POSITION_CHANGED) ? dxScaled / DELTA_SCALE_FACTOR : 0;
    const dy = (changeFlags & DELTA_POSITION_CHANGED) ? dyScaled / DELTA_SCALE_FACTOR : 0;
    const dz = (changeFlags & DELTA_POSITION_CHANGED) ? dzScaled / DELTA_SCALE_FACTOR : 0;
    const dvx = (changeFlags & DELTA_VELOCITY_CHANGED) ? dvxScaled / DELTA_SCALE_FACTOR : 0;
    const dvy = (changeFlags & DELTA_VELOCITY_CHANGED) ? dvyScaled / DELTA_SCALE_FACTOR : 0;
    const dvz = (changeFlags & DELTA_VELOCITY_CHANGED) ? dvzScaled / DELTA_SCALE_FACTOR : 0;

    // FIX 3 (client): V4 delta overflow sanity check.
    // If applying this delta would produce an out-of-range position (> 100000),
    // the delta was likely clamped due to i16 overflow on the server.
    // Log a warning so developers can diagnose the issue.
    const POSITION_SANITY_BOUND = 100000;
    if (Math.abs(dx) > POSITION_SANITY_BOUND || Math.abs(dy) > POSITION_SANITY_BOUND || Math.abs(dz) > POSITION_SANITY_BOUND) {
      logger.warn(
        `V4 delta overflow detected for node ${nodeId}: delta=[${dx.toFixed(2)}, ${dy.toFixed(2)}, ${dz.toFixed(2)}]. ` +
        `Likely i16 clamping corruption. Wait for next full V3 frame.`
      );
    }

    nodes.push({
      nodeId,
      position: { x: dx, y: dy, z: dz },
      velocity: { x: dvx, y: dvy, z: dvz },
      ssspDistance: Infinity,
      ssspParent: -1,
    });

    offset += DELTA_ITEM_SIZE;
  }

  return nodes;
}

/**
 * Parse V5 frame: [1 byte: version=5][8 bytes: broadcast_sequence LE][V3 node data without version byte]
 * Extracts broadcast sequence, reconstructs a V3 buffer, and delegates to V3 parsing.
 * Updates the module-level `lastBroadcastSequence` export.
 */
function parseV5Nodes(buffer: ArrayBuffer): BinaryNodeData[] {
  // Minimum: 1 (version) + 8 (sequence) = 9 bytes
  if (buffer.byteLength < 9) {
    return [];
  }
  const view = new DataView(buffer);
  // Read 8-byte LE u64 as Number (safe up to 2^53)
  const seqLow = view.getUint32(1, true);
  const seqHigh = view.getUint32(5, true);
  lastBroadcastSequence = seqLow + seqHigh * 0x100000000;

  // Reconstruct a V3 buffer: [version=3][node data from offset 9 onward]
  const nodeDataLen = buffer.byteLength - 9;
  if (nodeDataLen <= 0) {
    return [];
  }
  const v3Buffer = new ArrayBuffer(1 + nodeDataLen);
  const v3View = new Uint8Array(v3Buffer);
  v3View[0] = PROTOCOL_V3;
  v3View.set(new Uint8Array(buffer, 9), 1);
  return parseBinaryNodeData(v3Buffer);
}

/**
 * Parse binary data and return a typed frame that distinguishes full vs delta updates.
 * This is the preferred entry point for callers that need to handle delta encoding.
 *
 * - Full frames (V2/V3/V5): nodes contain absolute positions
 * - Delta frames (V4): nodes contain position/velocity DELTAS
 */
export function parseBinaryFrameData(buffer: ArrayBuffer): ParsedBinaryFrame {
  if (!buffer || buffer.byteLength === 0) {
    return { type: 'full', nodes: [] };
  }

  const safeBuffer = buffer.slice(0);
  const view = new DataView(safeBuffer);
  const protocolVersion = view.getUint8(0);

  if (protocolVersion === PROTOCOL_V4) {
    const frameNumber = safeBuffer.byteLength >= 2 ? view.getUint8(1) : 0;
    const deltaNodes = parseDeltaNodes(safeBuffer);
    return {
      type: 'delta',
      nodes: deltaNodes,
      frameNumber,
    };
  }

  if (protocolVersion === PROTOCOL_V5) {
    // V5: extract broadcast sequence, parse node data as V3
    const v5Nodes = parseV5Nodes(safeBuffer);
    return {
      type: 'full',
      nodes: v5Nodes,
      broadcastSequence: lastBroadcastSequence,
    };
  }

  // V2/V3: delegate to existing full-state parser
  const fullNodes = parseBinaryNodeData(buffer);
  return { type: 'full', nodes: fullNodes };
}

/**
 * Create binary node data for sending to server
 * Uses Protocol V3 format (52 bytes per node)
 */
export function createBinaryNodeData(nodes: BinaryNodeData[]): ArrayBuffer {
  // 1 byte version header + nodes * 52 bytes
  const buffer = new ArrayBuffer(1 + nodes.length * BINARY_NODE_SIZE_V3);
  const view = new DataView(buffer);

  // Write version header
  view.setUint8(0, PROTOCOL_V3);

  nodes.forEach((node, i) => {
    const offset = 1 + (i * BINARY_NODE_SIZE_V3);

    // Node ID
    view.setUint32(offset + BINARY_NODE_ID_OFFSET, node.nodeId, true);

    // Position
    view.setFloat32(offset + BINARY_POSITION_OFFSET, node.position.x, true);
    view.setFloat32(offset + BINARY_POSITION_OFFSET + 4, node.position.y, true);
    view.setFloat32(offset + BINARY_POSITION_OFFSET + 8, node.position.z, true);

    // Velocity
    view.setFloat32(offset + BINARY_VELOCITY_OFFSET, node.velocity.x, true);
    view.setFloat32(offset + BINARY_VELOCITY_OFFSET + 4, node.velocity.y, true);
    view.setFloat32(offset + BINARY_VELOCITY_OFFSET + 8, node.velocity.z, true);

    // SSSP data
    view.setFloat32(offset + BINARY_SSSP_DISTANCE_OFFSET, node.ssspDistance ?? Infinity, true);
    view.setInt32(offset + BINARY_SSSP_PARENT_OFFSET, node.ssspParent ?? -1, true);

    // V3 analytics data
    view.setUint32(offset + BINARY_CLUSTER_ID_OFFSET, node.clusterId ?? 0, true);
    view.setFloat32(offset + BINARY_ANOMALY_SCORE_OFFSET, node.anomalyScore ?? 0, true);
    view.setUint32(offset + BINARY_COMMUNITY_ID_OFFSET, node.communityId ?? 0, true);
    view.setFloat32(offset + BINARY_CENTRALITY_OFFSET, node.centrality ?? 0, true);
  });

  return buffer;
}

/**
 * Message type constants (must match server)
 */
export enum MessageType {
  BinaryPositions = 0x00,
  VoiceData = 0x02,
  ControlFrame = 0x03,
  PositionDelta = 0x04,
  BroadcastAck = 0x34,
}
