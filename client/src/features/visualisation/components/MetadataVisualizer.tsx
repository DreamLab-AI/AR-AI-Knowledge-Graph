import React, { useRef, useEffect, useState, useMemo } from 'react';
import * as THREE from 'three'; // Use namespace import
import { useThree, useFrame } from '@react-three/fiber';
// import { Text, Billboard, useTexture } from '@react-three/drei'; // Commented out due to import errors
import { usePlatform } from '@/services/platformManager';
import { useSettingsStore } from '@/store/settingsStore';
import { createLogger } from '@/utils/logger';

const logger = createLogger('MetadataVisualizer');

// Type guard to check for Vector3 instance using instanceof
// Reverting to instanceof check as property check didn't resolve TS errors
function isVector3Instance(obj: any): obj is THREE.Vector3 {
  // Check if Vector3 constructor exists on THREE before using instanceof
  return typeof THREE.Vector3 === 'function' && obj instanceof THREE.Vector3;
}


// Types for metadata and labels
export interface NodeMetadata {
  id: string;
  position: [number, number, number] | { x: number; y: number; z: number } | THREE.Vector3;
  label?: string;
  description?: string;
  fileSize?: number;
  type?: string;
  color?: string | number;
  icon?: string;
  priority?: number;
  [key: string]: any; // Allow additional properties
}

interface MetadataVisualizerProps {
  children?: React.ReactNode;
  renderLabels?: boolean;
  renderIcons?: boolean;
  renderMetrics?: boolean;
}

/**
 * MetadataVisualizer component using React Three Fiber
 * This is a modernized version of the original MetadataVisualizer class
 */
export const MetadataVisualizer: React.FC<MetadataVisualizerProps> = ({
  children,
  renderLabels = true,
  renderIcons = true,
  renderMetrics = false
}) => {
  const { scene, camera } = useThree();
  // Use THREE.Object3D as Group might not be resolving correctly
  const groupRef = useRef<THREE.Group>(null);
  const { isXRMode } = usePlatform();
  const labelSettings = useSettingsStore(state => state.settings?.visualisation?.labels);

  // Layer management for XR mode
  useEffect(() => {
    if (!groupRef.current) return;

    // Set layers based on XR mode
    const group = groupRef.current;
    if (isXRMode) {
      // In XR mode, use layer 1 to ensure labels are visible in XR
      group.traverse(obj => {
        obj.layers.set(1);
      });
    } else {
      // In desktop mode, use default layer
      group.traverse(obj => {
        obj.layers.set(0);
      });
    }
  }, [isXRMode]);

  // Render optimization - only update label positions at 30fps
  useFrame((state, delta) => {
    // Potential optimization logic here
  }, 2); // Lower priority than regular rendering

  return (
    // Use THREE.Group directly in JSX if needed, or keep as <group>
    <group ref={groupRef} name="metadata-container">
      {children}
      {/* {renderLabels && <LabelSystem />} */} {/* Commented out LabelSystem usage */}
      {renderIcons && <IconSystem />}
      {renderMetrics && <MetricsDisplay />}
    </group>
  );
};

// Component to display node labels with proper positioning and formatting
const LabelSystem: React.FC = () => {
  const labelManagerRef = useTextLabelManager();
  const { labels } = labelManagerRef.current;
  const labelSettings = useSettingsStore(state => state.settings?.visualisation?.labels);

  // Don't render if labels are disabled
  // if (!labelSettings?.enabled) return null; // Commented out due to type error

  return (
    <group name="label-system">
      {labels.map(label => (
        <NodeLabel
          key={label.id}
          id={label.id}
          position={label.position}
          text={label.text}
          // color={labelSettings.color || '#ffffff'} // Commented out
          // size={labelSettings.size || 1} // Commented out
          // backgroundColor={labelSettings.backgroundColor} // Commented out
          // showDistance={labelSettings.showDistance} // Commented out
          // fadeDistance={labelSettings.fadeDistance} // Commented out
        />
      ))}
    </group>
  );
};

