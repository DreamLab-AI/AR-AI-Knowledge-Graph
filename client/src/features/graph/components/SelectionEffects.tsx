import React, { useRef, useEffect } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';

interface SelectionEffectsProps {
  selectedNodeId: string | null;
  nodePosition: THREE.Vector3 | null;
  color?: string;
}

export const SelectionEffects: React.FC<SelectionEffectsProps> = ({ 
  selectedNodeId, 
  nodePosition,
  color = '#00ffff'
}) => {
  const ringRef = useRef<THREE.Mesh>(null);
  const pulseRef = useRef<THREE.Mesh>(null);
  const glowRef = useRef<THREE.PointLight>(null);
  const animationRef = useRef({ time: 0, scale: 1 });
  
  useEffect(() => {
    if (selectedNodeId && nodePosition) {
      animationRef.current.time = 0;
      animationRef.current.scale = 0.1;
    }
  }, [selectedNodeId, nodePosition]);
  
  useFrame((state, delta) => {
    if (!selectedNodeId || !nodePosition) return;
    
    animationRef.current.time += delta;
    
    // Animate selection ring
    if (ringRef.current) {
      ringRef.current.position.copy(nodePosition);
      ringRef.current.rotation.z = animationRef.current.time * 0.5;
      
      // Pulsing scale
      const pulseScale = 1 + Math.sin(animationRef.current.time * 3) * 0.1;
      ringRef.current.scale.setScalar(pulseScale);
    }
    
    // Animate pulse wave
    if (pulseRef.current) {
      pulseRef.current.position.copy(nodePosition);
      
      // Expand and fade
      const expansion = (animationRef.current.time * 2) % 3;
      pulseRef.current.scale.setScalar(1 + expansion);
      
      const opacity = Math.max(0, 1 - expansion / 3);
      (pulseRef.current.material as THREE.MeshBasicMaterial).opacity = opacity * 0.3;
    }
    
    // Animate glow light
    if (glowRef.current) {
      glowRef.current.position.copy(nodePosition);
      glowRef.current.intensity = 2 + Math.sin(animationRef.current.time * 4) * 0.5;
    }
  });
  
  if (!selectedNodeId || !nodePosition) return null;
  
  return (
    <group>
      {/* Selection ring */}
      <mesh ref={ringRef}>
        <torusGeometry args={[1, 0.1, 16, 32]} />
        <meshBasicMaterial 
          color={color}
          transparent
          opacity={0.8}
          side={THREE.DoubleSide}
        />
      </mesh>
      
      {/* Pulse wave */}
      <mesh ref={pulseRef}>
        <ringGeometry args={[0.8, 1, 32]} />
        <meshBasicMaterial 
          color={color}
          transparent
          opacity={0.3}
          side={THREE.DoubleSide}
          blending={THREE.AdditiveBlending}
        />
      </mesh>
      
      {/* Glow light */}
      <pointLight 
        ref={glowRef}
        color={color}
        intensity={2}
        distance={10}
        decay={2}
      />
    </group>
  );
};