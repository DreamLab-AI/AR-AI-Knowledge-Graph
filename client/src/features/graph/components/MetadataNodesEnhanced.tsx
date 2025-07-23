import React, { useRef, useEffect, useMemo, useState } from 'react'
import { useFrame } from '@react-three/fiber'
import * as THREE from 'three'
import { type Node as GraphNode } from '../managers/graphDataManager'
import { HologramNodeMaterial } from '../shaders/HologramNodeMaterial'
import { SatelliteSystem } from './SatelliteSystem'

interface MetadataNodesProps {
  nodes: GraphNode[]
  nodePositions: Float32Array | null
  onNodeClick?: (nodeId: string, event: any) => void
  settings: any
}

// Group nodes by geometry type for efficient rendering
const groupNodesByType = (nodes: GraphNode[]) => {
  const groups = new Map<string, { nodes: GraphNode[], indices: number[] }>()
  
  nodes.forEach((node, index) => {
    const type = node.metadata?.type?.toLowerCase() || 'default'
    if (!groups.has(type)) {
      groups.set(type, { nodes: [], indices: [] })
    }
    const group = groups.get(type)!
    group.nodes.push(node)
    group.indices.push(index)
  })
  
  return groups
}

// Geometry factory with optimized shapes
const createGeometryForType = (type: string): THREE.BufferGeometry => {
  switch (type) {
    case 'folder':
      return new THREE.OctahedronGeometry(0.6, 0)
    case 'file':
      return new THREE.BoxGeometry(0.8, 0.8, 0.8)
    case 'concept':
      return new THREE.IcosahedronGeometry(0.5, 1)
    case 'todo':
      return new THREE.ConeGeometry(0.5, 1, 4)
    case 'reference':
      return new THREE.TorusGeometry(0.5, 0.2, 8, 16)
    default:
      return new THREE.SphereGeometry(0.5, 32, 32)
  }
}

