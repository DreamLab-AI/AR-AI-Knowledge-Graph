/**
 * Performance optimizations for dual graph visualization
 * Implements LOD, frustum culling, instanced rendering improvements, and SharedArrayBuffer communication
 */

import * as THREE from 'three';
import { createLogger } from './logger';

const logger = createLogger('DualGraphOptimizations');

// Frustum culling helper
export class FrustumCuller {
  private frustum = new THREE.Frustum();
  private matrix = new THREE.Matrix4();
  
  public updateFrustum(camera: THREE.Camera) {
    this.matrix.multiplyMatrices(camera.projectionMatrix, camera.matrixWorldInverse);
    this.frustum.setFromProjectionMatrix(this.matrix);
  }
  
  public isNodeVisible(position: THREE.Vector3, radius: number = 1): boolean {
    const sphere = new THREE.Sphere(position, radius);
    return this.frustum.intersectsSphere(sphere);
  }
  
  public cullNodes(nodes: Array<{ position: THREE.Vector3; radius?: number }>) {
    return nodes.filter(node => this.isNodeVisible(node.position, node.radius));
  }
}

// Level of Detail (LOD) manager
export class LODManager {
  private camera: THREE.Camera | null = null;
  
  constructor(camera?: THREE.Camera) {
    this.camera = camera || null;
  }
  
  public setCamera(camera: THREE.Camera) {
    this.camera = camera;
  }
  
  public getLODLevel(nodePosition: THREE.Vector3): 'high' | 'medium' | 'low' | 'hidden' {
    if (!this.camera) return 'high';
    
    const distance = this.camera.position.distanceTo(nodePosition);
    
    if (distance < 20) return 'high';
    if (distance < 50) return 'medium';
    if (distance < 100) return 'low';
    return 'hidden';
  }
  
  public getGeometryForLOD(level: 'high' | 'medium' | 'low'): THREE.BufferGeometry {
    switch (level) {
      case 'high':
        return new THREE.SphereGeometry(0.5, 32, 32);
      case 'medium':
        return new THREE.SphereGeometry(0.5, 16, 16);
      case 'low':
        return new THREE.SphereGeometry(0.5, 8, 8);
      default:
        return new THREE.SphereGeometry(0.5, 8, 8);
    }
  }
  
  public shouldRenderNode(nodePosition: THREE.Vector3, minDistance: number = 150): boolean {
    if (!this.camera) return true;
    return this.camera.position.distanceTo(nodePosition) < minDistance;
  }
}

// Enhanced instanced rendering manager
export class InstancedRenderingManager {
  private geometryPool = new Map<string, THREE.BufferGeometry>();
  private materialPool = new Map<string, THREE.Material>();
  private maxInstances: number;
  
  constructor(maxInstances: number = 5000) {
    this.maxInstances = maxInstances;
  }
  
  public createInstancedMesh(
    geometryKey: string,
    materialKey: string,
    instanceCount: number,
    geometryFactory: () => THREE.BufferGeometry,
    materialFactory: () => THREE.Material
  ): THREE.InstancedMesh {
    // Get or create geometry
    if (!this.geometryPool.has(geometryKey)) {
      const geometry = geometryFactory();
      this.geometryPool.set(geometryKey, geometry);
    }
    
    // Get or create material
    if (!this.materialPool.has(materialKey)) {
      const material = materialFactory();
      this.materialPool.set(materialKey, material);
    }
    
    const geometry = this.geometryPool.get(geometryKey)!;
    const material = this.materialPool.get(materialKey)!;
    
    const count = Math.min(instanceCount, this.maxInstances);
    const mesh = new THREE.InstancedMesh(geometry, material, count);
    
    // Enable automatic frustum culling per instance
    mesh.frustumCulled = true;
    
    return mesh;
  }
  
