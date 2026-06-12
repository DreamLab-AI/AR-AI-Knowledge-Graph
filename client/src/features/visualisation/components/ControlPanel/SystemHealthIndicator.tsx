import React, { useEffect, useMemo, useState } from 'react';
import { Activity, Wifi, Database, Server, Check, AlertCircle, Loader, Filter, Link, Unlink, Network, Boxes, Bot, GitBranch, Sigma, Zap } from 'lucide-react';
import { webSocketService, useWebSocketStore } from '../../../../store/websocketStore';
import { useSettingsStore } from '../../../../store/settingsStore';
import { useConstraintStats } from '../../../ontology/hooks/useConstraintStats';
import { useInferredEdgesStore } from '../../../ontology/store/useInferredEdgesStore';

interface ConnectionStatus {
  websocket: 'connected' | 'connecting' | 'disconnected';
  metadata: 'loaded' | 'loading' | 'error' | 'none';
  nodes: number;
  edges: number;
  mcpSwarm: 'connected' | 'disconnected' | 'unknown';
  visibleNodes?: number;
  totalNodes?: number;
}

interface SystemHealthIndicatorProps {
  graphData?: {
    nodes: any[];
    edges: any[];
  };
  /** Agent/bots graph stats — authoritative source for the agent node count. */
  botsData?: {
    nodeCount: number;
    edgeCount: number;
  };
  mcpConnected?: boolean;
  websocketStatus?: 'connected' | 'connecting' | 'disconnected';
  metadataStatus?: 'loaded' | 'loading' | 'error' | 'none';
}

/** Per-graph-type node tallies derived from the live node population. */
interface GraphTypeCounts {
  knowledge: number;
  ontology: number;
  agent: number;
}

/**
 * Bucket nodes into the three graph types by their carried classification.
 * Mirrors the renderer's detection (useGraphVisualState): ontology nodes carry
 * `owl_class`/`ontology_node` type or hierarchy/owlClass signals; agent nodes
 * carry `agentType`/`tokenRate`; everything else is knowledge (page/linked_page).
 */
function bucketNodeTypes(nodes: any[] | undefined): GraphTypeCounts {
  const counts: GraphTypeCounts = { knowledge: 0, ontology: 0, agent: 0 };
  if (!Array.isArray(nodes)) return counts;
  for (const n of nodes) {
    const meta = n?.metadata ?? {};
    const type = String(meta.type ?? meta.nodeType ?? '').toLowerCase();
    const isAgent = !!meta.agentType || meta.tokenRate !== undefined || type.startsWith('agent') || type.startsWith('bot');
    const isOntology =
      type === 'owl_class' || type === 'ontology_node' || type.startsWith('owl_') ||
      !!(n?.owlClassIri || meta.owlClassIri || meta.class_iri) ||
      meta.hierarchyDepth !== undefined;
    if (isAgent) counts.agent++;
    else if (isOntology) counts.ontology++;
    else counts.knowledge++;
  }
  return counts;
}

