import { useState, useEffect } from 'react';
import { useSettingsStore } from '../../../store/settingsStore';
import { createLogger, createErrorMetadata } from '../../../utils/logger';
import { nostrAuth, AuthState } from '../../../services/nostrAuthService';

const logger = createLogger('useAuth');

/**
 * Hook for accessing Nostr authentication functionality
 * This hook synchronizes the auth state with the settings store
 */
const useAuth = () => {
  const { setAuthenticated, setUser, authenticated, user } = useSettingsStore();
  const [authError, setAuthError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  // Listen for auth state changes and update the settings store
  useEffect(() => {
    const unsubscribe = nostrAuth.onAuthStateChanged((state: AuthState) => {
      if (state.authenticated && state.user) {
        setAuthenticated(true);
        setUser({
          isPowerUser: state.user.isPowerUser,
          pubkey: state.user.pubkey
        });
        setAuthError(null);
      } else {
        setAuthenticated(false);
        setUser(null);
        if (state.error) {
          setAuthError(state.error);
        }
      }
    });

    return () => {
      unsubscribe();
    };
  }, [setAuthenticated, setUser]);

  const login = async () => {
    try {
      setIsLoading(true);
      setAuthError(null);
      await nostrAuth.login();
      logger.info('Login successful');
    } catch (error) {
      logger.error('Login failed:', createErrorMetadata(error));
      setAuthError(error instanceof Error ? error.message : 'Login failed');
    } finally {
      setIsLoading(false);
    }
  };

  const logout = async () => {
    try {
      setIsLoading(true);
      setAuthError(null);
      await nostrAuth.logout();
      logger.info('Logout successful');
    } catch (error) {
      logger.error('Logout failed:', createErrorMetadata(error));
      setAuthError(error instanceof Error ? error.message : 'Logout failed');
    } finally {
      setIsLoading(false);
    }
  };

  return {
    authenticated,
    user,
    authError,
    isLoading,
    login,
    logout,
  };
};

export default useAuth;