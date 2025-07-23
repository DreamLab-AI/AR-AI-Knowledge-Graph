import React, { useRef, useMemo } from 'react'
import { useFrame } from '@react-three/fiber'
import * as THREE from 'three'
import { type Node as GraphNode } from '../managers/graphDataManager'

interface SatelliteSystemProps {
  node: GraphNode
  position: [number, number, number]
  scale: number
}

export const SatelliteSystem: React.FC<SatelliteSystemProps> = ({ node, position, scale }) => {
  const groupRef = useRef<THREE.Group>(null)
  
  // Calculate satellite count based on hyperlink count
  const satelliteCount = node.metadata?.hyperlinkCount || 0
  
  // Create satellite positions
  const satellitePositions = useMemo(() => {
    const positions: [number, number, number][] = []
    if (satelliteCount === 0) return positions
    
    // Use golden angle for even distribution
    const goldenAngle = Math.PI * (3 - Math.sqrt(5))
    
    for (let i = 0; i < satelliteCount; i++) {
      const theta = i * goldenAngle
      const y = 1 - (i / satelliteCount) * 2
      const radius = Math.sqrt(1 - y * y)
      
      const orbitRadius = (1.5 + scale * 0.5) * (1 + i * 0.1)
      const x = Math.cos(theta) * radius * orbitRadius
      const z = Math.sin(theta) * radius * orbitRadius
      const yPos = y * orbitRadius * 0.5
      
      positions.push([x, yPos, z])
    }
    
    return positions
  }, [satelliteCount, scale])
  
  // Animate satellites
  useFrame((state) => {
    if (groupRef.current && satelliteCount > 0) {
      groupRef.current.rotation.y = state.clock.elapsedTime * 0.2
      
      // Add vertical oscillation
      groupRef.current.children.forEach((child, i) => {
        if (child instanceof THREE.Mesh) {
          const offset = i * Math.PI * 2 / satelliteCount
          child.position.y = satellitePositions[i][1] + Math.sin(state.clock.elapsedTime + offset) * 0.1
        }
      })
    }
  })
  
  if (satelliteCount === 0) return null
  
  return (
    <group ref={groupRef} position={position}>
      {satellitePositions.map((pos, i) => (
        <mesh key={i} position={pos}>
          <sphereGeometry args={[0.05 + i * 0.01, 8, 8]} />
          <meshBasicMaterial 
            color="#00ffff" 
            transparent 
            opacity={0.8 - i * 0.05}
            emissive="#00ffff"
            emissiveIntensity={2}
          />
        </mesh>
      ))}
      
      {/* Connection lines */}
      {satellitePositions.map((pos, i) => (
        <line key={`line-${i}`}>
          <bufferGeometry>
            <bufferAttribute
              attach="attributes-position"
              count={2}
              array={new Float32Array([0, 0, 0, ...pos])}
              itemSize={3}
            />
          </bufferGeometry>
          <lineBasicMaterial 
            color="#00ffff" 
            transparent 
            opacity={0.3}
            linewidth={1}
          />
        </line>
      ))}
    </group>
  )
}