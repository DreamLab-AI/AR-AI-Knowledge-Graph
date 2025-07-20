import React from 'react';
import { Html } from '@react-three/drei';

interface SwarmDebugInfoProps {
  isLoading: boolean;
  error: string | null;
  nodeCount: number;
  edgeCount: number;
  mcpConnected: boolean;
  dataSource: 'mcp' | 'api' | 'mock' | 'none';
}

export const SwarmDebugInfo: React.FC<SwarmDebugInfoProps> = ({
  isLoading,
  error,
  nodeCount,
  edgeCount,
  mcpConnected,
  dataSource
}) => {
  return (
    <Html position={[-50, 20, 0]} center>
      <div style={{
        background: 'rgba(0, 0, 0, 0.9)',
        padding: '15px',
        borderRadius: '5px',
        color: '#fff',
        fontFamily: 'monospace',
        fontSize: '11px',
        minWidth: '250px',
        border: '2px solid #E74C3C'
      }}>
        <h4 style={{ margin: '0 0 10px 0', color: '#E74C3C' }}>🔍 Swarm Debug Info</h4>
        <div>Loading: {isLoading ? '✅ Yes' : '❌ No'}</div>
        <div>Error: {error || 'None'}</div>
        <div>Data Source: {dataSource}</div>
        <div>MCP Connected: {mcpConnected ? '✅ Yes' : '❌ No'}</div>
        <div>Nodes: {nodeCount}</div>
        <div>Edges: {edgeCount}</div>
        <div style={{ marginTop: '10px', fontSize: '10px', color: '#999' }}>
          {new Date().toLocaleTimeString()}
        </div>
      </div>
    </Html>
  );
};