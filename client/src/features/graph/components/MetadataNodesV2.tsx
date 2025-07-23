import React, { useRef, useEffect } from 'react'
import { useFrame } from '@react-three/fiber'
import * as THREE from 'three'
import { type Node as GraphNode } from '../managers/graphDataManager'
import { SatelliteSystem } from './SatelliteSystem'

interface MetadataNodesProps {
  nodes: GraphNode[]
  nodePositions: Float32Array | null
  onNodeClick?: (nodeId: string, event: any) => void
  settings: any
}

// Since we can't use different geometries in a single InstancedMesh,
// we'll render individual meshes for each node to maintain indexing
export const MetadataNodesV2: React.FC<MetadataNodesProps> = ({ 
  nodes, 
  nodePositions, 
  onNodeClick,
  settings 
}) => {
  const meshGroupRef = useRef<THREE.Group>(null)
  const meshRefs = useRef<Map<string, THREE.Mesh>>(new Map())
  const materialsRef = useRef<Map<string, THREE.Material>>(new Map())
  
  // Create materials for each node type to avoid cloning issues
  const createMaterialForType = (type?: string, nodeSettings?: any) => {
    const baseColor = nodeSettings?.baseColor || '#0066ff'
    
    // Use standard material for now to avoid shader issues
    const material = new THREE.MeshPhongMaterial({
      color: baseColor,
      emissive: baseColor,
      emissiveIntensity: 0.3,
      transparent: true,
      opacity: nodeSettings?.opacity ?? 0.8,
      shininess: 100,
      specular: '#ffffff',
    })
    
    return material
  }
  
  // Initialize materials
  useEffect(() => {
    const logseqSettings = settings?.visualisation?.graphs?.logseq
    const nodeSettings = logseqSettings?.nodes || settings?.visualisation?.nodes
    
    // Create shared materials for each node type
    const types = ['folder', 'file', 'concept', 'todo', 'reference', 'default']
    types.forEach(type => {
      if (!materialsRef.current.has(type)) {
        const material = createMaterialForType(type, nodeSettings)
        materialsRef.current.set(type, material)
      }
    })
    
    return () => {
      // Cleanup materials
      materialsRef.current.forEach(material => material.dispose())
      materialsRef.current.clear()
    }
  }, [])
  
  // Get geometry for node type
  const getGeometryForNodeType = (type?: string): THREE.BufferGeometry => {
    switch (type?.toLowerCase()) {
      case 'folder':
        return new THREE.OctahedronGeometry(0.6, 0)
      case 'file':
        return new THREE.BoxGeometry(0.8, 0.8, 0.8)
      case 'concept':
        return new THREE.IcosahedronGeometry(0.5, 0)
      case 'todo':
        return new THREE.ConeGeometry(0.5, 1, 4)
      case 'reference':
        return new THREE.TorusGeometry(0.5, 0.2, 8, 16)
      default:
        return new THREE.SphereGeometry(0.5, 32, 32)
    }
  }
  
  // Update positions and properties
  useFrame(() => {
    if (!nodePositions || !meshGroupRef.current) return
    
    const logseqSettings = settings?.visualisation?.graphs?.logseq
    const nodeSettings = logseqSettings?.nodes || settings?.visualisation?.nodes
    const defaultColor = nodeSettings?.baseColor || '#00ffff'
    
    nodes.forEach((node, index) => {
      const mesh = meshRefs.current.get(node.id)
      if (!mesh) return
      
      const i3 = index * 3
      const x = nodePositions[i3] || 0
      const y = nodePositions[i3 + 1] || 0
      const z = nodePositions[i3 + 2] || 0
      
      // Update position
      mesh.position.set(x, y, z)
      
      // Scale based on file size or hyperlink count
      let scale = 1
      if (node.metadata?.fileSize) {
        const sizeInKB = parseInt(node.metadata.fileSize) / 1024
        scale = 0.5 + Math.log10(sizeInKB + 1) * 0.3
      } else if (node.metadata?.hyperlinkCount) {
        scale = 1 + node.metadata.hyperlinkCount * 0.1
      }
      scale = Math.min(Math.max(scale, 0.5), 3)
      mesh.scale.setScalar(scale)
      
      // Color based on age
      if (mesh.material instanceof THREE.Material && 'color' in mesh.material) {
        if (node.metadata?.lastModified) {
          const now = Date.now()
          const lastModified = new Date(node.metadata.lastModified).getTime()
          const ageInDays = (now - lastModified) / (1000 * 60 * 60 * 24)
          const brightness = Math.max(0.3, 1 - ageInDays / 365)
          ;(mesh.material as any).color.setHSL(0.55, 1, brightness)
        } else {
          ;(mesh.material as any).color.set(defaultColor)
        }
      }
    })
  })
  
  return (
    <>
      <group ref={meshGroupRef}>
        {nodes.map((node, index) => {
          const nodeType = node.metadata?.type?.toLowerCase() || 'default'
          const materialKey = ['folder', 'file', 'concept', 'todo', 'reference'].includes(nodeType) ? nodeType : 'default'
          const material = materialsRef.current.get(materialKey)
          
          return (
            <mesh
              key={node.id}
              ref={(ref) => {
                if (ref) meshRefs.current.set(node.id, ref)
              }}
              geometry={getGeometryForNodeType(node.metadata?.type)}
              material={material}
              onClick={(e) => {
                if (onNodeClick) {
                  onNodeClick(node.id, e)
                }
              }}
            />
          )
        })}
      </group>
      
      {/* Render satellite systems */}
      {nodes.filter(n => n.metadata?.hyperlinkCount && n.metadata.hyperlinkCount > 0).map((node, index) => {
        const mesh = meshRefs.current.get(node.id)
        if (!mesh) return null
        
        const pos = mesh.position
        const scale = mesh.scale.x
        
        return (
          <SatelliteSystem
            key={`satellites-${node.id}`}
            node={node}
            position={[pos.x, pos.y, pos.z]}
            scale={scale}
          />
        )
      })}
    </>
  )
}