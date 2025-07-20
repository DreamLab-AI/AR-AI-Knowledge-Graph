import React, { useRef, useEffect, useState, useMemo, useCallback } from 'react'
import { useThree, useFrame, ThreeEvent } from '@react-three/fiber'
import { Line } from '@react-three/drei/core/Line'
// Assuming Text and Billboard are still directly available, if not adjust path later
import { Text, Billboard } from '@react-three/drei' // OrbitControls removed
// Use namespace import for THREE to access constructors
import * as THREE from 'three'
import { graphDataManager, type GraphData, type Node as GraphNode } from '../managers/graphDataManager'
import { graphWorkerProxy } from '../managers/graphWorkerProxy'; // Import the proxy
import { createLogger, createErrorMetadata } from '../../../utils/logger'
import { debugState } from '../../../utils/debugState'
import { useSettingsStore } from '../../../store/settingsStore'
import { BinaryNodeData, createBinaryNodeData } from '../../../types/binaryProtocol'

const logger = createLogger('GraphManager')

// Function to get random position if node is at origin
const getPositionForNode = (node: GraphNode, index: number): [number, number, number] => {
  if (!node.position ||
      (node.position.x === 0 && node.position.y === 0 && node.position.z === 0)) {
    // All nodes are at (0,0,0), so generate a random position in a sphere
    const radius = 10
    const phi = Math.acos(2 * Math.random() - 1)
    const theta = Math.random() * Math.PI * 2

    const x = radius * Math.sin(phi) * Math.cos(theta)
    const y = radius * Math.sin(phi) * Math.sin(theta)
    const z = radius * Math.cos(phi)

    // Update the original node position so edges will work
    if (node.position) {
      node.position.x = x
      node.position.y = y
      node.position.z = z
    } else {
      node.position = { x, y, z }
    }

    return [x, y, z]
  }

  return [node.position.x, node.position.y, node.position.z]
}

