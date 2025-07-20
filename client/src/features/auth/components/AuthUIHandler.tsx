import React, { useEffect, useState } from 'react';
import { nostrAuth, AuthState } from '@/services/nostrAuthService';
import { createLogger, createErrorMetadata } from '@/utils/logger';
import { Button } from '@/features/design-system/components/Button';
import { Card, CardContent } from '@/features/design-system/components/Card';

const logger = createLogger('AuthUIHandler');

interface AuthUIHandlerProps {
  className?: string;
}

/**
 * AuthUIHandler component that handles the UI for authentication
 */
const AuthUIHandler: React.FC<AuthUIHandlerProps> = ({ className = '' }) => {
  const [authState, setAuthState] = useState<AuthState>({
    authenticated: false
  });
  const [isLoading, setIsLoading] = useState(false);

  useEffect(() => {
    // Subscribe to auth state changes
    const unsubscribe = nostrAuth.onAuthStateChanged((state) => {
      setAuthState(state);
    });

    // Cleanup subscription on unmount
    return () => {
      unsubscribe();
    };
  }, []);

  const handleLogin = async () => {
    try {
      setIsLoading(true);
      await nostrAuth.login();
    } catch (error) {
      logger.error('Login failed:', createErrorMetadata(error));
      // Error is already handled by the auth service and will be in authState
    } finally {
      setIsLoading(false);
    }
  };

  const handleLogout = async () => {
    try {
      setIsLoading(true);
      await nostrAuth.logout();
    } catch (error) {
      logger.error('Logout failed:', createErrorMetadata(error));
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className={`auth-ui-handler ${className}`}>
      {authState.authenticated && authState.user ? (
        <div className="user-info space-y-4">
          <div className="flex flex-col space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-sm text-gray-400">Logged in as:</span>
              <span className="font-mono text-sm">
                {authState.user.pubkey.slice(0, 8)}...{authState.user.pubkey.slice(-8)}
              </span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm text-gray-400">Role:</span>
              <span className="text-sm font-medium">
                {authState.user.isPowerUser ? 'Power User' : 'Authenticated User'}
              </span>
            </div>
          </div>
          <Button
            variant="destructive"
            onClick={handleLogout}
            disabled={isLoading}
            className="w-full"
          >
            {isLoading ? 'Logging out...' : 'Logout'}
          </Button>
        </div>
      ) : (
        <div className="space-y-4">
          <Button
            onClick={handleLogin}
            disabled={isLoading}
            className="w-full"
          >
            {isLoading ? 'Connecting...' : 'Login with Nostr'}
          </Button>
          {authState.error && (
            <div className="text-red-500 text-sm mt-2">{authState.error}</div>
          )}
        </div>
      )}
    </div>
  );
};

export default AuthUIHandler;
