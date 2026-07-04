import { useEffect, useCallback, useState, lazy, Suspense } from 'react'
import AppInitializer from './AppInitializer'
import { ApplicationModeProvider } from '../contexts/ApplicationModeContext';
import { useSettingsStore } from '../store/settingsStore';
import { createLogger } from '../utils/loggerConfig';
import MainLayout from './MainLayout';
import { useQuest3Integration } from '../hooks/useQuest3Integration';
// Lazy-load the WebXR/three.js immersive client. It is only rendered on
// Quest 3 / forced-VR sessions, so the desktop majority never downloads the
// heavy XR bundle. Code-split out of the initial chunk.
const ImmersiveApp = lazy(() => import('../immersive/components/ImmersiveApp'));
import { BotsDataProvider } from '../features/bots/contexts/BotsDataContext';
import { CommandPalette } from '../features/command-palette/components/CommandPalette';
import { initializeCommandPalette } from '../features/command-palette/defaultCommands';
import { registerSettingsCommands } from '../features/control-center/registry/paletteCommands';
import { HelpProvider } from '../features/help/components/HelpProvider';
import { registerSettingsHelp } from '../features/help/settingsHelp';
import { OnboardingProvider } from '../features/onboarding/components/OnboardingProvider';
import { registerOnboardingCommands } from '../features/onboarding/flows/defaultFlows';
import { TooltipProvider } from '../features/design-system/components/Tooltip';
import { useBotsWebSocketIntegration } from '../features/bots/hooks/useBotsWebSocketIntegration';
import { DebugControlPanel } from '../components/DebugControlPanel';
import { ConnectionWarning } from '../components/ConnectionWarning';
import { useAutoBalanceNotifications } from '../hooks/useAutoBalanceNotifications';
import ErrorBoundary from '../components/ErrorBoundary';
import { remoteLogger } from '../services/remoteLogger';
import { VircadiaProvider } from '../contexts/VircadiaContext';
import { VircadiaBridgesProvider } from '../contexts/VircadiaBridgesContext';
import { useNostrAuth } from '../hooks/useNostrAuth';
import { OnboardingWizard } from '../components/OnboardingWizard';
import { LoadingScreen } from '../components/LoadingScreen';
import { WorkerErrorModal } from '../components/WorkerErrorModal';
import solidPodService from '../services/SolidPodService';

const logger = createLogger('App');

// Initialize remote logging for Quest 3 debugging
if (typeof window !== 'undefined') {
  remoteLogger.logXRInfo();
}

