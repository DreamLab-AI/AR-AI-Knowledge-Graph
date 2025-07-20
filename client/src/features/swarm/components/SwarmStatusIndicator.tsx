import React from 'react';
import { Html } from '@react-three/drei';

interface SwarmStatusIndicatorProps {
  agentCount: number;
  edgeCount: number;
  totalTokens: number;
  connected: boolean;
}

export const SwarmStatusIndicator: React.FC<SwarmStatusIndicatorProps> = ({
  agentCount,
  edgeCount,
  totalTokens,
  connected
}) => {
  return (
    <Html position={[0, 20, 0]} center>
      <div style={{
        background: 'rgba(0, 0, 0, 0.8)',
        padding: '10px',
        borderRadius: '5px',
        color: '#fff',
        fontFamily: 'monospace',
        fontSize: '12px',
        minWidth: '200px',
        border: `2px solid ${connected ? '#2ECC71' : '#E74C3C'}`
      }}>
        <h3 style={{ margin: '0 0 10px 0', color: '#F1C40F' }}>🐝 Swarm Status</h3>
        <div style={{ display: 'grid', gridTemplateColumns: 'auto auto', gap: '5px' }}>
          <span>Status:</span>
          <span style={{ color: connected ? '#2ECC71' : '#E74C3C' }}>
            {connected ? 'Connected' : 'Disconnected'}
          </span>
          
          <span>Agents:</span>
          <span style={{ color: '#F1C40F' }}>{agentCount}</span>
          
          <span>Communications:</span>
          <span style={{ color: '#F1C40F' }}>{edgeCount}</span>
          
          <span>Total Tokens:</span>
          <span style={{ color: '#F39C12' }}>{totalTokens.toLocaleString()}</span>
        </div>
      </div>
    </Html>
  );
};