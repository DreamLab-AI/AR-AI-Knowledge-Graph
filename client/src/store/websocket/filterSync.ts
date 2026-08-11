/**
 * filterSync.ts — Client filter state synchronization
 *
 * Handles: filter subscription, filter update messages, node filter
 * settings sync over WebSocket.
 */

import { createLogger } from '../../utils/loggerConfig';
import { useSettingsStore } from '../settingsStore';
import { graphDataManager } from '../../features/graph/managers/graphDataManager';
import type { FilterUpdateParams, FilterSnapshot } from './types';

const logger = createLogger('WebSocketStore');

// ── Encapsulated module-level state ────────────────────────────────────
let filterSubscriptionSet = false;
let filterUnsubscribers: (() => void)[] = [];
let lastFilterSnapshot: FilterSnapshot | null = null;

// ── Filter-response expectation window ─────────────────────────────────
//
// After the client sends a filter_update, the server answers with a
// per-client initialGraphLoad carrying the FILTERED (usually smaller) node
// set. handleInitialGraphLoad's shrink-guard — which exists to stop the
// 200-node connect-time payload from clobbering a full REST load — must NOT
// swallow that response, or the quality gates appear to do nothing (the
// long-standing break). The guard consults this window: a smaller load is
// authoritative while a filter response is pending.
const FILTER_RESPONSE_WINDOW_MS = 15_000;
let filterResponseExpectedUntil = 0;

export function expectFilterResponse() {
  filterResponseExpectedUntil = Date.now() + FILTER_RESPONSE_WINDOW_MS;
}

export function isFilterResponseExpected(): boolean {
  return Date.now() < filterResponseExpectedUntil;
}

export function clearFilterResponseExpectation() {
  filterResponseExpectedUntil = 0;
}

// ── State accessors (used by index.ts for _reset) ──

export function resetFilterState() {
  filterUnsubscribers.forEach(unsub => { try { unsub(); } catch (_) { /* ignore */ } });
  filterUnsubscribers = [];
  filterSubscriptionSet = false;
  lastFilterSnapshot = null;
}

export function cleanupFilterSubscriptions() {
  filterUnsubscribers.forEach(unsub => { try { unsub(); } catch (_) { /* ignore */ } });
  filterUnsubscribers = [];
  filterSubscriptionSet = false;
}

export function clearFilterSnapshot() {
  lastFilterSnapshot = null;
}

// ── Filter subscription setup ──────────────────────────────────────────

export function setupFilterSubscription(
  get: () => { isConnected: boolean; sendFilterUpdate: (filter: FilterUpdateParams) => void },
) {
  if (filterSubscriptionSet) return;
  filterSubscriptionSet = true;

  const filterPaths = [
    'nodeFilter.enabled',
    'nodeFilter.qualityThreshold',
    'nodeFilter.authorityThreshold',
    'nodeFilter.filterByQuality',
    'nodeFilter.filterByAuthority',
    'nodeFilter.filterMode',
    'nodeFilter.includeLinkedPages',
  ] as const;

  filterPaths.forEach(path => {
    const store = useSettingsStore.getState();
    if (store.subscribe) {
      const unsub = store.subscribe(path as Parameters<typeof store.subscribe>[0], () => {
        handleFilterChange(get);
      });
      filterUnsubscribers.push(unsub);
    }
  });

  const zustandUnsub = useSettingsStore.subscribe((state) => {
    const nodeFilter = state.settings?.nodeFilter;
    const wsState = get();
    if (nodeFilter && wsState.isConnected) {
      const prev = lastFilterSnapshot;
      if (
        !prev ||
        prev.enabled !== nodeFilter.enabled ||
        prev.qualityThreshold !== nodeFilter.qualityThreshold ||
        prev.authorityThreshold !== nodeFilter.authorityThreshold ||
        prev.filterByQuality !== nodeFilter.filterByQuality ||
        prev.filterByAuthority !== nodeFilter.filterByAuthority ||
        prev.filterMode !== nodeFilter.filterMode ||
        prev.includeLinkedPages !== nodeFilter.includeLinkedPages
      ) {
        lastFilterSnapshot = {
          enabled: nodeFilter.enabled,
          qualityThreshold: nodeFilter.qualityThreshold,
          authorityThreshold: nodeFilter.authorityThreshold,
          filterByQuality: nodeFilter.filterByQuality,
          filterByAuthority: nodeFilter.filterByAuthority,
          filterMode: nodeFilter.filterMode,
          includeLinkedPages: nodeFilter.includeLinkedPages,
        };
        wsState.sendFilterUpdate(lastFilterSnapshot);
      }
    }
  });
  filterUnsubscribers.push(zustandUnsub);

  logger.info('Filter subscription set up - changes will sync to server');
}

function handleFilterChange(
  get: () => { isConnected: boolean; sendFilterUpdate: (filter: FilterUpdateParams) => void },
) {
  const state = get();
  if (!state.isConnected) return;

  const nodeFilter = useSettingsStore.getState().settings?.nodeFilter;
  if (nodeFilter) {
    state.sendFilterUpdate({
      enabled: nodeFilter.enabled,
      qualityThreshold: nodeFilter.qualityThreshold,
      authorityThreshold: nodeFilter.authorityThreshold,
      filterByQuality: nodeFilter.filterByQuality,
      filterByAuthority: nodeFilter.filterByAuthority,
      filterMode: nodeFilter.filterMode,
      includeLinkedPages: nodeFilter.includeLinkedPages,
    });
  }
}

// ── Force refresh ──────────────────────────────────────────────────────

export async function forceRefreshFilter(
  get: () => { isConnected: boolean; sendFilterUpdate: (filter: FilterUpdateParams) => void },
) {
  const state = get();
  if (!state.isConnected) {
    logger.warn('Cannot force refresh filter: WebSocket not connected');
    return;
  }

  const nodeFilter = useSettingsStore.getState().settings?.nodeFilter;
  if (nodeFilter) {
    lastFilterSnapshot = null;

    logger.info('[Refresh] Clearing local graph and requesting fresh filtered data', nodeFilter);

    await graphDataManager.setGraphData({ nodes: [], edges: [] });
    logger.info('[Refresh] Local graph cleared, awaiting server response...');

    state.sendFilterUpdate({
      enabled: nodeFilter.enabled,
      qualityThreshold: nodeFilter.qualityThreshold,
      authorityThreshold: nodeFilter.authorityThreshold,
      filterByQuality: nodeFilter.filterByQuality,
      filterByAuthority: nodeFilter.filterByAuthority,
      filterMode: nodeFilter.filterMode,
      includeLinkedPages: nodeFilter.includeLinkedPages,
    });
  } else {
    logger.warn('No nodeFilter settings found in store');
  }
}
