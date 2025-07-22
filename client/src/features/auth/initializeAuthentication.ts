import { nostrAuth, AuthState } from '../../services/nostrAuthService';
import { createLogger, createErrorMetadata } from '../../utils/logger';
import { useSettingsStore } from '../../store/settingsStore';

const logger = createLogger('initAuth');

/**
 * Initialize the authentication system and subscribe to auth changes
 */
export async function initializeAuth(): Promise<void> {
  try {
    // Initialize Nostr auth service
    await nostrAuth.initialize(); // This will also trigger an initial state notification if a session exists

    // Subscribe to auth state changes from nostrAuthService
    nostrAuth.onAuthStateChanged((authState: AuthState) => {
      console.log('[initAuth] Auth state changed:', JSON.stringify(authState, null, 2)); // DEBUG CONSOLE LOG
      logger.info('Auth state changed:', authState);
      if (authState.authenticated && authState.user) {
        console.log('[initAuth] User authenticated:', JSON.stringify(authState.user, null, 2)); // DEBUG CONSOLE LOG
        useSettingsStore.getState().setUser({
          pubkey: authState.user.pubkey,
          isPowerUser: authState.user.isPowerUser,
        });
        useSettingsStore.getState().setAuthenticated(true);
        logger.info('User set in settingsStore:', authState.user);
      } else {
        console.log('[initAuth] User not authenticated or no user data.'); // DEBUG CONSOLE LOG
        useSettingsStore.getState().setUser(null);
        useSettingsStore.getState().setAuthenticated(false);
        logger.info('User cleared from settingsStore.');
      }
    });
    
    logger.info('Auth system initialized and listener set up successfully');
  } catch (error) {
    logger.error('Failed to initialize auth system:', createErrorMetadata(error));
    // Ensure settings store reflects unauthenticated state on error
    useSettingsStore.getState().setUser(null);
    useSettingsStore.getState().setAuthenticated(false);
    throw error;
  }
}
