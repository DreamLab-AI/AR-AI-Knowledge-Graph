import React, { useState, useEffect } from 'react';
import GraphCanvasWrapper from '../features/graph/components/GraphCanvasWrapper';
import { ControlCenter, StatusSurface } from '../features/control-center';
import { useSettingsStore } from '../store/settingsStore';
import { useBotsData } from '../features/bots/contexts/BotsDataContext';
import { BrowserSupportWarning } from '../components/BrowserSupportWarning';
import { AudioInputService } from '../services/AudioInputService';
import { graphDataManager, type GraphData } from '../features/graph/managers/graphDataManager';
import { NodeDetailPanel } from '../features/graph/components/NodeDetailPanel';
import { NodeContextMenu } from '../features/graph/components/NodeContextMenu';
import { AgentOpsSurface } from '../features/control-center/agents/AgentOpsSurface';
import { AcspCaseQueue } from '../features/control-center/governance/AcspCaseQueue';
import { KpiPanel } from '../features/control-center/kpi/KpiPanel';
import { createLogger } from '../utils/loggerConfig';

const logger = createLogger('MainLayout');

const MainLayoutContent: React.FC = () => {
  
  const { settings } = useSettingsStore();
  const { botsData } = useBotsData();
  const showStats = settings?.system?.debug?.enablePerformanceDebug ?? false;
  const enableBloom = settings?.visualisation?.glow?.enabled ?? false;
  const [hasVoiceSupport, setHasVoiceSupport] = useState(true);
  
  
  const [graphData, setGraphData] = useState<GraphData>({ nodes: [], edges: [] });
  const [otherGraphData, setOtherGraphData] = useState<GraphData | undefined>();

  useEffect(() => {
    const support = AudioInputService.getBrowserSupport();
    const isSupported = support.getUserMedia && support.isHttps && support.audioContext && support.mediaRecorder;
    setHasVoiceSupport(isSupported);
  }, []);
  
  
  useEffect(() => {
    const unsubscribe = graphDataManager.onGraphDataChange((data: GraphData) => {
      setGraphData(data);
      
      
      if (data.nodes.length > 0) {
        setOtherGraphData({
          nodes: data.nodes.slice(0, Math.floor(data.nodes.length / 2)),
          edges: data.edges.slice(0, Math.floor(data.edges.length / 2))
        });
      }
    });
    
    return unsubscribe;
  }, []);

  return (
    <main
      role="main"
      aria-label="VisionClaw Graph Visualization"
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        width: '100vw',
        height: '100vh',
        backgroundColor: '#000022'
      }}
    >
      {/* Skip link for keyboard navigation */}
      <a
        href="#control-panel"
        className="sr-only focus:not-sr-only focus:absolute focus:z-50 focus:top-2 focus:left-2 focus:bg-white focus:text-black focus:p-2 focus:rounded"
      >
        Skip to controls
      </a>

      <section aria-label="Graph visualization canvas">
        <GraphCanvasWrapper />
      </section>

      <nav id="control-panel" aria-label="Visualization controls">
        <ControlCenter
          showStats={showStats}
          enableBloom={enableBloom}
          onOrbitControlsToggle={() => {}}
          botsData={botsData ?? undefined}
          graphData={graphData}
          otherGraphData={otherGraphData}
        />
      </nav>

      {/* Node detail slide-in panel — driven by visionclaw:node-selected events */}
      <NodeDetailPanel />

      {/* Right-click additive-expansion menu — driven by visionclaw:node-contextmenu */}
      <NodeContextMenu />

      {/* D2/D8 (PRD-023 WP-3): per-agent steering + swarm observability, opened
          behind an agent-node selection or the ambient "Agents" pill. */}
      <AgentOpsSurface />

      {/* REC-2/D3 (PRD-023 WP-4): broker case queue + ambient ACSP open-case
          indicator, driven by broker:* WS events. */}
      <AcspCaseQueue />

      {/* REC-4 (PRD-023 WP-8, ADR-043 resurrection): four-KPI dashboard —
          Augmentation Ratio + Trust Variance computed live; Mesh Velocity +
          HITL Precision render "awaiting data source". */}
      <KpiPanel />

      {/* Unified system-status surface — dock-anchored badge carrying connection
          telemetry, agent/bots summary, the SpacePilot connect + support/secure-
          context guidance, and an honest layout-motion readout. Replaces the old
          top-right StatusCluster pill and the standalone SpaceMouseStatus banner. */}
      <aside aria-label="System status">
        <StatusSurface />
      </aside>

      {/* OntologyPanel is accessed via the control panel's ontology tab, not rendered as overlay */}

      {!hasVoiceSupport && (
        <aside
          className="fixed bottom-20 left-4 z-40 max-w-sm pointer-events-auto"
          role="alert"
          aria-live="polite"
        >
          <BrowserSupportWarning className="shadow-lg" />
        </aside>
      )}
    </main>
  );
};

const MainLayout: React.FC = () => {
  // BotsDataProvider is mounted in App.tsx — no duplicate here
  return <MainLayoutContent />;
};

export default MainLayout;