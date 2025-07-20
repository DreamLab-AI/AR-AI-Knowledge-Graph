import { createLogger, createErrorMetadata } from '../utils/logger';
import { debugState } from '../utils/debugState';
import { RagflowChatRequestPayload, RagflowChatResponsePayload } from '../types/ragflowTypes';

const logger = createLogger('ApiService');

/**
 * API Service for making requests to the backend
 */
class ApiService {
  private static instance: ApiService;
  private baseUrl: string;

  private constructor() {
    this.baseUrl = '/api';
  }

  public static getInstance(): ApiService {
    if (!ApiService.instance) {
      ApiService.instance = new ApiService();
    }
    return ApiService.instance;
  }

  /**
   * Set the base URL for API requests
   * @param url The new base URL
   */
  public setBaseUrl(url: string): void {
    this.baseUrl = url;
    logger.info(`API base URL set to: ${url}`);
  }

  /**
   * Get the current base URL
   */
  public getBaseUrl(): string {
    return this.baseUrl;
  }

  /**
   * Make a GET request to the API
   * @param endpoint The API endpoint
   * @param headers Optional request headers
   * @returns The response data
   */
  public async get<T>(endpoint: string, headers: Record<string, string> = {}): Promise<T> {
    try {
      const url = `${this.baseUrl}${endpoint}`;

      if (debugState.isEnabled()) {
        logger.debug(`Making GET request to ${url}`);
      }

      const response = await fetch(url, {
        method: 'GET',
        headers: {
          'Content-Type': 'application/json',
          ...headers
        }
      });

      if (!response.ok) {
        // Check if this is the graph data endpoint and provide mock data
        if (endpoint === '/graph/data') {
          logger.warn('Backend not available, returning mock graph data');
          return this.getMockGraphData() as T;
        }
        // Check if this is the swarm data endpoint and provide mock data
        if (endpoint === '/swarm/data') {
          logger.warn('Backend not available, returning mock swarm data');
          return this.getMockSwarmData() as T;
        }
        throw new Error(`API request failed with status ${response.status}: ${response.statusText}`);
      }

      const data = await response.json();

      if (debugState.isEnabled()) {
        logger.debug(`GET request to ${endpoint} succeeded`);
      }

      return data;
    } catch (error) {
      // If fetch itself failed (network error), check for graph data endpoint
      if (endpoint === '/graph/data') {
        logger.warn('Backend not available, returning mock graph data');
        return this.getMockGraphData() as T;
      }
      // Check if this is the swarm data endpoint and provide mock data
      if (endpoint === '/swarm/data') {
        logger.warn('Backend not available, returning mock swarm data');
        return this.getMockSwarmData() as T;
      }
      logger.error(`GET request to ${endpoint} failed:`, createErrorMetadata(error));
      throw error;
    }
  }