  public updateInstancedMesh(
    mesh: THREE.InstancedMesh,
    nodes: Array<{ position: THREE.Vector3; scale?: number; color?: THREE.Color }>,
    lodManager?: LODManager,
    frustumCuller?: FrustumCuller
  ): { renderedCount: number; culledCount: number } {
    const matrix = new THREE.Matrix4();
    const color = new THREE.Color();
    let renderedCount = 0;
    let culledCount = 0;
    
    // Create instance color attribute if it doesn't exist
    if (!mesh.geometry.attributes.instanceColor) {
      const colors = new Float32Array(mesh.count * 3);
      mesh.geometry.setAttribute('instanceColor', new THREE.InstancedBufferAttribute(colors, 3));
    }
    
    const colorAttribute = mesh.geometry.attributes.instanceColor as THREE.InstancedBufferAttribute;
    
    for (let i = 0; i < Math.min(nodes.length, mesh.count); i++) {
      const node = nodes[i];
      
      // Check if node should be rendered based on LOD and frustum culling
      let shouldRender = true;
      
      if (lodManager && !lodManager.shouldRenderNode(node.position)) {
        shouldRender = false;
      }
      
      if (frustumCuller && !frustumCuller.isNodeVisible(node.position)) {
        shouldRender = false;
      }
      
      if (shouldRender) {
        // Set transformation matrix
        const scale = node.scale || 1;
        matrix.makeScale(scale, scale, scale);
        matrix.setPosition(node.position);
        mesh.setMatrixAt(i, matrix);
        
        // Set color
        const nodeColor = node.color || new THREE.Color(0x00ffff);
        colorAttribute.setXYZ(i, nodeColor.r, nodeColor.g, nodeColor.b);
        
        renderedCount++;
      } else {
        // Hide instance by scaling to zero
        matrix.makeScale(0, 0, 0);
        matrix.setPosition(0, 0, 0);
        mesh.setMatrixAt(i, matrix);
        culledCount++;
      }
    }
    
    // Update instance matrices and colors
    mesh.instanceMatrix.needsUpdate = true;
    colorAttribute.needsUpdate = true;
    mesh.count = renderedCount;
    
    return { renderedCount, culledCount };
  }
  
  public dispose() {
    this.geometryPool.forEach(geometry => geometry.dispose());
    this.materialPool.forEach(material => material.dispose());
    this.geometryPool.clear();
    this.materialPool.clear();
  }
}

// SharedArrayBuffer communication for workers (if supported)
export class SharedBufferCommunication {
  private sharedBuffer: SharedArrayBuffer | null = null;
  private positionArray: Float32Array | null = null;
  private metadataArray: Int32Array | null = null;
  private supported: boolean;
  
  constructor() {
    this.supported = typeof SharedArrayBuffer !== 'undefined';
    
    if (!this.supported) {
      logger.warn('SharedArrayBuffer not supported, falling back to message passing');
    }
  }
  
  public isSupported(): boolean {
    return this.supported;
  }
  
  public initializeBuffer(nodeCount: number): boolean {
    if (!this.supported) return false;
    
    try {
      // Allocate space for positions (3 floats per node) + metadata (1 int per node)
      const positionBytes = nodeCount * 3 * 4; // 3 floats * 4 bytes
      const metadataBytes = nodeCount * 4; // 1 int * 4 bytes
      const totalBytes = positionBytes + metadataBytes;
      
      this.sharedBuffer = new SharedArrayBuffer(totalBytes);
      
      // Create typed array views
      this.positionArray = new Float32Array(this.sharedBuffer, 0, nodeCount * 3);
      this.metadataArray = new Int32Array(this.sharedBuffer, positionBytes, nodeCount);
      
      logger.info(`SharedArrayBuffer initialized: ${totalBytes} bytes for ${nodeCount} nodes`);
      return true;
    } catch (error) {
      logger.error('Failed to initialize SharedArrayBuffer:', error);
      return false;
    }
  }
  
