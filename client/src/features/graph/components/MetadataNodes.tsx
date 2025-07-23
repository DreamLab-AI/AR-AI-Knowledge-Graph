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

// Get geometry for node type
const getGeometryForNodeType = (type?: string): THREE.BufferGeometry => {
  switch (type?.toLowerCase()) {
    case 'folder':
      return new THREE.OctahedronGeometry(0.6, 0) // Folder = octahedron
    case 'file':
      return new THREE.BoxGeometry(0.8, 0.8, 0.8) // File = cube
    case 'concept':
      return new THREE.IcosahedronGeometry(0.5, 0) // Concept = icosahedron
    case 'todo':
      return new THREE.ConeGeometry(0.5, 1, 4) // Todo = pyramid
    case 'reference':
      return new THREE.TorusGeometry(0.5, 0.2, 8, 16) // Reference = torus
    default:
      return new THREE.SphereGeometry(0.5, 32, 32) // Default = sphere
  }
}

// Group nodes by their geometry type
const groupNodesByGeometry = (nodes: GraphNode[]) => {
  const groups = new Map<string, GraphNode[]>()
  
  nodes.forEach(node => {
    const type = node.metadata?.type?.toLowerCase() || 'default'
    if (!groups.has(type)) {
      groups.set(type, [])
    }
    groups.get(type)!.push(node)
  })
  
  return groups
}

export const MetadataNodes: React.FC<MetadataNodesProps> = ({ 
  nodes, 
  nodePositions, 
  onNodeClick,
  settings 
}) => {
  const materialRef = useRef<HologramNodeMaterial | null>(null)
  const meshRefs = useRef<Map<string, THREE.InstancedMesh>>(new Map())
  
  // Create hologram material
  useEffect(() => {
    if (!materialRef.current) {
      const logseqSettings = settings?.visualisation?.graphs?.logseq
      const nodeSettings = logseqSettings?.nodes || settings?.visualisation?.nodes
      
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
      
      materialRef.current.defines = { ...materialRef.current.defines, USE_INSTANCING_COLOR: '' }
      materialRef.current.needsUpdate = true
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
    }
  }, [settings?.visualisation])
  
  // Group nodes and create instanced meshes
  const nodeGroups = useMemo(() => groupNodesByGeometry(nodes), [nodes])
  
  // Track node positions for satellites
  const [nodeWorldPositions, setNodeWorldPositions] = useState<Map<string, [number, number, number]>>(new Map())
  
  // Update instance matrices and colors
  useFrame(() => {
    if (!nodePositions) return
    
    const tempMatrix = new THREE.Matrix4()
    const tempColor = new THREE.Color()
    const newPositions = new Map<string, [number, number, number]>()
    
    // Get node settings for colors
    const logseqSettings = settings?.visualisation?.graphs?.logseq
    const nodeSettings = logseqSettings?.nodes || settings?.visualisation?.nodes
    
    nodeGroups.forEach((groupNodes, type) => {
      const mesh = meshRefs.current.get(type)
      if (!mesh) return
      
      groupNodes.forEach((node, localIndex) => {
        // Find the original index of this node in the nodes array
        const originalIndex = nodes.findIndex(n => n.id === node.id)
        if (originalIndex === -1) return
        
        const i3 = originalIndex * 3
        const x = nodePositions[i3] || 0
        const y = nodePositions[i3 + 1] || 0
        const z = nodePositions[i3 + 2] || 0
        
        // Store position for satellites
        newPositions.set(node.id, [x, y, z])
        
        // Position
        tempMatrix.setPosition(x, y, z)
        
        // Scale based on file size or hyperlink count
        let scale = 1
        if (node.metadata?.fileSize) {
          // Scale based on file size (log scale)
          const sizeInKB = parseInt(node.metadata.fileSize) / 1024
          scale = 0.5 + Math.log10(sizeInKB + 1) * 0.3
        } else if (node.metadata?.hyperlinkCount) {
          // Scale based on hyperlink count
          scale = 1 + node.metadata.hyperlinkCount * 0.1
        }
        scale = Math.min(Math.max(scale, 0.5), 3) // Clamp between 0.5 and 3
        
        tempMatrix.scale(new THREE.Vector3(scale, scale, scale))
        mesh.setMatrixAt(localIndex, tempMatrix)
        
        // Color based on age (lastModified)
        if (node.metadata?.lastModified) {
          const now = Date.now()
          const lastModified = new Date(node.metadata.lastModified).getTime()
          const ageInDays = (now - lastModified) / (1000 * 60 * 60 * 24)
          
          // Fresh = bright, old = dim
          const brightness = Math.max(0.3, 1 - ageInDays / 365)
          tempColor.setHSL(0.55, 1, brightness) // Cyan hue with variable brightness
        } else {
          tempColor.set(nodeSettings?.baseColor || '#00ffff')
        }
        
        mesh.setColorAt(localIndex, tempColor)
      })
      
      mesh.instanceMatrix.needsUpdate = true
      if (mesh.instanceColor) {
        mesh.instanceColor.needsUpdate = true
      }
    })
    
    // Update node positions for satellites
    setNodeWorldPositions(newPositions)
  })
  
  return (
    <>
      {Array.from(nodeGroups.entries()).map(([type, groupNodes]) => (
        <instancedMesh
          key={type}
          ref={(ref) => {
            if (ref) meshRefs.current.set(type, ref)
          }}
          args={[getGeometryForNodeType(type), materialRef.current || undefined, groupNodes.length]}
          frustumCulled={false}
          onClick={(e) => {
            const instanceId = e.instanceId
            if (instanceId !== undefined && onNodeClick) {
              const node = groupNodes[instanceId]
              if (node) {
                onNodeClick(node.id, e)
              }
            }
          }}
        />
      ))}
      
      {/* Render satellite systems for nodes with hyperlinks */}
      {nodes.filter(node => node.metadata?.hyperlinkCount && node.metadata.hyperlinkCount > 0).map(node => {
        const position = nodeWorldPositions.get(node.id)
        if (!position) return null
        
        // Calculate scale based on file size
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
    </>
  )
}