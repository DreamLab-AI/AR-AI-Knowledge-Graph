// // Extend the THREE namespace with WebXR types that are missing from @types/three
// declare module 'three' {
//   // Import the THREE namespace itself to extend it
//   import * as THREE from 'three';
//
//   /**
//    * XRTargetRaySpace represents the space in which the target ray is positioned.
//    * This interface extends Object3D, allowing it to be used in the Three.js scene graph.
//    */
//   export interface XRTargetRaySpace extends Object3D {
//     // Add any specific properties or methods needed
//   }
//   
//   // WebGL renderer needs XR properties
//   export interface WebGLRenderer {
//     xr: {
//       enabled: boolean;
//       setReferenceSpaceType: (type: string) => void;
//       // Add other XR-related properties and methods as needed
//     };
//     setClearColor: (color: Color | string | number, alpha?: number) => void;
//   }
//
//   // Make sure all THREE exports are accessible
//   export namespace THREE {
//     export type XRTargetRaySpace = XRTargetRaySpace;
//     
//     // Core THREE classes
//     export type Vector2 = THREE.Vector2;
//     export type Vector3 = THREE.Vector3;
//     export type Color = THREE.Color;
//     export type Quaternion = THREE.Quaternion;
//     export type Group = THREE.Group;
//     export type Line = THREE.Line;
//     export type Material = THREE.Material;
//     export type Object3D = THREE.Object3D;
//     export type InstancedMesh = THREE.InstancedMesh;
//     export type Raycaster = THREE.Raycaster;
//     
//     // Geometries
//     export type BufferGeometry = THREE.BufferGeometry;
//     export type SphereGeometry = THREE.SphereGeometry;
//     
//     // Materials
//     export type MeshStandardMaterial = THREE.MeshStandardMaterial;
//     export type MeshBasicMaterial = THREE.MeshBasicMaterial;
//     export type MeshPhysicalMaterial = THREE.MeshPhysicalMaterial;
//     export type LineBasicMaterial = THREE.LineBasicMaterial;
//     
//     // Lights
//     export type AmbientLight = THREE.AmbientLight;
//     export type DirectionalLight = THREE.DirectionalLight;
//     
//     // Cameras
//     export type PerspectiveCamera = THREE.PerspectiveCamera;
//     
//     // Constants
//     export const FrontSide: typeof THREE.FrontSide;
//     export const BackSide: typeof THREE.BackSide;
//     export const DoubleSide: typeof THREE.DoubleSide;
//   }
//
//   // Make necessary Raycaster properties available
//   export interface Raycaster {
//     near: number;
//     far: number;
//     params: {
//       Line?: { threshold: number };
//       Points?: { threshold: number };
//       [key: string]: any;
//     };
//     intersectObjects: (objects: Object3D[], recursive?: boolean) => Intersection[];
//     ray: {
//       origin: Vector3;
//       direction: Vector3;
//     };
//   }
//
//   // Extend Object3D with properties used in the codebase
//   export interface Object3D {
//     position: Vector3;
//     name: string;
//   }
//
//   // Additional types for intersection testing
//   export interface Intersection {
//     distance: number;
//     point: Vector3;
//     object: Object3D;
//     // Add other intersection properties as needed
//   }
// }
//
// // Extend React Three Fiber component props
// declare module '@react-three/fiber' {
//   interface MeshProps {
//     color?: any;
//     side?: any;
//   }
// }