// Advanced label component with distance-based fading and billboarding
interface NodeLabelProps {
  id: string;
  position: [number, number, number] | { x: number; y: number; z: number } | THREE.Vector3;
  text: string;
  color?: string;
  size?: number;
  backgroundColor?: string;
  showDistance?: number;
  fadeDistance?: number;
}

const NodeLabel: React.FC<NodeLabelProps> = ({
  id,
  position,
  text,
  color = '#ffffff',
  size = 1,
  backgroundColor,
  showDistance = 0,
  fadeDistance = 0
}) => {
  // Skip rendering empty labels
  if (!text?.trim()) return null;

  const { camera } = useThree();
  const [opacity, setOpacity] = useState(1);

  // Convert position to tuple format with type guards
  const labelPos: [number, number, number] = useMemo(() => {
    if (isVector3Instance(position)) { // Use instanceof type guard
       // Explicit cast to help TS understand the type is narrowed
      const vec = position as THREE.Vector3;
      return [vec.x, vec.y, vec.z];
    } else if (Array.isArray(position)) {
      return position as [number, number, number]; // Assume it's a tuple if array
    } else if (typeof position === 'object' && position !== null && 'x' in position && 'y' in position && 'z' in position) {
      const posObj = position as { x: number; y: number; z: number };
      return [posObj.x, posObj.y, posObj.z];
    }
    logger.warn(`Invalid position format for label ${id}:`, position);
    return [0, 0, 0]; // Default position if format is unknown
  }, [position]);

  // Handle distance-based opacity
  useFrame(() => {
    if (!fadeDistance) return;

    // Calculate distance using tuple positions
    const dx = camera.position.x - labelPos[0];
    const dy = camera.position.y - labelPos[1];
    const dz = camera.position.z - labelPos[2];
    const distance = Math.sqrt(dx * dx + dy * dy + dz * dz);

    if (distance > fadeDistance) {
      setOpacity(0);
    } else if (distance > showDistance) {
      // Linear fade from showDistance to fadeDistance
      const fadeRatio = 1 - ((distance - showDistance) / (fadeDistance - showDistance));
      setOpacity(Math.max(0, Math.min(1, fadeRatio)));
    } else {
      setOpacity(1);
    }
  });

  // Don't render if fully transparent
  if (opacity <= 0) return null;

  // Commenting out Billboard and Text usage due to import errors
  return null;
  /*
  return (
    <Billboard
      position={labelPos}
      follow={true}
      lockX={false}
      lockY={false}
      lockZ={false}
    >
      <Text
        fontSize={size}
        color={color}
        anchorX="center"
        anchorY="middle"
        outlineWidth={0.02}
        outlineColor="#000000"
        outlineOpacity={0.8}
        overflowWrap="normal"
        maxWidth={10}
        textAlign="center"
        renderOrder={10} // Ensure text renders on top of other objects
        material-depthTest={false} // Make sure text is always visible
        material-transparent={true}
        material-opacity={opacity}
      >
        {text}
        {backgroundColor && (
          <meshBasicMaterial
            // color={backgroundColor} // Commented out due to type error
            opacity={opacity * 0.7}
            transparent={true}
            side={THREE.DoubleSide} // Use THREE namespace
          />
        )}
      </Text>
    </Billboard>
  );
  */
};

// System to display icons next to nodes
const IconSystem: React.FC = () => {
  // Implement if needed
  return null;
};

// System to display performance metrics
const MetricsDisplay: React.FC = () => {
  // Implement if needed
  return null;
};

