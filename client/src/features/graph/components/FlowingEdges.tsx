import React, { useRef, useMemo, useEffect } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { EdgeSettings } from '../../settings/config/settings';

interface FlowingEdgesProps {
  points: number[];
  settings: EdgeSettings;
  edgeData?: Array<{
    source: string;
    target: string;
    weight?: number;
    active?: boolean;
  }>;
}

// Custom shader for flowing edges
const flowVertexShader = `
  attribute float lineDistance;
  attribute vec3 instanceColorStart;
  attribute vec3 instanceColorEnd;
  
  varying float vLineDistance;
  varying vec3 vColor;
  
  void main() {
    vLineDistance = lineDistance;
    vColor = mix(instanceColorStart, instanceColorEnd, lineDistance);
    
    vec4 mvPosition = modelViewMatrix * vec4(position, 1.0);
    gl_Position = projectionMatrix * mvPosition;
  }
`;

const flowFragmentShader = `
  uniform float time;
  uniform float flowSpeed;
  uniform float flowIntensity;
  uniform float opacity;
  uniform vec3 baseColor;
  uniform bool enableFlowEffect;
  uniform bool useGradient;
  uniform float glowStrength;
  uniform float distanceIntensity;
  
  varying float vLineDistance;
  varying vec3 vColor;
  
  void main() {
    vec3 color = useGradient ? vColor : baseColor;
    
    // Flow effect
    float flow = 0.0;
    if (enableFlowEffect) {
      float offset = time * flowSpeed;
      flow = sin(vLineDistance * 10.0 - offset) * 0.5 + 0.5;
      flow = pow(flow, 3.0) * flowIntensity;
    }
    
    // Distance-based intensity
    float distanceFade = 1.0 - vLineDistance * distanceIntensity;
    
    // Glow effect
    float glow = pow(1.0 - abs(vLineDistance - 0.5) * 2.0, 2.0) * glowStrength;
    
    // Combine effects
    color += vec3(flow + glow) * 0.5;
    float alpha = opacity * distanceFade * (1.0 + flow * 0.5);
    
    gl_FragColor = vec4(color, alpha);
  }
`;

export const FlowingEdges: React.FC<FlowingEdgesProps> = ({ points, settings, edgeData }) => {
  const lineRef = useRef<THREE.LineSegments>(null);
  const materialRef = useRef<THREE.LineBasicMaterial>(null);
  
  // Create single geometry for all edges
  const geometry = useMemo(() => {
    if (points.length < 6) return null; // Need at least 2 points (6 values)
    
    const geo = new THREE.BufferGeometry();
    const positions = new Float32Array(points);
    geo.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    
    return geo;
  }, [points]);
  
  // Create simple line material
  const material = useMemo(() => {
    const color = new THREE.Color(settings.color || '#56b6c2');
    
    const mat = new THREE.LineBasicMaterial({
      color: color,
      transparent: true,
      opacity: settings.opacity || 0.6, // Increased for better visibility
      linewidth: settings.baseWidth || 2, // Increased line width
      depthWrite: true, // Enable depth writing for proper sorting
      alphaTest: 0.01, // Prevent z-fighting on nearly transparent areas
    });
    
    return mat;
  }, [settings.color, settings.opacity, settings.baseWidth]);
  
  // Update material reference
  useEffect(() => {
    materialRef.current = material;
  }, [material]);
  
  // Animate flow effect by modifying opacity
  useFrame((state) => {
    if (materialRef.current && settings.enableFlowEffect) {
      const flowIntensity = Math.sin(state.clock.elapsedTime * (settings.flowSpeed || 1.0)) * 0.3 + 0.7;
      materialRef.current.opacity = (settings.opacity || 0.25) * flowIntensity;
    }
  });
  
  if (!geometry) return null;
  
  return (
    <lineSegments ref={lineRef} geometry={geometry} material={material} renderOrder={5} />
  );
};