  /**
   * Get mock swarm data for development when backend is unavailable
   */
  private getMockSwarmData(): any {
    const mockNodes = [
      { id: 'agent-1', type: 'coordinator', status: 'active', name: 'Coordinator Alpha', cpuUsage: 45, health: 95, workload: 0.7 },
      { id: 'agent-2', type: 'coder', status: 'active', name: 'Coder Beta', cpuUsage: 78, health: 88, workload: 0.9 },
      { id: 'agent-3', type: 'tester', status: 'active', name: 'Tester Gamma', cpuUsage: 32, health: 92, workload: 0.5 },
      { id: 'agent-4', type: 'analyst', status: 'active', name: 'Analyst Delta', cpuUsage: 56, health: 90, workload: 0.6 },
      { id: 'agent-5', type: 'researcher', status: 'active', name: 'Researcher Epsilon', cpuUsage: 41, health: 94, workload: 0.4 },
      { id: 'agent-6', type: 'architect', status: 'active', name: 'Architect Zeta', cpuUsage: 62, health: 91, workload: 0.8 },
      { id: 'agent-7', type: 'reviewer', status: 'active', name: 'Reviewer Eta', cpuUsage: 28, health: 96, workload: 0.3 },
      { id: 'agent-8', type: 'optimizer', status: 'active', name: 'Optimizer Theta', cpuUsage: 85, health: 85, workload: 0.95 }
    ];

    const mockEdges = [
      { id: 'edge-1', source: 'agent-1', target: 'agent-2', dataVolume: 1024, messageCount: 15, lastMessageTime: Date.now() },
      { id: 'edge-2', source: 'agent-1', target: 'agent-3', dataVolume: 512, messageCount: 8, lastMessageTime: Date.now() },
      { id: 'edge-3', source: 'agent-2', target: 'agent-4', dataVolume: 2048, messageCount: 22, lastMessageTime: Date.now() },
      { id: 'edge-4', source: 'agent-3', target: 'agent-5', dataVolume: 768, messageCount: 11, lastMessageTime: Date.now() },
      { id: 'edge-5', source: 'agent-4', target: 'agent-6', dataVolume: 1536, messageCount: 18, lastMessageTime: Date.now() },
      { id: 'edge-6', source: 'agent-5', target: 'agent-7', dataVolume: 384, messageCount: 6, lastMessageTime: Date.now() },
      { id: 'edge-7', source: 'agent-6', target: 'agent-8', dataVolume: 896, messageCount: 13, lastMessageTime: Date.now() },
      { id: 'edge-8', source: 'agent-7', target: 'agent-1', dataVolume: 640, messageCount: 9, lastMessageTime: Date.now() }
    ];

    // Initialize positions in a circle formation
    const positions = new Float32Array(mockNodes.length * 3);
    mockNodes.forEach((_, index) => {
      const angle = (index / mockNodes.length) * Math.PI * 2;
      const radius = 15;
      positions[index * 3] = Math.cos(angle) * radius;
      positions[index * 3 + 1] = Math.sin(angle) * radius;
      positions[index * 3 + 2] = (Math.random() - 0.5) * 10;
    });

    return {
      nodes: mockNodes,
      edges: mockEdges,
      positions: Array.from(positions)
    };
  }

  /**
   * Get mock graph data for development when backend is unavailable
   */
  private getMockGraphData(): any {
    return {
      nodes: [
        {
          id: "1",
          metadataId: "Visionflow Development",
          label: "Visionflow Development",
          data: {
            position: { x: 0, y: 0, z: 0 },
            velocity: { x: 0, y: 0, z: 0 },
            mass: 20,
            flags: 1,
            padding: [0, 0]
          },
          metadata: {
            metadataId: "Visionflow Development",
            name: "Visionflow Development",
            nodeSize: "20",
            lastModified: new Date().toISOString(),
            fileSize: "1000",
            fileName: "development.md",
            hyperlinkCount: "5"
          },
          size: 20
        },
        {
          id: "2",
          metadataId: "Agent Swarm Visualization",
          label: "Agent Swarm Visualization",
          data: {
            position: { x: 50, y: 0, z: 0 },
            velocity: { x: 0, y: 0, z: 0 },
            mass: 15,
            flags: 1,
            padding: [0, 0]
          },
          metadata: {
            metadataId: "Agent Swarm Visualization",
            name: "Agent Swarm Visualization",
            nodeSize: "15",
            lastModified: new Date().toISOString(),
            fileSize: "800",
            fileName: "swarm-viz.md",
            hyperlinkCount: "3"
          },
          size: 15
        }
      ],
      edges: [
        {
          id: "1-2",
          source: "1",
          target: "2",
          data: {
            weight: 1
          }
        }
      ]
    };
  }

  /**
   * Make a POST request to the API
   * @param endpoint The API endpoint
   * @param data The request body data
   * @param headers Optional request headers
   * @returns The response data
   */
  public async post<T>(endpoint: string, data: any, headers: Record<string, string> = {}): Promise<T> {
    try {
      const url = `${this.baseUrl}${endpoint}`;

      if (debugState.isEnabled()) {
        logger.debug(`Making POST request to ${url}`);
      }

      const response = await fetch(url, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          ...headers
        },
        body: JSON.stringify(data)
      });

      if (!response.ok) {
        throw new Error(`API request failed with status ${response.status}: ${response.statusText}`);
      }

