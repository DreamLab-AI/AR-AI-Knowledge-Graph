/**
 * Solid Pod Service
 *
 * Orchestration entry-point for VisionClaw's Solid Pod integration.
 * Delegates all protocol-level work to focused sub-modules:
 *
 *   solidPod/ldpClient.ts        — LDP CRUD, auth headers, path helpers
 *   solidPod/podNotifications.ts — WebSocket notifications (solid-0.1)
 *   solidPod/wacManager.ts       — WAC ACL construction and write
 *   solidPod/agentMemory.ts      — Per-agent memory CRUD with WAC isolation
 *   solidPod/typeIndex.ts        — Solid Type Index discovery (Phase 3)
 */

import { createLogger } from '../utils/loggerConfig';

import {
  SOLID_POD_BASE_URL,
  JsonLdDocument,
  sanitizePreferenceKey,
  resolvePath,
  fetchWithAuth,
  fetchJsonLd,
  fetchTurtle,
  putResource,
  postResource,
  deleteResource,
  resourceExists,
  ensureJsonLd,
} from './solidPod/ldpClient';

import {
  podNotificationManager,
  JSS_WS_URL,
  SolidNotification,
} from './solidPod/podNotifications';

import {
  TypeRegistration,
  TypeIndexDocument,
  DiscoveredView,
  DiscoveredAgent,
  ensurePublicTypeIndex as _ensurePublicTypeIndex,
  linkTypeIndexFromProfile,
  registerViewInTypeIndex as _registerViewInTypeIndex,
  registerAgentInTypeIndex as _registerAgentInTypeIndex,
  discoverSharedViews as _discoverSharedViews,
  discoverAgents as _discoverAgents,
} from './solidPod/typeIndex';

import {
  agentMemoryContainerPath,
  storeAgentMemory as _storeAgentMemory,
  listAgentMemories as _listAgentMemories,
  getAgentMemory as _getAgentMemory,
  deleteAgentMemory as _deleteAgentMemory,
  setAgentMemoryAccess as _setAgentMemoryAccess,
} from './solidPod/agentMemory';

// Re-export types consumed by other modules
export type {
  JsonLdDocument,
  SolidNotification,
  TypeRegistration,
  TypeIndexDocument,
  DiscoveredView,
  DiscoveredAgent,
};

const logger = createLogger('SolidPodService');

// ---------------------------------------------------------------------------
// Pod-level interfaces
// ---------------------------------------------------------------------------

export interface PodStructure {
  profile: string;
  ontology_contributions: string;
  ontology_proposals: string;
  ontology_annotations: string;
  preferences: string;
  inbox: string;
}

export interface PodInfo {
  exists: boolean;
  podUrl?: string;
  webId?: string;
  suggestedUrl?: string;
  structure?: PodStructure;
}

export interface PodCreationResult {
  success: boolean;
  podUrl?: string;
  webId?: string;
  created?: boolean;
  structure?: PodStructure;
  error?: string;
}

export interface PodInitResult {
  success: boolean;
  podUrl?: string;
  webId?: string;
  created: boolean;
  structure?: PodStructure;
  npub?: string;
  error?: string;
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

class SolidPodService {
  private static instance: SolidPodService;
  // ADR-2100: the ONE JSS notification socket, shared with the Zustand
  // WebSocket store. Not a private instance — a second one would recreate the
  // duplicate-client divergence this consolidated.
  private readonly notifications = podNotificationManager;
  private lastKnownPreferencesPath: string | null = null;

  private constructor() {}

  public static getInstance(): SolidPodService {
    if (!SolidPodService.instance) {
      SolidPodService.instance = new SolidPodService();
    }
    return SolidPodService.instance;
  }

  // =========================================================================
  // Pod lifecycle
  // =========================================================================

  public async checkPodExists(): Promise<PodInfo> {
    try {
      const response = await fetchWithAuth(`${SOLID_POD_BASE_URL}/pods/check`);
      if (!response.ok) throw new Error(`Failed to check pod: ${response.status}`);
      return await response.json();
    } catch (error) {
      logger.error('Failed to check pod existence', { error });
      return { exists: false };
    }
  }

