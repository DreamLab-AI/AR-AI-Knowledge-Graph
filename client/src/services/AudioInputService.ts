/**
 * AudioInputService - Manages microphone access, WebRTC setup, and audio recording
 * Provides comprehensive audio input handling with Web Audio API integration
 */

import { AudioContextManager } from './AudioContextManager';
import { gatedConsole } from '../utils/console';

export interface AudioConstraints {
  echoCancellation?: boolean;
  noiseSuppression?: boolean;
  autoGainControl?: boolean;
  sampleRate?: number;
  channelCount?: number;
}

export interface AudioChunk {
  data: ArrayBuffer;
  timestamp: number;
  duration: number;
}

export type AudioInputState = 'idle' | 'requesting' | 'ready' | 'recording' | 'paused' | 'error';

export class AudioInputService {
  private static instance: AudioInputService;
  private stream: MediaStream | null = null;
  private mediaRecorder: MediaRecorder | null = null;
  private audioContext: AudioContext | null = null;
  private sourceNode: MediaStreamAudioSourceNode | null = null;
  private analyserNode: AnalyserNode | null = null;
  private processorNode: AudioWorkletNode | null = null;
  private state: AudioInputState = 'idle';
  private listeners: Map<string, Set<Function>> = new Map();
  private audioChunks: AudioChunk[] = [];
  private recordingStartTime: number = 0;

  private constructor() {
    this.initializeAudioContext();
  }

  static getInstance(): AudioInputService {
    if (!AudioInputService.instance) {
      AudioInputService.instance = new AudioInputService();
    }
    return AudioInputService.instance;
  }

  private async initializeAudioContext() {
    this.audioContext = AudioContextManager.getInstance().getContext();
  }

  /**
   * Request microphone access with specified constraints
   */
  async requestMicrophoneAccess(constraints: AudioConstraints = {}): Promise<boolean> {
    try {
      this.setState('requesting');

      // Comprehensive browser support check
      if (!navigator || !navigator.mediaDevices) {
        throw new Error('Browser does not support media devices. Please use a modern browser with HTTPS.');
      }

      // Check for getUserMedia with fallbacks
      const getUserMedia = navigator.mediaDevices.getUserMedia ||
                          (navigator as any).webkitGetUserMedia ||
                          (navigator as any).mozGetUserMedia ||
                          (navigator as any).msGetUserMedia;

      if (!getUserMedia) {
        throw new Error('Browser does not support microphone access. Please use a modern browser with HTTPS.');
      }

      // Check if we're in a secure context
      if (location.protocol !== 'https:' && location.hostname !== 'localhost' && location.hostname !== '127.0.0.1') {
        throw new Error('Microphone access requires HTTPS or localhost. Please use a secure connection.');
      }

      const defaultConstraints: MediaStreamConstraints = {
        audio: {
          echoCancellation: constraints.echoCancellation ?? true,
          noiseSuppression: constraints.noiseSuppression ?? true,
          autoGainControl: constraints.autoGainControl ?? true,
          sampleRate: constraints.sampleRate ?? 48000,
          channelCount: constraints.channelCount ?? 1
        }
      };

      // Use the modern API if available, fallback to older APIs
      if (navigator.mediaDevices.getUserMedia) {
        this.stream = await navigator.mediaDevices.getUserMedia(defaultConstraints);
      } else {
        // Fallback for older browsers
        this.stream = await new Promise<MediaStream>((resolve, reject) => {
          const legacyGetUserMedia = getUserMedia.bind(navigator);
          legacyGetUserMedia(defaultConstraints, resolve, reject);
        });
      }

      await this.setupAudioNodes();
      this.setState('ready');
      return true;
    } catch (error) {
      gatedConsole.voice.error('Failed to access microphone:', error);
      this.setState('error');
      this.emit('error', error);
      return false;
    }
  }

  /**
   * Setup Web Audio API nodes for processing
   */
  private async setupAudioNodes() {
    if (!this.stream || !this.audioContext) return;

    // Create source from stream
    this.sourceNode = this.audioContext.createMediaStreamSource(this.stream);

    // Create analyser for visualization
    this.analyserNode = this.audioContext.createAnalyser();
    this.analyserNode.fftSize = 2048;
    this.analyserNode.smoothingTimeConstant = 0.8;

    // Connect nodes
    this.sourceNode.connect(this.analyserNode);

    // Load and setup audio worklet if needed
    try {
      await this.setupAudioWorklet();
    } catch (error) {
      gatedConsole.voice.warn('Audio worklet setup failed, continuing without processing:', error);
    }
  }

  /**
   * Setup audio worklet for advanced processing
   */
  private async setupAudioWorklet() {
    if (!this.audioContext || !this.sourceNode) return;

    // Register worklet module
    await this.audioContext.audioWorklet.addModule('/audio-processor.js');

    // Create processor node
    this.processorNode = new AudioWorkletNode(this.audioContext, 'audio-processor', {
      numberOfInputs: 1,
      numberOfOutputs: 1,
      processorOptions: {
        bufferSize: 4096
      }
    });

    // Handle messages from processor
    this.processorNode.port.onmessage = (event) => {
      if (event.data.type === 'audioLevel') {
        this.emit('audioLevel', event.data.level);
      }
    };

    // Connect in chain
    this.sourceNode.disconnect();
    this.sourceNode.connect(this.processorNode);
    this.processorNode.connect(this.analyserNode!);
  }