// Define props for GraphManager
const GraphManager: React.FC = () => {
  const meshRef = useRef<THREE.InstancedMesh>(null) // Initialize with null, use THREE namespace
  // REMOVE: const orbitControlsRef = useRef<any>(null);

  // Use useMemo for stable object references across renders
  const tempMatrix = useMemo(() => new THREE.Matrix4(), [])
  const tempPosition = useMemo(() => new THREE.Vector3(), [])
  const tempScale = useMemo(() => new THREE.Vector3(), [])
  const tempQuaternion = useMemo(() => new THREE.Quaternion(), [])
  const screenPosition = useMemo(() => new THREE.Vector2(), [])

  const [graphData, setGraphData] = useState<GraphData>({ nodes: [], edges: [] })
  const nodePositionsRef = useRef<Float32Array | null>(null);
  const [edgePoints, setEdgePoints] = useState<number[]>([]);
  const [nodesAreAtOrigin, setNodesAreAtOrigin] = useState(false)
  const settings = useSettingsStore(state => state.settings)
  const [forceUpdate, setForceUpdate] = useState(0) // Force re-render on settings change

  // Minimal drag state for UI feedback
  const [dragState, setDragState] = useState<{
    nodeId: string | null;
    instanceId: number | null;
  }>({ nodeId: null, instanceId: null });

  // Performance-optimized drag data using refs (no re-renders)
  const dragDataRef = useRef({
    // General state
    isDragging: false,
    pointerDown: false,
    nodeId: null as string | null,
    instanceId: null as number | null,

    // Drag detection
    startPointerPos: new THREE.Vector2(), // Screen position (in pixels) on pointer down
    startTime: 0, // Time on pointer down

    // 3D positions
    startNodePos3D: new THREE.Vector3(),
    currentNodePos3D: new THREE.Vector3(),

    // Server updates
    lastUpdateTime: 0,
    pendingUpdate: null as BinaryNodeData | null,
  });

  const { camera, size } = useThree()

  useEffect(() => {
    if (meshRef.current) {
      const count = graphData.nodes.length;
      const mesh = meshRef.current;
      mesh.count = count; // Set the count

      if (count > 0) {
        // Check if matrices need initialization (e.g., if they are identity)
        // This avoids re-initializing if positions are already set by useFrame
        let needsInitialization = false;
        const identityMatrix = new THREE.Matrix4(); // Re-use for comparison
        for (let i = 0; i < count; i++) {
          const currentMatrix = new THREE.Matrix4();
          // Ensure mesh has enough allocated matrices before calling getMatrixAt
          if (i < mesh.instanceMatrix.array.length / 16) { // 16 floats per matrix
            mesh.getMatrixAt(i, currentMatrix);
            if (currentMatrix.equals(identityMatrix)) {
              needsInitialization = true;
              break;
            }
          } else {
            // If count increased beyond allocated, it needs initialization
            needsInitialization = true;
            break;
          }
        }

        if (needsInitialization) {
          for (let i = 0; i < count; i++) {
            // Set to identity or a default non-zero position if appropriate
            mesh.setMatrixAt(i, tempMatrix.identity());
          }
        }
      }
      mesh.instanceMatrix.needsUpdate = true;
      if (debugState.isEnabled()) {
        logger.debug(`InstancedMesh count updated to: ${count}`);
      }
    }
  }, [graphData.nodes.length, tempMatrix]);

  // Separate matrix update function for better performance
  const updateInstanceMatrix = (
    index: number,
    x: number,
    y: number,
    z: number,
    scale: number
  ) => {
    if (!meshRef.current) return

    tempPosition.set(x, y, z)
    tempScale.set(scale, scale, scale)

    tempMatrix.makeScale(scale, scale, scale)
    tempMatrix.setPosition(tempPosition)

    meshRef.current.setMatrixAt(index, tempMatrix)
  }

  // Drag detection threshold in screen pixels
  const DRAG_THRESHOLD = 5;

  const slugifyNodeLabel = (label: string): string => {
    return label.toLowerCase().replace(/\s+/g, '%20');
  };

  const handlePointerDown = useCallback((event: ThreeEvent<PointerEvent>) => {
  const instanceId = event.instanceId;
  if (instanceId === undefined) return;

  event.stopPropagation();
  const node = graphData.nodes[instanceId];
  if (!node) return;

  // Record initial state for drag detection
  dragDataRef.current = {
    ...dragDataRef.current,
    pointerDown: true,
    isDragging: false, // Reset dragging state
    nodeId: node.id,
    instanceId,
    startPointerPos: new THREE.Vector2(event.nativeEvent.offsetX, event.nativeEvent.offsetY),
    startTime: Date.now(),
    startNodePos3D: new THREE.Vector3(node.position.x, node.position.y, node.position.z),
    currentNodePos3D: new THREE.Vector3(node.position.x, node.position.y, node.position.z),
  };

  if (debugState.isEnabled()) {
    logger.debug(`Pointer down on node ${node.id}`);
  }
}, [graphData.nodes]);

  const handlePointerMove = useCallback((event: ThreeEvent<PointerEvent>) => {
    const drag = dragDataRef.current;
    if (!drag.pointerDown) return; // Only proceed if pointer is down

    // Step 1: Check if we should START dragging
    if (!drag.isDragging) {
      const currentPos = new THREE.Vector2(event.nativeEvent.offsetX, event.nativeEvent.offsetY);
      const distance = currentPos.distanceTo(drag.startPointerPos);

      if (distance > DRAG_THRESHOLD) {
        // Threshold exceeded, officially start the drag
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

    // Step 2: If we are dragging, execute the move logic
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
  }, [camera, settings?.visualisation?.nodes?.nodeSize]);

  const handlePointerUp = useCallback(() => {
    const drag = dragDataRef.current;
    if (!drag.pointerDown) return; // Not a tracked interaction

    if (drag.isDragging) {
      // --- End of a DRAG action ---
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
      // --- This was a CLICK action ---
      const node = graphData.nodes.find(n => n.id === drag.nodeId);
      if (node?.label) {
        if (debugState.isEnabled()) logger.debug(`Click action on node ${node.id}`);

        const slug = slugifyNodeLabel(node.label);
        const narrativeGoldmineUrl = `https://narrativegoldmine.com//#/page/${slug}`;
        const narrativeIframe = document.getElementById('narrative-goldmine-iframe') as HTMLIFrameElement | null;

        if (narrativeIframe) {
          narrativeIframe.src = narrativeGoldmineUrl;
        } else {
          logger.warn('Narrative Goldmine iframe not found. Cannot update URL.');
        }
      }
    }

    // --- Reset state for the next interaction ---
    dragDataRef.current.pointerDown = false;
    dragDataRef.current.isDragging = false;
    dragDataRef.current.nodeId = null;
    dragDataRef.current.instanceId = null;
    dragDataRef.current.pendingUpdate = null;
    setDragState({ nodeId: null, instanceId: null });

  }, [graphData.nodes]);

  // Global pointer up listener for cases where mouse is released outside canvas
  useEffect(() => {
    const handleGlobalPointerUp = () => {
      if (dragDataRef.current.pointerDown) {
        handlePointerUp();
      }
    };

    window.addEventListener('pointerup', handleGlobalPointerUp);
    return () => {
      window.removeEventListener('pointerup', handleGlobalPointerUp);
    };
  }, [handlePointerUp]);

  // Pass settings to worker whenever they change
  useEffect(() => {
    graphWorkerProxy.updateSettings(settings);
  }, [settings]);

  // Subscribe to graph *structural* changes (add/remove nodes)
  useEffect(() => {
    const handleGraphDataChange = (newData: GraphData) => {
      // This now only handles structural changes, not frequent position updates
      setGraphData(newData);
    };
    const unsubscribeData = graphDataManager.onGraphDataChange(handleGraphDataChange);

    // Initial data load
    graphDataManager.getGraphData().then(handleGraphDataChange);

    return () => unsubscribeData();
  }, []);

  // Update node positions from binary data
  // Update node positions - Modified to NOT directly update mesh matrices from WebSocket data
  const updateNodePositions = useCallback((positions: Float32Array) => {
    // This function is called by GraphDataManager when WebSocket binary data arrives.
    // GraphDataManager is responsible for updating the central 'graphData' state.
    // This component (GraphManager) re-renders when 'graphData' (from useState) changes.
    // The useFrame hook then uses the updated 'graphData' to set instance matrices.
    // Therefore, this callback doesn't need to directly manipulate meshRef.current.
    if (debugState.isEnabled()) {
      const sample = positions.slice(0, Math.min(12, positions.length)); // Log first few nodes
      logger.debug('GraphManager received raw position update data (sample):', sample);
    }
  }, []); // No dependencies needed if it's just logging or relying on external state updates.

  // Constants for file size normalization
  const MIN_LOG_FILE_SIZE_ESTIMATE = Math.log10(100 + 1); // Approx 2, for 100 bytes
  const MAX_LOG_FILE_SIZE_ESTIMATE = Math.log10(5 * 1024 * 1024 + 1); // Approx 6.7, for 5MB
  const BASE_SPHERE_RADIUS = 0.5;

  const labelsRef = useRef<THREE.Group>(null!);
  const [_, setForceRender] = useState(0);


  // The main animation loop
  useFrame(async (state, delta) => {
    if (!meshRef.current || !labelsRef.current || graphData.nodes.length === 0) return;

    // Pull smooth positions from the worker on every frame
    const positions = await graphWorkerProxy.tick(delta);
    nodePositionsRef.current = positions;

    if (positions) {
      // 1. Update InstancedMesh (Nodes)
      const nodeSize = settings?.visualisation?.nodes?.nodeSize || 0.01;
      const BASE_SPHERE_RADIUS = 0.5;
      const scale = nodeSize / BASE_SPHERE_RADIUS;
      const tempMatrix = new THREE.Matrix4();

      for (let i = 0; i < graphData.nodes.length; i++) {
        const i3 = i * 3;
        tempMatrix.makeScale(scale, scale, scale);
        tempMatrix.setPosition(positions[i3], positions[i3 + 1], positions[i3 + 2]);
        meshRef.current.setMatrixAt(i, tempMatrix);
      }
      meshRef.current.instanceMatrix.needsUpdate = true;

      // 2. Update Line (Edges)
      const newEdgePoints: number[] = [];
      graphData.edges.forEach(edge => {
        const sourceNodeIndex = graphData.nodes.findIndex(n => n.id === edge.source);
        const targetNodeIndex = graphData.nodes.findIndex(n => n.id === edge.target);
        if (sourceNodeIndex !== -1 && targetNodeIndex !== -1) {
          const i3s = sourceNodeIndex * 3;
          const i3t = targetNodeIndex * 3;
          newEdgePoints.push(positions[i3s], positions[i3s + 1], positions[i3s + 2]);
          newEdgePoints.push(positions[i3t], positions[i3t + 1], positions[i3t + 2]);
        }
      });
      setEdgePoints(newEdgePoints);


      // 3. Update Text Labels
      const labelSettings = settings?.visualisation?.labels;
      const mainLabelFontSize = labelSettings?.desktopFontSize || 0.1;
      const lineSpacing = mainLabelFontSize * 0.15;
      labelsRef.current.children.forEach((billboard, i) => {
        if (billboard instanceof THREE.Group && graphData.nodes[i]) {
           const node = graphData.nodes[i];
           let metadataString = '';
           if (node.metadata?.fileSize) {
              metadataString = `${(parseInt(node.metadata.fileSize, 10) / 1024).toFixed(1)} KB`;
           }
           const i3 = i * 3;
           billboard.position.set(
              positions[i3],
              positions[i3 + 1] + (labelSettings?.textPadding || 0.3) + (metadataString ? mainLabelFontSize / 2 + lineSpacing / 2 : 0),
              positions[i3 + 2]
           );
        }
      });
    }

    // ... handle pending drag updates ...
    const drag = dragDataRef.current;
    if (drag.pendingUpdate && graphDataManager.webSocketService && graphDataManager.webSocketService.isReady()) {
      graphDataManager.webSocketService.send(
        createBinaryNodeData([drag.pendingUpdate])
      );
      drag.pendingUpdate = null; // Clear after sending
    }
  });


  // Node labels component using settings from YAML - Rendered once, updated in useFrame
  const NodeLabels = useMemo(() => {
    const labelSettings = settings?.visualisation?.labels;
    if (!labelSettings?.enableLabels) return null;

    const mainLabelFontSize = labelSettings.desktopFontSize || 0.1;
    const metadataFontSize = mainLabelFontSize * 0.7;
    const lineSpacing = mainLabelFontSize * 0.15;

    return (
      <group ref={labelsRef}>
        {graphData.nodes.map((node) => {
          if (!node.label) return null;
          let metadataString = '';
          if (node.metadata?.fileSize) {
            metadataString = `${(parseInt(node.metadata.fileSize, 10) / 1024).toFixed(1)} KB`;
          }
          return (
            <Billboard key={node.id} follow={labelSettings.billboardMode === 'camera'}>
              <Text
                fontSize={mainLabelFontSize}
                color={labelSettings.textColor || '#000000'}
                anchorX="center"
                anchorY="middle"
                outlineWidth={labelSettings.textOutlineWidth || 0.01}
                outlineColor={labelSettings.textOutlineColor || '#ffffff'}
                renderOrder={10}
                material-depthTest={false}
              >
                {node.label}
              </Text>
              {metadataString && (
                <Text
                  fontSize={metadataFontSize}
                  color={labelSettings.textColor ? new THREE.Color(labelSettings.textColor).multiplyScalar(0.8).getStyle() : '#333333'}
                  anchorX="center"
                  anchorY="top"
                  position={[0, -mainLabelFontSize / 2 - lineSpacing, 0]}
                  outlineWidth={(labelSettings.textOutlineWidth || 0.01) * 0.7}
                  outlineColor={labelSettings.textOutlineColor || '#ffffff'}
                  renderOrder={10}
                  material-depthTest={false}
                >
                  {metadataString}
                </Text>
              )}
            </Billboard>
          );
        })}
      </group>
    );
  }, [graphData.nodes, settings?.visualisation?.labels]);

  return (
    <>
      {/* REMOVE THE LOCAL OrbitControls INSTANCE:
      <OrbitControls ref={orbitControlsRef} ... />
      */}

      <instancedMesh
        ref={meshRef}
        args={[undefined, undefined, graphData.nodes.length]} // Geometry, Material, Count
        frustumCulled={false}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp} // To handle release on the mesh
        onPointerMissed={() => { // To handle release outside the mesh but on canvas
          if (dragDataRef.current.isDragging) {
            handlePointerUp();
          }
        }}
      >
        <sphereGeometry args={[0.5, 16, 16]} />
        <meshStandardMaterial
          color={settings?.visualisation?.nodes?.baseColor || "#ffffff"}
          emissive={settings?.visualisation?.nodes?.baseColor || "#00ffff"}
          emissiveIntensity={0.8}
          metalness={settings?.visualisation?.nodes?.metalness || 0.2}
          roughness={settings?.visualisation?.nodes?.roughness || 0.3}
          opacity={settings?.visualisation?.nodes?.opacity || 1.0}
          transparent={true}
          toneMapped={false} // Important for bloom effect
        />
      </instancedMesh>

      {edgePoints.length > 0 && (
        <Line
          points={edgePoints}
          color={settings?.visualisation?.edges?.color || "#00ffff"}
          lineWidth={settings?.visualisation?.edges?.baseWidth || 1.0}
          transparent
          opacity={settings?.visualisation?.edges?.opacity || 0.6}
          toneMapped={false}
        />
      )}
      {NodeLabels}
    </>
  )
}


export default GraphManager
