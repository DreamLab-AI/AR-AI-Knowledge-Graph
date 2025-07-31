import { describe, it, expect } from 'vitest';
import { isViewportSetting, getViewportPaths } from '../../features/settings/config/viewportSettings';

describe('Viewport Settings Configuration', () => {
  describe('isViewportSetting', () => {
    it('should identify visualization node settings as viewport settings', () => {
      expect(isViewportSetting('visualisation.nodes.baseColor')).toBe(true);
      expect(isViewportSetting('visualisation.nodes.metalness')).toBe(true);
      expect(isViewportSetting('visualisation.nodes.opacity')).toBe(true);
    });

    it('should identify visualization edge settings as viewport settings', () => {
      expect(isViewportSetting('visualisation.edges.color')).toBe(true);
      expect(isViewportSetting('visualisation.edges.opacity')).toBe(true);
      expect(isViewportSetting('visualisation.edges.enableFlowEffect')).toBe(true);
    });

    it('should identify physics settings as viewport settings', () => {
      expect(isViewportSetting('visualisation.physics.enabled')).toBe(true);
      expect(isViewportSetting('visualisation.physics.damping')).toBe(true);
    });

    it('should identify rendering settings as viewport settings', () => {
      expect(isViewportSetting('visualisation.rendering.backgroundColor')).toBe(true);
      expect(isViewportSetting('visualisation.rendering.ambientLightIntensity')).toBe(true);
    });

    it('should identify bloom settings as viewport settings', () => {
      expect(isViewportSetting('visualisation.bloom.enabled')).toBe(true);
      expect(isViewportSetting('visualisation.bloom.strength')).toBe(true);
    });

    it('should identify graph-specific settings as viewport settings', () => {
      expect(isViewportSetting('visualisation.graphs.logseq.nodes.baseColor')).toBe(true);
      expect(isViewportSetting('visualisation.graphs.test.edges.opacity')).toBe(true);
      expect(isViewportSetting('visualisation.graphs.any-name.physics.enabled')).toBe(true);
    });

    it('should identify XR settings as viewport settings', () => {
      expect(isViewportSetting('xr.mode')).toBe(true);
      expect(isViewportSetting('xr.quality')).toBe(true);
      expect(isViewportSetting('xr.enable_hand_tracking')).toBe(true);
    });

    it('should NOT identify non-viewport settings', () => {
      expect(isViewportSetting('system.websocket.reconnectAttempts')).toBe(false);
      expect(isViewportSetting('auth.enabled')).toBe(false);
      expect(isViewportSetting('ragflow.apiKey')).toBe(false);
      expect(isViewportSetting('system.persistSettings')).toBe(false);
    });

    it('should handle edge cases', () => {
      expect(isViewportSetting('')).toBe(false);
      expect(isViewportSetting('visualisation')).toBe(false); // Too broad
      expect(isViewportSetting('xr')).toBe(false); // Too broad
    });
  });

  describe('getViewportPaths', () => {
    it('should filter viewport paths from mixed paths', () => {
      const paths = [
        'visualisation.nodes.baseColor',
        'system.websocket.reconnectAttempts',
        'visualisation.edges.opacity',
        'auth.enabled',
        'visualisation.bloom.strength',
        'xr.quality'
      ];

      const viewportPaths = getViewportPaths(paths);
      
      expect(viewportPaths).toHaveLength(4);
      expect(viewportPaths).toContain('visualisation.nodes.baseColor');
      expect(viewportPaths).toContain('visualisation.edges.opacity');
      expect(viewportPaths).toContain('visualisation.bloom.strength');
      expect(viewportPaths).toContain('xr.quality');
      expect(viewportPaths).not.toContain('system.websocket.reconnectAttempts');
      expect(viewportPaths).not.toContain('auth.enabled');
    });

    it('should return empty array for no viewport paths', () => {
      const paths = [
        'system.websocket.reconnectAttempts',
        'auth.enabled',
        'ragflow.apiKey'
      ];

      const viewportPaths = getViewportPaths(paths);
      expect(viewportPaths).toHaveLength(0);
    });
  });
});