import React from 'react';
import { VoiceButton, VoiceButtonProps } from './VoiceButton';
import { useSettingsStore } from '../store/settingsStore';
import { nostrAuth } from '../services/nostrAuthService';
import { Lock } from 'lucide-react';

export interface AuthGatedVoiceButtonProps extends VoiceButtonProps {
  onAuthRequired?: () => void;
}

export const AuthGatedVoiceButton: React.FC<AuthGatedVoiceButtonProps> = ({
  onAuthRequired,
  ...voiceButtonProps
}) => {
  const authenticated = useSettingsStore(state => state.authenticated);
  const authEnabled = useSettingsStore(state => state.settings?.auth?.enabled);

  // If auth is not enabled in settings, show the voice button normally
  if (!authEnabled) {
    return <VoiceButton {...voiceButtonProps} />;
  }

  // If auth is enabled but user is not authenticated, show locked button
  if (!authenticated || !nostrAuth.isAuthenticated()) {
    return (
      <button
        className={`
          ${voiceButtonProps.size === 'sm' ? 'h-8 w-8' : voiceButtonProps.size === 'lg' ? 'h-12 w-12' : 'h-10 w-10'}
          ${voiceButtonProps.variant === 'primary' ? 'bg-primary hover:bg-primary/90 text-primary-foreground' : 
            voiceButtonProps.variant === 'secondary' ? 'bg-secondary hover:bg-secondary/90 text-secondary-foreground' : 
            'hover:bg-accent text-accent-foreground'}
          ${voiceButtonProps.className || ''}
          relative flex items-center justify-center
          rounded-full transition-all duration-200
          focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 focus:ring-offset-background
          opacity-50 cursor-not-allowed
        `}
        onClick={() => {
          if (onAuthRequired) {
            onAuthRequired();
          } else {
            alert('Please authenticate with Nostr to use voice features');
          }
        }}
        aria-label="Voice features require authentication"
        title="Voice features require Nostr authentication"
      >
        <Lock className="w-5 h-5" />
      </button>
    );
  }

  // User is authenticated, show normal voice button
  return <VoiceButton {...voiceButtonProps} />;
};