import React, { useRef, useEffect, useMemo } from 'react'
import { Canvas, useFrame, useThree } from '@react-three/fiber'
import { XR, Controllers, Hands, useXR } from '@react-three/xr'
import { Environment, OrbitControls, Instance, Instances, Box, Plane } from '@react-three/drei'
import * as THREE from 'three'
import GraphManager from '../../graph/components/GraphManager'
import { useSettingsStore } from '../../../store/settingsStore'
import { useMultiUserStore } from '../../../store/multiUserStore'
import { createLogger } from '../../../utils/logger'

const logger = createLogger('XRScene')

// Room-scale AR scene setup component
const ARSceneSetup = () => {
  const { gl, scene } = useThree()
  const { isPresenting, session } = useXR()
  
  useEffect(() => {
    if (!isPresenting || !session) return
    
    // Configure for Quest 3 passthrough
    gl.xr.enabled = true
    gl.xr.setReferenceSpaceType('local-floor')
    gl.setClearColor(0x000000, 0) // Transparent for AR
    gl.outputColorSpace = THREE.SRGBColorSpace
    
    // Enable fixed foveated rendering for Quest 3
    if (gl.xr.setFoveation) {
      gl.xr.setFoveation(1.0)
    }
    
    logger.info('AR scene configured for Quest 3 passthrough')
  }, [isPresenting, session, gl])
  
  return null
}

// Shadow plane for AR grounding
const ARShadowPlane = () => {
  return (
    <Plane
      args={[10, 10]}
      rotation={[-Math.PI / 2, 0, 0]}
      position={[0, 0, 0]}
      receiveShadow
    >
      <shadowMaterial opacity={0.3} />
    </Plane>
  )
}

// Multi-user avatar component
const UserAvatar = ({ userId, position, color, isSelecting }: {
  userId: string
  position: [number, number, number]
  color: string
  isSelecting: boolean
}) => {
  const meshRef = useRef<THREE.Mesh>(null)
  
  useFrame((state) => {
    if (meshRef.current && isSelecting) {
      // Pulse effect when selecting
      const scale = 1 + Math.sin(state.clock.elapsedTime * 5) * 0.1
      meshRef.current.scale.setScalar(scale)
    }
  })
  
  return (
    <group position={position}>
      {/* Avatar head */}
      <mesh ref={meshRef}>
        <sphereGeometry args={[0.1, 16, 16]} />
        <meshPhysicalMaterial
          color={color}
          emissive={color}
          emissiveIntensity={isSelecting ? 0.5 : 0.2}
          metalness={0.8}
          roughness={0.2}
        />
      </mesh>
      {/* Selection indicator */}
      {isSelecting && (
        <mesh position={[0, 0.2, 0]}>
          <coneGeometry args={[0.05, 0.1, 8]} />
          <meshBasicMaterial color={color} />
        </mesh>
      )}
    </group>
  )
}

// Multi-user visualization component
const MultiUserVisualization = () => {
  const users = useMultiUserStore(state => state.users)
  const localUserId = useMultiUserStore(state => state.localUserId)
  
  const userColors = useMemo(() => {
    const colors = ['#FF6B6B', '#4ECDC4', '#45B7D1', '#96CEB4', '#FECA57']
    return Object.keys(users).reduce((acc, userId, index) => {
      acc[userId] = colors[index % colors.length]
      return acc
    }, {} as Record<string, string>)
  }, [users])
  
  return (
    <>
      {Object.entries(users).map(([userId, userData]) => {
        if (userId === localUserId) return null // Don't render local user avatar
        
        return (
          <UserAvatar
            key={userId}
            userId={userId}
            position={userData.position || [0, 1.6, 0]}
            color={userColors[userId]}
            isSelecting={userData.isSelecting || false}
          />
        )
      })}
    </>
  )
}

// Room-scale graph container
const RoomScaleGraph = () => {
  const containerRef = useRef<THREE.Group>(null)
  const settings = useSettingsStore(state => state.settings)
  const graphScale = settings?.xr?.graphScale || 0.8
  
  useEffect(() => {
    if (containerRef.current) {
      // Position graph at comfortable viewing distance
      containerRef.current.position.set(0, 1.2, -1)
      containerRef.current.scale.setScalar(graphScale)
    }
  }, [graphScale])
  
  return (
    <group ref={containerRef}>
      <GraphManager />
    </group>
  )
}

// Performance-optimized node instances for large graphs
const OptimizedNodeInstances = ({ nodeCount = 1000 }: { nodeCount?: number }) => {
  const meshRef = useRef<THREE.InstancedMesh>(null)
  
  useEffect(() => {
    if (!meshRef.current) return
    
    const dummy = new THREE.Object3D()
    const color = new THREE.Color()
    
    // Initialize node positions and colors
    for (let i = 0; i < nodeCount; i++) {
      dummy.position.set(
        (Math.random() - 0.5) * 4,
        Math.random() * 2,
        (Math.random() - 0.5) * 4
      )
      dummy.scale.setScalar(0.05 + Math.random() * 0.05)
      dummy.updateMatrix()
      meshRef.current.setMatrixAt(i, dummy.matrix)
      
      color.setHSL(Math.random(), 0.7, 0.5)
      meshRef.current.setColorAt(i, color)
    }
    
    meshRef.current.instanceMatrix.needsUpdate = true
    if (meshRef.current.instanceColor) {
      meshRef.current.instanceColor.needsUpdate = true
    }
  }, [nodeCount])
  
  return (
    <instancedMesh ref={meshRef} args={[undefined, undefined, nodeCount]}>
      <sphereGeometry args={[1, 8, 6]} />
      <meshPhysicalMaterial
        metalness={0.8}
        roughness={0.2}
        envMapIntensity={0.5}
      />
    </instancedMesh>
  )
}

export const XRScene = () => {
  const settings = useSettingsStore(state => state.settings)
  const xrSettings = settings?.xr
  const enableMultiUser = settings?.xr?.enableMultiUser !== false
  const enableOptimizedNodes = settings?.xr?.enableOptimizedNodes !== false

  return (
    <Canvas
      camera={{ position: [0, 1.6, 3], fov: 50 }}
      shadows
      gl={{
        antialias: true,
        alpha: true,
        powerPreference: 'high-performance',
        preserveDrawingBuffer: true
      }}
    >
      <XR>
        <ARSceneSetup />
        <Controllers />
        <Hands />
        
        {/* AR-optimized lighting */}
        <ambientLight intensity={0.5} />
        <directionalLight
          position={[10, 10, 5]}
          intensity={1}
          castShadow
          shadow-mapSize={[2048, 2048]}
          shadow-bias={-0.0001}
        />
        
        {/* AR shadow plane for grounding */}
        <ARShadowPlane />
        
        {/* Environment for reflections */}
        <Environment preset="sunset" />
        
        {/* Room-scale graph visualization */}
        <RoomScaleGraph />
        
        {/* Performance-optimized nodes for large graphs */}
        {enableOptimizedNodes && <OptimizedNodeInstances />}
        
        {/* Multi-user avatars and indicators */}
        {enableMultiUser && <MultiUserVisualization />}
        
        {/* Optional orbit controls for non-AR mode */}
        {!xrSettings?.enabled && (
          <OrbitControls
            enablePan={true}
            enableZoom={true}
            enableRotate={true}
            target={[0, 0, 0]}
          />
        )}
      </XR>
    </Canvas>
  )
} 