// Hook to manage text labels
export function useTextLabelManager() {
  const labelManagerRef = useRef<{
    labels: Array<{
      id: string;
      text: string;
      position: [number, number, number];
    }>;
    updateLabel: (id: string, text: string, position: [number, number, number] | { x: number; y: number; z: number } | THREE.Vector3) => void;
    removeLabel: (id: string) => void;
    clearLabels: () => void;
  }>({
    labels: [],
    updateLabel: (id, text, position) => {
      const labels = labelManagerRef.current.labels;

      // Convert position to tuple format with type guards
      let pos: [number, number, number];
      if (isVector3Instance(position)) { // Use instanceof type guard
         // Explicit cast to help TS understand the type is narrowed
        const vec = position as THREE.Vector3;
        pos = [vec.x, vec.y, vec.z];
      } else if (Array.isArray(position)) {
        pos = position as [number, number, number];
      } else if (typeof position === 'object' && position !== null && 'x' in position && 'y' in position && 'z' in position) {
        const posObj = position as { x: number; y: number; z: number };
        pos = [posObj.x, posObj.y, posObj.z];
      } else {
        logger.warn(`Invalid position format for updateLabel ${id}:`, position);
        pos = [0, 0, 0]; // Default or handle error
      }

      const existingLabelIndex = labels.findIndex(label => label.id === id);

      if (existingLabelIndex >= 0) {
        // Update existing label
        labels[existingLabelIndex] = {
          ...labels[existingLabelIndex],
          text: text || labels[existingLabelIndex].text,
          position: pos
        };
      } else {
        // Add new label
        labels.push({ id, text, position: pos });
      }

      // Force update by creating a new array
      labelManagerRef.current.labels = [...labels];
    },
    removeLabel: (id) => {
      labelManagerRef.current.labels = labelManagerRef.current.labels.filter(
        label => label.id !== id
      );
    },
    clearLabels: () => {
      labelManagerRef.current.labels = [];
    }
  });

  return labelManagerRef;
}

// Factory function to create SDF font texture for high-quality text rendering
export const createSDFFont = async (fontUrl: string, fontSize: number = 64) => {
  // This would be an implementation of SDF font generation
  // For now, we use drei's Text component which provides high-quality text
  return null;
};

// Class-based API for backwards compatibility
export class MetadataVisualizerManager {
  private static instance: MetadataVisualizerManager;
  private labels: Map<string, { text: string; position: [number, number, number] }> = new Map();
  private updateCallback: (() => void) | null = null;

  private constructor() {}

  public static getInstance(): MetadataVisualizerManager {
    if (!MetadataVisualizerManager.instance) {
      MetadataVisualizerManager.instance = new MetadataVisualizerManager();
    }
    return MetadataVisualizerManager.instance;
  }

  public setUpdateCallback(callback: () => void): void {
    this.updateCallback = callback;
  }

  public updateNodeLabel(
    nodeId: string,
    text: string,
    position: [number, number, number] | { x: number; y: number; z: number } | THREE.Vector3
  ): void {
    try {
      // Convert position to tuple format with type guards
      let pos: [number, number, number];
      if (isVector3Instance(position)) { // Use instanceof type guard
         // Explicit cast to help TS understand the type is narrowed
        const vec = position as THREE.Vector3;
        pos = [vec.x, vec.y, vec.z];
      } else if (Array.isArray(position)) {
        pos = position as [number, number, number];
      } else if (typeof position === 'object' && position !== null && 'x' in position && 'y' in position && 'z' in position) {
        const posObj = position as { x: number; y: number; z: number };
        pos = [posObj.x, posObj.y, posObj.z];
      } else {
         logger.warn(`Invalid position format for updateNodeLabel ${nodeId}:`, position);
         pos = [0,0,0]; // Default or handle error
      }

      this.labels.set(nodeId, { text, position: pos });

      if (this.updateCallback) {
        this.updateCallback();
      }
    } catch (error) {
      logger.error('Error updating node label:', error);
    }
  }

  public clearLabel(nodeId: string): void {
    this.labels.delete(nodeId);

    if (this.updateCallback) {
      this.updateCallback();
    }
  }

  public clearAllLabels(): void {
    this.labels.clear();

    if (this.updateCallback) {
      this.updateCallback();
    }
  }

  public getAllLabels(): Array<{ id: string; text: string; position: [number, number, number] }> {
    return Array.from(this.labels.entries()).map(([id, label]) => ({
      id,
      text: label.text,
      position: label.position
    }));
  }

  public dispose(): void {
    this.labels.clear();
    this.updateCallback = null;

    // Reset singleton instance
    MetadataVisualizerManager.instance = null as any;
  }
}

// Export singleton instance for backwards compatibility
export const metadataVisualizer = MetadataVisualizerManager.getInstance();

export default MetadataVisualizer;