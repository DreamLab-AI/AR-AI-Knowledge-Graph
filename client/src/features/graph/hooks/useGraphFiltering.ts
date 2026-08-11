/**
 * useGraphFiltering
 *
 * Extracts node filtering logic (quality/authority thresholds, hierarchy-based
 * visibility, expansion state) and derived lookup maps from GraphManager.
 *
 * Provides:
 *  - visibleNodes: the filtered subset of graphData.nodes that should be rendered
 *  - nodeIdToIndexMap: O(1) lookup from string node ID to its index in graphData.nodes
 *  - filteredEdges: edges whose both endpoints survive filtering (same as graphData.edges
 *    today since edge filtering is position-based, but provided for future use)
 *
 * Population scope (PRD-018 WS-4): this hook filters by quality/authority/
 * linked-page/degree — NOT by population type. Restricting the rendered graph to
 * a single population (knowledge | ontology | agent) is done SERVER-SIDE via
 * `graphDataManager.setGraphTypeFilter(...)` → `?graph_type=` so the whole graph
 * is no longer transferred-then-filtered. The two layers are orthogonal: the
 * server scopes *which population* is fetched; this hook scopes *which of those
 * fetched nodes* pass quality/visibility thresholds.
 */

import { useMemo, useEffect } from 'react';
import { useSettingsStore } from '../../../store/settingsStore';
import { useExpansionState, type ExpansionState } from '../hooks/useExpansionState';
import { createLogger } from '../../../utils/loggerConfig';
import type { GraphData, Node as GraphNode, Edge } from '../managers/graphDataManager';
import type { HierarchyNode } from '../utils/hierarchyDetector';

const logger = createLogger('useGraphFiltering');

/**
 * Ordinal rank for the OWL `maturity` metadata field — the only real per-node
 * ontology quality signal on the wire. `qualityScore` is near-constant (0.7-0.8)
 * and present on ~34% of ontology nodes, so it cannot discriminate; `maturity`
 * spans six tiers and the server emits it on every owl_type=Class node.
 */
const MATURITY_RANK: Record<string, number> = {
  draft: 1,
  developing: 2,
  emerging: 3,
  growing: 4,
  established: 5,
  mature: 6,
};

// ---------------------------------------------------------------------------
// Hook return type
// ---------------------------------------------------------------------------

export interface GraphFilteringResult {
  /** Nodes that pass hierarchy-expansion and quality/authority filters */
  visibleNodes: GraphNode[];
  /** O(1) lookup: string node-id -> index in graphData.nodes (full set, not filtered) */
  nodeIdToIndexMap: Map<string, number>;
  /** Edges from graphData (pass-through today; hook owns the contract for future filtering) */
  filteredEdges: Edge[];
  /** Expansion state controller (for toggling node expansion in hierarchy view) */
  expansionState: ExpansionState;
}

// ---------------------------------------------------------------------------
// Hook implementation
// ---------------------------------------------------------------------------