  public async createPod(name?: string): Promise<PodCreationResult> {
    try {
      const response = await fetchWithAuth(`${SOLID_POD_BASE_URL}/pods`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name }),
      });
      if (!response.ok) {
        const error = await response.json();
        return { success: false, error: error.error || 'Pod creation failed' };
      }
      const result = await response.json();
      logger.info('Pod created', { podUrl: result.pod_url });
      return { success: true, podUrl: result.pod_url, webId: result.webid };
    } catch (error) {
      logger.error('Failed to create pod', { error });
      return { success: false, error: error instanceof Error ? error.message : 'Unknown error' };
    }
  }

  public async getPodUrl(): Promise<string | null> {
    return (await this.checkPodExists()).podUrl || null;
  }

  public async initPod(): Promise<PodInitResult> {
    try {
      const response = await fetchWithAuth(`${SOLID_POD_BASE_URL}/pods/init`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({}),
      });
      if (!response.ok) {
        const error = await response.json().catch(() => ({ error: 'Initialization failed' }));
        return { success: false, created: false, error: error.error || 'Pod initialization failed' };
      }
      const result = await response.json();
      logger.info('Pod initialized', { podUrl: result.pod_url, created: result.created });
      return {
        success: true,
        podUrl: result.pod_url,
        webId: result.webid,
        created: result.created,
        structure: result.structure,
      };
    } catch (error) {
      logger.error('Failed to initialize pod', { error });
      return { success: false, created: false, error: error instanceof Error ? error.message : 'Unknown error' };
    }
  }

  public async ensurePodExists(): Promise<PodInitResult> {
    return this.initPod();
  }

  public async getPodStructure(): Promise<PodStructure | null> {
    const result = await this.initPod();
    const structure = result.structure || null;
    if (structure?.preferences) this.lastKnownPreferencesPath = structure.preferences;
    return structure;
  }

  // =========================================================================
  // Preferences
  // =========================================================================

  public async setPreference(key: string, value: unknown): Promise<boolean> {
    const structure = await this.getPodStructure();
    if (!structure) { logger.error('Cannot set preference: pod not initialized'); return false; }
    const safeKey = sanitizePreferenceKey(key);
    return putResource(`${structure.preferences}${safeKey}.jsonld`, {
      '@context': 'https://www.w3.org/ns/ldp',
      '@type': 'Preference',
      key,
      value,
      modified: new Date().toISOString(),
    });
  }

  public async getPreference(key: string): Promise<unknown | null> {
    const structure = await this.getPodStructure();
    if (!structure) { logger.error('Cannot get preference: pod not initialized'); return null; }
    try {
      const safeKey = sanitizePreferenceKey(key);
      const doc = await fetchJsonLd(`${structure.preferences}${safeKey}.jsonld`);
      return (doc as { value?: unknown }).value ?? null;
    } catch { return null; }
  }

  // =========================================================================
  // Graph views
  // =========================================================================

  public async saveGraphView(name: string, viewData: {
    camera?: { x: number; y: number; z: number; fov?: number };
    filters?: Record<string, unknown>;
    physics?: Record<string, unknown>;
    clusters?: Record<string, unknown>;
    pinnedNodes?: number[];
    nodeTypeVisibility?: Record<string, boolean>;
  }): Promise<boolean> {
    const structure = await this.getPodStructure();
    if (!structure) { logger.error('Cannot save graph view: pod not initialized'); return false; }
    const safeName = sanitizePreferenceKey(name);
    const success = await putResource(`${structure.preferences}graph-views/${safeName}.jsonld`, {
      '@context': 'https://schema.org',
      '@type': 'ViewAction',
      '@id': `#${safeName}`,
      name,
      dateCreated: new Date().toISOString(),
      ...viewData,
    });
    if (success) logger.info(`Graph view "${name}" saved to Pod`);
    return success;
  }

  public async loadGraphView(name: string): Promise<Record<string, unknown> | null> {
    const structure = await this.getPodStructure();
    if (!structure) return null;
    try {
      const safeName = sanitizePreferenceKey(name);
      return (await fetchJsonLd(`${structure.preferences}graph-views/${safeName}.jsonld`)) as Record<string, unknown>;
    } catch { logger.warn(`Graph view "${name}" not found in Pod`); return null; }
  }

  public async listGraphViews(): Promise<string[]> {
    const structure = await this.getPodStructure();
    if (!structure) return [];
    try {
      const response = await fetchWithAuth(`/solid${structure.preferences}graph-views/`, {
        headers: { Accept: 'application/ld+json' },
      });
      if (!response.ok) return [];
      const data = await response.json();
      const contains = data['ldp:contains'] || data['contains'] || [];
      const items: Array<{ '@id'?: string; url?: string }> = Array.isArray(contains) ? contains : [contains];
      return items
        .map((item) => { const url = item['@id'] || item.url || ''; const m = url.match(/\/([^/]+)\.jsonld$/); return m ? decodeURIComponent(m[1]) : ''; })
        .filter(Boolean);
    } catch { return []; }
  }

  public async deleteGraphView(name: string): Promise<boolean> {
    const structure = await this.getPodStructure();
    if (!structure) return false;
    const safeName = sanitizePreferenceKey(name);
    return deleteResource(`${structure.preferences}graph-views/${safeName}.jsonld`);
  }

  public subscribeToGraphViewChanges(callback: (viewName: string) => void): () => void {
    const prefPath = this.lastKnownPreferencesPath;
    if (!prefPath) { logger.warn('Cannot subscribe to graph view changes: pod structure not cached'); return () => {}; }
    return this.subscribeToChanges(`${prefPath}graph-views/`, (url) => {
      const m = url.match(/\/([^/]+)\.jsonld$/);
      if (m) callback(decodeURIComponent(m[1]));
    });
  }

  // =========================================================================
  // Ontology contributions / proposals
  // =========================================================================

  public async submitOntologyContribution(contribution: {
    title: string; description: string; changes: unknown;
  }): Promise<string | null> {
    const structure = await this.getPodStructure();
    if (!structure) { logger.error('Cannot submit contribution: pod not initialized'); return null; }
    return postResource(structure.ontology_contributions, {
      '@context': { '@vocab': 'https://narrativegoldmine.com/ontology#', schema: 'https://schema.org/' },
      '@type': 'OntologyContribution',
      'schema:name': contribution.title,
      'schema:description': contribution.description,
      changes: contribution.changes,
      status: 'draft',
      'schema:dateCreated': new Date().toISOString(),
    }, `contrib-${Date.now()}`);
  }

  public async submitOntologyProposal(proposal: {
    title: string; description: string; contributionUrl: string;
  }): Promise<string | null> {
    const structure = await this.getPodStructure();
    if (!structure) { logger.error('Cannot submit proposal: pod not initialized'); return null; }
    return postResource(structure.ontology_proposals, {
      '@context': { '@vocab': 'https://narrativegoldmine.com/ontology#', schema: 'https://schema.org/' },
      '@type': 'OntologyProposal',
      'schema:name': proposal.title,
      'schema:description': proposal.description,
      contribution: proposal.contributionUrl,
      status: 'pending',
      'schema:dateCreated': new Date().toISOString(),
    }, `proposal-${Date.now()}`);
  }

  // =========================================================================
  // Agent memory — delegates to solidPod/agentMemory.ts
  // =========================================================================

  private async podPathFor(agentId: string): Promise<string | null> {
    const podUrl = await this.getPodUrl();
    return podUrl ? podUrl.replace(/^https?:\/\/[^/]+/, '') : null;
  }

  public async storeAgentMemory(agentId: string, entry: {
    key: string; value: string; namespace: string; tags?: string[]; timestamp?: string;
  }): Promise<boolean> {
    const podPath = await this.podPathFor(agentId);
    return podPath ? _storeAgentMemory(podPath, agentId, entry) : false;
  }

  public async listAgentMemories(agentId: string): Promise<Array<{ key: string; value: string; namespace: string }>> {
    const podPath = await this.podPathFor(agentId);
    return podPath ? _listAgentMemories(podPath, agentId) : [];
  }

  public async getAgentMemory(agentId: string, key: string): Promise<Record<string, unknown> | null> {
    const podPath = await this.podPathFor(agentId);
    return podPath ? _getAgentMemory(podPath, agentId, key) : null;
  }

  public async deleteAgentMemory(agentId: string, key: string): Promise<boolean> {
    const podPath = await this.podPathFor(agentId);
    return podPath ? _deleteAgentMemory(podPath, agentId, key) : false;
  }

  public async setAgentMemoryAccess(agentId: string, permissions: {
    agentWebId: string; modes: ('Read' | 'Write' | 'Append')[];
  }): Promise<boolean> {
    const podPath = await this.podPathFor(agentId);
    if (!podPath) return false;
    const podInfo = await this.checkPodExists();
    return _setAgentMemoryAccess(podPath, agentId, podInfo.webId || '', permissions);
  }

  // =========================================================================
  // Pod connection
  // =========================================================================

  public async connectToPod(npub: string): Promise<PodInfo> {
    logger.info('Connecting to pod for npub', { npub });
    if (!npub.startsWith('npub1')) throw new Error('Invalid npub format. Expected npub1...');

    try {
      const response = await fetchWithAuth(`${SOLID_POD_BASE_URL}/pods/connect`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ npub }),
      });
      if (!response.ok) {
        const error = await response.json().catch(() => ({ error: 'Connection failed' }));
        throw new Error(error.error || `Pod connection failed: ${response.status}`);
      }
      const info = await response.json();
      logger.info('Connected to pod', { podUrl: info.podUrl });
      if (JSS_WS_URL && !this.notifications.isConnected) this.notifications.connect();
      return { exists: true, podUrl: info.podUrl || info.pod_url, webId: info.webId || info.webid };
    } catch (error) {
      logger.error('Failed to connect to pod', { npub, error });
      throw error;
    }
  }

  // =========================================================================
  // LDP public facade
  // =========================================================================

  public async readResource(path: string): Promise<JsonLdDocument> { return fetchJsonLd(path); }
  public async writeResource(path: string, content: JsonLdDocument | Record<string, unknown> | string): Promise<boolean> { return putResource(path, ensureJsonLd(content)); }
  public async fetchJsonLd(resourcePath: string): Promise<JsonLdDocument> { return fetchJsonLd(resourcePath); }
  public async fetchTurtle(resourcePath: string): Promise<string> { return fetchTurtle(resourcePath); }
  public async putResource(resourcePath: string, data: JsonLdDocument | string, contentType: 'application/ld+json' | 'text/turtle' = 'application/ld+json'): Promise<boolean> { return putResource(resourcePath, data, contentType); }
  public async postResource(containerPath: string, data: JsonLdDocument, slug?: string): Promise<string | null> { return postResource(containerPath, data, slug); }
  public async deleteResource(resourcePath: string): Promise<boolean> { return deleteResource(resourcePath); }
  public async resourceExists(resourcePath: string): Promise<boolean> { return resourceExists(resourcePath); }

  // =========================================================================
  // WebSocket notifications facade
  // =========================================================================

  public connectWebSocket(): void { this.notifications.connect(); }
  public disconnect(): void { this.notifications.disconnect(); }

  public subscribe(resourceUrl: string, callback: (n: SolidNotification) => void): () => void {
    return this.notifications.subscribe(resourceUrl, callback);
  }

  public subscribeToChanges(path: string, callback: (url: string, type: 'created' | 'updated' | 'deleted') => void): () => void {
    const resourceUrl = resolvePath(path);
    if (!this.notifications.isConnected) this.notifications.connect();
    return this.notifications.subscribe(resourceUrl, (notification) => {
      if (notification.type === 'pub') callback(notification.url, 'updated');
    });
  }

  // =========================================================================
  // Type Index facade
  // =========================================================================

  public async ensurePublicTypeIndex(): Promise<string> {
    const structure = await this.getPodStructure();
    if (!structure) throw new Error('Cannot ensure Type Index: pod not initialized');
    const url = await _ensurePublicTypeIndex(structure.preferences);
    const settingsBase = structure.preferences.substring(0, structure.preferences.indexOf('/settings/') + '/settings/'.length);
    const podInfo = await this.checkPodExists();
    if (podInfo.webId) await linkTypeIndexFromProfile(`${settingsBase}publicTypeIndex.jsonld`, podInfo.webId);
    return url;
  }

  public async registerViewInTypeIndex(viewName: string, viewUrl: string): Promise<boolean> {
    const structure = await this.getPodStructure();
    return structure ? _registerViewInTypeIndex(structure.preferences, viewName, viewUrl) : false;
  }

  public async registerAgentInTypeIndex(agentId: string, capabilities: string[]): Promise<boolean> {
    const structure = await this.getPodStructure();
    return structure ? _registerAgentInTypeIndex(structure.preferences, agentId, capabilities) : false;
  }

  public async discoverSharedViews(webId: string): Promise<DiscoveredView[]> { return _discoverSharedViews(webId); }
  public async discoverAgents(webId: string): Promise<DiscoveredAgent[]> { return _discoverAgents(webId); }
}

const solidPodService = SolidPodService.getInstance();
export default solidPodService;
