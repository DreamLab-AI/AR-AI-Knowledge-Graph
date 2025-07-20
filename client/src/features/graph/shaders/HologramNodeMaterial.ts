import * as THREE from 'three';
import { extend } from '@react-three/fiber';

// Vertex shader for hologram effect
const hologramVertexShader = `
  uniform float time;
  uniform float pulseSpeed;
  uniform float pulseStrength;
  
  varying vec3 vPosition;
  varying vec3 vNormal;
  varying vec3 vWorldPosition;
  varying vec3 vInstanceColor;
  
  void main() {
    vPosition = position;
    vNormal = normalize(normalMatrix * normal);
    
    // Get instance color from built-in instanceColor attribute
    #ifdef USE_INSTANCING_COLOR
      vInstanceColor = instanceColor;
    #else
      vInstanceColor = vec3(1.0);
    #endif
    
    // Apply instance transform (matrix already includes position and scale)
    vec3 transformed = position;
    
    // Add subtle vertex displacement for organic feel
    vec4 worldPosition = modelMatrix * instanceMatrix * vec4(transformed, 1.0);
    float displacement = sin(time * pulseSpeed + worldPosition.x * 0.1) * pulseStrength;
    worldPosition.xyz += normalize(normalMatrix * normal) * displacement * 0.1;
    
    vWorldPosition = worldPosition.xyz;
    
    gl_Position = projectionMatrix * viewMatrix * worldPosition;
  }
`;

// Fragment shader for hologram effect
const hologramFragmentShader = `
  uniform float time;
  uniform vec3 baseColor;
  uniform vec3 emissiveColor;
  uniform float opacity;
  uniform float scanlineSpeed;
  uniform float scanlineCount;
  uniform float glowStrength;
  uniform float rimPower;
  uniform bool enableHologram;
  uniform float hologramStrength;
  
  varying vec3 vPosition;
  varying vec3 vNormal;
  varying vec3 vWorldPosition;
  varying vec3 vInstanceColor;
  
  void main() {
    vec3 viewDirection = normalize(cameraPosition - vWorldPosition);
    
    // Base color with instance color modulation - favor instance color
    vec3 color = mix(baseColor, vInstanceColor, 0.9);
    
    // Fresnel rim lighting
    float rim = 1.0 - max(dot(viewDirection, vNormal), 0.0);
    rim = pow(rim, rimPower);
    
    // Hologram scanlines
    float scanline = 0.0;
    if (enableHologram) {
      float scan = sin(vWorldPosition.y * scanlineCount + time * scanlineSpeed);
      scanline = smoothstep(0.0, 0.1, scan) * hologramStrength;
    }
    
    // Glitch effect
    float glitch = 0.0;
    if (enableHologram) {
      float glitchTime = time * 10.0;
      glitch = step(0.99, sin(glitchTime * 1.0 + vWorldPosition.y * 12.0)) * 0.1;
    }
    
    // Combine effects
    vec3 emission = emissiveColor * (rim * glowStrength + scanline + glitch);
    color += emission;
    
    // Alpha with rim fade
    float alpha = mix(opacity, 1.0, rim * 0.5);
    alpha *= (1.0 - glitch * 0.5); // Flicker during glitch
    
    // Distance fade for depth
    float distance = length(cameraPosition - vWorldPosition);
    float distanceFade = 1.0 - smoothstep(50.0, 200.0, distance);
    alpha *= distanceFade;
    
    gl_FragColor = vec4(color, alpha);
  }
`;

// Shader material class
export class HologramNodeMaterial extends THREE.ShaderMaterial {
  constructor(parameters?: {
    baseColor?: THREE.Color | string;
    emissiveColor?: THREE.Color | string;
    opacity?: number;
    enableHologram?: boolean;
    scanlineSpeed?: number;
    scanlineCount?: number;
    glowStrength?: number;
    rimPower?: number;
    pulseSpeed?: number;
    pulseStrength?: number;
    hologramStrength?: number;
  }) {
    const params = {
      baseColor: new THREE.Color(parameters?.baseColor || '#00ffff'),
      emissiveColor: new THREE.Color(parameters?.emissiveColor || '#00ffff'),
      opacity: parameters?.opacity || 0.8,
      enableHologram: parameters?.enableHologram !== undefined ? parameters.enableHologram : true,
      scanlineSpeed: parameters?.scanlineSpeed || 2.0,
      scanlineCount: parameters?.scanlineCount || 30.0,
      glowStrength: parameters?.glowStrength || 1.0,
      rimPower: parameters?.rimPower || 2.0,
      pulseSpeed: parameters?.pulseSpeed || 1.0,
      pulseStrength: parameters?.pulseStrength || 0.1,
      hologramStrength: parameters?.hologramStrength || 0.3,
    };

    super({
      uniforms: {
        time: { value: 0 },
        baseColor: { value: params.baseColor },
        emissiveColor: { value: params.emissiveColor },
        opacity: { value: params.opacity },
        enableHologram: { value: params.enableHologram },
        scanlineSpeed: { value: params.scanlineSpeed },
        scanlineCount: { value: params.scanlineCount },
        glowStrength: { value: params.glowStrength },
        rimPower: { value: params.rimPower },
        pulseSpeed: { value: params.pulseSpeed },
        pulseStrength: { value: params.pulseStrength },
        hologramStrength: { value: params.hologramStrength },
      },
      vertexShader: hologramVertexShader,
      fragmentShader: hologramFragmentShader,
      transparent: true,
      side: THREE.DoubleSide,
      depthWrite: true,
      blending: THREE.AdditiveBlending, // Change to additive for more glow
    });
  }

  // Update time uniform for animations
  updateTime(time: number) {
    this.uniforms.time.value = time;
  }

  // Update colors from settings
  updateColors(baseColor: string | THREE.Color, emissiveColor: string | THREE.Color) {
    this.uniforms.baseColor.value = new THREE.Color(baseColor);
    this.uniforms.emissiveColor.value = new THREE.Color(emissiveColor);
  }

  // Toggle hologram effect
  setHologramEnabled(enabled: boolean) {
    this.uniforms.enableHologram.value = enabled;
  }

  // Update hologram parameters
  updateHologramParams(params: {
    scanlineSpeed?: number;
    scanlineCount?: number;
    glowStrength?: number;
    rimPower?: number;
    hologramStrength?: number;
  }) {
    if (params.scanlineSpeed !== undefined) this.uniforms.scanlineSpeed.value = params.scanlineSpeed;
    if (params.scanlineCount !== undefined) this.uniforms.scanlineCount.value = params.scanlineCount;
    if (params.glowStrength !== undefined) this.uniforms.glowStrength.value = params.glowStrength;
    if (params.rimPower !== undefined) this.uniforms.rimPower.value = params.rimPower;
    if (params.hologramStrength !== undefined) this.uniforms.hologramStrength.value = params.hologramStrength;
  }
}

// Extend for use in React Three Fiber
extend({ HologramNodeMaterial });