import { apiService } from './api';
import { createLogger, createErrorMetadata } from '../utils/logger';
import { Event, UnsignedEvent, nip19 } from 'nostr-tools';
import { v4 as uuidv4 } from 'uuid'; // Import uuid
import type {} from '../types/nip07'; // Import the types to ensure Window augmentation

const logger = createLogger('NostrAuthService');

// --- Interfaces ---

// User info stored locally and used in AuthState
export interface SimpleNostrUser {
  pubkey: string; // hex pubkey
  npub?: string; // npub format
  isPowerUser: boolean; // Keep for UI rendering decisions
}

// User info returned by backend
export interface BackendNostrUser {
  pubkey: string;
  npub?: string;
  isPowerUser: boolean; // Server determines this based on POWER_USER_PUBKEYS
  // Add other fields the backend might send if needed
}

// Response from POST /auth/nostr
export interface AuthResponse {
  user: BackendNostrUser;
  token: string;
  expiresAt: number; // Unix timestamp (seconds)
  features?: string[]; // Optional features list from backend
}

// Response from POST /auth/nostr/verify
export interface VerifyResponse {
  valid: boolean;
  user?: BackendNostrUser;
  features?: string[];
}

// Payload for POST /auth/nostr (signed NIP-42 event)
export interface AuthEventPayload {
  id: string;
  pubkey: string;
  content: string;
  sig: string;
  created_at: number; // Unix timestamp
  kind: number;
  tags: string[][];
}

// State exposed to the application
export interface AuthState {
  authenticated: boolean;
  user?: SimpleNostrUser;
  error?: string;
}

type AuthStateListener = (state: AuthState) => void;

// --- Service Implementation ---

class NostrAuthService {
  private static instance: NostrAuthService;
  private sessionToken: string | null = null;
  private currentUser: SimpleNostrUser | null = null;
  private authStateListeners: AuthStateListener[] = [];
  private initialized = false;

  private constructor() {}

  public static getInstance(): NostrAuthService {
    if (!NostrAuthService.instance) {
      NostrAuthService.instance = new NostrAuthService();
    }
    return NostrAuthService.instance;
  }

  /**
   * Checks if a NIP-07 provider (window.nostr) is available.
   */
  public hasNip07Provider(): boolean {
    return typeof window !== 'undefined' && window.nostr !== undefined;
  }

  /**
   * Initializes the service, checking for stored sessions.
   */
  public async initialize(): Promise<void> {
    if (this.initialized) return;
    logger.debug('Initializing NostrAuthService...');

    const storedToken = localStorage.getItem('nostr_session_token');
    const storedUserJson = localStorage.getItem('nostr_user'); // Stores SimpleNostrUser

    if (storedToken && storedUserJson) {
      let storedUser: SimpleNostrUser | null = null;
      try {
        storedUser = JSON.parse(storedUserJson);
      } catch (parseError) {
        logger.error('Failed to parse stored user data:', createErrorMetadata(parseError));
        this.clearSession();
      }

      if (storedUser) {
        logger.info(`Verifying stored session for pubkey: ${storedUser.pubkey}`);
        try {
          // Verify token with backend
          const verificationResponse = await apiService.post<VerifyResponse>('/auth/nostr/verify', {
            pubkey: storedUser.pubkey,
            token: storedToken
          });

          if (verificationResponse.valid) { // Check only for token validity
            this.sessionToken = storedToken;
            if (verificationResponse.user) { // If backend provides updated user info
              this.currentUser = {
                pubkey: verificationResponse.user.pubkey,
                npub: verificationResponse.user.npub || this.hexToNpub(verificationResponse.user.pubkey),
                isPowerUser: verificationResponse.user.isPowerUser,
              };
              logger.info('Token verified and user details updated from backend.');
            } else if (storedUser) {
              // Token is valid, but backend didn't return user info.
              // Trust the locally stored user info for this session.
              this.currentUser = storedUser; // storedUser is confirmed non-null here
              logger.info('Token verified, using stored user details as backend did not provide them on verify.');
            } else {
              // This case should ideally not be reached if storedUser was parsed correctly.
              logger.error('Token verified but no user details available from backend or local storage. Clearing session.');
              this.clearSession();
              this.notifyListeners({ authenticated: false, error: 'User details missing after verification' });
              return; // Exit early
            }
            this.storeCurrentUser(); // Store potentially updated or re-confirmed user
            this.notifyListeners(this.getCurrentAuthState());
            logger.info('Restored and verified session from local storage.');
          } else {
            // Session invalid (verificationResponse.valid is false)
            logger.warn('Stored session token is invalid (verification failed), clearing session.');
            this.clearSession();
            this.notifyListeners({ authenticated: false });
          }
        } catch (error) {
          logger.error('Failed to verify stored session with backend:', createErrorMetadata(error));
          this.clearSession();
          this.notifyListeners({ authenticated: false, error: 'Session verification failed' });
        }
      }
    } else {
      logger.info('No stored session found.');
      this.notifyListeners({ authenticated: false });
    }
    this.initialized = true;
    logger.debug('NostrAuthService initialized.');
  }

