import React, { useRef, useEffect, useState, useMemo, useCallback } from 'react'
import { useThree, useFrame, ThreeEvent } from '@react-three/fiber'
import { Text, Billboard } from '@react-three/drei'
import * as THREE from 'three'
import { graphDataManager, type GraphData, type Node as GraphNode } from '../managers/graphDataManager'
import { graphWorkerProxy } from '../managers/graphWorkerProxy'
import { createLogger, createErrorMetadata } from '../../../utils/logger'
import { debugState } from '../../../utils/debugState'
import { useSettingsStore } from '../../../store/settingsStore'
import { BinaryNodeData, createBinaryNodeData } from '../../../types/binaryProtocol'
import { HologramNodeMaterial } from '../shaders/HologramNodeMaterial'
import { FlowingEdges } from './FlowingEdges'
import { createEventHandlers } from './GraphManager_EventHandlers'

const logger = createLogger('GraphManager')

// Enhanced position calculation with better distribution
const getPositionForNode = (node: GraphNode, index: number, totalNodes: number): [number, number, number] => {
  if (!node.position || (node.position.x === 0 && node.position.y === 0 && node.position.z === 0)) {
    // Golden angle for better distribution
    const goldenAngle = Math.PI * (3 - Math.sqrt(5))
    const theta = index * goldenAngle
    const y = 1 - (index / totalNodes) * 2
    const radius = Math.sqrt(1 - y * y)
    
    const scaleFactor = 15 + Math.random() * 5 // Vary radius for organic feel
    const x = Math.cos(theta) * radius * scaleFactor
    const z = Math.sin(theta) * radius * scaleFactor
    const yScaled = y * scaleFactor
    
    if (node.position) {
      node.position.x = x
      node.position.y = yScaled
      node.position.z = z
    } else {
      node.position = { x, y: yScaled, z }
    }
    
    return [x, yScaled, z]
  }
  
  return [node.position.x, node.position.y, node.position.z]
}

// Get node color based on type/metadata
const getNodeColor = (node: GraphNode): THREE.Color => {
  const typeColors: Record<string, string> = {
    'folder': '#FFD700',     // Gold
    'file': '#00CED1',       // Dark turquoise
    'function': '#FF6B6B',   // Coral
    'class': '#4ECDC4',      // Turquoise
    'variable': '#95E1D3',   // Mint
    'import': '#F38181',     // Light coral
    'export': '#AA96DA',     // Lavender
  }
  
  const nodeType = node.metadata?.type || 'default'
  const color = typeColors[nodeType] || '#00ffff'
  return new THREE.Color(color)
}

// Get node scale based on importance/connections
const getNodeScale = (node: GraphNode, edges: any[]): number => {
  const baseSize = node.metadata?.size || 1.0
  const connectionCount = edges.filter(e => 
    e.source === node.id || e.target === node.id
  ).length
  
  // Scale based on connections (more connections = larger node)
  const connectionScale = 1 + Math.log(connectionCount + 1) * 0.3
  
  // Also scale based on node type importance
  const typeScale = getTypeImportance(node.metadata?.type)
  
  return baseSize * connectionScale * typeScale
}

// Get importance multiplier based on node type
const getTypeImportance = (nodeType?: string): number => {
  const importanceMap: Record<string, number> = {
    'folder': 1.5,      // Folders are important containers
    'function': 1.3,    // Functions are important code elements
    'class': 1.4,       // Classes are structural elements
    'file': 1.0,        // Files are baseline
    'variable': 0.8,    // Variables are smaller
    'import': 0.7,      // Imports are small
    'export': 0.9,      // Exports are medium
  }
  
  return importanceMap[nodeType || 'default'] || 1.0
}

