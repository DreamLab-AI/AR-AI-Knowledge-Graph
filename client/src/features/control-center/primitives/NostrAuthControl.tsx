/**
 * NostrAuthControl — ported from UnifiedSettingsTabContent's 'nostr-button'
 * case verbatim (login/logout calls, connected-state readout, power-user
 * badge). design-spec.md §1.9.
 */

import React, { useCallback, useState } from 'react';
import type { RegistryField } from '../registry/types';
import { nostrAuth } from '../../../services/nostrAuthService';
import { useSettingsStore } from '../../../store/settingsStore';
import { cn } from '../../../utils/classNameUtils';

export interface NostrAuthControlProps {
  field: RegistryField;
  testId: string;
  onSuccess?: (message: string) => void;
  onError?: (message: string) => void;
}

export const NostrAuthControl: React.FC<NostrAuthControlProps> = ({ field, testId, onSuccess, onError }) => {
  const isPowerUser = useSettingsStore((s) => s.isPowerUser);
  const [nostrConnected, setNostrConnected] = useState(false);
  const [nostrPublicKey, setNostrPublicKey] = useState('');

  const isConnected = nostrConnected || nostrAuth.isAuthenticated();
  const pubKey = nostrPublicKey || nostrAuth.getCurrentUser()?.pubkey || '';

  const handleLogin = useCallback(async () => {
    try {
      const state = await nostrAuth.login();
      if (state.authenticated && state.user) {
        setNostrConnected(true);
        setNostrPublicKey(state.user.pubkey);
        onSuccess?.('Connected to Nostr');
      }
    } catch {
      onError?.('Failed to connect to Nostr');
    }
  }, [onError, onSuccess]);

  const handleLogout = useCallback(async () => {
    await nostrAuth.logout();
    setNostrConnected(false);
    setNostrPublicKey('');
    onSuccess?.('Disconnected from Nostr');
  }, [onSuccess]);

  return (
    <div className="flex flex-col gap-2" data-testid={`${testId}-container`}>
      <label className="cc-field-label">{field.label}</label>
      {isConnected ? (
        <div className="flex flex-col gap-1.5">
          <div className="text-[9px] break-all p-1.5 rounded border border-emerald-500/30 bg-emerald-500/10 text-emerald-400">
            {pubKey.slice(0, 16)}...{pubKey.slice(-8)}
          </div>
          {isPowerUser && (
            <div className="text-[8px] text-center py-1 px-1.5 rounded bg-amber-400/10 text-amber-400">
              Power User - Full access
            </div>
          )}
          <button
            type="button"
            id={testId}
            data-testid={testId}
            onClick={handleLogout}
            aria-label="Disconnect Nostr"
            className={cn(
              'w-full py-1 px-2.5 rounded text-xs font-semibold text-white',
              'bg-gradient-to-r from-red-500 to-red-600 hover:brightness-110'
            )}
          >
            Disconnect
          </button>
        </div>
      ) : (
        <button
          type="button"
          id={testId}
          data-testid={testId}
          onClick={handleLogin}
          aria-label="Connect Nostr"
          className={cn(
            'w-full py-1 px-2.5 rounded text-xs font-semibold text-white',
            'bg-gradient-to-r from-purple-500 to-purple-600 hover:brightness-110'
          )}
        >
          Connect Nostr
        </button>
      )}
    </div>
  );
};

NostrAuthControl.displayName = 'NostrAuthControl';

export default NostrAuthControl;
