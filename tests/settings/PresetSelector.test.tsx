/**
 * Quality Preset Selector Tests
 *
 * Tests for the PresetSelector component and quality presets system
 */

import React from 'react';
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { PresetSelector, PresetSelectorCompact } from '../../client/src/features/settings/components/PresetSelector';
import { QUALITY_PRESETS, getPresetById, getRecommendedPreset } from '../../client/src/features/settings/presets/qualityPresets';
import { useSettingsStore } from '../../client/src/store/settingsStore';

// Mock the settings store
vi.mock('../../client/src/store/settingsStore');

describe('Quality Presets System', () => {

  describe('Preset Definitions', () => {
    it('should have all 4 standard presets', () => {
      expect(QUALITY_PRESETS).toHaveLength(4);
      expect(QUALITY_PRESETS.map(p => p.id)).toEqual(['low', 'medium', 'high', 'ultra']);
    });

    it('should have valid preset structure', () => {
      QUALITY_PRESETS.forEach(preset => {
        expect(preset).toHaveProperty('id');
        expect(preset).toHaveProperty('name');
        expect(preset).toHaveProperty('description');
        expect(preset).toHaveProperty('icon');
        expect(preset).toHaveProperty('category');
        expect(preset).toHaveProperty('settings');
        expect(typeof preset.settings).toBe('object');
      });
    });

    it('should have non-empty settings for each preset', () => {
      QUALITY_PRESETS.forEach(preset => {
        expect(Object.keys(preset.settings).length).toBeGreaterThan(0);
        expect(Object.keys(preset.settings).length).toBeGreaterThanOrEqual(45);
      });
    });

    it('should have physics settings in all presets', () => {
      QUALITY_PRESETS.forEach(preset => {
        expect(preset.settings).toHaveProperty('visualisation.graphs.knowledge.physics.iterations');
        expect(preset.settings).toHaveProperty('visualisation.graphs.knowledge.physics.warmupIterations');
      });
    });

    it('should have performance settings in all presets', () => {
      QUALITY_PRESETS.forEach(preset => {
        expect(preset.settings).toHaveProperty('performance.targetFPS');
        expect(preset.settings).toHaveProperty('performance.gpuMemoryLimit');
      });
    });

    it('should have rendering settings in all presets', () => {
      QUALITY_PRESETS.forEach(preset => {
        expect(preset.settings).toHaveProperty('visualisation.rendering.enableAntialiasing');
        expect(preset.settings).toHaveProperty('visualisation.rendering.textureQuality');
      });
    });

    it('should have XR settings in all presets', () => {
      QUALITY_PRESETS.forEach(preset => {
        expect(preset.settings).toHaveProperty('xr.renderScale');
        expect(preset.settings).toHaveProperty('xr.targetFrameRate');
      });
    });
  });

  describe('Preset Values', () => {
    it('should have increasing physics iterations from Low to Ultra', () => {
      const iterations = QUALITY_PRESETS.map(
        p => p.settings['visualisation.graphs.knowledge.physics.iterations']
      );
      expect(iterations).toEqual([100, 300, 500, 1000]);
    });

    it('should have appropriate FPS targets', () => {
      const fps = QUALITY_PRESETS.map(p => p.settings['performance.targetFPS']);
      expect(fps[0]).toBe(30);  // Low
      expect(fps[1]).toBe(60);  // Medium
      expect(fps[2]).toBe(60);  // High
      expect(fps[3]).toBe(120); // Ultra
    });

    it('should have increasing GPU memory limits', () => {
      const memory = QUALITY_PRESETS.map(p => p.settings['performance.gpuMemoryLimit']);
      expect(memory).toEqual([1024, 2048, 4096, 8192]);
    });

    it('should have increasing XR render scales', () => {
      const scales = QUALITY_PRESETS.map(p => p.settings['xr.renderScale']);
      expect(scales).toEqual([0.7, 1.0, 1.2, 1.5]);
    });

    it('should disable effects in Low preset', () => {
      const low = QUALITY_PRESETS[0];
      expect(low.settings['visualisation.rendering.enableShadows']).toBe(false);
      expect(low.settings['visualisation.rendering.enableAmbientOcclusion']).toBe(false);
      expect(low.settings['visualisation.glow.enabled']).toBe(false);
    });

    it('should enable all effects in Ultra preset', () => {
      const ultra = QUALITY_PRESETS[3];
      expect(ultra.settings['visualisation.rendering.enableShadows']).toBe(true);
      expect(ultra.settings['visualisation.rendering.enableAmbientOcclusion']).toBe(true);
      expect(ultra.settings['visualisation.glow.enabled']).toBe(true);
    });
  });

  describe('Preset Utilities', () => {
    it('should get preset by ID', () => {
      const preset = getPresetById('high');
      expect(preset).toBeDefined();
      expect(preset?.id).toBe('high');
    });

    it('should return undefined for invalid ID', () => {
      const preset = getPresetById('invalid');
      expect(preset).toBeUndefined();
    });

    it('should recommend Low preset for low-end systems', () => {
      const recommended = getRecommendedPreset({ ram: 4, vram: 1, gpu: 'Intel UHD' });
      expect(recommended.id).toBe('low');
    });

    it('should recommend Medium preset for mid-range systems', () => {
      const recommended = getRecommendedPreset({ ram: 8, vram: 2, gpu: 'GTX 1060' });
      expect(recommended.id).toBe('medium');
    });

    it('should recommend High preset for modern systems', () => {
      const recommended = getRecommendedPreset({ ram: 16, vram: 4, gpu: 'RTX 2060' });
      expect(recommended.id).toBe('high');
    });

    it('should recommend Ultra preset for high-end systems', () => {
      const recommended = getRecommendedPreset({ ram: 32, vram: 8, gpu: 'RTX 3080' });
      expect(recommended.id).toBe('ultra');
    });
  });

  describe('PresetSelector Component', () => {
    beforeEach(() => {
      (useSettingsStore as vi.Mock).mockReturnValue({
        settings: {},
        updateSettings: vi.fn(),
      });
    });

    it('should render all preset buttons', () => {
      render(<PresetSelector />);

      expect(screen.getByText(/Low \(Battery Saver\)/i)).toBeInTheDocument();
      expect(screen.getByText(/Medium \(Balanced\)/i)).toBeInTheDocument();
      expect(screen.getByText(/High \(Recommended\)/i)).toBeInTheDocument();
      expect(screen.getByText(/Ultra \(High-End\)/i)).toBeInTheDocument();
    });

    it('should render descriptions when showDescription is true', () => {
      render(<PresetSelector showDescription={true} />);

      expect(screen.getByText(/Optimized for battery life/i)).toBeInTheDocument();
      expect(screen.getByText(/Balanced performance/i)).toBeInTheDocument();
    });

    it('should apply preset when clicked', async () => {
      const mockUpdateSettings = vi.fn();
      (useSettingsStore as vi.Mock).mockReturnValue({
        settings: {},
        updateSettings: mockUpdateSettings,
      });

      render(<PresetSelector />);

      const highButton = screen.getByText(/Apply Preset/i, {
        selector: 'button:has-text("High")'
      });
      fireEvent.click(highButton);

      await waitFor(() => {
        expect(mockUpdateSettings).toHaveBeenCalled();
      });
    });

    it('should show loading state when applying', async () => {
      const mockUpdateSettings = vi.fn(() => new Promise(resolve => setTimeout(resolve, 100)));
      (useSettingsStore as vi.Mock).mockReturnValue({
        settings: {},
        updateSettings: mockUpdateSettings,
      });

      render(<PresetSelector />);

      const button = screen.getAllByText(/Apply Preset/i)[0];
      fireEvent.click(button);

      expect(screen.getByText(/Applying/i)).toBeInTheDocument();
    });

    it('should persist selected preset to localStorage', async () => {
      const mockUpdateSettings = vi.fn();
      (useSettingsStore as vi.Mock).mockReturnValue({
        settings: {},
        updateSettings: mockUpdateSettings,
      });

      const setItemSpy = vi.spyOn(Storage.prototype, 'setItem');

      render(<PresetSelector />);

      const button = screen.getAllByText(/Apply Preset/i)[0];
      fireEvent.click(button);

      await waitFor(() => {
        expect(setItemSpy).toHaveBeenCalledWith('quality-preset', expect.any(String));
      });
    });
  });

  describe('PresetSelectorCompact Component', () => {
    beforeEach(() => {
      (useSettingsStore as vi.Mock).mockReturnValue({
        settings: {},
        updateSettings: vi.fn(),
      });
    });

    it('should render in compact mode', () => {
      render(<PresetSelectorCompact />);

      // Should show abbreviated names
      expect(screen.getByText('Low')).toBeInTheDocument();
      expect(screen.getByText('Medium')).toBeInTheDocument();
      expect(screen.getByText('High')).toBeInTheDocument();
      expect(screen.getByText('Ultra')).toBeInTheDocument();
    });

    it('should render icons', () => {
      const { container } = render(<PresetSelectorCompact />);

      // Check for icon elements
      const icons = container.querySelectorAll('svg');
      expect(icons.length).toBeGreaterThanOrEqual(4);
    });

    it('should apply preset when clicked', async () => {
      const mockUpdateSettings = vi.fn();
      (useSettingsStore as vi.Mock).mockReturnValue({
        settings: {},
        updateSettings: mockUpdateSettings,
      });

      render(<PresetSelectorCompact />);

      const mediumButton = screen.getByText('Medium');
      fireEvent.click(mediumButton);

      await waitFor(() => {
        expect(mockUpdateSettings).toHaveBeenCalled();
      });
    });

    it('should show active state', () => {
      (useSettingsStore as vi.Mock).mockReturnValue({
        settings: {},
        updateSettings: vi.fn(),
      });

      const { container } = render(<PresetSelectorCompact />);

      const highButton = screen.getByText('High');
      fireEvent.click(highButton);

      // Should have active styling
      expect(highButton.parentElement).toHaveClass('ring-primary');
    });
  });

  describe('Integration Tests', () => {
    it('should apply all settings from preset', async () => {
      const mockUpdateSettings = vi.fn();
      (useSettingsStore as vi.Mock).mockReturnValue({
        settings: {},
        updateSettings: mockUpdateSettings,
      });

      render(<PresetSelector />);

      const highButton = screen.getAllByText(/Apply Preset/i)[2]; // High preset
      fireEvent.click(highButton);

      await waitFor(() => {
        expect(mockUpdateSettings).toHaveBeenCalled();
        const callArgs = mockUpdateSettings.mock.calls[0][0];

        // Verify it's the High preset settings
        expect(callArgs).toHaveProperty('visualisation.graphs.knowledge.physics.iterations', 500);
        expect(callArgs).toHaveProperty('performance.targetFPS', 60);
        expect(callArgs).toHaveProperty('xr.renderScale', 1.2);
      });
    });

    it('should handle rapid preset switching', async () => {
      const mockUpdateSettings = vi.fn();
      (useSettingsStore as vi.Mock).mockReturnValue({
        settings: {},
        updateSettings: mockUpdateSettings,
      });

      render(<PresetSelector />);

      // Click multiple presets rapidly
      const buttons = screen.getAllByText(/Apply Preset/i);
      fireEvent.click(buttons[0]); // Low
      fireEvent.click(buttons[1]); // Medium
      fireEvent.click(buttons[2]); // High

      await waitFor(() => {
        // All clicks should be processed
        expect(mockUpdateSettings).toHaveBeenCalledTimes(3);
      });
    });
  });

  describe('System Requirements Display', () => {
    it('should show system requirements when info is clicked', () => {
      render(<PresetSelector />);

      const infoButtons = screen.getAllByTitle(/info/i);
      fireEvent.click(infoButtons[0]);

      expect(screen.getByText(/RAM: 4GB/i)).toBeInTheDocument();
      expect(screen.getByText(/VRAM: 1GB/i)).toBeInTheDocument();
    });

    it('should have system requirements for all presets', () => {
      QUALITY_PRESETS.forEach(preset => {
        expect(preset.systemRequirements).toBeDefined();
        expect(preset.systemRequirements?.minRAM).toBeGreaterThan(0);
        expect(preset.systemRequirements?.minVRAM).toBeGreaterThan(0);
      });
    });
  });

  describe('Error Handling', () => {
    it('should handle updateSettings failure gracefully', async () => {
      const mockUpdateSettings = vi.fn().mockRejectedValue(new Error('Update failed'));
      (useSettingsStore as vi.Mock).mockReturnValue({
        settings: {},
        updateSettings: mockUpdateSettings,
      });

      const consoleError = vi.spyOn(console, 'error').mockImplementation();

      render(<PresetSelector />);

      const button = screen.getAllByText(/Apply Preset/i)[0];
      fireEvent.click(button);

      await waitFor(() => {
        expect(consoleError).toHaveBeenCalledWith(
          'Failed to apply preset:',
          expect.any(Error)
        );
      });

      consoleError.mockRestore();
    });

    it('should handle missing preset gracefully', () => {
      const preset = getPresetById('non-existent');
      expect(preset).toBeUndefined();
    });
  });
});