  public getSharedBuffer(): SharedArrayBuffer | null {
    return this.sharedBuffer;
  }
  
  public getPositionArray(): Float32Array | null {
    return this.positionArray;
  }
  
  public getMetadataArray(): Int32Array | null {
    return this.metadataArray;
  }
  
  public updatePositions(positions: Float32Array): boolean {
    if (!this.positionArray || positions.length > this.positionArray.length) {
      return false;
    }
    
    this.positionArray.set(positions);
    return true;
  }
  
  public updateMetadata(metadata: Int32Array): boolean {
    if (!this.metadataArray || metadata.length > this.metadataArray.length) {
      return false;
    }
    
    this.metadataArray.set(metadata);
    return true;
  }
  
  public dispose() {
    this.sharedBuffer = null;
    this.positionArray = null;
    this.metadataArray = null;
  }
}

// Spatial partitioning using Octree for large node counts
export class SpatialOctree {
  private root: OctreeNode | null = null;
  private bounds: THREE.Box3;
  private maxDepth: number;
  private maxObjectsPerNode: number;
  
  constructor(bounds: THREE.Box3, maxDepth: number = 8, maxObjectsPerNode: number = 10) {
    this.bounds = bounds.clone();
    this.maxDepth = maxDepth;
    this.maxObjectsPerNode = maxObjectsPerNode;
  }
  
  public insert(object: { position: THREE.Vector3; id: string; [key: string]: any }) {
    if (!this.root) {
      this.root = new OctreeNode(this.bounds, 0);
    }
    
    this.root.insert(object, this.maxDepth, this.maxObjectsPerNode);
  }
  
  public query(frustum: THREE.Frustum): Array<{ position: THREE.Vector3; id: string; [key: string]: any }> {
    if (!this.root) return [];
    
    const results: Array<{ position: THREE.Vector3; id: string; [key: string]: any }> = [];
    this.root.query(frustum, results);
    return results;
  }
  
  public queryRadius(center: THREE.Vector3, radius: number): Array<{ position: THREE.Vector3; id: string; [key: string]: any }> {
    if (!this.root) return [];
    
    const results: Array<{ position: THREE.Vector3; id: string; [key: string]: any }> = [];
    const sphere = new THREE.Sphere(center, radius);
    this.root.queryRadius(sphere, results);
    return results;
  }
  
  public clear() {
    this.root = null;
  }
}

class OctreeNode {
  private bounds: THREE.Box3;
  private depth: number;
  private objects: Array<{ position: THREE.Vector3; id: string; [key: string]: any }> = [];
  private children: OctreeNode[] | null = null;
  
  constructor(bounds: THREE.Box3, depth: number) {
    this.bounds = bounds.clone();
    this.depth = depth;
  }
  
  public insert(object: { position: THREE.Vector3; id: string; [key: string]: any }, maxDepth: number, maxObjects: number) {
    if (!this.bounds.containsPoint(object.position)) {
      return false;
    }
    
    if (this.objects.length < maxObjects || this.depth >= maxDepth) {
      this.objects.push(object);
      return true;
    }
    
    if (!this.children) {
      this.subdivide();
    }
    
    for (const child of this.children!) {
      if (child.insert(object, maxDepth, maxObjects)) {
        return true;
      }
    }
    
    // If no child could contain it, store in this node
    this.objects.push(object);
    return true;
  }
  
  public query(frustum: THREE.Frustum, results: Array<{ position: THREE.Vector3; id: string; [key: string]: any }>) {
    if (!frustum.intersectsBox(this.bounds)) {
      return;
    }
    
    for (const object of this.objects) {
      results.push(object);
    }
    
    if (this.children) {
      for (const child of this.children) {
        child.query(frustum, results);
      }
    }
  }
  
