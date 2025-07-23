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
  
  // Refs for satellite groups
  const satelliteGroupRefs = useRef<THREE.Group[]>([])
  
  // Animate satellites
  useFrame((state) => {
    if (groupRef.current && satelliteCount > 0) {
      groupRef.current.rotation.y = state.clock.elapsedTime * 0.2
      
      // Add vertical oscillation to satellite groups
      satelliteGroupRefs.current.forEach((group, i) => {
        if (group && satellitePositions[i]) {
          const offset = i * Math.PI * 2 / satelliteCount
          const baseY = satellitePositions[i][1]
          group.position.y = baseY + Math.sin(state.clock.elapsedTime + offset) * 0.1
        }
      })
    }
  })
  
  if (satelliteCount === 0) return null
  
  return (
    <group ref={groupRef} position={position}>
      {satellitePositions.map((pos, i) => (
        <group 
          key={i} 
          position={pos}
          ref={(el) => {
            if (el) satelliteGroupRefs.current[i] = el
          }}
        >
          {/* Satellite sphere */}
          <mesh>
            <sphereGeometry args={[0.05 + i * 0.01, 8, 8]} />
            <meshPhongMaterial 
              color="#00ffff" 
              transparent 
              opacity={0.8 - i * 0.05}
              emissive="#00ffff"
              emissiveIntensity={2}
            />
          </mesh>
          
          {/* Glowing halo */}
          <mesh>
            <sphereGeometry args={[0.08 + i * 0.01, 8, 8]} />
            <meshBasicMaterial 
              color="#00ffff" 
              transparent 
              opacity={0.2}
              side={THREE.BackSide}
              blending={THREE.AdditiveBlending}
            />
          </mesh>
        </group>
      ))}
      
      {/* Connection beams with gradient */}
      {satellitePositions.map((pos, i) => {
        const points = [
          new THREE.Vector3(0, 0, 0),
          new THREE.Vector3(...pos)
        ]
        const geometry = new THREE.BufferGeometry().setFromPoints(points)
        
        return (
          <line key={`line-${i}`} geometry={geometry}>
            <lineBasicMaterial 
              color="#00ffff" 
              transparent 
              opacity={0.4 - i * 0.02}
              blending={THREE.AdditiveBlending}
            />
          </line>
        )
      })}
      
      {/* Orbital ring */}
      <mesh rotation={[Math.PI / 2, 0, 0]}>
        <torusGeometry args={[1.5 + scale * 0.5, 0.01, 8, 32]} />
        <meshBasicMaterial 
          color="#00ffff" 
          transparent 
          opacity={0.15}
          blending={THREE.AdditiveBlending}
        />
      </mesh>
    </group>
  )
}