export const SystemHealthIndicator: React.FC<SystemHealthIndicatorProps> = ({
  graphData,
  botsData,
  mcpConnected = false,
  websocketStatus = 'disconnected',
  metadataStatus = 'none'
}) => {
  const [status, setStatus] = useState<ConnectionStatus>({
    websocket: websocketStatus,
    metadata: metadataStatus,
    nodes: graphData?.nodes?.length || 0,
    edges: graphData?.edges?.length || 0,
    mcpSwarm: mcpConnected ? 'connected' : 'disconnected'
  });

  // Live GPU constraint stats (axioms processed, active constraints, GPU health).
  const { stats: constraintStats } = useConstraintStats(8000);
  // Inferred-edge count comes from the materialised reasoning report (ADR-099).
  const inferredCount = useInferredEdgesStore(s => s.report.count);
  const refreshInferred = useInferredEdgesStore(s => s.refresh);

  // The always-on status box owns the first inferred-report pull so the count
  // populates without requiring the Ontology tab to be opened (empty-safe).
  useEffect(() => {
    void refreshInferred();
  }, [refreshInferred]);

  // Per-type node tallies. Agents are authoritative from botsData (separate
  // graph); knowledge/ontology are bucketed from the main node population.
  const typeCounts = useMemo<GraphTypeCounts>(() => {
    const bucketed = bucketNodeTypes(graphData?.nodes);
    const agent = botsData?.nodeCount ?? bucketed.agent;
    return { knowledge: bucketed.knowledge, ontology: bucketed.ontology, agent };
  }, [graphData, botsData?.nodeCount]);

  useEffect(() => {
    setStatus({
      websocket: websocketStatus,
      metadata: metadataStatus,
      nodes: graphData?.nodes?.length || 0,
      edges: graphData?.edges?.length || 0,
      mcpSwarm: mcpConnected ? 'connected' : 'disconnected'
    });
  }, [graphData, mcpConnected, websocketStatus, metadataStatus]);

  // Listen for filter updates from WebSocket
  useEffect(() => {
    if (webSocketService) {
      const unsubscribe = webSocketService.on('filterApplied', (data: unknown) => {
        const filterData = data as { visibleNodes: number; totalNodes: number };
        setStatus(prev => ({
          ...prev,
          visibleNodes: filterData.visibleNodes,
          totalNodes: filterData.totalNodes
        }));
      });
      return unsubscribe;
    }
  }, []);

  // Honest liveness: statistics.lastActivity is bumped on every real WS
  // message (rx and tx) since the Front-4 fix — a wedged or half-open feed
  // stops bumping it, so the indicator can no longer show "All systems
  // synchronized" over a dead connection. 90s grace covers converged-physics
  // quiet periods (heartbeat pings refresh activity every 30s regardless).
  const lastActivity = useWebSocketStore(s => s.statistics.lastActivity);
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const t = window.setInterval(() => setNow(Date.now()), 15000);
    return () => window.clearInterval(t);
  }, []);
  const feedFresh = now - lastActivity < 90000;

  const isFullyConnected =
    status.websocket === 'connected' &&
    status.metadata === 'loaded' &&
    status.nodes > 0 &&
    feedFresh;

  const getStatusColor = (connected: boolean | string): string => {
    if (connected === true || connected === 'connected' || connected === 'loaded') {
      return '#22c55e'; // green
    }
    if (connected === 'connecting' || connected === 'loading') {
      return '#f59e0b'; // amber
    }
    return '#ef4444'; // red
  };

  const getStatusIcon = (connected: boolean | string) => {
    if (connected === true || connected === 'connected' || connected === 'loaded') {
      return <Check size={8} />;
    }
    if (connected === 'connecting' || connected === 'loading') {
      return <Loader size={8} className="animate-spin" />;
    }
    return <AlertCircle size={8} />;
  };

  return (
    <div style={{
      background: isFullyConnected
        ? 'linear-gradient(135deg, rgba(34,197,94,0.15), rgba(16,185,129,0.1))'
        : 'linear-gradient(135deg, rgba(239,68,68,0.15), rgba(245,158,11,0.1))',
      border: `1px solid ${isFullyConnected ? 'rgba(34,197,94,0.3)' : 'rgba(239,68,68,0.3)'}`,
      borderRadius: '4px',
      padding: '6px 8px',
      marginBottom: '6px'
    }}>
      {/* Header */}
      <div style={{
        display: 'flex',
        alignItems: 'center',
        gap: '6px',
        marginBottom: '6px',
        color: isFullyConnected ? '#22c55e' : '#f59e0b',
        fontWeight: '600',
        fontSize: '10px'
      }}>
        <Activity size={12} />
        <span>System Status</span>
        <div style={{
          marginLeft: 'auto',
          width: '8px',
          height: '8px',
          borderRadius: '50%',
          background: isFullyConnected ? '#22c55e' : '#f59e0b',
          boxShadow: `0 0 6px ${isFullyConnected ? '#22c55e' : '#f59e0b'}`
        }} />
      </div>

      {/* Status Grid */}
      <div style={{
        display: 'grid',
        gridTemplateColumns: 'repeat(2, 1fr)',
        gap: '4px',
        fontSize: '9px'
      }}>
        {/* WebSocket */}
        <div style={{
          display: 'flex',
          alignItems: 'center',
          gap: '4px',
          padding: '3px 6px',
          background: 'rgba(255,255,255,0.05)',
          borderRadius: '3px'
        }}>
          <Wifi size={10} style={{ color: getStatusColor(status.websocket) }} />
          <span style={{ color: 'rgba(255,255,255,0.7)' }}>WS</span>
          <span style={{
            marginLeft: 'auto',
            color: getStatusColor(status.websocket),
            display: 'flex',
            alignItems: 'center',
            gap: '2px'
          }}>
            {getStatusIcon(status.websocket)}
          </span>
        </div>

        {/* Metadata */}
        <div style={{
          display: 'flex',
          alignItems: 'center',
          gap: '4px',
          padding: '3px 6px',
          background: 'rgba(255,255,255,0.05)',
          borderRadius: '3px'
        }}>
          <Database size={10} style={{ color: getStatusColor(status.metadata) }} />
          <span style={{ color: 'rgba(255,255,255,0.7)' }}>Meta</span>
          <span style={{
            marginLeft: 'auto',
            color: getStatusColor(status.metadata),
            display: 'flex',
            alignItems: 'center',
            gap: '2px'
          }}>
            {getStatusIcon(status.metadata)}
          </span>
        </div>

        {/* Filtered Nodes (if filter active) */}
        {status.visibleNodes !== undefined && status.totalNodes !== undefined && (
          <div style={{
            display: 'flex',
            alignItems: 'center',
            gap: '4px',
            padding: '3px 6px',
            background: 'rgba(255,255,255,0.05)',
            borderRadius: '3px'
          }}>
            <Filter size={10} style={{ color: '#a855f7' }} />
            <span style={{ color: 'rgba(255,255,255,0.7)' }}>Visible</span>
            <span style={{
              marginLeft: 'auto',
              color: '#a855f7',
              fontWeight: '600',
              fontSize: '8px'
            }}>
              {status.visibleNodes}/{status.totalNodes}
            </span>
          </div>
        )}

        {/* MCP Swarm */}
        <div style={{
          display: 'flex',
          alignItems: 'center',
          gap: '4px',
          padding: '3px 6px',
          background: 'rgba(255,255,255,0.05)',
          borderRadius: '3px'
        }}>
          <Server size={10} style={{ color: getStatusColor(status.mcpSwarm) }} />
          <span style={{ color: 'rgba(255,255,255,0.7)' }}>MCP</span>
          <span style={{
            marginLeft: 'auto',
            color: getStatusColor(status.mcpSwarm),
            display: 'flex',
            alignItems: 'center',
            gap: '2px'
          }}>
            {getStatusIcon(status.mcpSwarm)}
          </span>
        </div>
      </div>

      {/* Graph-type breakdown — knowledge / ontology / agent populations */}
      <div style={{
        marginTop: '6px',
        display: 'flex',
        alignItems: 'center',
        gap: '4px',
        fontSize: '8px',
        color: 'rgba(255,255,255,0.45)',
        textTransform: 'uppercase',
        letterSpacing: '0.04em'
      }}>
        <Network size={9} />
        <span>Graphs</span>
        <span style={{ marginLeft: 'auto', color: 'rgba(255,255,255,0.35)' }}>
          {(typeCounts.knowledge + typeCounts.ontology + typeCounts.agent).toLocaleString()} total
        </span>
      </div>
      <div style={{
        marginTop: '4px',
        display: 'grid',
        gridTemplateColumns: 'repeat(3, 1fr)',
        gap: '4px',
        fontSize: '9px'
      }}>
        <GraphTypeTile icon={<Boxes size={10} />} label="Know" count={typeCounts.knowledge} color="#66BB6A" />
        <GraphTypeTile icon={<GitBranch size={10} />} label="Onto" count={typeCounts.ontology} color="#F2C14E" />
        <GraphTypeTile icon={<Bot size={10} />} label="Agent" count={typeCounts.agent} color="#4FC3F7" />
      </div>

      {/* Ontology rigour — classes, axioms, inferred edges, live GPU forces */}
      {(typeCounts.ontology > 0 || constraintStats.axiomsProcessed > 0 || inferredCount > 0) && (
        <>
          <div style={{
            marginTop: '6px',
            display: 'flex',
            alignItems: 'center',
            gap: '4px',
            fontSize: '8px',
            color: 'rgba(242,193,78,0.6)',
            textTransform: 'uppercase',
            letterSpacing: '0.04em'
          }}>
            <Sigma size={9} />
            <span>Ontology</span>
            {(constraintStats.gpuFailureCount > 0 || constraintStats.cpuFallbackCount > 0) && (
              <span
                title={`GPU constraint failures: ${constraintStats.gpuFailureCount}, CPU fallbacks: ${constraintStats.cpuFallbackCount}`}
                style={{ marginLeft: 'auto', color: '#f59e0b', display: 'flex', alignItems: 'center', gap: '2px' }}
              >
                <AlertCircle size={8} />
                {constraintStats.gpuFailureCount + constraintStats.cpuFallbackCount}
              </span>
            )}
          </div>
          <div style={{
            marginTop: '4px',
            display: 'grid',
            gridTemplateColumns: 'repeat(2, 1fr)',
            gap: '4px',
            fontSize: '9px'
          }}>
            <OntologyFieldTile icon={<GitBranch size={9} />} label="Classes" value={typeCounts.ontology} color="#F2C14E" />
            <OntologyFieldTile icon={<Sigma size={9} />} label="Axioms" value={constraintStats.axiomsProcessed} color="#C9A227" />
            <OntologyFieldTile icon={<Network size={9} />} label="Inferred" value={inferredCount} color="#FBBF24" />
            <OntologyFieldTile
              icon={<Zap size={9} />}
              label="Forces"
              value={constraintStats.activeConstraints}
              color={constraintStats.activeConstraints > 0 ? '#22c55e' : '#ef4444'}
            />
          </div>
        </>
      )}

      {/* Settings Sync Toggle */}
      <SettingsSyncToggle />

      {/* Sync status text */}
      <div style={{
        marginTop: '6px',
        fontSize: '8px',
        color: 'rgba(255,255,255,0.5)',
        textAlign: 'center'
      }}>
        {isFullyConnected
          ? 'All systems synchronized'
          : 'Waiting for connections...'}
      </div>
    </div>
  );
};