  /**
   * Initiates the NIP-07 login flow.
   */
  public async login(): Promise<AuthState> {
    logger.info('Attempting NIP-07 login...');
    if (!this.hasNip07Provider()) {
      const errorMsg = 'Nostr NIP-07 provider (e.g., Alby) not found. Please install a compatible extension.';
      logger.error(errorMsg);
      this.notifyListeners({ authenticated: false, error: errorMsg });
      throw new Error(errorMsg);
    }

    try {
      // 1. Get public key from NIP-07 provider
      const pubkey = await window.nostr!.getPublicKey();
      if (!pubkey) {
        throw new Error('Could not get public key from NIP-07 provider.');
      }
      logger.info(`Got pubkey via NIP-07: ${pubkey}`);

      // 2. Construct NIP-42 Authentication Event (Kind 22242)
      const challenge = uuidv4(); // Use uuidv4 to generate the challenge
      // TODO: Make relayUrl configurable or obtained from the provider if possible
      const relayUrl = 'wss://relay.damus.io';

      // Prepare the unsigned event structure expected by NIP-07 signEvent
      const unsignedNip07Event = {
        created_at: Math.floor(Date.now() / 1000),
        kind: 22242,
        tags: [
          ['relay', relayUrl],
          ['challenge', challenge]
        ],
        content: 'Authenticate to LogseqSpringThing' // Customize as needed
      };

      // 3. Sign the event using NIP-07 provider
      logger.debug('Requesting signature via NIP-07 for event:', unsignedNip07Event);
      const signedEvent: Event = await window.nostr!.signEvent(unsignedNip07Event);
      logger.debug('Event signed successfully via NIP-07.');

      // 4. Prepare payload for backend
      const eventPayload: AuthEventPayload = {
        id: signedEvent.id,
        pubkey: signedEvent.pubkey, // pubkey is added by the signer
        content: signedEvent.content,
        sig: signedEvent.sig,
        created_at: signedEvent.created_at,
        kind: signedEvent.kind,
        tags: signedEvent.tags,
      };

      // 5. Send the signed event to the backend API
      logger.info(`Sending auth event to backend for pubkey: ${pubkey}`);
      const response = await apiService.post<AuthResponse>('/auth/nostr', eventPayload);
      logger.info(`Backend auth successful for pubkey: ${response.user.pubkey}`);

      // 6. Store session and update state
      this.sessionToken = response.token;
      this.currentUser = {
        pubkey: response.user.pubkey,
        npub: response.user.npub || this.hexToNpub(response.user.pubkey),
        isPowerUser: response.user.isPowerUser,
      };

      this.storeSessionToken(response.token);
      this.storeCurrentUser(); // Store SimpleNostrUser

      const newState = this.getCurrentAuthState();
      this.notifyListeners(newState);
      return newState;

    } catch (error: any) {
      const errorMeta = createErrorMetadata(error);
      logger.error(`NIP-07 login failed. Details: ${JSON.stringify(errorMeta, null, 2)}`);
      let errorMessage = 'Login failed';
      if (error?.response?.data?.error) { // Check for backend error structure
        errorMessage = error.response.data.error;
      } else if (error?.message) {
        errorMessage = error.message;
      } else if (typeof error === 'string') {
        errorMessage = error;
      }

      // Refine common error messages
      if (errorMessage.includes('User rejected') || errorMessage.includes('extension rejected')) {
        errorMessage = 'Login request rejected in Nostr extension.';
      } else if (errorMessage.includes('401') || errorMessage.includes('Invalid signature')) {
        errorMessage = 'Authentication failed: Invalid signature or credentials.';
      } else if (errorMessage.includes('Could not get public key')) {
        errorMessage = 'Failed to get public key from Nostr extension.';
      }

      const errorState: AuthState = { authenticated: false, error: errorMessage };
      this.notifyListeners(errorState);
      // Re-throw the error so UI components can potentially handle it too
      throw new Error(errorMessage);
    }
  }