      const responseData = await response.json();

      if (debugState.isEnabled()) {
        logger.debug(`POST request to ${endpoint} succeeded`);
      }

      return responseData;
    } catch (error) {
      logger.error(`POST request to ${endpoint} failed:`, createErrorMetadata(error));
      throw error;
    }
  }

  /**
   * Make a PUT request to the API
   * @param endpoint The API endpoint
   * @param data The request body data
   * @param headers Optional request headers
   * @returns The response data
   */
  public async put<T>(endpoint: string, data: any, headers: Record<string, string> = {}): Promise<T> {
    try {
      const url = `${this.baseUrl}${endpoint}`;

      if (debugState.isEnabled()) {
        logger.debug(`Making PUT request to ${url}`);
      }

      const response = await fetch(url, {
        method: 'PUT',
        headers: {
          'Content-Type': 'application/json',
          ...headers
        },
        body: JSON.stringify(data)
      });

      if (!response.ok) {
        throw new Error(`API request failed with status ${response.status}: ${response.statusText}`);
      }

      const responseData = await response.json();

      if (debugState.isEnabled()) {
        logger.debug(`PUT request to ${endpoint} succeeded`);
      }

      return responseData;
    } catch (error) {
      logger.error(`PUT request to ${endpoint} failed:`, createErrorMetadata(error));
      throw error;
    }
  }

  /**
   * Make a DELETE request to the API
   * @param endpoint The API endpoint
   * @param headers Optional request headers
   * @returns The response data
   */
  public async delete<T>(endpoint: string, headers: Record<string, string> = {}): Promise<T> {
    try {
      const url = `${this.baseUrl}${endpoint}`;

      if (debugState.isEnabled()) {
        logger.debug(`Making DELETE request to ${url}`);
      }

      const response = await fetch(url, {
        method: 'DELETE',
        headers: {
          'Content-Type': 'application/json',
          ...headers
        }
      });

      if (!response.ok) {
        throw new Error(`API request failed with status ${response.status}: ${response.statusText}`);
      }

      const data = await response.json();

      if (debugState.isEnabled()) {
        logger.debug(`DELETE request to ${endpoint} succeeded`);
      }

      return data;
    } catch (error) {
      logger.error(`DELETE request to ${endpoint} failed:`, createErrorMetadata(error));
      throw error;
    }
  }

  public async sendRagflowChatMessage(
    payload: RagflowChatRequestPayload,
    headers: Record<string, string> = {} // For auth
  ): Promise<RagflowChatResponsePayload> {
    try {
      const url = `${this.baseUrl}/ragflow/chat`; // Path defined in Rust backend, /api is prepended by baseUrl
      if (debugState.isEnabled()) {
        logger.debug(`Making POST request to ${url} for RAGFlow chat`);
      }
      // Assume headers (like auth) will be passed in or handled globally by apiService
      const response = await fetch(url, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          ...headers, // Include auth headers
        },
        body: JSON.stringify(payload),
      });
      if (!response.ok) {
            const errorData = await response.json().catch(() => ({ message: response.statusText, error: response.statusText }));
            throw new Error(`RAGFlow chat API request failed with status ${response.status}: ${errorData.error || errorData.message}`);
          }
          const responseData = await response.json();
          if (debugState.isEnabled()) {
            logger.debug(`POST request to ${url} (RAGFlow chat) succeeded`);
          }
          return responseData as RagflowChatResponsePayload;
        } catch (error) {
          logger.error(`POST request to /ragflow/chat failed:`, createErrorMetadata(error));
          throw error;
        }
      }

  /**
   * Get swarm data from the backend API
   */
  public async getSwarmData(): Promise<any> {
    try {
      return await this.get('/swarm/data');
    } catch (error) {
      logger.warn('Failed to get swarm data from backend, using mock data', error);
      return this.getMockSwarmData();
    }
  }

  /**
   * Update swarm data on the backend
   */
  public async updateSwarmData(data: { nodes: any[], edges: any[] }): Promise<any> {
    return await this.post('/swarm/update', data);
  }
}

export const apiService = ApiService.getInstance();
