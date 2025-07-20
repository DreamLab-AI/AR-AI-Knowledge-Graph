import { useEffect, useRef, useCallback } from 'react';
import { webSocketService } from '../../../services/WebSocketService';
import { createLogger } from '../../../utils/logger';
import * as THREE from 'three';

const logger = createLogger('useSwarmBinaryUpdates');

interface SwarmBinaryUpdateOptions {
  enabled: boolean;
  onPositionUpdate?: (positions: Float32Array) => void;
}

export function useSwarmBinaryUpdates({
  enabled,
  onPositionUpdate
}: SwarmBinaryUpdateOptions) {
  const positionsRef = useRef<Float32Array | null>(null);
  const nodeIdMapRef = useRef<Map<number, number>>(new Map());
  
  // Handle binary updates from WebSocket
  const handleBinaryData = useCallback((data: ArrayBuffer) => {
    try {
      const view = new DataView(data);
      const nodeCount = data.byteLength / 28; // 28 bytes per node
      
      // Check if any nodes have the swarm flag (0x80)
      let hasSwarmNodes = false;
      for (let i = 0; i < nodeCount; i++) {
        const offset = i * 28;
        const flags = view.getUint8(offset + 24); // flags are at offset 24
        if (flags & 0x80) {
          hasSwarmNodes = true;
          break;
        }
      }
      
      if (!hasSwarmNodes) {
        return; // Not swarm data
      }
      
      logger.debug(`Processing swarm binary update: ${nodeCount} nodes`);
      
      // Extract positions for swarm nodes
      const swarmPositions: { id: number, position: THREE.Vector3 }[] = [];
      
      for (let i = 0; i < nodeCount; i++) {
        const offset = i * 28;
        const nodeId = view.getUint32(offset, true); // little-endian
        const flags = view.getUint8(offset + 24);
        
        if (flags & 0x80) { // Check swarm flag
          const x = view.getFloat32(offset + 4, true);
          const y = view.getFloat32(offset + 8, true);
          const z = view.getFloat32(offset + 12, true);
          
          swarmPositions.push({
            id: nodeId,
            position: new THREE.Vector3(x, y, z)
          });
        }
      }
      
      // Update positions array if we have swarm nodes
      if (swarmPositions.length > 0) {
        // Initialize positions array if needed
        if (!positionsRef.current || positionsRef.current.length !== swarmPositions.length * 3) {
          positionsRef.current = new Float32Array(swarmPositions.length * 3);
          nodeIdMapRef.current.clear();
        }
        
        // Update positions and mapping
        swarmPositions.forEach((node, index) => {
          nodeIdMapRef.current.set(node.id, index);
          positionsRef.current![index * 3] = node.position.x;
          positionsRef.current![index * 3 + 1] = node.position.y;
          positionsRef.current![index * 3 + 2] = node.position.z;
        });
        
        // Notify callback
        if (onPositionUpdate && positionsRef.current) {
          onPositionUpdate(positionsRef.current);
        }
        
        logger.debug(`Updated ${swarmPositions.length} swarm node positions`);
      }
    } catch (error) {
      logger.error('Error processing swarm binary data:', error);
    }
  }, [onPositionUpdate]);
  
  // Request swarm positions periodically
  const requestSwarmPositions = useCallback(() => {
    if (webSocketService.getConnectionStatus()) {
      webSocketService.send({
        type: 'requestSwarmPositions'
      });
    }
  }, []);
  
  useEffect(() => {
    if (!enabled) return;
    
    // Subscribe to binary updates
    const unsubscribe = webSocketService.onBinaryMessage(handleBinaryData);
    
    // Request initial swarm positions
    requestSwarmPositions();
    
    // Request updates periodically
    const interval = setInterval(requestSwarmPositions, 1000); // Every second
    
    return () => {
      unsubscribe();
      clearInterval(interval);
    };
  }, [enabled, handleBinaryData, requestSwarmPositions]);
  
  return {
    positions: positionsRef.current,
    nodeIdMap: nodeIdMapRef.current,
    requestUpdate: requestSwarmPositions
  };
}