  /**
   * Start recording audio
   */
  async startRecording(mimeType: string = 'audio/webm;codecs=opus'): Promise<void> {
    if (!this.stream || this.state !== 'ready') {
      throw new Error('Microphone not ready. Call requestMicrophoneAccess first.');
    }

    this.audioChunks = [];
    this.recordingStartTime = Date.now();

    // Check supported mime types
    const supportedType = this.getSupportedMimeType(mimeType);

    this.mediaRecorder = new MediaRecorder(this.stream, {
      mimeType: supportedType,
      audioBitsPerSecond: 128000
    });

    this.mediaRecorder.ondataavailable = (event) => {
      if (event.data.size > 0) {
        this.handleAudioData(event.data);
      }
    };

    this.mediaRecorder.onerror = (error) => {
      gatedConsole.voice.error('MediaRecorder error:', error);
      this.emit('error', error);
      this.stopRecording();
    };

    this.mediaRecorder.onstop = () => {
      this.emit('recordingStopped', this.audioChunks);
    };

    // Start recording with timeslice for streaming
    this.mediaRecorder.start(100); // Get data every 100ms
    this.setState('recording');
    this.emit('recordingStarted');
  }

  /**
   * Stop recording
   */
  stopRecording(): void {
    if (this.mediaRecorder && this.state === 'recording') {
      this.mediaRecorder.stop();
      this.setState('ready');
    }
  }

  /**
   * Pause recording
   */
  pauseRecording(): void {
    if (this.mediaRecorder && this.state === 'recording') {
      this.mediaRecorder.pause();
      this.setState('paused');
      this.emit('recordingPaused');
    }
  }

  /**
   * Resume recording
   */
  resumeRecording(): void {
    if (this.mediaRecorder && this.state === 'paused') {
      this.mediaRecorder.resume();
      this.setState('recording');
      this.emit('recordingResumed');
    }
  }

  /**
   * Handle incoming audio data chunks
   */
  private async handleAudioData(blob: Blob) {
    const arrayBuffer = await blob.arrayBuffer();
    const chunk: AudioChunk = {
      data: arrayBuffer,
      timestamp: Date.now() - this.recordingStartTime,
      duration: 100 // Approximate based on timeslice
    };

    this.audioChunks.push(chunk);
    this.emit('audioChunk', chunk);
  }

  /**
   * Get supported MIME type
   */
  private getSupportedMimeType(preferred: string): string {
    const types = [
      preferred,
      'audio/webm;codecs=opus',
      'audio/webm',
      'audio/ogg;codecs=opus',
      'audio/mp4'
    ];

    for (const type of types) {
      if (MediaRecorder.isTypeSupported(type)) {
        return type;
      }
    }

    return ''; // Browser will use default
  }

  /**
   * Get audio level (0-1)
   */
  getAudioLevel(): number {
    if (!this.analyserNode) return 0;

    const bufferLength = this.analyserNode.frequencyBinCount;
    const dataArray = new Uint8Array(bufferLength);
    this.analyserNode.getByteFrequencyData(dataArray);

    let sum = 0;
    for (let i = 0; i < bufferLength; i++) {
      sum += dataArray[i];
    }

    return sum / (bufferLength * 255);
  }

  /**
   * Get frequency data for visualization
   */
  getFrequencyData(): Uint8Array {
    if (!this.analyserNode) return new Uint8Array(0);

    const bufferLength = this.analyserNode.frequencyBinCount;
    const dataArray = new Uint8Array(bufferLength);
    this.analyserNode.getByteFrequencyData(dataArray);

    return dataArray;
  }

  /**
   * Get waveform data for visualization
   */
  getWaveformData(): Uint8Array {
    if (!this.analyserNode) return new Uint8Array(0);

    const bufferLength = this.analyserNode.frequencyBinCount;
    const dataArray = new Uint8Array(bufferLength);
    this.analyserNode.getByteTimeDomainData(dataArray);

    return dataArray;
  }

  /**
   * Release all resources
   */
  async release() {
    this.stopRecording();

    if (this.processorNode) {
      this.processorNode.disconnect();
      this.processorNode = null;
    }

    if (this.analyserNode) {
      this.analyserNode.disconnect();
      this.analyserNode = null;
    }

    if (this.sourceNode) {
      this.sourceNode.disconnect();
      this.sourceNode = null;
    }

    if (this.stream) {
      this.stream.getTracks().forEach(track => track.stop());
      this.stream = null;
    }

    this.setState('idle');
  }

  /**
   * Get current state
   */
  getState(): AudioInputState {
    return this.state;
  }

  /**
   * Set state and notify listeners
   */
  private setState(state: AudioInputState) {
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
    return !!(navigator.mediaDevices &&
             navigator.mediaDevices.getUserMedia &&
             window.MediaRecorder &&
             (window.AudioContext || (window as any).webkitAudioContext));
  }

  /**
   * Get detailed browser support information
   */
  static getBrowserSupport(): {
    mediaDevices: boolean;
    getUserMedia: boolean;
    mediaRecorder: boolean;
    audioContext: boolean;
    isHttps: boolean;
  } {
    return {
      mediaDevices: !!navigator.mediaDevices,
      getUserMedia: !!(navigator.mediaDevices && navigator.mediaDevices.getUserMedia) ||
                   !!((navigator as any).webkitGetUserMedia ||
                      (navigator as any).mozGetUserMedia ||
                      (navigator as any).msGetUserMedia),
      mediaRecorder: !!window.MediaRecorder,
      audioContext: !!(window.AudioContext || (window as any).webkitAudioContext),
      isHttps: location.protocol === 'https:' ||
               location.hostname === 'localhost' ||
               location.hostname === '127.0.0.1'
    };
  }
}