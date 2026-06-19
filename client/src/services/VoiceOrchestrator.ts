/**
 * VoiceOrchestrator — Coordinates all voice services for a single user session.
 *
 * Wires together:
 *   - PushToTalkService:   PTT key handling → route mic to agents or chat
 *   - VoiceWebSocketService: /ws/speech for agent commands (Plane 1) + agent TTS (Plane 2)
 *   - LiveKitVoiceService:  WebRTC spatial voice chat (Plane 3) + agent spatial (Plane 4)
 *   - AudioInputService:    Microphone capture
 *
 * Lifecycle:
 *   1. User logs in → orchestrator.initialize(userId, serverUrl, livekitToken)
 *   2. PTT held → mic routes to /ws/speech (Turbo Whisper STT → agent commands)
 *   3. PTT released → mic routes to LiveKit (spatial voice chat)
 *   4. Agent response → Kokoro TTS → audio arrives on /ws/speech → user hears it
 *   5. User disconnects → orchestrator.dispose()
 */

import { PushToTalkService, PTTState } from './PushToTalkService';
import { VoiceWebSocketService } from './VoiceWebSocketService';
import { LiveKitVoiceService, SpatialPosition } from './LiveKitVoiceService';
import { AudioInputService } from './AudioInputService';
import { createLogger } from '../utils/loggerConfig';

const logger = createLogger('VoiceOrchestrator');

export interface VoiceOrchestratorConfig {
  /** VisionClaw API base URL (e.g., http://localhost:4000) */
  serverUrl: string;
  /** LiveKit server URL (e.g., ws://localhost:7880) */
  livekitUrl: string;
  /** LiveKit access token (generated server-side) */
  livekitToken: string;
  /** LiveKit room name (typically "visionclaw-{world_id}") */
  livekitRoom: string;
  /** User identifier */
  userId: string;
  /** Push-to-talk key (default: Space) */
  pttKey?: string;
  /** Enable spatial audio */
  spatialAudio?: boolean;
  /** Max distance for spatial audio rolloff */
  maxDistance?: number;
}

export class VoiceOrchestrator {
  private ptt: PushToTalkService;
  private voiceWs: VoiceWebSocketService;
  private livekit: LiveKitVoiceService;
  private audioInput: AudioInputService;
  private config: VoiceOrchestratorConfig | null = null;
  private isInitialized = false;
  private cleanupCallbacks: (() => void)[] = [];

  constructor() {
    this.ptt = PushToTalkService.getInstance();
    this.voiceWs = VoiceWebSocketService.getInstance();
    this.livekit = LiveKitVoiceService.getInstance();
    this.audioInput = AudioInputService.getInstance();
  }

  /**
   * Initialize the full voice pipeline for a user.
   */
  async initialize(config: VoiceOrchestratorConfig): Promise<void> {
    if (this.isInitialized) {
      logger.warn('VoiceOrchestrator already initialized, disposing first');
      await this.dispose();
    }

    this.config = config;
    logger.info(`Initializing voice orchestrator for user ${config.userId}`);

    // 1. Connect to VisionClaw speech WebSocket (agent commands + TTS response)
    try {
      await this.voiceWs.connectToSpeech(config.serverUrl);
      logger.info('Connected to VisionClaw speech WebSocket');
    } catch (error) {
      logger.error('Failed to connect to speech WebSocket:', error);
      throw error;
    }

    // 2. Connect to LiveKit room (spatial voice chat)
    try {
      await this.livekit.connect({
        serverUrl: config.livekitUrl,
        token: config.livekitToken,
        roomName: config.livekitRoom,
        spatialAudio: config.spatialAudio ?? true,
        maxDistance: config.maxDistance ?? 50,
      });
      logger.info('Connected to LiveKit room');
    } catch (error) {
      // LiveKit is optional — warn but don't fail
      logger.warn('LiveKit connection failed (spatial voice chat unavailable):', error);
    }

    // 3. Set up PTT routing
    this.ptt.updateConfig({
      key: config.pttKey ?? ' ',
      mode: 'push',
      voiceChatEnabled: this.livekit.getIsConnected(),
    });

    // Notify server of PTT state via WebSocket
    this.ptt.onServerNotify((pttActive: boolean) => {
      if (this.voiceWs) {
        // Send PTT state to server so it knows to route audio to STT
        const msg = JSON.stringify({
          type: 'ptt',
          active: pttActive,
          userId: config.userId,
        });
        // Access internal send — or use the public sendTextForTTS pathway
        // For now, rely on the voice WS handling binary audio when PTT is active
      }
    });

    // Wire PTT state changes to audio routing
    const pttUnsub = this.ptt.onStateChange((state: PTTState) => {
      this.handlePTTStateChange(state);
    });
    this.cleanupCallbacks.push(pttUnsub);

    // 4. Activate PTT key listener
    this.ptt.activate(config.userId);

    // 5. Request microphone access
    try {
      await this.audioInput.requestMicrophoneAccess({
        echoCancellation: true,
        noiseSuppression: true,
        autoGainControl: true,
        sampleRate: 48000,
        channelCount: 1,
      });
      logger.info('Microphone access granted');
    } catch (error) {
      logger.error('Microphone access denied:', error);
      throw error;
    }

    this.isInitialized = true;
    logger.info('Voice orchestrator initialized');
  }

  /**
   * Handle PTT state transitions — the core routing logic.
   */
  private async handlePTTStateChange(state: PTTState): Promise<void> {
    switch (state) {
      case 'commanding':
        // PTT pressed → route mic to agent commands via /ws/speech
        logger.debug('PTT: routing mic → agent commands');

        // Mute LiveKit publishing (don't send commands to voice chat)
        await this.livekit.stopPublishing();

        // Start recording for STT
        try {
          await this.voiceWs.startAudioStreaming({ language: 'en' });
        } catch (error) {
          logger.error('Failed to start audio streaming for commands:', error);
        }
        break;

      case 'chatting':
        // PTT released → route mic to LiveKit spatial voice chat
        logger.debug('PTT: routing mic → spatial voice chat');

        // Stop sending to /ws/speech
        this.voiceWs.stopAudioStreaming();

        // Start publishing to LiveKit
        await this.livekit.startPublishing();
        break;

      case 'idle':
        // Both channels muted
        logger.debug('PTT: idle — all channels muted');
        this.voiceWs.stopAudioStreaming();
        await this.livekit.stopPublishing();
        break;
    }
  }

  /**
   * Update the local user's spatial position.
   * Call this from the XR presence sync loop.
   */
  updateUserPosition(position: SpatialPosition): void {
    this.livekit.updateListenerPosition(position);
  }

  /**
   * Update a remote participant's (user or agent) spatial position.
   * Call this when XR entity positions change.
   */
  updateRemotePosition(participantId: string, position: SpatialPosition): void {
    this.livekit.updateParticipantPosition(participantId, position);
  }

  /**
   * Clean up all voice services.
   */
  async dispose(): Promise<void> {
    logger.info('Disposing voice orchestrator');

    this.ptt.deactivate();
    this.voiceWs.stopAllAudio();
    await this.livekit.disconnect();

    // Run cleanup callbacks
    this.cleanupCallbacks.forEach(cb => cb());
    this.cleanupCallbacks = [];

    this.isInitialized = false;
    this.config = null;
  }

  /** Check if the orchestrator is active */
  getIsInitialized(): boolean {
    return this.isInitialized;
  }

  /** Get current PTT state */
  getPTTState(): PTTState {
    return this.ptt.getState();
  }
}