function App() {
  const [initializationState, setInitializationState] = useState<'loading' | 'initialized' | 'error'>('loading');
  const [initializationError, setInitializationError] = useState<Error | null>(null);
  const initialized = useSettingsStore(state => state.initialized);

  // Auth state
  const { authenticated, isLoading: isAuthLoading, user } = useNostrAuth();

  const { shouldUseQuest3Layout, isQuest3Detected, autoStartSuccessful } = useQuest3Integration({
    enableAutoStart: false
  });


  const botsConnectionStatus = useBotsWebSocketIntegration();


  useAutoBalanceNotifications();

  // Update settings store with auth state and connect Solid WebSocket
  useEffect(() => {
    if (authenticated && user) {
      const settingsStore = useSettingsStore.getState();
      settingsStore.setAuthenticated(true);
      settingsStore.setUser({
        isPowerUser: user.isPowerUser,
        pubkey: user.pubkey
      });

      // Connect Solid WebSocket for real-time pod notifications
      solidPodService.connectWebSocket();
    } else {
      const settingsStore = useSettingsStore.getState();
      settingsStore.setAuthenticated(false);
      settingsStore.setUser(null);

      // Disconnect Solid WebSocket when user logs out
      solidPodService.disconnect();
    }

    // Cleanup on unmount
    return () => {
      solidPodService.disconnect();
    };
  }, [authenticated, user]);

  
  const shouldUseImmersiveClient = () => {
    // Check for desktop force FIRST - allows overriding VR detection
    const forceDesktop = window.location.search.includes('force=desktop') ||
                         window.location.search.includes('vr=false') ||
                         window.location.search.includes('mode=desktop');

    if (forceDesktop) {
      logger.info('[App] Desktop mode forced via URL parameter');
      return false;
    }

    const userAgent = navigator.userAgent;

    const isQuest3Browser = userAgent.includes('Quest 3') ||
                            userAgent.includes('Quest3') ||
                            userAgent.includes('OculusBrowser') ||
                            (userAgent.includes('VR') && userAgent.includes('Quest')) ||
                            userAgent.toLowerCase().includes('meta quest');


    const forceQuest3 = window.location.search.includes('force=quest3') ||
                        window.location.search.includes('directar=true') ||
                        window.location.search.includes('immersive=true');

    return (isQuest3Browser || forceQuest3 || shouldUseQuest3Layout) && initialized;
  };

  useEffect(() => {
    
    if (initialized) {
      initializeCommandPalette();
      registerSettingsCommands();
      registerSettingsHelp();
      registerOnboardingCommands();

      const hasVisited = localStorage.getItem('hasVisited');
      if (!hasVisited) {
        localStorage.setItem('hasVisited', 'true');
        setTimeout(() => {
          window.dispatchEvent(new CustomEvent('start-onboarding', {
            detail: { flowId: 'welcome' }
          }));
        }, 1000);
      }
    }
  }, [initialized])

  const handleInitialized = useCallback(() => {
    setInitializationState('initialized');
    const settings = useSettingsStore.getState().settings;
    const debugEnabled = settings?.system?.debug?.enabled === true;
    if (debugEnabled) {
      logger.debug('Application initialized');
      logger.debug('Bots WebSocket connection status:', botsConnectionStatus);
    }
  }, [botsConnectionStatus]);

  const handleInitializationError = useCallback((error: Error) => {
    setInitializationError(error);
    setInitializationState('error');
  }, []);

  // Show loading screen while checking auth
  if (isAuthLoading) {
    return <LoadingScreen message="Checking authentication..." />;
  }

  // Allow bypass for visual testing via URL parameter (DEVELOPMENT ONLY)
  const isDevelopment = process.env.NODE_ENV === 'development';
  const skipAuth = isDevelopment && (
    window.location.search.includes('skipAuth=true') ||
    window.location.search.includes('test=visual')
  );

  // Show login screen if not authenticated (unless testing bypass in dev mode)
  if (!authenticated && !skipAuth) {
    return <OnboardingWizard onComplete={() => {
      // Auth state update happens inside the wizard via nostrAuth
      // React re-renders automatically via useNostrAuth subscription
    }} />;
  }

  const renderContent = () => {
    switch (initializationState) {
      case 'loading':
        return <LoadingScreen message="Connecting to server..." />;
      case 'error':
        return (
          <div>
            <h2>Error Initializing Application</h2>
            <p>{initializationError?.message || 'An unknown error occurred.'}</p>
            <button onClick={() => window.location.reload()}>Retry</button>
          </div>
        );
      case 'initialized':
        return shouldUseImmersiveClient() ? (
          <BotsDataProvider>
            <VircadiaBridgesProvider enableBotsBridge={true} enableGraphBridge={true}>
              <Suspense fallback={<LoadingScreen message="Loading immersive client..." />}>
                <ImmersiveApp />
              </Suspense>
            </VircadiaBridgesProvider>
          </BotsDataProvider>
        ) : (
          <BotsDataProvider>
            <VircadiaBridgesProvider enableBotsBridge={true} enableGraphBridge={false}>
              <MainLayout />
            </VircadiaBridgesProvider>
          </BotsDataProvider>
        );
    }
  };

  return (
    <VircadiaProvider autoConnect={false}>
      <a href="#main-content" className="sr-only focus:not-sr-only focus:absolute focus:z-50 focus:p-4 focus:bg-white focus:text-black">Skip to graph</a>
      <TooltipProvider delayDuration={300} skipDelayDuration={100}>
        <HelpProvider>
          <OnboardingProvider>
            <ErrorBoundary>
              <ApplicationModeProvider>
                <div id="main-content">
                {renderContent()}
                </div>
                {initializationState === 'loading' && (
                  <AppInitializer onInitialized={handleInitialized} onError={handleInitializationError} />
                )}
                {initializationState === 'initialized' && (
                  <>
                    <ConnectionWarning />
                    <CommandPalette />
                    <DebugControlPanel />
                    <WorkerErrorModal />
                  </>
                )}
              </ApplicationModeProvider>
            </ErrorBoundary>
          </OnboardingProvider>
        </HelpProvider>
      </TooltipProvider>
    </VircadiaProvider>
  );
}

export default App
