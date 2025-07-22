import { useCallback } from 'react';
import { ThreeEvent } from '@react-three/fiber';
import * as THREE from 'three';
import { graphDataManager } from '../managers/graphDataManager';
import { graphWorkerProxy } from '../managers/graphWorkerProxy';
import { createBinaryNodeData, BinaryNodeData } from '../../../types/binaryProtocol';
import { createLogger } from '../../../utils/logger';
import { debugState } from '../../../utils/debugState';
import { navigateNarrativeGoldmine } from '../../../utils/iframeCommunication';

const logger = createLogger('GraphManager');

const DRAG_THRESHOLD = 5; // pixels
const BASE_SPHERE_RADIUS = 0.5;

// Helper function to slugify node labels
const slugifyNodeLabel = (label: string): string => {
  return label
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
};

export const createEventHandlers = (
  meshRef: React.RefObject<THREE.InstancedMesh>,
  dragDataRef: React.MutableRefObject<any>,
  setDragState: React.Dispatch<React.SetStateAction<{ nodeId: string | null; instanceId: number | null }>>,
  graphData: any,
  camera: THREE.Camera,
  size: { width: number; height: number },
  settings: any,
  setGraphData: React.Dispatch<React.SetStateAction<any>>
) => {
  const handlePointerDown = useCallback((event: ThreeEvent<PointerEvent>) => {
    event.stopPropagation();
    if (!meshRef.current) return;

    const instanceId = event.instanceId;
    if (instanceId === undefined || instanceId < 0 || instanceId >= graphData.nodes.length) return;

    const node = graphData.nodes[instanceId];
    if (!node || !node.position) return;

    // Record the initial state for drag detection
    dragDataRef.current = {
      ...dragDataRef.current,
      pointerDown: true,
      nodeId: node.id,
      instanceId: instanceId,
      startPointerPos: new THREE.Vector2(event.nativeEvent.offsetX, event.nativeEvent.offsetY),
      startTime: Date.now(),
      startNodePos3D: new THREE.Vector3(node.position.x, node.position.y, node.position.z),
      currentNodePos3D: new THREE.Vector3(node.position.x, node.position.y, node.position.z),
    };

    if (debugState.isEnabled()) {
      logger.debug(`Pointer down on node ${node.id}`);
    }
  }, [graphData.nodes, meshRef, dragDataRef]);

  const handlePointerMove = useCallback((event: ThreeEvent<PointerEvent>) => {
    const drag = dragDataRef.current;
    if (!drag.pointerDown) return;

    // Check if we should START dragging
    if (!drag.isDragging) {
      const currentPos = new THREE.Vector2(event.nativeEvent.offsetX, event.nativeEvent.offsetY);
      const distance = currentPos.distanceTo(drag.startPointerPos);

      if (distance > DRAG_THRESHOLD) {
        drag.isDragging = true;
        setDragState({ nodeId: drag.nodeId, instanceId: drag.instanceId });

        const numericId = graphDataManager.nodeIdMap.get(drag.nodeId!);
        if (numericId !== undefined) {
          graphWorkerProxy.pinNode(numericId);
        }
        if (debugState.isEnabled()) {
          logger.debug(`Drag started on node ${drag.nodeId}`);
        }
      }
    }

    // If we are dragging, execute the move logic
    if (drag.isDragging) {
      event.stopPropagation();

      // Create a plane at the node's starting depth, facing the camera
      const planeNormal = camera.getWorldDirection(new THREE.Vector3()).negate();
      const plane = new THREE.Plane(planeNormal, -planeNormal.dot(drag.startNodePos3D));

      // Cast a ray from the camera through the current mouse position
      const raycaster = new THREE.Raycaster();
      raycaster.setFromCamera(event.pointer, camera);

      // Find where the ray intersects the plane
      const intersection = new THREE.Vector3();
      if (raycaster.ray.intersectPlane(plane, intersection)) {
        const numericId = graphDataManager.nodeIdMap.get(drag.nodeId!);
        if (numericId !== undefined) {
          graphWorkerProxy.updateUserDrivenNodePosition(numericId, intersection);
        }

        drag.currentNodePos3D.copy(intersection);

        // Update visual immediately for responsiveness
        const nodeSize = settings?.visualisation?.nodes?.nodeSize || 0.01;
        const scale = nodeSize / BASE_SPHERE_RADIUS;
        const tempMatrix = new THREE.Matrix4();
        tempMatrix.makeScale(scale, scale, scale);
        tempMatrix.setPosition(drag.currentNodePos3D);
        if (meshRef.current) {
          meshRef.current.setMatrixAt(drag.instanceId!, tempMatrix);
          meshRef.current.instanceMatrix.needsUpdate = true;
        }

        // Update graphData to keep edges/labels in sync
        setGraphData(prev => ({
          ...prev,
          nodes: prev.nodes.map((node, idx) =>
            idx === drag.instanceId
              ? { ...node, position: { x: drag.currentNodePos3D.x, y: drag.currentNodePos3D.y, z: drag.currentNodePos3D.z } }
              : node
          )
        }));

        // Throttle WebSocket updates
        const now = Date.now();
        if (now - drag.lastUpdateTime > 30) {
          if (numericId !== undefined) {
            drag.pendingUpdate = {
              nodeId: numericId,
              position: { x: drag.currentNodePos3D.x, y: drag.currentNodePos3D.y, z: drag.currentNodePos3D.z },
              velocity: { x: 0, y: 0, z: 0 }
            };
            drag.lastUpdateTime = now;
          }
        }
      }
    }
  }, [camera, settings?.visualisation?.nodes?.nodeSize, meshRef, dragDataRef, setDragState, setGraphData]);

  const handlePointerUp = useCallback(() => {
    const drag = dragDataRef.current;
    if (!drag.pointerDown) return;

    if (drag.isDragging) {
      // End of a DRAG action
      if (debugState.isEnabled()) logger.debug(`Drag ended for node ${drag.nodeId}`);

      const numericId = graphDataManager.nodeIdMap.get(drag.nodeId!);
      if (numericId !== undefined) {
        graphWorkerProxy.unpinNode(numericId);

        // Send final position update
        const finalUpdate: BinaryNodeData = {
          nodeId: numericId,
          position: { x: drag.currentNodePos3D.x, y: drag.currentNodePos3D.y, z: drag.currentNodePos3D.z },
          velocity: { x: 0, y: 0, z: 0 }
        };
        graphDataManager.webSocketService?.send(createBinaryNodeData([finalUpdate]));
      }
    } else {
      // This was a CLICK action
      const node = graphData.nodes.find(n => n.id === drag.nodeId);
      if (node?.label) {
        if (debugState.isEnabled()) logger.debug(`Click action on node ${node.id}`);

        const slug = slugifyNodeLabel(node.label);
        
        // Use the utility function for secure iframe communication
        const success = navigateNarrativeGoldmine(node.id, node.label, slug);
        
        if (!success) {
          logger.warn('Failed to send navigation message to Narrative Goldmine iframe');
        } else if (debugState.isEnabled()) {
          logger.debug(`Successfully sent navigation message for node ${node.id} to iframe`);
        }
      }
    }

    // Reset state for the next interaction
    dragDataRef.current.pointerDown = false;
    dragDataRef.current.isDragging = false;
    dragDataRef.current.nodeId = null;
    dragDataRef.current.instanceId = null;
    dragDataRef.current.pendingUpdate = null;
    setDragState({ nodeId: null, instanceId: null });
  }, [graphData.nodes, dragDataRef, setDragState]);

  return {
    handlePointerDown,
    handlePointerMove,
    handlePointerUp
  };
};