/** Compact per-graph-type tile: icon + short label + node count. */
const GraphTypeTile: React.FC<{ icon: React.ReactNode; label: string; count: number; color: string }> = ({
  icon, label, count, color,
}) => (
  <div style={{
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'center',
    gap: '1px',
    padding: '3px 2px',
    background: 'rgba(255,255,255,0.05)',
    borderRadius: '3px',
    borderTop: `2px solid ${count > 0 ? color : 'rgba(255,255,255,0.1)'}`
  }}>
    <span style={{ color: count > 0 ? color : 'rgba(255,255,255,0.3)', display: 'flex', alignItems: 'center', gap: '3px' }}>
      {icon}
      <span style={{ fontSize: '8px', color: 'rgba(255,255,255,0.6)' }}>{label}</span>
    </span>
    <span style={{ color: count > 0 ? color : 'rgba(255,255,255,0.4)', fontWeight: 700, fontSize: '11px' }}>
      {count.toLocaleString()}
    </span>
  </div>
);

/** Compact ontology metric field: icon + label + value. */
const OntologyFieldTile: React.FC<{ icon: React.ReactNode; label: string; value: number; color: string }> = ({
  icon, label, value, color,
}) => (
  <div style={{
    display: 'flex',
    alignItems: 'center',
    gap: '4px',
    padding: '3px 6px',
    background: 'rgba(255,255,255,0.05)',
    borderRadius: '3px'
  }}>
    <span style={{ color, display: 'flex', alignItems: 'center' }}>{icon}</span>
    <span style={{ color: 'rgba(255,255,255,0.7)' }}>{label}</span>
    <span style={{ marginLeft: 'auto', color, fontWeight: 600 }}>{value.toLocaleString()}</span>
  </div>
);

