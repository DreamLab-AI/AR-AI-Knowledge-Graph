/**
 * StatusCluster — top-right compact status cluster. design-spec.md §6.1, §9.1.
 *
 * At rest: a slim glass pill — health dot + agent-count badge + a SpacePilot
 * dot that only appears once a device is connected. Hover/focus/click expands
 * it into a glass flyout stacking the three existing status widgets
 * (SystemHealthIndicator, BotsStatusPanel, SpacePilotStatus) unchanged.
 *
 * The heavy status widgets mount only while expanded, so at rest the cluster
 * carries no live-subscription/polling cost of its own — the pill's health
 * dot is derived from the props ControlCenter already threads through, not
 * from a second websocket-store subscription.
 */

import React, { useCallback, useRef, useState } from 'react';
import { Activity, Bot } from 'lucide-react';
import { GlassPanel } from '../primitives/GlassPanel';
import { SystemHealthIndicator } from '../../visualisation/components/ControlPanel/SystemHealthIndicator';
import { BotsStatusPanel } from '../../visualisation/components/ControlPanel/BotsStatusPanel';
import { SpacePilotStatus } from '../../visualisation/components/ControlPanel/SpacePilotStatus';
import type { BotsData, GraphData } from '../../visualisation/components/ControlPanel/types';

export interface StatusClusterProps {
  graphData?: GraphData;
  botsData?: BotsData;
  mcpConnected?: boolean;
  websocketStatus?: 'connected' | 'connecting' | 'disconnected';
  metadataStatus?: 'loaded' | 'loading' | 'error' | 'none';
  /** SpacePilot / SpaceDriver state, wired in ControlCenter. */
  webHidAvailable?: boolean;
  spacePilotConnected?: boolean;
  spacePilotButtons?: string[];
  onConnectSpacePilot?: () => void;
}

/**
 * Mirrors SystemHealthIndicator's own `getStatusColor` bucketing
 * (green = fully connected, amber = connecting/loading, red = anything else)
 * so the collapsed dot never disagrees with the widget's verdict once
 * expanded. Deliberately does not read the websocket store's `lastActivity`
 * heartbeat (the expanded widget's job) — that would add a live subscription
 * to the at-rest pill, which the design explicitly avoids.
 */
function healthColor(
  websocketStatus: StatusClusterProps['websocketStatus'],
  metadataStatus: StatusClusterProps['metadataStatus'],
  hasNodes: boolean,
): string {
  if (websocketStatus === 'connected' && metadataStatus === 'loaded' && hasNodes) return '#22c55e';
  if (websocketStatus === 'connecting' || metadataStatus === 'loading') return '#f59e0b';
  return '#ef4444';
}

export const StatusCluster: React.FC<StatusClusterProps> = ({
  graphData,
  botsData,
  mcpConnected = false,
  websocketStatus = 'connected',
  metadataStatus = 'none',
  webHidAvailable = false,
  spacePilotConnected = false,
  spacePilotButtons = [],
  onConnectSpacePilot,
}) => {
  const [expanded, setExpanded] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const agentCount = botsData?.nodeCount ?? 0;
  const dotColor = healthColor(websocketStatus, metadataStatus, (graphData?.nodes?.length ?? 0) > 0);

  const handleBlurCapture = useCallback((e: React.FocusEvent<HTMLDivElement>) => {
    if (!containerRef.current?.contains(e.relatedTarget as Node)) setExpanded(false);
  }, []);

  return (
    <div
      ref={containerRef}
      className="fixed top-4 right-4 z-40 flex flex-col items-end gap-2"
      onMouseEnter={() => setExpanded(true)}
      onMouseLeave={() => setExpanded(false)}
      onFocusCapture={() => setExpanded(true)}
      onBlurCapture={handleBlurCapture}
    >
      <button
        type="button"
        data-testid="status-cluster"
        aria-label="System status"
        aria-expanded={expanded}
        onClick={() => setExpanded((v) => !v)}
        className="cc-glass flex items-center gap-2 px-3 py-1.5 rounded-full text-xs text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        <span
          className="inline-block h-2 w-2 rounded-full"
          style={{ background: dotColor }}
          aria-hidden="true"
        />
        <Activity size={13} aria-hidden="true" />
        <span className="cc-value-readout flex items-center gap-1">
          <Bot size={11} aria-hidden="true" />
          {agentCount.toLocaleString()}
        </span>
        {spacePilotConnected && (
          <span
            className="inline-block h-1.5 w-1.5 rounded-full"
            style={{ background: '#22c55e' }}
            role="img"
            aria-label="SpacePilot connected"
            title="SpacePilot connected"
          />
        )}
      </button>

      {expanded && (
        <GlassPanel
          data-testid="status-cluster-expanded"
          role="region"
          aria-label="System status detail"
          className="w-72 max-h-[72vh] overflow-y-auto p-2 text-[11px]"
        >
          <SystemHealthIndicator
            graphData={graphData}
            botsData={botsData}
            mcpConnected={mcpConnected}
            websocketStatus={websocketStatus}
            metadataStatus={metadataStatus}
          />
          <BotsStatusPanel botsData={botsData} />
          <SpacePilotStatus
            webHidAvailable={webHidAvailable}
            spacePilotConnected={spacePilotConnected}
            spacePilotButtons={spacePilotButtons}
            onConnect={onConnectSpacePilot ?? (() => {})}
          />
        </GlassPanel>
      )}
    </div>
  );
};

StatusCluster.displayName = 'StatusCluster';

export default StatusCluster;
