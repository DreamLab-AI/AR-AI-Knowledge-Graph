/**
 * Innovative Graph Features Integration
 * Main component that integrates all world-class creative features for dual graph visualization
 */

import React, { useEffect, useState, useRef, useCallback, useMemo } from 'react';
import { useThree, useFrame } from '@react-three/fiber';
import { Html } from '@react-three/drei';
import { Vector3, Color, Camera } from 'three';
import { graphSynchronization, type SyncOptions } from '../services/graphSynchronization';
import { graphComparison, type NodeMatch, type GraphDifference, type SimilarityAnalysis } from '../services/graphComparison';
import { graphAnimations, type AnimationOptions } from '../services/graphAnimations';
import { aiInsights, type LayoutOptimization, type ClusterDetection, type NodeRecommendation } from '../services/aiInsights';
import { advancedInteractionModes, type TimeTravelState, type ExplorationState, type CollaborationState } from '../services/advancedInteractionModes';
import { useSettingsStore } from '../../../store/settingsStore';
import { createLogger } from '../../../utils/logger';
import type { GraphData } from '../managers/graphDataManager';

const logger = createLogger('InnovativeGraphFeatures');

interface InnovativeGraphFeaturesProps {
  graphId: string;
  graphData: GraphData;
  otherGraphData?: GraphData;
  isVisible: boolean;
  camera: Camera;
  onFeatureUpdate?: (feature: string, data: any) => void;
}

interface FeatureState {
  synchronization: {
    enabled: boolean;
    options: SyncOptions;
  };
  comparison: {
    enabled: boolean;
    matches: NodeMatch[];
    differences: GraphDifference | null;
    analysis: SimilarityAnalysis | null;
    highlighting: boolean;
  };
  animations: {
    enabled: boolean;
    nodeAnimations: Map<string, any>;
    transitionQueue: string[];
  };
  aiInsights: {
    enabled: boolean;
    optimization: LayoutOptimization | null;
    clusters: ClusterDetection | null;
    recommendations: NodeRecommendation[];
    autoOptimize: boolean;
  };
  interactionModes: {
    timeTravel: TimeTravelState;
    exploration: ExplorationState;
    collaboration: CollaborationState;
    vrAr: boolean;
  };
}

