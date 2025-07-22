import { useEffect, useRef, useCallback } from 'react';
import { webSocketService } from '../../../services/WebSocketService';
import { createLogger } from '../../../utils/logger';
import * as THREE from 'three';

const logger = createLogger('useBotsBinaryUpdates');

interface BotsBinaryUpdateOptions {
  enabled: boolean;
  onPositionUpdate?: (positions: Float32Array) => void;
}

export function useBotsBinaryUpdates({
  enabled,
  onPositionUpdate
}: BotsBinaryUpdateOptions) {
  const positionsRef = useRef<Float32Array | null>(null);
  const nodeIdMapRef = useRef<Map<number, number>>(new Map());

  // Handle binary updates from WebSocket
  const handleBinaryData = useCallback((data: ArrayBuffer) => {
    try {
      const view = new DataView(data);
      const nodeCount = data.byteLength / 28; // 28 bytes per node

      // Check if any nodes have the bots flag (0x80)
      let hasBotsNodes = false;
      for (let i = 0; i < nodeCount; i++) {
        const offset = i * 28;
        const flags = view.getUint8(offset + 24); // flags are at offset 24
        if (flags & 0x80) {
          hasBotsNodes = true;
          break;
        }
      }

      if (!hasBotsNodes) {
        return; // Not bots data
      }

      logger.debug(`Processing bots binary update: ${nodeCount} nodes`);

      // Extract positions for bots nodes
      const botsPositions: { id: number, position: THREE.Vector3 }[] = [];

      for (let i = 0; i < nodeCount; i++) {
        const offset = i * 28;
        const nodeId = view.getUint32(offset, true); // little-endian
        const flags = view.getUint8(offset + 24);

        if (flags & 0x80) { // Check bots flag
          const x = view.getFloat32(offset + 4, true);
          const y = view.getFloat32(offset + 8, true);
          const z = view.getFloat32(offset + 12, true);

          botsPositions.push({
            id: nodeId,
            position: new THREE.Vector3(x, y, z)
          });
        }
      }

      // Update positions array if we have bots nodes
      if (botsPositions.length > 0) {
        // Initialize positions array if needed
        if (!positionsRef.current || positionsRef.current.length !== botsPositions.length * 3) {
          positionsRef.current = new Float32Array(botsPositions.length * 3);
          nodeIdMapRef.current.clear();
        }

        // Update positions and mapping
        botsPositions.forEach((node, index) => {
          nodeIdMapRef.current.set(node.id, index);
          positionsRef.current![index * 3] = node.position.x;
          positionsRef.current![index * 3 + 1] = node.position.y;
          positionsRef.current![index * 3 + 2] = node.position.z;
        });

        // Notify callback
        if (onPositionUpdate && positionsRef.current) {
          onPositionUpdate(positionsRef.current);
        }

        logger.debug(`Updated ${botsPositions.length} bots node positions`);
      }
    } catch (error) {
      logger.error('Error processing bots binary data:', error);
    }
  }, [onPositionUpdate]);

  // Request bots positions periodically
  const requestBotsPositions = useCallback(() => {
    if (webSocketService.isReady()) {
      webSocketService.sendMessage('requestBotsPositions');
    }
  }, []);

  useEffect(() => {
    if (!enabled) return;

    // Subscribe to binary updates
    const unsubscribe = webSocketService.onBinaryMessage(handleBinaryData);

    // Request initial bots positions
    requestBotsPositions();

    // Request updates periodically
    const interval = setInterval(requestBotsPositions, 1000); // Every second

    return () => {
      unsubscribe();
      clearInterval(interval);
    };
  }, [enabled, handleBinaryData, requestBotsPositions]);

  return {
    positions: positionsRef.current,
    nodeIdMap: nodeIdMapRef.current,
    requestUpdate: requestBotsPositions
  };
}