import { createLogger } from './logger';

const logger = createLogger('DebugState');

class DebugState {
  private debugEnabled: boolean = false;
  private dataDebugEnabled: boolean = false;
  private performanceDebugEnabled: boolean = false;

  constructor() {
    // Initialize from localStorage if available
    this.loadFromStorage();
  }

  private loadFromStorage(): void {
    if (typeof window !== 'undefined') {
      try {
        this.debugEnabled = localStorage.getItem('debug.enabled') === 'true';
        this.dataDebugEnabled = localStorage.getItem('debug.data') === 'true';
        this.performanceDebugEnabled = localStorage.getItem('debug.performance') === 'true';
      } catch (e) {
        logger.warn('Failed to load debug state from localStorage');
      }
    }
  }

  private saveToStorage(): void {
    if (typeof window !== 'undefined') {
      try {
        localStorage.setItem('debug.enabled', this.debugEnabled.toString());
        localStorage.setItem('debug.data', this.dataDebugEnabled.toString());
        localStorage.setItem('debug.performance', this.performanceDebugEnabled.toString());
      } catch (e) {
        logger.warn('Failed to save debug state to localStorage');
      }
    }
  }

  public isEnabled(): boolean {
    return this.debugEnabled;
  }

  public enableDebug(enable: boolean = true): void {
    this.debugEnabled = enable;
    this.saveToStorage();
    logger.info(`Debug mode ${enable ? 'enabled' : 'disabled'}`);
  }

  public isDataDebugEnabled(): boolean {
    return this.debugEnabled && this.dataDebugEnabled;
  }

  public enableDataDebug(enable: boolean = true): void {
    this.dataDebugEnabled = enable;
    this.saveToStorage();
    logger.info(`Data debug mode ${enable ? 'enabled' : 'disabled'}`);
  }

  public isPerformanceDebugEnabled(): boolean {
    return this.debugEnabled && this.performanceDebugEnabled;
  }

  public enablePerformanceDebug(enable: boolean = true): void {
    this.performanceDebugEnabled = enable;
    this.saveToStorage();
    logger.info(`Performance debug mode ${enable ? 'enabled' : 'disabled'}`);
  }
}

// Create a singleton instance
export const debugState = new DebugState();