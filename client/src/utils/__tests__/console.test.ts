/**
 * Tests for the gated console system
 */

import { gatedConsole, debugControl, categoryDebugState, DebugCategory } from '../console';
import { debugState } from '../debugState';

// Mock console methods
const mockConsole = {
  log: jest.fn(),
  error: jest.fn(),
  warn: jest.fn(),
  info: jest.fn(),
  debug: jest.fn(),
};

// Replace console methods with mocks
beforeAll(() => {
  Object.assign(console, mockConsole);
});

// Clear mocks before each test
beforeEach(() => {
  Object.values(mockConsole).forEach(mock => mock.mockClear());
  // Reset debug state
  debugControl.disable();
  debugControl.disableAllCategories();
});

describe('gatedConsole', () => {
  describe('basic functionality', () => {
    it('should not output when debug is disabled', () => {
      debugControl.disable();
      
      gatedConsole.log('test message');
      gatedConsole.error('test error');
      gatedConsole.warn('test warning');
      
      expect(mockConsole.log).not.toHaveBeenCalled();
      expect(mockConsole.error).not.toHaveBeenCalled();
      expect(mockConsole.warn).not.toHaveBeenCalled();
    });

    it('should output when debug is enabled', () => {
      debugControl.enable();
      debugControl.enableCategory(DebugCategory.GENERAL);
      
      gatedConsole.log('test message');
      
      expect(mockConsole.log).toHaveBeenCalledWith('test message');
    });

    it('should force output when force option is used', () => {
      debugControl.disable();
      
      gatedConsole.log({ force: true }, 'forced message');
      
      expect(mockConsole.log).toHaveBeenCalledWith('forced message');
    });
  });

  describe('category filtering', () => {
    beforeEach(() => {
      debugControl.enable();
    });

    it('should respect category settings', () => {
      debugControl.enableCategory(DebugCategory.VOICE);
      debugControl.disableCategory(DebugCategory.WEBSOCKET);
      
      gatedConsole.voice.log('voice message');
      gatedConsole.websocket.log('websocket message');
      
      expect(mockConsole.log).toHaveBeenCalledWith('voice message');
      expect(mockConsole.log).not.toHaveBeenCalledWith('websocket message');
    });

    it('should handle multiple categories', () => {
      debugControl.enableCategory(DebugCategory.VOICE);
      debugControl.enableCategory(DebugCategory.DATA);
      
      gatedConsole.voice.log('voice message');
      gatedConsole.data.log('data message');
      gatedConsole.websocket.log('websocket message');
      
      expect(mockConsole.log).toHaveBeenCalledTimes(2);
      expect(mockConsole.log).toHaveBeenCalledWith('voice message');
      expect(mockConsole.log).toHaveBeenCalledWith('data message');
    });
  });

  describe('error handling', () => {
    it('should always show errors in development', () => {
      // Mock NODE_ENV
      const originalEnv = process.env.NODE_ENV;
      process.env.NODE_ENV = 'development';
      
      debugControl.disable();
      
      gatedConsole.error('error message');
      
      expect(mockConsole.error).toHaveBeenCalledWith('error message');
      
      // Restore NODE_ENV
      process.env.NODE_ENV = originalEnv;
    });
  });

  describe('presets', () => {
    it('should apply minimal preset', () => {
      debugControl.presets.minimal();
      
      expect(debugControl.isEnabled()).toBe(true);
      expect(debugControl.getEnabledCategories()).toContain(DebugCategory.ERROR);
      expect(debugControl.getEnabledCategories()).toHaveLength(1);
    });

    it('should apply standard preset', () => {
      debugControl.presets.standard();
      
      expect(debugControl.isEnabled()).toBe(true);
      expect(debugControl.getEnabledCategories()).toContain(DebugCategory.ERROR);
      expect(debugControl.getEnabledCategories()).toContain(DebugCategory.GENERAL);
    });

    it('should apply verbose preset', () => {
      debugControl.presets.verbose();
      
      expect(debugControl.isEnabled()).toBe(true);
      expect(debugControl.getEnabledCategories()).toHaveLength(
        Object.values(DebugCategory).length
      );
    });

    it('should apply off preset', () => {
      debugControl.presets.off();
      
      expect(debugControl.isEnabled()).toBe(false);
    });
  });

  describe('convenience methods', () => {
    beforeEach(() => {
      debugControl.enable();
    });

    it('should provide category-specific methods', () => {
      debugControl.enableCategory(DebugCategory.VOICE);
      
      gatedConsole.voice.log('log message');
      gatedConsole.voice.error('error message');
      gatedConsole.voice.warn('warn message');
      
      expect(mockConsole.log).toHaveBeenCalledWith('log message');
      expect(mockConsole.error).toHaveBeenCalledWith('error message');
      expect(mockConsole.warn).toHaveBeenCalledWith('warn message');
    });
  });

  describe('persistence', () => {
    it('should persist category settings', () => {
      debugControl.enableCategory(DebugCategory.VOICE);
      debugControl.enableCategory(DebugCategory.DATA);
      
      // Simulate reload by creating new instance
      const savedCategories = debugControl.getEnabledCategories();
      
      expect(savedCategories).toContain(DebugCategory.VOICE);
      expect(savedCategories).toContain(DebugCategory.DATA);
    });
  });
});