  public queryRadius(sphere: THREE.Sphere, results: Array<{ position: THREE.Vector3; id: string; [key: string]: any }>) {
    if (!sphere.intersectsBox(this.bounds)) {
      return;
    }
    
    for (const object of this.objects) {
      if (sphere.containsPoint(object.position)) {
        results.push(object);
      }
    }
    
    if (this.children) {
      for (const child of this.children) {
        child.queryRadius(sphere, results);
      }
    }
  }
  
  private subdivide() {
    const center = this.bounds.getCenter(new THREE.Vector3());
    const min = this.bounds.min;
    const max = this.bounds.max;
    
    this.children = [
      new OctreeNode(new THREE.Box3(new THREE.Vector3(min.x, min.y, min.z), new THREE.Vector3(center.x, center.y, center.z)), this.depth + 1),
      new OctreeNode(new THREE.Box3(new THREE.Vector3(center.x, min.y, min.z), new THREE.Vector3(max.x, center.y, center.z)), this.depth + 1),
      new OctreeNode(new THREE.Box3(new THREE.Vector3(min.x, center.y, min.z), new THREE.Vector3(center.x, max.y, center.z)), this.depth + 1),
      new OctreeNode(new THREE.Box3(new THREE.Vector3(center.x, center.y, min.z), new THREE.Vector3(max.x, max.y, center.z)), this.depth + 1),
      new OctreeNode(new THREE.Box3(new THREE.Vector3(min.x, min.y, center.z), new THREE.Vector3(center.x, center.y, max.z)), this.depth + 1),
      new OctreeNode(new THREE.Box3(new THREE.Vector3(center.x, min.y, center.z), new THREE.Vector3(max.x, center.y, max.z)), this.depth + 1),
      new OctreeNode(new THREE.Box3(new THREE.Vector3(min.x, center.y, center.z), new THREE.Vector3(center.x, max.y, max.z)), this.depth + 1),
      new OctreeNode(new THREE.Box3(new THREE.Vector3(center.x, center.y, center.z), new THREE.Vector3(max.x, max.y, max.z)), this.depth + 1),
    ];
  }
}

// Performance optimization helper
export class DualGraphOptimizer {
  private frustumCuller = new FrustumCuller();
  private lodManager = new LODManager();
  private instancedManager = new InstancedRenderingManager();
  private sharedBuffer = new SharedBufferCommunication();
  private octree: SpatialOctree | null = null;
  
  public initializeOptimizations(camera: THREE.Camera, renderer: THREE.WebGLRenderer) {
    this.lodManager.setCamera(camera);
    
    // Initialize spatial partitioning for large graphs
    const bounds = new THREE.Box3(
      new THREE.Vector3(-100, -100, -100),
      new THREE.Vector3(100, 100, 100)
    );
    this.octree = new SpatialOctree(bounds);
    
    logger.info('Dual graph optimizations initialized', {
      sharedBufferSupported: this.sharedBuffer.isSupported(),
      rendererCapabilities: {
        maxTextures: renderer.capabilities.maxTextures,
        maxVertexUniforms: renderer.capabilities.maxVertexUniforms,
        maxFragmentUniforms: renderer.capabilities.maxFragmentUniforms
      }
    });
  }
  
  public optimizeFrame(camera: THREE.Camera) {
    this.frustumCuller.updateFrustum(camera);
    this.lodManager.setCamera(camera);
  }
  
  public getFrustumCuller(): FrustumCuller {
    return this.frustumCuller;
  }
  
  public getLODManager(): LODManager {
    return this.lodManager;
  }
  
  public getInstancedManager(): InstancedRenderingManager {
    return this.instancedManager;
  }
  
  public getSharedBuffer(): SharedBufferCommunication {
    return this.sharedBuffer;
  }
  
  public getOctree(): SpatialOctree | null {
    return this.octree;
  }
  
  public dispose() {
    this.instancedManager.dispose();
    this.sharedBuffer.dispose();
    this.octree?.clear();
  }
}

// Singleton instance
export const dualGraphOptimizer = new DualGraphOptimizer();