import { createMockBotsState, updateAgentMetrics, generateMockCommunications, MOCK_BOTS_DATA } from './mockBotsData';
import type { BotsState } from '../types/BotsTypes';

/**
 * Example usage of the mock data for bots visualization
 * This demonstrates how to integrate the mock data with the visualization components
 */

// Initialize the mock bots state
export function initializeMockBots(): BotsState {
  return createMockBotsState();
}

// Update loop for dynamic animation (call this in requestAnimationFrame or setInterval)
export function updateMockBotsAnimation(state: BotsState, deltaTime: number): void {
  // Update each agent's metrics
  state.agents.forEach(agent => {
    updateAgentMetrics(agent, deltaTime);
  });

  // Generate new communications periodically (every 3 seconds)
  if (Date.now() - state.lastUpdate > 3000) {
    const agents = Array.from(state.agents.values());
    const newCommunications = generateMockCommunications(agents);
    
    // Add new communications to the state
    state.communications = [...state.communications.slice(-50), ...newCommunications]; // Keep last 50
    
    // Update edges based on new communications
    newCommunications.forEach(comm => {
      comm.receivers.forEach(receiver => {
        const edgeId = [comm.sender, receiver].sort().join('-');
        const edge = state.edges.get(edgeId);
        
        if (edge) {
          edge.dataVolume += comm.metadata.size;
          edge.messageCount += 1;
          edge.lastMessageTime = Date.now();
        } else {
          state.edges.set(edgeId, {
            id: edgeId,
            source: comm.sender,
            target: receiver,
            dataVolume: comm.metadata.size,
            messageCount: 1,
            lastMessageTime: Date.now()
          });
        }
      });
    });

    state.lastUpdate = Date.now();
  }

  // Update token usage periodically
  if (Math.random() < 0.01) { // 1% chance per frame
    const agents = Array.from(state.agents.values());
    state.tokenUsage = MOCK_BOTS_DATA.generateTokenUsage(agents);
  }
}

// Example React component integration
export const mockDataIntegrationExample = `
import React, { useEffect, useState, useRef } from 'react';
import { initializeMockBots, updateMockBotsAnimation } from './mockDataUsageExample';
import type { BotsState } from '../types/BotsTypes';

export function BotsVisualizationWithMockData() {
  const [botsState, setBotsState] = useState<BotsState>(initializeMockBots());
  const animationFrameRef = useRef<number>();
  const lastTimeRef = useRef<number>(Date.now());

  useEffect(() => {
    // Animation loop
    const animate = () => {
      const now = Date.now();
      const deltaTime = now - lastTimeRef.current;
      lastTimeRef.current = now;

      // Update the mock data
      updateMockBotsAnimation(botsState, deltaTime);

      // Trigger re-render periodically (every 100ms for smooth animation)
      if (deltaTime > 100) {
        setBotsState({ ...botsState });
      }

      animationFrameRef.current = requestAnimationFrame(animate);
    };

    animationFrameRef.current = requestAnimationFrame(animate);

    return () => {
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
      }
    };
  }, [botsState]);

  // Render your visualization using botsState
  return (
    <div>
      {/* Your Three.js or D3 visualization here */}
      <div>Total Agents: {botsState.agents.size}</div>
      <div>Active Communications: {botsState.communications.length}</div>
      <div>Total Tokens Used: {botsState.tokenUsage.total}</div>
    </div>
  );
}
`;

// Helper function to get agents by role type
export function getAgentsByRole(state: BotsState, roleType: 'meta' | 'primary') {
  const roles = roleType === 'meta' 
    ? ['coordinator', 'analyst', 'architect', 'optimizer', 'monitor']
    : ['coder', 'tester', 'researcher', 'reviewer', 'documenter', 'specialist'];
  
  return Array.from(state.agents.values()).filter(agent => 
    roles.includes(agent.type)
  );
}

// Get agent status summary
export function getAgentStatusSummary(state: BotsState) {
  const summary = {
    total: state.agents.size,
    byStatus: {
      idle: 0,
      busy: 0,
      error: 0,
      initializing: 0,
      terminating: 0
    },
    byType: {} as Record<string, number>,
    avgHealth: 0,
    avgCpuUsage: 0,
    avgMemoryUsage: 0
  };

  let totalHealth = 0;
  let totalCpu = 0;
  let totalMemory = 0;

  state.agents.forEach(agent => {
    summary.byStatus[agent.status]++;
    summary.byType[agent.type] = (summary.byType[agent.type] || 0) + 1;
    totalHealth += agent.health;
    totalCpu += agent.cpuUsage;
    totalMemory += agent.memoryUsage;
  });

  summary.avgHealth = totalHealth / state.agents.size;
  summary.avgCpuUsage = totalCpu / state.agents.size;
  summary.avgMemoryUsage = totalMemory / state.agents.size;

  return summary;
}

// Get communication patterns analysis
export function getCommunicationAnalysis(state: BotsState) {
  const analysis = {
    totalCommunications: state.communications.length,
    totalDataVolume: 0,
    communicationsByType: {} as Record<string, number>,
    mostActiveAgents: [] as Array<{ agentId: string, messageCount: number }>,
    networkDensity: 0
  };

  // Count messages per agent
  const agentMessageCount = new Map<string, number>();

  state.communications.forEach(comm => {
    analysis.communicationsByType[comm.metadata.type || 'unknown'] = 
      (analysis.communicationsByType[comm.metadata.type || 'unknown'] || 0) + 1;
    
    // Count sender messages
    agentMessageCount.set(comm.sender, (agentMessageCount.get(comm.sender) || 0) + 1);
  });

  // Calculate total data volume from edges
  state.edges.forEach(edge => {
    analysis.totalDataVolume += edge.dataVolume;
  });

  // Get most active agents
  analysis.mostActiveAgents = Array.from(agentMessageCount.entries())
    .map(([agentId, messageCount]) => ({ agentId, messageCount }))
    .sort((a, b) => b.messageCount - a.messageCount)
    .slice(0, 5);

  // Calculate network density (actual edges / possible edges)
  const possibleEdges = (state.agents.size * (state.agents.size - 1)) / 2;
  analysis.networkDensity = state.edges.size / possibleEdges;

  return analysis;
}