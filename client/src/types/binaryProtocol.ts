/**
 * Binary protocol types for WebSocket communication
 * 
 * This aligns with the server's binary protocol format (src/utils/binary_protocol.rs)
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
}

/**
 * Node binary format:
 * - Node ID: 4 bytes (uint32)
 * - Position: 12 bytes (3 float32 values)
 * - Velocity: 12 bytes (3 float32 values)
 * Total: 28 bytes per node
 */
export const BINARY_NODE_SIZE = 28;
export const BINARY_NODE_ID_OFFSET = 0;
export const BINARY_POSITION_OFFSET = 4;
export const BINARY_VELOCITY_OFFSET = 16;

/**
 * Parse binary data buffer into an array of BinaryNodeData objects
 */
export function parseBinaryNodeData(buffer: ArrayBuffer): BinaryNodeData[] {
  if (!buffer || buffer.byteLength === 0) {
    return [];
  }

  // Make a copy of the buffer to avoid any issues with shared references
  const safeBuffer = buffer.slice(0);
  const view = new DataView(safeBuffer);
  const nodes: BinaryNodeData[] = [];
  
  try {
    // Check if data length is not a multiple of the expected size
    if (safeBuffer.byteLength % BINARY_NODE_SIZE !== 0) {
      console.warn(`Binary data length (${safeBuffer.byteLength} bytes) is not a multiple of ${BINARY_NODE_SIZE}. This may indicate compressed data.`);
      console.warn(`First few bytes: ${new Uint8Array(safeBuffer.slice(0, Math.min(16, safeBuffer.byteLength))).join(', ')}`);
      
      // Check for zlib header (0x78 followed by compression level byte)
      const header = new Uint8Array(safeBuffer.slice(0, Math.min(4, safeBuffer.byteLength)));
      if (header[0] === 0x78 && (header[1] === 0x01 || header[1] === 0x5E || header[1] === 0x9C || header[1] === 0xDA)) {
        console.error("Data appears to be zlib compressed but decompression failed or wasn't attempted");
      }
    }
    
    // Calculate how many complete nodes we can process
    const completeNodes = Math.floor(safeBuffer.byteLength / BINARY_NODE_SIZE);
    
    if (completeNodes === 0) {
      console.warn(`Received binary data with insufficient length: ${safeBuffer.byteLength} bytes (needed at least ${BINARY_NODE_SIZE} bytes per node)`);
      return [];
    }
    
    for (let i = 0; i < completeNodes; i++) {
      const offset = i * BINARY_NODE_SIZE;
      
      // Bounds check to prevent errors on corrupted data
      if (offset + BINARY_NODE_SIZE > safeBuffer.byteLength) {
        break;
      }
      
      // Read node ID (uint32, 4 bytes)
      const nodeId = view.getUint32(offset + BINARY_NODE_ID_OFFSET, true);
      
      // Read position (3 float32 values, 12 bytes)
      const position: Vec3 = {
        x: view.getFloat32(offset + BINARY_POSITION_OFFSET, true),
        y: view.getFloat32(offset + BINARY_POSITION_OFFSET + 4, true),
        z: view.getFloat32(offset + BINARY_POSITION_OFFSET + 8, true)
      };
      
      // Read velocity (3 float32 values, 12 bytes)
      const velocity: Vec3 = {
        x: view.getFloat32(offset + BINARY_VELOCITY_OFFSET, true),
        y: view.getFloat32(offset + BINARY_VELOCITY_OFFSET + 4, true),
        z: view.getFloat32(offset + BINARY_VELOCITY_OFFSET + 8, true)
      };

      // Basic validation to detect corrupted data
      const isValid = 
        !isNaN(position.x) && isFinite(position.x) &&
        !isNaN(position.y) && isFinite(position.y) &&
        !isNaN(position.z) && isFinite(position.z) &&
        !isNaN(velocity.x) && isFinite(velocity.x) &&
        !isNaN(velocity.y) && isFinite(velocity.y) &&
        !isNaN(velocity.z) && isFinite(velocity.z);
      
      if (isValid) {
        nodes.push({ nodeId, position, velocity });
      } else {
        console.warn(`Skipping corrupted node data at offset ${offset} (nodeId: ${nodeId})`);
      }
    }
  } catch (error) {
    console.error('Error parsing binary data:', error);
    // Return any nodes we've successfully parsed
  }

  return nodes;
}

/**
 * Create a binary buffer from an array of BinaryNodeData objects
 */
export function createBinaryNodeData(nodes: BinaryNodeData[]): ArrayBuffer {
  const buffer = new ArrayBuffer(nodes.length * BINARY_NODE_SIZE);
  const view = new DataView(buffer);
  
  nodes.forEach((node, i) => {
    const offset = i * BINARY_NODE_SIZE;
    
    // Write node ID (uint32, 4 bytes)
    view.setUint32(offset + BINARY_NODE_ID_OFFSET, node.nodeId, true);
    
    // Write position (3 float32 values, 12 bytes)
    view.setFloat32(offset + BINARY_POSITION_OFFSET, node.position.x, true);
    view.setFloat32(offset + BINARY_POSITION_OFFSET + 4, node.position.y, true);
    view.setFloat32(offset + BINARY_POSITION_OFFSET + 8, node.position.z, true);
    
    // Write velocity (3 float32 values, 12 bytes)
    view.setFloat32(offset + BINARY_VELOCITY_OFFSET, node.velocity.x, true);
    view.setFloat32(offset + BINARY_VELOCITY_OFFSET + 4, node.velocity.y, true);
    view.setFloat32(offset + BINARY_VELOCITY_OFFSET + 8, node.velocity.z, true);
  });
  
  return buffer;
}