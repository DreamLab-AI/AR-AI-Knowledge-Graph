import React, { useRef, useEffect, useMemo } from 'react'
import { useFrame } from '@react-three/fiber'
import * as THREE from 'three'
import { type Node as GraphNode } from '../managers/graphDataManager'

interface MetadataNodesProps {
  nodes: GraphNode[]
  nodePositions: Float32Array | null
  onNodeClick?: (nodeId: string, event: any) => void
  settings: any
}

// More efficient version using a single instanced mesh with vertex attributes
export const MetadataNodesV3: React.FC<MetadataNodesProps> = ({ 
  nodes, 
  nodePositions, 
  onNodeClick,
  settings 
}) => {
  const meshRef = useRef<THREE.InstancedMesh>(null)
  const tempMatrix = useMemo(() => new THREE.Matrix4(), [])
  const tempColor = useMemo(() => new THREE.Color(), [])
  
  // Create a merged geometry that contains all shape types
  const geometry = useMemo(() => {
    // For now, use spheres for all nodes but vary the scale
    return new THREE.SphereGeometry(0.5, 16, 16)
  }, [])
  
  // Create material
  const material = useMemo(() => {
    const logseqSettings = settings?.visualisation?.graphs?.logseq
    const nodeSettings = logseqSettings?.nodes || settings?.visualisation?.nodes
    
    return new THREE.MeshPhongMaterial({
      color: nodeSettings?.baseColor || '#0066ff',
      emissive: nodeSettings?.baseColor || '#0066ff',
      emissiveIntensity: 0.3,
      transparent: true,
      opacity: nodeSettings?.opacity ?? 0.8,
      shininess: 100,
      specular: '#ffffff',
    })
  }, [settings])
  
  // Initialize instance attributes
  useEffect(() => {
    if (!meshRef.current) return
    
    const mesh = meshRef.current
    const logseqSettings = settings?.visualisation?.graphs?.logseq
    const nodeSettings = logseqSettings?.nodes || settings?.visualisation?.nodes
    const baseColor = nodeSettings?.baseColor || '#0066ff'
    
    // Set initial colors
    nodes.forEach((node, i) => {
      tempColor.set(baseColor)
      mesh.setColorAt(i, tempColor)
    })
    
    if (mesh.instanceColor) {
      mesh.instanceColor.needsUpdate = true
    }
  }, [nodes])
  
  // Update positions and properties
  useFrame(() => {
    if (!nodePositions || !meshRef.current) return
    
    const mesh = meshRef.current
    const logseqSettings = settings?.visualisation?.graphs?.logseq
    const nodeSettings = logseqSettings?.nodes || settings?.visualisation?.nodes
    
    nodes.forEach((node, index) => {
      const i3 = index * 3
      const x = nodePositions[i3] || 0
      const y = nodePositions[i3 + 1] || 0
      const z = nodePositions[i3 + 2] || 0
      
      // Calculate scale based on metadata
      let baseScale = 1
      
      // Shape-based scale multipliers
      switch (node.metadata?.type?.toLowerCase()) {
        case 'folder':
          baseScale = 1.2 // Folders are larger
          break
        case 'file':
          baseScale = 1.0
          break
        case 'concept':
          baseScale = 0.8
          break
        case 'todo':
          baseScale = 0.7
          break
        case 'reference':
          baseScale = 0.9
          break
        default:
          baseScale = 1.0
      }
      
      // Size based on file size
      if (node.metadata?.fileSize) {
        const sizeInKB = parseInt(node.metadata.fileSize) / 1024
        const sizeScale = 0.5 + Math.log10(sizeInKB + 1) * 0.3
        baseScale *= Math.min(Math.max(sizeScale, 0.5), 2)
      }
      
      // Additional scale for nodes with many hyperlinks
      if (node.metadata?.hyperlinkCount) {
        baseScale *= (1 + node.metadata.hyperlinkCount * 0.05)
      }
      
      baseScale = Math.min(Math.max(baseScale, 0.3), 3)
      
      // Set transform
      tempMatrix.makeScale(baseScale, baseScale, baseScale)
      tempMatrix.setPosition(x, y, z)
      mesh.setMatrixAt(index, tempMatrix)
      
      // Update color based on age
      if (node.metadata?.lastModified) {
        const now = Date.now()
        const lastModified = new Date(node.metadata.lastModified).getTime()
        const ageInDays = (now - lastModified) / (1000 * 60 * 60 * 24)
        const brightness = Math.max(0.3, 1 - ageInDays / 365)
        tempColor.setHSL(0.55, 1, brightness)
      } else {
        tempColor.set(nodeSettings?.baseColor || '#0066ff')
      }
      
      mesh.setColorAt(index, tempColor)
    })
    
    mesh.instanceMatrix.needsUpdate = true
    if (mesh.instanceColor) {
      mesh.instanceColor.needsUpdate = true
    }
  })
  
  return (
    <instancedMesh
      ref={meshRef}
      args={[geometry, material, nodes.length]}
      frustumCulled={false}
      onClick={(e) => {
        if (e.instanceId !== undefined && onNodeClick) {
          const node = nodes[e.instanceId]
          if (node) {
            onNodeClick(node.id, e)
          }
        }
      }}
    />
  )
}