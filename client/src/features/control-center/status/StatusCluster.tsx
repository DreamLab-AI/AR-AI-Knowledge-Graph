/**
 * StatusCluster — top-right compact status cluster. design-spec.md §6.1.
 *
 * ⚠️ PLACEHOLDER (owned by WP2, finalised by WP3). This is the minimal §6.1
 * surface: a compact glass pill that expands on hover/focus to reveal the three
 * existing status widgets (SystemHealthIndicator, BotsStatusPanel,
 * SpacePilotStatus) unchanged. WP3 replaces this file with the finished cluster;
 * it MUST preserve the `StatusClusterProps` contract below, against which
 * ControlCenter already renders.
 *
 * The heavy status widgets mount only while expanded, so at rest the cluster
 * carries no live-subscription / polling cost.
 */

import React, { useState } from 'react';
import { Activity } from 'lucide-react';
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
  const healthy = websocketStatus === 'connected' && (graphData?.nodes?.length ?? 0) > 0;

  return (
    <div
      data-testid="status-cluster"
      className="fixed top-4 right-4 z-40 flex flex-col items-end gap-2"
      onMouseEnter={() => setExpanded(true)}
      onMouseLeave={() => setExpanded(false)}
      onFocusCapture={() => setExpanded(true)}
      onBlurCapture={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node)) setExpanded(false);
      }}
    >
      <button
        type="button"
        aria-label="System status"
        aria-expanded={expanded}
        onClick={() => setExpanded((v) => !v)}
        className="cc-glass flex items-center gap-2 px-3 py-1.5 rounded-full text-xs text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        <Activity size={13} aria-hidden="true" />
        <span
          className="inline-block h-2 w-2 rounded-full"
          style={{ background: healthy ? '#10b981' : '#f59e0b' }}
          aria-hidden="true"
        />
        <span className="cc-value-readout">
          {(graphData?.nodes?.length ?? 0).toLocaleString()} nodes
        </span>
      </button>

      {expanded && (
        <GlassPanel
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
