import React, { useState, useMemo } from 'react';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/features/design-system/components/Tabs';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/features/design-system/components/Card';
import { Button } from '@/features/design-system/components/Button';
import { SearchInput } from '@/features/design-system/components/SearchInput';
import {
  Eye,
  Settings,
  Smartphone,
  Info,
  ChevronDown,
  ChevronUp,
  Check,
  Search,
  Keyboard,
  User,
} from 'lucide-react';
import { useSettingsStore } from '@/store/settingsStore';
import { SettingControlComponent } from '../SettingControlComponent';
import { settingsUIDefinition } from '../../config/settingsUIDefinition';
import { cn } from '@/utils/cn';
import { useKeyboardShortcuts } from '@/hooks/useKeyboardShortcuts';
import { KeyboardShortcutsModal } from '@/components/KeyboardShortcutsModal';
import { LoadingSpinner, LoadingOverlay } from '@/features/design-system/components/LoadingSpinner';
import { SkeletonSetting } from '@/features/design-system/components/LoadingSkeleton';
import { useErrorHandler } from '@/hooks/useErrorHandler';
import { useToast } from '@/features/design-system/components/Toast';
import { UndoRedoControls } from '../UndoRedoControls';
import NostrAuthSection from '../../../auth/components/NostrAuthSection';
import { useSelectiveSetting, useSettingSetter } from '@/hooks/useSelectiveSettingsStore';
import { VirtualizedSettingsGroup } from '../VirtualizedSettingsGroup';

interface SettingItem {
  key: string;
  path: string;
  definition: any;
  isPowerUser?: boolean;
}

interface SettingGroup {
  title: string;
  description?: string;
  items: SettingItem[];
  isPowerUser?: boolean;
}

interface SettingsPanelRedesignOptimizedProps {
  toggleLowerRightPaneDock: () => void;
  isLowerRightPaneDocked: boolean;
}

