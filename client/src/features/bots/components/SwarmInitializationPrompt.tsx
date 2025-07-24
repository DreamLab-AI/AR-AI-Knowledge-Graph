import React, { useState } from 'react';
import { createLogger } from '../../../utils/logger';
import { apiService } from '../../../services/apiService';

const logger = createLogger('SwarmInitializationPrompt');

interface SwarmInitializationPromptProps {
  onClose: () => void;
  onInitialized: () => void;
}

export const SwarmInitializationPrompt: React.FC<SwarmInitializationPromptProps> = ({ 
  onClose, 
  onInitialized 
}) => {
  const [isLoading, setIsLoading] = useState(false);
  const [topology, setTopology] = useState<'mesh' | 'hierarchical' | 'ring' | 'star'>('mesh');
  const [maxAgents, setMaxAgents] = useState(8);
  const [enableNeural, setEnableNeural] = useState(true);
  const [agentTypes, setAgentTypes] = useState({
    coordinator: true,
    researcher: true,
    coder: true,
    analyst: true,
    tester: true,
    architect: true,
    optimizer: true,
    reviewer: false,
    documenter: false,
  });
  const [customPrompt, setCustomPrompt] = useState('');
  const [error, setError] = useState<string | null>(null);

  const handleInitialize = async () => {
    setIsLoading(true);
    setError(null);

    try {
      // Count selected agent types
      const selectedAgentTypes = Object.entries(agentTypes)
        .filter(([_, enabled]) => enabled)
        .map(([type, _]) => type);

      if (selectedAgentTypes.length === 0) {
        setError('Please select at least one agent type');
        setIsLoading(false);
        return;
      }

      if (!customPrompt.trim()) {
        setError('Please provide a task for the hive mind');
        setIsLoading(false);
        return;
      }

      // Prepare initialization configuration for hive mind
      const config = {
        topology,
        maxAgents,
        strategy: 'adaptive', // Adaptive hive mind strategy
        enableNeural,
        agentTypes: selectedAgentTypes,
        customPrompt: customPrompt.trim(),
      };

      logger.info('Spawning hive mind with config:', config);

      // Call API to initialize swarm
      const response = await apiService.post('/bots/initialize-swarm', config);

      if (response.success) {
        logger.info('Hive mind spawned successfully:', response);
        onInitialized();
      } else {
        throw new Error(response.error || 'Failed to spawn hive mind');
      }
    } catch (err) {
      logger.error('Failed to spawn hive mind:', err);
      setError(err instanceof Error ? err.message : 'Failed to spawn hive mind');
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div style={{
      position: 'fixed',
      top: '50%',
      left: '50%',
      transform: 'translate(-50%, -50%)',
      backgroundColor: 'rgba(20, 20, 20, 0.98)',
      border: '1px solid #F1C40F',
      borderRadius: '8px',
      padding: '20px',
      maxWidth: '500px',
      width: '90%',
      maxHeight: '80vh',
      overflow: 'auto',
      zIndex: 10000,
      boxShadow: '0 4px 20px rgba(241, 196, 15, 0.3)',
    }}>
      <h2 style={{ 
        margin: '0 0 20px 0', 
        color: '#F1C40F',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between'
      }}>
        <span>🧠 Spawn Hive Mind</span>
        <button
          onClick={onClose}
          style={{
            background: 'transparent',
            border: 'none',
            color: '#E74C3C',
            fontSize: '24px',
            cursor: 'pointer',
            padding: '0',
            width: '30px',
            height: '30px',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
          }}
        >
          ×
        </button>
      </h2>

      {error && (
        <div style={{
          backgroundColor: 'rgba(231, 76, 60, 0.2)',
          border: '1px solid #E74C3C',
          borderRadius: '4px',
          padding: '10px',
          marginBottom: '15px',
          color: '#E74C3C',
        }}>
          {error}
        </div>
      )}

      <div style={{ marginBottom: '20px' }}>
        <label style={{ display: 'block', marginBottom: '8px', color: '#F1C40F' }}>
          Topology
        </label>
        <select
          value={topology}
          onChange={(e) => setTopology(e.target.value as any)}
          style={{
            width: '100%',
            padding: '8px',
            backgroundColor: 'rgba(255, 255, 255, 0.1)',
            border: '1px solid rgba(255, 255, 255, 0.3)',
            borderRadius: '4px',
            color: 'white',
          }}
        >
          <option value="mesh">Mesh - Fully connected, best for collaboration</option>
          <option value="hierarchical">Hierarchical - Structured with clear command chain</option>
          <option value="ring">Ring - Sequential processing pipeline</option>
          <option value="star">Star - Central coordinator with workers</option>
        </select>
      </div>

      <div style={{ marginBottom: '20px' }}>
        <label style={{ display: 'block', marginBottom: '8px', color: '#F1C40F' }}>
          Maximum Agents: {maxAgents}
        </label>
        <input
          type="range"
          min="3"
          max="20"
          value={maxAgents}
          onChange={(e) => setMaxAgents(Number(e.target.value))}
          style={{ width: '100%' }}
        />
      </div>

      <div style={{ marginBottom: '20px' }}>
        <label style={{ display: 'block', marginBottom: '8px', color: '#F1C40F' }}>
          Agent Types
        </label>
        <div style={{ 
          display: 'grid', 
          gridTemplateColumns: 'repeat(2, 1fr)', 
          gap: '8px',
          fontSize: '14px',
        }}>
          {Object.entries(agentTypes).map(([type, enabled]) => (
            <label key={type} style={{ 
              display: 'flex', 
              alignItems: 'center',
              cursor: 'pointer',
              color: enabled ? '#F1C40F' : 'rgba(255, 255, 255, 0.5)',
            }}>
              <input
                type="checkbox"
                checked={enabled}
                onChange={(e) => setAgentTypes(prev => ({
                  ...prev,
                  [type]: e.target.checked
                }))}
                style={{ marginRight: '8px' }}
              />
              {type.charAt(0).toUpperCase() + type.slice(1)}
            </label>
          ))}
        </div>
      </div>

      <div style={{ marginBottom: '20px' }}>
        <label style={{ 
          display: 'flex', 
          alignItems: 'center',
          marginBottom: '8px',
          color: '#F1C40F',
          cursor: 'pointer',
        }}>
          <input
            type="checkbox"
            checked={enableNeural}
            onChange={(e) => setEnableNeural(e.target.checked)}
            style={{ marginRight: '8px' }}
          />
          Enable Neural Enhancements
        </label>
        <div style={{ 
          fontSize: '12px', 
          color: 'rgba(255, 255, 255, 0.6)',
          marginLeft: '24px',
        }}>
          Activates WASM-accelerated neural networks for collective intelligence
        </div>
      </div>

      <div style={{ marginBottom: '20px' }}>
        <label style={{ display: 'block', marginBottom: '8px', color: '#F1C40F' }}>
          Task for Hive Mind <span style={{ color: '#E74C3C' }}>*</span>
        </label>
        <textarea
          value={customPrompt}
          onChange={(e) => setCustomPrompt(e.target.value)}
          placeholder="Describe the task for the hive mind to accomplish..."
          required
          style={{
            width: '100%',
            minHeight: '100px',
            padding: '8px',
            backgroundColor: 'rgba(255, 255, 255, 0.1)',
            border: '1px solid rgba(255, 255, 255, 0.3)',
            borderRadius: '4px',
            color: 'white',
            resize: 'vertical',
          }}
        />
        <div style={{ 
          fontSize: '12px', 
          color: 'rgba(255, 255, 255, 0.6)',
          marginTop: '4px',
        }}>
          Example: "Build a REST API with user authentication and database integration"
        </div>
      </div>

      <div style={{ display: 'flex', gap: '10px', justifyContent: 'flex-end' }}>
        <button
          onClick={onClose}
          disabled={isLoading}
          style={{
            padding: '10px 20px',
            backgroundColor: 'rgba(255, 255, 255, 0.1)',
            border: '1px solid rgba(255, 255, 255, 0.3)',
            borderRadius: '4px',
            color: 'white',
            cursor: isLoading ? 'not-allowed' : 'pointer',
            opacity: isLoading ? 0.5 : 1,
          }}
        >
          Cancel
        </button>
        <button
          onClick={handleInitialize}
          disabled={isLoading}
          style={{
            padding: '10px 20px',
            backgroundColor: '#F1C40F',
            border: 'none',
            borderRadius: '4px',
            color: 'black',
            fontWeight: 'bold',
            cursor: isLoading ? 'not-allowed' : 'pointer',
            opacity: isLoading ? 0.8 : 1,
          }}
        >
          {isLoading ? 'Spawning...' : 'Spawn Hive Mind'}
        </button>
      </div>
    </div>
  );
};