export const MetadataNodesEnhanced: React.FC<MetadataNodesProps> = ({ 
  nodes, 
  nodePositions, 
  onNodeClick,
  settings 
}) => {
  const groupRefs = useRef<Map<string, THREE.InstancedMesh>>(new Map())
  const materialRef = useRef<HologramNodeMaterial | null>(null)
  const [nodeWorldPositions, setNodeWorldPositions] = useState<Map<string, [number, number, number]>>(new Map())
  
  // Create hologram material with proper initialization
  useEffect(() => {
    const logseqSettings = settings?.visualisation?.graphs?.logseq
    const nodeSettings = logseqSettings?.nodes || settings?.visualisation?.nodes
    
    // Create base material
    materialRef.current = new HologramNodeMaterial({
      baseColor: nodeSettings?.baseColor || '#0066ff',
      emissiveColor: nodeSettings?.baseColor || '#00ffff',
      opacity: nodeSettings?.opacity ?? 0.8,
      enableHologram: nodeSettings?.enableHologram !== false,
      glowStrength: 3.0,
      pulseSpeed: 1.0,
      hologramStrength: 0.8,
      rimPower: 3.0,
    })
    
    // Enable instancing
    materialRef.current.defines = { 
      ...materialRef.current.defines, 
      USE_INSTANCING: '',
      USE_INSTANCING_COLOR: '' 
    }
    materialRef.current.needsUpdate = true
    
    return () => {
      materialRef.current?.dispose()
    }
  }, [])
  
  // Update material settings
  useEffect(() => {
    if (materialRef.current && settings?.visualisation) {
      const logseqSettings = settings.visualisation.graphs?.logseq
      const nodeSettings = logseqSettings?.nodes || settings.visualisation.nodes
      
      materialRef.current.updateColors(
        nodeSettings?.baseColor || '#00ffff',
        nodeSettings?.baseColor || '#00ffff'
      )
      materialRef.current.uniforms.opacity.value = nodeSettings?.opacity ?? 0.8
      materialRef.current.setHologramEnabled(nodeSettings?.enableHologram !== false)
      
      // Update animation settings
      materialRef.current.updateHologramParams({
        glowStrength: settings.visualisation.animations?.pulseStrength || 1.0,
        pulseSpeed: settings.visualisation.animations?.pulseSpeed || 1.0,
      })
    }
  }, [settings?.visualisation])
  
  // Group nodes by type
  const nodeGroups = useMemo(() => groupNodesByType(nodes), [nodes])
  
  // Create geometries for each type
  const geometries = useMemo(() => {
    const geos = new Map<string, THREE.BufferGeometry>()
    const types = ['folder', 'file', 'concept', 'todo', 'reference', 'default']
    
    types.forEach(type => {
      geos.set(type, createGeometryForType(type))
    })
    
    return geos
  }, [])
  
  // Update animation time
  useFrame((state) => {
    if (materialRef.current) {
      materialRef.current.updateTime(state.clock.elapsedTime)
    }
  })
  
  // Update positions and properties
  useFrame(() => {
    if (!nodePositions) return
    
    const tempMatrix = new THREE.Matrix4()
    const tempColor = new THREE.Color()
    const newPositions = new Map<string, [number, number, number]>()
    
    const logseqSettings = settings?.visualisation?.graphs?.logseq
    const nodeSettings = logseqSettings?.nodes || settings?.visualisation?.nodes
    const baseColor = nodeSettings?.baseColor || '#00ffff'
    
    nodeGroups.forEach((group, type) => {
      const mesh = groupRefs.current.get(type)
      if (!mesh) return
      
      group.nodes.forEach((node, localIndex) => {
        const globalIndex = group.indices[localIndex]
        const i3 = globalIndex * 3
        const x = nodePositions[i3] || 0
        const y = nodePositions[i3 + 1] || 0
        const z = nodePositions[i3 + 2] || 0
        
        // Store position for satellites
        newPositions.set(node.id, [x, y, z])
        
        // Calculate scale based on metadata
        let scale = 1
        
        // File size scaling
        if (node.metadata?.fileSize) {
          const sizeInKB = parseInt(node.metadata.fileSize) / 1024
          scale = 0.5 + Math.log10(sizeInKB + 1) * 0.3
        }
        
        // Hyperlink count adds to scale
        if (node.metadata?.hyperlinkCount) {
          scale *= (1 + node.metadata.hyperlinkCount * 0.1)
        }
        
        scale = Math.min(Math.max(scale, 0.5), 3)
        
        // Apply transform
        tempMatrix.makeScale(scale, scale, scale)
        tempMatrix.setPosition(x, y, z)
        mesh.setMatrixAt(localIndex, tempMatrix)
        
        // Color based on age with energy effect
        if (node.metadata?.lastModified) {
          const now = Date.now()
          const lastModified = new Date(node.metadata.lastModified).getTime()
          const ageInDays = (now - lastModified) / (1000 * 60 * 60 * 24)
          
          // Recent files have higher energy (brighter, more saturated)
          const energy = Math.max(0.2, 1 - ageInDays / 90) // 90 days falloff
          const hue = 0.55 - (energy * 0.1) // Shift from cyan to blue-green for fresh files
          const saturation = 0.5 + (energy * 0.5)
          const lightness = 0.3 + (energy * 0.4)
          
          tempColor.setHSL(hue, saturation, lightness)
        } else {
          tempColor.set(baseColor)
        }
        
        mesh.setColorAt(localIndex, tempColor)
      })
      
      mesh.instanceMatrix.needsUpdate = true
      if (mesh.instanceColor) {
        mesh.instanceColor.needsUpdate = true
      }
    })
    
    setNodeWorldPositions(newPositions)
  })
  
  return (
    <>
      {/* Render instanced meshes for each geometry type */}
      {Array.from(nodeGroups.entries()).map(([type, group]) => {
        const geometry = geometries.get(type) || geometries.get('default')!
        
        return (
          <instancedMesh
            key={type}
            ref={(ref) => {
              if (ref) groupRefs.current.set(type, ref)
            }}
            args={[geometry, materialRef.current || undefined, group.nodes.length]}
            frustumCulled={false}
            onClick={(e) => {
              if (e.instanceId !== undefined && onNodeClick) {
                const node = group.nodes[e.instanceId]
                if (node) {
                  onNodeClick(node.id, e)
                }
              }
            }}
          />
        )
      })}
      
      {/* Satellite systems for nodes with hyperlinks */}
      {nodes.filter(n => n.metadata?.hyperlinkCount && n.metadata.hyperlinkCount > 0).map(node => {
        const position = nodeWorldPositions.get(node.id)
        if (!position) return null
        
        // Calculate scale
        let scale = 1
        if (node.metadata?.fileSize) {
          const sizeInKB = parseInt(node.metadata.fileSize) / 1024
          scale = 0.5 + Math.log10(sizeInKB + 1) * 0.3
          scale = Math.min(Math.max(scale, 0.5), 3)
        }
        
        return (
          <SatelliteSystem
            key={`satellites-${node.id}`}
            node={node}
            position={position}
            scale={scale}
          />
        )
      })}
      
      {/* Energy rings for recently modified nodes */}
      {nodes.filter(n => {
        if (!n.metadata?.lastModified) return false
        const ageInDays = (Date.now() - new Date(n.metadata.lastModified).getTime()) / (1000 * 60 * 60 * 24)
        return ageInDays < 7 // Show energy rings for files modified in last week
      }).map(node => {
        const position = nodeWorldPositions.get(node.id)
        if (!position) return null
        
        return (
          <EnergyRing
            key={`energy-${node.id}`}
            position={position}
            age={(Date.now() - new Date(node.metadata!.lastModified!).getTime()) / (1000 * 60 * 60 * 24)}
          />
        )
      })}
    </>
  )
}

// Energy ring component for recently modified nodes
const EnergyRing: React.FC<{ position: [number, number, number], age: number }> = ({ position, age }) => {
  const meshRef = useRef<THREE.Mesh>(null)
  
  useFrame((state) => {
    if (!meshRef.current) return
    
    // Pulse based on age
    const intensity = 1 - (age / 7) // 7 days falloff
    const scale = 1.5 + Math.sin(state.clock.elapsedTime * 3) * 0.2 * intensity
    meshRef.current.scale.set(scale, scale, scale)
    
    // Rotate
    meshRef.current.rotation.z = state.clock.elapsedTime * 2
    
    // Fade opacity
    if (meshRef.current.material instanceof THREE.Material) {
      (meshRef.current.material as any).opacity = 0.3 * intensity
    }
  })
  
  return (
    <mesh ref={meshRef} position={position}>
      <ringGeometry args={[0.8, 1, 32]} />
      <meshBasicMaterial 
        color="#00ffff" 
        transparent 
        opacity={0.3}
        side={THREE.DoubleSide}
        blending={THREE.AdditiveBlending}
      />
    </mesh>
  )
}