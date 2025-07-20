/**
 * AudioOutputService - Manages audio playback with queue management and Web Audio API
 * Handles TTS audio output with proper buffering and playback control
 */

import { AudioContextManager } from './AudioContextManager';

export interface AudioQueueItem {
  id: string;
  buffer: ArrayBuffer;
  timestamp: number;
  metadata?: any;
}

export type AudioOutputState = 'idle' | 'playing' | 'paused' | 'buffering';

export class AudioOutputService {
  private static instance: AudioOutputService;
  private audioContext: AudioContext;
  private playbackQueue: AudioQueueItem[] = [];
  private currentSource: AudioBufferSourceNode | null = null;
  private gainNode: GainNode;
  private state: AudioOutputState = 'idle';
  private listeners: Map<string, Set<Function>> = new Map();
  private isProcessing = false;
  private volume = 1.0;

  private constructor() {
    this.audioContext = AudioContextManager.getInstance().getContext();
    this.gainNode = this.audioContext.createGain();
    this.gainNode.connect(this.audioContext.destination);
  }

  static getInstance(): AudioOutputService {
    if (!AudioOutputService.instance) {
      AudioOutputService.instance = new AudioOutputService();
    }
    return AudioOutputService.instance;
  }

  /**
   * Add audio data to playback queue
   */
  async queueAudio(audioData: ArrayBuffer, id?: string): Promise<void> {
    const item: AudioQueueItem = {
      id: id || Date.now().toString(),
      buffer: audioData,
      timestamp: Date.now()
    };

    this.playbackQueue.push(item);
    this.emit('audioQueued', item);

    // Start processing if not already
    if (!this.isProcessing) {
      this.processQueue();
    }
  }

  /**
   * Process the audio queue
   */
  private async processQueue() {
    if (this.isProcessing || this.playbackQueue.length === 0) {
      return;
    }

    this.isProcessing = true;
    this.setState('buffering');

    while (this.playbackQueue.length > 0 && this.state !== 'paused') {
      const item = this.playbackQueue.shift()!;
      
      try {
        await this.playAudioBuffer(item);
      } catch (error) {
        console.error('Error playing audio:', error);
        this.emit('error', { item, error });
      }
    }

    this.isProcessing = false;
    this.setState('idle');
  }

  /**
   * Play an audio buffer
   */
  private async playAudioBuffer(item: AudioQueueItem): Promise<void> {
    try {
      // Decode audio data
      const audioBuffer = await this.audioContext.decodeAudioData(item.buffer.slice(0));
      
      // Create source
      this.currentSource = this.audioContext.createBufferSource();
      this.currentSource.buffer = audioBuffer;
      this.currentSource.connect(this.gainNode);

      // Wait for playback to complete
      return new Promise((resolve) => {
        if (!this.currentSource) {
          resolve();
          return;
        }

        this.currentSource.onended = () => {
          this.currentSource = null;
          this.emit('audioEnded', item);
          resolve();
        };

        this.setState('playing');
        this.emit('audioStarted', item);
        this.currentSource.start(0);
      });
    } catch (error) {
      console.error('Failed to decode audio:', error);
      throw error;
    }
  }

  /**
   * Stop current playback and clear queue
   */
  stop() {
    if (this.currentSource) {
      try {
        this.currentSource.stop();
        this.currentSource.disconnect();
      } catch (e) {
        // Ignore errors when stopping
      }
      this.currentSource = null;
    }

    this.playbackQueue = [];
    this.isProcessing = false;
    this.setState('idle');
    this.emit('stopped');
  }

  /**
   * Pause playback
   */
  pause() {
    if (this.state === 'playing') {
      this.setState('paused');
      // Note: Web Audio API doesn't support pause, so we stop and will need to restart
      if (this.currentSource) {
        this.currentSource.stop();
        this.currentSource = null;
      }
      this.emit('paused');
    }
  }

  /**
   * Resume playback
   */
  resume() {
    if (this.state === 'paused') {
      this.setState('idle');
      this.processQueue();
      this.emit('resumed');
    }
  }

  /**
   * Set volume (0-1)
   */
  setVolume(volume: number) {
    this.volume = Math.max(0, Math.min(1, volume));
    this.gainNode.gain.setValueAtTime(this.volume, this.audioContext.currentTime);
    this.emit('volumeChanged', this.volume);
  }

  /**
   * Get current volume
   */
  getVolume(): number {
    return this.volume;
  }

  /**
   * Get queue length
   */
  getQueueLength(): number {
    return this.playbackQueue.length;
  }

  /**
   * Clear the queue without stopping current playback
   */
  clearQueue() {
    this.playbackQueue = [];
    this.emit('queueCleared');
  }

  /**
   * Get current state
   */
  getState(): AudioOutputState {
    return this.state;
  }

  /**
   * Set state and notify listeners
   */
  private setState(state: AudioOutputState) {
    this.state = state;
    this.emit('stateChange', state);
  }

  /**
   * Event emitter functionality
   */
  on(event: string, callback: Function) {
    if (!this.listeners.has(event)) {
      this.listeners.set(event, new Set());
    }
    this.listeners.get(event)!.add(callback);
  }

  off(event: string, callback: Function) {
    if (this.listeners.has(event)) {
      this.listeners.get(event)!.delete(callback);
    }
  }

  private emit(event: string, ...args: any[]) {
    if (this.listeners.has(event)) {
      this.listeners.get(event)!.forEach(callback => {
        callback(...args);
      });
    }
  }

  /**
   * Check if browser supports required APIs
   */
  static isSupported(): boolean {
    return !!(window.AudioContext && window.ArrayBuffer);
  }
}