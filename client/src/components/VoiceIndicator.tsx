import React, { useEffect, useState } from 'react';
import { VoiceWebSocketService, TranscriptionResult } from '../services/VoiceWebSocketService';

// Simple icon components to avoid lucide-react import issues
const MicIcon = ({ className }: { className?: string }) => (
  <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z" />
    <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
    <line x1="12" y1="19" x2="12" y2="23" />
    <line x1="8" y1="23" x2="16" y2="23" />
  </svg>
);

const VolumeIcon = ({ className }: { className?: string }) => (
  <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polygon points="11 5,6 9,2 9,2 15,6 15,11 19,11 5" />
    <path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07" />
  </svg>
);

const LoaderIcon = ({ className }: { className?: string }) => (
  <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M21 12a9 9 0 11-6.219-8.56" />
  </svg>
);

export interface VoiceIndicatorProps {
  className?: string;
  showTranscription?: boolean;
  showStatus?: boolean;
}

export const VoiceIndicator: React.FC<VoiceIndicatorProps> = ({
  className = '',
  showTranscription = true,
  showStatus = true
}) => {
  const [status, setStatus] = useState<'idle' | 'listening' | 'processing' | 'speaking'>('idle');
  const [transcription, setTranscription] = useState<string>('');
  const [partialTranscription, setPartialTranscription] = useState<string>('');
  const [audioLevel, setAudioLevel] = useState(0);
  const voiceService = VoiceWebSocketService.getInstance();

  useEffect(() => {
    // Set up event listeners
    const handleTranscription = (result: TranscriptionResult) => {
      if (result.isFinal) {
        setTranscription(result.text);
        setPartialTranscription('');
      } else {
        setPartialTranscription(result.text);
      }
    };

    const handleAudioLevel = (level: number) => setAudioLevel(level);

    const handleAudioStreamingStarted = () => setStatus('listening');
    const handleAudioStreamingStopped = () => setStatus('idle');
    const handleAudioStarted = () => setStatus('speaking');
    const handleAudioEnded = () => setStatus('idle');

    voiceService.on('transcription', handleTranscription);
    voiceService.on('audioStreamingStarted', handleAudioStreamingStarted);
    voiceService.on('audioStreamingStopped', handleAudioStreamingStopped);

    const audioInput = voiceService.getAudioInput();
    const audioOutput = voiceService.getAudioOutput();

    audioInput.on('audioLevel', handleAudioLevel);
    audioOutput.on('audioStarted', handleAudioStarted);
    audioOutput.on('audioEnded', handleAudioEnded);

    return () => {
      voiceService.off('transcription', handleTranscription);
      voiceService.off('audioStreamingStarted', handleAudioStreamingStarted);
      voiceService.off('audioStreamingStopped', handleAudioStreamingStopped);
      audioInput.off('audioLevel', handleAudioLevel);
      audioOutput.off('audioStarted', handleAudioStarted);
      audioOutput.off('audioEnded', handleAudioEnded);
    };
  }, []);

  const statusConfig = {
    idle: { icon: null, text: 'Ready', color: 'text-muted-foreground' },
    listening: { icon: MicIcon, text: 'Listening...', color: 'text-destructive' },
    processing: { icon: LoaderIcon, text: 'Processing...', color: 'text-primary' },
    speaking: { icon: VolumeIcon, text: 'Speaking...', color: 'text-accent' }
  };

  const currentStatus = statusConfig[status];

  return (
    <div className={`flex flex-col gap-2 p-3 rounded-lg bg-card border border-border shadow-sm ${className} ${(status === 'idle' && !transcription && !partialTranscription) ? 'opacity-50' : ''}`}>
      {/* Status indicator */}
      {showStatus && currentStatus.icon && (
        <div className={`flex items-center gap-2 ${currentStatus.color}`}>
          <currentStatus.icon
            className={`w-4 h-4 ${status === 'processing' ? 'animate-spin' : ''}`}
          />
          <span className="text-sm font-medium">{currentStatus.text}</span>

          {/* Audio level bars */}
          {status === 'listening' && (
            <div className="flex gap-1 items-center ml-2">
              {[...Array(5)].map((_, i) => (
                <div
                  key={i}
                  className={`w-1 h-3 bg-current transition-all duration-100 ${
                    audioLevel > (i + 1) / 5 ? 'opacity-100' : 'opacity-20'
                  }`}
                  style={{
                    height: audioLevel > (i + 1) / 5 ? '12px' : '6px'
                  }}
                />
              ))}
            </div>
          )}
        </div>
      )}

      {/* Transcription display */}
      {showTranscription && (transcription || partialTranscription) && (
        <div className="text-sm">
          {transcription && (
            <p className="text-foreground">{transcription}</p>
          )}
          {partialTranscription && (
            <p className="text-muted-foreground italic">{partialTranscription}</p>
          )}
        </div>
      )}
    </div>
  );
};