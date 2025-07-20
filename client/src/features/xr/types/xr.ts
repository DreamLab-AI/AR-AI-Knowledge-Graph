// XR-related type definitions
import { Object3D, Vector3, Quaternion } from 'three'; // Import specific types needed

// XR session state types
export type XRSessionState = 'inactive' | 'active' | 'ending' | 'cooldown';

// XR controller types
export type XRControllerType = 'none' | 'hands' | 'touch' | 'gamepad' | 'gaze';

// XR reference space types
export type XRReferenceSpaceType = 'local' | 'local-floor' | 'bounded-floor' | 'unbounded' | 'viewer';

// XR interaction types
export type XRInteractionType = 'select' | 'grab' | 'drag' | 'scale' | 'rotate' | 'pinch' | 'scroll';

// XR controller/hand interaction mode
export type XRInteractionMode = 'controllers-only' | 'hands-only' | 'both';

// XR interactable object interface
export interface InteractableObject {
  object: Object3D;
  id: string;
  interactable: boolean;
  handlers?: Record<string, (event: any) => void>;
}

// XR hand joint types (standard WebXR hand joints)
export type XRHandJoint = 
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

// XR Input Source types (aligned with WebXR standards)
export interface XRInputSource {
  handedness: 'none' | 'left' | 'right';
  targetRayMode: 'gaze' | 'tracked-pointer' | 'screen';
  targetRaySpace: any; // XRSpace in WebXR
  gripSpace?: any; // XRSpace in WebXR
  gamepad?: Gamepad;
  profiles: string[];
  hand?: any; // XRHand in WebXR
}

// XR hit test result for AR interactions
export interface XRHitTestResult {
  hitMatrix: Float32Array;
  distance: number;
  objectId?: string;
}

// Gesture recognition types for hand tracking
export interface GestureState {
  left: {
    pinch: boolean;
    grip: boolean;
    point: boolean;
    thumbsUp: boolean;
  };
  right: {
    pinch: boolean;
    grip: boolean;
    point: boolean;
    thumbsUp: boolean;
  };
}

// XR controller state
export interface XRControllerState {
  isConnected: boolean;
  isTriggerPressed: boolean;
  isGripPressed: boolean;
  isThumbstickPressed: boolean;
  thumbstickX: number;
  thumbstickY: number;
  triggerValue: number;
  gripValue: number;
  position: Vector3;
  rotation: Quaternion;
  hapticActuator?: any;
}

// XR Settings (moved from settings feature)
export interface XRSettings {
  isAREnabled: boolean;
  interactionMode: 'grab' | 'point' | 'touch';
  scale: number;
  position: [number, number, number];
  rotation: [number, number, number];
  handTrackingEnabled?: boolean; // Added setting for hand tracking
  movementSpeed?: number;
  showFloor?: boolean;
  teleportEnabled?: boolean;
}