const GraphManager: React.FC = () => {
  const meshRef = useRef<THREE.InstancedMesh>(null)
  const materialRef = useRef<HologramNodeMaterial | null>(null)
  const particleSystemRef = useRef<THREE.Points>(null)
  
  // Memoized objects for performance
  const tempMatrix = useMemo(() => new THREE.Matrix4(), [])
  const tempPosition = useMemo(() => new THREE.Vector3(), [])
  const tempScale = useMemo(() => new THREE.Vector3(), [])
  const tempQuaternion = useMemo(() => new THREE.Quaternion(), [])
  const tempColor = useMemo(() => new THREE.Color(), [])
  
  const [graphData, setGraphData] = useState<GraphData>({ nodes: [], edges: [] })
  const nodePositionsRef = useRef<Float32Array | null>(null)
  const [edgePoints, setEdgePoints] = useState<number[]>([])
  const [nodesAreAtOrigin, setNodesAreAtOrigin] = useState(false)
  const settings = useSettingsStore(state => state.settings)
  const [forceUpdate, setForceUpdate] = useState(0)
  
  // Animation state
  const animationStateRef = useRef({
    time: 0,
    selectedNode: null as string | null,
    hoveredNode: null as string | null,
    pulsePhase: 0,
  })
  
  // Drag state (same as original)
  const [dragState, setDragState] = useState<{
    nodeId: string | null;
    instanceId: number | null;
  }>({ nodeId: null, instanceId: null })
  
  const dragDataRef = useRef({
    isDragging: false,
    pointerDown: false,
    nodeId: null as string | null,
    instanceId: null as number | null,
    startPointerPos: new THREE.Vector2(),
    startTime: 0,
    startNodePos3D: new THREE.Vector3(),
    currentNodePos3D: new THREE.Vector3(),
    lastUpdateTime: 0,
    pendingUpdate: null as BinaryNodeData | null,
  })
  
  const { camera, size } = useThree()
  
  // Create custom hologram material
  useEffect(() => {
    if (!materialRef.current) {
      materialRef.current = new HologramNodeMaterial({
        baseColor: '#0066ff', // Bright blue base
        emissiveColor: '#00ffff', // Cyan emissive
        opacity: settings?.visualisation?.graphs?.logseq?.nodes?.opacity ?? settings?.visualisation?.nodes?.opacity ?? 0.8,
        enableHologram: true, // Force enable to test
        glowStrength: 3.0, // Maximum glow
        pulseSpeed: 1.0,
        hologramStrength: 0.8, // Strong hologram effect
        rimPower: 3.0, // Strong rim lighting
      })
      
      // Enable instance color support
      materialRef.current.defines = { ...materialRef.current.defines, USE_INSTANCING_COLOR: '' }
      materialRef.current.needsUpdate = true
    }
  }, [])
  
  // Update material settings for Logseq graph
  useEffect(() => {
    if (materialRef.current && settings?.visualisation) {
      // Use Logseq-specific settings, fallback to legacy nodes settings
      const logseqSettings = settings.visualisation.graphs?.logseq;
      const nodeSettings = logseqSettings?.nodes || settings.visualisation.nodes;
      
      materialRef.current.updateColors(
        nodeSettings?.baseColor || '#00ffff',
        nodeSettings?.baseColor || '#00ffff'
      )
      materialRef.current.uniforms.opacity.value = nodeSettings?.opacity ?? 0.8;
      materialRef.current.setHologramEnabled(
        nodeSettings?.enableHologram !== false
      )
      materialRef.current.updateHologramParams({
        glowStrength: settings.visualisation.animations?.pulseStrength || 1.0,
      })
    }
  }, [settings?.visualisation])
  
  // Initialize instance attributes
  useEffect(() => {
    if (meshRef.current && graphData.nodes.length > 0) {
      const mesh = meshRef.current
      mesh.count = graphData.nodes.length
      
      // Set up instance colors
      const colors = new Float32Array(graphData.nodes.length * 3)
      
      graphData.nodes.forEach((node, i) => {
        const color = getNodeColor(node)
        colors[i * 3] = color.r
        colors[i * 3 + 1] = color.g
        colors[i * 3 + 2] = color.b
      })
      
      mesh.geometry.setAttribute('instanceColor', new THREE.InstancedBufferAttribute(colors, 3))
      
      // Force material update
      if (materialRef.current) {
        materialRef.current.needsUpdate = true
      }
    }
  }, [graphData])
  
  // Pass settings to worker whenever they change
  useEffect(() => {
    graphWorkerProxy.updateSettings(settings);
  }, [settings]);
  
  // Animation loop with physics updates
  useFrame(async (state, delta) => {
    animationStateRef.current.time = state.clock.elapsedTime
    
    // Update material time
    if (materialRef.current) {
      materialRef.current.updateTime(animationStateRef.current.time)
    }
    
    // Get smooth positions from physics worker
    if (meshRef.current && graphData.nodes.length > 0) {
      const positions = await graphWorkerProxy.tick(delta);
      nodePositionsRef.current = positions;
      
      if (positions) {
        // Update node positions from physics
        const logseqSettings = settings?.visualisation?.graphs?.logseq;
        const nodeSettings = logseqSettings?.nodes || settings?.visualisation?.nodes;
        const nodeSize = nodeSettings?.nodeSize || 0.5;
        const BASE_SPHERE_RADIUS = 0.5;
        const baseScale = nodeSize / BASE_SPHERE_RADIUS;
        
        for (let i = 0; i < graphData.nodes.length; i++) {
          const i3 = i * 3;
          const node = graphData.nodes[i];
          const nodeScale = getNodeScale(node, graphData.edges) * baseScale;
          tempMatrix.makeScale(nodeScale, nodeScale, nodeScale);
          tempMatrix.setPosition(positions[i3], positions[i3 + 1], positions[i3 + 2]);
          meshRef.current.setMatrixAt(i, tempMatrix);
        }
        meshRef.current.instanceMatrix.needsUpdate = true;
        
        // Update edge points based on new positions with endpoint offset
        const newEdgePoints: number[] = [];
        graphData.edges.forEach(edge => {
          const sourceNodeIndex = graphData.nodes.findIndex(n => n.id === edge.source);
          const targetNodeIndex = graphData.nodes.findIndex(n => n.id === edge.target);
          if (sourceNodeIndex !== -1 && targetNodeIndex !== -1) {
            const i3s = sourceNodeIndex * 3;
            const i3t = targetNodeIndex * 3;
            
            // Get node positions
            const sourcePos = new THREE.Vector3(positions[i3s], positions[i3s + 1], positions[i3s + 2]);
            const targetPos = new THREE.Vector3(positions[i3t], positions[i3t + 1], positions[i3t + 2]);
            
            // Calculate edge direction
            const direction = new THREE.Vector3().subVectors(targetPos, sourcePos);
            const edgeLength = direction.length();
            
            if (edgeLength > 0) {
              direction.normalize();
              
              // Calculate node radii based on scale
              const sourceNode = graphData.nodes[sourceNodeIndex];
              const targetNode = graphData.nodes[targetNodeIndex];
              const logseqSettings = settings?.visualisation?.graphs?.logseq;
              const nodeSettings = logseqSettings?.nodes || settings?.visualisation?.nodes;
              const sourceRadius = getNodeScale(sourceNode, graphData.edges) * (nodeSettings?.nodeSize || 0.5);
              const targetRadius = getNodeScale(targetNode, graphData.edges) * (nodeSettings?.nodeSize || 0.5);
              
              // Offset endpoints to stop before node surfaces (plus small gap)
              const offsetSource = new THREE.Vector3().addVectors(sourcePos, direction.clone().multiplyScalar(sourceRadius + 0.1));
              const offsetTarget = new THREE.Vector3().subVectors(targetPos, direction.clone().multiplyScalar(targetRadius + 0.1));
              
              // Only add edge if there's still visible length after offset
              if (offsetSource.distanceTo(offsetTarget) > 0.2) {
                newEdgePoints.push(offsetSource.x, offsetSource.y, offsetSource.z);
                newEdgePoints.push(offsetTarget.x, offsetTarget.y, offsetTarget.z);
              }
            }
          }
        });
        setEdgePoints(newEdgePoints);
        
        // Update label positions in real-time
        const newLabelPositions = graphData.nodes.map((node, i) => {
          const i3 = i * 3;
          return {
            x: positions[i3],
            y: positions[i3 + 1],
            z: positions[i3 + 2]
          };
        });
        setLabelPositions(newLabelPositions);
      }
    }
    
    // Animate particles - disabled
    // if (particleSystemRef.current) {
    //   particleSystemRef.current.rotation.y = animationStateRef.current.time * 0.05
    //   const positions = particleSystemRef.current.geometry.attributes.position.array as Float32Array
    //   
    //   for (let i = 0; i < positions.length; i += 3) {
    //     positions[i + 1] += Math.sin(animationStateRef.current.time + i) * 0.01
    //   }
    //   
    //   particleSystemRef.current.geometry.attributes.position.needsUpdate = true
    // }
    
    // Pulse selected nodes
    if (meshRef.current && animationStateRef.current.selectedNode !== null) {
      const mesh = meshRef.current
      const nodes = graphData.nodes
      
      nodes.forEach((node, i) => {
        if (node.id === animationStateRef.current.selectedNode) {
          mesh.getMatrixAt(i, tempMatrix)
          tempMatrix.decompose(tempPosition, tempQuaternion, tempScale)
          
          const pulseFactor = 1 + Math.sin(animationStateRef.current.time * 3) * 0.1
          tempScale.multiplyScalar(pulseFactor)
          
          tempMatrix.compose(tempPosition, tempQuaternion, tempScale)
          mesh.setMatrixAt(i, tempMatrix)
          mesh.instanceMatrix.needsUpdate = true
        }
      })
    }
  })
  
  // Graph data subscription (same as original)
  useEffect(() => {
    const handleGraphUpdate = (data: GraphData) => {
      if (debugState.isEnabled()) {
        logger.info('Graph data updated', { nodeCount: data.nodes.length, edgeCount: data.edges.length })
      }
      
      // Ensure nodes have valid positions
      const dataWithPositions = {
        ...data,
        nodes: data.nodes.map((node, i) => {
          if (!node.position || (node.position.x === 0 && node.position.y === 0 && node.position.z === 0)) {
            const position = getPositionForNode(node, i, data.nodes.length)
            return {
              ...node,
              position: { x: position[0], y: position[1], z: position[2] }
            }
          }
          return node
        })
      }
      
      const allAtOrigin = dataWithPositions.nodes.every(node => 
        !node.position || (node.position.x === 0 && node.position.y === 0 && node.position.z === 0)
      )
      setNodesAreAtOrigin(allAtOrigin)
      
      setGraphData(dataWithPositions)
      
      // Update edge points
      const newEdgePoints: number[] = []
      data.edges.forEach((edge) => {
        const sourceNode = data.nodes.find(n => n.id === edge.source)
        const targetNode = data.nodes.find(n => n.id === edge.target)
        
        if (sourceNode?.position && targetNode?.position) {
          newEdgePoints.push(
            sourceNode.position.x, sourceNode.position.y, sourceNode.position.z,
            targetNode.position.x, targetNode.position.y, targetNode.position.z
          )
        }
      })
      
      setEdgePoints(newEdgePoints)
    }
    
    const unsubscribe = graphDataManager.onGraphDataChange(handleGraphUpdate)
    
    // Get initial data and update worker
    graphDataManager.getGraphData().then((data) => {
      handleGraphUpdate(data)
      // Ensure worker has the data
      graphWorkerProxy.setGraphData(data)
    })
    
    return () => {
      unsubscribe()
    }
  }, [])
  
  // Event handlers
  const { handlePointerDown, handlePointerMove, handlePointerUp } = createEventHandlers(
    meshRef,
    dragDataRef,
    setDragState,
    graphData,
    camera,
    size,
    settings,
    setGraphData
  )
  
  // Create ambient particles (disabled for now to avoid white blocks)
  const particleGeometry = useMemo(() => {
    return null // Disable particles that were causing white blocks
  }, [])
  
  // Node labels with dynamic positioning
  const [labelPositions, setLabelPositions] = useState<Array<{x: number, y: number, z: number}>>([])
  
  // Update label positions from physics
  useEffect(() => {
    if (nodePositionsRef.current && graphData.nodes.length > 0) {
      const newPositions = graphData.nodes.map((node, i) => {
        const i3 = i * 3
        return {
          x: nodePositionsRef.current![i3],
          y: nodePositionsRef.current![i3 + 1],
          z: nodePositionsRef.current![i3 + 2]
        }
      })
      setLabelPositions(newPositions)
    }
  }, [nodePositionsRef.current, graphData.nodes])
  
  // Node labels (enhanced version) - using physics positions
  const NodeLabels = useMemo(() => {
    const logseqSettings = settings?.visualisation?.graphs?.logseq;
    const labelSettings = logseqSettings?.labels || settings?.visualisation?.labels;
    if (!labelSettings?.enableLabels || graphData.nodes.length === 0) return null
    
    return graphData.nodes.map((node, index) => {
      // Use physics position if available, otherwise fallback to node position
      const physicsPos = labelPositions[index]
      const position = physicsPos || node.position || { x: 0, y: 0, z: 0 }
      const scale = getNodeScale(node, graphData.edges)
      const labelOffsetY = scale * 1.5 + 0.5 // Stable offset calculation
      
      return (
        <Billboard
          key={`label-${node.id}`}
          position={[position.x, position.y + labelOffsetY, position.z]}
          follow={true}
          lockX={false}
          lockY={false}
          lockZ={false}
        >
          <Text
            fontSize={labelSettings.desktopFontSize || 0.2}
            color={labelSettings.textColor || '#ffffff'}
            anchorX="center"
            anchorY="bottom"
            outlineWidth={labelSettings.textOutlineWidth || 0.02}
            outlineColor={labelSettings.textOutlineColor || '#000000'}
            maxWidth={3}
            textAlign="center"
          >
            {node.label || node.id}
          </Text>
          {(node.metadata?.type || node.metadata?.description) && (
            <Text
              position={[0, -0.15, 0]}
              fontSize={(labelSettings.desktopFontSize || 0.2) * 0.6}
              color={new THREE.Color(labelSettings.textColor || '#ffffff').multiplyScalar(0.6).getStyle()}
              anchorX="center"
              anchorY="top"
              maxWidth={2}
              textAlign="center"
            >
              {node.metadata.description || node.metadata.type}
            </Text>
          )}
        </Billboard>
      )
    })
  }, [graphData.nodes, graphData.edges, labelPositions, settings?.visualisation?.labels])
  
  return (
    <>
      {/* Main node mesh with hologram shader */}
      <instancedMesh
        ref={meshRef}
        args={[undefined, undefined, graphData.nodes.length]}
        frustumCulled={false}
        material={materialRef.current || undefined}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onPointerMissed={() => {
          if (dragDataRef.current.isDragging) {
            handlePointerUp()
          }
        }}
      >
        <sphereGeometry args={[0.5, 32, 32]} />
      </instancedMesh>
      
      {/* Enhanced flowing edges */}
      {edgePoints.length > 0 && (
        <FlowingEdges
          points={edgePoints}
          settings={settings?.visualisation?.graphs?.logseq?.edges || settings?.visualisation?.edges || {}}
          edgeData={graphData.edges}
        />
      )}
      
      {/* Ambient particle system - disabled to prevent white blocks */}
      {particleGeometry && (
        <points ref={particleSystemRef} geometry={particleGeometry}>
          <pointsMaterial
            size={0.1}
            color={settings?.visualisation?.graphs?.logseq?.nodes?.baseColor || settings?.visualisation?.nodes?.baseColor || '#00ffff'}
            transparent
            opacity={0.3}
            vertexColors
            blending={THREE.AdditiveBlending}
            depthWrite={false}
          />
        </points>
      )}
      
      {/* Enhanced node labels */}
      {NodeLabels}
    </>
  )
}

export default GraphManager