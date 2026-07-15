/**
 * BotsVisualization.tsx
 * Thin orchestrator: wires BotsDataContext → per-agent positions →
 * BotsNode + BotsEdgeComponent renderers.
 *
 * Responsibilities:
 *   - Consume `useBotsData()` context (no WS/polling logic here)
 *   - Maintain `positionsRef` (server positions or initial circle layout)
 *   - Resolve colour palette from settings
 *   - Render loading / error / empty states
 *   - Delegate all 3-D rendering to BotsNode / BotsEdgeComponent
 */
import React, { useRef, useEffect, useState, useMemo } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { Html } from '@react-three/drei';
import { BotsAgent, BotsEdge, BotsState, TokenUsage } from '../types/BotsTypes';
import { createLogger } from '../../../utils/loggerConfig';
import { useTelemetry } from '../../../telemetry/useTelemetry';
import { agentTelemetry } from '../../../telemetry/AgentTelemetry';
import { useSettingsStore } from '../../../store/settingsStore';
import { useBotsData } from '../contexts/BotsDataContext';
import { getVisionClawColors } from './BotsShared';
import { BotsNode } from './BotsNode';
import { BotsEdgeComponent } from './BotsEdgeComponent';

const logger = createLogger('BotsVisualization');

// ---------------------------------------------------------------------------
// Main Visualization Component
// Note: pure rendering component — positions come from server physics via
// binary protocol. No client-side physics computation.
// ---------------------------------------------------------------------------
export const BotsVisualization: React.FC = () => {
  const settings = useSettingsStore(state => state.settings);
  // Per-swarm hue tint (control-centre Agents → Behaviour). Client-only, default on.
  const swarmTint = useSettingsStore(s => s.get<boolean>('visualisation.graphTypeVisuals.agent.swarmTint')) ?? true;
  const { botsData: contextBotsData } = useBotsData();
  const telemetry = useTelemetry('BotsVisualization');

  const [botsData, setBotsData] = useState<BotsState>({
    agents: new Map(),
    edges: new Map(),
    communications: [],
    tokenUsage: { total: 0, byAgent: {} },
    lastUpdate: 0,
  });
  const [isLoading, setIsLoading]     = useState(true);
  const [error]                       = useState<string | null>(null);
  const [_mcpConnected, setMcpConnected] = useState(false);

  // Positions keyed by agent ID — updated from server or assigned as initial circle layout
  const positionsRef = useRef<Map<string, THREE.Vector3>>(new Map());

  const colors = useMemo(
    () => getVisionClawColors(settings as unknown as Record<string, unknown> | undefined),
    [settings],
  );

  // Sync context data → local BotsState + positionsRef
  useEffect(() => {
    if (!contextBotsData) {
      logger.debug('[VISIONCLAW] No context data available yet');
      return;
    }

    logger.debug('[VISIONCLAW] Processing bots data from context', contextBotsData);
    setIsLoading(false);

    const agents = contextBotsData.agents || [];
    const agentMap = new Map<string, BotsAgent>();

    agents.forEach((agent, index) => {
      agentMap.set(agent.id, agent);

      agentTelemetry.logAgentAction(agent.id, agent.type, 'state_update', {
        status: agent.status, health: agent.health,
        cpuUsage: agent.cpuUsage, tokenRate: agent.tokenRate,
      });

      if (
        agent.position &&
        (agent.position.x !== undefined ||
         agent.position.y !== undefined ||
         agent.position.z !== undefined)
      ) {
        positionsRef.current.set(
          agent.id,
          new THREE.Vector3(
            agent.position.x || 0,
            agent.position.y || 0,
            agent.position.z || 0,
          ),
        );
      } else if (!positionsRef.current.has(agent.id)) {
        const radius = 25;
        const angle  = (index / agents.length) * Math.PI * 2;
        const height = (Math.random() - 0.5) * 15;
        const newPosition = new THREE.Vector3(
          Math.cos(angle) * radius,
          height,
          Math.sin(angle) * radius,
        );
        positionsRef.current.set(agent.id, newPosition);

        agentTelemetry.logThreeJSOperation('position_update', agent.id,
          { x: newPosition.x, y: newPosition.y, z: newPosition.z },
          undefined,
          { reason: 'initial_calculation', agentType: agent.type, index, totalAgents: agents.length },
        );
      }
    });

    const edges   = contextBotsData.edges || [];
    const edgeMap = new Map<string, BotsEdge>();
    edges.forEach((edge: BotsEdge) => edgeMap.set(edge.id, edge));

    const contextRecord = contextBotsData as unknown as Record<string, unknown>;
    setBotsData({
      agents: agentMap,
      edges:  edgeMap,
      communications: [],
      tokenUsage: (contextRecord.tokenUsage as TokenUsage | undefined) || { total: 0, byAgent: {} },
      lastUpdate: Date.now(),
    });

    setMcpConnected(agentMap.size > 0);

    agentTelemetry.logAgentAction('visualization', 'system', 'data_update', {
      agentCount: agentMap.size,
      edgeCount:  edgeMap.size,
      hasContextData: !!contextBotsData,
    });
  }, [contextBotsData]);

  // Placeholder frame hook (reserved for future per-frame global logic)
  useFrame(() => {});

  // -------------------------------------------------------------------------
  // Render states
  // -------------------------------------------------------------------------
  if (error) {
    return (
      <Html center>
        <div style={{ color: '#E74C3C', padding: '20px', textAlign: 'center' }}>
          <h3>VisionClaw Error</h3>
          <p>{error}</p>
        </div>
      </Html>
    );
  }

  if (isLoading) {
    return (
      <Html center>
        <div style={{ color: '#F1C40F', padding: '20px', textAlign: 'center' }}>
          <h3>Loading VisionClaw...</h3>
          <p>Initializing hive mind visualization</p>
        </div>
      </Html>
    );
  }

  if (botsData.agents.size === 0) return null;

  // -------------------------------------------------------------------------
  // Main render
  // -------------------------------------------------------------------------
  return (
    <group>
      {/* Edges */}
      {Array.from(botsData.edges.values()).map(edge => {
        const sourcePos = positionsRef.current.get(edge.source);
        const targetPos = positionsRef.current.get(edge.target);
        if (!sourcePos || !targetPos) return null;

        return (
          <BotsEdgeComponent
            key={edge.id}
            edge={edge}
            sourcePos={sourcePos}
            targetPos={targetPos}
            color={colors.edge}
            sourceAgent={botsData.agents.get(edge.source)}
            targetAgent={botsData.agents.get(edge.target)}
          />
        );
      })}

      {/* Nodes */}
      {Array.from(botsData.agents.values()).map((node, index) => {
        const position = positionsRef.current.get(node.id);
        if (!position) return null;

        const nodeColor = colors.getAgentColor
          ? colors.getAgentColor(node.type)
          : ((colors as unknown as Record<string, string>)[node.type] || colors.coordinator);

        return (
          <BotsNode
            key={node.id}
            agent={node}
            position={position}
            index={index}
            color={nodeColor}
            swarmTint={swarmTint}
          />
        );
      })}
    </group>
  );
};
