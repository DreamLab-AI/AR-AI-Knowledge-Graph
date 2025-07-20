// Extended WebXR type definitions for hand tracking and related features

// Extend existing WebXR types
declare module '@react-three/xr' {
  import { Object3D, Group } from 'three';
  import { ReactNode } from 'react';

  // Event types
  export interface XREvent extends Event {
    target: any;
  }

  export type XRHandedness = 'left' | 'right' | 'none';

  // React hooks
  export function useXR(): {
    player: Group;
    isPresenting: boolean;
    session: XRSession | null;
    controllers: Array<XRController>;
    hands: { left: XRHand, right: XRHand } | null;
    hoverState: any;
  };

  // Interactive component
  export interface InteractiveProps {
    onHover?: (event: any) => void;
    onBlur?: (event: any) => void;
    onSelect?: (event: any) => void;
    onMove?: (event: any) => void;
    onSqueeze?: (event: any) => void;
    onSqueezeEnd?: (event: any) => void;
    onSelectEnd?: (event: any) => void;
    onSelectStart?: (event: any) => void;
    onSqueezeStart?: (event: any) => void;
    children?: ReactNode;
  }

  export const Interactive: React.FC<InteractiveProps>;
  export const Hands: React.FC<any>;
  export const XR: React.FC<any>;
  export const Controllers: React.FC<any>;
  export const useController: (handedness: XRHandedness) => XRController | null;
}

// Extended WebXR types for advanced hand tracking
interface XRHand extends Map<XRHandJoint, XRJointSpace> {
  // Additional methods and properties
  get(joint: XRHandJoint): XRJointSpace | undefined;
  keys(): IterableIterator<XRHandJoint>;
  values(): IterableIterator<XRJointSpace>;
}

interface XRJointSpace extends XRSpace {
  // Remove readonly modifier conflict
  jointName: XRHandJoint;
  space: XRSpace;
  // Remove readonly modifier conflict
  radius: number | undefined;
  pose?: XRPose;
}

// WebXR Hand Joint types
// Rename to avoid conflict with @types/webxr
type XRHandJointType = 
  | 'wrist'
  | 'thumb-metacarpal'
  | 'thumb-phalanx-proximal'
  | 'thumb-phalanx-distal'
  | 'thumb-tip'
  | 'index-finger-metacarpal'
  | 'index-finger-phalanx-proximal'
  | 'index-finger-phalanx-intermediate'
  | 'index-finger-phalanx-distal'
  | 'index-finger-tip'
  | 'middle-finger-metacarpal'
  | 'middle-finger-phalanx-proximal'
  | 'middle-finger-phalanx-intermediate'
  | 'middle-finger-phalanx-distal'
  | 'middle-finger-tip'
  | 'ring-finger-metacarpal'
  | 'ring-finger-phalanx-proximal'
  | 'ring-finger-phalanx-intermediate'
  | 'ring-finger-phalanx-distal'
  | 'ring-finger-tip'
  | 'pinky-finger-metacarpal'
  | 'pinky-finger-phalanx-proximal'
  | 'pinky-finger-phalanx-intermediate'
  | 'pinky-finger-phalanx-distal'
  | 'pinky-finger-tip';

// Extended WebXR session 
interface XRSession {
  // Avoid readonly modifier conflict
  supportedModules?: string[];
  // Match type signature from @types/webxr
  // updateTargetFrameRate(rate: number): Promise<void>;
}

// Extended XRFrame
interface XRFrame {
  // Match type signature from @types/webxr
  // getJointPose(joint: XRJointSpace, baseSpace: XRSpace): XRJointPose | undefined;
}

// Extended XRInputSource
interface XRInputSource {
  // Avoid readonly modifier conflict
}

// Extended controller type
interface XRController {
  grip: THREE.Group;
  controller: THREE.Group;
  inputSource: XRInputSource;
  targetRayMode: 'gaze' | 'tracked-pointer' | 'screen';
}

// Joint pose with radius
interface XRJointPose extends XRPose {
  // Avoid readonly modifier conflict
}

// Additional global interfaces for TS compatibility
interface XRInputSourceArray {
  length: number;
  [Symbol.iterator](): IterableIterator<XRInputSource>;
}