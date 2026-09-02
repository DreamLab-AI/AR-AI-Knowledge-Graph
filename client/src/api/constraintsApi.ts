// frontend/src/api/constraintsApi.ts
// REAL API client for constraints management - NO MOCKS
// Auth handled by global axios interceptor in settingsApi.ts

import axios, { AxiosResponse } from 'axios';

const API_BASE = '/api';

// ============================================================================
// Type Definitions (matching Rust backend)
// ============================================================================

export type ConstraintKind =
  | 'FixedPosition'
  | 'Separation'
  | 'AlignmentHorizontal'
  | 'AlignmentVertical'
  | 'AlignmentDepth'
  | 'Clustering'
  | 'Boundary'
  | 'DirectionalFlow'
  | 'RadialDistance'
  | 'LayerDepth';

export interface Constraint {
  kind: ConstraintKind;
  nodeIndices: number[];
  params: number[];
  weight: number;
  active: boolean;
}

export interface ConstraintSystem {
  separation: LegacyConstraintData;
  boundary: LegacyConstraintData;
  alignment: LegacyConstraintData;
  cluster: LegacyConstraintData;
}

export interface LegacyConstraintData {
  constraintType: number;
  strength: number;
  param1: number;
  param2: number;
}

export interface ConstraintStats {
  total: number;
  enabled: number;
  userDefined: number;
  avgStrength: number;
}

export interface ConstraintListResponse {
  constraints: any[];
  count: number;
  dataSource: string;
  gpuAvailable: boolean;
  modes?: {
    knowledgeComputeMode: number;
    visionclawComputeMode: number;
  };
}

export interface ApplyConstraintRequest {
  constraintType: 'separation' | 'boundary' | 'alignment' | 'cluster';
  nodeIds: number[];
  strength?: number;
}

export interface RemoveConstraintRequest {
  constraintType?: string;
  nodeIds?: number[];
}

export interface ValidationResponse {
  valid: boolean;
  message?: string;
  error?: string;
}

// ============================================================================
// API Client
// ============================================================================

export const constraintsApi = {
  
  define: (
    system: ConstraintSystem
  ): Promise<AxiosResponse<{ status: string; constraints: ConstraintSystem }>> =>
    axios.post(`${API_BASE}/constraints/define`, system),


  apply: (
    request: ApplyConstraintRequest
  ): Promise<
    AxiosResponse<{
      status: string;
      constraintType: string;
      nodeCount: number;
      strength: number;
      gpuAvailable: boolean;
    }>
  > => axios.post(`${API_BASE}/constraints/apply`, request),


  remove: (
    request: RemoveConstraintRequest
  ): Promise<
    AxiosResponse<{
      status: string;
      removedCount: number;
      gpuAvailable: boolean;
    }>
  > => axios.post(`${API_BASE}/constraints/remove`, request),


  list: (): Promise<AxiosResponse<ConstraintListResponse>> =>
    axios.get(`${API_BASE}/constraints/list`),


  validate: (
    constraint: LegacyConstraintData
  ): Promise<AxiosResponse<ValidationResponse>> =>
    axios.post(`${API_BASE}/constraints/validate`, constraint),
};

// ============================================================================
// Helper Functions
// ============================================================================

export const createConstraintSystem = (
  overrides?: Partial<ConstraintSystem>
): ConstraintSystem => {
  return {
    separation: {
      constraintType: 1,
      strength: 0.5,
      param1: 50.0,
      param2: 0.0,
    },
    boundary: {
      constraintType: 2,
      strength: 0.7,
      param1: 500.0,
      param2: 500.0,
    },
    alignment: {
      constraintType: 3,
      strength: 0.3,
      param1: 0.0,
      param2: 0.0,
    },
    cluster: {
      constraintType: 4,
      strength: 0.6,
      param1: 0.0,
      param2: 0.0,
    },
    ...overrides,
  };
};

export const validateConstraintStrength = (strength: number): boolean => {
  return strength >= 0.0 && strength <= 10.0;
};

export const validateConstraintType = (type: number): boolean => {
  return type >= 0 && type <= 4;
};

export const getConstraintTypeName = (type: number): string => {
  const names = ['None', 'Separation', 'Boundary', 'Alignment', 'Cluster'];
  return names[type] || 'Unknown';
};

export const clampStrength = (strength: number): number => {
  return Math.max(0.0, Math.min(10.0, strength));
};