/** Settings sync telltale — shows whether physics/analytics changes propagate to the server */
const SettingsSyncToggle: React.FC = () => {
  const syncEnabled = useSettingsStore(s => s.settingsSyncEnabled);
  const setSyncEnabled = useSettingsStore(s => s.setSettingsSyncEnabled);

  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: '4px',
        padding: '3px 6px',
        marginTop: '4px',
        background: syncEnabled ? 'rgba(34,197,94,0.1)' : 'rgba(239,68,68,0.1)',
        borderRadius: '3px',
        cursor: 'pointer',
        border: `1px solid ${syncEnabled ? 'rgba(34,197,94,0.3)' : 'rgba(239,68,68,0.3)'}`,
        fontSize: '9px',
        transition: 'all 0.2s ease'
      }}
      onClick={() => setSyncEnabled(!syncEnabled)}
      title={syncEnabled
        ? 'Settings sync ON — your changes update the shared server state. Click to switch to local-only.'
        : 'Settings sync OFF — changes are local to this browser session. Click to re-enable sync.'}
    >
      {syncEnabled
        ? <Link size={10} style={{ color: '#22c55e' }} />
        : <Unlink size={10} style={{ color: '#ef4444' }} />}
      <span style={{ color: syncEnabled ? '#22c55e' : '#ef4444' }}>
        {syncEnabled ? 'Sync' : 'Local'}
      </span>
      <span style={{
        marginLeft: 'auto',
        width: '6px',
        height: '6px',
        borderRadius: '50%',
        background: syncEnabled ? '#22c55e' : '#ef4444',
      }} />
    </div>
  );
};
