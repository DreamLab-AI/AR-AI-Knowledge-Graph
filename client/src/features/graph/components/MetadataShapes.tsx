// client/src/features/graph/components/MetadataShapes.tsx
import React, { useRef, useMemo } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { type Node as GraphNode } from '../managers/graphDataManager';
import { HologramNodeMaterial } from '../shaders/HologramNodeMaterial';

// --- 1. Define Visual Metaphor Logic ---
// This helper function is the heart of our new system.
const getVisualsForNode = (node: GraphNode) => {
  const visuals = {
    geometryType: 'sphere' as 'sphere' | 'box' | 'octahedron' | 'icosahedron',
    scale: 1.0,
    color: new THREE.Color('#00ffff'), // Default cyan
    emissive: new THREE.Color('#00ffff'),
    pulseSpeed: 0.5,
  };

  const { metadata } = node;
  if (!metadata) return visuals;
  
  // Debug: Log first few nodes to see what metadata we have
  if (Math.random() < 0.05) { // Log 5% of nodes to see variation
    console.log('MetadataShapes: Node metadata sample:', {
      id: node.id,
      label: node.label,
      metadata: metadata,
      hasLastModified: !!metadata.lastModified,
      lastModified: metadata.lastModified,
      fileSize: metadata.fileSize,
      hyperlinkCount: metadata.hyperlinkCount
    });
  }

  // METAPHOR 1: Geometry from Connectivity (hyperlinkCount)
  const hyperlinkCount = parseInt(metadata.hyperlinkCount || '0', 10);
  if (hyperlinkCount > 7) {
    visuals.geometryType = 'icosahedron'; // Highly connected: Complex shape
  } else if (hyperlinkCount > 3) {
    visuals.geometryType = 'octahedron'; // Well-connected: Interesting shape
  } else if (hyperlinkCount > 0) {
    visuals.geometryType = 'box'; // Some connections: Simple, distinct shape
  } else {
    visuals.geometryType = 'sphere'; // No connections: Base shape
  }

  // METAPHOR 2: Scale from Connectivity & File Size
  const fileSize = parseInt(metadata.fileSize || '0', 10);
  const sizeScale = 0.8 + Math.log10(Math.max(1, fileSize / 1024)) * 0.2; // Log scale for size
  const connectionScale = 1 + hyperlinkCount * 0.05;
  visuals.scale = THREE.MathUtils.clamp(sizeScale * connectionScale, 0.5, 3.0);

  // METAPHOR 3: Color from Recency (lastModified) or Type
  const lastModified = metadata.lastModified ? new Date(metadata.lastModified).getTime() : 0;
  if (lastModified > 0) {
    const ageInDays = (Date.now() - lastModified) / (1000 * 60 * 60 * 24);
    // Fade from hot (yellow/white) to cold (cyan/blue) over 90 days
    const heat = Math.max(0, 1 - ageInDays / 90);
    const hue = 0.5 + heat * 0.1; // Shift from cyan (0.5) to yellow (0.6)
    const saturation = 0.6 + heat * 0.4; // More saturated when hot
    const lightness = 0.4 + heat * 0.3; // Brighter when hot
    visuals.color.setHSL(hue, saturation, lightness);
  } else if (metadata.type) {
    // Fallback to type-based colors if no lastModified
    const typeColors: Record<string, string> = {
      'folder': '#FFD700',     // Gold
      'file': '#00CED1',       // Dark turquoise
      'function': '#FF6B6B',   // Coral
      'class': '#4ECDC4',      // Turquoise
      'variable': '#95E1D3',   // Mint
      'import': '#F38181',     // Light coral
      'export': '#AA96DA',     // Lavender
      'default': '#00ffff'     // Default cyan
    };
    const color = typeColors[metadata.type] || typeColors['default'];
    visuals.color.set(color);
  } else {
    // Use connectivity-based color as another fallback
    const colorIntensity = Math.min(hyperlinkCount / 10, 1);
    const hue = 0.5 - colorIntensity * 0.3; // From cyan to purple based on connections
    const saturation = 0.6 + colorIntensity * 0.4;
    const lightness = 0.5;
    visuals.color.setHSL(hue, saturation, lightness);
  }

  // METAPHOR 4: Emissive Glow from AI Processing (perplexityLink)
  if (metadata.perplexityLink) {
    visuals.emissive.set('#FFD700'); // Gold emissive for AI-processed nodes
  } else {
    visuals.emissive.copy(visuals.color).multiplyScalar(0.5);
  }

  // METAPHOR 5: Pulse Speed from File Size
  visuals.pulseSpeed = 0.5 + Math.log10(Math.max(1, fileSize / 1024)) * 0.5;

  return visuals;
};