const InnovativeGraphFeatures: React.FC<InnovativeGraphFeaturesProps> = ({
  graphId,
  graphData,
  otherGraphData,
  isVisible,
  camera,
  onFeatureUpdate
}) => {
  const { scene } = useThree();
  const settings = useSettingsStore(state => state.settings);
  
  const [featureState, setFeatureState] = useState<FeatureState>({
    synchronization: {
      enabled: false,
      options: {
        enableCameraSync: true,
        enableSelectionSync: true,
        enableZoomSync: true,
        enablePanSync: true,
        smoothTransitions: true,
        transitionDuration: 300
      }
    },
    comparison: {
      enabled: false,
      matches: [],
      differences: null,
      analysis: null,
      highlighting: false
    },
    animations: {
      enabled: true,
      nodeAnimations: new Map(),
      transitionQueue: []
    },
    aiInsights: {
      enabled: false,
      optimization: null,
      clusters: null,
      recommendations: [],
      autoOptimize: false
    },
    interactionModes: {
      timeTravel: advancedInteractionModes.getTimeTravelState(),
      exploration: advancedInteractionModes.getExplorationState(),
      collaboration: advancedInteractionModes.getCollaborationState(),
      vrAr: false
    }
  });

  const lastCameraUpdate = useRef<number>(0);
  const animationFrameRef = useRef<number>();

  // Initialize feature systems
  useEffect(() => {
    logger.info(`Initializing innovative features for graph: ${graphId}`);
    
    // Start animation system
    graphAnimations.start();
    
    // Subscribe to synchronization updates
    const unsubscribeSync = graphSynchronization.subscribe(graphId, (syncState) => {
      if (syncState.interaction.lastUpdate > lastCameraUpdate.current) {
        // Apply sync updates to camera
        camera.position.copy(syncState.camera.position);
        camera.zoom = syncState.camera.zoom;
        camera.updateProjectionMatrix();
        lastCameraUpdate.current = syncState.interaction.lastUpdate;
      }
    });

    // Subscribe to interaction mode updates
    const unsubscribeInteraction = advancedInteractionModes.on('stateChanged', (newState) => {
      setFeatureState(prev => ({
        ...prev,
        interactionModes: {
          timeTravel: newState.timeTravel || prev.interactionModes.timeTravel,
          exploration: newState.exploration || prev.interactionModes.exploration,
          collaboration: newState.collaboration || prev.interactionModes.collaboration,
          vrAr: newState.vrAr || prev.interactionModes.vrAr
        }
      }));
    });

    return () => {
      unsubscribeSync();
      unsubscribeInteraction();
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
      }
    };
  }, [graphId, camera]);

  // Graph comparison effect
  useEffect(() => {
    if (featureState.comparison.enabled && otherGraphData) {
      performGraphComparison();
    }
  }, [featureState.comparison.enabled, graphData, otherGraphData]);

  // AI insights effect
  useEffect(() => {
    if (featureState.aiInsights.enabled && featureState.aiInsights.autoOptimize) {
      performAIOptimization();
    }
  }, [featureState.aiInsights.enabled, featureState.aiInsights.autoOptimize, graphData]);

  // Animation frame loop
  useFrame((state, delta) => {
    if (featureState.animations.enabled) {
      updateAnimations(state.clock.elapsedTime, delta);
    }
  });

  const performGraphComparison = async () => {
    if (!otherGraphData) return;

    try {
      logger.info('Performing graph comparison');
      
      // Find node matches
      const matches = await graphComparison.findNodeMatches(graphData, otherGraphData);
      
      // Create relationship bridges
      const bridges = graphComparison.createRelationshipBridges(matches, graphData, otherGraphData);
      
      // Analyze differences
      const differences = graphComparison.analyzeDifferences(graphData, otherGraphData, matches);
      
      // Perform similarity analysis
      const analysis = graphComparison.performSimilarityAnalysis(graphData, otherGraphData, matches);
      
      setFeatureState(prev => ({
        ...prev,
        comparison: {
          ...prev.comparison,
          matches,
          differences,
          analysis
        }
      }));

      onFeatureUpdate?.('comparison', { matches, bridges, differences, analysis });
      
    } catch (error) {
      logger.error('Error performing graph comparison:', error);
    }
  };

  const performAIOptimization = async () => {
    try {
      logger.info('Performing AI optimization');
      
      // Get current node positions
      const currentPositions = new Map<string, Vector3>();
      graphData.nodes.forEach(node => {
        if (node.position) {
          currentPositions.set(node.id, new Vector3(node.position.x, node.position.y, node.position.z));
        }
      });

      // Optimize layout
      const optimization = await aiInsights.optimizeLayout(graphData, currentPositions);
      
      // Detect clusters
      const clusters = await aiInsights.detectClusters(graphData);
      
      // Generate recommendations
      const recommendations = await aiInsights.generateNodeRecommendations(graphData);
      
      setFeatureState(prev => ({
        ...prev,
        aiInsights: {
          ...prev.aiInsights,
          optimization,
          clusters,
          recommendations
        }
      }));

      onFeatureUpdate?.('aiInsights', { optimization, clusters, recommendations });
      
    } catch (error) {
      logger.error('Error performing AI optimization:', error);
    }
  };

  const updateAnimations = (elapsedTime: number, delta: number) => {
    // Update node animations
    featureState.animations.nodeAnimations.forEach((animation, nodeId) => {
      const animationValues = graphAnimations.getNodeAnimationValues(nodeId, elapsedTime);
      // Apply animation values to nodes (would need access to mesh instances)
    });

    // Process animation queue
    if (featureState.animations.transitionQueue.length > 0) {
      // Process queued animations
    }
  };

  // Feature control functions
  const toggleSynchronization = useCallback((enabled: boolean) => {
    setFeatureState(prev => ({
      ...prev,
      synchronization: { ...prev.synchronization, enabled }
    }));

    if (enabled) {
      graphSynchronization.updateSyncOptions(featureState.synchronization.options);
    }
  }, [featureState.synchronization.options]);

  const updateSyncOptions = useCallback((options: Partial<SyncOptions>) => {
    const newOptions = { ...featureState.synchronization.options, ...options };
    setFeatureState(prev => ({
      ...prev,
      synchronization: { ...prev.synchronization, options: newOptions }
    }));
    graphSynchronization.updateSyncOptions(newOptions);
  }, [featureState.synchronization.options]);

  const toggleComparison = useCallback((enabled: boolean) => {
    setFeatureState(prev => ({
      ...prev,
      comparison: { ...prev.comparison, enabled }
    }));
  }, []);

  const toggleAnimations = useCallback((enabled: boolean) => {
    setFeatureState(prev => ({
      ...prev,
      animations: { ...prev.animations, enabled }
    }));

    if (enabled) {
      graphAnimations.start();
    } else {
      graphAnimations.stop();
    }
  }, []);

  const toggleAIInsights = useCallback((enabled: boolean, autoOptimize = false) => {
    setFeatureState(prev => ({
      ...prev,
      aiInsights: { ...prev.aiInsights, enabled, autoOptimize }
    }));
  }, []);

  const animateNodeAppearance = useCallback((nodeId: string, options?: Partial<AnimationOptions>) => {
    graphAnimations.animateNodeAppearance(nodeId, options);
    
    setFeatureState(prev => ({
      ...prev,
      animations: {
        ...prev.animations,
        nodeAnimations: new Map(prev.animations.nodeAnimations).set(nodeId, { type: 'appear', ...options })
      }
    }));
  }, []);

  const animateGraphTransition = useCallback(async (show: boolean) => {
    if (!isVisible && show) {
      await graphAnimations.animateGraphTransition(graphId, true);
    } else if (isVisible && !show) {
      await graphAnimations.animateGraphTransition(graphId, false);
    }
  }, [graphId, isVisible]);

  const startTimeTravelMode = useCallback((graphStates: GraphData[]) => {
    advancedInteractionModes.activateTimeTravelMode(graphStates, {
      onStateChange: (step, graphData) => {
        onFeatureUpdate?.('timeTravel', { step, graphData });
      }
    });
  }, [onFeatureUpdate]);

  const createExplorationTour = useCallback((tourId: string, waypoints: any[]) => {
    advancedInteractionModes.createExplorationTour(tourId, waypoints);
  }, []);

  const startCollaboration = useCallback((sessionId: string) => {
    advancedInteractionModes.startCollaborationSession(sessionId);
  }, []);

  const activateVRMode = useCallback(() => {
    advancedInteractionModes.activateVRMode({
      handTracking: true,
      eyeTracking: true,
      hapticFeedback: true,
      spatialAudio: true,
      immersiveUI: true
    });
    
    setFeatureState(prev => ({
      ...prev,
      interactionModes: { ...prev.interactionModes, vrAr: true }
    }));
  }, []);

  // Feature panels
  const SynchronizationPanel = useMemo(() => (
    <div className="innovative-feature-panel synchronization-panel">
      <h3>Graph Synchronization</h3>
      <div className="feature-controls">
        <label>
          <input
            type="checkbox"
            checked={featureState.synchronization.enabled}
            onChange={(e) => toggleSynchronization(e.target.checked)}
          />
          Enable Sync
        </label>
        
        {featureState.synchronization.enabled && (
          <div className="sync-options">
            <label>
              <input
                type="checkbox"
                checked={featureState.synchronization.options.enableCameraSync}
                onChange={(e) => updateSyncOptions({ enableCameraSync: e.target.checked })}
              />
              Camera Sync
            </label>
            <label>
              <input
                type="checkbox"
                checked={featureState.synchronization.options.enableSelectionSync}
                onChange={(e) => updateSyncOptions({ enableSelectionSync: e.target.checked })}
              />
              Selection Sync
            </label>
            <label>
              <input
                type="checkbox"
                checked={featureState.synchronization.options.enableZoomSync}
                onChange={(e) => updateSyncOptions({ enableZoomSync: e.target.checked })}
              />
              Zoom Sync
            </label>
          </div>
        )}
      </div>
    </div>
  ), [featureState.synchronization, toggleSynchronization, updateSyncOptions]);

  const ComparisonPanel = useMemo(() => (
    <div className="innovative-feature-panel comparison-panel">
      <h3>Graph Comparison</h3>
      <div className="feature-controls">
        <label>
          <input
            type="checkbox"
            checked={featureState.comparison.enabled}
            onChange={(e) => toggleComparison(e.target.checked)}
          />
          Enable Comparison
        </label>
        
        {featureState.comparison.enabled && featureState.comparison.analysis && (
          <div className="comparison-results">
            <div className="similarity-metrics">
              <p>Overall Similarity: {(featureState.comparison.analysis.overallSimilarity * 100).toFixed(1)}%</p>
              <p>Structural: {(featureState.comparison.analysis.structuralSimilarity * 100).toFixed(1)}%</p>
              <p>Semantic: {(featureState.comparison.analysis.semanticSimilarity * 100).toFixed(1)}%</p>
            </div>
            <div className="match-count">
              <p>Node Matches: {featureState.comparison.matches.length}</p>
            </div>
          </div>
        )}
      </div>
    </div>
  ), [featureState.comparison, toggleComparison]);

  const AnimationPanel = useMemo(() => (
    <div className="innovative-feature-panel animation-panel">
      <h3>Animation System</h3>
      <div className="feature-controls">
        <label>
          <input
            type="checkbox"
            checked={featureState.animations.enabled}
            onChange={(e) => toggleAnimations(e.target.checked)}
          />
          Enable Animations
        </label>
        
        {featureState.animations.enabled && (
          <div className="animation-controls">
            <button onClick={() => animateGraphTransition(!isVisible)}>
              {isVisible ? 'Hide' : 'Show'} Graph
            </button>
            <div className="active-animations">
              <p>Active Animations: {featureState.animations.nodeAnimations.size}</p>
            </div>
          </div>
        )}
      </div>
    </div>
  ), [featureState.animations, toggleAnimations, animateGraphTransition, isVisible]);

  const AIInsightsPanel = useMemo(() => (
    <div className="innovative-feature-panel ai-insights-panel">
      <h3>AI Insights</h3>
      <div className="feature-controls">
        <label>
          <input
            type="checkbox"
            checked={featureState.aiInsights.enabled}
            onChange={(e) => toggleAIInsights(e.target.checked)}
          />
          Enable AI Insights
        </label>
        
        {featureState.aiInsights.enabled && (
          <div className="ai-controls">
            <label>
              <input
                type="checkbox"
                checked={featureState.aiInsights.autoOptimize}
                onChange={(e) => toggleAIInsights(true, e.target.checked)}
              />
              Auto Optimize
            </label>
            
            {featureState.aiInsights.optimization && (
              <div className="optimization-results">
                <p>Algorithm: {featureState.aiInsights.optimization.algorithmUsed}</p>
                <p>Confidence: {(featureState.aiInsights.optimization.confidence * 100).toFixed(1)}%</p>
              </div>
            )}
            
            {featureState.aiInsights.recommendations.length > 0 && (
              <div className="recommendations">
                <p>Recommendations: {featureState.aiInsights.recommendations.length}</p>
                <div className="recommendation-list">
                  {featureState.aiInsights.recommendations.slice(0, 3).map((rec, index) => (
                    <div key={index} className="recommendation-item">
                      <span className="rec-type">{rec.recommendationType}</span>
                      <span className="rec-confidence">{(rec.confidence * 100).toFixed(0)}%</span>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  ), [featureState.aiInsights, toggleAIInsights]);

  const InteractionPanel = useMemo(() => (
    <div className="innovative-feature-panel interaction-panel">
      <h3>Advanced Interactions</h3>
      <div className="feature-controls">
        <div className="interaction-modes">
          <button 
            onClick={() => startTimeTravelMode([])}
            disabled={featureState.interactionModes.timeTravel.isActive}
          >
            Time Travel Mode
          </button>
          
          <button 
            onClick={() => createExplorationTour('default', [])}
          >
            Create Tour
          </button>
          
          <button 
            onClick={() => startCollaboration('session-' + Date.now())}
            disabled={featureState.interactionModes.collaboration.isActive}
          >
            Start Collaboration
          </button>
          
          <button 
            onClick={activateVRMode}
            disabled={featureState.interactionModes.vrAr}
          >
            Activate VR
          </button>
        </div>
        
        <div className="interaction-status">
          {featureState.interactionModes.timeTravel.isActive && (
            <p>Time Travel: Step {featureState.interactionModes.timeTravel.currentStep} / {featureState.interactionModes.timeTravel.totalSteps}</p>
          )}
          {featureState.interactionModes.exploration.isActive && (
            <p>Exploration: {featureState.interactionModes.exploration.currentTour}</p>
          )}
          {featureState.interactionModes.collaboration.isActive && (
            <p>Collaboration: {featureState.interactionModes.collaboration.participants.length} participants</p>
          )}
        </div>
      </div>
    </div>
  ), [featureState.interactionModes, startTimeTravelMode, createExplorationTour, startCollaboration, activateVRMode]);

  return (
    <>
      {/* Feature Control Panels */}
      <Html
        position={[0, 25, 0]}
        center
        style={{
          background: 'rgba(0, 0, 0, 0.9)',
          borderRadius: '12px',
          padding: '20px',
          color: 'white',
          fontFamily: 'Inter, system-ui, sans-serif',
          pointerEvents: 'auto',
          userSelect: 'none',
          maxWidth: '800px',
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(250px, 1fr))',
          gap: '15px'
        }}
      >
        {SynchronizationPanel}
        {ComparisonPanel}
        {AnimationPanel}
        {AIInsightsPanel}
        {InteractionPanel}
      </Html>

      {/* Feature Status Indicator */}
      <Html
        position={[-30, 20, 0]}
        center
        style={{
          background: 'rgba(0, 0, 0, 0.8)',
          borderRadius: '8px',
          padding: '12px',
          color: 'white',
          fontFamily: 'Inter, system-ui, sans-serif',
          fontSize: '12px',
          pointerEvents: 'none'
        }}
      >
        <div className="feature-status">
          <h4>Active Features</h4>
          <div className="status-indicators">
            <div className={`status-item ${featureState.synchronization.enabled ? 'active' : ''}`}>
              🔄 Sync
            </div>
            <div className={`status-item ${featureState.comparison.enabled ? 'active' : ''}`}>
              🔍 Compare
            </div>
            <div className={`status-item ${featureState.animations.enabled ? 'active' : ''}`}>
              ✨ Animate
            </div>
            <div className={`status-item ${featureState.aiInsights.enabled ? 'active' : ''}`}>
              🧠 AI
            </div>
            <div className={`status-item ${Object.values(featureState.interactionModes).some(mode => 
              typeof mode === 'boolean' ? mode : mode.isActive
            ) ? 'active' : ''}`}>
              🎮 Interact
            </div>
          </div>
        </div>
      </Html>

      <style jsx>{`
        .innovative-feature-panel {
          background: rgba(255, 255, 255, 0.1);
          border-radius: 8px;
          padding: 15px;
          margin-bottom: 10px;
        }

        .innovative-feature-panel h3 {
          margin: 0 0 10px 0;
          font-size: 14px;
          font-weight: 600;
          color: #00ffff;
        }

        .feature-controls {
          display: flex;
          flex-direction: column;
          gap: 8px;
        }

        .feature-controls label {
          display: flex;
          align-items: center;
          gap: 8px;
          font-size: 12px;
          cursor: pointer;
        }

        .feature-controls input[type="checkbox"] {
          margin: 0;
        }

        .feature-controls button {
          padding: 6px 12px;
          background: rgba(0, 255, 255, 0.2);
          border: 1px solid rgba(0, 255, 255, 0.5);
          border-radius: 4px;
          color: white;
          font-size: 12px;
          cursor: pointer;
          transition: all 0.2s;
        }

        .feature-controls button:hover:not(:disabled) {
          background: rgba(0, 255, 255, 0.3);
          border-color: rgba(0, 255, 255, 0.8);
        }

        .feature-controls button:disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }

        .sync-options, .comparison-results, .animation-controls, .ai-controls, .interaction-modes {
          margin-top: 8px;
          padding-left: 16px;
          border-left: 2px solid rgba(0, 255, 255, 0.3);
        }

        .similarity-metrics, .match-count, .optimization-results, .recommendations {
          margin: 5px 0;
          font-size: 11px;
        }

        .recommendation-list {
          display: flex;
          flex-direction: column;
          gap: 4px;
          margin-top: 5px;
        }

        .recommendation-item {
          display: flex;
          justify-content: space-between;
          align-items: center;
          padding: 4px 8px;
          background: rgba(255, 255, 255, 0.1);
          border-radius: 4px;
        }

        .rec-type {
          font-size: 10px;
          text-transform: capitalize;
        }

        .rec-confidence {
          font-size: 10px;
          color: #00ff88;
          font-weight: 600;
        }

        .interaction-status {
          margin-top: 10px;
          padding-top: 8px;
          border-top: 1px solid rgba(255, 255, 255, 0.2);
          font-size: 11px;
        }

        .feature-status {
          text-align: center;
        }

        .feature-status h4 {
          margin: 0 0 8px 0;
          font-size: 12px;
          color: #00ffff;
        }

        .status-indicators {
          display: flex;
          flex-direction: column;
          gap: 4px;
        }

        .status-item {
          padding: 4px 8px;
          border-radius: 4px;
          font-size: 11px;
          background: rgba(255, 255, 255, 0.1);
          opacity: 0.5;
          transition: all 0.2s;
        }

        .status-item.active {
          opacity: 1;
          background: rgba(0, 255, 255, 0.2);
          color: #00ffff;
        }
      `}</style>
    </>
  );
};

export default InnovativeGraphFeatures;