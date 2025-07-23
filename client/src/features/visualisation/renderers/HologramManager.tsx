import React, { useRef, useEffect, useState } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import { useSettingsStore } from '@/store/settingsStore';
import { createLogger } from '@/utils/logger';
import { HologramMaterial } from './materials/HologramMaterial';

const logger = createLogger('HologramManager');

// Component for an individual hologram ring
export const HologramRing: React.FC<{
  size?: number;
  color?: string | THREE.Color | number;
  opacity?: number;
  rotationAxis?: readonly [number, number, number];
  rotationSpeed?: number;
  segments?: number;
}> = ({
  size = 1,
  color = '#00ffff',
  opacity = 0.7,
  rotationAxis = [0, 1, 0],
  rotationSpeed = 0.5,
  segments = 64
}) => {
  // Use state for rotation instead of imperatively updating refs
  const [rotation, setRotation] = useState<[number, number, number]>([0, 0, 0]);

  // Animate ring rotation
  useFrame((_, delta) => {
    if (rotationSpeed > 0) {
      setRotation(prev => [
        prev[0] + delta * rotationSpeed * rotationAxis[0],
        prev[1] + delta * rotationSpeed * rotationAxis[1],
        prev[2] + delta * rotationSpeed * rotationAxis[2]
      ]);
    }
  });

  return (
    <mesh rotation={rotation}>
      <ringGeometry args={[size * 0.8, size, segments]} />
      <meshBasicMaterial color={color} transparent={true} opacity={opacity} wireframe={true} />
    </mesh>
  );
};

// Component for a hologram sphere
export const HologramSphere: React.FC<{
  size?: number;
  color?: string | THREE.Color | number;
  opacity?: number;
  detail?: number;
  wireframe?: boolean;
  rotationSpeed?: number;
}> = ({
  size = 1,
  color = '#00ffff',
  opacity = 0.5,
  detail = 1,
  wireframe = true, // Always true for wireframe effect
  rotationSpeed = 0.2
}) => {
  // Use state for rotation instead of imperatively updating refs
  const [rotationY, setRotationY] = useState(0);

  // Animate sphere rotation
  useFrame((_, delta) => {
    if (rotationSpeed > 0) {
      setRotationY(prev => prev + delta * rotationSpeed);
    }
  });

  return (
    <mesh rotation={[0, rotationY, 0]}>
      <icosahedronGeometry args={[size, detail]} />
      <meshBasicMaterial
        color={color}
        transparent={true}
        opacity={opacity}
        wireframe={true}
        toneMapped={false}
      />
    </mesh>
  );
};

// Main HologramManager component that manages multiple hologram elements
export const HologramManager: React.FC<{
  position?: readonly [number, number, number];
  isXRMode?: boolean;
}> = ({
  position = [0, 0, 0],
  isXRMode = false
}) => {
  const settings = useSettingsStore(state => state.settings?.visualisation?.hologram);
  const groupRef = useRef<THREE.Group>(null);

  // Parse sphere sizes from settings
  const sphereSizes: number[] = React.useMemo(() => {
    const sizesSetting: unknown = settings?.sphereSizes; // Start with unknown

    if (typeof sizesSetting === 'string') {
      // Explicitly cast after check
      const strSetting = sizesSetting as string;
      return strSetting.split(',').map(s => parseFloat(s.trim())).filter(n => !isNaN(n));
    } else if (Array.isArray(sizesSetting)) {
      // Filter for numbers within the array
      const arrSetting = sizesSetting as unknown[]; // Cast to unknown array first
      return arrSetting.filter((n): n is number => typeof n === 'number' && !isNaN(n));
    }
    // Default value if setting is missing or invalid
    return [40, 80];
  }, [settings?.sphereSizes]);

  // Set layers for bloom effect
  useEffect(() => {
    const group = groupRef.current;
    if (group) {
      // Use type assertion to get around TypeScript issues
      (group as any).layers.set(0); // Default layer
      (group as any).layers.enable(1); // Bloom layer

      // Apply to all children
      (group as any).traverse((child: any) => {
        if (child.layers) {
          child.layers.set(0);
          child.layers.enable(1);
        }
      });
    }
  }, []);

  const quality = isXRMode ? 'high' : 'medium';
  const color: string | number = settings?.ringColor || '#00ffff'; // Use ringColor instead of color
  const opacity = settings?.ringOpacity !== undefined ? settings.ringOpacity : 0.7;
  const rotationSpeed = settings?.ringRotationSpeed !== undefined ? settings.ringRotationSpeed : 0.5;
  const enableTriangleSphere = settings?.enableTriangleSphere !== false;
  const triangleSphereSize = settings?.triangleSphereSize || 60;
  const triangleSphereOpacity = settings?.triangleSphereOpacity || 0.3;
  
  // Ensure we have at least some spheres to render
  const finalSphereSizes = sphereSizes.length > 0 ? sphereSizes : [40, 80];

  return (
    <group ref={groupRef} position={position as any}>
      {/* Render rings based on settings */}
      {finalSphereSizes.map((size, index) => (
        <HologramRing
          key={`ring-${index}`}
          size={size / 100} // Convert to meters
          color={color}
          opacity={opacity}
          rotationAxis={[
            Math.cos(index * Math.PI / 3),
            Math.sin(index * Math.PI / 3),
            0.5
          ]}
          rotationSpeed={rotationSpeed * (0.8 + index * 0.2)}
          segments={quality === 'high' ? 64 : 32}
        />
      ))}

      {/* Render triangle sphere if enabled */}
      {enableTriangleSphere && (
        <HologramSphere
          size={triangleSphereSize / 100} // Convert to meters
          color={color}
          opacity={triangleSphereOpacity}
          detail={quality === 'high' ? 2 : 1}
          wireframe={true}
        />
      )}
    </group>
  );
};

