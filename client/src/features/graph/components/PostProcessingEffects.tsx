import React, { useRef, useMemo } from 'react';
import { useThree, useFrame, extend } from '@react-three/fiber';
import { EffectComposer } from 'three/examples/jsm/postprocessing/EffectComposer';
import { RenderPass } from 'three/examples/jsm/postprocessing/RenderPass';
import { UnrealBloomPass } from 'three/examples/jsm/postprocessing/UnrealBloomPass';
import { ShaderPass } from 'three/examples/jsm/postprocessing/ShaderPass';
import * as THREE from 'three';
import { useSettingsStore } from '../../../store/settingsStore';

// Extend Three.js objects for React Three Fiber
extend({ EffectComposer, RenderPass, UnrealBloomPass, ShaderPass });

// Custom vignette shader
const VignetteShader = {
  uniforms: {
    tDiffuse: { value: null },
    offset: { value: 1.0 },
    darkness: { value: 1.0 }
  },
  vertexShader: `
    varying vec2 vUv;
    void main() {
      vUv = uv;
      gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
    }
  `,
  fragmentShader: `
    uniform sampler2D tDiffuse;
    uniform float offset;
    uniform float darkness;
    varying vec2 vUv;
    
    void main() {
      vec4 texel = texture2D(tDiffuse, vUv);
      vec2 uv = (vUv - vec2(0.5)) * vec2(offset);
      gl_FragColor = vec4(mix(texel.rgb, vec3(1.0 - darkness), dot(uv, uv)), texel.a);
    }
  `
};

// Depth of field shader (simplified)
const DepthOfFieldShader = {
  uniforms: {
    tDiffuse: { value: null },
    tDepth: { value: null },
    focus: { value: 1.0 },
    maxblur: { value: 0.01 },
    aspect: { value: 1.0 }
  },
  vertexShader: `
    varying vec2 vUv;
    void main() {
      vUv = uv;
      gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
    }
  `,
  fragmentShader: `
    uniform sampler2D tDiffuse;
    uniform sampler2D tDepth;
    uniform float focus;
    uniform float maxblur;
    uniform float aspect;
    varying vec2 vUv;
    
    float getDepth(vec2 coord) {
      return texture2D(tDepth, coord).r;
    }
    
    void main() {
      vec2 aspectCorrect = vec2(1.0, aspect);
      float depth = getDepth(vUv);
      float factor = depth - focus;
      
      vec2 dofblur = vec2(clamp(factor * maxblur, -maxblur, maxblur));
      vec2 dofblur9 = dofblur * 0.9;
      vec2 dofblur7 = dofblur * 0.7;
      vec2 dofblur4 = dofblur * 0.4;
      
      vec4 col = vec4(0.0);
      col += texture2D(tDiffuse, vUv);
      col += texture2D(tDiffuse, vUv + (vec2(0.0, 0.4) * aspectCorrect) * dofblur);
      col += texture2D(tDiffuse, vUv + (vec2(0.15, 0.37) * aspectCorrect) * dofblur);
      col += texture2D(tDiffuse, vUv + (vec2(0.29, 0.29) * aspectCorrect) * dofblur);
      col += texture2D(tDiffuse, vUv + (vec2(-0.37, 0.15) * aspectCorrect) * dofblur);
      col += texture2D(tDiffuse, vUv + (vec2(0.40, 0.0) * aspectCorrect) * dofblur);
      col += texture2D(tDiffuse, vUv + (vec2(0.37, -0.15) * aspectCorrect) * dofblur);
      col += texture2D(tDiffuse, vUv + (vec2(0.29, -0.29) * aspectCorrect) * dofblur);
      col += texture2D(tDiffuse, vUv + (vec2(-0.15, -0.37) * aspectCorrect) * dofblur);
      
      gl_FragColor = col / 9.0;
    }
  `
};

export const PostProcessingEffects: React.FC = () => {
  const { gl, scene, camera, size } = useThree();
  const composerRef = useRef<EffectComposer>();
  const settings = useSettingsStore(state => state.settings?.visualisation);
  
  // Create effect composer and passes
  const [composer, bloomPass, vignettePass] = useMemo(() => {
    const composer = new EffectComposer(gl);
    composer.setSize(size.width, size.height);
    
    // Render pass
    const renderPass = new RenderPass(scene, camera);
    composer.addPass(renderPass);
    
    // Bloom pass
    const bloomPass = new UnrealBloomPass(
      new THREE.Vector2(size.width, size.height),
      settings?.bloom?.strength || 1.5,
      settings?.bloom?.radius || 0.4,
      settings?.bloom?.threshold || 0.85
    );
    
    // Configure bloom
    bloomPass.threshold = settings?.bloom?.threshold || 0.0;
    bloomPass.strength = settings?.bloom?.strength || 1.5;
    bloomPass.radius = settings?.bloom?.radius || 0.4;
    
    if (settings?.bloom?.enabled !== false) {
      composer.addPass(bloomPass);
    }
    
    // Vignette pass
    const vignettePass = new ShaderPass(VignetteShader);
    vignettePass.uniforms.offset.value = 0.95;
    vignettePass.uniforms.darkness.value = 0.5;
    composer.addPass(vignettePass);
    
    return [composer, bloomPass, vignettePass];
  }, [gl, scene, camera, size, settings?.bloom]);
  
  // Update composer on resize
  React.useEffect(() => {
    composer.setSize(size.width, size.height);
  }, [composer, size]);
  
  // Update bloom settings
  React.useEffect(() => {
    if (bloomPass && settings?.bloom) {
      bloomPass.enabled = settings.bloom.enabled !== false;
      bloomPass.threshold = settings.bloom.threshold || 0.0;
      bloomPass.strength = settings.bloom.strength || 1.5;
      bloomPass.radius = settings.bloom.radius || 0.4;
    }
  }, [bloomPass, settings?.bloom]);
  
  // Render with composer
  useFrame(() => {
    if (composerRef.current) {
      composerRef.current.render();
    }
  }, 1);
  
  React.useEffect(() => {
    composerRef.current = composer;
    return () => {
      composer.dispose();
    };
  }, [composer]);
  
  return null;
};