import React, { useEffect, useState } from 'react';
import { apiService } from '../../../services/api';

export const SwarmTest: React.FC = () => {
  const [data, setData] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const fetchData = async () => {
      try {
        console.log('[SWARM TEST] Fetching swarm data...');
        const response = await apiService.getSwarmData();
        console.log('[SWARM TEST] Response:', response);
        setData(response);
      } catch (err) {
        console.error('[SWARM TEST] Error:', err);
        setError(err?.toString() || 'Unknown error');
      } finally {
        setLoading(false);
      }
    };

    fetchData();
  }, []);

  return (
    <div style={{
      position: 'fixed',
      top: '10px',
      right: '10px',
      background: 'rgba(0, 0, 0, 0.9)',
      color: 'white',
      padding: '15px',
      borderRadius: '8px',
      border: '2px solid #F1C40F',
      fontFamily: 'monospace',
      fontSize: '12px',
      maxWidth: '300px',
      zIndex: 9999
    }}>
      <h4 style={{ margin: '0 0 10px 0', color: '#F1C40F' }}>🔍 Swarm API Test</h4>
      {loading && <div>Loading...</div>}
      {error && <div style={{ color: '#E74C3C' }}>Error: {error}</div>}
      {data && (
        <>
          <div>Nodes: {data.nodes?.length || 0}</div>
          <div>Edges: {data.edges?.length || 0}</div>
          <div>Has Mock Flag: {data._isMock ? 'Yes' : 'No'}</div>
          <div style={{ marginTop: '10px', fontSize: '10px' }}>
            <pre>{JSON.stringify(data, null, 2).slice(0, 200)}...</pre>
          </div>
        </>
      )}
    </div>
  );
};