export function SettingsPanelRedesignOptimized({ toggleLowerRightPaneDock, isLowerRightPaneDocked }: SettingsPanelRedesignOptimizedProps) {
  const { isPowerUser } = useSettingsStore();
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set(['Node Appearance']));
  const [savedNotification, setSavedNotification] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [showShortcuts, setShowShortcuts] = useState(false);
  const [loadingSettings, setLoadingSettings] = useState<Set<string>>(new Set());
  const [isInitializing, setIsInitializing] = useState(true);
  const { handleError } = useErrorHandler();
  const { toast } = useToast();

  // Dynamically get background and text color from settings
  const panelBackground: string = useSelectiveSetting('visualisation.rendering.backgroundColor') ?? '#18181b';
  const panelForeground: string = useSelectiveSetting('visualisation.labels.textOutlineColor') ?? '#fff';
  const { set: setSetting } = useSettingSetter();

  // Organize settings into logical groups with better structure
  const settingsStructure = useMemo(() => ({
    appearance: {
      label: 'Appearance',
      icon: <Eye className="h-4 w-4" />,
      groups: [
        {
          title: 'Node Appearance',
          description: 'Customize how nodes look',
          items: [
            { key: 'baseColor', path: 'visualisation.nodes.baseColor', definition: settingsUIDefinition.visualisation.subsections.nodes.settings.baseColor },
            { key: 'opacity', path: 'visualisation.nodes.opacity', definition: settingsUIDefinition.visualisation.subsections.nodes.settings.opacity },
            { key: 'metalness', path: 'visualisation.nodes.metalness', definition: settingsUIDefinition.visualisation.subsections.nodes.settings.metalness },
            { key: 'roughness', path: 'visualisation.nodes.roughness', definition: settingsUIDefinition.visualisation.subsections.nodes.settings.roughness },
            { key: 'nodeSize', path: 'visualisation.nodes.nodeSize', definition: settingsUIDefinition.visualisation.subsections.nodes.settings.nodeSize },
            { key: 'enableInstancing', path: 'visualisation.nodes.enableInstancing', definition: settingsUIDefinition.visualisation.subsections.nodes.settings.enableInstancing },
            { key: 'enableHologram', path: 'visualisation.nodes.enableHologram', definition: settingsUIDefinition.visualisation.subsections.nodes.settings.enableHologram },
            { key: 'enableMetadataShape', path: 'visualisation.nodes.enableMetadataShape', definition: settingsUIDefinition.visualisation.subsections.nodes.settings.enableMetadataShape },
            { key: 'enableMetadataVisualisation', path: 'visualisation.nodes.enableMetadataVisualisation', definition: settingsUIDefinition.visualisation.subsections.nodes.settings.enableMetadataVisualisation },
          ]
        },
        {
          title: 'Edge Appearance',
          description: 'Customize connection lines',
          items: [
            { key: 'color', path: 'visualisation.edges.color', definition: settingsUIDefinition.visualisation.subsections.edges.settings.color },
            { key: 'opacity', path: 'visualisation.edges.opacity', definition: settingsUIDefinition.visualisation.subsections.edges.settings.opacity },
            { key: 'baseWidth', path: 'visualisation.edges.baseWidth', definition: settingsUIDefinition.visualisation.subsections.edges.settings.baseWidth },
            { key: 'enableArrows', path: 'visualisation.edges.enableArrows', definition: settingsUIDefinition.visualisation.subsections.edges.settings.enableArrows },
            { key: 'arrowSize', path: 'visualisation.edges.arrowSize', definition: settingsUIDefinition.visualisation.subsections.edges.settings.arrowSize },
            { key: 'widthRange', path: 'visualisation.edges.widthRange', definition: settingsUIDefinition.visualisation.subsections.edges.settings.widthRange },
            { key: 'enableFlowEffect', path: 'visualisation.edges.enableFlowEffect', definition: settingsUIDefinition.visualisation.subsections.edges.settings.enableFlowEffect },
            { key: 'flowSpeed', path: 'visualisation.edges.flowSpeed', definition: settingsUIDefinition.visualisation.subsections.edges.settings.flowSpeed },
            { key: 'flowIntensity', path: 'visualisation.edges.flowIntensity', definition: settingsUIDefinition.visualisation.subsections.edges.settings.flowIntensity },
            { key: 'glowStrength', path: 'visualisation.edges.glowStrength', definition: settingsUIDefinition.visualisation.subsections.edges.settings.glowStrength },
            { key: 'distanceIntensity', path: 'visualisation.edges.distanceIntensity', definition: settingsUIDefinition.visualisation.subsections.edges.settings.distanceIntensity },
            { key: 'useGradient', path: 'visualisation.edges.useGradient', definition: settingsUIDefinition.visualisation.subsections.edges.settings.useGradient },
            { key: 'gradientColors', path: 'visualisation.edges.gradientColors', definition: settingsUIDefinition.visualisation.subsections.edges.settings.gradientColors },
          ]
        },
        {
          title: 'Labels',
          description: 'Text display settings',
          items: [
            { key: 'enableLabels', path: settingsUIDefinition.visualisation.subsections.labels.settings.enableLabels.path, definition: settingsUIDefinition.visualisation.subsections.labels.settings.enableLabels },
            { key: 'desktopFontSize', path: 'visualisation.labels.desktopFontSize', definition: settingsUIDefinition.visualisation.subsections.labels.settings.desktopFontSize },
            { key: 'textColor', path: 'visualisation.labels.textColor', definition: settingsUIDefinition.visualisation.subsections.labels.settings.textColor },
            { key: 'textOutlineColor', path: 'visualisation.labels.textOutlineColor', definition: settingsUIDefinition.visualisation.subsections.labels.settings.textOutlineColor },
            { key: 'textOutlineWidth', path: 'visualisation.labels.textOutlineWidth', definition: settingsUIDefinition.visualisation.subsections.labels.settings.textOutlineWidth },
            { key: 'textResolution', path: 'visualisation.labels.textResolution', definition: settingsUIDefinition.visualisation.subsections.labels.settings.textResolution },
            { key: 'textPadding', path: 'visualisation.labels.textPadding', definition: settingsUIDefinition.visualisation.subsections.labels.settings.textPadding },
            { key: 'billboardMode', path: 'visualisation.labels.billboardMode', definition: settingsUIDefinition.visualisation.subsections.labels.settings.billboardMode },
          ]
        },
        {
          title: 'Visual Effects',
          description: 'Bloom and glow effects',
          items: [
            { key: 'bloomEnabled', path: 'visualisation.bloom.enabled', definition: settingsUIDefinition.visualisation.subsections.bloom.settings.enabled },
            { key: 'bloomStrength', path: 'visualisation.bloom.strength', definition: settingsUIDefinition.visualisation.subsections.bloom.settings.strength },
            { key: 'bloomRadius', path: 'visualisation.bloom.radius', definition: settingsUIDefinition.visualisation.subsections.bloom.settings.radius },
            { key: 'edgeBloomStrength', path: 'visualisation.bloom.edgeBloomStrength', definition: settingsUIDefinition.visualisation.subsections.bloom.settings.edgeBloomStrength },
            { key: 'environmentBloomStrength', path: 'visualisation.bloom.environmentBloomStrength', definition: settingsUIDefinition.visualisation.subsections.bloom.settings.environmentBloomStrength },
            { key: 'nodeBloomStrength', path: 'visualisation.bloom.nodeBloomStrength', definition: settingsUIDefinition.visualisation.subsections.bloom.settings.nodeBloomStrength },
            { key: 'threshold', path: 'visualisation.bloom.threshold', definition: settingsUIDefinition.visualisation.subsections.bloom.settings.threshold },
          ]
        },
        {
          title: 'Lighting & Rendering',
          description: 'Control lighting and background',
          items: [
            { key: 'ambientLightIntensity', path: 'visualisation.rendering.ambientLightIntensity', definition: settingsUIDefinition.visualisation.subsections.rendering.settings.ambientLightIntensity },
            { key: 'directionalLightIntensity', path: 'visualisation.rendering.directionalLightIntensity', definition: settingsUIDefinition.visualisation.subsections.rendering.settings.directionalLightIntensity },
            { key: 'environmentIntensity', path: 'visualisation.rendering.environmentIntensity', definition: settingsUIDefinition.visualisation.subsections.rendering.settings.environmentIntensity },
            { key: 'backgroundColor', path: 'visualisation.rendering.backgroundColor', definition: settingsUIDefinition.visualisation.subsections.rendering.settings.backgroundColor },
            { key: 'enableAmbientOcclusion', path: 'visualisation.rendering.enableAmbientOcclusion', definition: settingsUIDefinition.visualisation.subsections.rendering.settings.enableAmbientOcclusion, isPowerUser: true },
            { key: 'enableShadows', path: 'visualisation.rendering.enableShadows', definition: settingsUIDefinition.visualisation.subsections.rendering.settings.enableShadows, isPowerUser: true },
            { key: 'shadowMapSize', path: 'visualisation.rendering.shadowMapSize', definition: settingsUIDefinition.visualisation.subsections.rendering.settings.shadowMapSize, isPowerUser: true },
            { key: 'shadowBias', path: 'visualisation.rendering.shadowBias', definition: settingsUIDefinition.visualisation.subsections.rendering.settings.shadowBias, isPowerUser: true },
            { key: 'context', path: 'visualisation.rendering.context', definition: settingsUIDefinition.visualisation.subsections.rendering.settings.context },
          ]
        },
        {
          title: 'Hologram Effect',
          description: 'Control hologram visualization',
          items: [
            { key: 'ringCount', path: 'visualisation.hologram.ringCount', definition: settingsUIDefinition.visualisation.subsections.hologram.settings.ringCount },
            { key: 'ringColor', path: 'visualisation.hologram.ringColor', definition: settingsUIDefinition.visualisation.subsections.hologram.settings.ringColor },
            { key: 'ringOpacity', path: 'visualisation.hologram.ringOpacity', definition: settingsUIDefinition.visualisation.subsections.hologram.settings.ringOpacity },
            { key: 'sphereSizes', path: 'visualisation.hologram.sphereSizes', definition: settingsUIDefinition.visualisation.subsections.hologram.settings.sphereSizes },
            { key: 'ringRotationSpeed', path: 'visualisation.hologram.ringRotationSpeed', definition: settingsUIDefinition.visualisation.subsections.hologram.settings.ringRotationSpeed },
            { key: 'globalRotationSpeed', path: 'visualisation.hologram.globalRotationSpeed', definition: settingsUIDefinition.visualisation.subsections.hologram.settings.globalRotationSpeed },
          ]
        },
        {
          title: 'Animations',
          description: 'Animation controls',
          items: [
            { key: 'enableNodeAnimations', path: 'visualisation.animations.enableNodeAnimations', definition: settingsUIDefinition.visualisation.subsections.animations.settings.enableNodeAnimations },
            { key: 'enableMotionBlur', path: 'visualisation.animations.enableMotionBlur', definition: settingsUIDefinition.visualisation.subsections.animations.settings.enableMotionBlur, isPowerUser: true },
            { key: 'motionBlurStrength', path: 'visualisation.animations.motionBlurStrength', definition: settingsUIDefinition.visualisation.subsections.animations.settings.motionBlurStrength, isPowerUser: true },
            { key: 'selectionWaveEnabled', path: 'visualisation.animations.selectionWaveEnabled', definition: settingsUIDefinition.visualisation.subsections.animations.settings.selectionWaveEnabled },
            { key: 'pulseEnabled', path: 'visualisation.animations.pulseEnabled', definition: settingsUIDefinition.visualisation.subsections.animations.settings.pulseEnabled },
            { key: 'pulseSpeed', path: 'visualisation.animations.pulseSpeed', definition: settingsUIDefinition.visualisation.subsections.animations.settings.pulseSpeed },
            { key: 'pulseStrength', path: 'visualisation.animations.pulseStrength', definition: settingsUIDefinition.visualisation.subsections.animations.settings.pulseStrength },
            { key: 'waveSpeed', path: 'visualisation.animations.waveSpeed', definition: settingsUIDefinition.visualisation.subsections.animations.settings.waveSpeed },
          ]
        }
      ]
    },
    performance: {
      label: 'Performance',
      icon: <Settings className="h-4 w-4" />,
      groups: [
        {
          title: 'Rendering Quality',
          description: 'Balance quality and performance',
          items: [
            { key: 'nodeQuality', path: 'visualisation.nodes.quality', definition: settingsUIDefinition.visualisation.subsections.nodes.settings.quality },
            { key: 'edgeQuality', path: 'visualisation.edges.quality', definition: settingsUIDefinition.visualisation.subsections.edges.settings.quality },
            { key: 'enableAntialiasing', path: 'visualisation.rendering.enableAntialiasing', definition: settingsUIDefinition.visualisation.subsections.rendering.settings.enableAntialiasing },
          ]
        },
        {
          title: 'Physics Engine',
          description: 'Node movement behavior',
          items: [
            { key: 'physicsEnabled', path: 'visualisation.physics.enabled', definition: settingsUIDefinition.visualisation.subsections.physics.settings.enabled },
            { key: 'iterations', path: 'visualisation.physics.iterations', definition: settingsUIDefinition.visualisation.subsections.physics.settings.iterations },
            { key: 'damping', path: 'visualisation.physics.damping', definition: settingsUIDefinition.visualisation.subsections.physics.settings.damping },
            { key: 'boundsSize', path: 'visualisation.physics.boundsSize', definition: settingsUIDefinition.visualisation.subsections.physics.settings.boundsSize },
            { key: 'collisionRadius', path: 'visualisation.physics.collisionRadius', definition: settingsUIDefinition.visualisation.subsections.physics.settings.collisionRadius },
            { key: 'enableBounds', path: 'visualisation.physics.enableBounds', definition: settingsUIDefinition.visualisation.subsections.physics.settings.enableBounds },
            { key: 'maxVelocity', path: 'visualisation.physics.maxVelocity', definition: settingsUIDefinition.visualisation.subsections.physics.settings.maxVelocity },
            { key: 'repulsionDistance', path: 'visualisation.physics.repulsionDistance', definition: settingsUIDefinition.visualisation.subsections.physics.settings.repulsionDistance },
            { key: 'massScale', path: 'visualisation.physics.massScale', definition: settingsUIDefinition.visualisation.subsections.physics.settings.massScale },
            { key: 'boundaryDamping', path: 'visualisation.physics.boundaryDamping', definition: settingsUIDefinition.visualisation.subsections.physics.settings.boundaryDamping },
          ]
        },
        {
          title: 'Force Settings',
          description: 'Control attraction, repulsion, and spring forces',
          items: [
            { key: 'attractionStrength', path: 'visualisation.physics.attractionStrength', definition: settingsUIDefinition.visualisation.subsections.physics.settings.attractionStrength },
            { key: 'repulsionStrength', path: 'visualisation.physics.repulsionStrength', definition: settingsUIDefinition.visualisation.subsections.physics.settings.repulsionStrength },
            { key: 'springStrength', path: 'visualisation.physics.springStrength', definition: settingsUIDefinition.visualisation.subsections.physics.settings.springStrength },
          ]
        },
        {
          title: 'Network Settings',
          description: 'Connection optimization',
          items: [
            { key: 'updateRate', path: 'system.websocket.updateRate', definition: settingsUIDefinition.system.subsections.websocket.settings.updateRate },
            { key: 'compressionEnabled', path: 'system.websocket.compressionEnabled', definition: settingsUIDefinition.system.subsections.websocket.settings.compressionEnabled },
          ],
          isPowerUser: true
        }
      ]
    },
    xr: {
      label: 'XR/VR',
      icon: <Smartphone className="h-4 w-4" />,
      groups: [
        {
          title: 'XR Mode',
          description: 'Virtual reality settings',
          items: [
            { key: 'clientSideEnableXR', path: settingsUIDefinition.xr.subsections.general.settings.clientSideEnableXR.path, definition: settingsUIDefinition.xr.subsections.general.settings.clientSideEnableXR },
            { key: 'displayMode', path: settingsUIDefinition.xr.subsections.general.settings.displayMode.path, definition: settingsUIDefinition.xr.subsections.general.settings.displayMode },
            { key: 'quality', path: settingsUIDefinition.xr.subsections.general.settings.quality.path, definition: settingsUIDefinition.xr.subsections.general.settings.quality },
          ]
        },
        {
          title: 'Interaction',
          description: 'Hand tracking and controls',
          items: [
            { key: 'enableHandTracking', path: 'xr.handFeatures.enableHandTracking', definition: settingsUIDefinition.xr.subsections.handFeatures.settings.enableHandTracking },
            { key: 'enableHaptics', path: 'xr.handFeatures.enableHaptics', definition: settingsUIDefinition.xr.subsections.handFeatures.settings.enableHaptics },
            { key: 'interactionRadius', path: 'xr.handFeatures.interactionRadius', definition: settingsUIDefinition.xr.subsections.handFeatures.settings.interactionRadius },
            { key: 'movementAxesHorizontal', path: 'xr.handFeatures.movementAxesHorizontal', definition: settingsUIDefinition.xr.subsections.handFeatures.settings.movementAxesHorizontal },
            { key: 'movementAxesVertical', path: 'xr.handFeatures.movementAxesVertical', definition: settingsUIDefinition.xr.subsections.handFeatures.settings.movementAxesVertical },
          ],
          isPowerUser: true
        },
        {
          title: 'Environment Understanding',
          description: 'Settings for AR environment features',
          items: [
            { key: 'enableLightEstimation', path: 'xr.environmentUnderstanding.enableLightEstimation', definition: settingsUIDefinition.xr.subsections.environmentUnderstanding.settings.enableLightEstimation },
            { key: 'enablePlaneDetection', path: 'xr.environmentUnderstanding.enablePlaneDetection', definition: settingsUIDefinition.xr.subsections.environmentUnderstanding.settings.enablePlaneDetection },
            { key: 'enableSceneUnderstanding', path: 'xr.environmentUnderstanding.enableSceneUnderstanding', definition: settingsUIDefinition.xr.subsections.environmentUnderstanding.settings.enableSceneUnderstanding },
            { key: 'planeColor', path: 'xr.environmentUnderstanding.planeColor', definition: settingsUIDefinition.xr.subsections.environmentUnderstanding.settings.planeColor },
            { key: 'planeOpacity', path: 'xr.environmentUnderstanding.planeOpacity', definition: settingsUIDefinition.xr.subsections.environmentUnderstanding.settings.planeOpacity },
            { key: 'planeDetectionDistance', path: 'xr.environmentUnderstanding.planeDetectionDistance', definition: settingsUIDefinition.xr.subsections.environmentUnderstanding.settings.planeDetectionDistance },
            { key: 'showPlaneOverlay', path: 'xr.environmentUnderstanding.showPlaneOverlay', definition: settingsUIDefinition.xr.subsections.environmentUnderstanding.settings.showPlaneOverlay },
            { key: 'snapToFloor', path: 'xr.environmentUnderstanding.snapToFloor', definition: settingsUIDefinition.xr.subsections.environmentUnderstanding.settings.snapToFloor },
          ],
          isPowerUser: true
        },
        {
          title: 'Passthrough',
          description: 'Control passthrough portal settings',
          items: [
            { key: 'enablePassthroughPortal', path: 'xr.passthrough.enablePassthroughPortal', definition: settingsUIDefinition.xr.subsections.passthrough.settings.enablePassthroughPortal },
            { key: 'passthroughOpacity', path: 'xr.passthrough.passthroughOpacity', definition: settingsUIDefinition.xr.subsections.passthrough.settings.passthroughOpacity },
            { key: 'passthroughBrightness', path: 'xr.passthrough.passthroughBrightness', definition: settingsUIDefinition.xr.subsections.passthrough.settings.passthroughBrightness },
            { key: 'passthroughContrast', path: 'xr.passthrough.passthroughContrast', definition: settingsUIDefinition.xr.subsections.passthrough.settings.passthroughContrast },
            { key: 'portalSize', path: 'xr.passthrough.portalSize', definition: settingsUIDefinition.xr.subsections.passthrough.settings.portalSize },
            { key: 'portalEdgeColor', path: 'xr.passthrough.portalEdgeColor', definition: settingsUIDefinition.xr.subsections.passthrough.settings.portalEdgeColor },
            { key: 'portalEdgeWidth', path: 'xr.passthrough.portalEdgeWidth', definition: settingsUIDefinition.xr.subsections.passthrough.settings.portalEdgeWidth },
          ],
          isPowerUser: true
        }
      ]
    },
    auth: {
      label: 'Authentication',
      icon: <User className="h-4 w-4" />,
      groups: [
        {
          title: 'Nostr Authentication',
          description: 'Authenticate to access advanced features.',
          items: [], // The NostrAuthSection component will be rendered directly.
        },
      ],
    },
    advanced: {
      label: 'Advanced',
      icon: <Settings className="h-4 w-4" />,
      isPowerUser: true,
      groups: [
        {
          title: 'Debug Options',
          description: 'Developer tools',
          items: [
            { key: 'debugMode', path: settingsUIDefinition.system.subsections.debug.settings.enabled.path, definition: settingsUIDefinition.system.subsections.debug.settings.enabled },
            { key: 'logLevel', path: 'system.debug.logLevel', definition: settingsUIDefinition.system.subsections.debug.settings.logLevel },
          ]
        },
        {
          title: 'AI Services',
          description: 'API configuration',
          items: [
            { key: 'ragflowApiKey', path: settingsUIDefinition.ai.subsections.ragflow.settings.apiKey.path, definition: settingsUIDefinition.ai.subsections.ragflow.settings.apiKey },
            { key: 'perplexityKey', path: settingsUIDefinition.ai.subsections.perplexity.settings.apiKey.path, definition: settingsUIDefinition.ai.subsections.perplexity.settings.apiKey },
            { key: 'openaiKey', path: settingsUIDefinition.ai.subsections.openai.settings.apiKey.path, definition: settingsUIDefinition.ai.subsections.openai.settings.apiKey },
          ]
        }
      ]
    }
  }), []);

  // Filter settings based on search query
  const filterSettings = (groups: SettingGroup[], query: string): SettingGroup[] => {
    if (!query.trim()) return groups;

    const lowerQuery = query.toLowerCase();
    return groups
      .map(group => {
        const filteredItems = group.items.filter(item => {
          const matchesKey = item.key.toLowerCase().includes(lowerQuery);
          const matchesLabel = item.definition?.label?.toLowerCase().includes(lowerQuery);
          const matchesDescription = item.definition?.description?.toLowerCase().includes(lowerQuery);
          const matchesGroup = group.title.toLowerCase().includes(lowerQuery);
          const matchesGroupDesc = group.description?.toLowerCase().includes(lowerQuery);

          return matchesKey || matchesLabel || matchesDescription || matchesGroup || matchesGroupDesc;
        });

        if (filteredItems.length > 0) {
          return { ...group, items: filteredItems };
        }
        return null;
      })
      .filter(group => group !== null) as SettingGroup[];
  };

  // Auto-expand groups when searching
  React.useEffect(() => {
    if (searchQuery.trim()) {
      // Expand all groups that have matching items
      const groupsToExpand = new Set<string>();
      Object.values(settingsStructure).forEach(section => {
        const filtered = filterSettings(section.groups, searchQuery);
        filtered.forEach(group => {
          groupsToExpand.add(group.title);
        });
      });
      setExpandedGroups(groupsToExpand);
    }
  }, [searchQuery, settingsStructure]);

  // Register keyboard shortcuts
  useKeyboardShortcuts({
    'settings-search': {
      key: '/',
      ctrl: true,
      description: 'Focus search in settings',
      category: 'Settings',
      handler: () => {
        const searchInput = document.querySelector('input[placeholder="Search settings..."]') as HTMLInputElement;
        searchInput?.focus();
      }
    },
    'settings-shortcuts': {
      key: '?',
      shift: true,
      description: 'Show keyboard shortcuts',
      category: 'General',
      handler: () => setShowShortcuts(true)
    },
    'settings-clear-search': {
      key: 'Escape',
      description: 'Clear search',
      category: 'Settings',
      handler: () => setSearchQuery(''),
      enabled: !!searchQuery
    },
    'settings-toggle-poweruser': {
      key: 'p',
      ctrl: true,
      shift: true,
      description: 'Toggle power user mode',
      category: 'Settings',
      handler: () => {
        toast({
          title: 'Authentication required',
          description: 'Please authenticate with Nostr to enable power user features',
        });
      }
    }
  });

  // Simulate initial loading
  React.useEffect(() => {
    const timer = setTimeout(() => {
      setIsInitializing(false);
    }, 800);
    return () => clearTimeout(timer);
  }, []);

  const toggleGroup = (groupTitle: string) => {
    setExpandedGroups(prev => {
      const next = new Set(prev);
      if (next.has(groupTitle)) {
        next.delete(groupTitle);
      } else {
        next.add(groupTitle);
      }
      return next;
    });
  };

  const handleSettingChange = async (path: string, value: any) => {
    // Add loading state for this setting
    setLoadingSettings(prev => new Set(prev).add(path));

    try {
      // Save the setting - this should handle persistence automatically
      setSetting(path, value);

      // Show success toast
      toast({
        title: 'Setting saved',
        description: `Successfully updated ${path.split('.').pop()}`,
      });

      // Also show inline notification
      setSavedNotification(path);
      setTimeout(() => setSavedNotification(null), 2000);
    } catch (error) {
      handleError(error, {
        title: 'Failed to save setting',
        actionLabel: 'Retry',
        onAction: () => handleSettingChange(path, value)
      });
    } finally {
      // Remove loading state
      setLoadingSettings(prev => {
        const next = new Set(prev);
        next.delete(path);
        return next;
      });
    }
  };

  const renderSettingGroup = (group: SettingGroup, groupIndex: number) => {
    if (group.isPowerUser && !isPowerUser) return null;

    const isExpanded = expandedGroups.has(group.title);

    return (
      <Card key={group.title} className="mb-3 overflow-hidden">
        <CardHeader
          className="cursor-pointer py-3 px-4 hover:bg-muted/50 transition-colors"
          onClick={() => toggleGroup(group.title)}
        >
          <div className="flex items-center justify-between">
            <div className="flex-1">
              <CardTitle className="text-sm font-medium flex items-center gap-2">
                {group.title}
                {group.isPowerUser && (
                  <span className="text-xs px-1.5 py-0.5 bg-primary/10 text-primary rounded">
                    Pro
                  </span>
                )}
              </CardTitle>
              {group.description && (
                <CardDescription className="text-xs mt-1">
                  {group.description}
                </CardDescription>
              )}
            </div>
            <ChevronDown
              className={cn(
                "h-4 w-4 transition-transform duration-200",
                isExpanded ? "" : "-rotate-90"
              )}
            />
          </div>
        </CardHeader>

        {isExpanded && (
          <CardContent className="pt-0 pb-3 px-4 space-y-3">
            <VirtualizedSettingsGroup
              title={group.title}
              items={group.items}
              isExpanded={isExpanded}
              onToggle={() => toggleGroup(group.title)}
              isPowerUser={isPowerUser}
              loadingSettings={loadingSettings}
              savedNotification={savedNotification}
              onSettingChange={handleSettingChange}
              groupIndex={groupIndex}
            />
          </CardContent>
        )}
      </Card>
    );
  };

  const renderTabContent = (tabKey: string) => {
    const tab = settingsStructure[tabKey];
    if (!tab) return null;

    if (tab.isPowerUser && !isPowerUser) {
      return (
        <div className="flex flex-col items-center justify-center h-64 text-center p-6">
          <Settings className="h-12 w-12 text-muted-foreground mb-4" />
          <h3 className="text-lg font-medium mb-2">Power User Features</h3>
          <p className="text-sm text-muted-foreground max-w-sm">
            Authenticate with Nostr to unlock advanced settings and features.
          </p>
        </div>
      );
    }

    // Special handling for the Auth tab
    if (tabKey === 'auth') {
      return <NostrAuthSection />;
    }

    const filteredGroups = filterSettings(tab.groups, searchQuery);

    if (searchQuery && filteredGroups.length === 0) {
      return (
        <div className="flex flex-col items-center justify-center h-64 text-center p-6">
          <Search className="h-12 w-12 text-muted-foreground mb-4" />
          <h3 className="text-lg font-medium mb-2">No results found</h3>
          <p className="text-sm text-muted-foreground max-w-sm">
            Try searching with different keywords or browse categories.
          </p>
        </div>
      );
    }

    return (
      <div className="flex-1 min-h-0 space-y-3">
        {filteredGroups.map((group, index) => (
          <React.Fragment key={group.title}>
            {renderSettingGroup(group, index)}
          </React.Fragment>
        ))}
      </div>
    );
  };

  return (
    // Dynamically set background and text color from settings
    <div className="w-full h-full flex flex-col min-h-0 bg-background text-foreground">
      <div className="border-b border-border">
        <div className="px-4 py-3 flex items-center justify-between">
          <div>
            <h2 className="text-lg font-semibold">Settings & Controls</h2>
            <p className="text-sm text-muted-foreground">
              Customize your visualization and experience
            </p>
          </div>
          <div className="flex items-center gap-2">
            <UndoRedoControls showHistory />
            <div className="w-px h-6 bg-border" />
            <Button
              variant="ghost"
              size="icon"
              onClick={toggleLowerRightPaneDock}
              title={isLowerRightPaneDocked ? "Expand lower panels" : "Collapse lower panels"}
            >
              {isLowerRightPaneDocked ? <ChevronDown className="h-4 w-4" /> : <ChevronUp className="h-4 w-4" />}
            </Button>
          </div>
        </div>
        <div className="px-4 pb-3">
          <SearchInput
            value={searchQuery}
            onChange={setSearchQuery}
            placeholder="Search settings..."
            className="w-full"
            onKeyDown={(e) => {
              if (e.key === 'Escape') {
                setSearchQuery('');
              }
            }}
          />
          <div className="mt-2 flex items-center justify-between text-xs text-muted-foreground">
            <span>Press <kbd className="px-1 py-0.5 bg-muted rounded text-xs">Ctrl+/</kbd> to search</span>
            <button
              onClick={() => setShowShortcuts(true)}
              className="flex items-center gap-1 hover:text-foreground transition-colors"
            >
              <Keyboard className="h-3 w-3" />
              <span>View shortcuts</span>
            </button>
          </div>
        </div>
      </div>

      <div className="flex-1 overflow-auto">
        <Tabs defaultValue="appearance" className="h-full">
          <TabsList className="px-4 bg-muted/30 w-full justify-start">
            {Object.entries(settingsStructure).map(([key, section]) => (
              <TabsTrigger key={key} value={key} className="flex items-center gap-2">
                {section.icon}
                {section.label}
              </TabsTrigger>
            ))}
          </TabsList>
          {Object.entries(settingsStructure).map(([key, section]) => (
            <TabsContent key={key} value={key} className="px-4 py-3">
              {renderTabContent(key)}
            </TabsContent>
          ))}
        </Tabs>
      </div>

      {/* Status bar */}
      <div className="px-4 py-2 border-t border-border bg-muted/30 flex items-center justify-between text-xs">
        <div className="flex items-center gap-2">
          <Info className="h-3 w-3" />
          <span>Changes save automatically</span>
        </div>
        {isPowerUser && (
          <div className="flex items-center gap-1 text-primary">
            <Settings className="h-3 w-3" />
            <span>Power User</span>
          </div>
        )}
      </div>

      {/* Keyboard Shortcuts Modal */}
      <KeyboardShortcutsModal
        isOpen={showShortcuts}
        onClose={() => setShowShortcuts(false)}
      />
    </div>
  );
}