// A composite hologram component for easy use
export const Hologram: React.FC<{
  position?: readonly [number, number, number];
  color?: string | THREE.Color | number;
  size?: number;
  children?: React.ReactNode;
}> = ({
  position = [0, 0, 0],
  color = '#00ffff',
  size = 1,
  children
}) => {
  return (
    <group position={position as any} scale={size}>
      {children}
      <HologramManager />
    </group>
  );
};

// Class-based wrapper for non-React usage (for backward compatibility)
// Using 'any' types to avoid TypeScript errors with THREE.js
export class HologramManagerClass {
  private scene: any; // THREE.Scene
  private group: any; // THREE.Group
  private ringInstances: any[] = []; // THREE.Mesh[]
  private sphereInstances: any[] = []; // THREE.Mesh[]
  private isXRMode: boolean = false;
  private settings: any;

  constructor(scene: any, settings: any) {
    this.scene = scene;
    this.settings = settings;
    this.group = new THREE.Group();

    // Enable bloom layer
    this.group.layers.set(0);
    this.group.layers.enable(1);

    this.createHolograms();
    this.scene.add(this.group);
  }

  private createHolograms() {
    // Clear existing holograms
    const group = this.group;
    while (group.children.length > 0) {
      const child = group.children[0];
      group.remove(child);

      // Handle geometry and material disposal
      if (child.geometry) child.geometry.dispose();
      if (child.material) {
        if (Array.isArray(child.material)) {
          child.material.forEach((m: any) => m && m.dispose());
        } else {
          child.material.dispose();
        }
      }
    }

    this.ringInstances = [];
    this.sphereInstances = [];

    // Quality based on XR mode
    const quality = this.isXRMode ? 'high' : 'medium';
    const segments = quality === 'high' ? 64 : 32;

    // Extract settings
    const hologramSettings = this.settings?.visualisation?.hologram || {};
    const color = hologramSettings.color || 0x00ffff;
    const opacity = hologramSettings.ringOpacity !== undefined ? hologramSettings.ringOpacity : 0.7;
    const sphereSizes = Array.isArray(hologramSettings.sphereSizes)
      ? hologramSettings.sphereSizes
      : [40, 80];

    // Create ring instances using type assertions for THREE classes
    sphereSizes.forEach((size, index) => {
      // Use any type to bypass TypeScript checks
      const geometry = new (THREE as any).RingGeometry(size * 0.8 / 100, size / 100, segments);
      const material = new (THREE as any).MeshBasicMaterial({
        color: color,
        transparent: true,
        opacity: opacity,
        side: (THREE as any).DoubleSide,
        depthWrite: false
      });

      const ring = new (THREE as any).Mesh(geometry, material);

      // Set random rotation
      ring.rotation.x = Math.PI / 3 * index;
      ring.rotation.y = Math.PI / 6 * index;

      // Enable bloom layer
      ring.layers.set(0);
      ring.layers.enable(1);

      this.ringInstances.push(ring);
      group.add(ring);
    });

    // Create triangle sphere if enabled
    if (hologramSettings.enableTriangleSphere) {
      const size = hologramSettings.triangleSphereSize || 60;
      const sphereOpacity = hologramSettings.triangleSphereOpacity || 0.3;
      const detail = quality === 'high' ? 2 : 1;

      // Use any type to bypass TypeScript checks
      const geometry = new (THREE as any).IcosahedronGeometry(size / 100, detail);
      const material = new (THREE as any).MeshBasicMaterial({
        color: color,
        wireframe: true,
        transparent: true,
        opacity: sphereOpacity,
        side: (THREE as any).DoubleSide,
        depthWrite: false
      });

      const sphere = new (THREE as any).Mesh(geometry, material);

      // Enable bloom layer
      sphere.layers.set(0);
      sphere.layers.enable(1);

      this.sphereInstances.push(sphere);
      group.add(sphere);
    }
  }

  setXRMode(enabled: boolean) {
    this.isXRMode = enabled;
    this.createHolograms();
  }

  update(deltaTime: number) {
    // Get rotation speed from settings
    const rotationSpeed = this.settings?.visualisation?.hologram?.ringRotationSpeed || 0.5;

    // Update ring rotations
    this.ringInstances.forEach((ring: any, index: number) => {
      // Each ring rotates at a different speed
      const speed = rotationSpeed * (1.0 + index * 0.2);
      ring.rotation.y += deltaTime * speed;
    });

    // Update sphere rotations
    this.sphereInstances.forEach((sphere: any) => {
      sphere.rotation.y += deltaTime * rotationSpeed * 0.5;
    });
  }

  updateSettings(newSettings: any) {
    this.settings = newSettings;
    this.createHolograms();
  }

  getGroup() {
    return this.group;
  }

  dispose() {
    this.scene.remove(this.group);

    // Dispose geometries and materials
    this.ringInstances.forEach((ring: any) => {
      if (ring.geometry) ring.geometry.dispose();
      if (ring.material) {
        if (Array.isArray(ring.material)) {
          ring.material.forEach((m: any) => m && m.dispose());
        } else {
          ring.material.dispose();
        }
      }
    });

    this.sphereInstances.forEach((sphere: any) => {
      if (sphere.geometry) sphere.geometry.dispose();
      if (sphere.material) {
        if (Array.isArray(sphere.material)) {
          sphere.material.forEach((m: any) => m && m.dispose());
        } else {
          sphere.material.dispose();
        }
      }
    });

    this.ringInstances = [];
    this.sphereInstances = [];
  }
}

export default HologramManager;