// --- 2. Create Geometry and Material Resources ---
const useGeometries = () => useMemo(() => ({
  sphere: new THREE.SphereGeometry(0.5, 32, 16),
  box: new THREE.BoxGeometry(0.8, 0.8, 0.8),
  octahedron: new THREE.OctahedronGeometry(0.7, 0),
  icosahedron: new THREE.IcosahedronGeometry(0.6, 1),
}), []);

const useHologramMaterial = (settings: any) => useMemo(() => {
  const nodeSettings = settings?.visualisation?.graphs?.logseq?.nodes || settings?.visualisation?.nodes;
  const material = new HologramNodeMaterial({
    baseColor: nodeSettings?.baseColor || '#00ffff',
    emissiveColor: nodeSettings?.emissiveColor || '#00ffff',
    opacity: nodeSettings?.opacity ?? 0.8,
    glowStrength: 2.0,
    rimPower: 2.5,
  });
  material.defines = { ...material.defines, USE_INSTANCING_COLOR: '' };
  material.needsUpdate = true;
  return material;
}, [settings]);


// --- 3. The React Component ---
interface MetadataShapesProps {
  nodes: GraphNode[];
  nodePositions: Float32Array | null;
  onNodeClick?: (nodeId: string, event: any) => void;
  settings: any;
}

export const MetadataShapes: React.FC<MetadataShapesProps> = ({ nodes, nodePositions, onNodeClick, settings }) => {
  const geometries = useGeometries();
  const material = useHologramMaterial(settings);
  const meshRefs = useRef<Map<string, THREE.InstancedMesh>>(new Map());

  // Group nodes by their new geometry type for instanced rendering
  const nodeGroups = useMemo(() => {
    const groups = new Map<string, { nodes: GraphNode[], originalIndices: number[] }>();
    nodes.forEach((node, index) => {
      const { geometryType } = getVisualsForNode(node);
      if (!groups.has(geometryType)) {
        groups.set(geometryType, { nodes: [], originalIndices: [] });
      }
      groups.get(geometryType)!.nodes.push(node);
      groups.get(geometryType)!.originalIndices.push(index);
    });
    return groups;
  }, [nodes]);

  // Frame loop to update instances
  useFrame((state) => {
    if (!nodePositions) return;

    material.updateTime(state.clock.elapsedTime);
    const tempMatrix = new THREE.Matrix4();
    const tempColor = new THREE.Color();

    nodeGroups.forEach((group, geometryType) => {
      const mesh = meshRefs.current.get(geometryType);
      if (!mesh) return;

      group.nodes.forEach((node, localIndex) => {
        const originalIndex = group.originalIndices[localIndex];
        const i3 = originalIndex * 3;

        const visuals = getVisualsForNode(node);
        material.uniforms.pulseSpeed.value = visuals.pulseSpeed;

        // Position & Scale
        tempMatrix.makeScale(visuals.scale, visuals.scale, visuals.scale);
        tempMatrix.setPosition(nodePositions[i3], nodePositions[i3 + 1], nodePositions[i3 + 2]);
        mesh.setMatrixAt(localIndex, tempMatrix);

        // Color
        tempColor.copy(visuals.color);
        mesh.setColorAt(localIndex, tempColor);
      });

      mesh.instanceMatrix.needsUpdate = true;
      if (mesh.instanceColor) {
        mesh.instanceColor.needsUpdate = true;
      }
    });
  });

  return (
    <>
      {Array.from(nodeGroups.entries()).map(([geometryType, group]) => (
        <instancedMesh
          key={geometryType}
          ref={(ref) => { if (ref) meshRefs.current.set(geometryType, ref); }}
          args={[geometries[geometryType], material, group.nodes.length]}
          frustumCulled={false}
          onClick={(e) => {
            if (e.instanceId !== undefined && onNodeClick) {
              onNodeClick(group.nodes[e.instanceId].id, e);
            }
          }}
        />
      ))}
    </>
  );
};