  /**
   * Logs the user out.
   */
  public async logout(): Promise<void> {
    logger.info('Attempting logout...');
    const token = this.sessionToken;
    const user = this.currentUser;

    // Clear local state immediately for faster UI update
    const wasAuthenticated = this.isAuthenticated();
    this.clearSession();
    if (wasAuthenticated) {
        this.notifyListeners({ authenticated: false }); // Notify UI immediately only if state changed
    }


    if (token && user) {
      try {
        logger.info(`Calling server logout for pubkey: ${user.pubkey}`);
        // Server expects DELETE with pubkey and token in body
        await apiService.delete<any>('/auth/nostr', {
          pubkey: user.pubkey,
          token: token
        });
        logger.info('Server logout successful.');
      } catch (error) {
        // Log the error but don't re-throw, as client-side logout is already done
        logger.error('Server logout call failed:', createErrorMetadata(error));
        // Optionally notify listeners about the server error?
        // this.notifyListeners({ authenticated: false, error: 'Server logout failed but client session cleared' });
      }
    } else {
      logger.warn('Logout called but no active session found locally.');
    }
  }

  // --- State Management & Helpers ---

  private storeSessionToken(token: string): void {
    localStorage.setItem('nostr_session_token', token);
  }

  private storeCurrentUser(): void {
    if (this.currentUser) {
      localStorage.setItem('nostr_user', JSON.stringify(this.currentUser));
    } else {
      localStorage.removeItem('nostr_user');
    }
  }

  private clearSession(): void {
    this.sessionToken = null;
    this.currentUser = null;
    localStorage.removeItem('nostr_session_token');
    localStorage.removeItem('nostr_user');
  }

  public onAuthStateChanged(listener: AuthStateListener): () => void {
    this.authStateListeners.push(listener);
    if (this.initialized) { // Notify immediately if already initialized
      listener(this.getCurrentAuthState());
    }
    // Return unsubscribe function
    return () => {
      this.authStateListeners = this.authStateListeners.filter(l => l !== listener);
    };
  }

  private notifyListeners(state: AuthState): void {
    this.authStateListeners.forEach(listener => {
      try {
        listener(state);
      } catch (error) {
        logger.error('Error in auth state listener:', createErrorMetadata(error));
      }
    });
  }

  public getCurrentUser(): SimpleNostrUser | null {
    return this.currentUser;
  }

  public getSessionToken(): string | null {
    return this.sessionToken;
  }

  public isAuthenticated(): boolean {
    return !!this.sessionToken && !!this.currentUser;
  }

  public getCurrentAuthState(): AuthState {
    return {
      authenticated: this.isAuthenticated(),
      user: this.currentUser ? { ...this.currentUser } : undefined, // Return a copy
      error: undefined // Reset error on state check, or manage error state separately
    };
  }

  // --- NIP-19 Helpers ---

  public hexToNpub(pubkey: string): string | undefined {
    if (!pubkey) return undefined;
    try {
      return nip19.npubEncode(pubkey);
    } catch (error) {
      logger.warn(`Failed to convert hex to npub: ${pubkey}`, createErrorMetadata(error));
      return undefined;
    }
  }

  public npubToHex(npub: string): string | undefined {
    if (!npub) return undefined;
    try {
      const decoded = nip19.decode(npub);
      if (decoded.type === 'npub') {
        return decoded.data;
      }
      throw new Error('Invalid npub format');
    } catch (error) {
      logger.warn(`Failed to convert npub to hex: ${npub}`, createErrorMetadata(error));
      return undefined;
    }
  }
}

// Export a singleton instance
export const nostrAuth = NostrAuthService.getInstance();
