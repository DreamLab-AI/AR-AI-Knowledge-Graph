import React from 'react';
import { VoiceIndicator, VoiceIndicatorProps } from './VoiceIndicator';
import { useSettingsStore } from '../store/settingsStore';
import { nostrAuth } from '../services/nostrAuthService';

export interface AuthGatedVoiceIndicatorProps extends VoiceIndicatorProps {
  onAuthRequired?: () => void;
}

export const AuthGatedVoiceIndicator: React.FC<AuthGatedVoiceIndicatorProps> = ({
  onAuthRequired,
  ...voiceIndicatorProps
}) => {
  const authenticated = useSettingsStore(state => state.authenticated);
  const authEnabled = useSettingsStore(state => state.settings?.auth?.enabled);

  // If auth is not enabled in settings, show the voice indicator normally
  if (!authEnabled) {
    return <VoiceIndicator {...voiceIndicatorProps} />;
  }

  // If auth is enabled but user is not authenticated, show auth required message
  if (!authenticated || !nostrAuth.isAuthenticated()) {
    return (
      <div 
        className={`${voiceIndicatorProps.className || ''} text-muted-foreground italic cursor-pointer`}
        onClick={() => {
          if (onAuthRequired) {
            onAuthRequired();
          } else {
            alert('Please authenticate with Nostr to use voice features');
          }
        }}
      >
        Voice features require Nostr authentication
      </div>
    );
  }

  // User is authenticated, show normal voice indicator
  return <VoiceIndicator {...voiceIndicatorProps} />;
};