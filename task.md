High-Level Plan & Strategy
The core problem is a mismatch between frontend assumptions and backend reality. The frontend code in MetadataNodes.tsx and its variants expects a type field in the node metadata (e.g., 'folder', 'concept'), which the backend doesn't provide.
Our strategy is to create a new, definitive component that uses the actual available metadata fields to drive a rich and performant visualization. We will create meaningful mappings for:
Connectivity (hyperlinkCount): Determines the geometric complexity of the node.
Content Volume (fileSize): Influences the node's scale and a subtle pulsing effect.
Recency (lastModified): Dictates the node's color, from "hot" (new) to "cold" (old).
AI Processing (perplexityLink): Adds a special emissive glow to indicate AI enrichment.
This approach finds a balance, avoiding excessive visual complexity while making the visualization dense with information.
Instructions for the Coding Agent
Step 1: Analyze the Real Metadata Payload
First, confirm the available metadata fields by inspecting the Rust backend code. The file src/actors/graph_actor.rs in the build_from_metadata function reveals the actual payload sent to the client:
fileName (string)
fileSize (string, represents bytes)
hyperlinkCount (string, represents integer)
lastModified (string, RFC3339 timestamp)
perplexityLink (string, exists only if present)
nodeSize (string, a pre-calculated value we can ignore in favor of dynamic scaling)
We will use hyperlinkCount, fileSize, lastModified, and the existence of perplexityLink as our primary drivers for visualization.
Step 2: Create the New, Definitive Metadata Component
Instead of modifying the three existing MetadataNodes components, you will create a single, superior version that encapsulates the new logic.
Action: Create a new file: client/src/features/graph/components/MetadataShapes.tsx.
Generated bash
touch client/src/features/graph/components/MetadataShapes.tsx
Use code with caution.
Bash
Populate this new file with the following code. This structure includes the core logic for our new visual metaphors.
Generated tsx
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

  // METAPHOR 3: Color from Recency (lastModified)
  const lastModified = metadata.lastModified ? new Date(metadata.lastModified).getTime() : 0;
  if (lastModified > 0) {
    const ageInDays = (Date.now() - lastModified) / (1000 * 60 * 60 * 24);
    // Fade from hot (yellow/white) to cold (cyan/blue) over 90 days
    const heat = Math.max(0, 1 - ageInDays / 90);
    const hue = 0.5 + heat * 0.1; // Shift from cyan (0.5) to yellow (0.6)
    const saturation = 0.6 + heat * 0.4; // More saturated when hot
    const lightness = 0.4 + heat * 0.3; // Brighter when hot
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
Use code with caution.
Tsx
Step 3: Integrate the New Component into GraphManager
Now, you will replace the old, confusing MetadataNodes components with your new, unified MetadataShapes component.
Action: Modify client/src/features/graph/components/GraphManager.tsx.
Import the new component:
Generated ts
import { MetadataShapes } from './MetadataShapes';
Use code with caution.
Ts
Remove old imports: Delete the imports for MetadataNodesEnhanced, MetadataNodes, MetadataNodesV2, and MetadataNodesV3.
Update the rendering logic: Find the JSX part where the MetadataNodes... components are rendered. Replace the entire block with this single, clean implementation:
Generated tsx
// Find this line:
const enableMetadataShape = nodeSettings?.enableMetadataShape ?? false;

// ... and in the return statement's JSX, replace the old logic with this:
{enableMetadataShape ? (
  <MetadataShapes
    nodes={graphData.nodes}
    nodePositions={nodePositionsRef.current}
    onNodeClick={(nodeId, event) => {
      const nodeIndex = graphData.nodes.findIndex(n => n.id === nodeId);
      if (nodeIndex !== -1) {
        handlePointerDown({ ...event, instanceId: nodeIndex } as any);
      }
    }}
    settings={settings}
  />
) : (
  <instancedMesh
    ref={meshRef}
    args={[undefined, undefined, graphData.nodes.length]}
    // ... (the rest of the instancedMesh props remain the same)
  >
    <sphereGeometry args={[0.5, 32, 32]} />
  </instancedMesh>
)}
Use code with caution.
Tsx
Step 4: Cleanup Old and Unused Files
To complete the migration and reduce code clutter, remove the now-redundant files.
Action: Delete the following files:
Generated bash
rm client/src/features/graph/components/MetadataNodes.tsx
rm client/src/features/graph/components/MetadataNodesV2.tsx
rm client/src/features/graph/components/MetadataNodesV3.tsx
rm client/src/features/graph/components/MetadataNodesEnhanced.tsx
rm client/src/features/graph/components/SatelliteSystem.tsx # This is a complex feature that we are removing for now to find a balanced visualisation. It can be added back later.
Use code with caution.
Bash
Final Vision & Expected Outcome
After completing these steps, the "Enable Metadata Shape" toggle in the settings will activate a rich, information-dense visualization where:
Node Shape directly communicates how connected a document is.
Node Size communicates its content volume and importance.
Node Color communicates its recency, with newer files appearing hotter and brighter.
A Special Glow instantly identifies nodes that have been processed by AI.
Pulse Speed gives a subtle hint about the file's size.
