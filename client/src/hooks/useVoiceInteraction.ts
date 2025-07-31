/**
 * useVoiceInteraction - React hook for voice-to-voice interaction
 * Provides a simple interface for voice input/output in components
 */

import { useState, useEffect, useCallback, useRef } from 'react';
import { VoiceWebSocketService, TranscriptionResult } from '../services/VoiceWebSocketService';
import { useSettingsStore } from '../store/settingsStore';
import { gatedConsole } from '../utils/console';

export interface UseVoiceInteractionOptions {
  autoConnect?: boolean;
  onTranscription?: (text: string, isFinal: boolean) => void;
  onError?: (error: any) => void;
  language?: string;
}

export interface UseVoiceInteractionReturn {
  isConnected: boolean;
  isListening: boolean;
  isSpeaking: boolean;
  transcription: string;
  partialTranscription: string;
  startListening: () => Promise<void>;
  stopListening: () => void;
  speak: (text: string) => Promise<void>;
  toggleListening: () => Promise<void>;
  connect: () => Promise<void>;
  disconnect: () => Promise<void>;
}

export function useVoiceInteraction(options: UseVoiceInteractionOptions = {}): UseVoiceInteractionReturn {
  const { autoConnect = true, onTranscription, onError, language } = options;
  const settings = useSettingsStore((state) => state.settings);

  // All state hooks must be called unconditionally
  const [isConnected, setIsConnected] = useState(false);
  const [isListening, setIsListening] = useState(false);
  const [isSpeaking, setIsSpeaking] = useState(false);
  const [transcription, setTranscription] = useState('');
  const [partialTranscription, setPartialTranscription] = useState('');

  // All ref hooks must be called unconditionally
  const voiceServiceRef = useRef<VoiceWebSocketService>();
  const autoConnectAttemptedRef = useRef(false);
  const onTranscriptionRef = useRef(onTranscription);
  const onErrorRef = useRef(onError);

  // Update refs on every render to have latest callbacks
  onTranscriptionRef.current = onTranscription;
  onErrorRef.current = onError;

  // All useEffect hooks must be called unconditionally and in the same order
  useEffect(() => {
    voiceServiceRef.current = VoiceWebSocketService.getInstance();
    const voiceService = voiceServiceRef.current;

    // Set up event listeners
    const handleConnected = () => setIsConnected(true);
    const handleDisconnected = () => {
      setIsConnected(false);
      setIsListening(false);
    };

    const handleTranscription = (result: TranscriptionResult) => {
      if (result.isFinal) {
        setTranscription(result.text);
        setPartialTranscription('');
        onTranscriptionRef.current?.(result.text, true);
      } else {
        setPartialTranscription(result.text);
        onTranscriptionRef.current?.(result.text, false);
      }
    };

    const handleAudioStarted = () => setIsSpeaking(true);
    const handleAudioEnded = () => setIsSpeaking(false);

    const handleError = (error: any) => {
      gatedConsole.voice.error('Voice interaction error:', error);
      onErrorRef.current?.(error);
    };

    // Subscribe to events
    voiceService.on('connected', handleConnected);
    voiceService.on('disconnected', handleDisconnected);
    voiceService.on('transcription', handleTranscription);
    voiceService.on('voiceError', handleError);

    const audioOutput = voiceService.getAudioOutput();
    audioOutput.on('audioStarted', handleAudioStarted);
    audioOutput.on('audioEnded', handleAudioEnded);

    // Cleanup
    return () => {
      voiceService.off('connected', handleConnected);
      voiceService.off('disconnected', handleDisconnected);
      voiceService.off('transcription', handleTranscription);
      voiceService.off('voiceError', handleError);
      audioOutput.off('audioStarted', handleAudioStarted);
      audioOutput.off('audioEnded', handleAudioEnded);
    };
  }, []); // Empty dependency array to avoid re-registration

  // Separate effect for auto-connect to avoid dependency issues
  useEffect(() => {
    if (autoConnect && !autoConnectAttemptedRef.current && voiceServiceRef.current && (settings.system?.customBackendUrl || window.location.origin)) {
      autoConnectAttemptedRef.current = true;
      connect().catch((error) => gatedConsole.voice.error('Auto-connect failed:', error));
    }
  }, [autoConnect, settings.system?.customBackendUrl]);

  const connect = useCallback(async () => {
    if (!voiceServiceRef.current || isConnected) return;

    try {
      const baseUrl = settings.system?.customBackendUrl || window.location.origin;
      await voiceServiceRef.current.connectToSpeech(baseUrl);
    } catch (error) {
      gatedConsole.voice.error('Failed to connect to voice service:', error);
      onError?.(error);
    }
  }, [isConnected, settings.system?.customBackendUrl]);

  const disconnect = useCallback(async () => {
    if (!voiceServiceRef.current) return;

    try {
      await voiceServiceRef.current.disconnect();
      autoConnectAttemptedRef.current = false; // Reset auto-connect flag
    } catch (error) {
      gatedConsole.voice.error('Failed to disconnect from voice service:', error);
      onError?.(error);
    }
  }, []);

  const startListening = useCallback(async () => {
    if (!voiceServiceRef.current || !isConnected || isListening) return;

    try {
      await voiceServiceRef.current.startAudioStreaming({ language });
      setIsListening(true);
    } catch (error) {
      gatedConsole.voice.error('Failed to start listening:', error);
      onError?.(error);
    }
  }, [isConnected, isListening, language]);

  const stopListening = useCallback(() => {
    if (!voiceServiceRef.current || !isListening) return;

    voiceServiceRef.current.stopAudioStreaming();
    setIsListening(false);
  }, [isListening]);

  const speak = useCallback(async (text: string) => {
    if (!voiceServiceRef.current || !isConnected) {
      throw new Error('Not connected to voice service');
    }

    try {
      await voiceServiceRef.current.sendTextForTTS({
        text,
        voice: settings.kokoro?.defaultVoice,
        speed: settings.kokoro?.defaultSpeed,
        stream: settings.kokoro?.stream ?? true
      });
    } catch (error) {
      gatedConsole.voice.error('Failed to speak:', error);
      onError?.(error);
      throw error;
    }
  }, [isConnected, settings.kokoro?.defaultVoice, settings.kokoro?.defaultSpeed, settings.kokoro?.stream]);

  const toggleListening = useCallback(async () => {
    if (isListening) {
      stopListening();
    } else {
      await startListening();
    }
  }, [isListening, startListening, stopListening]);

  return {
    isConnected,
    isListening,
    isSpeaking,
    transcription,
    partialTranscription,
    startListening,
    stopListening,
    speak,
    toggleListening,
    connect,
    disconnect
  };
}