export function useGraphFiltering(
  graphData: GraphData,
  hierarchyMap: Map<string, HierarchyNode>,
  connectionCountMap: Map<string, number>,
): GraphFilteringResult {
  // Narrow selector: only subscribe to nodeFilter — prevents re-renders when
  // unrelated settings (glow, edges, physics, etc.) change.
  const storeNodeFilter = useSettingsStore(s => s.settings?.nodeFilter);

  // --- nodeIdToIndexMap ---
  const nodeIdToIndexMap = useMemo(() =>
    new Map(graphData.nodes.map((n, i) => [String(n.id), i])),
    [graphData.nodes]
  );

  // --- Expansion state (per-client, no server persistence) ---
  const expansionState = useExpansionState(true); // Default: all expanded
  const filterEnabled = storeNodeFilter?.enabled ?? false;
  const qualityThreshold = storeNodeFilter?.qualityThreshold ?? 0.7;
  const authorityThreshold = storeNodeFilter?.authorityThreshold ?? 0.5;
  const filterByQuality = storeNodeFilter?.filterByQuality ?? true;
  const filterByAuthority = storeNodeFilter?.filterByAuthority ?? false;
  const filterMode = storeNodeFilter?.filterMode ?? 'or';
  // Owns linked_page visibility: when false, wikilink-stub nodes (linked_page)
  // are excluded from the render set entirely (default false, matches server flag).
  const includeLinkedPages = storeNodeFilter?.includeLinkedPages ?? false;
  // Degree cutoff: hide nodes whose graph degree is below this (default 0 = off).
  // Set to 1 to suppress the degree-0 orphan spray. Keys directly on the edge-
  // derived connectionCountMap, independent of the quality/authority filter.
  const minConnections = storeNodeFilter?.minConnections ?? 0;
  // Ontology maturity gate: drop nodes below this OWL maturity tier. 'off' = no
  // gate. Independent of the quality/authority AND/OR combination (which the
  // default OR-mode neutralises), mirroring the minConnections hard cutoff.
  const rawMinMaturity = storeNodeFilter?.minMaturity ?? '';
  const minMaturityRank =
    rawMinMaturity && rawMinMaturity !== 'off' ? MATURITY_RANK[rawMinMaturity] ?? 0 : 0;

  // Log filter settings changes for debugging
  useEffect(() => {
    logger.info('[NodeFilter] Settings updated:', {
      enabled: filterEnabled,
      qualityThreshold,
      authorityThreshold,
      filterByQuality,
      filterByAuthority,
      filterMode,
      hasStoreFilter: !!storeNodeFilter,
    });
  }, [filterEnabled, qualityThreshold, authorityThreshold, filterByQuality, filterByAuthority, filterMode, storeNodeFilter]);

  // --- visibleNodes ---
  const visibleNodes = useMemo(() => {
    if (graphData.nodes.length === 0) return [];

    logger.debug(`[NodeFilter] Computing visible nodes: filterEnabled=${filterEnabled}, qualityThreshold=${qualityThreshold}, authorityThreshold=${authorityThreshold}`);

    const visible = graphData.nodes.filter(node => {
      // linked_page gate: wikilink-stub nodes are hidden unless includeLinkedPages
      // is on. Read the AUTHORITATIVE origin from metadata.type (single source of
      // truth, matching the server's Node::population_type and useGraphVisualState).
      // The top-level `type` field (serde rename of node_type) is non-classifying
      // elevation scaffold and is consulted only as a legacy fallback when
      // metadata.type is absent — mirroring the server's population_type ordering.
      if (!includeLinkedPages) {
        const nodeType = (node.metadata?.type as string | undefined)
          || (node as unknown as { type?: string }).type
          || node.metadata?.nodeType
          || (node as unknown as { nodeType?: string }).nodeType
          || '';
        if (nodeType === 'linked_page') {
          return false;
        }
      }

      // Degree cutoff: drop orphans / low-degree nodes when minConnections > 0.
      if (minConnections > 0) {
        const degree = connectionCountMap.get(node.id) || 0;
        if (degree < minConnections) {
          return false;
        }
      }

      // Ontology maturity gate: drop nodes whose OWL maturity tier is below the
      // selected minimum. Nodes without a `maturity` field (knowledge pages) and
      // nodes carrying an unrecognised maturity string are kept untouched.
      if (minMaturityRank > 0) {
        const m = node.metadata?.maturity as string | undefined;
        if (m) {
          const rank = MATURITY_RANK[m];
          if (rank !== undefined && rank < minMaturityRank) {
            return false;
          }
        }
      }

      // First apply hierarchy/expansion filtering
      const hierarchyNode = hierarchyMap.get(node.id);
      if (hierarchyNode) {
        // Root nodes always pass hierarchy check
        if (!hierarchyNode.isRoot) {
          // Child nodes visible only if parent is expanded
          if (!expansionState.isVisible(node.id, hierarchyNode.parentId)) {
            return false;
          }
        }
      }

      // Then apply quality/authority filtering if enabled
      if (filterEnabled) {
        // Get quality score - use metadata if available, otherwise compute from connections
        // Authored score first: the sync publishes the corpus JSON-LD
        // "quality" as metadata.quality_score (pages) / qualityScore
        // (ontology entities). Only unscored nodes fall back to the degree
        // heuristic — before this, the authored key was never consulted and
        // the client filter disagreed with the server's.
        let quality = node.metadata?.quality_score ?? node.metadata?.quality ?? node.metadata?.qualityScore;
        if (typeof quality === 'string') quality = parseFloat(quality);
        if (quality === undefined || quality === null || Number.isNaN(quality)) {
          // Compute quality from node connections (normalized 0-1) using pre-built map
          const connectionCount = connectionCountMap.get(node.id) || 0;
          // Map connections to 0-1 range: 0 connections = 0, 10+ connections = 1
          quality = Math.min(1.0, connectionCount / 10);
        }

        // Get authority score - use metadata if available, otherwise compute from hierarchy
        let authority = node.metadata?.authority ?? node.metadata?.authorityScore;
        if (authority === undefined || authority === null) {
          // Compute authority from hierarchy depth and connections
          const hierarchyNode = hierarchyMap.get(node.id);
          const depth = hierarchyNode?.depth ?? 0;
          // Root nodes (depth 0) have high authority, deeper nodes have less
          authority = Math.max(0, 1.0 - (depth * 0.2));
        }

        const passesQuality = !filterByQuality || quality >= qualityThreshold;
        const passesAuthority = !filterByAuthority || authority >= authorityThreshold;

        // Apply filter mode (AND requires both, OR requires at least one)
        if (filterMode === 'and') {
          if (!passesQuality || !passesAuthority) {
            return false;
          }
        } else {
          // OR mode - but only if at least one filter is active
          if (filterByQuality || filterByAuthority) {
            if (!passesQuality && !passesAuthority) {
              return false;
            }
          }
        }
      }

      return true;
    });

    // Always log when filtering is active
    if (filterEnabled) {
      logger.info(`[NodeFilter] Result: ${visible.length}/${graphData.nodes.length} nodes visible (quality>=${qualityThreshold}, authority>=${authorityThreshold}, mode=${filterMode})`);
    }

    return visible;
  }, [graphData.nodes, connectionCountMap, hierarchyMap, expansionState, filterEnabled, qualityThreshold, authorityThreshold, filterByQuality, filterByAuthority, filterMode, includeLinkedPages, minConnections, minMaturityRank]);

  // --- filteredEdges (pass-through today) ---
  const filteredEdges = graphData.edges;

  return {
    visibleNodes,
    nodeIdToIndexMap,
    filteredEdges,
    expansionState,
  };
}
