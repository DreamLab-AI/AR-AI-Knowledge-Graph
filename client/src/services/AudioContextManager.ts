/**
 * AudioContextManager - Singleton manager for Web Audio API context
 * Handles shared AudioContext instance and state management
 */

export class AudioContextManager {
  private static instance: AudioContextManager;
  private audioContext: AudioContext | null = null;
  private initialized: boolean = false;

  private constructor() {}

  static getInstance(): AudioContextManager {
    if (!AudioContextManager.instance) {
      AudioContextManager.instance = new AudioContextManager();
    }
    return AudioContextManager.instance;
  }

  /**
   * Get or create the AudioContext
   */
  getContext(): AudioContext {
    if (!this.audioContext) {
      this.audioContext = new (window.AudioContext || (window as any).webkitAudioContext)();
      this.initialized = true;
    }
    return this.audioContext;
  }

  /**
   * Resume AudioContext if suspended (required for some browsers)
   */
  async resume(): Promise<void> {
    const ctx = this.getContext();
    if (ctx.state === 'suspended') {
      await ctx.resume();
    }
  }

  /**
   * Suspend AudioContext to save resources
   */
  async suspend(): Promise<void> {
    if (this.audioContext && this.audioContext.state === 'running') {
      await this.audioContext.suspend();
    }
  }

  /**
   * Close and cleanup AudioContext
   */
  async close(): Promise<void> {
    if (this.audioContext) {
      await this.audioContext.close();
      this.audioContext = null;
      this.initialized = false;
    }
  }

  /**
   * Get current state of AudioContext
   */
  getState(): AudioContextState | null {
    return this.audioContext ? this.audioContext.state : null;
  }

  /**
   * Check if AudioContext is initialized
   */
  isInitialized(): boolean {
    return this.initialized;
  }
}